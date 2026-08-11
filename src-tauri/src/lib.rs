//! Desktop application bootstrap.

mod ai_service;
mod dashboard_service;
mod database;
mod disclosure_adapter;
#[allow(dead_code)]
// Phase 6-D defines the future Adapter ingestion/CRUD API; calendar UI is read-only.
mod event_service;
mod initialization_service;
mod market_refresh_service;
pub mod market_service;
#[allow(dead_code)] // Phase 6-A defines the future Adapter ingestion/CRUD API; UI is read-only.
mod news_service;
pub mod portfolio_service;
mod portfolio_ui_service;
mod research_service;
mod review_service;
mod secure_storage;
mod settings_service;

use ai_service::{
    AiProviderConfigView, AiProviderConnectionTestView, AiReviewView, AiService,
    AiServiceStatusView,
};
use dashboard_service::{
    AssetSummaryView, DashboardDataStatusView, DashboardService, MarketIndexQuoteView,
};
use event_service::{EventService, EventView};
use initialization_service::{InitializationService, InitializationStatusView};
use market_refresh_service::{ManualMarketSnapshotView, MarketRefreshService, MarketRefreshView};
use news_service::{HoldingNewsView, NewsService};
use portfolio_ui_service::{
    CreateHoldingInput, CreateWatchlistInput, DeleteWatchlistInput, PortfolioHoldingView,
    PortfolioUiService, SecurityLookupView, UpdateHoldingInput,
};
use review_service::{DailyReviewView, ReviewService};
use secure_storage::{
    get_ai_provider_key_status as load_ai_provider_key_status,
    get_deepseek_status as load_deepseek_status, get_tushare_status as load_tushare_status,
    remove_ai_provider_api_key as delete_ai_provider_api_key,
    remove_deepseek_api_key as delete_deepseek_api_key,
    remove_tushare_token as delete_tushare_token,
    save_ai_provider_api_key as store_ai_provider_api_key,
    save_deepseek_api_key as store_deepseek_api_key, save_tushare_token as store_tushare_token,
    DeepSeekStatusView, TushareStatusView,
};
use settings_service::{
    BackupView, CashAccountView, CreateCashAccountInput, SettingsService, SettingsStatusView,
};

#[tauri::command]
fn get_asset_summary(app: tauri::AppHandle) -> Result<AssetSummaryView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    DashboardService::load_asset_summary(&database).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_market_snapshot(app: tauri::AppHandle) -> Result<Vec<MarketIndexQuoteView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    Ok(DashboardService::load_market_snapshot(&database))
}

#[tauri::command]
fn get_dashboard_data_status(app: tauri::AppHandle) -> Result<DashboardDataStatusView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    DashboardService::load_data_status(&database).map_err(|error| error.to_string())
}

/// Collects one user-requested market snapshot. This command is never scheduled or kept alive;
/// public-provider data remains labelled delayed, and missing news/event adapters return NO_DATA.
#[tauri::command]
fn refresh_today_market_snapshot(
    app: tauri::AppHandle,
) -> Result<ManualMarketSnapshotView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    MarketRefreshService::refresh_today_snapshot(&database).map_err(|error| error.to_string())
}

/// Legacy explicit Tushare-only refresh. The Dashboard uses `refresh_today_market_snapshot`.
#[tauri::command]
fn refresh_tushare_market_data(app: tauri::AppHandle) -> Result<MarketRefreshView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    MarketRefreshService::refresh_tushare(&database).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_portfolio_holdings(app: tauri::AppHandle) -> Result<Vec<PortfolioHoldingView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    PortfolioUiService::list_watchlist(&database).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_portfolio_holding(
    app: tauri::AppHandle,
    input: CreateHoldingInput,
) -> Result<PortfolioHoldingView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    PortfolioUiService::create(&database, input).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_watchlist_item(
    app: tauri::AppHandle,
    input: CreateWatchlistInput,
) -> Result<PortfolioHoldingView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    PortfolioUiService::create_watchlist(&database, input).map_err(|error| error.to_string())
}

/// Fetches and stores a quote for one already-saved followed security. It intentionally does not
/// refresh indices, other follows, news or events; callers must treat a NO_DATA result as a quote
/// issue only, never as a failure to save the follow relationship.
#[tauri::command]
fn refresh_followed_security_quote(
    app: tauri::AppHandle,
    security_id: i64,
) -> Result<MarketRefreshView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    MarketRefreshService::refresh_followed_security_quote(&database, security_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn search_watchlist_securities(
    app: tauri::AppHandle,
    query: String,
) -> Result<Vec<SecurityLookupView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    PortfolioUiService::search_securities(&database, &query).map_err(|error| error.to_string())
}

#[tauri::command]
fn update_portfolio_holding(
    app: tauri::AppHandle,
    input: UpdateHoldingInput,
) -> Result<PortfolioHoldingView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    PortfolioUiService::update(&database, input).map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_portfolio_holding(app: tauri::AppHandle, holding_id: i64) -> Result<(), String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    PortfolioUiService::delete(&database, holding_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_followed_security_completely(
    app: tauri::AppHandle,
    input: DeleteWatchlistInput,
) -> Result<(), String> {
    eprintln!(
        "[hero-call][watchlist-diagnostic] remove_followed_security item_id={} security_id={}",
        input.watchlist_item_id, input.security_id
    );
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    PortfolioUiService::remove_followed_security_completely(&database, input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_settings_status(app: tauri::AppHandle) -> Result<SettingsStatusView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    SettingsService::load_status(&database).map_err(|error| error.to_string())
}

/// Stores the token in the OS secure credential store (macOS Keychain). It returns only the
/// safe configured/unconfigured state; the token is never persisted to SQLite or returned to UI.
#[tauri::command]
fn save_tushare_token(token: String) -> Result<TushareStatusView, String> {
    store_tushare_token(&token).map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_tushare_token() -> Result<TushareStatusView, String> {
    delete_tushare_token().map_err(|error| error.to_string())
}

#[tauri::command]
fn get_tushare_status() -> Result<TushareStatusView, String> {
    load_tushare_status().map_err(|error| error.to_string())
}

/// DeepSeek credentials use macOS Keychain and only the configured state crosses IPC.
#[tauri::command]
fn save_deepseek_api_key(key: String) -> Result<DeepSeekStatusView, String> {
    store_deepseek_api_key(&key).map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_deepseek_api_key() -> Result<DeepSeekStatusView, String> {
    delete_deepseek_api_key().map_err(|error| error.to_string())
}

#[tauri::command]
fn get_deepseek_status() -> Result<DeepSeekStatusView, String> {
    load_deepseek_status().map_err(|error| error.to_string())
}

#[tauri::command]
fn get_cash_accounts(app: tauri::AppHandle) -> Result<Vec<CashAccountView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    SettingsService::list_cash_accounts(&database).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_cash_account(
    app: tauri::AppHandle,
    input: CreateCashAccountInput,
) -> Result<CashAccountView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    SettingsService::create_cny_cash_account(&database, input).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_database_backup(app: tauri::AppHandle) -> Result<BackupView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    SettingsService::create_backup(&app, &database).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_holding_news_articles(app: tauri::AppHandle) -> Result<HoldingNewsView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    NewsService::list_for_holdings_with_status(&database).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_daily_review(app: tauri::AppHandle, review_date: String) -> Result<DailyReviewView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    ReviewService::get(&database, &review_date).map_err(|error| error.to_string())
}

#[tauri::command]
fn generate_daily_review(
    app: tauri::AppHandle,
    review_date: String,
) -> Result<DailyReviewView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    ReviewService::generate(&database, &review_date).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_ai_service_status() -> AiServiceStatusView {
    AiService::status()
}

#[tauri::command]
fn get_ai_provider_configs(app: tauri::AppHandle) -> Result<Vec<AiProviderConfigView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    AiService::provider_configs(&database).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_ai_provider_api_key(provider: String, key: String) -> Result<DeepSeekStatusView, String> {
    store_ai_provider_api_key(&provider, &key).map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_ai_provider_api_key(provider: String) -> Result<DeepSeekStatusView, String> {
    delete_ai_provider_api_key(&provider).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_ai_provider_key_status(provider: String) -> Result<DeepSeekStatusView, String> {
    load_ai_provider_key_status(&provider).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_ai_provider_enabled(
    app: tauri::AppHandle,
    provider: String,
    enabled: bool,
) -> Result<Vec<AiProviderConfigView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    AiService::set_provider_enabled(&database, &provider, enabled)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn test_ai_provider_connection(
    app: tauri::AppHandle,
    provider: String,
) -> Result<AiProviderConnectionTestView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    AiService::test_provider_connection(&database, &provider).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_latest_ai_review(
    app: tauri::AppHandle,
    review_id: i64,
) -> Result<Option<AiReviewView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    AiService::latest_for_review(&database, review_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn generate_ai_review_for_snapshot(
    app: tauri::AppHandle,
    review_date: String,
) -> Result<AiReviewView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    AiService::generate_from_runtime(&database, &review_date).map_err(|error| error.to_string())
}

#[tauri::command]
fn generate_ai_reviews_for_snapshot(
    app: tauri::AppHandle,
    review_date: String,
) -> Result<Vec<AiReviewView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    AiService::generate_all_from_runtime(&database, &review_date).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_ai_reviews_for_date(
    app: tauri::AppHandle,
    review_date: String,
) -> Result<Vec<AiReviewView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    let review = ReviewService::get(&database, &review_date).map_err(|error| error.to_string())?;
    AiService::list_for_review(&database, review.id).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_calendar_events(
    app: tauri::AppHandle,
    status: Option<String>,
) -> Result<Vec<EventView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    EventService::list(&database, status.as_deref()).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_initialization_status(app: tauri::AppHandle) -> Result<InitializationStatusView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    InitializationService::status(&database).map_err(|error| error.to_string())
}

#[tauri::command]
fn complete_initialization(app: tauri::AppHandle) -> Result<InitializationStatusView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    InitializationService::complete(&database).map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            database::service::DatabaseService::initialize_app_database(&app.handle())
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_asset_summary,
            get_market_snapshot,
            get_dashboard_data_status,
            refresh_today_market_snapshot,
            refresh_tushare_market_data,
            get_portfolio_holdings,
            create_portfolio_holding,
            create_watchlist_item,
            refresh_followed_security_quote,
            search_watchlist_securities,
            update_portfolio_holding,
            delete_portfolio_holding,
            remove_followed_security_completely,
            get_settings_status,
            save_tushare_token,
            remove_tushare_token,
            get_tushare_status,
            save_deepseek_api_key,
            remove_deepseek_api_key,
            get_deepseek_status,
            get_cash_accounts,
            create_cash_account,
            create_database_backup,
            get_holding_news_articles,
            get_daily_review,
            generate_daily_review,
            get_ai_service_status,
            get_ai_provider_configs,
            save_ai_provider_api_key,
            remove_ai_provider_api_key,
            get_ai_provider_key_status,
            set_ai_provider_enabled,
            test_ai_provider_connection,
            get_latest_ai_review,
            generate_ai_review_for_snapshot,
            generate_ai_reviews_for_snapshot,
            get_ai_reviews_for_date,
            get_calendar_events,
            get_initialization_status,
            complete_initialization
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Hero Call");
}
