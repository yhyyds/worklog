mod closing;
mod commands;
mod db;
#[cfg(debug_assertions)]
mod demo;
mod growth;
mod categories;
mod planning;
mod sharing;
mod inbox;
mod model;
mod notes;
mod obsidian;
mod reports;
mod report_details;
mod storage;
mod timer;
mod windowing;

use rusqlite::Connection;
use serde::Serialize;
use std::{fs, sync::Mutex};
use tauri::{menu::{Menu, MenuItem}, tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}, Manager, State, WindowEvent};

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
            #[cfg(debug_assertions)]
            let demo_dir = if std::env::args().any(|arg|arg=="--demo") { Some(demo::prepare()?) } else { None };
            #[cfg(not(debug_assertions))]
            let demo_dir: Option<std::path::PathBuf> = None;
            let app_dir = match demo_dir.as_ref() { Some(path) => path.clone(), None => app.path().app_data_dir()? };
            fs::create_dir_all(&app_dir)?;
            let data_dir = storage::resolve_data_directory(&app_dir);
            fs::create_dir_all(&data_dir)?;
            let connection = db::open_database(&data_dir.join("worklog.db"))?;
            app.manage(Database(Mutex::new(connection)));
            app.manage(storage::StorageRuntime::new(app_dir, data_dir));
            if let Some(path) = demo_dir {
                if let Some(window) = app.get_webview_window("main") {
                    window.set_title(&format!("Worklog 1.0 · 隔离演示 · {}",path.display()))?;
                }
            }

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
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let action = if let Some(database) = window.app_handle().try_state::<Database>() {
                    match database.0.lock() {
                        Ok(connection) => windowing::load_settings(&connection)
                            .map(|settings| settings.close_action)
                            .unwrap_or(windowing::CloseAction::HideToTray),
                        Err(_) => windowing::CloseAction::HideToTray,
                    }
                } else {
                    windowing::CloseAction::HideToTray
                };
                match action {
                    windowing::CloseAction::Quit => window.app_handle().exit(0),
                    windowing::CloseAction::HideToTray => {
                        let _ = window.hide();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            healthcheck,
            categories::get_growth_catalog,
            categories::save_growth_category,
            categories::assign_growth_category,
            planning::save_goal_action_plan,
            planning::delete_goal_action,
            sharing::get_report_overview,
            sharing::get_share_preference,
            sharing::save_share_preference,
            inbox::list_inbox,
            inbox::create_inbox_task,
            inbox::move_task_to_inbox,
            inbox::schedule_inbox_task,
            inbox::list_historical_unfinished,
            inbox::reschedule_historical_task,
            commands::get_day_snapshot,
            commands::create_task,
            commands::update_task,
            commands::set_task_status,
            commands::add_work_entry,
            commands::start_focus,
            commands::pause_focus,
            commands::resume_focus,
            commands::switch_focus,
            commands::complete_focus,
            growth::list_habits,
            growth::create_habit,
            growth::archive_habit,
            growth::get_habit_review,
            growth::complete_habit_review,
            growth::list_long_term_goals,
            growth::create_long_term_goal,
            growth::create_goal_phase,
            growth::save_goal_phase_note,
            growth::create_goal_action,
            growth::set_goal_action_progress,
            reports::get_weekly_report,
            reports::save_weekly_report_image,
            timer::get_timer_settings,
            timer::save_timer_settings,
            windowing::get_window_behavior,
            windowing::save_window_behavior,
            storage::get_storage_settings,
            storage::migrate_storage_directory,
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
            obsidian::get_daily_note_sync_status,
            notes::list_vault_notes,
            notes::read_vault_note,
            notes::save_vault_note,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Worklog");
}
