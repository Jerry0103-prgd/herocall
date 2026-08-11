//! Research-agent preparation boundary. The model receives only this service's evidence context;
//! it never fetches arbitrary web content itself.

use std::{error::Error, fmt, process::Command};

use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;

use crate::{
    database::service::{DatabaseService, StoredPriceHistoryBar},
    market_service::MarketSecurity,
};

const EASTMONEY_HISTORY_URL: &str = "https://push2his.eastmoney.com/api/qt/stock/kline/get";

pub trait PriceHistoryAdapter {
    fn source(&self) -> &'static str;
    fn fetch(
        &self,
        symbol: &str,
        market: &str,
        trading_days: usize,
    ) -> Result<Vec<PriceHistoryBar>, PriceHistoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceHistoryError(pub String);
impl fmt::Display for PriceHistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for PriceHistoryError {}

pub struct EastmoneyPriceHistoryAdapter;
impl PriceHistoryAdapter for EastmoneyPriceHistoryAdapter {
    fn source(&self) -> &'static str {
        "东方财富历史行情"
    }
    fn fetch(
        &self,
        symbol: &str,
        market: &str,
        trading_days: usize,
    ) -> Result<Vec<PriceHistoryBar>, PriceHistoryError> {
        if !matches!(trading_days, 20 | 60) {
            return Err(PriceHistoryError("历史行情窗口仅支持20或60个交易日".into()));
        }
        let prefix = match market {
            "SSE" => "1",
            "SZSE" => "0",
            _ if symbol.starts_with('6') => "1",
            _ if symbol.starts_with('0') || symbol.starts_with('3') => "0",
            _ => {
                return Err(PriceHistoryError(
                    "证券交易所未确认，无法查询历史行情".into(),
                ))
            }
        };
        let url = format!("{EASTMONEY_HISTORY_URL}?secid={prefix}.{symbol}&klt=101&fqt=1&lmt={trading_days}&fields1=f1,f2,f3,f4,f5,f6&fields2=f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61");
        let output = Command::new("curl")
            .args([
                "--fail",
                "--silent",
                "--show-error",
                "--max-time",
                "15",
                &url,
            ])
            .output()
            .map_err(|_| PriceHistoryError("历史行情网络请求失败".into()))?;
        if !output.status.success() {
            return Err(PriceHistoryError("历史行情数据源不可用".into()));
        }
        parse_eastmoney_history(&output.stdout)
    }
}

pub struct ResearchService;
impl ResearchService {
    /// Uses local source-backed bars first. Only when fewer than the requested number are stored
    /// does the trusted history adapter fetch a replacement set, which is persisted verbatim.
    pub fn ensure_price_history<A: PriceHistoryAdapter>(
        database: &DatabaseService,
        security: &MarketSecurity,
        trading_days: usize,
        adapter: &A,
    ) -> Result<Vec<PriceHistoryBar>, PriceHistoryError> {
        let cached = database
            .list_price_history(security.security_id, trading_days as i64)
            .map_err(|error| PriceHistoryError(error.to_string()))?;
        if cached.len() >= trading_days {
            return Ok(cached.into_iter().map(PriceHistoryBar::from).collect());
        }
        let fetched = adapter.fetch(&security.symbol, &security.market, trading_days)?;
        let now = chrono::Utc::now().to_rfc3339();
        database
            .upsert_price_history(
                &fetched
                    .iter()
                    .map(|bar| StoredPriceHistoryBar {
                        security_id: security.security_id,
                        trade_date: bar.trade_date.clone(),
                        open_price: bar.open.clone(),
                        high_price: bar.high.clone(),
                        low_price: bar.low.clone(),
                        close_price: bar.close.clone(),
                        volume: bar.volume.clone(),
                        amount: bar.amount.clone(),
                        change_percent: bar.change_percent.clone(),
                        source: adapter.source().into(),
                        market_timestamp: format!("{}T00:00:00+08:00", bar.trade_date),
                        fetched_at: now.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
            .map_err(|error| PriceHistoryError(error.to_string()))?;
        Ok(database
            .list_price_history(security.security_id, trading_days as i64)
            .map_err(|error| PriceHistoryError(error.to_string()))?
            .into_iter()
            .map(PriceHistoryBar::from)
            .collect())
    }
}

impl From<StoredPriceHistoryBar> for PriceHistoryBar {
    fn from(value: StoredPriceHistoryBar) -> Self {
        Self {
            trade_date: value.trade_date,
            open: value.open_price,
            high: value.high_price,
            low: value.low_price,
            close: value.close_price,
            volume: value.volume,
            amount: value.amount,
            change_percent: value.change_percent,
        }
    }
}

fn parse_eastmoney_history(body: &[u8]) -> Result<Vec<PriceHistoryBar>, PriceHistoryError> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| PriceHistoryError("历史行情返回格式错误".into()))?;
    let rows = value
        .pointer("/data/klines")
        .and_then(Value::as_array)
        .ok_or_else(|| PriceHistoryError("历史行情无可用数据".into()))?;
    rows.iter()
        .map(|row| {
            let values = row
                .as_str()
                .unwrap_or_default()
                .split(',')
                .collect::<Vec<_>>();
            if values.len() < 9 {
                return Err(PriceHistoryError("历史行情字段不完整".into()));
            }
            Ok(PriceHistoryBar {
                trade_date: values[0].into(),
                open: values[1].into(),
                close: values[2].into(),
                high: values[3].into(),
                low: values[4].into(),
                volume: values[5].into(),
                amount: values[6].into(),
                change_percent: (!values[8].is_empty()).then(|| values[8].into()),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PriceHistoryBar {
    pub trade_date: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub amount: String,
    pub change_percent: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalSnapshot {
    pub status: String,
    pub ma5: Option<String>,
    pub ma10: Option<String>,
    pub ma20: Option<String>,
    pub change_5d: Option<String>,
    pub change_10d: Option<String>,
    pub change_20d: Option<String>,
    pub high_20d: Option<String>,
    pub low_20d: Option<String>,
    pub average_volume_5d: Option<String>,
    pub current_volume_vs_average_5d: Option<String>,
    pub current_price_vs_ma5: Option<String>,
    pub current_price_vs_ma10: Option<String>,
    pub current_price_vs_ma20: Option<String>,
}

/// Calculates technical facts from ascending (oldest-to-newest) source-backed daily bars only.
/// No result is synthesized when the required history is missing.
pub fn calculate_technical_snapshot(bars: &[PriceHistoryBar]) -> TechnicalSnapshot {
    let parsed = bars
        .iter()
        .map(|bar| {
            Some((
                parse(&bar.close)?,
                parse(&bar.high)?,
                parse(&bar.low)?,
                parse(&bar.volume)?,
            ))
        })
        .collect::<Option<Vec<_>>>();
    let Some(values) = parsed else {
        return unavailable();
    };
    if values.len() < 20 {
        return unavailable();
    }
    let closes = values.iter().map(|value| value.0).collect::<Vec<_>>();
    let volumes = values.iter().map(|value| value.3).collect::<Vec<_>>();
    let current = *closes.last().expect("20 bars guarantees one close");
    let ma = |days: usize| average(&closes[closes.len() - days..]);
    let percent_change = |days: usize| percent(current, closes[closes.len() - days]);
    let high_20 = values[values.len() - 20..]
        .iter()
        .map(|value| value.1)
        .max();
    let low_20 = values[values.len() - 20..]
        .iter()
        .map(|value| value.2)
        .min();
    let avg_volume = average(&volumes[volumes.len() - 5..]);
    TechnicalSnapshot {
        status: "AVAILABLE".into(),
        ma5: Some(format_decimal(ma(5))),
        ma10: Some(format_decimal(ma(10))),
        ma20: Some(format_decimal(ma(20))),
        change_5d: Some(format_decimal(percent_change(5))),
        change_10d: Some(format_decimal(percent_change(10))),
        change_20d: Some(format_decimal(percent_change(20))),
        high_20d: high_20.map(format_decimal),
        low_20d: low_20.map(format_decimal),
        average_volume_5d: Some(format_decimal(avg_volume)),
        current_volume_vs_average_5d: Some(format_decimal(percent(
            volumes[volumes.len() - 1],
            avg_volume,
        ))),
        current_price_vs_ma5: Some(format_decimal(percent(current, ma(5)))),
        current_price_vs_ma10: Some(format_decimal(percent(current, ma(10)))),
        current_price_vs_ma20: Some(format_decimal(percent(current, ma(20)))),
    }
}

fn unavailable() -> TechnicalSnapshot {
    TechnicalSnapshot {
        status: "INSUFFICIENT_HISTORY".into(),
        ma5: None,
        ma10: None,
        ma20: None,
        change_5d: None,
        change_10d: None,
        change_20d: None,
        high_20d: None,
        low_20d: None,
        average_volume_5d: None,
        current_volume_vs_average_5d: None,
        current_price_vs_ma5: None,
        current_price_vs_ma10: None,
        current_price_vs_ma20: None,
    }
}
fn parse(value: &str) -> Option<Decimal> {
    value.parse().ok()
}
fn average(values: &[Decimal]) -> Decimal {
    values.iter().copied().sum::<Decimal>() / Decimal::from(values.len())
}
fn percent(current: Decimal, base: Decimal) -> Decimal {
    if base.is_zero() {
        Decimal::ZERO
    } else {
        (current - base) * Decimal::from(100) / base
    }
}
fn format_decimal(value: Decimal) -> String {
    value.round_dp(4).normalize().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn technical_snapshot_is_calculated_from_twenty_real_bars() {
        let bars = (1..=20)
            .map(|day| PriceHistoryBar {
                trade_date: format!("2026-08-{day:02}"),
                open: day.to_string(),
                high: (day + 1).to_string(),
                low: (day - 1).to_string(),
                close: day.to_string(),
                volume: (day * 10).to_string(),
                amount: "0".into(),
                change_percent: None,
            })
            .collect::<Vec<_>>();
        let snapshot = calculate_technical_snapshot(&bars);
        assert_eq!(snapshot.status, "AVAILABLE");
        assert_eq!(snapshot.ma5.as_deref(), Some("18"));
        assert_eq!(snapshot.ma20.as_deref(), Some("10.5"));
        assert_eq!(snapshot.high_20d.as_deref(), Some("21"));
    }
    #[test]
    fn insufficient_history_is_explicit() {
        assert_eq!(
            calculate_technical_snapshot(&[]).status,
            "INSUFFICIENT_HISTORY"
        );
    }

    #[test]
    fn eastmoney_history_parser_preserves_source_fields_without_inference() {
        let bars = parse_eastmoney_history(
            br#"{"data":{"klines":["2026-08-07,10.00,10.50,10.80,9.90,1234,5678,0,5.00,0,0"]}}"#,
        )
        .unwrap();
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].trade_date, "2026-08-07");
        assert_eq!(bars[0].close, "10.50");
        assert_eq!(bars[0].change_percent.as_deref(), Some("5.00"));
    }
}
