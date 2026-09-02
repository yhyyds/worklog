use rusqlite::Connection;
use serde::Serialize;
use std::{fs, sync::Mutex};
use tauri::{Manager, State};

struct Database(Mutex<Connection>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Healthcheck {
    app_version: &'static str,
    database_status: String,
    sqlite_version: String,
}

#[tauri::command]
fn healthcheck(database: State<'_, Database>) -> Result<Healthcheck, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    let database_status: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let sqlite_version: String = connection
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    Ok(Healthcheck { app_version: env!("CARGO_PKG_VERSION"), database_status, sqlite_version })
}

fn open_database(path: &std::path::Path) -> Result<Connection, Box<dyn std::error::Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(include_str!("../migrations/0001_initial.sql"))?;
    Ok(connection)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_dir)?;
            let connection = open_database(&app_dir.join("worklog.db"))?;
            app.manage(Database(Mutex::new(connection)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![healthcheck])
        .run(tauri::generate_context!())
        .expect("failed to run Worklog");
}
