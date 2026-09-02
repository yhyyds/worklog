use crate::model::{DayState, DayTask, FocusSession, RestSession, TimelineEvent};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn open_database(path: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    let connection = Connection::open(path)?;
    initialize(&connection)?;
    Ok(connection)
}

pub fn initialize(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(include_str!("../migrations/0001_initial.sql"))?;
    connection.execute_batch(include_str!("../migrations/0002_obsidian.sql"))?;
    connection.execute_batch(include_str!("../migrations/0003_day_closing.sql"))?;
    connection.execute_batch(include_str!("../migrations/0004_essay_notes.sql"))?;
    connection.execute_batch(include_str!("../migrations/0005_focus_lifecycle.sql"))?;
    Ok(())
}

pub fn minute_value(value: &Option<String>) -> Result<Option<i64>, String> {
    value.as_ref().map(|time| {
        let mut parts = time.split(':');
        let hour: i64 = parts.next().ok_or("无效时间")?.parse().map_err(|_| "无效小时")?;
        let minute: i64 = parts.next().ok_or("无效时间")?.parse().map_err(|_| "无效分钟")?;
        if parts.next().is_some() || !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
            return Err("时间必须为 HH:MM".to_string());
        }
        Ok(hour * 60 + minute)
    }).transpose()
}

fn minute_text(value: Option<i64>) -> Option<String> {
    value.map(|minute| format!("{:02}:{:02}", minute / 60, minute % 60))
}

pub fn remaining_from_target(status: &str, stored: i64, target: &Option<String>) -> i64 {
    if status != "running" { return stored; }
    target.as_ref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|end| (end.timestamp() - Utc::now().timestamp()).max(0))
        .unwrap_or(stored)
}

pub fn append_event(
    transaction: &Transaction<'_>, event_type: &str, aggregate_type: &str,
    aggregate_id: &str, work_date: &str, visibility: &str, title: &str,
    detail: Option<&str>, transaction_id: &str,
) -> Result<(), String> {
    let payload = json!({ "title": title, "detail": detail });
    transaction.execute(
        "INSERT INTO events(event_id,event_type,aggregate_type,aggregate_id,work_date,occurred_at_utc,actor_type,transaction_id,default_visibility,payload_json) VALUES(?1,?2,?3,?4,?5,?6,'user',?7,?8,?9)",
        params![new_id(), event_type, aggregate_type, aggregate_id, work_date, now_iso(), transaction_id, visibility, payload.to_string()],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn read_day(connection: &Connection, work_date: &str) -> Result<DayState, String> {
    let mut task_statement = connection.prepare(
        "SELECT i.id,t.id,i.parent_instance_id,i.display_code,t.title,i.day_status,i.importance,i.urgency,i.planned_start_minute,i.planned_end_minute,t.created_at_utc FROM task_day_instances i JOIN tasks t ON t.id=i.task_id WHERE i.work_date=?1 ORDER BY i.top_level_no,COALESCE(i.child_no,0)"
    ).map_err(|error| error.to_string())?;
    let tasks = task_statement.query_map([work_date], |row| Ok(DayTask {
        id: row.get(0)?, permanent_task_id: row.get(1)?, parent_id: row.get(2)?,
        display_code: row.get(3)?, title: row.get(4)?, status: row.get(5)?,
        importance: row.get(6)?, urgency: row.get(7)?,
        planned_start: minute_text(row.get(8)?), planned_end: minute_text(row.get(9)?),
        created_at: row.get(10)?,
    })).map_err(|error| error.to_string())?
      .collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;

    let mut event_statement = connection.prepare(
        "SELECT event_id,event_type,occurred_at_utc,default_visibility,payload_json FROM events WHERE work_date=?1 ORDER BY seq"
    ).map_err(|error| error.to_string())?;
    let timeline = event_statement.query_map([work_date], |row| {
        let payload_text: String = row.get(4)?;
        let payload: Value = serde_json::from_str(&payload_text).unwrap_or_else(|_| json!({}));
        Ok(TimelineEvent {
            id: row.get(0)?, event_type: row.get(1)?, occurred_at: row.get(2)?,
            visibility: row.get(3)?,
            title: payload.get("title").and_then(Value::as_str).unwrap_or("记录").to_string(),
            detail: payload.get("detail").and_then(Value::as_str).map(str::to_string),
        })
    }).map_err(|error| error.to_string())?
      .collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;

    let focus = connection.query_row(
        "SELECT id,primary_task_instance_id,status,planned_seconds,remaining_seconds,target_end_at_utc,started_at_utc FROM focus_sessions WHERE active_guard=1 AND work_date=?1 LIMIT 1",
        [work_date], |row| {
            let status: String = row.get(2)?;
            let stored: i64 = row.get(4)?;
            let target: Option<String> = row.get(5)?;
            Ok(FocusSession { id: row.get(0)?, task_id: row.get(1)?, status: status.clone(), planned_seconds: row.get(3)?, remaining_seconds: remaining_from_target(&status, stored, &target), target_end_at: target, started_at: row.get(6)? })
        }
    ).optional().map_err(|error| error.to_string())?;

    let rest = connection.query_row(
        "SELECT id,rest_kind,status,planned_seconds,remaining_seconds,target_end_at_utc,started_at_utc
         FROM rest_sessions WHERE active_guard=1 AND work_date=?1 LIMIT 1",
        [work_date], |row| {
            let status: String = row.get(2)?;
            let stored: i64 = row.get(4)?;
            let target: Option<String> = row.get(5)?;
            Ok(RestSession {
                id: row.get(0)?, rest_kind: row.get(1)?, status: status.clone(),
                planned_seconds: row.get(3)?,
                remaining_seconds: remaining_from_target(&status, stored, &target),
                target_end_at: target, started_at: row.get(6)?,
            })
        }
    ).optional().map_err(|error| error.to_string())?;

    Ok(DayState { work_date: work_date.to_string(), tasks, timeline, focus, rest })
}
