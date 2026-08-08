//! Market data application service.
//!
//! The module owns adapter contracts, canonical quote normalization and snapshot persistence
//! ports. It never manufactures a quote: a failed, incomplete or unverified provider response
//! becomes a `NO_DATA` snapshot instead of a substitute price.

use std::{
    collections::HashMap,
    env,
    error::Error,
    fmt,
    io::Write,
    process::{Command, Stdio},
};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::{json, Value};

const TUSHARE_API_URL: &str = "https://api.tushare.pro";
const EASTMONEY_QUOTE_URL: &str = "https://push2.eastmoney.com/api/qt/ulist.np/get";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketStatus {
    Realtime,
    Delayed,
    Closed,
    NoData,
}

impl MarketStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Realtime => "REALTIME",
            Self::Delayed => "DELAYED",
            Self::Closed => "CLOSED",
            Self::NoData => "NO_DATA",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketPhase {
    /// A confirmed trading session. The calendar service will supply this value in a later phase.
    Trading,
    Closed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceClass {
    /// A provider contract explicitly licenses real-time use.
    AuthorizedRealtime,
    /// A public quote endpoint. It is always labelled delayed during a trading session.
    PublicQuote,
    /// A valid end-of-day record, not an intraday quote.
    EndOfDay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSourcePriority {
    PrimaryApi,
    PublicQuote,
    Backup,
}

impl DataSourcePriority {
    pub fn as_i64(self) -> i64 {
        match self {
            Self::PrimaryApi => 1,
            Self::PublicQuote => 2,
            Self::Backup => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketDataSource {
    pub name: String,
    pub base_url: String,
    pub priority: DataSourcePriority,
    pub source_class: SourceClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketSecurity {
    pub security_id: i64,
    pub symbol: String,
    pub name: String,
    pub market: String,
}

#[derive(Debug, Clone)]
pub struct MarketFetchRequest {
    pub securities: Vec<MarketSecurity>,
    pub market_phase: MarketPhase,
    /// Supplied by the caller to keep source-time and local retrieval-time independently traceable.
    pub fetched_at: DateTime<Utc>,
}

impl MarketFetchRequest {
    pub fn now(securities: Vec<MarketSecurity>, market_phase: MarketPhase) -> Self {
        Self {
            securities,
            market_phase,
            fetched_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMarketQuote {
    pub security: MarketSecurity,
    pub current_price: Decimal,
    pub previous_close: Decimal,
    pub price_change: Decimal,
    pub change_percent: Decimal,
    pub volume: Decimal,
    pub volume_unit: String,
    pub turnover_amount: Decimal,
    pub turnover_unit: String,
    pub market_timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketQuote {
    pub security_id: i64,
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub current_price: Decimal,
    pub previous_close: Decimal,
    pub price_change: Decimal,
    pub change_percent: Decimal,
    pub volume: Decimal,
    pub volume_unit: String,
    pub turnover_amount: Decimal,
    pub turnover_unit: String,
    pub market_timestamp: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
    pub source: String,
    pub delay_status: MarketStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketSnapshot {
    pub source: MarketDataSource,
    pub market_timestamp: Option<DateTime<Utc>>,
    pub fetched_at: DateTime<Utc>,
    pub delay_status: MarketStatus,
    pub quotes: Vec<MarketQuote>,
    /// Safe diagnostic text. It must never contain an API key or raw request authorization.
    pub unavailable_reason: Option<String>,
}

impl MarketSnapshot {
    fn no_data(source: MarketDataSource, fetched_at: DateTime<Utc>, reason: String) -> Self {
        Self {
            source,
            market_timestamp: None,
            fetched_at,
            delay_status: MarketStatus::NoData,
            quotes: Vec::new(),
            unavailable_reason: Some(reason),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketAdapterError {
    Configuration(&'static str),
    Unavailable(String),
    InvalidResponse(String),
    NoData,
}

impl fmt::Display for MarketAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(message) => {
                write!(formatter, "adapter configuration error: {message}")
            }
            Self::Unavailable(message) => write!(formatter, "data source unavailable: {message}"),
            Self::InvalidResponse(message) => {
                write!(formatter, "invalid data source response: {message}")
            }
            Self::NoData => write!(formatter, "data source returned no usable quote"),
        }
    }
}

impl Error for MarketAdapterError {}

/// A provider-specific transport and mapping boundary. Adapters return canonical raw records but
/// do not assign a user-facing freshness status; that happens once in `MarketDataNormalizer`.
pub trait MarketDataAdapter {
    fn source(&self) -> MarketDataSource;
    fn fetch(
        &self,
        request: &MarketFetchRequest,
    ) -> Result<Vec<RawMarketQuote>, MarketAdapterError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketStoreError {
    pub message: String,
}

impl fmt::Display for MarketStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for MarketStoreError {}

/// Port implemented by the SQLite infrastructure. The Market Service never needs a direct
/// connection or a UI dependency to persist a snapshot.
pub trait MarketSnapshotStore {
    fn save_market_snapshot(&self, snapshot: &MarketSnapshot) -> Result<(), MarketStoreError>;
}

pub struct MarketService;

impl MarketService {
    /// Only verified real-time or completed-market data may be used by automatic valuation.
    /// Public delayed quotes remain display-only until a user-facing valuation policy exists.
    pub fn is_usable_for_valuation(status: MarketStatus) -> bool {
        matches!(status, MarketStatus::Realtime | MarketStatus::Closed)
    }

    pub fn fetch_snapshot<A: MarketDataAdapter + ?Sized>(
        adapter: &A,
        request: &MarketFetchRequest,
    ) -> MarketSnapshot {
        let source = adapter.source();
        match adapter.fetch(request) {
            Ok(raw_quotes) if raw_quotes.is_empty() => MarketSnapshot::no_data(
                source,
                request.fetched_at,
                "data source returned no usable quote".into(),
            ),
            Ok(raw_quotes) => MarketDataNormalizer::normalize(
                source,
                raw_quotes,
                request.fetched_at,
                request.market_phase,
            ),
            Err(error) => MarketSnapshot::no_data(source, request.fetched_at, error.to_string()),
        }
    }

    pub fn fetch_and_store<A: MarketDataAdapter, S: MarketSnapshotStore>(
        adapter: &A,
        request: &MarketFetchRequest,
        store: &S,
    ) -> Result<MarketSnapshot, MarketStoreError> {
        let snapshot = Self::fetch_snapshot(adapter, request);
        store.save_market_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    /// Uses providers in priority order. A provider is only bypassed after it returns `NO_DATA`;
    /// the selected snapshot always retains the actual provider name and its own delay status.
    pub fn fetch_with_fallback(
        adapters: &[&dyn MarketDataAdapter],
        request: &MarketFetchRequest,
    ) -> MarketSnapshot {
        let mut last_snapshot = None;
        for adapter in adapters {
            let snapshot = Self::fetch_snapshot(*adapter, request);
            if snapshot.delay_status != MarketStatus::NoData {
                return snapshot;
            }
            last_snapshot = Some(snapshot);
        }
        last_snapshot.unwrap_or_else(|| {
            MarketSnapshot::no_data(
                MarketDataSource {
                    name: "未配置数据源".into(),
                    base_url: String::new(),
                    priority: DataSourcePriority::Backup,
                    source_class: SourceClass::PublicQuote,
                },
                request.fetched_at,
                "no market data adapter is configured".into(),
            )
        })
    }
}

pub struct MarketDataNormalizer;

impl MarketDataNormalizer {
    pub fn normalize(
        source: MarketDataSource,
        raw_quotes: Vec<RawMarketQuote>,
        fetched_at: DateTime<Utc>,
        market_phase: MarketPhase,
    ) -> MarketSnapshot {
        let quotes: Vec<MarketQuote> = raw_quotes
            .into_iter()
            .filter_map(|raw| {
                if !has_usable_quote_values(&raw) {
                    return None;
                }
                let status = determine_status(
                    source.source_class,
                    market_phase,
                    raw.market_timestamp,
                    fetched_at,
                );
                (status != MarketStatus::NoData).then(|| MarketQuote {
                    security_id: raw.security.security_id,
                    symbol: raw.security.symbol,
                    name: raw.security.name,
                    market: raw.security.market,
                    current_price: raw.current_price,
                    previous_close: raw.previous_close,
                    price_change: raw.price_change,
                    change_percent: raw.change_percent,
                    volume: raw.volume,
                    volume_unit: raw.volume_unit,
                    turnover_amount: raw.turnover_amount,
                    turnover_unit: raw.turnover_unit,
                    market_timestamp: raw.market_timestamp,
                    fetched_at,
                    source: source.name.clone(),
                    delay_status: status,
                })
            })
            .collect();

        if quotes.is_empty() {
            return MarketSnapshot::no_data(
                source,
                fetched_at,
                "quote timestamp is missing, invalid or stale".into(),
            );
        }

        let market_timestamp = quotes.iter().map(|quote| quote.market_timestamp).max();
        let delay_status = aggregate_status(&quotes);
        MarketSnapshot {
            source,
            market_timestamp,
            fetched_at,
            delay_status,
            quotes,
            unavailable_reason: None,
        }
    }
}

fn has_usable_quote_values(quote: &RawMarketQuote) -> bool {
    quote.current_price > Decimal::ZERO
        && quote.previous_close > Decimal::ZERO
        && quote.volume >= Decimal::ZERO
        && quote.turnover_amount >= Decimal::ZERO
}

/// Applies the V1 freshness baseline. Market phase must be provided by a verified trading calendar;
/// when unavailable, public data stays delayed and an authorized source is never promoted to realtime.
pub fn determine_status(
    source_class: SourceClass,
    market_phase: MarketPhase,
    market_timestamp: DateTime<Utc>,
    fetched_at: DateTime<Utc>,
) -> MarketStatus {
    let age = fetched_at.signed_duration_since(market_timestamp);
    if age < chrono::Duration::minutes(-2) {
        return MarketStatus::NoData;
    }

    match source_class {
        SourceClass::EndOfDay => MarketStatus::Closed,
        SourceClass::PublicQuote => match market_phase {
            MarketPhase::Closed => MarketStatus::Closed,
            MarketPhase::Trading | MarketPhase::Unknown => MarketStatus::Delayed,
        },
        SourceClass::AuthorizedRealtime => match market_phase {
            MarketPhase::Trading if age <= chrono::Duration::minutes(1) => MarketStatus::Realtime,
            MarketPhase::Trading if age <= chrono::Duration::minutes(5) => MarketStatus::Delayed,
            MarketPhase::Trading => MarketStatus::NoData,
            MarketPhase::Closed => MarketStatus::Closed,
            MarketPhase::Unknown => MarketStatus::Delayed,
        },
    }
}

fn aggregate_status(quotes: &[MarketQuote]) -> MarketStatus {
    if quotes
        .iter()
        .all(|quote| quote.delay_status == MarketStatus::Realtime)
    {
        MarketStatus::Realtime
    } else if quotes
        .iter()
        .all(|quote| quote.delay_status == MarketStatus::Closed)
    {
        MarketStatus::Closed
    } else {
        MarketStatus::Delayed
    }
}

/// Tushare Pro adapter. It reads `TUSHARE_TOKEN` only at runtime and uses the daily endpoint,
/// therefore all usable records are labelled `CLOSED`, never real-time.
pub struct TushareAdapter {}

impl TushareAdapter {
    pub fn new() -> Result<Self, MarketAdapterError> {
        Ok(Self {})
    }

    fn token() -> Result<String, MarketAdapterError> {
        env::var("TUSHARE_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or(MarketAdapterError::Configuration(
                "TUSHARE_TOKEN is not configured",
            ))
    }
}

impl MarketDataAdapter for TushareAdapter {
    fn source(&self) -> MarketDataSource {
        MarketDataSource {
            name: "Tushare".into(),
            base_url: TUSHARE_API_URL.into(),
            priority: DataSourcePriority::PrimaryApi,
            source_class: SourceClass::EndOfDay,
        }
    }

    fn fetch(
        &self,
        request: &MarketFetchRequest,
    ) -> Result<Vec<RawMarketQuote>, MarketAdapterError> {
        let token = Self::token()?;
        let mut quotes = Vec::with_capacity(request.securities.len());
        for security in &request.securities {
            let ts_code = to_tushare_code(security)?;
            // The token only exists in this JSON stdin payload; it is never an executable argument.
            let body = json!({
                "api_name": "daily",
                "token": token,
                "params": { "ts_code": ts_code },
                "fields": "ts_code,trade_date,close,pre_close,change,pct_chg,vol,amount"
            });
            let payload = curl_json("POST", TUSHARE_API_URL, Some(&body))?;
            quotes.extend(parse_tushare_daily(&payload, security)?);
        }
        (!quotes.is_empty())
            .then_some(quotes)
            .ok_or(MarketAdapterError::NoData)
    }
}

/// Eastmoney's public quote adapter. It intentionally uses `PublicQuote`, so a quote is never
/// advertised as real-time without a separate lawful real-time license.
pub struct EastmoneyAdapter {}

impl EastmoneyAdapter {
    pub fn new() -> Result<Self, MarketAdapterError> {
        Ok(Self {})
    }
}

impl MarketDataAdapter for EastmoneyAdapter {
    fn source(&self) -> MarketDataSource {
        MarketDataSource {
            name: "东方财富公开行情".into(),
            base_url: EASTMONEY_QUOTE_URL.into(),
            priority: DataSourcePriority::PublicQuote,
            source_class: SourceClass::PublicQuote,
        }
    }

    fn fetch(
        &self,
        request: &MarketFetchRequest,
    ) -> Result<Vec<RawMarketQuote>, MarketAdapterError> {
        if request.securities.is_empty() {
            return Err(MarketAdapterError::NoData);
        }
        let secids = request
            .securities
            .iter()
            .map(to_eastmoney_secid)
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        let url = format!(
            "{EASTMONEY_QUOTE_URL}?fltt=2&invt=2&secids={secids}&fields=f2,f3,f4,f5,f6,f12,f14,f18,f124"
        );
        let payload = curl_json("GET", &url, None)?;
        parse_eastmoney_quotes(&payload, &request.securities)
    }
}

/// Minimal cross-platform HTTPS transport using the system `curl` executable. macOS and current
/// supported Windows releases include curl. Tushare request bodies are written through stdin, so
/// keys cannot leak through process arguments or error diagnostics.
fn curl_json(method: &str, url: &str, body: Option<&Value>) -> Result<Value, MarketAdapterError> {
    let mut command = Command::new("curl");
    command
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "15",
            "-X",
            method,
        ])
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if body.is_some() {
        command
            .args([
                "--header",
                "Content-Type: application/json",
                "--data-binary",
                "@-",
            ])
            .stdin(Stdio::piped());
    }
    let mut child = command.spawn().map_err(|_| {
        MarketAdapterError::Unavailable("system HTTP transport is unavailable".into())
    })?;
    if let Some(body) = body {
        let bytes = serde_json::to_vec(body).map_err(|_| {
            MarketAdapterError::InvalidResponse("request JSON serialization failed".into())
        })?;
        child
            .stdin
            .as_mut()
            .ok_or_else(|| {
                MarketAdapterError::Unavailable("system HTTP transport is unavailable".into())
            })?
            .write_all(&bytes)
            .map_err(|_| {
                MarketAdapterError::Unavailable("data source request could not be sent".into())
            })?;
    }
    let output = child
        .wait_with_output()
        .map_err(|_| MarketAdapterError::Unavailable("data source request failed".into()))?;
    if !output.status.success() {
        return Err(MarketAdapterError::Unavailable(
            "data source request failed".into(),
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|_| MarketAdapterError::InvalidResponse("data source response is not JSON".into()))
}

fn to_tushare_code(security: &MarketSecurity) -> Result<String, MarketAdapterError> {
    let exchange = match security.market.as_str() {
        "SSE" | "SH" => "SH",
        "SZSE" | "SZ" => "SZ",
        _ => {
            return Err(MarketAdapterError::InvalidResponse(
                "unsupported security market".into(),
            ))
        }
    };
    validate_symbol(&security.symbol)?;
    Ok(format!("{}.{}", security.symbol, exchange))
}

fn to_eastmoney_secid(security: &MarketSecurity) -> Result<String, MarketAdapterError> {
    let exchange = match security.market.as_str() {
        "SSE" | "SH" => "1",
        "SZSE" | "SZ" => "0",
        _ => {
            return Err(MarketAdapterError::InvalidResponse(
                "unsupported security market".into(),
            ))
        }
    };
    validate_symbol(&security.symbol)?;
    Ok(format!("{exchange}.{}", security.symbol))
}

fn validate_symbol(symbol: &str) -> Result<(), MarketAdapterError> {
    (symbol.len() == 6 && symbol.bytes().all(|byte| byte.is_ascii_digit()))
        .then_some(())
        .ok_or(MarketAdapterError::InvalidResponse(
            "invalid A-share symbol".into(),
        ))
}

fn parse_tushare_daily(
    payload: &Value,
    security: &MarketSecurity,
) -> Result<Vec<RawMarketQuote>, MarketAdapterError> {
    if payload
        .get("code")
        .and_then(Value::as_i64)
        .unwrap_or_default()
        != 0
    {
        return Err(MarketAdapterError::Unavailable(
            "Tushare rejected the request".into(),
        ));
    }
    let fields = payload
        .pointer("/data/fields")
        .and_then(Value::as_array)
        .ok_or_else(|| MarketAdapterError::InvalidResponse("Tushare fields are missing".into()))?;
    let indexes = field_indexes(fields)?;
    let items = payload
        .pointer("/data/items")
        .and_then(Value::as_array)
        .ok_or(MarketAdapterError::NoData)?;
    items
        .iter()
        .take(1)
        .map(|item| parse_tushare_row(item, &indexes, security))
        .collect()
}

fn parse_tushare_row(
    row: &Value,
    indexes: &HashMap<&str, usize>,
    security: &MarketSecurity,
) -> Result<RawMarketQuote, MarketAdapterError> {
    let cells = row
        .as_array()
        .ok_or_else(|| MarketAdapterError::InvalidResponse("Tushare row is invalid".into()))?;
    let decimal = |field| decimal_from_value(tushare_cell(cells, indexes, field)?);
    let trade_date = string_from_value(tushare_cell(cells, indexes, "trade_date")?)?;
    Ok(RawMarketQuote {
        security: security.clone(),
        current_price: decimal("close")?,
        previous_close: decimal("pre_close")?,
        price_change: decimal("change")?,
        change_percent: decimal("pct_chg")?,
        volume: decimal("vol")?,
        volume_unit: "LOTS".into(),
        turnover_amount: decimal("amount")?,
        turnover_unit: "THOUSAND_CNY".into(),
        market_timestamp: close_timestamp_from_trade_date(&trade_date)?,
    })
}

fn parse_eastmoney_quotes(
    payload: &Value,
    securities: &[MarketSecurity],
) -> Result<Vec<RawMarketQuote>, MarketAdapterError> {
    let records = payload
        .pointer("/data/diff")
        .and_then(Value::as_array)
        .ok_or(MarketAdapterError::NoData)?;
    let by_symbol: HashMap<&str, &MarketSecurity> = securities
        .iter()
        .map(|security| (security.symbol.as_str(), security))
        .collect();
    let quotes: Result<Vec<_>, _> = records
        .iter()
        .map(|record| {
            let symbol = string_from_value(field(record, "f12")?)?;
            let security = by_symbol.get(symbol.as_str()).ok_or_else(|| {
                MarketAdapterError::InvalidResponse("response has an unexpected symbol".into())
            })?;
            let timestamp = field(record, "f124")?
                .as_i64()
                .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
                .ok_or_else(|| {
                    MarketAdapterError::InvalidResponse(
                        "Eastmoney quote timestamp is missing".into(),
                    )
                })?;
            Ok(RawMarketQuote {
                security: (*security).clone(),
                current_price: decimal_from_value(field(record, "f2")?)?,
                previous_close: decimal_from_value(field(record, "f18")?)?,
                price_change: decimal_from_value(field(record, "f4")?)?,
                change_percent: decimal_from_value(field(record, "f3")?)?,
                volume: decimal_from_value(field(record, "f5")?)?,
                volume_unit: "SOURCE_DECLARED".into(),
                turnover_amount: decimal_from_value(field(record, "f6")?)?,
                turnover_unit: "SOURCE_DECLARED".into(),
                market_timestamp: timestamp,
            })
        })
        .collect();
    let quotes = quotes?;
    (!quotes.is_empty())
        .then_some(quotes)
        .ok_or(MarketAdapterError::NoData)
}

fn field_indexes(fields: &[Value]) -> Result<HashMap<&str, usize>, MarketAdapterError> {
    let mut indexes = HashMap::new();
    for (index, field) in fields.iter().enumerate() {
        let name = field.as_str().ok_or_else(|| {
            MarketAdapterError::InvalidResponse("Tushare field is invalid".into())
        })?;
        indexes.insert(name, index);
    }
    for expected in [
        "trade_date",
        "close",
        "pre_close",
        "change",
        "pct_chg",
        "vol",
        "amount",
    ] {
        if !indexes.contains_key(expected) {
            return Err(MarketAdapterError::InvalidResponse(
                "Tushare response lacks a required field".into(),
            ));
        }
    }
    Ok(indexes)
}

fn tushare_cell<'a>(
    cells: &'a [Value],
    indexes: &HashMap<&str, usize>,
    field_name: &str,
) -> Result<&'a Value, MarketAdapterError> {
    let index = indexes.get(field_name).ok_or_else(|| {
        MarketAdapterError::InvalidResponse("Tushare required field is missing".into())
    })?;
    cells.get(*index).ok_or_else(|| {
        MarketAdapterError::InvalidResponse("Tushare row lacks a required value".into())
    })
}

fn field<'a>(record: &'a Value, name: &str) -> Result<&'a Value, MarketAdapterError> {
    record
        .get(name)
        .filter(|value| !value.is_null())
        .ok_or_else(|| {
            MarketAdapterError::InvalidResponse("provider response lacks a required value".into())
        })
}

fn decimal_from_value(value: &Value) -> Result<Decimal, MarketAdapterError> {
    let value = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        _ => {
            return Err(MarketAdapterError::InvalidResponse(
                "numeric value is invalid".into(),
            ))
        }
    };
    value
        .parse()
        .map_err(|_| MarketAdapterError::InvalidResponse("numeric value is invalid".into()))
}

fn string_from_value(value: &Value) -> Result<String, MarketAdapterError> {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| MarketAdapterError::InvalidResponse("text value is invalid".into()))
}

fn close_timestamp_from_trade_date(trade_date: &str) -> Result<DateTime<Utc>, MarketAdapterError> {
    if trade_date.len() != 8 || !trade_date.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(MarketAdapterError::InvalidResponse(
            "Tushare trade date is invalid".into(),
        ));
    }
    let iso = format!(
        "{}-{}-{}T07:00:00Z",
        &trade_date[0..4],
        &trade_date[4..6],
        &trade_date[6..8]
    );
    DateTime::parse_from_rfc3339(&iso)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| MarketAdapterError::InvalidResponse("Tushare trade date is invalid".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn decimal(value: &str) -> Decimal {
        value.parse().expect("valid decimal fixture")
    }

    fn source(source_class: SourceClass) -> MarketDataSource {
        MarketDataSource {
            name: "Recorded test provider".into(),
            base_url: "https://test.invalid".into(),
            priority: DataSourcePriority::PublicQuote,
            source_class,
        }
    }

    fn security() -> MarketSecurity {
        MarketSecurity {
            security_id: 42,
            symbol: "600519".into(),
            name: "测试夹具证券".into(),
            market: "SSE".into(),
        }
    }

    fn raw_quote(market_timestamp: DateTime<Utc>) -> RawMarketQuote {
        RawMarketQuote {
            security: security(),
            current_price: decimal("1500"),
            previous_close: decimal("1490"),
            price_change: decimal("10"),
            change_percent: decimal("0.6711"),
            volume: decimal("100"),
            volume_unit: "LOTS".into(),
            turnover_amount: decimal("150000"),
            turnover_unit: "CNY".into(),
            market_timestamp,
        }
    }

    struct RecordedAdapter {
        response: Result<Vec<RawMarketQuote>, MarketAdapterError>,
    }

    impl MarketDataAdapter for RecordedAdapter {
        fn source(&self) -> MarketDataSource {
            // Recorded fixtures are public delayed test records, never simulated real-time data.
            source(SourceClass::PublicQuote)
        }

        fn fetch(&self, _: &MarketFetchRequest) -> Result<Vec<RawMarketQuote>, MarketAdapterError> {
            self.response.clone()
        }
    }

    fn request(at: DateTime<Utc>, market_phase: MarketPhase) -> MarketFetchRequest {
        MarketFetchRequest {
            securities: vec![security()],
            market_phase,
            fetched_at: at,
        }
    }

    #[test]
    fn adapter_contract_normalizes_required_quote_metadata() {
        let now = Utc.with_ymd_and_hms(2026, 8, 10, 2, 0, 30).unwrap();
        let adapter = RecordedAdapter {
            response: Ok(vec![raw_quote(now - chrono::Duration::seconds(30))]),
        };

        let snapshot = MarketService::fetch_snapshot(&adapter, &request(now, MarketPhase::Trading));

        assert_eq!(snapshot.delay_status, MarketStatus::Delayed);
        assert_eq!(snapshot.quotes.len(), 1);
        let quote = &snapshot.quotes[0];
        assert_eq!(quote.symbol, "600519");
        assert_eq!(quote.name, "测试夹具证券");
        assert_eq!(quote.current_price, decimal("1500"));
        assert_eq!(quote.previous_close, decimal("1490"));
        assert_eq!(quote.price_change, decimal("10"));
        assert_eq!(quote.change_percent, decimal("0.6711"));
        assert_eq!(quote.source, "Recorded test provider");
        assert_eq!(quote.fetched_at, now);
    }

    #[test]
    fn adapter_failure_returns_no_data_without_a_substitute_quote() {
        let now = Utc.with_ymd_and_hms(2026, 8, 10, 2, 0, 0).unwrap();
        let adapter = RecordedAdapter {
            response: Err(MarketAdapterError::Unavailable(
                "test network outage".into(),
            )),
        };

        let snapshot = MarketService::fetch_snapshot(&adapter, &request(now, MarketPhase::Trading));

        assert_eq!(snapshot.delay_status, MarketStatus::NoData);
        assert!(snapshot.quotes.is_empty());
        assert!(snapshot.unavailable_reason.is_some());
    }

    #[test]
    fn empty_provider_response_returns_no_data() {
        let now = Utc.with_ymd_and_hms(2026, 8, 10, 2, 0, 0).unwrap();
        let adapter = RecordedAdapter {
            response: Ok(vec![]),
        };

        let snapshot = MarketService::fetch_snapshot(&adapter, &request(now, MarketPhase::Trading));

        assert_eq!(snapshot.delay_status, MarketStatus::NoData);
        assert!(snapshot.quotes.is_empty());
    }

    #[test]
    fn fallback_uses_the_next_adapter_but_preserves_its_actual_source() {
        let now = Utc.with_ymd_and_hms(2026, 8, 10, 2, 0, 0).unwrap();
        let unavailable = RecordedAdapter {
            response: Err(MarketAdapterError::NoData),
        };
        let fallback = RecordedAdapter {
            response: Ok(vec![raw_quote(now)]),
        };

        let snapshot = MarketService::fetch_with_fallback(
            &[&unavailable, &fallback],
            &request(now, MarketPhase::Trading),
        );

        assert_eq!(snapshot.delay_status, MarketStatus::Delayed);
        assert_eq!(snapshot.source.name, "Recorded test provider");
    }

    #[test]
    fn status_distinguishes_authorized_realtime_delayed_closed_and_stale() {
        let now = Utc.with_ymd_and_hms(2026, 8, 10, 2, 10, 0).unwrap();
        assert_eq!(
            determine_status(
                SourceClass::AuthorizedRealtime,
                MarketPhase::Trading,
                now - chrono::Duration::seconds(59),
                now
            ),
            MarketStatus::Realtime
        );
        assert_eq!(
            determine_status(
                SourceClass::AuthorizedRealtime,
                MarketPhase::Trading,
                now - chrono::Duration::minutes(2),
                now
            ),
            MarketStatus::Delayed
        );
        assert_eq!(
            determine_status(
                SourceClass::AuthorizedRealtime,
                MarketPhase::Trading,
                now - chrono::Duration::minutes(6),
                now
            ),
            MarketStatus::NoData
        );
        assert_eq!(
            determine_status(SourceClass::PublicQuote, MarketPhase::Trading, now, now),
            MarketStatus::Delayed
        );
        assert_eq!(
            determine_status(
                SourceClass::EndOfDay,
                MarketPhase::Trading,
                now - chrono::Duration::days(1),
                now
            ),
            MarketStatus::Closed
        );
    }

    #[test]
    fn eastmoney_recorded_response_maps_to_canonical_quote() {
        let response: Value = serde_json::from_str(
            r#"{"data":{"diff":[{"f2":1500.5,"f3":0.67,"f4":10,"f5":100,"f6":150000,"f12":"600519","f14":"测试夹具证券","f18":1490.5,"f124":1786327200}]}}"#,
        )
        .unwrap();

        let quotes = parse_eastmoney_quotes(&response, &[security()]).unwrap();

        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].current_price, decimal("1500.5"));
        assert_eq!(quotes[0].previous_close, decimal("1490.5"));
        assert_eq!(
            quotes[0].market_timestamp,
            Utc.timestamp_opt(1786327200, 0).unwrap()
        );
    }

    #[test]
    fn tushare_recorded_daily_response_maps_to_closed_quote_fields() {
        let response: Value = serde_json::from_str(
            r#"{"code":0,"data":{"fields":["ts_code","trade_date","close","pre_close","change","pct_chg","vol","amount"],"items":[["600519.SH","20260810",1500.5,1490.5,10,0.67,100,150000]]}}"#,
        )
        .unwrap();

        let quotes = parse_tushare_daily(&response, &security()).unwrap();
        let adapter = TushareAdapter::new().unwrap();

        assert_eq!(adapter.source().source_class, SourceClass::EndOfDay);
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].current_price, decimal("1500.5"));
        assert_eq!(quotes[0].previous_close, decimal("1490.5"));
        assert_eq!(quotes[0].volume_unit, "LOTS");
        assert_eq!(quotes[0].turnover_unit, "THOUSAND_CNY");
    }
}
