//! Read-only Dashboard application service.
//!
//! Phase 5-A intentionally exposes no derived financial values until the reporting calculation
//! pipeline has a complete, traceable input set. `None` is serialized to the UI as “暂无数据”.

use serde::Serialize;

use crate::database::service::DatabaseService;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssetSummaryView {
    pub total_assets: Option<String>,
    pub stock_market_value: Option<String>,
    pub cash: Option<String>,
    pub daily_pnl: Option<String>,
    pub total_pnl: Option<String>,
    pub return_rate: Option<String>,
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

pub struct DashboardService;

impl DashboardService {
    /// This is deliberately a no-data response rather than a zero-value portfolio. Phase 5-A
    /// has no reporting aggregation and index persistence model, so calculating any financial
    /// field here would create an untraceable value.
    pub fn load_asset_summary(_database: &DatabaseService) -> AssetSummaryView {
        AssetSummaryView {
            total_assets: None,
            stock_market_value: None,
            cash: None,
            daily_pnl: None,
            total_pnl: None,
            return_rate: None,
        }
    }

    /// Index identities are presentation configuration only. They carry no prices or inferred
    /// values unless a future verified index-data adapter persists source-backed quotes.
    pub fn load_market_snapshot(_database: &DatabaseService) -> Vec<MarketIndexQuoteView> {
        [
            ("上证指数", "000001.SH"),
            ("深证成指", "399001.SZ"),
            ("创业板指", "399006.SZ"),
            ("科创50", "000688.SH"),
        ]
        .into_iter()
        .map(|(name, symbol)| MarketIndexQuoteView {
            name: name.into(),
            symbol: symbol.into(),
            current_price: None,
            change_percent: None,
            source: None,
            status: "NO_DATA".into(),
            updated_at: None,
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_never_substitutes_missing_financial_values_with_zero() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let summary = DashboardService::load_asset_summary(&database);
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
}
