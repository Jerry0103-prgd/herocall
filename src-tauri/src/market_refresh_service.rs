//! Application boundary for user-triggered, source-backed market refreshes.
//!
//! It selects only existing holdings, delegates all external work to a `MarketDataAdapter`, and
//! persists the resulting canonical snapshot through `MarketService`. A missing Tushare token is
//! an explicit unconfigured state, never a reason to fabricate a quote or use a hidden fallback.

use std::{error::Error, fmt};

use serde::Serialize;

use crate::{
    database::service::{DatabaseError, DatabaseService},
    market_service::{
        EastmoneyAdapter, MarketDataAdapter, MarketFetchRequest, MarketPhase, MarketSecurity,
        MarketService, MarketSnapshot, MarketSnapshotStore, MarketStoreError, TencentAdapter,
        TushareAdapter,
    },
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketRefreshView {
    pub source: String,
    pub configuration_status: String,
    pub status: String,
    pub quote_count: usize,
    pub market_timestamp: Option<String>,
    pub fetched_at: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSectionView {
    pub source: String,
    pub status: String,
    pub item_count: usize,
    pub updated_at: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManualMarketSnapshotView {
    pub holdings: MarketRefreshView,
    pub indices: SnapshotSectionView,
    pub news: SnapshotSectionView,
    pub events: SnapshotSectionView,
}

#[derive(Debug)]
pub enum MarketRefreshError {
    Database(DatabaseError),
    Store(MarketStoreError),
}

impl fmt::Display for MarketRefreshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::Store(error) => write!(formatter, "market snapshot storage error: {error}"),
        }
    }
}

impl Error for MarketRefreshError {}

impl From<DatabaseError> for MarketRefreshError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<MarketStoreError> for MarketRefreshError {
    fn from(error: MarketStoreError) -> Self {
        Self::Store(error)
    }
}

pub struct MarketRefreshService;

impl MarketRefreshService {
    /// Performs exactly one user-initiated collection. There is no scheduler, polling loop, or
    /// persistent connection: every provider request originates from this command invocation.
    pub fn refresh_today_snapshot(
        database: &DatabaseService,
    ) -> Result<ManualMarketSnapshotView, MarketRefreshError> {
        let holdings = database.list_market_securities_for_holdings()?;
        let holding_snapshot = Self::fetch_holding_snapshot(&holdings);
        database.save_market_snapshot(&holding_snapshot)?;

        let index_snapshot = Self::fetch_index_snapshot();
        database.save_market_index_snapshot(&index_snapshot)?;

        let holding_configuration = if TushareAdapter::is_configured() {
            "CONFIGURED"
        } else {
            "PUBLIC_ONLY"
        };
        let holdings_view = Self::to_view(holding_snapshot, holding_configuration);
        let indices_view = Self::section_view(index_snapshot);

        // News/Event Adapter contracts already exist, but no external Adapter is enabled in V1.
        // The manual snapshot reports this explicitly instead of inventing articles or dates.
        let no_adapter = |message: &str| SnapshotSectionView {
            source: "未配置数据源".into(),
            status: "NO_DATA".into(),
            item_count: 0,
            updated_at: None,
            message: Some(message.into()),
        };
        Ok(ManualMarketSnapshotView {
            holdings: holdings_view,
            indices: indices_view,
            news: no_adapter("新闻数据 Adapter 尚未配置；本次未保存新闻。"),
            events: no_adapter("事件数据 Adapter 尚未配置；本次未保存事件。"),
        })
    }

    pub fn refresh_tushare(
        database: &DatabaseService,
    ) -> Result<MarketRefreshView, MarketRefreshError> {
        let adapter = TushareAdapter::new().expect("Tushare adapter has no constructor state");
        let configuration_status = if TushareAdapter::is_configured() {
            "CONFIGURED"
        } else {
            "UNCONFIGURED"
        };
        let snapshot = Self::refresh_with_adapter(database, &adapter)?;
        Ok(Self::to_view(snapshot, configuration_status))
    }

    fn refresh_with_adapter<A: MarketDataAdapter>(
        database: &DatabaseService,
        adapter: &A,
    ) -> Result<MarketSnapshot, MarketRefreshError> {
        let securities = database.list_market_securities_for_holdings()?;
        // Tushare daily data represents an exchange close. The Adapter source class therefore
        // always produces CLOSED records; this phase avoids ever calling it a live quote.
        let request = MarketFetchRequest::now(securities, MarketPhase::Closed);
        Ok(MarketService::fetch_and_store(adapter, &request, database)?)
    }

    fn fetch_holding_snapshot(securities: &[MarketSecurity]) -> MarketSnapshot {
        let eastmoney =
            EastmoneyAdapter::new().expect("Eastmoney adapter has no constructor state");
        let tencent = TencentAdapter::new().expect("Tencent adapter has no constructor state");
        let request = MarketFetchRequest::now(securities.to_vec(), MarketPhase::Unknown);
        if TushareAdapter::is_configured() {
            let tushare = TushareAdapter::new().expect("Tushare adapter has no constructor state");
            MarketService::fetch_with_fallback(&[&tushare, &eastmoney, &tencent], &request)
        } else {
            MarketService::fetch_with_fallback(&[&eastmoney, &tencent], &request)
        }
    }

    fn fetch_index_snapshot() -> MarketSnapshot {
        let eastmoney =
            EastmoneyAdapter::new().expect("Eastmoney adapter has no constructor state");
        let tencent = TencentAdapter::new().expect("Tencent adapter has no constructor state");
        let request = MarketFetchRequest::now(
            vec![
                MarketSecurity {
                    security_id: -1,
                    symbol: "000001".into(),
                    name: "上证指数".into(),
                    market: "SSE".into(),
                },
                MarketSecurity {
                    security_id: -2,
                    symbol: "399001".into(),
                    name: "深证成指".into(),
                    market: "SZSE".into(),
                },
                MarketSecurity {
                    security_id: -3,
                    symbol: "399006".into(),
                    name: "创业板指".into(),
                    market: "SZSE".into(),
                },
                MarketSecurity {
                    security_id: -4,
                    symbol: "000688".into(),
                    name: "科创50".into(),
                    market: "SSE".into(),
                },
            ],
            MarketPhase::Unknown,
        );
        let mut snapshot = MarketService::fetch_with_fallback(&[&eastmoney, &tencent], &request);
        for quote in &mut snapshot.quotes {
            let canonical_symbol = match (quote.symbol.as_str(), quote.market.as_str()) {
                ("000001", "SSE") => "000001.SH".into(),
                ("399001", "SZSE") => "399001.SZ".into(),
                ("399006", "SZSE") => "399006.SZ".into(),
                ("000688", "SSE") => "000688.SH".into(),
                _ => quote.symbol.clone(),
            };
            quote.symbol = canonical_symbol;
        }
        snapshot
    }

    fn section_view(snapshot: MarketSnapshot) -> SnapshotSectionView {
        SnapshotSectionView {
            source: snapshot.source.name,
            status: snapshot.delay_status.as_str().into(),
            item_count: snapshot.quotes.len(),
            updated_at: snapshot.market_timestamp.map(|time| time.to_rfc3339()),
            message: snapshot.unavailable_reason,
        }
    }

    fn to_view(snapshot: MarketSnapshot, configuration_status: &str) -> MarketRefreshView {
        let message = if configuration_status == "UNCONFIGURED" {
            Some("Tushare 未配置；请在设置页保存 Token 到系统钥匙串。".into())
        } else {
            snapshot.unavailable_reason.clone()
        };
        MarketRefreshView {
            source: snapshot.source.name,
            configuration_status: configuration_status.into(),
            status: snapshot.delay_status.as_str().into(),
            quote_count: snapshot.quotes.len(),
            market_timestamp: snapshot
                .market_timestamp
                .map(|timestamp| timestamp.to_rfc3339()),
            fetched_at: snapshot.fetched_at.to_rfc3339(),
            message,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;

    use super::*;
    use crate::{
        database::service::{NewCashAccount, NewHolding, NewSecurity},
        market_service::{
            DataSourcePriority, MarketDataSource, MarketStatus, RawMarketQuote, SourceClass,
        },
    };

    struct RecordedAdapter;

    impl MarketDataAdapter for RecordedAdapter {
        fn source(&self) -> MarketDataSource {
            MarketDataSource {
                name: "Recorded verification adapter".into(),
                base_url: "https://test.invalid/market".into(),
                priority: DataSourcePriority::PrimaryApi,
                source_class: SourceClass::EndOfDay,
            }
        }

        fn fetch(
            &self,
            request: &MarketFetchRequest,
        ) -> Result<Vec<RawMarketQuote>, crate::market_service::MarketAdapterError> {
            Ok(request
                .securities
                .iter()
                .map(|security| RawMarketQuote {
                    security: security.clone(),
                    current_price: Decimal::new(1200, 2),
                    previous_close: Decimal::new(1180, 2),
                    price_change: Decimal::new(20, 2),
                    change_percent: Decimal::new(16949, 4),
                    volume: Decimal::new(100, 0),
                    volume_unit: "LOTS".into(),
                    turnover_amount: Decimal::new(120000, 0),
                    turnover_unit: "THOUSAND_CNY".into(),
                    market_timestamp: Utc.with_ymd_and_hms(2026, 8, 7, 7, 0, 0).unwrap(),
                })
                .collect())
        }
    }

    #[test]
    fn refresh_persists_a_source_backed_snapshot_for_current_holdings() {
        let database = DatabaseService::open_in_memory().expect("open database");
        let account = database
            .create_cash_account(NewCashAccount {
                name: "验证账户".into(),
                currency: "CNY".into(),
                available_to_buy: "0".into(),
                withdrawable_cash: "0".into(),
                pending_settlement: "0".into(),
            })
            .expect("create account");
        let security = database
            .create_security(NewSecurity {
                symbol: "600519".into(),
                name: "测试夹具证券".into(),
                market: "SSE".into(),
                exchange: "SSE".into(),
                security_type: "STOCK".into(),
                industry: None,
                concepts_json: "[]".into(),
                trade_rule: "T_PLUS_1".into(),
            })
            .expect("create security");
        database
            .create_holding(NewHolding {
                cash_account_id: account.id,
                security_id: security.id,
                quantity: 100,
                available_quantity: 100,
                average_cost: "10".into(),
                cost_amount: "1000".into(),
                position_source: "INITIAL_POSITION".into(),
                as_of_date: Some("2026-08-07".into()),
            })
            .expect("create holding");

        let snapshot = MarketRefreshService::refresh_with_adapter(&database, &RecordedAdapter)
            .expect("refresh and persist");
        assert_eq!(snapshot.delay_status, MarketStatus::Closed);
        assert_eq!(snapshot.quotes.len(), 1);

        let holding = database
            .list_portfolio_holding_data()
            .expect("read persisted quote")
            .pop()
            .expect("holding view");
        assert_eq!(holding.current_price.as_deref(), Some("12.00"));
        assert_eq!(holding.quote_status.as_deref(), Some("CLOSED"));
    }

    #[test]
    fn unconfigured_tushare_state_is_explicit_and_has_no_quote_values() {
        let fetched_at = Utc.with_ymd_and_hms(2026, 8, 8, 1, 0, 0).unwrap();
        let view = MarketRefreshService::to_view(
            MarketSnapshot {
                source: MarketDataSource {
                    name: "Tushare".into(),
                    base_url: "https://api.tushare.pro".into(),
                    priority: DataSourcePriority::PrimaryApi,
                    source_class: SourceClass::EndOfDay,
                },
                market_timestamp: None,
                fetched_at,
                delay_status: MarketStatus::NoData,
                quotes: Vec::new(),
                unavailable_reason: Some("adapter configuration error".into()),
            },
            "UNCONFIGURED",
        );

        assert_eq!(view.configuration_status, "UNCONFIGURED");
        assert_eq!(view.status, "NO_DATA");
        assert_eq!(view.quote_count, 0);
        assert_eq!(view.market_timestamp, None);
        assert!(view.message.expect("safe message").contains("未配置"));
    }

    #[test]
    fn manual_snapshot_section_preserves_source_status_and_time() {
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 8, 1, 0, 0).unwrap();
        let section = MarketRefreshService::section_view(MarketSnapshot {
            source: MarketDataSource {
                name: "东方财富公开行情".into(),
                base_url: "https://example.invalid".into(),
                priority: DataSourcePriority::PublicQuote,
                source_class: SourceClass::PublicQuote,
            },
            market_timestamp: Some(timestamp),
            fetched_at: timestamp,
            delay_status: MarketStatus::Delayed,
            quotes: Vec::new(),
            unavailable_reason: None,
        });
        assert_eq!(section.source, "东方财富公开行情");
        assert_eq!(section.status, "DELAYED");
        assert_eq!(section.updated_at, Some(timestamp.to_rfc3339()));
    }
}
