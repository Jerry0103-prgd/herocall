//! Desktop application bootstrap.

mod database;
pub mod market_service;
pub mod portfolio_service;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            database::service::DatabaseService::initialize_app_database(&app.handle())
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run AStock AI Workbench");
}
