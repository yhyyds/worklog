use crate::{db, obsidian, Database};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use tauri::State;

const MAX_NOTE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_LISTED_NOTES: usize = 5000;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VaultNoteSummary {
    pub relative_path: String,
    pub title: String,
    pub modified_at: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VaultNote {
    pub relative_path: String,
    pub title: String,
    pub content: String,
    pub modified_at: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVaultNoteInput {
    pub work_date: String,
    pub relative_path: Option<String>,
    pub directory: Option<String>,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVaultNoteResult {
    pub note: VaultNote,
    pub created: bool,
    pub backup_path: Option<String>,
}

pub fn initialize(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(include_str!("../migrations/0004_essay_notes.sql"))
}

fn configured_vault(connection: &Connection) -> Result<PathBuf, String> {
    let settings = obsidian::load_settings(connection)?;
    let path = PathBuf::from(settings.vault_path.ok_or("请先在 Obsidian 同步中选择工作区")?);
    if !path.is_dir() {
        return Err("已配置的 Obsidian 工作区不可访问，请重新选择".to_string());
    }
    path.canonicalize().map_err(|error| format!("无法解析 Obsidian 工作区：{error}"))
}

fn safe_relative(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.is_absolute() {
        return Err("笔记路径必须是工作区内的相对路径".to_string());
    }
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            _ => return Err("笔记路径包含不安全的路径片段".to_string()),
        }
    }
    Ok(safe)
}

fn markdown_relative(value: &str) -> Result<PathBuf, String> {
    let path = safe_relative(value)?;
    let extension = path.extension().and_then(|part| part.to_str()).unwrap_or_default();
    if !extension.eq_ignore_ascii_case("md") {
        return Err("只能读取或保存 Markdown 文件".to_string());
    }
    Ok(path)
}

fn validate_title(value: &str) -> Result<String, String> {
    let title = value.trim();
    if title.is_empty() {
        return Err("笔记标题不能为空".to_string());
    }
    if title.len() > 160 {
        return Err("笔记标题不能超过 160 个字符".to_string());
    }
    if title.chars().any(|character| character.is_control() || r#"<>:"/\|?*"#.contains(character)) {
        return Err("笔记标题包含 Windows 文件名不允许的字符".to_string());
    }
    if title.ends_with('.') || title.ends_with(' ') {
        return Err("笔记标题不能以点或空格结尾".to_string());
    }
    let upper = title.to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];
    if reserved.contains(&upper.as_str()) {
        return Err("笔记标题是 Windows 保留文件名".to_string());
    }
    Ok(title.to_string())
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\', "/")
}

fn modified_text(metadata: &fs::Metadata) -> String {
    metadata.modified().ok()
        .map(DateTime::<Utc>::from)
        .map(|value| value.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_else(db::now_iso)
}

fn ensure_existing_inside(vault: &Path, relative: &Path) -> Result<PathBuf, String> {
    let target = vault.join(relative);
    let canonical = target.canonicalize().map_err(|_| "笔记不存在或已被移动".to_string())?;
    if !canonical.starts_with(vault) {
        return Err("笔记路径越出了 Obsidian 工作区".to_string());
    }
    Ok(canonical)
}

fn collect_notes(vault: &Path, directory: &Path, notes: &mut Vec<VaultNoteSummary>) -> Result<(), String> {
    if notes.len() >= MAX_LISTED_NOTES {
        return Ok(());
    }
    let entries = fs::read_dir(directory).map_err(|error| format!("无法读取工作区目录：{error}"))?;
    for entry in entries {
        if notes.len() >= MAX_LISTED_NOTES {
            break;
        }
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_notes(vault, &path, notes)?;
        } else if file_type.is_file() && path.extension().and_then(|part| part.to_str()).is_some_and(|extension| extension.eq_ignore_ascii_case("md")) {
            let metadata = entry.metadata().map_err(|error| error.to_string())?;
            let relative = path.strip_prefix(vault).map_err(|error| error.to_string())?;
            notes.push(VaultNoteSummary {
                relative_path: path_text(relative),
                title: path.file_stem().and_then(|part| part.to_str()).unwrap_or("未命名").to_string(),
                modified_at: modified_text(&metadata),
                size_bytes: metadata.len(),
            });
        }
    }
    Ok(())
}

fn list_core(connection: &Connection) -> Result<Vec<VaultNoteSummary>, String> {
    let vault = configured_vault(connection)?;
    let mut notes = Vec::new();
    collect_notes(&vault, &vault, &mut notes)?;
    notes.sort_by(|left, right| right.modified_at.cmp(&left.modified_at).then_with(|| left.relative_path.cmp(&right.relative_path)));
    Ok(notes)
}

fn read_core(connection: &Connection, relative_path: &str) -> Result<VaultNote, String> {
    let vault = configured_vault(connection)?;
    let relative = markdown_relative(relative_path)?;
    let target = ensure_existing_inside(&vault, &relative)?;
    let metadata = fs::metadata(&target).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_NOTE_BYTES {
        return Err("笔记超过 4 MB，暂不在 Worklog 中打开".to_string());
    }
    let content = fs::read_to_string(&target).map_err(|error| format!("笔记不是有效的 UTF-8 Markdown：{error}"))?;
    Ok(VaultNote {
        relative_path: path_text(&relative),
        title: target.file_stem().and_then(|part| part.to_str()).unwrap_or("未命名").to_string(),
        content,
        modified_at: modified_text(&metadata),
        size_bytes: metadata.len(),
    })
}

fn save_core(connection: &mut Connection, input: SaveVaultNoteInput) -> Result<SaveVaultNoteResult, String> {
    chrono::NaiveDate::parse_from_str(&input.work_date, "%Y-%m-%d")
        .map_err(|_| "日期必须为 YYYY-MM-DD".to_string())?;
    let vault = configured_vault(connection)?;
    let title = validate_title(&input.title)?;
    let relative = if let Some(existing) = &input.relative_path {
        markdown_relative(existing)?
    } else {
        let directory = safe_relative(input.directory.as_deref().unwrap_or("随笔"))?;
        directory.join(format!("{title}.md"))
    };
    let target = vault.join(&relative);
    let existed = target.exists();

    if existed {
        ensure_existing_inside(&vault, &relative)?;
    } else {
        let parent = target.parent().ok_or("笔记路径没有父目录")?;
        fs::create_dir_all(parent).map_err(|error| format!("无法创建笔记目录：{error}"))?;
        let canonical_parent = parent.canonicalize().map_err(|error| error.to_string())?;
        if !canonical_parent.starts_with(&vault) {
            return Err("笔记目录越出了 Obsidian 工作区".to_string());
        }
    }

    let backup = if existed {
        Some(obsidian::create_backup(&vault, &relative, &target)?)
    } else {
        None
    };
    obsidian::atomic_write(&target, &input.content)?;
    let metadata = fs::metadata(&target).map_err(|error| error.to_string())?;
    let relative_text = path_text(&relative);
    let now = db::now_iso();
    let existing_id: Option<String> = connection.query_row(
        "SELECT note_id FROM essay_notes WHERE relative_path=?1",
        [&relative_text],
        |row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    let note_id = existing_id.unwrap_or_else(db::new_id);
    let created = !existed;

    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    transaction.execute(
        "INSERT INTO essay_notes(note_id,relative_path,title,created_at_utc,updated_at_utc)
         VALUES(?1,?2,?3,?4,?4)
         ON CONFLICT(relative_path) DO UPDATE SET title=excluded.title,updated_at_utc=excluded.updated_at_utc,row_version=essay_notes.row_version+1",
        params![note_id, relative_text, title, now],
    ).map_err(|error| error.to_string())?;
    if created {
        db::append_event(
            &transaction, "essay.created", "essay", &note_id, &input.work_date, "summary",
            &format!("新建笔记《{title}》"), Some(&relative_text), &db::new_id(),
        )?;
    } else {
        db::append_event(
            &transaction, "essay.updated", "essay", &note_id, &input.work_date, "hidden",
            &format!("更新笔记《{title}》"), Some(&relative_text), &db::new_id(),
        )?;
    }
    transaction.commit().map_err(|error| error.to_string())?;

    Ok(SaveVaultNoteResult {
        note: VaultNote {
            relative_path: relative_text,
            title,
            content: input.content,
            modified_at: modified_text(&metadata),
            size_bytes: metadata.len(),
        },
        created,
        backup_path: backup.map(|path| path_text(&path)),
    })
}

#[tauri::command]
pub fn list_vault_notes(database: State<'_, Database>) -> Result<Vec<VaultNoteSummary>, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    list_core(&connection)
}

#[tauri::command(rename_all = "camelCase")]
pub fn read_vault_note(database: State<'_, Database>, relative_path: String) -> Result<VaultNote, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    read_core(&connection, &relative_path)
}

#[tauri::command]
pub fn save_vault_note(database: State<'_, Database>, input: SaveVaultNoteInput) -> Result<SaveVaultNoteResult, String> {
    let mut connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    save_core(&mut connection, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace() -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("worklog-notes-test-{stamp}-{}", db::new_id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn connection(vault: &Path) -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        obsidian::initialize(&connection).unwrap();
        crate::closing::initialize(&connection).unwrap();
        initialize(&connection).unwrap();
        let settings = obsidian::ObsidianSettings {
            vault_path: Some(vault.to_string_lossy().to_string()),
            daily_root: "工作日志".into(),
        };
        connection.execute(
            "INSERT INTO app_settings(key,value_json,updated_at_utc) VALUES('obsidian',?1,?2)",
            params![serde_json::to_string(&settings).unwrap(), db::now_iso()],
        ).unwrap();
        connection
    }

    #[test]
    fn unsafe_paths_and_windows_names_are_rejected() {
        assert!(safe_relative("../secret.md").is_err());
        assert!(markdown_relative("随笔/note.txt").is_err());
        assert!(validate_title("会议:纪要").is_err());
        assert!(validate_title("CON").is_err());
    }

    #[test]
    fn new_note_is_file_source_and_creates_one_visible_event() {
        let vault = workspace();
        let mut connection = connection(&vault);
        let result = save_core(&mut connection, SaveVaultNoteInput {
            work_date: "2026-09-02".into(), relative_path: None, directory: Some("随笔/2026/2026-09".into()),
            title: "会议纪要".into(), content: "# 会议纪要\n\n结论".into(),
        }).unwrap();
        assert!(result.created);
        assert_eq!(fs::read_to_string(vault.join("随笔/2026/2026-09/会议纪要.md")).unwrap(), "# 会议纪要\n\n结论");
        let event: (String, String) = connection.query_row(
            "SELECT default_visibility,payload_json FROM events WHERE event_type='essay.created'",
            [], |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(event.0, "summary");
        assert!(event.1.contains("新建笔记《会议纪要》"));
        fs::remove_dir_all(&vault).unwrap();
    }

    #[test]
    fn external_markdown_files_are_listed_and_read() {
        let vault = workspace();
        fs::create_dir_all(vault.join("项目")).unwrap();
        fs::write(vault.join("项目/计划.md"), "# 计划").unwrap();
        fs::create_dir_all(vault.join(".worklog-backups")).unwrap();
        fs::write(vault.join(".worklog-backups/旧.md"), "backup").unwrap();
        let connection = connection(&vault);
        let notes = list_core(&connection).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].relative_path, "项目/计划.md");
        assert_eq!(read_core(&connection, "项目/计划.md").unwrap().content, "# 计划");
        fs::remove_dir_all(&vault).unwrap();
    }
}
