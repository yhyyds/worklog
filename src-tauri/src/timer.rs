use crate::{commands, db, model::{DayState, FocusActionInput}, Database};
use chrono::{Duration, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;

const SETTINGS_KEY: &str = "focus_timer";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerSettings {
    pub work_minutes: i64,
    pub short_break_minutes: i64,
    pub long_break_minutes: i64,
    pub long_break_interval: i64,
    pub auto_start_break: bool,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            work_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            long_break_interval: 4,
            auto_start_break: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

pub fn initialize(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(include_str!("../migrations/0005_focus_lifecycle.sql"))
}

fn load_settings_from(connection: &Connection) -> Result<TimerSettings, String> {
    let value: Option<String> = connection.query_row(
        "SELECT value_json FROM app_settings WHERE key=?1",
        [SETTINGS_KEY],
        |row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    value.map(|json| serde_json::from_str(&json).map_err(|error| format!("番茄钟设置损坏：{error}")))
        .transpose()
        .map(|settings| settings.unwrap_or_default())
}

fn load_settings_tx(transaction: &Transaction<'_>) -> Result<TimerSettings, String> {
    let value: Option<String> = transaction.query_row(
        "SELECT value_json FROM app_settings WHERE key=?1",
        [SETTINGS_KEY],
        |row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    value.map(|json| serde_json::from_str(&json).map_err(|error| format!("番茄钟设置损坏：{error}")))
        .transpose()
        .map(|settings| settings.unwrap_or_default())
}

fn validate_settings(settings: &TimerSettings) -> Result<(), String> {
    if !(1..=180).contains(&settings.work_minutes) {
        return Err("专注时长必须在 1–180 分钟之间".to_string());
    }
    if !(1..=60).contains(&settings.short_break_minutes) || !(1..=120).contains(&settings.long_break_minutes) {
        return Err("休息时长超出允许范围".to_string());
    }
    if !(1..=12).contains(&settings.long_break_interval) {
        return Err("长休息间隔必须在 1–12 轮之间".to_string());
    }
    Ok(())
}

pub(crate) fn begin_rest_in_transaction(
    transaction: &Transaction<'_>,
    work_date: &str,
    focus_session_id: &str,
) -> Result<(), String> {
    let settings = load_settings_tx(transaction)?;
    if !settings.auto_start_break {
        return Ok(());
    }
    let completed: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM focus_sessions WHERE work_date=?1 AND status='completed'",
        [work_date],
        |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    let long = completed > 0 && completed % settings.long_break_interval == 0;
    let kind = if long { "long" } else { "short" };
    let minutes = if long { settings.long_break_minutes } else { settings.short_break_minutes };
    let seconds = minutes * 60;
    let now = db::now_iso();
    let target = (Utc::now() + Duration::seconds(seconds)).to_rfc3339_opts(SecondsFormat::Millis, true);
    let rest_id = db::new_id();
    transaction.execute(
        "INSERT INTO rest_sessions(id,work_date,focus_session_id,rest_kind,status,planned_seconds,remaining_seconds,target_end_at_utc,started_at_utc,active_guard)
         VALUES(?1,?2,?3,?4,'running',?5,?5,?6,?7,1)",
        params![rest_id, work_date, focus_session_id, kind, seconds, target, now],
    ).map_err(|error| error.to_string())?;
    db::append_event(
        transaction, "rest.started", "rest", &rest_id, work_date, "hidden",
        &format!("开始{}休息", if long { "长" } else { "短" }),
        Some(&format!("{minutes}分钟")), &db::new_id(),
    )
}

pub(crate) fn pause_rest_core(connection: &mut Connection, work_date: &str) -> Result<DayState, String> {
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let (id, status, stored, target): (String, String, i64, Option<String>) = transaction.query_row(
        "SELECT id,status,remaining_seconds,target_end_at_utc FROM rest_sessions WHERE active_guard=1 AND work_date=?1",
        [work_date], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).map_err(|_| "当前没有可暂停的休息".to_string())?;
    if status != "running" {
        return Err("当前休息不是运行状态".to_string());
    }
    let remaining = db::remaining_from_target(&status, stored, &target);
    transaction.execute(
        "UPDATE rest_sessions SET status='paused',remaining_seconds=?1,target_end_at_utc=NULL WHERE id=?2",
        params![remaining, id],
    ).map_err(|error| error.to_string())?;
    db::append_event(&transaction, "rest.paused", "rest", &id, work_date, "hidden", "暂停休息", None, &db::new_id())?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, work_date)
}

pub(crate) fn resume_rest_core(connection: &mut Connection, work_date: &str) -> Result<DayState, String> {
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let (id, status, remaining): (String, String, i64) = transaction.query_row(
        "SELECT id,status,remaining_seconds FROM rest_sessions WHERE active_guard=1 AND work_date=?1",
        [work_date], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|_| "当前没有已暂停的休息".to_string())?;
    if status != "paused" {
        return Err("当前休息不是暂停状态".to_string());
    }
    let target = (Utc::now() + Duration::seconds(remaining)).to_rfc3339_opts(SecondsFormat::Millis, true);
    transaction.execute(
        "UPDATE rest_sessions SET status='running',target_end_at_utc=?1 WHERE id=?2",
        params![target, id],
    ).map_err(|error| error.to_string())?;
    db::append_event(&transaction, "rest.resumed", "rest", &id, work_date, "hidden", "继续休息", None, &db::new_id())?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, work_date)
}

pub(crate) fn finish_rest_core(connection: &mut Connection, work_date: &str, skipped: bool) -> Result<DayState, String> {
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let id: String = transaction.query_row(
        "SELECT id FROM rest_sessions WHERE active_guard=1 AND work_date=?1",
        [work_date], |row| row.get(0),
    ).map_err(|_| "当前没有进行中的休息".to_string())?;
    let status = if skipped { "skipped" } else { "completed" };
    let now = db::now_iso();
    transaction.execute(
        "UPDATE rest_sessions SET status=?1,remaining_seconds=0,target_end_at_utc=NULL,ended_at_utc=?2,active_guard=NULL WHERE id=?3",
        params![status, now, id],
    ).map_err(|error| error.to_string())?;
    db::append_event(
        &transaction, if skipped { "rest.skipped" } else { "rest.completed" }, "rest", &id,
        work_date, "hidden", if skipped { "跳过休息" } else { "休息结束" }, None, &db::new_id(),
    )?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, work_date)
}

pub fn focus_completion_notices(day: &DayState, reason: &str, task_text: &str) -> Vec<Notice> {
    if reason == "abandoned" {
        return Vec::new();
    }
    let mut notices = vec![Notice {
        title: "专注结束".to_string(),
        body: task_text.to_string(),
    }];
    if let Some(rest) = &day.rest {
        notices.push(Notice {
            title: "休息开始".to_string(),
            body: format!("{}休息 · {}分钟", if rest.rest_kind == "long" { "长" } else { "短" }, rest.planned_seconds / 60),
        });
    }
    notices
}

pub fn show_notices(app: &AppHandle, notices: &[Notice]) {
    for notice in notices {
        let _ = app.notification().builder().title(&notice.title).body(&notice.body).show();
    }
}

pub fn advance_expired(connection: &mut Connection) -> Result<Option<Vec<Notice>>, String> {
    let now = db::now_iso();
    let focus: Option<(String, String)> = connection.query_row(
        "SELECT fs.work_date,i.display_code || ' ' || t.title
         FROM focus_sessions fs
         JOIN task_day_instances i ON i.id=fs.primary_task_instance_id
         JOIN tasks t ON t.id=i.task_id
         WHERE fs.active_guard=1 AND fs.status='running' AND fs.target_end_at_utc<=?1 LIMIT 1",
        [&now], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(|error| error.to_string())?;
    if let Some((work_date, task_text)) = focus {
        let day = commands::complete_focus_core(connection, crate::model::CompleteFocusInput {
            work_date, reason: "elapsed".to_string(),
        })?;
        return Ok(Some(focus_completion_notices(&day, "elapsed", &task_text)));
    }

    let rest: Option<String> = connection.query_row(
        "SELECT work_date FROM rest_sessions WHERE active_guard=1 AND status='running' AND target_end_at_utc<=?1 LIMIT 1",
        [&now], |row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    if let Some(work_date) = rest {
        finish_rest_core(connection, &work_date, false)?;
        return Ok(Some(vec![Notice {
            title: "休息结束".to_string(),
            body: "选择下一项任务，准备开始新一轮工作。".to_string(),
        }]));
    }
    Ok(None)
}

pub fn start_background(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let transition = {
                let database = app.state::<Database>();
                let result = match database.0.lock() {
                    Ok(mut connection) => advance_expired(&mut connection).ok().flatten(),
                    Err(_) => None,
                };
                result
            };
            if let Some(notices) = transition {
                show_notices(&app, &notices);
                let _ = app.emit("worklog-timer-changed", ());
            }
        }
    });
}

#[tauri::command]
pub fn get_timer_settings(database: State<'_, Database>) -> Result<TimerSettings, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    load_settings_from(&connection)
}

#[tauri::command]
pub fn save_timer_settings(database: State<'_, Database>, settings: TimerSettings) -> Result<TimerSettings, String> {
    validate_settings(&settings)?;
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    let json = serde_json::to_string(&settings).map_err(|error| error.to_string())?;
    connection.execute(
        "INSERT INTO app_settings(key,value_json,updated_at_utc) VALUES(?1,?2,?3)
         ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at_utc=excluded.updated_at_utc",
        params![SETTINGS_KEY, json, db::now_iso()],
    ).map_err(|error| error.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn pause_rest(database: State<'_, Database>, input: FocusActionInput) -> Result<DayState, String> {
    let mut connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    pause_rest_core(&mut connection, &input.work_date)
}

#[tauri::command]
pub fn resume_rest(database: State<'_, Database>, input: FocusActionInput) -> Result<DayState, String> {
    let mut connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    resume_rest_core(&mut connection, &input.work_date)
}

#[tauri::command]
pub fn complete_rest(app: AppHandle, database: State<'_, Database>, input: FocusActionInput) -> Result<DayState, String> {
    let mut connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    let day = finish_rest_core(&mut connection, &input.work_date, false)?;
    drop(connection);
    show_notices(&app, &[Notice {
        title: "休息结束".to_string(),
        body: "选择下一项任务，准备开始新一轮工作。".to_string(),
    }]);
    let _ = app.emit("worklog-timer-changed", ());
    Ok(day)
}

#[tauri::command]
pub fn skip_rest(app: AppHandle, database: State<'_, Database>, input: FocusActionInput) -> Result<DayState, String> {
    let mut connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    let day = finish_rest_core(&mut connection, &input.work_date, true)?;
    let _ = app.emit("worklog-timer-changed", ());
    Ok(day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CreateTaskInput, StartFocusInput, CompleteFocusInput};

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        crate::obsidian::initialize(&connection).unwrap();
        crate::closing::initialize(&connection).unwrap();
        crate::notes::initialize(&connection).unwrap();
        initialize(&connection).unwrap();
        connection
    }

    fn task(connection: &mut Connection) -> String {
        commands::create_task_core(connection, CreateTaskInput {
            work_date: "2026-09-02".into(), title: "整理资料".into(),
            importance: "important".into(), urgency: "urgent".into(),
            parent_id: None, planned_start: None, planned_end: None,
        }).unwrap().tasks[0].id.clone()
    }

    fn complete_one(connection: &mut Connection, task_id: &str) -> DayState {
        commands::start_focus_core(connection, StartFocusInput {
            work_date: "2026-09-02".into(), task_id: task_id.into(), planned_seconds: 1500,
        }).unwrap();
        commands::complete_focus_core(connection, CompleteFocusInput {
            work_date: "2026-09-02".into(), reason: "early_complete".into(),
        }).unwrap()
    }

    #[test]
    fn completed_focus_starts_short_rest() {
        let mut connection = connection();
        let task_id = task(&mut connection);
        let day = complete_one(&mut connection, &task_id);
        let rest = day.rest.unwrap();
        assert_eq!(rest.rest_kind, "short");
        assert_eq!(rest.planned_seconds, 300);
    }

    #[test]
    fn fourth_completed_focus_starts_long_rest() {
        let mut connection = connection();
        let task_id = task(&mut connection);
        let mut last = None;
        for _ in 0..4 {
            last = Some(complete_one(&mut connection, &task_id));
            finish_rest_core(&mut connection, "2026-09-02", true).unwrap();
        }
        assert_eq!(last.unwrap().rest.unwrap().rest_kind, "long");
    }

    #[test]
    fn rest_pause_resume_and_skip_are_persisted() {
        let mut connection = connection();
        let task_id = task(&mut connection);
        complete_one(&mut connection, &task_id);
        let paused = pause_rest_core(&mut connection, "2026-09-02").unwrap();
        assert_eq!(paused.rest.as_ref().unwrap().status, "paused");
        let resumed = resume_rest_core(&mut connection, "2026-09-02").unwrap();
        assert_eq!(resumed.rest.as_ref().unwrap().status, "running");
        let skipped = finish_rest_core(&mut connection, "2026-09-02", true).unwrap();
        assert!(skipped.rest.is_none());
    }
}
