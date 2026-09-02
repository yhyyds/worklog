mod closing;
mod commands;
mod db;
mod model;
mod notes;
mod obsidian;
mod timer;

use rusqlite::Connection;
use serde::Serialize;
use std::{fs, sync::Mutex};
use tauri::{menu::{Menu, MenuItem}, tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}, Manager, State};

pub(crate) struct Database(pub(crate) Mutex<Connection>);

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
    let database_status: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    let sqlite_version: String = connection.query_row("SELECT sqlite_version()", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    Ok(Healthcheck { app_version: env!("CARGO_PKG_VERSION"), database_status, sqlite_version })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_dir)?;
            let connection = db::open_database(&app_dir.join("worklog.db"))?;
            app.manage(Database(Mutex::new(connection)));

            let open_item = MenuItem::with_id(app, "open", "打开 Worklog", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &quit_item])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Worklog")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            timer::start_background(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            healthcheck,
            commands::get_day_snapshot,
            commands::create_task,
            commands::set_task_status,
            commands::add_work_entry,
            commands::start_focus,
            commands::pause_focus,
            commands::resume_focus,
            commands::switch_focus,
            commands::complete_focus,
            timer::get_timer_settings,
            timer::save_timer_settings,
            timer::pause_rest,
            timer::resume_rest,
            timer::complete_rest,
            timer::skip_rest,
            closing::preview_end_of_day,
            closing::close_day,
            obsidian::get_obsidian_settings,
            obsidian::save_obsidian_settings,
            obsidian::save_daily_root,
            obsidian::preview_daily_note,
            obsidian::sync_daily_note,
            notes::list_vault_notes,
            notes::read_vault_note,
            notes::save_vault_note,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Worklog");
}
