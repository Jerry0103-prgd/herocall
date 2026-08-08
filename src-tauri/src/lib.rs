//! Desktop application bootstrap.

mod dashboard_service;
mod database;
pub mod market_service;
pub mod portfolio_service;

use dashboard_service::{AssetSummaryView, DashboardService, MarketIndexQuoteView};

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

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            database::service::DatabaseService::initialize_app_database(&app.handle())
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_asset_summary,
            get_market_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AStock AI Workbench");
}
