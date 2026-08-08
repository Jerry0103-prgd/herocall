//! Non-AI daily portfolio review service.
//!
//! Reviews are deterministic snapshots assembled only from local application services and saved
//! SQLite records. They never request market data, call an AI model, forecast returns, or issue
//! trading recommendations.

use std::{error::Error, fmt, str::FromStr};

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    dashboard_service::DashboardService,
    database::service::{DailyReview, DatabaseError, DatabaseService, NewDailyReview},
    news_service::{NewsService, NewsServiceError},
    portfolio_ui_service::{PortfolioUiError, PortfolioUiService},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPortfolioSummary {
    pub total_assets: Option<String>,
    pub daily_pnl: Option<String>,
    pub return_rate: Option<String>,
    pub holding_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSnapshot {
    pub id: i64,
    pub source: String,
    pub market_timestamp: String,
    pub fetched_at: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewMarketIndex {
    pub name: String,
    pub symbol: String,
    pub change_percent: Option<String>,
    pub source: Option<String>,
    pub status: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewMarketSummary {
    pub snapshot: Option<ReviewSnapshot>,
    pub major_indices: Vec<ReviewMarketIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HoldingContribution {
    pub name: String,
    pub symbol: String,
    pub daily_pnl: Option<String>,
    pub change_percent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewHoldingSummary {
    pub contributions: Vec<HoldingContribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRiskSummary {
    pub facts: Vec<String>,
    pub related_news_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DailyReviewView {
    pub id: i64,
    pub review_date: String,
    pub snapshot_id: Option<i64>,
    pub portfolio_summary: ReviewPortfolioSummary,
    pub market_summary: ReviewMarketSummary,
    pub holding_summary: ReviewHoldingSummary,
    pub risk_summary: ReviewRiskSummary,
    pub created_at: String,
}

#[derive(Debug)]
pub enum ReviewServiceError {
    Database(DatabaseError),
    Portfolio(PortfolioUiError),
    News(NewsServiceError),
    Serialization(serde_json::Error),
    Validation(&'static str),
    MissingReview,
}

impl fmt::Display for ReviewServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::Portfolio(error) => write!(formatter, "portfolio error: {error}"),
            Self::News(error) => write!(formatter, "news error: {error}"),
            Self::Serialization(error) => write!(formatter, "review serialization error: {error}"),
            Self::Validation(message) => formatter.write_str(message),
            Self::MissingReview => formatter.write_str("暂无当日复盘"),
        }
    }
}

impl Error for ReviewServiceError {}

impl From<DatabaseError> for ReviewServiceError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<PortfolioUiError> for ReviewServiceError {
    fn from(error: PortfolioUiError) -> Self {
        Self::Portfolio(error)
    }
}

impl From<NewsServiceError> for ReviewServiceError {
    fn from(error: NewsServiceError) -> Self {
        Self::News(error)
    }
}

impl From<serde_json::Error> for ReviewServiceError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

pub struct ReviewService;

impl ReviewService {
    pub fn generate(
        database: &DatabaseService,
        review_date: &str,
    ) -> Result<DailyReviewView, ReviewServiceError> {
        validate_review_date(review_date)?;
        let holdings = PortfolioUiService::list(database)?;
        let related_news = NewsService::list_for_holdings(database)?;
        let snapshot = database.latest_market_snapshot_for_review_date(review_date)?;
        let asset_summary = DashboardService::load_asset_summary(database);

        let portfolio_summary = ReviewPortfolioSummary {
            total_assets: asset_summary.total_assets,
            daily_pnl: asset_summary.daily_pnl,
            return_rate: asset_summary.return_rate,
            holding_count: holdings.len(),
        };
        let market_summary = ReviewMarketSummary {
            snapshot: snapshot.as_ref().map(|snapshot| ReviewSnapshot {
                id: snapshot.id,
                source: snapshot.source.clone(),
                market_timestamp: snapshot.market_timestamp.clone(),
                fetched_at: snapshot.fetched_at.clone(),
                status: snapshot.delay_status.clone(),
            }),
            major_indices: DashboardService::load_market_snapshot(database)
                .into_iter()
                .map(|index| ReviewMarketIndex {
                    name: index.name,
                    symbol: index.symbol,
                    change_percent: index.change_percent,
                    source: index.source,
                    status: index.status,
                    updated_at: index.updated_at,
                })
                .collect(),
        };
        let holding_summary = ReviewHoldingSummary {
            contributions: sorted_contributions(&holdings),
        };
        let risk_summary = factual_risks(snapshot.is_some(), &holdings, related_news.len());

        let stored = database.upsert_daily_review(NewDailyReview {
            review_date: review_date.into(),
            snapshot_id: snapshot.map(|snapshot| snapshot.id),
            portfolio_summary: serde_json::to_string(&portfolio_summary)?,
            market_summary: serde_json::to_string(&market_summary)?,
            holding_summary: serde_json::to_string(&holding_summary)?,
            risk_summary: serde_json::to_string(&risk_summary)?,
        })?;
        Self::view_from_record(stored)
    }

    pub fn get(
        database: &DatabaseService,
        review_date: &str,
    ) -> Result<DailyReviewView, ReviewServiceError> {
        validate_review_date(review_date)?;
        match database.get_daily_review_by_date(review_date) {
            Ok(review) => Self::view_from_record(review),
            Err(DatabaseError::Sqlite(rusqlite::Error::QueryReturnedNoRows)) => {
                Err(ReviewServiceError::MissingReview)
            }
            Err(error) => Err(ReviewServiceError::Database(error)),
        }
    }

    fn view_from_record(record: DailyReview) -> Result<DailyReviewView, ReviewServiceError> {
        Ok(DailyReviewView {
            id: record.id,
            review_date: record.review_date,
            snapshot_id: record.snapshot_id,
            portfolio_summary: serde_json::from_str(&record.portfolio_summary)?,
            market_summary: serde_json::from_str(&record.market_summary)?,
            holding_summary: serde_json::from_str(&record.holding_summary)?,
            risk_summary: serde_json::from_str(&record.risk_summary)?,
            created_at: record.created_at,
        })
    }
}

fn validate_review_date(value: &str) -> Result<(), ReviewServiceError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| ReviewServiceError::Validation("复盘日期必须为 YYYY-MM-DD"))
}

fn sorted_contributions(
    holdings: &[crate::portfolio_ui_service::PortfolioHoldingView],
) -> Vec<HoldingContribution> {
    let mut contributions: Vec<_> = holdings
        .iter()
        .map(|holding| HoldingContribution {
            name: holding.name.clone(),
            symbol: holding.symbol.clone(),
            daily_pnl: holding.daily_pnl.clone(),
            change_percent: holding.change_percent.clone(),
        })
        .collect();
    contributions.sort_by(|left, right| {
        match (
            decimal_or_none(&left.daily_pnl),
            decimal_or_none(&right.daily_pnl),
        ) {
            (Some(left), Some(right)) => right.cmp(&left),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left
                .name
                .cmp(&right.name)
                .then_with(|| left.symbol.cmp(&right.symbol)),
        }
    });
    contributions
}

fn decimal_or_none(value: &Option<String>) -> Option<Decimal> {
    value
        .as_deref()
        .and_then(|value| Decimal::from_str(value).ok())
}

fn factual_risks(
    has_snapshot: bool,
    holdings: &[crate::portfolio_ui_service::PortfolioHoldingView],
    related_news_count: usize,
) -> ReviewRiskSummary {
    let unavailable_valuations = holdings
        .iter()
        .filter(|holding| holding.current_price.is_none() || holding.daily_pnl.is_none())
        .count();
    let mut facts = Vec::new();
    if !has_snapshot {
        facts.push("未找到当日市场快照，市场表现未确认。".into());
    }
    if unavailable_valuations > 0 {
        facts.push(format!(
            "{unavailable_valuations} 个持仓缺少可验证的当前价格或今日盈亏。"
        ));
    }
    if related_news_count == 0 {
        facts.push("暂无与当前持仓关联的已保存资讯。".into());
    } else {
        facts.push(format!(
            "已保存与当前持仓关联的资讯：{related_news_count} 条。"
        ));
    }
    ReviewRiskSummary {
        facts,
        related_news_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        database::service::MarketSnapshotReference,
        market_service::{
            DataSourcePriority, MarketDataSource, MarketQuote, MarketSnapshot, MarketSnapshotStore,
            MarketStatus, SourceClass,
        },
        news_service::{NewsArticleInput, NewsSourceType},
        portfolio_ui_service::CreateHoldingInput,
    };
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;

    fn decimal(value: &str) -> Decimal {
        value.parse().expect("valid decimal fixture")
    }

    fn create_holding(database: &DatabaseService, symbol: &str, name: &str) -> i64 {
        PortfolioUiService::create(
            database,
            CreateHoldingInput {
                symbol: symbol.into(),
                name: name.into(),
                market: "SSE".into(),
                security_type: "STOCK".into(),
                quantity: "100".into(),
                average_cost: "8".into(),
            },
        )
        .expect("create test holding");
        database
            .find_security_by_symbol_and_market(symbol, "SSE")
            .expect("find security")
            .expect("security exists")
            .id
    }

    #[test]
    fn review_generation_associates_snapshot_and_sorts_holding_contributions() {
        let database = DatabaseService::open_in_memory().expect("create database");
        let maotai_id = create_holding(&database, "600519", "测试证券甲");
        let pingan_id = create_holding(&database, "600000", "测试证券乙");
        let source = MarketDataSource {
            name: "Recorded review test source".into(),
            base_url: "https://example.invalid/quotes".into(),
            priority: DataSourcePriority::PublicQuote,
            source_class: SourceClass::PublicQuote,
        };
        let market_timestamp = Utc.with_ymd_and_hms(2026, 8, 8, 7, 0, 0).unwrap();
        let snapshot = MarketSnapshot {
            source: source.clone(),
            market_timestamp: Some(market_timestamp),
            fetched_at: market_timestamp,
            delay_status: MarketStatus::Closed,
            quotes: vec![
                MarketQuote {
                    security_id: maotai_id,
                    symbol: "600519".into(),
                    name: "测试证券甲".into(),
                    market: "SSE".into(),
                    current_price: decimal("11"),
                    previous_close: decimal("10"),
                    price_change: decimal("1"),
                    change_percent: decimal("10"),
                    volume: decimal("0"),
                    volume_unit: "SHARE".into(),
                    turnover_amount: decimal("0"),
                    turnover_unit: "CNY".into(),
                    market_timestamp,
                    fetched_at: market_timestamp,
                    source: source.name.clone(),
                    delay_status: MarketStatus::Closed,
                },
                MarketQuote {
                    security_id: pingan_id,
                    symbol: "600000".into(),
                    name: "测试证券乙".into(),
                    market: "SSE".into(),
                    current_price: decimal("12"),
                    previous_close: decimal("10"),
                    price_change: decimal("2"),
                    change_percent: decimal("20"),
                    volume: decimal("0"),
                    volume_unit: "SHARE".into(),
                    turnover_amount: decimal("0"),
                    turnover_unit: "CNY".into(),
                    market_timestamp,
                    fetched_at: market_timestamp,
                    source: source.name.clone(),
                    delay_status: MarketStatus::Closed,
                },
            ],
            unavailable_reason: None,
        };
        database
            .save_market_snapshot(&snapshot)
            .expect("save recorded test snapshot");
        let snapshot_reference: MarketSnapshotReference = database
            .latest_market_snapshot_for_review_date("2026-08-08")
            .expect("query snapshot")
            .expect("snapshot exists");
        NewsService::create(
            &database,
            NewsArticleInput {
                title: "测试持仓资讯".into(),
                source: "测试公告来源".into(),
                source_type: NewsSourceType::Official,
                published_at: "2026-08-08T15:00:00+08:00".into(),
                fetch_time: "2026-08-08T15:05:00+08:00".into(),
                summary: "仅用于结构化复盘测试的可追溯夹具。".into(),
                url: "https://example.invalid/news/review".into(),
                related_security_id: Some(maotai_id),
            },
        )
        .expect("save test news");

        let review = ReviewService::generate(&database, "2026-08-08").expect("generate review");
        assert_eq!(review.snapshot_id, Some(snapshot_reference.id));
        assert_eq!(review.portfolio_summary.holding_count, 2);
        assert_eq!(review.holding_summary.contributions[0].symbol, "600000");
        assert_eq!(
            review.holding_summary.contributions[0].daily_pnl.as_deref(),
            Some("200")
        );
        assert_eq!(review.risk_summary.related_news_count, 1);
        assert!(review
            .risk_summary
            .facts
            .iter()
            .all(|fact| !fact.contains("买入")
                && !fact.contains("卖出")
                && !fact.contains("预测")));

        let stored = ReviewService::get(&database, "2026-08-08").expect("load saved review");
        assert_eq!(stored, review);
    }
}
