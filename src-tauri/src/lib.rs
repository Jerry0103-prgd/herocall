//! Desktop application bootstrap.
//! The SQLite plugin is registered here only; V1.0 business tables are intentionally absent.

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_sql::Builder::default().build())
        .run(tauri::generate_context!())
        .expect("failed to run AStock AI Workbench");
}
