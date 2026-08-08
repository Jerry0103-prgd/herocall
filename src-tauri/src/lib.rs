//! Desktop application bootstrap.

mod dashboard_service;
mod database;
pub mod market_service;
#[allow(dead_code)] // Phase 6-A defines the future Adapter ingestion/CRUD API; UI is read-only.
mod news_service;
pub mod portfolio_service;
mod portfolio_ui_service;
mod settings_service;

use dashboard_service::{AssetSummaryView, DashboardService, MarketIndexQuoteView};
use news_service::{NewsArticleView, NewsService};
use portfolio_ui_service::{
    CreateHoldingInput, PortfolioHoldingView, PortfolioUiService, UpdateHoldingInput,
};
use settings_service::{
    BackupView, CashAccountView, CreateCashAccountInput, SettingsService, SettingsStatusView,
};

#[tauri::command]
fn get_asset_summary(app: tauri::AppHandle) -> Result<AssetSummaryView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    Ok(DashboardService::load_asset_summary(&database))
}

#[tauri::command]
fn get_market_snapshot(app: tauri::AppHandle) -> Result<Vec<MarketIndexQuoteView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    Ok(DashboardService::load_market_snapshot(&database))
}

#[tauri::command]
fn get_portfolio_holdings(app: tauri::AppHandle) -> Result<Vec<PortfolioHoldingView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    PortfolioUiService::list(&database).map_err(|error| error.to_string())
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
fn get_settings_status(app: tauri::AppHandle) -> Result<SettingsStatusView, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    SettingsService::load_status(&database).map_err(|error| error.to_string())
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
fn get_holding_news_articles(app: tauri::AppHandle) -> Result<Vec<NewsArticleView>, String> {
    let database = database::service::DatabaseService::open_app_database(&app)
        .map_err(|error| error.to_string())?;
    NewsService::list_for_holdings(&database).map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            database::service::DatabaseService::initialize_app_database(&app.handle())
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_asset_summary,
            get_market_snapshot,
            get_portfolio_holdings,
            create_portfolio_holding,
            update_portfolio_holding,
            delete_portfolio_holding,
            get_settings_status,
            get_cash_accounts,
            create_cash_account,
            create_database_backup,
            get_holding_news_articles
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AStock AI Workbench");
}
