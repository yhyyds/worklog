use crate::{db, model::DayState, Database};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use tauri::State;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CarryCandidate {
    pub instance_id: String,
    pub permanent_task_id: String,
    pub parent_id: Option<String>,
    pub display_code: String,
    pub title: String,
    pub status: String,
    pub importance: String,
    pub urgency: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EndOfDayPreview {
    pub work_date: String,
    pub next_work_date: String,
    pub total_count: usize,
    pub completed_count: usize,
    pub waiting_count: usize,
    pub blocked_count: usize,
    pub candidates: Vec<CarryCandidate>,
    pub already_closed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseDayInput {
    pub work_date: String,
    pub next_work_date: String,
    pub selected_instance_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseDayResult {
    pub source_day: DayState,
    pub next_day: DayState,
    pub carried_count: usize,
    pub skipped_count: usize,
}

pub fn initialize(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(include_str!("../migrations/0003_day_closing.sql"))
}

fn following_date(work_date: &str) -> Result<String, String> {
    let date = NaiveDate::parse_from_str(work_date, "%Y-%m-%d")
        .map_err(|_| "日期必须为 YYYY-MM-DD".to_string())?;
    Ok(date.succ_opt().ok_or("无法计算次日日期")?.format("%Y-%m-%d").to_string())
}

fn is_carry_candidate(status: &str) -> bool {
    !matches!(status, "completed" | "cancelled")
}

fn preview_core(connection: &Connection, work_date: &str) -> Result<EndOfDayPreview, String> {
    let day = db::read_day(connection, work_date)?;
    let already_closed = connection
        .query_row("SELECT 1 FROM day_closures WHERE work_date=?1", [work_date], |_| Ok(true))
        .optional()
        .map_err(|error| error.to_string())?
        .unwrap_or(false);
    let candidates = day.tasks.iter().filter(|task| is_carry_candidate(&task.status)).map(|task| CarryCandidate {
        instance_id: task.id.clone(),
        permanent_task_id: task.permanent_task_id.clone(),
        parent_id: task.parent_id.clone(),
        display_code: task.display_code.clone(),
        title: task.title.clone(),
        status: task.status.clone(),
        importance: task.importance.clone(),
        urgency: task.urgency.clone(),
    }).collect();
    Ok(EndOfDayPreview {
        work_date: work_date.to_string(),
        next_work_date: following_date(work_date)?,
        total_count: day.tasks.len(),
        completed_count: day.tasks.iter().filter(|task| task.status == "completed").count(),
        waiting_count: day.tasks.iter().filter(|task| task.status == "waiting").count(),
        blocked_count: day.tasks.iter().filter(|task| task.status == "blocked").count(),
        candidates,
        already_closed,
    })
}

fn close_day_core(connection: &mut Connection, input: CloseDayInput) -> Result<CloseDayResult, String> {
    if following_date(&input.work_date)? != input.next_work_date {
        return Err("顺延目标必须是紧接着的下一自然日".to_string());
    }
    let active_focus: i64 = connection.query_row(
        "SELECT (SELECT COUNT(*) FROM focus_sessions WHERE active_guard=1 AND work_date=?1)
              + (SELECT COUNT(*) FROM rest_sessions WHERE active_guard=1 AND work_date=?1)",
        [&input.work_date],
        |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if active_focus > 0 {
        return Err("请先结束当前专注或休息，再进行日终收尾".to_string());
    }
    let closed: i64 = connection.query_row(
        "SELECT COUNT(*) FROM day_closures WHERE work_date=?1",
        [&input.work_date],
        |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if closed > 0 {
        return Err("今天已经完成日终收尾，不能重复顺延".to_string());
    }

    let source_day = db::read_day(connection, &input.work_date)?;
    let selected: HashSet<_> = input.selected_instance_ids.iter().cloned().collect();
    let eligible: HashSet<_> = source_day.tasks.iter()
        .filter(|task| is_carry_candidate(&task.status))
        .map(|task| task.id.clone())
        .collect();
    if let Some(invalid) = selected.iter().find(|id| !eligible.contains(*id)) {
        return Err(format!("事项 {invalid} 已完成、已取消或不属于今天"));
    }
    let carried_count = selected.len();
    let skipped_count = eligible.len().saturating_sub(carried_count);
    let completed_count = source_day.tasks.iter().filter(|task| task.status == "completed").count();

    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let mut next_top: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(top_level_no),0) FROM task_day_instances WHERE work_date=?1 AND parent_instance_id IS NULL",
        [&input.next_work_date],
        |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    let mut destination: HashMap<String, (String, String, i64)> = HashMap::new();
    let now = db::now_iso();

    for task in source_day.tasks.iter().filter(|task| selected.contains(&task.id)) {
        let exists: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM task_day_instances WHERE task_id=?1 AND work_date=?2",
            params![task.permanent_task_id, input.next_work_date],
            |row| row.get(0),
        ).map_err(|error| error.to_string())?;
        if exists > 0 {
            return Err(format!("次日已经包含任务{}，已停止重复顺延", task.display_code));
        }

        let parent_destination = task.parent_id.as_ref().and_then(|parent| destination.get(parent)).cloned();
        let (parent_instance_id, display_code, top_level_no, child_no) =
            if let Some((parent_id, parent_code, parent_top)) = parent_destination {
                let next_child: i64 = transaction.query_row(
                    "SELECT COALESCE(MAX(child_no),0)+1 FROM task_day_instances WHERE parent_instance_id=?1",
                    [&parent_id],
                    |row| row.get(0),
                ).map_err(|error| error.to_string())?;
                (Some(parent_id), format!("{parent_code}.{next_child}"), parent_top, Some(next_child))
            } else {
                next_top += 1;
                (None, format!("#{next_top}"), next_top, None)
            };
        let next_status = if matches!(task.status.as_str(), "waiting" | "blocked") {
            task.status.as_str()
        } else {
            "not_started"
        };
        let instance_id = db::new_id();
        let sort_order = top_level_no * 1000 + child_no.unwrap_or(0);
        transaction.execute(
            "INSERT INTO task_day_instances(
                id,task_id,work_date,parent_instance_id,carry_from_instance_id,display_code,
                top_level_no,child_no,importance,urgency,day_status,
                planned_start_minute,planned_end_minute,sort_order,created_at_utc,updated_at_utc
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,NULL,NULL,?12,?13,?13)",
            params![
                instance_id, task.permanent_task_id, input.next_work_date, parent_instance_id,
                task.id, display_code, top_level_no, child_no, task.importance, task.urgency,
                next_status, sort_order, now
            ],
        ).map_err(|error| error.to_string())?;
        transaction.execute(
            "UPDATE tasks SET status=?1,updated_at_utc=?2,row_version=row_version+1 WHERE id=?3",
            params![next_status, now, task.permanent_task_id],
        ).map_err(|error| error.to_string())?;
        transaction.execute(
            "UPDATE task_day_instances SET day_status=CASE WHEN day_status IN ('not_started','in_progress') THEN 'deferred' ELSE day_status END,updated_at_utc=?1 WHERE id=?2",
            params![now, task.id],
        ).map_err(|error| error.to_string())?;
        destination.insert(task.id.clone(), (instance_id, display_code, top_level_no));
    }

    let closure_id = db::new_id();
    let transaction_id = db::new_id();
    let summary = json!({
        "completedCount": completed_count,
        "carriedCount": carried_count,
        "skippedCount": skipped_count,
        "selectedInstanceIds": input.selected_instance_ids,
    });
    transaction.execute(
        "INSERT INTO day_closures(work_date,next_work_date,closed_at_utc,carried_count,skipped_count,summary_json)
         VALUES(?1,?2,?3,?4,?5,?6)",
        params![input.work_date, input.next_work_date, now, carried_count as i64, skipped_count as i64, summary.to_string()],
    ).map_err(|error| error.to_string())?;
    db::append_event(
        &transaction, "day.closed", "day_closure", &closure_id, &input.work_date, "summary",
        &format!("日终收尾：完成{completed_count}项，顺延{carried_count}项至{}", input.next_work_date),
        None, &transaction_id,
    )?;
    if carried_count > 0 {
        db::append_event(
            &transaction, "day.carryover_received", "day_closure", &closure_id, &input.next_work_date, "detail",
            &format!("从{}顺延{carried_count}项任务", input.work_date),
            None, &transaction_id,
        )?;
    }
    transaction.commit().map_err(|error| error.to_string())?;

    Ok(CloseDayResult {
        source_day: db::read_day(connection, &input.work_date)?,
        next_day: db::read_day(connection, &input.next_work_date)?,
        carried_count,
        skipped_count,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn preview_end_of_day(database: State<'_, Database>, work_date: String) -> Result<EndOfDayPreview, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    preview_core(&connection, &work_date)
}

#[tauri::command]
pub fn close_day(database: State<'_, Database>, input: CloseDayInput) -> Result<CloseDayResult, String> {
    let mut connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    close_day_core(&mut connection, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{commands, model::{CreateTaskInput, SetTaskStatusInput}};

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        crate::obsidian::initialize(&connection).unwrap();
        initialize(&connection).unwrap();
        connection
    }

    fn create(connection: &mut Connection, title: &str, parent_id: Option<String>) -> crate::model::DayTask {
        commands::create_task_core(connection, CreateTaskInput {
            work_date: "2026-09-02".into(), title: title.into(),
            importance: "important".into(), urgency: "urgent".into(), parent_id,
            planned_start: None, planned_end: None,
        }).unwrap().tasks.last().unwrap().clone()
    }

    fn close(connection: &mut Connection, selected_instance_ids: Vec<String>) -> CloseDayResult {
        close_day_core(connection, CloseDayInput {
            work_date: "2026-09-02".into(),
            next_work_date: "2026-09-03".into(),
            selected_instance_ids,
        }).unwrap()
    }

    #[test]
    fn preview_excludes_completed_and_cancelled_tasks() {
        let mut connection = connection();
        let completed = create(&mut connection, "已完成", None);
        let open = create(&mut connection, "未完成", None);
        commands::set_task_status_core(&mut connection, SetTaskStatusInput {
            work_date: "2026-09-02".into(), instance_id: completed.id, status: "completed".into(),
        }).unwrap();
        let preview = preview_core(&connection, "2026-09-02").unwrap();
        assert_eq!(preview.completed_count, 1);
        assert_eq!(preview.candidates.len(), 1);
        assert_eq!(preview.candidates[0].instance_id, open.id);
    }

    #[test]
    fn carryover_keeps_permanent_ids_and_drops_completed_children() {
        let mut connection = connection();
        let parent = create(&mut connection, "整理资料", None);
        let completed_child = create(&mut connection, "已做完的图表", Some(parent.id.clone()));
        let open_child = create(&mut connection, "制作 PPT", Some(parent.id.clone()));
        commands::set_task_status_core(&mut connection, SetTaskStatusInput {
            work_date: "2026-09-02".into(), instance_id: completed_child.id.clone(), status: "completed".into(),
        }).unwrap();
        let result = close(&mut connection, vec![parent.id.clone(), open_child.id.clone()]);
        assert_eq!(result.next_day.tasks.len(), 2);
        assert_eq!(result.next_day.tasks[0].display_code, "#1");
        assert_eq!(result.next_day.tasks[1].display_code, "#1.1");
        assert_eq!(result.next_day.tasks[0].permanent_task_id, parent.permanent_task_id);
        assert_eq!(result.next_day.tasks[1].permanent_task_id, open_child.permanent_task_id);
        assert!(!result.next_day.tasks.iter().any(|task| task.permanent_task_id == completed_child.permanent_task_id));
    }

    #[test]
    fn selected_child_is_promoted_when_parent_is_not_carried() {
        let mut connection = connection();
        let parent = create(&mut connection, "父任务", None);
        let child = create(&mut connection, "仍需处理的部分", Some(parent.id));
        let result = close(&mut connection, vec![child.id.clone()]);
        assert_eq!(result.next_day.tasks[0].display_code, "#1");
        assert_eq!(result.next_day.tasks[0].parent_id, None);
        assert_eq!(result.next_day.tasks[0].permanent_task_id, child.permanent_task_id);
    }

    #[test]
    fn day_cannot_be_closed_twice() {
        let mut connection = connection();
        let task = create(&mut connection, "任务", None);
        close(&mut connection, vec![task.id]);
        let error = close_day_core(&mut connection, CloseDayInput {
            work_date: "2026-09-02".into(), next_work_date: "2026-09-03".into(), selected_instance_ids: vec![],
        }).unwrap_err();
        assert!(error.contains("已经完成日终收尾"));
    }
}
