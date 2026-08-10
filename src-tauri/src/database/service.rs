#![allow(dead_code)] // Phase 2-A defines the service API before later application commands consume it.

use std::{error::Error, fmt, fs, path::Path};

use rusqlite::{params, Connection, OptionalExtension, Row, Transaction as SqliteTransaction};
use serde::Serialize;
use tauri::Manager;

use super::migrations;
use crate::market_service::{
    MarketSecurity, MarketSnapshot, MarketSnapshotStore, MarketStatus, MarketStoreError,
};

pub type DatabaseResult<T> = Result<T, DatabaseError>;

#[derive(Debug)]
pub enum DatabaseError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    AppPath(String),
    MigrationChecksum { version: &'static str },
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "SQLite error: {error}"),
            Self::Io(error) => write!(formatter, "filesystem error: {error}"),
            Self::AppPath(error) => write!(formatter, "application data path error: {error}"),
            Self::MigrationChecksum { version } => {
                write!(
                    formatter,
                    "migration checksum mismatch for version {version}"
                )
            }
        }
    }
}

impl Error for DatabaseError {}

impl From<rusqlite::Error> for DatabaseError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<std::io::Error> for DatabaseError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Security {
    pub id: i64,
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: String,
    pub security_type: String,
    pub industry: Option<String>,
    pub concepts_json: String,
    pub trade_rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSecurity {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: String,
    pub security_type: String,
    pub industry: Option<String>,
    pub concepts_json: String,
    pub trade_rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashAccount {
    pub id: i64,
    pub name: String,
    pub currency: String,
    pub available_to_buy: String,
    pub withdrawable_cash: String,
    pub pending_settlement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCashAccount {
    pub name: String,
    pub currency: String,
    pub available_to_buy: String,
    pub withdrawable_cash: String,
    pub pending_settlement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketSourceStatus {
    pub name: String,
    pub status: String,
    pub last_success_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketIndexQuoteRecord {
    pub name: String,
    pub symbol: String,
    pub current_price: String,
    pub change_percent: Option<String>,
    pub source: String,
    pub status: String,
    pub updated_at: String,
}

/// One source-backed index close selected from a distinct persisted market date. This is an
/// infrastructure record; period calculations remain in the Dashboard service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalMarketIndexQuoteRecord {
    pub name: String,
    pub symbol: String,
    pub current_price: String,
    pub change_percent: Option<String>,
    pub source: String,
    pub status: String,
    pub market_timestamp: String,
    pub fetched_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualRefreshRun {
    pub id: i64,
    pub holdings_snapshot_id: Option<i64>,
    pub indices_snapshot_id: Option<i64>,
    pub portfolio_json: String,
    pub completed_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredMarketQuote {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub current_price: String,
    pub previous_close: String,
    pub price_change: String,
    pub change_percent: String,
    pub volume: String,
    pub turnover_amount: String,
    pub market_timestamp: String,
    pub fetched_at: String,
    pub source: String,
    pub delay_status: String,
}

#[derive(Debug, Clone)]
pub struct NewManualRefreshRun {
    pub started_at: String,
    pub completed_at: String,
    pub holdings_snapshot_id: Option<i64>,
    pub indices_snapshot_id: Option<i64>,
    pub portfolio_json: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct NewAiReviewContext {
    pub review_id: i64,
    pub manual_refresh_run_id: i64,
    pub portfolio_json: String,
    pub market_json: String,
    pub news_json: String,
    pub events_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsArticle {
    pub id: i64,
    pub title: String,
    pub source: String,
    pub source_type: String,
    pub published_at: String,
    pub fetch_time: String,
    pub summary: String,
    pub url: String,
    pub related_security_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNewsArticle {
    pub title: String,
    pub source: String,
    pub source_type: String,
    pub published_at: String,
    pub fetch_time: String,
    pub summary: String,
    pub url: String,
    pub related_security_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsArticleUpdate {
    pub title: String,
    pub source: String,
    pub source_type: String,
    pub published_at: String,
    pub fetch_time: String,
    pub summary: String,
    pub url: String,
    pub related_security_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsArticleWithSecurity {
    pub article: NewsArticle,
    pub related_security_name: Option<String>,
    pub related_security_symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketSnapshotReference {
    pub id: i64,
    pub source: String,
    pub market_timestamp: String,
    pub fetched_at: String,
    pub delay_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyReview {
    pub id: i64,
    pub review_date: String,
    pub snapshot_id: Option<i64>,
    pub portfolio_summary: String,
    pub market_summary: String,
    pub holding_summary: String,
    pub risk_summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDailyReview {
    pub review_date: String,
    pub snapshot_id: Option<i64>,
    pub portfolio_summary: String,
    pub market_summary: String,
    pub holding_summary: String,
    pub risk_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiReview {
    pub id: i64,
    pub review_id: i64,
    pub model: String,
    pub prompt_version: String,
    pub context_id: Option<i64>,
    pub provider: String,
    pub request_status: String,
    pub error_code: Option<String>,
    pub facts: String,
    pub inferences: String,
    pub risks: String,
    pub report_json: Option<String>,
    pub security_id: Option<i64>,
    pub security_name: Option<String>,
    pub security_symbol: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAiReview {
    pub review_id: i64,
    pub model: String,
    pub prompt_version: String,
    pub context_id: i64,
    pub provider: String,
    pub facts: String,
    pub inferences: String,
    pub risks: String,
    pub report_json: Option<String>,
    pub security_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderSetting {
    pub provider: String,
    pub model: String,
    pub enabled: bool,
    pub priority: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchlistItemData {
    pub watchlist_item_id: i64,
    pub security_id: i64,
    pub name: String,
    pub symbol: String,
    pub market: String,
    pub security_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub id: i64,
    pub event_type: String,
    pub title: String,
    pub security_id: Option<i64>,
    pub event_time: String,
    pub timezone: String,
    pub source: String,
    pub source_url: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEventRecord {
    pub event_type: String,
    pub title: String,
    pub security_id: Option<i64>,
    pub event_time: String,
    pub timezone: String,
    pub source: String,
    pub source_url: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecordUpdate {
    pub event_type: String,
    pub title: String,
    pub security_id: Option<i64>,
    pub event_time: String,
    pub timezone: String,
    pub source: String,
    pub source_url: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventWithSecurity {
    pub event: EventRecord,
    pub security_name: Option<String>,
    pub security_symbol: Option<String>,
    pub holding_related: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holding {
    pub id: i64,
    pub cash_account_id: i64,
    pub security_id: i64,
    pub quantity: i64,
    pub available_quantity: i64,
    pub average_cost: String,
    pub cost_amount: String,
    pub position_source: String,
    pub as_of_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewHolding {
    pub cash_account_id: i64,
    pub security_id: i64,
    pub quantity: i64,
    pub available_quantity: i64,
    pub average_cost: String,
    pub cost_amount: String,
    pub position_source: String,
    pub as_of_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingUpdate {
    pub quantity: i64,
    pub available_quantity: i64,
    pub average_cost: String,
    pub cost_amount: String,
    pub as_of_date: Option<String>,
}

/// Read model used by the Portfolio UI service. It retains raw persisted values; all financial
/// parsing and calculation remain in the Rust Portfolio and Market services.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioHoldingData {
    pub holding_id: i64,
    pub security_id: i64,
    pub name: String,
    pub symbol: String,
    pub market: String,
    pub security_type: String,
    pub trade_rule: String,
    pub quantity: i64,
    pub available_quantity: i64,
    pub average_cost: String,
    pub cost_amount: String,
    pub current_price: Option<String>,
    pub previous_close: Option<String>,
    pub change_percent: Option<String>,
    pub quote_status: Option<String>,
    pub transaction_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub id: i64,
    pub cash_account_id: i64,
    pub security_id: i64,
    pub side: String,
    pub status: String,
    pub record_source: String,
    pub trade_date: String,
    pub quantity: i64,
    pub price: String,
    pub commission: String,
    pub stamp_tax: String,
    pub transfer_fee: String,
    pub other_fee: String,
    pub minimum_commission: String,
    pub external_reference: Option<String>,
    pub import_batch_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTransaction {
    pub cash_account_id: i64,
    pub security_id: i64,
    pub side: String,
    pub status: String,
    pub record_source: String,
    pub trade_date: String,
    pub quantity: i64,
    pub price: String,
    pub commission: String,
    pub stamp_tax: String,
    pub transfer_fee: String,
    pub other_fee: String,
    pub minimum_commission: String,
    pub external_reference: Option<String>,
    pub import_batch_id: Option<String>,
    pub note: Option<String>,
}

/// SQLite access boundary for the Rust application layer. The UI has no direct database access.
pub struct DatabaseService {
    connection: Connection,
}

impl DatabaseService {
    pub fn open(path: impl AsRef<Path>) -> DatabaseResult<Self> {
        let mut connection = Connection::open(path)?;
        migrations::apply(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn open_in_memory() -> DatabaseResult<Self> {
        let mut connection = Connection::open_in_memory()?;
        migrations::apply(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn initialize_app_database(app: &tauri::AppHandle) -> DatabaseResult<()> {
        Self::open_app_database(app)?;
        Ok(())
    }

    /// Opens the application database at the same local path used during application startup.
    /// Application services use this boundary; Tauri commands never expose SQLite to the UI.
    pub fn open_app_database(app: &tauri::AppHandle) -> DatabaseResult<Self> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| DatabaseError::AppPath(error.to_string()))?;
        fs::create_dir_all(&app_data_dir)?;
        Self::open(app_data_dir.join("astock-ai-workbench.sqlite3"))
    }

    /// Persists non-sensitive application state. Provider keys must never use this store.
    pub fn set_app_setting(&self, key: &str, value: &str) -> DatabaseResult<()> {
        self.connection.execute(
            "
            INSERT INTO app_settings (setting_key, setting_value)
            VALUES (?1, ?2)
            ON CONFLICT(setting_key) DO UPDATE SET
                setting_value = excluded.setting_value,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_app_setting(&self, key: &str) -> DatabaseResult<Option<String>> {
        self.connection
            .query_row(
                "SELECT setting_value FROM app_settings WHERE setting_key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn create_security(&self, input: NewSecurity) -> DatabaseResult<Security> {
        self.connection.execute(
            "
            INSERT INTO securities (
                symbol, name, market, exchange, instrument_type, security_type,
                industry, concepts_json, trading_rule, trade_rule
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                input.symbol,
                input.name,
                input.market,
                input.exchange,
                input.security_type,
                input.security_type,
                input.industry,
                input.concepts_json,
                input.trade_rule,
                input.trade_rule,
            ],
        )?;
        self.get_security(self.connection.last_insert_rowid())
    }

    pub fn create_cash_account(&self, input: NewCashAccount) -> DatabaseResult<CashAccount> {
        self.connection.execute(
            "
            INSERT INTO cash_accounts (name, currency, available_to_buy, withdrawable_cash, pending_settlement)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                input.name,
                input.currency,
                input.available_to_buy,
                input.withdrawable_cash,
                input.pending_settlement,
            ],
        )?;
        self.get_cash_account(self.connection.last_insert_rowid())
    }

    pub fn create_news_article(&self, input: NewNewsArticle) -> DatabaseResult<NewsArticle> {
        self.connection.execute(
            "
            INSERT INTO news_articles (
                title, source, source_type, published_at, fetch_time, summary, url, related_security_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                input.title,
                input.source,
                input.source_type,
                input.published_at,
                input.fetch_time,
                input.summary,
                input.url,
                input.related_security_id,
            ],
        )?;
        let article_id = self.connection.last_insert_rowid();
        if let Some(security_id) = input.related_security_id {
            self.link_news_article_to_security(article_id, security_id)?;
        }
        self.get_news_article(article_id)
    }

    pub fn upsert_news_article(&self, input: NewNewsArticle) -> DatabaseResult<NewsArticle> {
        let url = input.url.clone();
        self.connection.execute(
            "INSERT INTO news_articles (title, source, source_type, published_at, fetch_time, summary, url, related_security_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(url) DO UPDATE SET title=excluded.title, source=excluded.source,
                source_type=excluded.source_type, published_at=excluded.published_at,
                fetch_time=excluded.fetch_time, summary=excluded.summary, related_security_id=excluded.related_security_id",
            params![input.title, input.source, input.source_type, input.published_at, input.fetch_time,
                input.summary, input.url, input.related_security_id],
        )?;
        let article = self.connection.query_row("SELECT id, title, source, source_type, published_at, fetch_time, summary, url, related_security_id, created_at FROM news_articles WHERE url = ?1", [url], Self::map_news_article).map_err(DatabaseError::from)?;
        if let Some(security_id) = input.related_security_id {
            self.link_news_article_to_security(article.id, security_id)?;
        }
        Ok(article)
    }

    pub fn upsert_daily_review(&self, input: NewDailyReview) -> DatabaseResult<DailyReview> {
        let review_date = input.review_date.clone();
        self.connection.execute(
            "
            INSERT INTO daily_reviews (
                review_date, snapshot_id, portfolio_summary, market_summary, holding_summary, risk_summary
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(review_date) DO UPDATE SET
                snapshot_id = excluded.snapshot_id,
                portfolio_summary = excluded.portfolio_summary,
                market_summary = excluded.market_summary,
                holding_summary = excluded.holding_summary,
                risk_summary = excluded.risk_summary
            ",
            params![
                input.review_date,
                input.snapshot_id,
                input.portfolio_summary,
                input.market_summary,
                input.holding_summary,
                input.risk_summary,
            ],
        )?;
        self.get_daily_review_by_date(&review_date)
    }

    pub fn create_ai_review(&self, input: NewAiReview) -> DatabaseResult<AiReview> {
        self.connection.execute(
            "
            INSERT INTO ai_reviews (
                review_id, model, prompt_version, context_id, provider, request_status,
                facts, inferences, risks, report_json, security_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'COMPLETED', ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                input.review_id,
                input.model,
                input.prompt_version,
                input.context_id,
                input.provider,
                input.facts,
                input.inferences,
                input.risks,
                input.report_json,
                input.security_id,
            ],
        )?;
        self.get_ai_review(self.connection.last_insert_rowid())
    }

    pub fn create_manual_refresh_run(
        &self,
        input: NewManualRefreshRun,
    ) -> DatabaseResult<ManualRefreshRun> {
        self.connection.execute(
            "
            INSERT INTO manual_refresh_runs (
                started_at, completed_at, holdings_snapshot_id, indices_snapshot_id, portfolio_json,
                news_status, events_status, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'NO_DATA', 'NO_DATA', ?6)
            ",
            params![
                input.started_at,
                input.completed_at,
                input.holdings_snapshot_id,
                input.indices_snapshot_id,
                input.portfolio_json,
                input.status,
            ],
        )?;
        self.get_manual_refresh_run(self.connection.last_insert_rowid())
    }

    pub fn get_manual_refresh_run(&self, id: i64) -> DatabaseResult<ManualRefreshRun> {
        self.connection
            .query_row(
                "SELECT id, holdings_snapshot_id, indices_snapshot_id, portfolio_json, completed_at
             FROM manual_refresh_runs WHERE id = ?1",
                [id],
                |row| {
                    Ok(ManualRefreshRun {
                        id: row.get(0)?,
                        holdings_snapshot_id: row.get(1)?,
                        indices_snapshot_id: row.get(2)?,
                        portfolio_json: row.get(3)?,
                        completed_at: row.get(4)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn latest_manual_refresh_run(&self) -> DatabaseResult<Option<ManualRefreshRun>> {
        self.connection
            .query_row(
                "SELECT id, holdings_snapshot_id, indices_snapshot_id, portfolio_json, completed_at
             FROM manual_refresh_runs ORDER BY completed_at DESC, id DESC LIMIT 1",
                [],
                |row| {
                    Ok(ManualRefreshRun {
                        id: row.get(0)?,
                        holdings_snapshot_id: row.get(1)?,
                        indices_snapshot_id: row.get(2)?,
                        portfolio_json: row.get(3)?,
                        completed_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn create_ai_review_context(&self, input: NewAiReviewContext) -> DatabaseResult<i64> {
        self.connection.execute(
            "INSERT INTO ai_review_contexts (
                review_id, manual_refresh_run_id, portfolio_json, market_json, news_json, events_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![input.review_id, input.manual_refresh_run_id, input.portfolio_json,
                input.market_json, input.news_json, input.events_json],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    #[cfg(test)]
    pub fn ai_review_context_count(&self) -> DatabaseResult<i64> {
        self.connection
            .query_row("SELECT COUNT(*) FROM ai_review_contexts", [], |row| {
                row.get(0)
            })
            .map_err(Into::into)
    }

    pub fn create_event(&self, input: NewEventRecord) -> DatabaseResult<EventRecord> {
        self.connection.execute(
            "
            INSERT INTO events (
                event_type, title, security_id, event_time, timezone, source, source_url, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                input.event_type,
                input.title,
                input.security_id,
                input.event_time,
                input.timezone,
                input.source,
                input.source_url,
                input.status,
            ],
        )?;
        let event_id = self.connection.last_insert_rowid();
        if let Some(security_id) = input.security_id {
            self.link_event_to_security(event_id, security_id)?;
        }
        self.get_event(event_id)
    }

    pub fn upsert_event_by_source_url(&self, input: NewEventRecord) -> DatabaseResult<EventRecord> {
        if let Some(source_url) = input.source_url.as_deref() {
            let existing: Option<i64> = self
                .connection
                .query_row(
                    "SELECT id FROM events WHERE source = ?1 AND source_url = ?2",
                    params![&input.source, source_url],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(id) = existing {
                self.connection.execute(
                    "UPDATE events SET event_type=?1, title=?2, security_id=?3, event_time=?4, timezone=?5, source=?6, source_url=?7, status=?8 WHERE id=?9",
                    params![input.event_type, input.title, input.security_id, input.event_time, input.timezone, input.source, input.source_url, input.status, id],
                )?;
                if let Some(security_id) = input.security_id {
                    self.link_event_to_security(id, security_id)?;
                }
                return self.get_event(id);
            }
        }
        self.create_event(input)
    }

    pub fn get_event(&self, id: i64) -> DatabaseResult<EventRecord> {
        self.connection
            .query_row(
                "
                SELECT id, event_type, title, security_id, event_time, timezone, source, source_url,
                       status, created_at
                FROM events WHERE id = ?1
                ",
                [id],
                Self::map_event,
            )
            .map_err(Into::into)
    }

    pub fn update_event(&self, id: i64, input: EventRecordUpdate) -> DatabaseResult<EventRecord> {
        self.connection.execute(
            "
            UPDATE events
            SET event_type = ?1, title = ?2, security_id = ?3, event_time = ?4, timezone = ?5,
                source = ?6, source_url = ?7, status = ?8
            WHERE id = ?9
            ",
            params![
                input.event_type,
                input.title,
                input.security_id,
                input.event_time,
                input.timezone,
                input.source,
                input.source_url,
                input.status,
                id,
            ],
        )?;
        self.connection
            .execute("DELETE FROM event_security_links WHERE event_id = ?1", [id])?;
        if let Some(security_id) = input.security_id {
            self.link_event_to_security(id, security_id)?;
        }
        self.get_event(id)
    }

    pub fn delete_event(&self, id: i64) -> DatabaseResult<usize> {
        self.connection
            .execute("DELETE FROM events WHERE id = ?1", [id])
            .map_err(Into::into)
    }

    /// Adds a shareable event-to-security association. The event body remains one record even
    /// when several followed securities point to the same disclosed source.
    pub fn link_event_to_security(&self, event_id: i64, security_id: i64) -> DatabaseResult<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO event_security_links (event_id, security_id) VALUES (?1, ?2)",
            params![event_id, security_id],
        )?;
        Ok(())
    }

    pub fn get_ai_review(&self, id: i64) -> DatabaseResult<AiReview> {
        self.connection
            .query_row(
                "
                SELECT ar.id, ar.review_id, ar.model, ar.prompt_version, ar.context_id, ar.provider, ar.request_status,
                       ar.error_code, ar.facts, ar.inferences, ar.risks, ar.report_json,
                       ar.security_id, s.name, s.symbol, ar.created_at
                FROM ai_reviews ar LEFT JOIN securities s ON s.id = ar.security_id WHERE ar.id = ?1
                ",
                [id],
                Self::map_ai_review,
            )
            .map_err(Into::into)
    }

    pub fn latest_ai_review_for_daily_review(
        &self,
        review_id: i64,
    ) -> DatabaseResult<Option<AiReview>> {
        self.connection
            .query_row(
                "
                SELECT ar.id, ar.review_id, ar.model, ar.prompt_version, ar.context_id, ar.provider, ar.request_status,
                       ar.error_code, ar.facts, ar.inferences, ar.risks, ar.report_json,
                       ar.security_id, s.name, s.symbol, ar.created_at
                FROM ai_reviews ar LEFT JOIN securities s ON s.id = ar.security_id WHERE ar.review_id = ?1
                ORDER BY ar.created_at DESC, ar.id DESC
                LIMIT 1
                ",
                [review_id],
                Self::map_ai_review,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_ai_reviews_for_daily_review(
        &self,
        review_id: i64,
    ) -> DatabaseResult<Vec<AiReview>> {
        let mut statement = self.connection.prepare(
            "
            SELECT ar.id, ar.review_id, ar.model, ar.prompt_version, ar.context_id, ar.provider, ar.request_status,
                   ar.error_code, ar.facts, ar.inferences, ar.risks, ar.report_json,
                   ar.security_id, s.name, s.symbol, ar.created_at
            FROM ai_reviews ar LEFT JOIN securities s ON s.id = ar.security_id
            WHERE ar.review_id = ?1
            ORDER BY CASE WHEN ar.security_id IS NULL THEN 1 ELSE 0 END, ar.created_at DESC, ar.id DESC
            ",
        )?;
        let rows = statement.query_map([review_id], Self::map_ai_review)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_ai_provider_settings(&self) -> DatabaseResult<Vec<AiProviderSetting>> {
        let mut statement = self.connection.prepare(
            "SELECT provider, model, enabled, priority FROM ai_provider_settings ORDER BY priority ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AiProviderSetting {
                provider: row.get(0)?,
                model: row.get(1)?,
                enabled: row.get::<_, i64>(2)? != 0,
                priority: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn set_ai_provider_enabled(&self, provider: &str, enabled: bool) -> DatabaseResult<()> {
        if self.connection.execute(
            "UPDATE ai_provider_settings SET enabled = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE provider = ?1",
            params![provider, i64::from(enabled)],
        )? == 0 {
            return Err(DatabaseError::AppPath("AI Provider 未确认".into()));
        }
        Ok(())
    }

    pub fn list_events(&self) -> DatabaseResult<Vec<EventWithSecurity>> {
        let mut statement = self.connection.prepare(
            "
            SELECT e.id, e.event_type, e.title, e.security_id, e.event_time, e.timezone, e.source,
                   e.source_url, e.status, e.created_at, s.name, s.symbol,
                   CASE WHEN EXISTS (
                       SELECT 1 FROM event_security_links l
                       JOIN watchlist_items w ON w.security_id = l.security_id
                       WHERE l.event_id = e.id
                   ) THEN 1 ELSE 0 END
            FROM events e
            LEFT JOIN securities s ON s.id = e.security_id
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(EventWithSecurity {
                event: Self::map_event(row)?,
                security_name: row.get(10)?,
                security_symbol: row.get(11)?,
                holding_related: row.get::<_, i64>(12)? == 1,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn link_news_articles_to_manual_refresh_run(
        &self,
        run_id: i64,
        article_ids: &[i64],
    ) -> DatabaseResult<()> {
        for article_id in article_ids {
            self.connection.execute("INSERT OR IGNORE INTO manual_refresh_news_articles (manual_refresh_run_id, news_article_id) VALUES (?1, ?2)", params![run_id, article_id])?;
        }
        Ok(())
    }

    pub fn link_events_to_manual_refresh_run(
        &self,
        run_id: i64,
        event_ids: &[i64],
    ) -> DatabaseResult<()> {
        for event_id in event_ids {
            self.connection.execute("INSERT OR IGNORE INTO manual_refresh_events (manual_refresh_run_id, event_id) VALUES (?1, ?2)", params![run_id, event_id])?;
        }
        Ok(())
    }

    pub fn get_daily_review_by_date(&self, review_date: &str) -> DatabaseResult<DailyReview> {
        self.connection
            .query_row(
                "
                SELECT id, review_date, snapshot_id, portfolio_summary, market_summary, holding_summary,
                       risk_summary, created_at
                FROM daily_reviews WHERE review_date = ?1
                ",
                [review_date],
                Self::map_daily_review,
            )
            .map_err(Into::into)
    }

    pub fn get_news_article(&self, id: i64) -> DatabaseResult<NewsArticle> {
        self.connection
            .query_row(
                "
                SELECT id, title, source, source_type, published_at, fetch_time, summary, url,
                       related_security_id, created_at
                FROM news_articles WHERE id = ?1
                ",
                [id],
                Self::map_news_article,
            )
            .map_err(Into::into)
    }

    pub fn update_news_article(
        &self,
        id: i64,
        input: NewsArticleUpdate,
    ) -> DatabaseResult<NewsArticle> {
        self.connection.execute(
            "
            UPDATE news_articles
            SET title = ?1, source = ?2, source_type = ?3, published_at = ?4, fetch_time = ?5,
                summary = ?6, url = ?7, related_security_id = ?8
            WHERE id = ?9
            ",
            params![
                input.title,
                input.source,
                input.source_type,
                input.published_at,
                input.fetch_time,
                input.summary,
                input.url,
                input.related_security_id,
                id,
            ],
        )?;
        self.connection.execute(
            "DELETE FROM news_security_links WHERE news_article_id = ?1",
            [id],
        )?;
        if let Some(security_id) = input.related_security_id {
            self.link_news_article_to_security(id, security_id)?;
        }
        self.get_news_article(id)
    }

    pub fn delete_news_article(&self, id: i64) -> DatabaseResult<usize> {
        self.connection
            .execute("DELETE FROM news_articles WHERE id = ?1", [id])
            .map_err(Into::into)
    }

    /// Adds a shareable news-to-security association while retaining the legacy primary
    /// association column for existing list views and external compatibility.
    pub fn link_news_article_to_security(
        &self,
        article_id: i64,
        security_id: i64,
    ) -> DatabaseResult<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO news_security_links (news_article_id, security_id) VALUES (?1, ?2)",
            params![article_id, security_id],
        )?;
        Ok(())
    }

    pub fn list_news_articles(&self) -> DatabaseResult<Vec<NewsArticleWithSecurity>> {
        self.list_news_articles_by_scope("")
    }

    pub fn list_news_articles_for_holdings(&self) -> DatabaseResult<Vec<NewsArticleWithSecurity>> {
        self.list_news_articles_by_scope(
            "
            WHERE EXISTS (
                SELECT 1
                FROM news_security_links l
                JOIN watchlist_items w ON w.security_id = l.security_id
                WHERE l.news_article_id = n.id
            )
            ",
        )
    }

    pub fn list_news_articles_for_manual_refresh_run(
        &self,
        run_id: i64,
    ) -> DatabaseResult<Vec<NewsArticleWithSecurity>> {
        self.list_news_articles_by_scope(&format!("JOIN manual_refresh_news_articles mrna ON mrna.news_article_id = n.id WHERE mrna.manual_refresh_run_id = {run_id}"))
    }

    pub fn list_events_for_manual_refresh_run(
        &self,
        run_id: i64,
    ) -> DatabaseResult<Vec<EventWithSecurity>> {
        let mut statement = self.connection.prepare(
            "SELECT e.id, e.event_type, e.title, e.security_id, e.event_time, e.timezone, e.source, e.source_url, e.status, e.created_at, s.name, s.symbol,
                    CASE WHEN EXISTS (SELECT 1 FROM event_security_links l JOIN watchlist_items w ON w.security_id=l.security_id WHERE l.event_id=e.id) THEN 1 ELSE 0 END
             FROM events e JOIN manual_refresh_events mre ON mre.event_id=e.id LEFT JOIN securities s ON s.id=e.security_id
             WHERE mre.manual_refresh_run_id=?1"
        )?;
        let rows = statement.query_map([run_id], |row| {
            Ok(EventWithSecurity {
                event: Self::map_event(row)?,
                security_name: row.get(10)?,
                security_symbol: row.get(11)?,
                holding_related: row.get::<_, i64>(12)? == 1,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_cash_accounts(&self) -> DatabaseResult<Vec<CashAccount>> {
        let mut statement = self.connection.prepare(
            "
            SELECT id, name, currency, available_to_buy, withdrawable_cash, pending_settlement
            FROM cash_accounts
            ORDER BY id ASC
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(CashAccount {
                id: row.get(0)?,
                name: row.get(1)?,
                currency: row.get(2)?,
                available_to_buy: row.get(3)?,
                withdrawable_cash: row.get(4)?,
                pending_settlement: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Returns only securities explicitly followed by the user. The refresh service never
    /// invents symbols or asks a provider for arbitrary codes.
    pub fn list_market_securities_for_holdings(&self) -> DatabaseResult<Vec<MarketSecurity>> {
        let mut statement = self.connection.prepare(
            "
            SELECT DISTINCT s.id, s.symbol, s.name, s.market
            FROM watchlist_items w
            JOIN securities s ON s.id = w.security_id
            ORDER BY w.created_at DESC, w.id DESC
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(MarketSecurity {
                security_id: row.get(0)?,
                symbol: row.get(1)?,
                name: row.get(2)?,
                market: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_watchlist_item_data(&self) -> DatabaseResult<Vec<WatchlistItemData>> {
        let mut statement = self.connection.prepare(
            "
            SELECT w.id, s.id, s.name, s.symbol, s.market, s.security_type, w.created_at
            FROM watchlist_items w
            JOIN securities s ON s.id = w.security_id
            ORDER BY w.created_at DESC, w.id DESC
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(WatchlistItemData {
                watchlist_item_id: row.get(0)?,
                security_id: row.get(1)?,
                name: row.get(2)?,
                symbol: row.get(3)?,
                market: row.get(4)?,
                security_type: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn create_watchlist_item(&self, security_id: i64) -> DatabaseResult<WatchlistItemData> {
        self.connection.execute(
            "INSERT INTO watchlist_items (security_id) VALUES (?1)",
            [security_id],
        )?;
        self.get_watchlist_item_data(self.connection.last_insert_rowid())
    }

    pub fn ensure_watchlist_item(&self, security_id: i64) -> DatabaseResult<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO watchlist_items (security_id) VALUES (?1)",
            [security_id],
        )?;
        Ok(())
    }

    pub fn find_watchlist_item_by_security(
        &self,
        security_id: i64,
    ) -> DatabaseResult<Option<WatchlistItemData>> {
        self.connection
            .query_row(
                "
                SELECT w.id, s.id, s.name, s.symbol, s.market, s.security_type, w.created_at
                FROM watchlist_items w JOIN securities s ON s.id = w.security_id
                WHERE w.security_id = ?1
                ",
                [security_id],
                |row| {
                    Ok(WatchlistItemData {
                        watchlist_item_id: row.get(0)?,
                        security_id: row.get(1)?,
                        name: row.get(2)?,
                        symbol: row.get(3)?,
                        market: row.get(4)?,
                        security_type: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Permanently removes one followed security and every security-owned local record in a
    /// single SQLite transaction. Global index snapshots, data-source configuration, cash
    /// accounts and any news/event/AI context shared with another security are intentionally
    /// retained. If any statement fails, the transaction is rolled back before returning.
    pub fn remove_followed_security_completely(
        &self,
        watchlist_item_id: i64,
        security_id: i64,
    ) -> DatabaseResult<()> {
        let transaction = self.connection.unchecked_transaction()?;
        let result = Self::remove_followed_security_with_transaction(
            &transaction,
            watchlist_item_id,
            security_id,
            false,
        );
        match result {
            Ok(()) => transaction.commit().map_err(Into::into),
            Err(error) => {
                let _ = transaction.rollback();
                Err(error)
            }
        }
    }

    #[cfg(test)]
    fn remove_followed_security_with_forced_failure(
        &self,
        watchlist_item_id: i64,
        security_id: i64,
    ) -> DatabaseResult<()> {
        let transaction = self.connection.unchecked_transaction()?;
        let result = Self::remove_followed_security_with_transaction(
            &transaction,
            watchlist_item_id,
            security_id,
            true,
        );
        match result {
            Ok(()) => transaction.commit().map_err(Into::into),
            Err(error) => {
                let _ = transaction.rollback();
                Err(error)
            }
        }
    }

    fn remove_followed_security_with_transaction(
        transaction: &SqliteTransaction<'_>,
        watchlist_item_id: i64,
        security_id: i64,
        force_failure_after_first_delete: bool,
    ) -> DatabaseResult<()> {
        let watchlist_exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM watchlist_items WHERE id = ?1 AND security_id = ?2",
                params![watchlist_item_id, security_id],
                |row| row.get(0),
            )
            .optional()?;
        if watchlist_exists.is_none() {
            return Err(DatabaseError::AppPath("未找到当前关注关系".into()));
        }

        // Delete AI-only evidence first. Shared contexts are retained for other reports.
        transaction.execute(
            "DELETE FROM ai_review_contexts
             WHERE id IN (
                 SELECT ar.context_id FROM ai_reviews ar
                 WHERE ar.security_id = ?1 AND ar.context_id IS NOT NULL
             )
             AND NOT EXISTS (
                 SELECT 1 FROM ai_reviews other
                 WHERE other.context_id = ai_review_contexts.id
                   AND (other.security_id IS NULL OR other.security_id <> ?1)
             )",
            [security_id],
        )?;
        transaction.execute(
            "DELETE FROM ai_reviews WHERE security_id = ?1",
            [security_id],
        )?;

        // Only delete an article/event body when every one of its security associations belongs
        // to the removed follow. Shared bodies keep their other links and refresh provenance.
        transaction.execute(
            "DELETE FROM manual_refresh_news_articles
             WHERE news_article_id IN (
                 SELECT l.news_article_id FROM news_security_links l
                 WHERE l.security_id = ?1
                   AND NOT EXISTS (
                     SELECT 1 FROM news_security_links other
                     WHERE other.news_article_id = l.news_article_id
                       AND other.security_id <> ?1
                   )
             )",
            [security_id],
        )?;
        transaction.execute(
            "DELETE FROM news_articles
             WHERE id IN (
                 SELECT l.news_article_id FROM news_security_links l
                 WHERE l.security_id = ?1
                   AND NOT EXISTS (
                     SELECT 1 FROM news_security_links other
                     WHERE other.news_article_id = l.news_article_id
                       AND other.security_id <> ?1
                   )
             )",
            [security_id],
        )?;
        transaction.execute(
            "DELETE FROM manual_refresh_events
             WHERE event_id IN (
                 SELECT l.event_id FROM event_security_links l
                 WHERE l.security_id = ?1
                   AND NOT EXISTS (
                     SELECT 1 FROM event_security_links other
                     WHERE other.event_id = l.event_id
                       AND other.security_id <> ?1
                   )
             )",
            [security_id],
        )?;
        transaction.execute(
            "DELETE FROM events
             WHERE id IN (
                 SELECT l.event_id FROM event_security_links l
                 WHERE l.security_id = ?1
                   AND NOT EXISTS (
                     SELECT 1 FROM event_security_links other
                     WHERE other.event_id = l.event_id
                       AND other.security_id <> ?1
                   )
             )",
            [security_id],
        )?;

        transaction.execute(
            "DELETE FROM news_security_links WHERE security_id = ?1",
            [security_id],
        )?;
        transaction.execute(
            "DELETE FROM event_security_links WHERE security_id = ?1",
            [security_id],
        )?;
        transaction.execute(
            "UPDATE news_articles
             SET related_security_id = (
                 SELECT security_id FROM news_security_links l
                 WHERE l.news_article_id = news_articles.id
                 ORDER BY l.security_id ASC LIMIT 1
             )
             WHERE related_security_id = ?1",
            [security_id],
        )?;
        transaction.execute(
            "UPDATE events
             SET security_id = (
                 SELECT security_id FROM event_security_links l
                 WHERE l.event_id = events.id
                 ORDER BY l.security_id ASC LIMIT 1
             )
             WHERE security_id = ?1",
            [security_id],
        )?;

        transaction.execute(
            "DELETE FROM market_quotes WHERE security_id = ?1",
            [security_id],
        )?;
        if force_failure_after_first_delete {
            return Err(DatabaseError::AppPath("测试注入：删除事务必须回滚".into()));
        }
        transaction.execute(
            "DELETE FROM corporate_actions WHERE security_id = ?1",
            [security_id],
        )?;
        transaction.execute(
            "DELETE FROM transactions WHERE security_id = ?1",
            [security_id],
        )?;
        transaction.execute("DELETE FROM holdings WHERE security_id = ?1", [security_id])?;
        transaction.execute(
            "DELETE FROM watchlist_items WHERE id = ?1 AND security_id = ?2",
            params![watchlist_item_id, security_id],
        )?;
        if transaction.execute("DELETE FROM securities WHERE id = ?1", [security_id])? == 0 {
            return Err(DatabaseError::AppPath("未找到需要删除的证券".into()));
        }
        Ok(())
    }

    fn get_watchlist_item_data(&self, id: i64) -> DatabaseResult<WatchlistItemData> {
        self.connection
            .query_row(
                "
                SELECT w.id, s.id, s.name, s.symbol, s.market, s.security_type, w.created_at
                FROM watchlist_items w JOIN securities s ON s.id = w.security_id
                WHERE w.id = ?1
                ",
                [id],
                |row| {
                    Ok(WatchlistItemData {
                        watchlist_item_id: row.get(0)?,
                        security_id: row.get(1)?,
                        name: row.get(2)?,
                        symbol: row.get(3)?,
                        market: row.get(4)?,
                        security_type: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn has_confirmed_transactions(&self) -> DatabaseResult<bool> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM transactions WHERE status = 'CONFIRMED')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|exists| exists != 0)
            .map_err(Into::into)
    }

    pub fn next_cny_cash_account_name(&self) -> DatabaseResult<String> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM cash_accounts WHERE name LIKE '人民币现金账户%'",
            [],
            |row| row.get(0),
        )?;
        Ok(format!("人民币现金账户-{}", count + 1))
    }

    pub fn latest_market_source_status(&self) -> DatabaseResult<Option<MarketSourceStatus>> {
        self.connection
            .query_row(
                "
                SELECT name, status, last_success_at
                FROM data_sources
                WHERE source_type = 'MARKET'
                ORDER BY
                    CASE WHEN last_success_at IS NULL THEN 1 ELSE 0 END,
                    last_success_at DESC,
                    id DESC
                LIMIT 1
                ",
                [],
                |row| {
                    Ok(MarketSourceStatus {
                        name: row.get(0)?,
                        status: row.get(1)?,
                        last_success_at: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn latest_market_snapshot_for_review_date(
        &self,
        review_date: &str,
    ) -> DatabaseResult<Option<MarketSnapshotReference>> {
        self.connection
            .query_row(
                "
                SELECT ms.id, ds.name, ms.market_timestamp, ms.fetched_at, ms.delay_status
                FROM market_snapshots ms
                JOIN data_sources ds ON ds.id = ms.data_source_id
                WHERE date(datetime(ms.market_timestamp, '+8 hours')) = ?1
                ORDER BY ms.market_timestamp DESC, ms.id DESC
                LIMIT 1
                ",
                [review_date],
                |row| {
                    Ok(MarketSnapshotReference {
                        id: row.get(0)?,
                        source: row.get(1)?,
                        market_timestamp: row.get(2)?,
                        fetched_at: row.get(3)?,
                        delay_status: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Returns a snapshot only when it is the latest quote source for every current holding. This
    /// lets aggregate valuation disclose one precise source/time rather than claiming a mixed or
    /// stale set of quotes comes from a single refresh.
    pub fn latest_market_snapshot_for_current_holdings(
        &self,
    ) -> DatabaseResult<Option<MarketSnapshotReference>> {
        self.connection
            .query_row(
                "
                SELECT ms.id, ds.name, ms.market_timestamp, ms.fetched_at, ms.delay_status
                FROM market_snapshots ms
                JOIN data_sources ds ON ds.id = ms.data_source_id
                WHERE ms.delay_status IN ('REALTIME', 'CLOSED')
                  AND EXISTS (SELECT 1 FROM holdings WHERE quantity > 0)
                  AND NOT EXISTS (
                    SELECT 1
                    FROM holdings h
                    WHERE h.quantity > 0
                      AND COALESCE((
                        SELECT q.market_snapshot_id
                        FROM market_quotes q
                        WHERE q.security_id = h.security_id
                        ORDER BY q.market_timestamp DESC, q.id DESC
                        LIMIT 1
                      ), -1) != ms.id
                  )
                ORDER BY ms.id DESC
                LIMIT 1
                ",
                [],
                |row| {
                    Ok(MarketSnapshotReference {
                        id: row.get(0)?,
                        source: row.get(1)?,
                        market_timestamp: row.get(2)?,
                        fetched_at: row.get(3)?,
                        delay_status: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn latest_market_index_quotes(&self) -> DatabaseResult<Vec<MarketIndexQuoteRecord>> {
        let mut statement = self.connection.prepare(
            "
            WITH latest AS (
                SELECT miq.symbol, MAX(miq.market_timestamp) AS market_timestamp
                FROM market_index_quotes miq
                GROUP BY miq.symbol
            )
            SELECT miq.name, miq.symbol, miq.current_price, miq.change_pct,
                   ds.name, miq.delay_status, miq.market_timestamp
            FROM market_index_quotes miq
            JOIN latest ON latest.symbol = miq.symbol
                      AND latest.market_timestamp = miq.market_timestamp
            JOIN market_snapshots ms ON ms.id = miq.market_snapshot_id
            JOIN data_sources ds ON ds.id = ms.data_source_id
            ORDER BY CASE miq.symbol
                WHEN '000001.SH' THEN 1
                WHEN '399001.SZ' THEN 2
                WHEN '399006.SZ' THEN 3
                WHEN '000688.SH' THEN 4
                ELSE 99 END
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(MarketIndexQuoteRecord {
                name: row.get(0)?,
                symbol: row.get(1)?,
                current_price: row.get(2)?,
                change_percent: row.get(3)?,
                source: row.get(4)?,
                status: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Returns at most one real persisted quote per market date, newest first. Multiple manual
    /// refreshes during the same market date never inflate a multi-day average.
    pub fn recent_market_index_quotes_by_trading_day(
        &self,
        symbol: &str,
        limit: usize,
    ) -> DatabaseResult<Vec<HistoricalMarketIndexQuoteRecord>> {
        let mut statement = self.connection.prepare(
            "
            WITH ranked AS (
                SELECT miq.name, miq.symbol, miq.current_price,
                       COALESCE(miq.change_percent, miq.change_pct) AS change_percent,
                       ds.name AS source, miq.delay_status, miq.market_timestamp, miq.fetched_at,
                       ROW_NUMBER() OVER (
                           PARTITION BY date(miq.market_timestamp, '+8 hours')
                           ORDER BY miq.market_timestamp DESC, miq.fetched_at DESC, miq.id DESC
                       ) AS day_rank
                FROM market_index_quotes miq
                JOIN market_snapshots ms ON ms.id = miq.market_snapshot_id
                JOIN data_sources ds ON ds.id = ms.data_source_id
                WHERE miq.symbol = ?1 AND trim(miq.current_price) <> ''
            )
            SELECT name, symbol, current_price, change_percent, source, delay_status,
                   market_timestamp, fetched_at
            FROM ranked
            WHERE day_rank = 1
            ORDER BY market_timestamp DESC, fetched_at DESC
            LIMIT ?2
            ",
        )?;
        let rows = statement.query_map(params![symbol, limit as i64], |row| {
            Ok(HistoricalMarketIndexQuoteRecord {
                name: row.get(0)?,
                symbol: row.get(1)?,
                current_price: row.get(2)?,
                change_percent: row.get(3)?,
                source: row.get(4)?,
                status: row.get(5)?,
                market_timestamp: row.get(6)?,
                fetched_at: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Produces a consistent SQLite copy through SQLite itself. The caller must provide a unique
    /// destination; existing backups are never overwritten.
    pub fn backup_to(&self, destination: &Path) -> DatabaseResult<()> {
        if destination.exists() {
            return Err(DatabaseError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "backup destination already exists",
            )));
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        self.connection
            .execute("VACUUM INTO ?1", [destination.to_string_lossy().as_ref()])?;
        Ok(())
    }

    pub fn create_holding(&self, input: NewHolding) -> DatabaseResult<Holding> {
        self.connection.execute(
            "
            INSERT INTO holdings (
                cash_account_id, security_id, quantity, available_quantity, average_cost,
                cost_amount, position_source, as_of_date
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                input.cash_account_id,
                input.security_id,
                input.quantity,
                input.available_quantity,
                input.average_cost,
                input.cost_amount,
                input.position_source,
                input.as_of_date,
            ],
        )?;
        self.get_holding(self.connection.last_insert_rowid())
    }

    /// Persists the user-entered ledger record only. It intentionally performs no cost,
    /// P&L, cash balance, or T+1 calculation in Phase 2-A.
    pub fn create_transaction(&self, input: NewTransaction) -> DatabaseResult<Transaction> {
        self.connection.execute(
            "
            INSERT INTO transactions (
                cash_account_id, security_id, side, status, record_source, trade_date, quantity, price,
                commission, stamp_duty, stamp_tax, transfer_fee, other_fee, minimum_commission,
                external_reference, import_batch_id, note
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ",
            params![
                input.cash_account_id,
                input.security_id,
                input.side,
                input.status,
                input.record_source,
                input.trade_date,
                input.quantity,
                input.price,
                input.commission,
                input.stamp_tax,
                input.stamp_tax,
                input.transfer_fee,
                input.other_fee,
                input.minimum_commission,
                input.external_reference,
                input.import_batch_id,
                input.note,
            ],
        )?;
        self.get_transaction(self.connection.last_insert_rowid())
    }

    pub fn get_holding(&self, id: i64) -> DatabaseResult<Holding> {
        self.connection.query_row(
            "
            SELECT id, cash_account_id, security_id, quantity, available_quantity, average_cost, cost_amount, position_source, as_of_date
            FROM holdings WHERE id = ?1
            ",
            [id],
            Self::map_holding,
        ).map_err(Into::into)
    }

    pub fn update_holding(&self, id: i64, input: HoldingUpdate) -> DatabaseResult<Holding> {
        self.connection.execute(
            "
            UPDATE holdings
            SET quantity = ?1, available_quantity = ?2, average_cost = ?3, cost_amount = ?4, as_of_date = ?5,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?6
            ",
            params![
                input.quantity,
                input.available_quantity,
                input.average_cost,
                input.cost_amount,
                input.as_of_date,
                id,
            ],
        )?;
        self.get_holding(id)
    }

    /// Preserves the immutable transaction history while making a record inactive for later rules.
    pub fn cancel_transaction(&self, id: i64) -> DatabaseResult<Transaction> {
        self.connection.execute(
            "
                UPDATE transactions
                SET status = 'CANCELLED', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id = ?1
                ",
            [id],
        )?;
        self.get_transaction(id)
    }

    pub fn delete_holding(&self, id: i64) -> DatabaseResult<usize> {
        self.connection
            .execute("DELETE FROM holdings WHERE id = ?1", [id])
            .map_err(Into::into)
    }

    pub fn delete_security(&self, id: i64) -> DatabaseResult<usize> {
        self.connection
            .execute("DELETE FROM securities WHERE id = ?1", [id])
            .map_err(Into::into)
    }

    pub fn get_or_create_local_holding_account(&self) -> DatabaseResult<CashAccount> {
        let existing = self.connection.query_row(
            "SELECT id, name, currency, available_to_buy, withdrawable_cash, pending_settlement FROM cash_accounts WHERE name = '本地持仓账户'",
            [],
            |row| Ok(CashAccount {
                id: row.get(0)?,
                name: row.get(1)?,
                currency: row.get(2)?,
                available_to_buy: row.get(3)?,
                withdrawable_cash: row.get(4)?,
                pending_settlement: row.get(5)?,
            }),
        );
        match existing {
            Ok(account) => Ok(account),
            Err(rusqlite::Error::QueryReturnedNoRows) => self.create_cash_account(NewCashAccount {
                name: "本地持仓账户".into(),
                currency: "CNY".into(),
                available_to_buy: "0".into(),
                withdrawable_cash: "0".into(),
                pending_settlement: "0".into(),
            }),
            Err(error) => Err(error.into()),
        }
    }

    pub fn find_security_by_symbol_and_market(
        &self,
        symbol: &str,
        market: &str,
    ) -> DatabaseResult<Option<Security>> {
        self.connection
            .query_row(
                "
                SELECT id, symbol, name, market, exchange, security_type, industry, concepts_json, trade_rule
                FROM securities WHERE symbol = ?1 AND market = ?2
                ",
                params![symbol, market],
                |row| {
                    Ok(Security {
                        id: row.get(0)?,
                        symbol: row.get(1)?,
                        name: row.get(2)?,
                        market: row.get(3)?,
                        exchange: row.get(4)?,
                        security_type: row.get(5)?,
                        industry: row.get(6)?,
                        concepts_json: row.get(7)?,
                        trade_rule: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Finds an already persisted security by exact code without treating a missing row as an
    /// error. A user may follow a code before any provider has populated local metadata.
    pub fn find_security_by_symbol(&self, symbol: &str) -> DatabaseResult<Option<Security>> {
        self.connection
            .query_row(
                "
                SELECT id, symbol, name, market, exchange, security_type, industry, concepts_json, trade_rule
                FROM securities
                WHERE symbol = ?1
                ORDER BY CASE WHEN market IN ('SSE', 'SZSE') THEN 0 ELSE 1 END, id DESC
                LIMIT 1
                ",
                [symbol],
                |row| {
                    Ok(Security {
                        id: row.get(0)?,
                        symbol: row.get(1)?,
                        name: row.get(2)?,
                        market: row.get(3)?,
                        exchange: row.get(4)?,
                        security_type: row.get(5)?,
                        industry: row.get(6)?,
                        concepts_json: row.get(7)?,
                        trade_rule: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Searches only locally persisted security metadata. It never guesses a symbol or company
    /// name when the local catalogue has no match.
    pub fn search_securities(&self, query: &str, limit: usize) -> DatabaseResult<Vec<Security>> {
        let query = query.trim();
        if query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let mut statement = self.connection.prepare(
            "
            SELECT id, symbol, name, market, exchange, security_type, industry, concepts_json, trade_rule
            FROM securities
            WHERE symbol = ?1 OR name LIKE '%' || ?1 || '%'
            ORDER BY
                CASE WHEN symbol = ?1 THEN 0 WHEN name = ?1 THEN 1 ELSE 2 END,
                updated_at DESC,
                id DESC
            LIMIT ?2
            ",
        )?;
        let rows = statement.query_map(params![query, limit as i64], |row| {
            Ok(Security {
                id: row.get(0)?,
                symbol: row.get(1)?,
                name: row.get(2)?,
                market: row.get(3)?,
                exchange: row.get(4)?,
                security_type: row.get(5)?,
                industry: row.get(6)?,
                concepts_json: row.get(7)?,
                trade_rule: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_security_for_portfolio(
        &self,
        id: i64,
        name: &str,
        security_type: &str,
        trade_rule: &str,
    ) -> DatabaseResult<Security> {
        self.connection.execute(
            "
            UPDATE securities
            SET name = ?1, instrument_type = ?2, security_type = ?2, trading_rule = ?3, trade_rule = ?3,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?4
            ",
            params![name, security_type, trade_rule, id],
        )?;
        self.get_security(id)
    }

    pub fn find_holding_by_account_and_security(
        &self,
        cash_account_id: i64,
        security_id: i64,
    ) -> DatabaseResult<Option<Holding>> {
        self.connection
            .query_row(
                "
                SELECT id, cash_account_id, security_id, quantity, available_quantity, average_cost, cost_amount, position_source, as_of_date
                FROM holdings WHERE cash_account_id = ?1 AND security_id = ?2
                ",
                params![cash_account_id, security_id],
                Self::map_holding,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_portfolio_holding_data(&self) -> DatabaseResult<Vec<PortfolioHoldingData>> {
        let mut statement = self.connection.prepare(
            "
            SELECT
                h.id, s.id, s.name, s.symbol, s.market, s.security_type, s.trade_rule,
                h.quantity, h.available_quantity, h.average_cost, h.cost_amount,
                q.current_price, q.previous_close, q.change_pct, q.delay_status,
                (
                    SELECT t.status FROM transactions t
                    WHERE t.cash_account_id = h.cash_account_id AND t.security_id = h.security_id
                    ORDER BY t.trade_date DESC, t.id DESC LIMIT 1
                )
            FROM holdings h
            JOIN securities s ON s.id = h.security_id
            LEFT JOIN market_quotes q ON q.id = (
                SELECT latest_quote.id FROM market_quotes latest_quote
                WHERE latest_quote.security_id = s.id
                ORDER BY latest_quote.market_timestamp DESC, latest_quote.id DESC LIMIT 1
            )
            ORDER BY s.market, s.symbol
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(PortfolioHoldingData {
                holding_id: row.get(0)?,
                security_id: row.get(1)?,
                name: row.get(2)?,
                symbol: row.get(3)?,
                market: row.get(4)?,
                security_type: row.get(5)?,
                trade_rule: row.get(6)?,
                quantity: row.get(7)?,
                available_quantity: row.get(8)?,
                average_cost: row.get(9)?,
                cost_amount: row.get(10)?,
                current_price: row.get(11)?,
                previous_close: row.get(12)?,
                change_percent: row.get(13)?,
                quote_status: row.get(14)?,
                transaction_status: row.get(15)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn upsert_market_source(&self, snapshot: &MarketSnapshot) -> DatabaseResult<i64> {
        let (status, last_success_at, last_error) = if snapshot.delay_status == MarketStatus::NoData
        {
            (
                MarketStatus::NoData.as_str(),
                None,
                snapshot.unavailable_reason.as_deref(),
            )
        } else {
            ("ACTIVE", Some(snapshot.fetched_at.to_rfc3339()), None)
        };
        self.connection.execute(
            "
            INSERT INTO data_sources (
                name, source_type, priority, base_url, enabled, status, last_success_at, last_error
            ) VALUES (?1, 'MARKET', ?2, ?3, 1, ?4, ?5, ?6)
            ON CONFLICT(name) DO UPDATE SET
                source_type = excluded.source_type,
                priority = excluded.priority,
                base_url = excluded.base_url,
                status = excluded.status,
                last_success_at = excluded.last_success_at,
                last_error = excluded.last_error,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                &snapshot.source.name,
                snapshot.source.priority.as_i64(),
                &snapshot.source.base_url,
                status,
                last_success_at,
                last_error,
            ],
        )?;
        self.connection
            .query_row(
                "SELECT id FROM data_sources WHERE name = ?1",
                [&snapshot.source.name],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn get_security(&self, id: i64) -> DatabaseResult<Security> {
        self.connection
            .query_row(
                "
            SELECT id, symbol, name, market, exchange, security_type, industry, concepts_json, trade_rule
            FROM securities WHERE id = ?1
            ",
                [id],
                |row| {
                    Ok(Security {
                        id: row.get(0)?,
                        symbol: row.get(1)?,
                        name: row.get(2)?,
                        market: row.get(3)?,
                        exchange: row.get(4)?,
                        security_type: row.get(5)?,
                        industry: row.get(6)?,
                        concepts_json: row.get(7)?,
                        trade_rule: row.get(8)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    fn get_cash_account(&self, id: i64) -> DatabaseResult<CashAccount> {
        self.connection
            .query_row(
                "
            SELECT id, name, currency, available_to_buy, withdrawable_cash, pending_settlement
            FROM cash_accounts WHERE id = ?1
            ",
                [id],
                |row| {
                    Ok(CashAccount {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        currency: row.get(2)?,
                        available_to_buy: row.get(3)?,
                        withdrawable_cash: row.get(4)?,
                        pending_settlement: row.get(5)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    fn map_daily_review(row: &Row<'_>) -> rusqlite::Result<DailyReview> {
        Ok(DailyReview {
            id: row.get(0)?,
            review_date: row.get(1)?,
            snapshot_id: row.get(2)?,
            portfolio_summary: row.get(3)?,
            market_summary: row.get(4)?,
            holding_summary: row.get(5)?,
            risk_summary: row.get(6)?,
            created_at: row.get(7)?,
        })
    }

    fn map_ai_review(row: &Row<'_>) -> rusqlite::Result<AiReview> {
        Ok(AiReview {
            id: row.get(0)?,
            review_id: row.get(1)?,
            model: row.get(2)?,
            prompt_version: row.get(3)?,
            context_id: row.get(4)?,
            provider: row.get(5)?,
            request_status: row.get(6)?,
            error_code: row.get(7)?,
            facts: row.get(8)?,
            inferences: row.get(9)?,
            risks: row.get(10)?,
            report_json: row.get(11)?,
            security_id: row.get(12)?,
            security_name: row.get(13)?,
            security_symbol: row.get(14)?,
            created_at: row.get(15)?,
        })
    }

    fn map_event(row: &Row<'_>) -> rusqlite::Result<EventRecord> {
        Ok(EventRecord {
            id: row.get(0)?,
            event_type: row.get(1)?,
            title: row.get(2)?,
            security_id: row.get(3)?,
            event_time: row.get(4)?,
            timezone: row.get(5)?,
            source: row.get(6)?,
            source_url: row.get(7)?,
            status: row.get(8)?,
            created_at: row.get(9)?,
        })
    }

    fn list_news_articles_by_scope(
        &self,
        scope: &str,
    ) -> DatabaseResult<Vec<NewsArticleWithSecurity>> {
        let query = format!(
            "
            SELECT n.id, n.title, n.source, n.source_type, n.published_at, n.fetch_time, n.summary,
                   n.url, n.related_security_id, n.created_at, s.name, s.symbol
            FROM news_articles n
            LEFT JOIN securities s ON s.id = n.related_security_id
            {scope}
            ORDER BY n.published_at DESC, n.id DESC
            "
        );
        let mut statement = self.connection.prepare(&query)?;
        let rows = statement.query_map([], |row| {
            Ok(NewsArticleWithSecurity {
                article: Self::map_news_article(row)?,
                related_security_name: row.get(10)?,
                related_security_symbol: row.get(11)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn map_news_article(row: &Row<'_>) -> rusqlite::Result<NewsArticle> {
        Ok(NewsArticle {
            id: row.get(0)?,
            title: row.get(1)?,
            source: row.get(2)?,
            source_type: row.get(3)?,
            published_at: row.get(4)?,
            fetch_time: row.get(5)?,
            summary: row.get(6)?,
            url: row.get(7)?,
            related_security_id: row.get(8)?,
            created_at: row.get(9)?,
        })
    }

    fn get_transaction(&self, id: i64) -> DatabaseResult<Transaction> {
        self.connection.query_row(
            "
            SELECT id, cash_account_id, security_id, side, status, record_source, trade_date, quantity, price,
                   commission, stamp_tax, transfer_fee, other_fee, minimum_commission,
                   external_reference, import_batch_id, note
            FROM transactions WHERE id = ?1
            ",
            [id],
            |row| {
                Ok(Transaction {
                    id: row.get(0)?,
                    cash_account_id: row.get(1)?,
                    security_id: row.get(2)?,
                    side: row.get(3)?,
                    status: row.get(4)?,
                    record_source: row.get(5)?,
                    trade_date: row.get(6)?,
                    quantity: row.get(7)?,
                    price: row.get(8)?,
                    commission: row.get(9)?,
                    stamp_tax: row.get(10)?,
                    transfer_fee: row.get(11)?,
                    other_fee: row.get(12)?,
                    minimum_commission: row.get(13)?,
                    external_reference: row.get(14)?,
                    import_batch_id: row.get(15)?,
                    note: row.get(16)?,
                })
            },
        ).map_err(Into::into)
    }

    fn map_holding(row: &Row<'_>) -> rusqlite::Result<Holding> {
        Ok(Holding {
            id: row.get(0)?,
            cash_account_id: row.get(1)?,
            security_id: row.get(2)?,
            quantity: row.get(3)?,
            available_quantity: row.get(4)?,
            average_cost: row.get(5)?,
            cost_amount: row.get(6)?,
            position_source: row.get(7)?,
            as_of_date: row.get(8)?,
        })
    }
}

impl MarketSnapshotStore for DatabaseService {
    fn save_market_snapshot(&self, snapshot: &MarketSnapshot) -> Result<(), MarketStoreError> {
        self.save_market_snapshot_inner(snapshot)
            .map(|_| ())
            .map_err(|error| MarketStoreError {
                message: error.to_string(),
            })
    }
}

impl DatabaseService {
    pub fn save_market_snapshot_with_id(
        &self,
        snapshot: &MarketSnapshot,
    ) -> DatabaseResult<Option<i64>> {
        self.save_market_snapshot_inner(snapshot)
    }

    fn save_market_snapshot_inner(&self, snapshot: &MarketSnapshot) -> DatabaseResult<Option<i64>> {
        let source_id = self.upsert_market_source(snapshot)?;

        // A NO_DATA result has no provider market timestamp. Persist its safe source status only;
        // inserting a fabricated timestamp into market_snapshots would violate traceability rules.
        let Some(market_timestamp) = snapshot.market_timestamp else {
            return Ok(None);
        };

        self.connection.execute(
            "
            INSERT INTO market_snapshots (
                data_source_id, snapshot_kind, market_timestamp, fetched_at, delay_status
            ) VALUES (?1, 'FULL', ?2, ?3, ?4)
            ",
            params![
                source_id,
                market_timestamp.to_rfc3339(),
                snapshot.fetched_at.to_rfc3339(),
                snapshot.delay_status.as_str(),
            ],
        )?;
        let snapshot_id = self.connection.last_insert_rowid();

        for quote in &snapshot.quotes {
            self.connection.execute(
                "
                INSERT INTO market_quotes (
                    market_snapshot_id, security_id, data_source_id, current_price, change_pct,
                    market_timestamp, fetched_at, delay_status, symbol, security_name, market,
                    previous_close, price_change, volume, volume_unit, turnover_amount, turnover_unit, source
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
                )
                ON CONFLICT(security_id, data_source_id, market_timestamp) DO UPDATE SET
                    market_snapshot_id = excluded.market_snapshot_id,
                    current_price = excluded.current_price,
                    change_pct = excluded.change_pct,
                    fetched_at = excluded.fetched_at,
                    delay_status = excluded.delay_status,
                    symbol = excluded.symbol,
                    security_name = excluded.security_name,
                    market = excluded.market,
                    previous_close = excluded.previous_close,
                    price_change = excluded.price_change,
                    volume = excluded.volume,
                    volume_unit = excluded.volume_unit,
                    turnover_amount = excluded.turnover_amount,
                    turnover_unit = excluded.turnover_unit,
                    source = excluded.source
                ",
                params![
                    snapshot_id,
                    quote.security_id,
                    source_id,
                    quote.current_price.to_string(),
                    quote.change_percent.to_string(),
                    quote.market_timestamp.to_rfc3339(),
                    quote.fetched_at.to_rfc3339(),
                    quote.delay_status.as_str(),
                    &quote.symbol,
                    &quote.name,
                    &quote.market,
                    quote.previous_close.to_string(),
                    quote.price_change.to_string(),
                    quote.volume.to_string(),
                    &quote.volume_unit,
                    quote.turnover_amount.to_string(),
                    &quote.turnover_unit,
                    &quote.source,
                ],
            )?;
        }
        Ok(Some(snapshot_id))
    }

    pub fn save_market_index_snapshot(&self, snapshot: &MarketSnapshot) -> DatabaseResult<()> {
        self.save_market_index_snapshot_with_id(snapshot)
            .map(|_| ())
    }

    pub fn save_market_index_snapshot_with_id(
        &self,
        snapshot: &MarketSnapshot,
    ) -> DatabaseResult<Option<i64>> {
        let source_id = self.upsert_market_source(snapshot)?;
        let Some(market_timestamp) = snapshot.market_timestamp else {
            return Ok(None);
        };
        self.connection.execute(
            "
            INSERT INTO market_snapshots (
                data_source_id, snapshot_kind, market_timestamp, fetched_at, delay_status
            ) VALUES (?1, 'INDICES', ?2, ?3, ?4)
            ",
            params![
                source_id,
                market_timestamp.to_rfc3339(),
                snapshot.fetched_at.to_rfc3339(),
                snapshot.delay_status.as_str(),
            ],
        )?;
        let snapshot_id = self.connection.last_insert_rowid();
        for quote in &snapshot.quotes {
            self.connection.execute(
                "
                INSERT INTO market_index_quotes (
                    market_snapshot_id, name, symbol, current_price, change_pct, change_percent,
                    market_timestamp, fetched_at, delay_status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(market_snapshot_id, symbol) DO UPDATE SET
                    name = excluded.name,
                    current_price = excluded.current_price,
                    change_pct = excluded.change_pct,
                    change_percent = excluded.change_percent,
                    market_timestamp = excluded.market_timestamp,
                    fetched_at = excluded.fetched_at,
                    delay_status = excluded.delay_status
                ",
                params![
                    snapshot_id,
                    &quote.name,
                    &quote.symbol,
                    quote.current_price.to_string(),
                    quote.change_percent.to_string(),
                    quote.change_percent.to_string(),
                    quote.market_timestamp.to_rfc3339(),
                    quote.fetched_at.to_rfc3339(),
                    quote.delay_status.as_str(),
                ],
            )?;
        }
        Ok(Some(snapshot_id))
    }

    pub fn list_market_quotes_for_snapshot(
        &self,
        snapshot_id: Option<i64>,
    ) -> DatabaseResult<Vec<StoredMarketQuote>> {
        let Some(snapshot_id) = snapshot_id else {
            return Ok(Vec::new());
        };
        let mut statement = self.connection.prepare(
            "SELECT symbol, security_name, market, current_price, previous_close, price_change,
                    change_pct, volume, turnover_amount, market_timestamp, fetched_at, source, delay_status
             FROM market_quotes WHERE market_snapshot_id = ?1 ORDER BY symbol"
        )?;
        let records = statement
            .query_map([snapshot_id], |row| {
                Ok(StoredMarketQuote {
                    symbol: row.get(0)?,
                    name: row.get(1)?,
                    market: row.get(2)?,
                    current_price: row.get(3)?,
                    previous_close: row.get(4)?,
                    price_change: row.get(5)?,
                    change_percent: row.get(6)?,
                    volume: row.get(7)?,
                    turnover_amount: row.get(8)?,
                    market_timestamp: row.get(9)?,
                    fetched_at: row.get(10)?,
                    source: row.get(11)?,
                    delay_status: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into);
        records
    }

    pub fn list_market_index_quotes_for_snapshot(
        &self,
        snapshot_id: Option<i64>,
    ) -> DatabaseResult<Vec<StoredMarketQuote>> {
        let Some(snapshot_id) = snapshot_id else {
            return Ok(Vec::new());
        };
        let mut statement = self.connection.prepare(
            "SELECT miq.symbol, miq.name,
                    CASE WHEN miq.symbol LIKE '%.SH' THEN 'SSE' ELSE 'SZSE' END,
                    miq.current_price, '0', '0',
                    COALESCE(miq.change_percent, miq.change_pct, '0'), '0', '0',
                    miq.market_timestamp, miq.fetched_at, ds.name, miq.delay_status
             FROM market_index_quotes miq
             JOIN market_snapshots ms ON ms.id = miq.market_snapshot_id
             JOIN data_sources ds ON ds.id = ms.data_source_id
             WHERE miq.market_snapshot_id = ?1
             ORDER BY miq.symbol",
        )?;
        let records = statement
            .query_map([snapshot_id], |row| {
                Ok(StoredMarketQuote {
                    symbol: row.get(0)?,
                    name: row.get(1)?,
                    market: row.get(2)?,
                    current_price: row.get(3)?,
                    previous_close: row.get(4)?,
                    price_change: row.get(5)?,
                    change_percent: row.get(6)?,
                    volume: row.get(7)?,
                    turnover_amount: row.get(8)?,
                    market_timestamp: row.get(9)?,
                    fetched_at: row.get(10)?,
                    source: row.get(11)?,
                    delay_status: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into);
        records
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_service::{
        DataSourcePriority, MarketDataSource, MarketQuote, MarketSnapshot, MarketStatus,
        SourceClass,
    };
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;

    fn decimal(value: &str) -> Decimal {
        value.parse().expect("valid decimal fixture")
    }

    fn new_security() -> NewSecurity {
        NewSecurity {
            symbol: "600519".into(),
            name: "贵州茅台".into(),
            market: "SSE".into(),
            exchange: "SSE".into(),
            security_type: "STOCK".into(),
            industry: Some("白酒".into()),
            concepts_json: "[\"消费\"]".into(),
            trade_rule: "T_PLUS_1".into(),
        }
    }

    fn new_cash_account() -> NewCashAccount {
        NewCashAccount {
            name: "主账户".into(),
            currency: "CNY".into(),
            available_to_buy: "100000.00".into(),
            withdrawable_cash: "100000.00".into(),
            pending_settlement: "0".into(),
        }
    }

    #[test]
    fn migrations_are_versioned_and_idempotent() {
        let mut connection = Connection::open_in_memory().expect("create in-memory database");
        migrations::apply(&mut connection).expect("first migration run");
        migrations::apply(&mut connection).expect("second migration run");

        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("read migration count");
        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                    'securities', 'holdings', 'transactions', 'cash_accounts',
                    'market_snapshots', 'market_quotes', 'data_sources', 'news_articles',
                    'daily_reviews', 'ai_reviews', 'events', 'app_settings', 'market_index_quotes',
                    'manual_refresh_runs', 'ai_review_contexts', 'manual_refresh_news_articles',
                    'manual_refresh_events', 'watchlist_items', 'ai_provider_settings',
                    'news_security_links', 'event_security_links'
                )",
                [],
                |row| row.get(0),
            )
            .expect("verify core tables");

        assert_eq!(migration_count, 18);
        assert_eq!(table_count, 21);
    }

    #[test]
    fn core_crud_persists_only_user_entered_records() {
        let database = DatabaseService::open_in_memory().expect("initialize database");

        let security = database
            .create_security(new_security())
            .expect("create security");
        assert_eq!(security.symbol, "600519");

        let cash_account = database
            .create_cash_account(new_cash_account())
            .expect("create cash account");

        let holding = database
            .create_holding(NewHolding {
                cash_account_id: cash_account.id,
                security_id: security.id,
                quantity: 100,
                available_quantity: 100,
                average_cost: "1500.00".into(),
                cost_amount: "150000.00".into(),
                position_source: "INITIAL_POSITION".into(),
                as_of_date: Some("2026-08-08".into()),
            })
            .expect("create holding");

        let transaction = database
            .create_transaction(NewTransaction {
                cash_account_id: cash_account.id,
                security_id: security.id,
                side: "BUY".into(),
                status: "CONFIRMED".into(),
                record_source: "MANUAL".into(),
                trade_date: "2026-08-08".into(),
                quantity: 100,
                price: "1500.00".into(),
                commission: "5.00".into(),
                stamp_tax: "0".into(),
                transfer_fee: "0".into(),
                other_fee: "0".into(),
                minimum_commission: "5.00".into(),
                external_reference: None,
                import_batch_id: None,
                note: Some("CRUD test record".into()),
            })
            .expect("create transaction");
        assert_eq!(transaction.quantity, 100);

        let fetched = database.get_holding(holding.id).expect("query holding");
        assert_eq!(fetched.average_cost, "1500.00");

        let updated = database
            .update_holding(
                holding.id,
                HoldingUpdate {
                    quantity: 200,
                    available_quantity: 100,
                    average_cost: "1550.00".into(),
                    cost_amount: "310000.00".into(),
                    as_of_date: Some("2026-08-09".into()),
                },
            )
            .expect("update holding");
        assert_eq!(updated.quantity, 200);
        assert_eq!(updated.average_cost, "1550.00");
        assert_eq!(updated.cost_amount, "310000.00");

        let cancelled = database
            .cancel_transaction(transaction.id)
            .expect("cancel transaction");
        assert_eq!(cancelled.status, "CANCELLED");
        assert_eq!(
            database.delete_holding(holding.id).expect("delete holding"),
            1
        );
    }

    #[test]
    fn removing_followed_security_deletes_owned_records_and_preserves_shared_news() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let security_a = database
            .create_security(NewSecurity {
                symbol: "300209".into(),
                name: "待删除标的".into(),
                market: "SZSE".into(),
                exchange: "SZSE".into(),
                security_type: "STOCK".into(),
                industry: None,
                concepts_json: "[]".into(),
                trade_rule: "T_PLUS_1".into(),
            })
            .expect("create security a");
        let security_b = database
            .create_security(NewSecurity {
                symbol: "600330".into(),
                name: "保留标的".into(),
                market: "SSE".into(),
                exchange: "SSE".into(),
                security_type: "STOCK".into(),
                industry: None,
                concepts_json: "[]".into(),
                trade_rule: "T_PLUS_1".into(),
            })
            .expect("create security b");
        let follow_a = database
            .create_watchlist_item(security_a.id)
            .expect("follow a");
        database
            .create_watchlist_item(security_b.id)
            .expect("follow b");
        let account = database
            .create_cash_account(NewCashAccount {
                name: "删除测试账户".into(),
                currency: "CNY".into(),
                available_to_buy: "0".into(),
                withdrawable_cash: "0".into(),
                pending_settlement: "0".into(),
            })
            .expect("create account");
        database
            .create_holding(NewHolding {
                cash_account_id: account.id,
                security_id: security_a.id,
                quantity: 100,
                available_quantity: 100,
                average_cost: "10".into(),
                cost_amount: "1000".into(),
                position_source: "MANUAL".into(),
                as_of_date: None,
            })
            .expect("create owned holding");
        database
            .connection
            .execute(
                "INSERT INTO data_sources (name, source_type, priority) VALUES ('删除测试行情源', 'MARKET', 2)",
                [],
            )
            .expect("create market source");
        let source_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO market_snapshots (data_source_id, market_timestamp, fetched_at, delay_status)
                 VALUES (?1, '2026-08-10T08:00:00Z', '2026-08-10T08:01:00Z', 'DELAYED')",
                [source_id],
            )
            .expect("create global market snapshot");
        let snapshot_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO market_quotes (market_snapshot_id, security_id, data_source_id, current_price, market_timestamp, fetched_at, delay_status)
                 VALUES (?1, ?2, ?3, '10', '2026-08-10T08:00:00Z', '2026-08-10T08:01:00Z', 'DELAYED')",
                params![snapshot_id, security_a.id, source_id],
            )
            .expect("create owned quote");
        database
            .create_transaction(NewTransaction {
                cash_account_id: account.id,
                security_id: security_a.id,
                side: "BUY".into(),
                status: "CONFIRMED".into(),
                record_source: "MANUAL".into(),
                trade_date: "2026-08-10".into(),
                quantity: 100,
                price: "10".into(),
                commission: "0".into(),
                stamp_tax: "0".into(),
                transfer_fee: "0".into(),
                other_fee: "0".into(),
                minimum_commission: "0".into(),
                external_reference: None,
                import_batch_id: None,
                note: None,
            })
            .expect("create owned transaction");
        let shared_news = database
            .create_news_article(NewNewsArticle {
                title: "共享公告".into(),
                source: "测试来源".into(),
                source_type: "OFFICIAL".into(),
                published_at: "2026-08-10T08:00:00Z".into(),
                fetch_time: "2026-08-10T08:01:00Z".into(),
                summary: "共享资讯".into(),
                url: "https://example.invalid/shared-news".into(),
                related_security_id: Some(security_a.id),
            })
            .expect("create shared news");
        database
            .link_news_article_to_security(shared_news.id, security_b.id)
            .expect("link shared news b");
        let owned_news = database
            .create_news_article(NewNewsArticle {
                title: "独有公告".into(),
                source: "测试来源".into(),
                source_type: "OFFICIAL".into(),
                published_at: "2026-08-10T08:00:00Z".into(),
                fetch_time: "2026-08-10T08:01:00Z".into(),
                summary: "独有资讯".into(),
                url: "https://example.invalid/owned-news".into(),
                related_security_id: Some(security_a.id),
            })
            .expect("create owned news");
        let shared_event = database
            .create_event(NewEventRecord {
                event_type: "EARNINGS".into(),
                title: "共享事件".into(),
                security_id: Some(security_a.id),
                event_time: "2026-08-10T09:00:00+08:00".into(),
                timezone: "Asia/Shanghai".into(),
                source: "测试来源".into(),
                source_url: Some("https://example.invalid/shared-event".into()),
                status: "CONFIRMED".into(),
            })
            .expect("create shared event");
        database
            .link_event_to_security(shared_event.id, security_b.id)
            .expect("link shared event b");
        let owned_event = database
            .create_event(NewEventRecord {
                event_type: "MAJOR_MATTER".into(),
                title: "独有事件".into(),
                security_id: Some(security_a.id),
                event_time: "2026-08-10T10:00:00+08:00".into(),
                timezone: "Asia/Shanghai".into(),
                source: "测试来源".into(),
                source_url: Some("https://example.invalid/owned-event".into()),
                status: "CONFIRMED".into(),
            })
            .expect("create owned event");
        let run = database
            .create_manual_refresh_run(NewManualRefreshRun {
                started_at: "2026-08-10T08:00:00Z".into(),
                completed_at: "2026-08-10T08:01:00Z".into(),
                holdings_snapshot_id: None,
                indices_snapshot_id: None,
                portfolio_json: "[]".into(),
                status: "NO_DATA".into(),
            })
            .expect("create run");
        database
            .link_news_articles_to_manual_refresh_run(run.id, &[shared_news.id, owned_news.id])
            .expect("link news refresh");
        database
            .link_events_to_manual_refresh_run(run.id, &[shared_event.id, owned_event.id])
            .expect("link events refresh");
        let review = database
            .upsert_daily_review(NewDailyReview {
                review_date: "2026-08-10".into(),
                snapshot_id: None,
                portfolio_summary: "{}".into(),
                market_summary: "{}".into(),
                holding_summary: "{}".into(),
                risk_summary: "{}".into(),
            })
            .expect("create daily review");
        let context_id = database
            .create_ai_review_context(NewAiReviewContext {
                review_id: review.id,
                manual_refresh_run_id: run.id,
                portfolio_json: "[]".into(),
                market_json: "[]".into(),
                news_json: "[]".into(),
                events_json: "[]".into(),
            })
            .expect("create ai context");
        database
            .create_ai_review(NewAiReview {
                review_id: review.id,
                model: "test".into(),
                prompt_version: "test".into(),
                context_id,
                provider: "TEST".into(),
                facts: "[]".into(),
                inferences: "[]".into(),
                risks: "[]".into(),
                report_json: None,
                security_id: Some(security_a.id),
            })
            .expect("create owned ai review");

        database
            .remove_followed_security_completely(follow_a.watchlist_item_id, security_a.id)
            .expect("remove followed security");

        assert!(database
            .find_security_by_symbol("300209")
            .expect("find removed security")
            .is_none());
        let owned_counts: (i64, i64, i64, i64, i64, i64, i64, i64) = database
            .connection
            .query_row(
                "SELECT
                (SELECT COUNT(*) FROM holdings WHERE security_id = ?1),
                (SELECT COUNT(*) FROM transactions WHERE security_id = ?1),
                (SELECT COUNT(*) FROM market_quotes WHERE security_id = ?1),
                (SELECT COUNT(*) FROM market_snapshots WHERE id = ?2),
                (SELECT COUNT(*) FROM news_articles WHERE id = ?3),
                (SELECT COUNT(*) FROM events WHERE id = ?4),
                (SELECT COUNT(*) FROM ai_reviews WHERE security_id = ?1),
                (SELECT COUNT(*) FROM ai_review_contexts WHERE id = ?5)",
                params![
                    security_a.id,
                    snapshot_id,
                    owned_news.id,
                    owned_event.id,
                    context_id
                ],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .expect("verify owned records removed");
        assert_eq!(owned_counts, (0, 0, 0, 1, 0, 0, 0, 0));
        let shared_state: (i64, i64, i64, i64) = database
            .connection
            .query_row(
                "SELECT
                (SELECT COUNT(*) FROM news_articles WHERE id = ?1),
                (SELECT related_security_id FROM news_articles WHERE id = ?1),
                (SELECT COUNT(*) FROM events WHERE id = ?2),
                (SELECT security_id FROM events WHERE id = ?2)",
                params![shared_news.id, shared_event.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("verify shared records remain");
        assert_eq!(shared_state, (1, security_b.id, 1, security_b.id));
    }

    #[test]
    fn failed_followed_security_removal_rolls_back_every_change() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let security = database
            .create_security(new_security())
            .expect("create security");
        let follow = database
            .create_watchlist_item(security.id)
            .expect("create follow");
        assert!(database
            .remove_followed_security_with_forced_failure(follow.watchlist_item_id, security.id)
            .is_err());
        assert!(database
            .find_watchlist_item_by_security(security.id)
            .expect("follow rolled back")
            .is_some());
        assert!(database.get_security(security.id).is_ok());
    }

    #[test]
    fn migration_002_upgrades_001_without_losing_existing_records() {
        let mut connection = Connection::open_in_memory().expect("create legacy database");
        migrations::apply_001_for_upgrade_test(&mut connection).expect("apply 001 only");

        connection
            .execute(
                "
                INSERT INTO securities (symbol, name, market, instrument_type, concepts_json, trading_rule)
                VALUES ('510300', '沪深300ETF', 'SSE', 'ETF', '[]', 'T_PLUS_ZERO')
                ",
                [],
            )
            .expect("insert legacy security");
        let security_id = connection.last_insert_rowid();
        connection
            .execute("INSERT INTO cash_accounts (name) VALUES ('旧账户')", [])
            .expect("insert legacy account");
        let cash_account_id = connection.last_insert_rowid();
        connection
            .execute(
                "
                INSERT INTO holdings (cash_account_id, security_id, quantity, available_quantity, average_cost)
                VALUES (?1, ?2, 100, 100, '3.85')
                ",
                [cash_account_id, security_id],
            )
            .expect("insert legacy holding");
        connection
            .execute(
                "
                INSERT INTO transactions (
                    cash_account_id, security_id, side, trade_date, quantity, price, commission, stamp_duty
                ) VALUES (?1, ?2, 'BUY', '2026-08-08', 100, '3.85', '5.00', '0.10')
                ",
                [cash_account_id, security_id],
            )
            .expect("insert legacy transaction");

        migrations::apply(&mut connection).expect("upgrade database through latest migration");

        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("read migration count");
        let upgraded_security: (String, String, String) = connection
            .query_row(
                "SELECT exchange, security_type, trade_rule FROM securities WHERE id = ?1",
                [security_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read upgraded security");
        let upgraded_holding: (i64, String, String) = connection
            .query_row(
                "SELECT quantity, average_cost, cost_amount FROM holdings WHERE security_id = ?1",
                [security_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read upgraded holding");
        let upgraded_transaction: (String, String, String, String) = connection
            .query_row(
                "SELECT side, status, stamp_tax, minimum_commission FROM transactions WHERE security_id = ?1",
                [security_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read upgraded transaction");
        let corporate_actions_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'corporate_actions'",
                [],
                |row| row.get(0),
            )
            .expect("verify corporate action table");
        let news_articles_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'news_articles'",
                [],
                |row| row.get(0),
            )
            .expect("verify news table");
        let daily_reviews_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'daily_reviews'",
                [],
                |row| row.get(0),
            )
            .expect("verify daily review table");
        let ai_reviews_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ai_reviews'",
                [],
                |row| row.get(0),
            )
            .expect("verify AI review table");
        let events_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'events'",
                [],
                |row| row.get(0),
            )
            .expect("verify event table");
        let app_settings_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'app_settings'",
                [],
                |row| row.get(0),
            )
            .expect("verify app settings table");
        let market_index_quotes_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'market_index_quotes'",
                [],
                |row| row.get(0),
            )
            .expect("verify market index quote table");
        let manual_refresh_runs_exists: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'manual_refresh_runs'",
            [], |row| row.get(0),
        ).expect("verify refresh run table");
        let ai_contexts_exists: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ai_review_contexts'",
            [], |row| row.get(0),
        ).expect("verify AI context table");
        let ai_review_provider_column: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('ai_reviews') WHERE name = 'provider'",
                [],
                |row| row.get(0),
            )
            .expect("verify AI provider column");
        let watchlist_item_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM watchlist_items WHERE security_id = ?1",
                [security_id],
                |row| row.get(0),
            )
            .expect("verify legacy holding migrated to follow");
        let ai_review_report_column: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('ai_reviews') WHERE name = 'report_json'",
                [],
                |row| row.get(0),
            )
            .expect("verify AI report column");
        let ai_review_security_column: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('ai_reviews') WHERE name = 'security_id'",
                [],
                |row| row.get(0),
            )
            .expect("verify per-security AI review column");
        let ai_provider_setting_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_provider_settings", [], |row| {
                row.get(0)
            })
            .expect("verify provider settings migration");

        assert_eq!(migration_count, 18);
        assert_eq!(
            upgraded_security,
            ("SSE".into(), "ETF".into(), "T_PLUS_0".into())
        );
        assert_eq!(upgraded_holding, (100, "3.85".into(), "0".into()));
        assert_eq!(
            upgraded_transaction,
            ("BUY".into(), "CONFIRMED".into(), "0.10".into(), "0".into())
        );
        assert_eq!(corporate_actions_exists, 1);
        assert_eq!(news_articles_exists, 1);
        assert_eq!(daily_reviews_exists, 1);
        assert_eq!(ai_reviews_exists, 1);
        assert_eq!(events_exists, 1);
        assert_eq!(app_settings_exists, 1);
        assert_eq!(manual_refresh_runs_exists, 1);
        assert_eq!(ai_contexts_exists, 1);
        assert_eq!(ai_review_provider_column, 1);
        assert_eq!(ai_review_report_column, 1);
        assert_eq!(ai_review_security_column, 1);
        assert_eq!(ai_provider_setting_count, 3);
        assert_eq!(watchlist_item_count, 1);
        assert_eq!(market_index_quotes_exists, 1);
    }

    #[test]
    fn migration_011_backfills_index_change_percent_without_losing_quotes() {
        let mut connection = Connection::open_in_memory().expect("create pre-011 database");
        migrations::apply_010_for_upgrade_test(&mut connection).expect("apply through 010");
        connection
            .execute(
                "INSERT INTO data_sources (name, source_type, priority, base_url)
             VALUES ('Legacy index source', 'MARKET', 2, 'https://test.invalid')",
                [],
            )
            .expect("insert source");
        let source_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO market_snapshots (
                data_source_id, snapshot_kind, market_timestamp, fetched_at, delay_status
             ) VALUES (?1, 'INDICES', '2026-08-08T08:00:00Z', '2026-08-08T08:01:00Z', 'DELAYED')",
                [source_id],
            )
            .expect("insert snapshot");
        let snapshot_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO market_index_quotes (
                market_snapshot_id, name, symbol, current_price, change_pct,
                market_timestamp, fetched_at, delay_status
             ) VALUES (?1, '上证指数', '000001.SH', '3500.00', '1.25',
                '2026-08-08T08:00:00Z', '2026-08-08T08:01:00Z', 'DELAYED')",
                [snapshot_id],
            )
            .expect("insert legacy index quote");

        migrations::apply(&mut connection).expect("upgrade through 011");
        migrations::apply(&mut connection).expect("reapply is idempotent");

        let quote: (String, String, String) = connection.query_row(
            "SELECT symbol, change_pct, change_percent FROM market_index_quotes WHERE market_snapshot_id = ?1",
            [snapshot_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).expect("query upgraded index quote");
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("read migration count");
        assert_eq!(quote, ("000001.SH".into(), "1.25".into(), "1.25".into()));
        assert_eq!(migration_count, 18);

        let database = DatabaseService { connection };
        let records = database
            .list_market_index_quotes_for_snapshot(Some(snapshot_id))
            .expect("query upgraded index quote through database service");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].symbol, "000001.SH");
        assert_eq!(records[0].change_percent, "1.25");
        assert_eq!(records[0].source, "Legacy index source");
        assert_eq!(records[0].delay_status, "DELAYED");
    }

    #[test]
    fn migration_012_preserves_existing_events_and_adds_refresh_links() {
        let mut connection = Connection::open_in_memory().expect("create pre-012 database");
        migrations::apply_011_for_upgrade_test(&mut connection).expect("apply through 011");
        connection.execute(
            "INSERT INTO events (event_type, title, event_time, timezone, source, source_url, status)
             VALUES ('EARNINGS', '既有财报公告', '2026-08-08T09:00:00+08:00', 'Asia/Shanghai', '旧来源', 'https://example.invalid/old-event', 'CONFIRMED')",
            [],
        ).expect("insert existing event");
        let event_id = connection.last_insert_rowid();

        migrations::apply(&mut connection).expect("upgrade through 012");
        migrations::apply(&mut connection).expect("reapply through 012");
        let event: (String, String, String) = connection
            .query_row(
                "SELECT event_type, title, source_url FROM events WHERE id = ?1",
                [event_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read preserved event");
        let refresh_link_tables: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('manual_refresh_news_articles', 'manual_refresh_events')",
            [], |row| row.get(0),
        ).expect("read refresh link tables");
        assert_eq!(
            event,
            (
                "EARNINGS".into(),
                "既有财报公告".into(),
                "https://example.invalid/old-event".into()
            )
        );
        assert_eq!(refresh_link_tables, 2);
    }

    #[test]
    fn migration_017_backfills_shareable_security_links_without_losing_legacy_rows() {
        let mut connection = Connection::open_in_memory().expect("create pre-017 database");
        migrations::apply_016_for_upgrade_test(&mut connection)
            .expect("apply migrations through 016");
        connection.execute(
            "INSERT INTO securities (symbol, name, market, instrument_type, concepts_json, trading_rule, exchange, security_type, trade_rule)
             VALUES ('600519', '测试证券', 'SSE', 'STOCK', '[]', 'T_PLUS_1', 'SSE', 'STOCK', 'T_PLUS_1')",
            [],
        ).expect("insert legacy security");
        let security_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO news_articles (title, source, source_type, published_at, fetch_time, summary, url, related_security_id)
             VALUES ('既有资讯', '旧来源', 'OFFICIAL', '2026-08-10T08:00:00Z', '2026-08-10T08:01:00Z', '测试摘要', 'https://example.invalid/legacy-news', ?1)",
            [security_id],
        ).expect("insert legacy news");
        let news_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO events (event_type, title, security_id, event_time, timezone, source, status)
             VALUES ('EARNINGS', '既有事件', ?1, '2026-08-10T08:00:00+08:00', 'Asia/Shanghai', '旧来源', 'CONFIRMED')",
            [security_id],
        ).expect("insert legacy event");
        let event_id = connection.last_insert_rowid();

        migrations::apply(&mut connection).expect("upgrade through 017");
        migrations::apply(&mut connection).expect("reapply is idempotent");

        let links: (i64, i64) = connection.query_row(
            "SELECT
                (SELECT COUNT(*) FROM news_security_links WHERE news_article_id = ?1 AND security_id = ?3),
                (SELECT COUNT(*) FROM event_security_links WHERE event_id = ?2 AND security_id = ?3)",
            params![news_id, event_id, security_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).expect("read backfilled links");
        assert_eq!(links, (1, 1));
    }

    #[test]
    fn migration_018_migrates_tencent_preference_without_losing_other_provider_settings() {
        let mut connection = Connection::open_in_memory().expect("create pre-018 database");
        migrations::apply_017_for_upgrade_test(&mut connection)
            .expect("apply migrations through 017");
        connection
            .execute(
                "UPDATE ai_provider_settings SET enabled = 1 WHERE provider = 'TENCENT_HUNYUAN'",
                [],
            )
            .expect("enable legacy Tencent preference");

        migrations::apply(&mut connection).expect("upgrade through 018");
        migrations::apply(&mut connection).expect("reapply 018 idempotently");

        let providers: Vec<(String, String, i64, i64)> = {
            let mut statement = connection
                .prepare(
                    "SELECT provider, model, enabled, priority FROM ai_provider_settings ORDER BY priority",
                )
                .expect("query migrated providers");
            statement
                .query_map([], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })
                .expect("read providers")
                .collect::<Result<_, _>>()
                .expect("collect providers")
        };
        assert_eq!(
            providers,
            vec![
                ("DEEPSEEK".into(), "deepseek-chat".into(), 1, 1),
                (
                    "TENCENT_TOKENHUB".into(),
                    "hunyuan-turbos-latest".into(),
                    1,
                    2,
                ),
                ("DOUBAO".into(), "doubao-seed-1-6-250615".into(), 0, 3),
            ]
        );
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("read migration count");
        assert_eq!(migration_count, 18);
    }

    #[test]
    fn market_snapshot_store_persists_normalized_quotes_and_no_data_source_state() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let security = database
            .create_security(new_security())
            .expect("create security for quote");
        let fetched_at = Utc.with_ymd_and_hms(2026, 8, 10, 2, 0, 0).unwrap();
        let market_timestamp = Utc.with_ymd_and_hms(2026, 8, 10, 1, 59, 0).unwrap();
        let source = MarketDataSource {
            name: "Recorded test market source".into(),
            base_url: "https://test.invalid/quotes".into(),
            priority: DataSourcePriority::PublicQuote,
            source_class: SourceClass::PublicQuote,
        };
        let snapshot = MarketSnapshot {
            source: source.clone(),
            market_timestamp: Some(market_timestamp),
            fetched_at,
            delay_status: MarketStatus::Delayed,
            quotes: vec![MarketQuote {
                security_id: security.id,
                symbol: security.symbol.clone(),
                name: security.name.clone(),
                market: security.market.clone(),
                current_price: decimal("1500.00"),
                previous_close: decimal("1490.00"),
                price_change: decimal("10.00"),
                change_percent: decimal("0.6711"),
                volume: decimal("100"),
                volume_unit: "LOTS".into(),
                turnover_amount: decimal("150000"),
                turnover_unit: "CNY".into(),
                market_timestamp,
                fetched_at,
                source: source.name.clone(),
                delay_status: MarketStatus::Delayed,
            }],
            unavailable_reason: None,
        };

        database
            .save_market_snapshot(&snapshot)
            .expect("persist traceable quote snapshot");
        let (symbol, previous_close, price_change, volume, source_name, delay_status):
            (String, String, String, String, String, String) = database
            .connection
            .query_row(
                "SELECT symbol, previous_close, price_change, volume, source, delay_status FROM market_quotes",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .expect("read persisted quote");
        assert_eq!(symbol, "600519");
        assert_eq!(previous_close, "1490.00");
        assert_eq!(price_change, "10.00");
        assert_eq!(volume, "100");
        assert_eq!(source_name, "Recorded test market source");
        assert_eq!(delay_status, "DELAYED");

        let no_data = MarketSnapshot {
            source,
            market_timestamp: None,
            fetched_at,
            delay_status: MarketStatus::NoData,
            quotes: Vec::new(),
            unavailable_reason: Some("recorded source failure".into()),
        };
        database
            .save_market_snapshot(&no_data)
            .expect("persist no-data source state without creating a fabricated timestamp");
        let (source_status, last_error): (String, Option<String>) = database
            .connection
            .query_row(
                "SELECT status, last_error FROM data_sources WHERE name = 'Recorded test market source'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read no-data source state");
        assert_eq!(source_status, "NO_DATA");
        assert_eq!(last_error.as_deref(), Some("recorded source failure"));
    }
}
