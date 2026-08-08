//! Desktop application bootstrap.

mod dashboard_service;
mod database;
pub mod market_service;
pub mod portfolio_service;
mod portfolio_ui_service;

use dashboard_service::{AssetSummaryView, DashboardService, MarketIndexQuoteView};
use portfolio_ui_service::{
    CreateHoldingInput, PortfolioHoldingView, PortfolioUiService, UpdateHoldingInput,
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
            delete_portfolio_holding
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AStock AI Workbench");
}
