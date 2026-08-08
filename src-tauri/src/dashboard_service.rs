//! Traceable Dashboard aggregation service.
//!
//! Values are calculated exclusively in Rust from local cash accounts, Portfolio Service views,
//! and source-backed Market Service quotes. If an input is incomplete, the corresponding field is
//! `None` so the UI renders “暂无数据” instead of substituting a financial value.

use std::{error::Error, fmt, str::FromStr};

use rust_decimal::Decimal;
use serde::Serialize;

use crate::{
    database::service::{DatabaseError, DatabaseService},
    portfolio_ui_service::{PortfolioUiError, PortfolioUiService},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssetSummaryView {
    pub total_assets: Option<String>,
    pub stock_market_value: Option<String>,
    pub cash: Option<String>,
    pub daily_pnl: Option<String>,
    pub total_pnl: Option<String>,
    pub return_rate: Option<String>,
    pub valuation_source: Option<String>,
    pub valuation_timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketIndexQuoteView {
    pub name: String,
    pub symbol: String,
    pub current_price: Option<String>,
    pub change_percent: Option<String>,
    pub source: Option<String>,
    pub status: String,
    pub updated_at: Option<String>,
}

#[derive(Debug)]
pub enum DashboardError {
    Database(DatabaseError),
    Portfolio(PortfolioUiError),
}

impl fmt::Display for DashboardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::Portfolio(error) => write!(formatter, "portfolio error: {error}"),
        }
    }
}

impl Error for DashboardError {}

impl From<DatabaseError> for DashboardError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<PortfolioUiError> for DashboardError {
    fn from(error: PortfolioUiError) -> Self {
        Self::Portfolio(error)
    }
}

pub struct DashboardService;

impl DashboardService {
    pub fn load_asset_summary(
        database: &DatabaseService,
    ) -> Result<AssetSummaryView, DashboardError> {
        let accounts = database.list_cash_accounts()?;
        let holdings = PortfolioUiService::list(database)?;
        let cash = (!accounts.is_empty())
            .then(|| {
                sum_decimals(
                    accounts
                        .iter()
                        .map(|account| account.available_to_buy.as_str()),
                )
            })
            .flatten();

        // A valid valuation requires a value and daily P&L for every current holding. This keeps
        // a partially refreshed portfolio from being displayed as a complete total.
        let valuations_complete = !holdings.is_empty()
            && holdings
                .iter()
                .all(|holding| holding.market_value.is_some() && holding.daily_pnl.is_some());
        let stock_market_value = valuations_complete
            .then(|| {
                sum_option_decimals(
                    holdings
                        .iter()
                        .map(|holding| holding.market_value.as_deref()),
                )
            })
            .flatten();
        let daily_pnl = valuations_complete
            .then(|| {
                sum_option_decimals(holdings.iter().map(|holding| holding.daily_pnl.as_deref()))
            })
            .flatten();

        let total_assets = match (cash, stock_market_value) {
            (Some(cash), Some(stock_market_value)) => Some(cash + stock_market_value),
            _ => None,
        };

        // The V1 UI currently creates opening/manual positions, not a user-facing transaction
        // ledger. With no confirmed ledger records, open-position P&L is also total portfolio
        // P&L. Once a confirmed transaction exists, realized P&L must be included by the ledger
        // before this field can be shown; returning None avoids an incomplete total.
        let total_pnl = if valuations_complete && !database.has_confirmed_transactions()? {
            let unrealized = holdings.iter().map(|holding| {
                let market_value = Decimal::from_str(
                    holding
                        .market_value
                        .as_deref()
                        .expect("checked valuation completeness"),
                )
                .ok()?;
                let quantity = Decimal::from_str(&holding.quantity).ok()?;
                let average_cost = Decimal::from_str(&holding.average_cost).ok()?;
                Some(market_value - quantity * average_cost)
            });
            sum_decimals_option(unrealized)
        } else {
            None
        };
        let cost_amount = holdings.iter().map(|holding| {
            let quantity = Decimal::from_str(&holding.quantity).ok()?;
            let average_cost = Decimal::from_str(&holding.average_cost).ok()?;
            Some(quantity * average_cost)
        });
        let cost_amount = sum_decimals_option(cost_amount);
        let return_rate = match (total_pnl, cost_amount) {
            (Some(total_pnl), Some(cost_amount)) if !cost_amount.is_zero() => {
                Some(total_pnl * Decimal::new(100, 0) / cost_amount)
            }
            _ => None,
        };
        let valuation_snapshot = valuations_complete
            .then(|| database.latest_market_snapshot_for_current_holdings())
            .transpose()?
            .flatten();

        Ok(AssetSummaryView {
            total_assets: total_assets.map(decimal_to_string),
            stock_market_value: stock_market_value.map(decimal_to_string),
            cash: cash.map(decimal_to_string),
            daily_pnl: daily_pnl.map(decimal_to_string),
            total_pnl: total_pnl.map(decimal_to_string),
            return_rate: return_rate.map(decimal_to_string),
            valuation_source: valuation_snapshot
                .as_ref()
                .map(|snapshot| snapshot.source.clone()),
            valuation_timestamp: valuation_snapshot
                .as_ref()
                .map(|snapshot| snapshot.market_timestamp.clone()),
        })
    }

    /// Returns only persisted, source-backed index records. Missing records stay `NO_DATA`;
    /// identity labels are presentation metadata, never financial values.
    pub fn load_market_snapshot(database: &DatabaseService) -> Vec<MarketIndexQuoteView> {
        let persisted = database
            .latest_market_index_quotes()
            .unwrap_or_default()
            .into_iter()
            .map(|record| (record.symbol.clone(), record))
            .collect::<std::collections::HashMap<_, _>>();
        [
            ("上证指数", "000001.SH"),
            ("深证成指", "399001.SZ"),
            ("创业板指", "399006.SZ"),
            ("科创50", "000688.SH"),
        ]
        .into_iter()
        .map(|(name, symbol)| match persisted.get(symbol) {
            Some(record) => MarketIndexQuoteView {
                name: record.name.clone(),
                symbol: record.symbol.clone(),
                current_price: Some(record.current_price.clone()),
                change_percent: record.change_percent.clone(),
                source: Some(record.source.clone()),
                status: record.status.clone(),
                updated_at: Some(record.updated_at.clone()),
            },
            None => MarketIndexQuoteView {
                name: name.into(),
                symbol: symbol.into(),
                current_price: None,
                change_percent: None,
                source: None,
                status: "NO_DATA".into(),
                updated_at: None,
            },
        })
        .collect()
    }
}

fn sum_decimals<'a>(values: impl Iterator<Item = &'a str>) -> Option<Decimal> {
    values
        .map(Decimal::from_str)
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .map(|values| values.into_iter().sum())
}

fn sum_option_decimals<'a>(values: impl Iterator<Item = Option<&'a str>>) -> Option<Decimal> {
    values
        .map(|value| value.and_then(|value| Decimal::from_str(value).ok()))
        .collect::<Option<Vec<_>>>()
        .map(|values| values.into_iter().sum())
}

fn sum_decimals_option(values: impl Iterator<Item = Option<Decimal>>) -> Option<Decimal> {
    values
        .collect::<Option<Vec<_>>>()
        .map(|values| values.into_iter().sum())
}

fn decimal_to_string(value: Decimal) -> String {
    value.normalize().to_string()
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::{
        database::service::{NewCashAccount, NewHolding, NewSecurity},
        market_service::{
            DataSourcePriority, MarketDataSource, MarketQuote, MarketSnapshot, MarketSnapshotStore,
            MarketStatus, SourceClass,
        },
    };

    fn decimal(value: &str) -> Decimal {
        Decimal::from_str(value).expect("valid decimal")
    }

    #[test]
    fn dashboard_aggregates_cash_and_closed_source_backed_positions_in_rust() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let account = database
            .create_cash_account(NewCashAccount {
                name: "人民币现金账户-1".into(),
                currency: "CNY".into(),
                available_to_buy: "500".into(),
                withdrawable_cash: "500".into(),
                pending_settlement: "0".into(),
            })
            .expect("create cash account");
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
                quantity: 10,
                available_quantity: 10,
                average_cost: "10".into(),
                cost_amount: "100".into(),
                position_source: "INITIAL_POSITION".into(),
                as_of_date: Some("2026-08-07".into()),
            })
            .expect("create holding");
        let market_timestamp = Utc.with_ymd_and_hms(2026, 8, 7, 7, 0, 0).unwrap();
        database
            .save_market_snapshot(&MarketSnapshot {
                source: MarketDataSource {
                    name: "Recorded dashboard verification source".into(),
                    base_url: "https://test.invalid/market".into(),
                    priority: DataSourcePriority::PrimaryApi,
                    source_class: SourceClass::EndOfDay,
                },
                market_timestamp: Some(market_timestamp),
                fetched_at: market_timestamp,
                delay_status: MarketStatus::Closed,
                quotes: vec![MarketQuote {
                    security_id: security.id,
                    symbol: security.symbol.clone(),
                    name: security.name.clone(),
                    market: security.market.clone(),
                    current_price: decimal("12"),
                    previous_close: decimal("11"),
                    price_change: decimal("1"),
                    change_percent: decimal("9.0909"),
                    volume: decimal("10"),
                    volume_unit: "LOTS".into(),
                    turnover_amount: decimal("120"),
                    turnover_unit: "THOUSAND_CNY".into(),
                    market_timestamp,
                    fetched_at: market_timestamp,
                    source: "Recorded dashboard verification source".into(),
                    delay_status: MarketStatus::Closed,
                }],
                unavailable_reason: None,
            })
            .expect("save verification snapshot");

        let summary = DashboardService::load_asset_summary(&database).expect("aggregate dashboard");
        assert_eq!(summary.cash.as_deref(), Some("500"));
        assert_eq!(summary.stock_market_value.as_deref(), Some("120"));
        assert_eq!(summary.total_assets.as_deref(), Some("620"));
        assert_eq!(summary.daily_pnl.as_deref(), Some("10"));
        assert_eq!(summary.total_pnl.as_deref(), Some("20"));
        assert_eq!(summary.return_rate.as_deref(), Some("20"));
        assert_eq!(
            summary.valuation_source.as_deref(),
            Some("Recorded dashboard verification source")
        );
        assert_eq!(
            summary.valuation_timestamp.as_deref(),
            Some("2026-08-07T07:00:00+00:00")
        );
    }

    #[test]
    fn dashboard_never_substitutes_missing_financial_values_with_zero() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let summary = DashboardService::load_asset_summary(&database).expect("read empty summary");
        let indices = DashboardService::load_market_snapshot(&database);

        assert_eq!(summary.total_assets, None);
        assert_eq!(summary.stock_market_value, None);
        assert_eq!(summary.cash, None);
        assert!(indices.iter().all(|index| {
            index.current_price.is_none()
                && index.change_percent.is_none()
                && index.source.is_none()
                && index.updated_at.is_none()
                && index.status == "NO_DATA"
        }));
    }

    #[test]
    fn dashboard_reads_only_persisted_index_snapshot_metadata() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 8, 6, 0, 0).unwrap();
        database
            .save_market_index_snapshot(&MarketSnapshot {
                source: MarketDataSource {
                    name: "Recorded index source".into(),
                    base_url: "https://test.invalid/index".into(),
                    priority: DataSourcePriority::PublicQuote,
                    source_class: SourceClass::PublicQuote,
                },
                market_timestamp: Some(timestamp),
                fetched_at: timestamp,
                delay_status: MarketStatus::Delayed,
                quotes: vec![MarketQuote {
                    security_id: -1,
                    symbol: "000001.SH".into(),
                    name: "上证指数".into(),
                    market: "SSE".into(),
                    current_price: decimal("3200"),
                    previous_close: decimal("3180"),
                    price_change: decimal("20"),
                    change_percent: decimal("0.63"),
                    volume: decimal("0"),
                    volume_unit: "SOURCE_DECLARED".into(),
                    turnover_amount: decimal("0"),
                    turnover_unit: "SOURCE_DECLARED".into(),
                    market_timestamp: timestamp,
                    fetched_at: timestamp,
                    source: "Recorded index source".into(),
                    delay_status: MarketStatus::Delayed,
                }],
                unavailable_reason: None,
            })
            .expect("persist index snapshot");

        let indices = DashboardService::load_market_snapshot(&database);
        assert_eq!(indices[0].current_price.as_deref(), Some("3200"));
        assert_eq!(indices[0].source.as_deref(), Some("Recorded index source"));
        assert_eq!(indices[0].status, "DELAYED");
        assert_eq!(indices[1].status, "NO_DATA");
    }
}
