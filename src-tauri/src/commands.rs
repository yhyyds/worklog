use crate::{db, model::*, Database};
use chrono::{Duration, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use tauri::{AppHandle, Emitter, State};

fn validate_choice(value: &str, allowed: &[&str], field: &str) -> Result<(), String> {
    if allowed.contains(&value) { Ok(()) } else { Err(format!("{field} 值无效")) }
}

fn task_info(transaction: &Transaction<'_>, instance_id: &str) -> Result<(String, String, String), String> {
    transaction.query_row(
        "SELECT t.id,i.display_code,t.title FROM task_day_instances i JOIN tasks t ON t.id=i.task_id WHERE i.id=?1",
        [instance_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|_| "任务不存在或不在今天".to_string())
}

pub(crate) fn create_task_core(connection: &mut Connection, input: CreateTaskInput) -> Result<DayState, String> {
    let title = input.title.trim();
    if title.is_empty() { return Err("任务内容不能为空".into()); }
    validate_choice(&input.importance, &["important", "secondary"], "重要性")?;
    validate_choice(&input.urgency, &["urgent", "relaxed"], "紧急性")?;
    let start = db::minute_value(&input.planned_start)?;
    let end = db::minute_value(&input.planned_end)?;
    if start.is_some() != end.is_some() || start.zip(end).is_some_and(|(a, b)| a >= b) {
        return Err("任务时间段无效".into());
    }

    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let (parent_task_id, top_level_no, child_no, display_code, importance, urgency) = if let Some(parent_id) = &input.parent_id {
        let parent = transaction.query_row(
            "SELECT t.id,i.top_level_no,i.display_code,i.importance,i.urgency,i.parent_instance_id FROM task_day_instances i JOIN tasks t ON t.id=i.task_id WHERE i.id=?1 AND i.work_date=?2",
            params![parent_id, input.work_date],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, Option<String>>(5)?)),
        ).map_err(|_| "父任务不存在或不在同一天".to_string())?;
        if parent.5.is_some() { return Err("任务最多只允许两级".into()); }
        let next_child: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(child_no),0)+1 FROM task_day_instances WHERE parent_instance_id=?1",
            [parent_id], |row| row.get(0),
        ).map_err(|error| error.to_string())?;
        (Some(parent.0), parent.1, Some(next_child), format!("{}.{}", parent.2, next_child), parent.3, parent.4)
    } else {
        let next_top: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(top_level_no),0)+1 FROM task_day_instances WHERE work_date=?1 AND parent_instance_id IS NULL",
            [&input.work_date], |row| row.get(0),
        ).map_err(|error| error.to_string())?;
        (None, next_top, None, format!("#{next_top}"), input.importance.clone(), input.urgency.clone())
    };

    let task_id = db::new_id();
    let instance_id = db::new_id();
    let transaction_id = db::new_id();
    let now = db::now_iso();
    transaction.execute(
        "INSERT INTO tasks(id,parent_task_id,title,status,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,'not_started',?4,?4)",
        params![task_id, parent_task_id, title, now],
    ).map_err(|error| error.to_string())?;
    transaction.execute(
        "INSERT INTO task_day_instances(id,task_id,work_date,parent_instance_id,display_code,top_level_no,child_no,importance,urgency,day_status,planned_start_minute,planned_end_minute,sort_order,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'not_started',?10,?11,?12,?13,?13)",
        params![instance_id, task_id, input.work_date, input.parent_id, display_code, top_level_no, child_no, importance, urgency, start, end, top_level_no * 1000 + child_no.unwrap_or(0), now],
    ).map_err(|error| error.to_string())?;
    db::append_event(&transaction, "task.created", "task", &task_id, &input.work_date, "summary", &format!("新增任务{display_code}：{title}"), None, &transaction_id)?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, &input.work_date)
}

pub(crate) fn update_task_core(connection: &mut Connection, input: UpdateTaskInput) -> Result<DayState, String> {
    let title = input.title.trim();
    if title.is_empty() { return Err("任务内容不能为空".into()); }
    let start = db::minute_value(&input.planned_start)?;
    let end = db::minute_value(&input.planned_end)?;
    if start.is_some() != end.is_some() || start.zip(end).is_some_and(|(a, b)| a >= b) {
        return Err("任务时间段无效".into());
    }

    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let (task_id, display_code, previous_title): (String, String, String) = transaction.query_row(
        "SELECT t.id,i.display_code,t.title FROM task_day_instances i
         JOIN tasks t ON t.id=i.task_id WHERE i.id=?1 AND i.work_date=?2",
        params![input.instance_id, input.work_date],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|_| "任务不存在或不在今天".to_string())?;
    let now = db::now_iso();
    transaction.execute(
        "UPDATE tasks SET title=?1,updated_at_utc=?2,row_version=row_version+1 WHERE id=?3",
        params![title, now, task_id],
    ).map_err(|error| error.to_string())?;
    transaction.execute(
        "UPDATE task_day_instances SET planned_start_minute=?1,planned_end_minute=?2,updated_at_utc=?3 WHERE id=?4",
        params![start, end, now, input.instance_id],
    ).map_err(|error| error.to_string())?;
    let schedule = match (&input.planned_start, &input.planned_end) {
        (Some(start), Some(end)) => format!("安排时间：{start}–{end}"),
        _ => "未安排固定时间".to_string(),
    };
    let detail = if previous_title == title {
        schedule
    } else {
        format!("原内容：{previous_title}；{schedule}")
    };
    db::append_event(
        &transaction, "task.updated", "task", &task_id, &input.work_date, "detail",
        &format!("更新任务{display_code}：{title}"), Some(&detail), &db::new_id(),
    )?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, &input.work_date)
}

pub(crate) fn set_task_status_core(connection: &mut Connection, input: SetTaskStatusInput) -> Result<DayState, String> {
    validate_choice(&input.status, &["not_started", "in_progress", "waiting", "blocked", "completed", "deferred", "cancelled"], "任务状态")?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let (task_id, display_code, title) = task_info(&transaction, &input.instance_id)?;
    let now = db::now_iso();
    let completed_at = if input.status == "completed" { Some(now.clone()) } else { None };
    transaction.execute("UPDATE tasks SET status=?1,completed_at_utc=?2,updated_at_utc=?3,row_version=row_version+1 WHERE id=?4", params![input.status, completed_at, now, task_id]).map_err(|error| error.to_string())?;
    transaction.execute("UPDATE task_day_instances SET day_status=?1,updated_at_utc=?2 WHERE id=?3", params![input.status, now, input.instance_id]).map_err(|error| error.to_string())?;
    let verb = match input.status.as_str() { "completed" => "完成", "not_started" => "恢复", "in_progress" => "开始", "waiting" => "等待", "blocked" => "阻塞", "deferred" => "延期", _ => "取消" };
    db::append_event(&transaction, &format!("task.{}", input.status), "task", &task_id, &input.work_date, "summary", &format!("{verb}{display_code}：{title}"), None, &db::new_id())?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, &input.work_date)
}

pub(crate) fn add_work_entry_core(connection: &mut Connection, input: WorkEntryInput) -> Result<DayState, String> {
    let content = input.content.trim();
    if content.is_empty() { return Err("工作想法不能为空".into()); }
    validate_choice(&input.entry_type, &["progress", "idea", "decision", "blocker", "result"], "记录类型")?;
    validate_choice(&input.review_level, &["key", "normal", "scratch"], "回顾等级")?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let task = input.task_id.as_ref().map(|id| task_info(&transaction, id)).transpose()?;
    let active_focus: Option<String> = transaction.query_row("SELECT id FROM focus_sessions WHERE active_guard=1 LIMIT 1", [], |row| row.get(0)).optional().map_err(|error| error.to_string())?;
    let entry_id = db::new_id();
    let now = db::now_iso();
    transaction.execute(
        "INSERT INTO work_entries(id,work_date,focus_session_id,task_instance_id,entry_type,review_level,content_md,occurred_at_utc,created_at_utc) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",
        params![entry_id, input.work_date, active_focus, input.task_id, input.entry_type, input.review_level, content, now],
    ).map_err(|error| error.to_string())?;
    let title = task.as_ref().map(|(_, code, _)| format!("{code} · {content}")).unwrap_or_else(|| content.to_string());
    let visibility = match input.review_level.as_str() { "key" => "summary", "scratch" => "hidden", _ => "detail" };
    let detail = format!("{} · {}", input.entry_type, input.review_level);
    db::append_event(&transaction, "work_entry.created", "work_entry", &entry_id, &input.work_date, visibility, &title, Some(&detail), &db::new_id())?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, &input.work_date)
}

pub(crate) fn start_focus_core(connection: &mut Connection, input: StartFocusInput) -> Result<DayState, String> {
    if input.planned_seconds <= 0 { return Err("专注时长必须大于零".into()); }
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let active: i64 = transaction.query_row("SELECT COUNT(*) FROM focus_sessions WHERE active_guard=1", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    if active > 0 { return Err("已有正在进行的专注".into()); }
    let active_rest: i64 = transaction.query_row("SELECT COUNT(*) FROM rest_sessions WHERE active_guard=1", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    if active_rest > 0 { return Err("请先完成或跳过当前休息".into()); }
    let parent_instance_id: Option<String> = transaction.query_row(
        "SELECT parent_instance_id FROM task_day_instances WHERE id=?1 AND work_date=?2",
        params![input.task_id, input.work_date], |row| row.get(0),
    ).map_err(|_| "任务不存在或不在今天".to_string())?;
    if parent_instance_id.is_some() { return Err("专注事项只能选择一级任务".into()); }
    let (task_id, display_code, title) = task_info(&transaction, &input.task_id)?;
    let session_id = db::new_id();
    let segment_id = db::new_id();
    let now = db::now_iso();
    let target = (Utc::now() + Duration::seconds(input.planned_seconds)).to_rfc3339_opts(SecondsFormat::Millis, true);
    transaction.execute(
        "INSERT INTO focus_sessions(id,work_date,status,primary_task_instance_id,planned_seconds,remaining_seconds,target_end_at_utc,started_at_utc,active_guard) VALUES(?1,?2,'running',?3,?4,?4,?5,?6,1)",
        params![session_id, input.work_date, input.task_id, input.planned_seconds, target, now],
    ).map_err(|error| error.to_string())?;
    transaction.execute("INSERT INTO focus_segments(id,focus_session_id,task_instance_id,started_at_utc) VALUES(?1,?2,?3,?4)", params![segment_id, session_id, input.task_id, now]).map_err(|error| error.to_string())?;
    transaction.execute("UPDATE task_day_instances SET day_status='in_progress',updated_at_utc=?1 WHERE id=?2 AND day_status='not_started'", params![now, input.task_id]).map_err(|error| error.to_string())?;
    transaction.execute("UPDATE tasks SET status='in_progress',updated_at_utc=?1,row_version=row_version+1 WHERE id=?2 AND status='not_started'", params![now, task_id]).map_err(|error| error.to_string())?;
    db::append_event(&transaction, "focus.started", "focus", &session_id, &input.work_date, "summary", &format!("开始一轮工作，任务内容：{display_code} {title}"), None, &db::new_id())?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, &input.work_date)
}

pub(crate) fn pause_focus_core(connection: &mut Connection, input: PauseFocusInput) -> Result<DayState, String> {
    let reason = input.reason.trim();
    if reason.is_empty() { return Err("请填写暂停原因".into()); }
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let (session_id, status, stored, target): (String, String, i64, Option<String>) = transaction.query_row(
        "SELECT id,status,remaining_seconds,target_end_at_utc FROM focus_sessions WHERE active_guard=1 AND work_date=?1", [&input.work_date],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).map_err(|_| "当前没有进行中的专注".to_string())?;
    if status != "running" { return Err("当前专注不是运行状态".into()); }
    let remaining = db::remaining_from_target(&status, stored, &target);
    transaction.execute("UPDATE focus_sessions SET status='paused',remaining_seconds=?1,target_end_at_utc=NULL WHERE id=?2", params![remaining, session_id]).map_err(|error| error.to_string())?;
    db::append_event(
        &transaction, "focus.paused", "focus", &session_id, &input.work_date, "detail",
        &format!("暂停本轮工作：{reason}"), Some(&format!("剩余{}分钟", (remaining + 59) / 60)), &db::new_id(),
    )?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, &input.work_date)
}

pub(crate) fn resume_focus_core(connection: &mut Connection, input: FocusActionInput) -> Result<DayState, String> {
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let (session_id, status, remaining): (String, String, i64) = transaction.query_row(
        "SELECT id,status,remaining_seconds FROM focus_sessions WHERE active_guard=1 AND work_date=?1", [&input.work_date],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|_| "当前没有暂停的专注".to_string())?;
    if status != "paused" { return Err("当前专注不是暂停状态".into()); }
    let target = (Utc::now() + Duration::seconds(remaining)).to_rfc3339_opts(SecondsFormat::Millis, true);
    transaction.execute("UPDATE focus_sessions SET status='running',target_end_at_utc=?1 WHERE id=?2", params![target, session_id]).map_err(|error| error.to_string())?;
    db::append_event(&transaction, "focus.resumed", "focus", &session_id, &input.work_date, "detail", "继续本轮工作", None, &db::new_id())?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, &input.work_date)
}

pub(crate) fn switch_focus_core(connection: &mut Connection, input: SwitchFocusInput) -> Result<DayState, String> {
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let (session_id, old_instance): (String, String) = transaction.query_row(
        "SELECT id,primary_task_instance_id FROM focus_sessions WHERE active_guard=1 AND work_date=?1", [&input.work_date],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|_| "当前没有进行中的专注".to_string())?;
    if old_instance == input.task_id { return db::read_day(&transaction, &input.work_date); }
    let (_, old_code, old_title) = task_info(&transaction, &old_instance)?;
    let new_parent: Option<String> = transaction.query_row(
        "SELECT parent_instance_id FROM task_day_instances WHERE id=?1 AND work_date=?2",
        params![input.task_id, input.work_date], |row| row.get(0),
    ).map_err(|_| "任务不存在或不在今天".to_string())?;
    if new_parent.is_some() { return Err("专注事项只能选择一级任务".into()); }
    let (_, new_code, new_title) = task_info(&transaction, &input.task_id)?;
    let now = db::now_iso();
    transaction.execute("UPDATE focus_segments SET ended_at_utc=?1 WHERE focus_session_id=?2 AND ended_at_utc IS NULL", params![now, session_id]).map_err(|error| error.to_string())?;
    transaction.execute("INSERT INTO focus_segments(id,focus_session_id,task_instance_id,started_at_utc) VALUES(?1,?2,?3,?4)", params![db::new_id(), session_id, input.task_id, now]).map_err(|error| error.to_string())?;
    transaction.execute("UPDATE focus_sessions SET primary_task_instance_id=?1 WHERE id=?2", params![input.task_id, session_id]).map_err(|error| error.to_string())?;
    db::append_event(&transaction, "focus.task_switched", "focus", &session_id, &input.work_date, "summary", &format!("本轮工作由{old_code} {old_title}切换至{new_code} {new_title}"), None, &db::new_id())?;
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, &input.work_date)
}

pub(crate) fn complete_focus_core(connection: &mut Connection, input: CompleteFocusInput) -> Result<DayState, String> {
    validate_choice(&input.reason, &["elapsed", "early_complete", "abandoned"], "结束原因")?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let (session_id, status, planned, stored, target): (String, String, i64, i64, Option<String>) = transaction.query_row(
        "SELECT id,status,planned_seconds,remaining_seconds,target_end_at_utc FROM focus_sessions WHERE active_guard=1 AND work_date=?1", [&input.work_date],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).map_err(|_| "当前没有进行中的专注".to_string())?;
    let remaining = db::remaining_from_target(&status, stored, &target);
    let actual = (planned - remaining).max(0);
    let final_status = if input.reason == "abandoned" { "abandoned" } else { "completed" };
    let now = db::now_iso();
    transaction.execute("UPDATE focus_sessions SET status=?1,remaining_seconds=?2,target_end_at_utc=NULL,ended_at_utc=?3,active_guard=NULL WHERE id=?4", params![final_status, remaining, now, session_id]).map_err(|error| error.to_string())?;
    transaction.execute("UPDATE focus_segments SET ended_at_utc=?1,allocated_seconds=?2 WHERE focus_session_id=?3 AND ended_at_utc IS NULL", params![now, actual, session_id]).map_err(|error| error.to_string())?;
    let minutes = ((actual + 30) / 60).max(1);
    let title = if input.reason == "abandoned" { format!("放弃本轮工作，已进行{minutes}分钟") } else { format!("完成一轮工作，共{minutes}分钟") };
    db::append_event(&transaction, &format!("focus.{}", input.reason), "focus", &session_id, &input.work_date, "summary", &title, None, &db::new_id())?;
    if input.reason != "abandoned" {
        crate::timer::begin_rest_in_transaction(&transaction, &input.work_date, &session_id)?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, &input.work_date)
}

fn with_database<T>(database: State<'_, Database>, action: impl FnOnce(&mut Connection) -> Result<T, String>) -> Result<T, String> {
    let mut connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    action(&mut connection)
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_day_snapshot(database: State<'_, Database>, work_date: String) -> Result<DayState, String> { with_database(database, |connection| db::read_day(connection, &work_date)) }
#[tauri::command]
pub fn create_task(database: State<'_, Database>, input: CreateTaskInput) -> Result<DayState, String> { with_database(database, |connection| create_task_core(connection, input)) }
#[tauri::command]
pub fn update_task(database: State<'_, Database>, input: UpdateTaskInput) -> Result<DayState, String> { with_database(database, |connection| update_task_core(connection, input)) }
#[tauri::command]
pub fn set_task_status(database: State<'_, Database>, input: SetTaskStatusInput) -> Result<DayState, String> { with_database(database, |connection| set_task_status_core(connection, input)) }
#[tauri::command]
pub fn add_work_entry(database: State<'_, Database>, input: WorkEntryInput) -> Result<DayState, String> { with_database(database, |connection| add_work_entry_core(connection, input)) }
#[tauri::command]
pub fn start_focus(database: State<'_, Database>, input: StartFocusInput) -> Result<DayState, String> { with_database(database, |connection| start_focus_core(connection, input)) }
#[tauri::command]
pub fn pause_focus(database: State<'_, Database>, input: PauseFocusInput) -> Result<DayState, String> { with_database(database, |connection| pause_focus_core(connection, input)) }
#[tauri::command]
pub fn resume_focus(database: State<'_, Database>, input: FocusActionInput) -> Result<DayState, String> { with_database(database, |connection| resume_focus_core(connection, input)) }
#[tauri::command]
pub fn switch_focus(database: State<'_, Database>, input: SwitchFocusInput) -> Result<DayState, String> { with_database(database, |connection| switch_focus_core(connection, input)) }
#[tauri::command]
pub fn complete_focus(app: AppHandle, database: State<'_, Database>, input: CompleteFocusInput) -> Result<DayState, String> {
    let mut connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    let task_text: String = connection.query_row(
        "SELECT i.display_code || ' ' || t.title FROM focus_sessions fs
         JOIN task_day_instances i ON i.id=fs.primary_task_instance_id
         JOIN tasks t ON t.id=i.task_id WHERE fs.active_guard=1 AND fs.work_date=?1",
        [&input.work_date], |row| row.get(0),
    ).map_err(|_| "当前没有进行中的专注".to_string())?;
    let reason = input.reason.clone();
    let day = complete_focus_core(&mut connection, input)?;
    drop(connection);
    let notices = crate::timer::focus_completion_notices(&day, &reason, &task_text);
    crate::timer::show_notices(&app, &notices);
    let _ = app.emit("worklog-timer-changed", ());
    Ok(day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        connection
    }

    fn task_input(title: &str, parent_id: Option<String>) -> CreateTaskInput {
        CreateTaskInput { work_date: "2026-09-02".into(), title: title.into(), importance: "important".into(), urgency: "urgent".into(), parent_id, planned_start: None, planned_end: None }
    }

    #[test]
    fn task_and_event_are_committed_together() {
        let mut connection = connection();
        let day = create_task_core(&mut connection, task_input("整理资料", None)).unwrap();
        assert_eq!(day.tasks[0].display_code, "#1");
        assert_eq!(day.timeline[0].event_type, "task.created");
    }

    #[test]
    fn child_numbering_and_depth_are_enforced() {
        let mut connection = connection();
        let parent = create_task_core(&mut connection, task_input("整理资料", None)).unwrap().tasks[0].id.clone();
        let child = create_task_core(&mut connection, task_input("做PPT", Some(parent))).unwrap().tasks[1].clone();
        assert_eq!(child.display_code, "#1.1");
        let error = create_task_core(&mut connection, task_input("非法三级", Some(child.id))).unwrap_err();
        assert!(error.contains("最多只允许两级"));
    }

    #[test]
    fn event_rows_are_immutable() {
        let mut connection = connection();
        create_task_core(&mut connection, task_input("整理资料", None)).unwrap();
        assert!(connection.execute("UPDATE events SET event_type='changed'", []).is_err());
        assert!(connection.execute("DELETE FROM events", []).is_err());
    }

    #[test]
    fn tasks_can_be_edited_after_creation() {
        let mut connection = connection();
        let task = create_task_core(&mut connection, task_input("整理资料", None)).unwrap().tasks[0].clone();
        let day = update_task_core(&mut connection, UpdateTaskInput {
            work_date: "2026-09-02".into(),
            instance_id: task.id,
            title: "进一步整理资料".into(),
            planned_start: Some("10:00".into()),
            planned_end: Some("11:00".into()),
        }).unwrap();
        assert_eq!(day.tasks[0].title, "进一步整理资料");
        assert_eq!(day.tasks[0].planned_start.as_deref(), Some("10:00"));
        assert_eq!(day.timeline.last().unwrap().event_type, "task.updated");
    }

    #[test]
    fn focus_requires_top_level_task_and_pause_reason() {
        let mut connection = connection();
        let parent = create_task_core(&mut connection, task_input("整理资料", None)).unwrap().tasks[0].id.clone();
        let child = create_task_core(&mut connection, task_input("做PPT", Some(parent.clone()))).unwrap().tasks[1].id.clone();
        assert!(start_focus_core(&mut connection, StartFocusInput {
            work_date: "2026-09-02".into(), task_id: child, planned_seconds: 1500,
        }).unwrap_err().contains("一级任务"));
        start_focus_core(&mut connection, StartFocusInput {
            work_date: "2026-09-02".into(), task_id: parent, planned_seconds: 1500,
        }).unwrap();
        assert!(pause_focus_core(&mut connection, PauseFocusInput {
            work_date: "2026-09-02".into(), reason: "   ".into(),
        }).unwrap_err().contains("暂停原因"));
        let day = pause_focus_core(&mut connection, PauseFocusInput {
            work_date: "2026-09-02".into(), reason: "临时回复消息".into(),
        }).unwrap();
        assert!(day.timeline.last().unwrap().title.contains("临时回复消息"));
    }

    #[test]
    fn switching_focus_writes_a_visible_detailed_event() {
        let mut connection = connection();
        let first = create_task_core(&mut connection, task_input("整理资料", None)).unwrap().tasks[0].id.clone();
        let second = create_task_core(&mut connection, task_input("制作汇报", None)).unwrap().tasks[1].id.clone();
        start_focus_core(&mut connection, StartFocusInput {
            work_date: "2026-09-02".into(), task_id: first, planned_seconds: 1500,
        }).unwrap();
        let day = switch_focus_core(&mut connection, SwitchFocusInput {
            work_date: "2026-09-02".into(), task_id: second,
        }).unwrap();
        let event = day.timeline.last().unwrap();
        assert_eq!(event.event_type, "focus.task_switched");
        assert_eq!(event.visibility, "summary");
        assert!(event.title.contains("#1 整理资料"));
        assert!(event.title.contains("#2 制作汇报"));
    }

    #[test]
    fn only_one_focus_session_can_be_active() {
        let mut connection = connection();
        let task_id = create_task_core(&mut connection, task_input("整理资料", None)).unwrap().tasks[0].id.clone();
        let input = StartFocusInput { work_date: "2026-09-02".into(), task_id, planned_seconds: 1500 };
        start_focus_core(&mut connection, input.clone()).unwrap();
        assert!(start_focus_core(&mut connection, input).unwrap_err().contains("已有正在进行"));
    }
}
