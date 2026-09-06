use crate::{db, Database};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

const SETTINGS_KEY: &str = "window_behavior";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowBehaviorSettings {
    pub close_action: CloseAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CloseAction {
    Quit,
    HideToTray,
}

impl Default for WindowBehaviorSettings {
    fn default() -> Self {
        Self { close_action: CloseAction::HideToTray }
    }
}

pub(crate) fn load_settings(connection: &Connection) -> Result<WindowBehaviorSettings, String> {
    let value: Option<String> = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key=?1",
            [SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    value
        .map(|json| serde_json::from_str(&json).map_err(|error| format!("窗口设置损坏：{error}")))
        .transpose()
        .map(|settings| settings.unwrap_or_default())
}

fn save_settings_core(
    connection: &Connection,
    settings: WindowBehaviorSettings,
) -> Result<WindowBehaviorSettings, String> {
    let json = serde_json::to_string(&settings).map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value_json,updated_at_utc) VALUES(?1,?2,?3)
             ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at_utc=excluded.updated_at_utc",
            params![SETTINGS_KEY, json, db::now_iso()],
        )
        .map_err(|error| error.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn get_window_behavior(
    database: State<'_, Database>,
) -> Result<WindowBehaviorSettings, String> {
    let connection = database
        .0
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    load_settings(&connection)
}

#[tauri::command]
pub fn save_window_behavior(
    database: State<'_, Database>,
    settings: WindowBehaviorSettings,
) -> Result<WindowBehaviorSettings, String> {
    let connection = database
        .0
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    save_settings_core(&connection, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE app_settings(
                    key TEXT PRIMARY KEY,
                    value_json TEXT NOT NULL,
                    updated_at_utc TEXT NOT NULL
                );",
            )
            .unwrap();
        connection
    }

    #[test]
    fn defaults_to_hiding_in_the_tray() {
        assert_eq!(
            load_settings(&connection()).unwrap(),
            WindowBehaviorSettings::default()
        );
    }

    #[test]
    fn saves_and_loads_the_quit_action() {
        let connection = connection();
        save_settings_core(
            &connection,
            WindowBehaviorSettings { close_action: CloseAction::Quit },
        )
        .unwrap();
        assert_eq!(
            load_settings(&connection).unwrap().close_action,
            CloseAction::Quit
        );
    }
}
