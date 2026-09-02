use crate::{db, model::DayState, Database};
use chrono::{DateTime, Local};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};
use tauri::State;

const START_MARKER: &str = "<!-- worklog:managed:start version=1 -->";
const END_MARKER: &str = "<!-- worklog:managed:end -->";
const SETTINGS_KEY: &str = "obsidian";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObsidianSettings {
    pub vault_path: Option<String>,
    pub daily_root: String,
}

impl Default for ObsidianSettings {
    fn default() -> Self {
        Self { vault_path: None, daily_root: String::new() }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyNotePreview {
    pub work_date: String,
    pub relative_path: String,
    pub markdown: String,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub work_date: String,
    pub relative_path: String,
    pub backup_path: Option<String>,
    pub content_hash: String,
}

pub fn initialize(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(include_str!("../migrations/0002_obsidian.sql"))
}

pub(crate) fn load_settings(connection: &Connection) -> Result<ObsidianSettings, String> {
    let stored: Option<String> = connection
        .query_row("SELECT value_json FROM app_settings WHERE key=?1", [SETTINGS_KEY], |row| row.get(0))
        .optional()
        .map_err(|error| error.to_string())?;
    let mut settings: ObsidianSettings = stored
        .map(|value| serde_json::from_str(&value).map_err(|error| format!("Obsidian 设置损坏：{error}")))
        .transpose()?
        .unwrap_or_default();
    if settings.daily_root == "工作日志" {
        settings.daily_root.clear();
    }
    validate_daily_root(&settings.daily_root)?;
    Ok(settings)
}

fn persist_settings(connection: &Connection, settings: &ObsidianSettings) -> Result<(), String> {
    let json = serde_json::to_string(settings).map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value_json,updated_at_utc) VALUES(?1,?2,?3)
             ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at_utc=excluded.updated_at_utc",
            params![SETTINGS_KEY, json, db::now_iso()],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn save_settings_core(connection: &Connection, vault_path: String) -> Result<ObsidianSettings, String> {
    let path = PathBuf::from(vault_path.trim());
    if !path.is_absolute() {
        return Err("Obsidian 工作区必须使用绝对路径".to_string());
    }
    if !path.is_dir() {
        return Err("所选 Obsidian 工作区不存在或不是文件夹".to_string());
    }
    let settings = ObsidianSettings {
        vault_path: Some(path.to_string_lossy().to_string()),
        ..ObsidianSettings::default()
    };
    persist_settings(connection, &settings)?;
    Ok(settings)
}

fn validate_daily_root(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if path.is_absolute() || path.components().any(|component| matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
        return Err("日记根目录必须位于 Obsidian 工作区内".to_string());
    }
    Ok(())
}

fn save_daily_root_core(connection: &Connection, daily_path: String) -> Result<ObsidianSettings, String> {
    let mut settings = load_settings(connection)?;
    let vault_text = settings.vault_path.clone().ok_or("请先选择 Obsidian 工作区")?;
    let vault = fs::canonicalize(&vault_text).map_err(|error| format!("无法访问 Obsidian 工作区：{error}"))?;
    let selected = PathBuf::from(daily_path.trim());
    if !selected.is_absolute() || !selected.is_dir() {
        return Err("日记根目录必须是已存在的绝对文件夹".to_string());
    }
    let selected = fs::canonicalize(selected).map_err(|error| format!("无法访问日记根目录：{error}"))?;
    let relative = selected.strip_prefix(&vault)
        .map_err(|_| "日记根目录必须位于所选 Obsidian 工作区内".to_string())?;
    settings.daily_root = relative.to_string_lossy().replace('\\', "/");
    validate_daily_root(&settings.daily_root)?;
    persist_settings(connection, &settings)?;
    Ok(settings)
}

pub fn daily_relative_path(settings: &ObsidianSettings, work_date: &str) -> Result<PathBuf, String> {
    validate_daily_root(&settings.daily_root)?;
    let date = chrono::NaiveDate::parse_from_str(work_date, "%Y-%m-%d")
        .map_err(|_| "日期必须为 YYYY-MM-DD".to_string())?;
    Ok(PathBuf::from(&settings.daily_root)
        .join(date.format("%Y").to_string())
        .join(date.format("%Y-%m").to_string())
        .join(format!("{}.md", date.format("%Y-%m-%d"))))
}

fn task_status_suffix(status: &str) -> &'static str {
    match status {
        "in_progress" => "（进行中）",
        "waiting" => "（等待他人）",
        "blocked" => "（阻塞）",
        "deferred" => "（延期）",
        "cancelled" => "（已取消）",
        _ => "",
    }
}

fn render_task_line(task: &crate::model::DayTask, indent: &str) -> String {
    let check = if task.status == "completed" { "x" } else { " " };
    format!(
        "{indent}- [{check}] {} {}{}\n",
        task.display_code,
        task.title,
        task_status_suffix(&task.status)
    )
}

pub fn render_managed(day: &DayState) -> String {
    let quadrants = [
        ("重要 · 紧急", "important", "urgent"),
        ("重要 · 稍缓", "important", "relaxed"),
        ("次要 · 紧急", "secondary", "urgent"),
        ("次要 · 稍缓", "secondary", "relaxed"),
    ];
    let mut output = format!("{START_MARKER}\n## 今日任务\n\n");
    for (title, importance, urgency) in quadrants {
        output.push_str(&format!("### {title}\n"));
        let parents: Vec<_> = day.tasks.iter().filter(|task| {
            task.parent_id.is_none() && task.importance == importance && task.urgency == urgency
        }).collect();
        if parents.is_empty() {
            output.push_str("- 暂无\n");
        } else {
            for task in parents {
                output.push_str(&render_task_line(task, ""));
                for child in day.tasks.iter().filter(|child| child.parent_id.as_deref() == Some(task.id.as_str())) {
                    output.push_str(&render_task_line(child, "  "));
                }
            }
        }
        output.push('\n');
    }

    output.push_str("## 今日安排\n\n");
    let scheduled: Vec<_> = day.tasks.iter().filter(|task| task.planned_start.is_some() && task.planned_end.is_some()).collect();
    if scheduled.is_empty() {
        output.push_str("- 暂无固定时段\n");
    } else {
        for task in scheduled {
            output.push_str(&format!(
                "- {}–{}：{} {}\n",
                task.planned_start.as_deref().unwrap_or_default(),
                task.planned_end.as_deref().unwrap_or_default(),
                task.display_code,
                task.title
            ));
        }
    }

    output.push_str("\n## 今日记录\n\n");
    let visible: Vec<_> = day.timeline.iter().filter(|event| event.visibility != "hidden").collect();
    if visible.is_empty() {
        output.push_str("- 今日暂无记录\n");
    } else {
        for event in visible {
            let time = DateTime::parse_from_rfc3339(&event.occurred_at)
                .map(|value| value.with_timezone(&Local).format("%H:%M").to_string())
                .unwrap_or_else(|_| "--:--".to_string());
            output.push_str(&format!("- {time}：{}\n", event.title));
        }
    }
    output.push_str(&format!("{END_MARKER}\n"));
    output
}

pub fn merge_managed(existing: &str, managed: &str) -> Result<String, String> {
    let starts: Vec<_> = existing.match_indices(START_MARKER).collect();
    let ends: Vec<_> = existing.match_indices(END_MARKER).collect();
    match (starts.len(), ends.len()) {
        (0, 0) => {
            if existing.trim().is_empty() {
                Ok(managed.to_string())
            } else {
                Ok(format!("{}\n\n{}", existing.trim_end(), managed))
            }
        }
        (1, 1) => {
            let start = starts[0].0;
            let end = ends[0].0 + END_MARKER.len();
            if start >= ends[0].0 {
                return Err("Obsidian 日记中的受管理区块标记顺序异常，已停止覆盖".to_string());
            }
            Ok(format!("{}{}{}", &existing[..start], managed.trim_end(), &existing[end..]))
        }
        _ => Err("Obsidian 日记中的受管理区块标记缺失或重复，已停止覆盖".to_string()),
    }
}

fn sha256(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn preview_core(connection: &Connection, work_date: &str) -> Result<DailyNotePreview, String> {
    let settings = load_settings(connection)?;
    let relative = daily_relative_path(&settings, work_date)?;
    let day = db::read_day(connection, work_date)?;
    Ok(DailyNotePreview {
        work_date: work_date.to_string(),
        relative_path: relative.to_string_lossy().to_string(),
        markdown: render_managed(&day),
        configured: settings.vault_path.is_some(),
    })
}

pub(crate) fn create_backup(vault: &Path, relative: &Path, source: &Path) -> Result<PathBuf, String> {
    let stamp = Local::now().format("%Y%m%d-%H%M%S%.3f").to_string();
    let file_name = relative.file_name().and_then(|value| value.to_str()).unwrap_or("daily.md");
    let backup = vault
        .join(".worklog-backups")
        .join(relative.parent().unwrap_or_else(|| Path::new("")))
        .join(format!("{file_name}.{stamp}.bak"));
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建备份目录：{error}"))?;
    }
    fs::copy(source, &backup).map_err(|error| format!("无法备份原日记：{error}"))?;
    Ok(backup)
}

pub(crate) fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or("日记路径没有父目录")?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建日记目录：{error}"))?;
    let file_name = path.file_name().and_then(|value| value.to_str()).unwrap_or("daily.md");
    let temp = parent.join(format!(".{file_name}.worklog-{}.tmp", db::new_id()));
    let result = (|| -> Result<(), String> {
        let mut file = File::create(&temp).map_err(|error| format!("无法创建临时文件：{error}"))?;
        file.write_all(content.as_bytes()).map_err(|error| format!("无法写入临时文件：{error}"))?;
        file.sync_all().map_err(|error| format!("无法同步临时文件：{error}"))?;
        replace_file(&temp, path).map_err(|error| format!("无法原子替换日记：{error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(windows)]
fn replace_file(temp: &Path, destination: &Path) -> io::Result<()> {
    if !destination.exists() {
        return fs::rename(temp, destination);
    }
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;
    let destination_wide: Vec<u16> = destination.as_os_str().encode_wide().chain(Some(0)).collect();
    let temp_wide: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        ReplaceFileW(
            destination_wide.as_ptr(),
            temp_wide.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

#[cfg(not(windows))]
fn replace_file(temp: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temp, destination)
}

fn record_sync_state(
    connection: &Connection,
    work_date: &str,
    relative_path: &str,
    state: &str,
    hash: Option<&str>,
    error: Option<&str>,
    succeeded: bool,
) -> Result<(), String> {
    let now = db::now_iso();
    connection.execute(
        "INSERT INTO daily_note_sync(work_date,relative_path,sync_state,last_generated_hash,last_error,last_attempt_at_utc,last_success_at_utc)
         VALUES(?1,?2,?3,?4,?5,?6,CASE WHEN ?7 THEN ?6 ELSE NULL END)
         ON CONFLICT(work_date) DO UPDATE SET
           relative_path=excluded.relative_path,sync_state=excluded.sync_state,
           last_generated_hash=COALESCE(excluded.last_generated_hash,daily_note_sync.last_generated_hash),
           last_error=excluded.last_error,last_attempt_at_utc=excluded.last_attempt_at_utc,
           last_success_at_utc=CASE WHEN ?7 THEN excluded.last_attempt_at_utc ELSE daily_note_sync.last_success_at_utc END,
           row_version=daily_note_sync.row_version+1",
        params![work_date, relative_path, state, hash, error, now, succeeded],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

fn sync_core(connection: &Connection, work_date: &str) -> Result<SyncResult, String> {
    let settings = load_settings(connection)?;
    let vault_text = settings.vault_path.clone().ok_or("请先选择 Obsidian 工作区")?;
    let vault = PathBuf::from(vault_text);
    if !vault.is_dir() {
        return Err("已配置的 Obsidian 工作区不可访问，请重新选择".to_string());
    }
    let relative = daily_relative_path(&settings, work_date)?;
    let relative_text = relative.to_string_lossy().to_string();
    let destination = vault.join(&relative);
    let day = db::read_day(connection, work_date)?;
    let managed = render_managed(&day);
    let existing = if destination.exists() {
        fs::read_to_string(&destination).map_err(|error| format!("无法读取现有日记：{error}"))?
    } else {
        format!("# {work_date}\n")
    };

    record_sync_state(connection, work_date, &relative_text, "writing", None, None, false)?;
    let merged = match merge_managed(&existing, &managed) {
        Ok(value) => value,
        Err(error) => {
            record_sync_state(connection, work_date, &relative_text, "conflict", None, Some(&error), false)?;
            return Err(error);
        }
    };
    let backup = if destination.exists() {
        Some(create_backup(&vault, &relative, &destination)?)
    } else {
        None
    };
    if let Err(error) = atomic_write(&destination, &merged) {
        record_sync_state(connection, work_date, &relative_text, "error", None, Some(&error), false)?;
        return Err(error);
    }
    let hash = sha256(&merged);
    record_sync_state(connection, work_date, &relative_text, "clean", Some(&hash), None, true)?;
    Ok(SyncResult {
        work_date: work_date.to_string(),
        relative_path: relative_text,
        backup_path: backup.map(|path| path.to_string_lossy().to_string()),
        content_hash: hash,
    })
}

#[tauri::command]
pub fn get_obsidian_settings(database: State<'_, Database>) -> Result<ObsidianSettings, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    load_settings(&connection)
}

#[tauri::command(rename_all = "camelCase")]
pub fn save_obsidian_settings(database: State<'_, Database>, vault_path: String) -> Result<ObsidianSettings, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    save_settings_core(&connection, vault_path)
}

#[tauri::command(rename_all = "camelCase")]
pub fn save_daily_root(database: State<'_, Database>, daily_path: String) -> Result<ObsidianSettings, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    save_daily_root_core(&connection, daily_path)
}

#[tauri::command(rename_all = "camelCase")]
pub fn preview_daily_note(database: State<'_, Database>, work_date: String) -> Result<DailyNotePreview, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    preview_core(&connection, &work_date)
}

#[tauri::command(rename_all = "camelCase")]
pub fn sync_daily_note(database: State<'_, Database>, work_date: String) -> Result<SyncResult, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    sync_core(&connection, &work_date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DayTask, TimelineEvent};

    fn settings() -> ObsidianSettings {
        ObsidianSettings::default()
    }

    fn sample_day() -> DayState {
        DayState {
            work_date: "2026-09-02".to_string(),
            tasks: vec![
                DayTask {
                    id: "instance-1".into(), permanent_task_id: "task-1".into(), parent_id: None,
                    display_code: "#1".into(), title: "整理资料".into(), status: "completed".into(),
                    importance: "important".into(), urgency: "urgent".into(),
                    planned_start: Some("08:00".into()), planned_end: Some("09:00".into()),
                    created_at: "2026-09-02T00:00:00Z".into(),
                },
                DayTask {
                    id: "instance-2".into(), permanent_task_id: "task-2".into(), parent_id: Some("instance-1".into()),
                    display_code: "#1.1".into(), title: "制作 PPT".into(), status: "not_started".into(),
                    importance: "important".into(), urgency: "urgent".into(),
                    planned_start: None, planned_end: None, created_at: "2026-09-02T00:00:00Z".into(),
                },
            ],
            timeline: vec![
                TimelineEvent {
                    id: "event-1".into(), event_type: "task.completed".into(),
                    occurred_at: "2026-09-02T10:43:00+08:00".into(),
                    title: "完成#1：整理资料".into(), detail: None, visibility: "summary".into(),
                },
                TimelineEvent {
                    id: "event-2".into(), event_type: "work_entry.created".into(),
                    occurred_at: "2026-09-02T10:44:00+08:00".into(),
                    title: "临时草稿".into(), detail: None, visibility: "hidden".into(),
                },
            ],
            focus: None,
            rest: None,
        }
    }

    #[test]
    fn daily_path_matches_obsidian_layout() {
        assert_eq!(
            daily_relative_path(&settings(), "2026-09-02").unwrap(),
            PathBuf::from("2026").join("2026-09").join("2026-09-02.md")
        );
    }

    #[test]
    fn unsafe_daily_root_is_rejected() {
        let mut value = settings();
        value.daily_root = "../outside".to_string();
        assert!(daily_relative_path(&value, "2026-09-02").is_err());
    }

    #[test]
    fn first_merge_preserves_manual_text() {
        let merged = merge_managed("# 旧日记\n\n人工内容", &render_managed(&sample_day())).unwrap();
        assert!(merged.starts_with("# 旧日记\n\n人工内容"));
        assert!(merged.contains(START_MARKER));
    }

    #[test]
    fn replacement_preserves_text_around_managed_block() {
        let existing = format!("前文\n{START_MARKER}\n旧内容\n{END_MARKER}\n后文");
        let merged = merge_managed(&existing, &render_managed(&sample_day())).unwrap();
        assert!(merged.starts_with("前文\n"));
        assert!(merged.ends_with("\n后文"));
        assert!(!merged.contains("旧内容"));
    }

    #[test]
    fn corrupt_markers_are_rejected() {
        assert!(merge_managed(&format!("内容\n{START_MARKER}"), "new").is_err());
    }

    #[test]
    fn renderer_is_quiet_and_structured() {
        let markdown = render_managed(&sample_day());
        assert!(markdown.contains("## 今日任务"));
        assert!(markdown.contains("- [x] #1 整理资料"));
        assert!(markdown.contains("  - [ ] #1.1 制作 PPT"));
        assert!(markdown.contains("08:00–09:00：#1 整理资料"));
        let local_time = DateTime::parse_from_rfc3339("2026-09-02T10:43:00+08:00")
            .unwrap()
            .with_timezone(&Local)
            .format("%H:%M")
            .to_string();
        assert!(markdown.contains(&format!("{local_time}：完成#1：整理资料")));
        assert!(!markdown.contains("临时草稿"));
    }
}
