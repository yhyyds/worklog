use crate::{db, Database};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::State;

const POINTER_FILE: &str = "storage-location.json";
const DATABASE_FILE: &str = "worklog.db";

pub(crate) struct StorageRuntime {
    app_data_directory: PathBuf,
    current_directory: Mutex<PathBuf>,
}

impl StorageRuntime {
    pub(crate) fn new(app_data_directory: PathBuf, current_directory: PathBuf) -> Self {
        Self {
            app_data_directory,
            current_directory: Mutex::new(current_directory),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoragePointer {
    data_directory: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSettings {
    current_directory: String,
    database_path: String,
    default_directory: String,
    is_default: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMigration {
    settings: StorageSettings,
    previous_database_path: String,
}

pub(crate) fn resolve_data_directory(app_data_directory: &Path) -> PathBuf {
    let pointer_path = app_data_directory.join(POINTER_FILE);
    let configured = fs::read_to_string(pointer_path)
        .ok()
        .and_then(|value| serde_json::from_str::<StoragePointer>(&value).ok())
        .map(|pointer| PathBuf::from(pointer.data_directory));

    configured
        .filter(|path| path.is_absolute() && path.is_dir())
        .and_then(|path| fs::canonicalize(path).ok())
        .unwrap_or_else(|| app_data_directory.to_path_buf())
}

fn settings(runtime: &StorageRuntime) -> Result<StorageSettings, String> {
    let current = runtime
        .current_directory
        .lock()
        .map_err(|_| "storage lock poisoned".to_string())?
        .clone();
    let current = fs::canonicalize(&current).unwrap_or(current);
    let default = fs::canonicalize(&runtime.app_data_directory)
        .unwrap_or_else(|_| runtime.app_data_directory.clone());

    Ok(StorageSettings {
        current_directory: current.to_string_lossy().to_string(),
        database_path: current.join(DATABASE_FILE).to_string_lossy().to_string(),
        default_directory: default.to_string_lossy().to_string(),
        is_default: current == default,
    })
}

fn persist_pointer(app_data_directory: &Path, data_directory: &Path) -> Result<(), String> {
    let pointer_path = app_data_directory.join(POINTER_FILE);
    let pointer = StoragePointer {
        data_directory: data_directory.to_string_lossy().to_string(),
    };
    let json = serde_json::to_vec_pretty(&pointer).map_err(|error| error.to_string())?;
    let mut file = File::create(&pointer_path)
        .map_err(|error| format!("无法保存本地存储位置：{error}"))?;
    file.write_all(&json)
        .map_err(|error| format!("无法写入本地存储位置：{error}"))?;
    file.sync_all()
        .map_err(|error| format!("无法同步本地存储位置：{error}"))
}

fn vacuum_into(connection: &Connection, destination: &Path) -> Result<(), String> {
    connection
        .execute("VACUUM INTO ?1", [destination.to_string_lossy().as_ref()])
        .map_err(|error| format!("无法迁移本地数据库：{error}"))?;
    Ok(())
}

#[tauri::command]
pub fn get_storage_settings(runtime: State<'_, StorageRuntime>) -> Result<StorageSettings, String> {
    settings(&runtime)
}

#[tauri::command(rename_all = "camelCase")]
pub fn migrate_storage_directory(
    database: State<'_, Database>,
    runtime: State<'_, StorageRuntime>,
    directory: String,
) -> Result<StorageMigration, String> {
    let selected = PathBuf::from(directory.trim());
    if !selected.is_absolute() || !selected.is_dir() {
        return Err("本地存储路径必须是已存在的绝对文件夹".to_string());
    }
    let selected = fs::canonicalize(selected)
        .map_err(|error| format!("无法访问所选本地文件夹：{error}"))?;
    let current = runtime
        .current_directory
        .lock()
        .map_err(|_| "storage lock poisoned".to_string())?
        .clone();
    let current = fs::canonicalize(&current).unwrap_or(current);
    let previous_database_path = current.join(DATABASE_FILE);

    if selected == current {
        return Ok(StorageMigration {
            settings: settings(&runtime)?,
            previous_database_path: previous_database_path.to_string_lossy().to_string(),
        });
    }

    let destination = selected.join(DATABASE_FILE);
    if destination.exists() {
        return Err("目标文件夹中已存在 worklog.db。为避免覆盖数据，请选择一个空文件夹。".to_string());
    }

    let temp = selected.join(format!(".worklog-{}.migrating", db::new_id()));
    let mut connection = database
        .0
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;

    if let Err(error) = vacuum_into(&connection, &temp) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temp, &destination) {
        let _ = fs::remove_file(&temp);
        return Err(format!("无法完成数据库迁移：{error}"));
    }

    let new_connection = match db::open_database(&destination) {
        Ok(value) => value,
        Err(error) => return Err(format!("无法打开迁移后的数据库：{error}")),
    };
    persist_pointer(&runtime.app_data_directory, &selected)?;
    *connection = new_connection;
    drop(connection);

    *runtime
        .current_directory
        .lock()
        .map_err(|_| "storage lock poisoned".to_string())? = selected;

    Ok(StorageMigration {
        settings: settings(&runtime)?,
        previous_database_path: previous_database_path.to_string_lossy().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_pointer_uses_default_directory() {
        let root = std::env::temp_dir().join(format!("worklog-storage-{}", db::new_id()));
        fs::create_dir_all(&root).unwrap();
        assert_eq!(resolve_data_directory(&root), root);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn valid_pointer_uses_selected_directory() {
        let root = std::env::temp_dir().join(format!("worklog-storage-{}", db::new_id()));
        let selected = root.join("selected");
        fs::create_dir_all(&selected).unwrap();
        persist_pointer(&root, &selected).unwrap();
        assert_eq!(
            resolve_data_directory(&root),
            fs::canonicalize(&selected).unwrap()
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
