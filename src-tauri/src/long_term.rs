use crate::{db, model::DayState, Database};
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LongTermItem {
    id: String,
    parent_id: Option<String>,
    title: String,
    workload: Option<String>,
    completed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveLongTermInput {
    id: Option<String>,
    parent_id: Option<String>,
    title: String,
    workload: Option<String>,
    completed: bool,
}

pub(crate) fn validate_workload(is_child: bool, workload: Option<&str>) -> Result<(), String> {
    match workload {
        None => Ok(()),
        Some(_) if !is_child => Err("只有子任务可以设置工作量".into()),
        Some("light" | "large" | "very_large") => Ok(()),
        Some(_) => Err("无效工作量；超过 3 小时请拆成更小的子任务".into()),
    }
}

pub(crate) fn workload_label(workload: Option<&str>) -> &'static str {
    match workload {
        Some("light") => "轻松（<1 h）",
        Some("large") => "较大（1–2 h）",
        Some("very_large") => "极大（2–3 h）",
        _ => "未估算",
    }
}

pub(crate) fn workload_change(before: Option<&str>, after: Option<&str>) -> String {
    if before == after { return String::new(); }
    format!("，工作量：{} → {}", workload_label(before), workload_label(after))
}

pub(crate) fn focus_instance(tx: &Transaction<'_>, target: &str, work_date: &str) -> Result<String, String> {
    let Some(item_id) = target.strip_prefix("long-term:") else { return Ok(target.to_string()); };
    let (title, task_id): (String, Option<String>) = tx.query_row(
        "SELECT l.title,m.task_id FROM long_term_work l LEFT JOIN long_term_focus_targets m ON m.item_id=l.id
         WHERE l.id=?1 AND l.parent_id IS NULL AND l.completed=0",
        [item_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(|error| error.to_string())?.ok_or("请选择未完成的长期主任务")?;
    let now = db::now_iso();
    let task_id = match task_id {
        Some(id) => id,
        None => {
            let id = db::new_id();
            tx.execute("INSERT INTO tasks(id,title,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?3)", params![id,title,now])
                .map_err(|error| error.to_string())?;
            tx.execute("INSERT INTO long_term_focus_targets(task_id,item_id) VALUES(?1,?2)", params![id,item_id])
                .map_err(|error| error.to_string())?;
            id
        }
    };
    let id = db::new_id();
    tx.execute(
        "INSERT INTO task_day_instances(id,task_id,work_date,display_code,top_level_no,importance,urgency,created_at_utc,updated_at_utc)
         VALUES(?1,?2,?3,?4,1,'important','relaxed',?5,?5) ON CONFLICT(task_id,work_date) DO NOTHING",
        params![id, task_id, work_date, format!("long-term:{item_id}"), now],
    ).map_err(|error| error.to_string())?;
    tx.query_row("SELECT id FROM task_day_instances WHERE task_id=?1 AND work_date=?2", params![task_id,work_date], |row| row.get(0))
        .map_err(|error| error.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MoveToLongTermInput {
    work_date: String,
    instance_id: String,
    parent_id: Option<String>,
    title: String,
    workload: Option<String>,
}

fn move_from_day(connection: &mut Connection, input: MoveToLongTermInput) -> Result<DayState, String> {
    let title = input.title.trim();
    if title.is_empty() { return Err("任务内容不能为空".into()); }
    // Run the transfer and its audit event together. Keep the dated instances as source records.
    let tx = connection.transaction().map_err(|error| error.to_string())?;
    let day = db::read_day(&tx, &input.work_date)?;
    let source = day.tasks.iter().find(|task| task.id == input.instance_id).ok_or("任务已移走或不存在")?;
    let children: Vec<_> = day.tasks.iter().filter(|task| task.parent_id.as_deref() == Some(source.id.as_str())).collect();
    if source.parent_id.is_some() && input.parent_id.is_none() {
        return Err("请为子任务选择一个长期主任务".into());
    }
    if let Some(parent_id) = &input.parent_id {
        if !children.is_empty() { return Err("有子任务的主任务只能整体移为长期主任务".into()); }
        let exists = tx.query_row("SELECT 1 FROM long_term_work WHERE id=?1 AND parent_id IS NULL", [parent_id], |_| Ok(true))
            .optional().map_err(|error| error.to_string())?.unwrap_or(false);
        if !exists { return Err("所选长期主任务不存在".into()); }
    }
    validate_workload(input.parent_id.is_some(), input.workload.as_deref())?;
    for task in std::iter::once(source).chain(children.iter().copied()) {
        let blocked: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM focus_sessions f JOIN task_day_instances d ON d.id=f.primary_task_instance_id
               WHERE f.active_guard=1 AND d.task_id=?1)
             OR EXISTS(SELECT 1 FROM task_day_instances WHERE task_id=?1 AND work_date>?2)
             OR EXISTS(SELECT 1 FROM task_inbox WHERE task_id=?1)
             OR EXISTS(SELECT 1 FROM goal_action_occurrences g JOIN tasks t ON t.id=?1
               WHERE g.active=1 AND g.task_id IN (t.id,t.parent_task_id))",
            params![task.permanent_task_id, input.work_date], |row| row.get(0),
        ).map_err(|error| error.to_string())?;
        if blocked { return Err("请先处理该任务的进行中专注、未来日程、待办或成长计划".into()); }
        let is_source = task.id == source.id;
        let parent = if is_source { input.parent_id.as_deref() } else { Some(source.permanent_task_id.as_str()) };
        let workload = if is_source { input.workload.as_deref() } else { task.workload.as_deref() };
        let moved_title = if is_source { title } else { &task.title };
        if moved_title.chars().count() > 200 { return Err("移入前请将主任务及子事项的内容缩短至 200 字以内".into()); }
        tx.execute(
            "INSERT INTO long_term_work(id,parent_id,title,workload,completed) VALUES(?1,?2,?3,?4,?5)",
            params![task.permanent_task_id, parent, moved_title, workload, task.status == "completed"],
        ).map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO long_term_transfers(task_id,source_instance_id,work_date) VALUES(?1,?2,?3)",
            params![task.permanent_task_id, task.id, input.work_date],
        ).map_err(|error| error.to_string())?;
    }
    let change = workload_change(source.workload.as_deref(), input.workload.as_deref());
    db::append_event(&tx, "task.moved_to_long_term", "task", &source.permanent_task_id, &input.work_date, "summary",
        &format!("任务{}移入长期工作：{title}{change}", source.display_code), None, &db::new_id())?;
    tx.commit().map_err(|error| error.to_string())?;
    db::read_day(connection, &input.work_date)
}

#[tauri::command]
pub(crate) fn move_task_to_long_term(database: State<'_, Database>, input: MoveToLongTermInput) -> Result<DayState, String> {
    let mut connection = database.0.lock().map_err(|error| error.to_string())?;
    move_from_day(&mut connection, input)
}

fn list(connection: &Connection) -> Result<Vec<LongTermItem>, String> {
    let mut statement = connection.prepare(
        "SELECT id,parent_id,title,workload,completed FROM long_term_work ORDER BY completed,rowid",
    ).map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| Ok(LongTermItem {
        id: row.get(0)?, parent_id: row.get(1)?, title: row.get(2)?,
        workload: row.get(3)?, completed: row.get(4)?,
    })).map_err(|error| error.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|error| error.to_string())
}

fn save(connection: &Connection, input: &SaveLongTermInput) -> Result<String, String> {
    let title = input.title.trim();
    if !(1..=200).contains(&title.chars().count()) {
        return Err("长期任务内容须为 1–200 字".into());
    }
    validate_workload(input.parent_id.is_some(), input.workload.as_deref())?;
    if let Some(parent_id) = &input.parent_id {
        let valid: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM long_term_work WHERE id=?1 AND parent_id IS NULL)",
            [parent_id], |row| row.get(0),
        ).map_err(|error| error.to_string())?;
        if !valid { return Err("长期任务父项不存在，或已达到两级上限".into()); }
    }
    if let Some(id) = &input.id {
        let changed = connection.execute(
            "UPDATE long_term_work SET title=?1,workload=?2,completed=?3 WHERE id=?4 AND parent_id IS ?5",
            params![title, input.workload, input.completed, id, input.parent_id],
        ).map_err(|error| error.to_string())?;
        if changed == 0 { return Err("长期任务不存在或父项不匹配".into()); }
        Ok(id.clone())
    } else {
        let id = db::new_id();
        connection.execute(
            "INSERT INTO long_term_work(id,parent_id,title,workload,completed) VALUES(?1,?2,?3,?4,?5)",
            params![id, input.parent_id, title, input.workload, input.completed],
        ).map_err(|error| error.to_string())?;
        Ok(id)
    }
}

fn save_with_event(connection: &mut Connection, input: SaveLongTermInput, work_date: &str) -> Result<Vec<LongTermItem>, String> {
    let tx = connection.transaction().map_err(|error| error.to_string())?;
    let previous = input.id.as_ref().map(|id| tx.query_row(
        "SELECT completed,workload FROM long_term_work WHERE id=?1",
        [id], |row| Ok((row.get::<_, bool>(0)?, row.get::<_, Option<String>>(1)?)),
    ).map_err(|_| "长期任务不存在".to_string())).transpose()?;
    let id = save(&tx, &input)?;
    let workload = input.workload.as_deref();
    let event = match previous {
        None => Some(("long_term.created", "新增", workload.map(|value| format!("，工作量：{}", workload_label(Some(value)))).unwrap_or_default())),
        Some((was_completed, old_workload)) => {
            let change = workload_change(old_workload.as_deref(), workload);
            match (was_completed, input.completed) {
                (false, true) => Some(("long_term.completed", "完成", change)),
                (true, false) => Some(("long_term.reopened", "重开", change)),
                _ if !change.is_empty() => Some(("long_term.workload_changed", "调整", change)),
                _ => None,
            }
        }
    };
    if let Some((event_type, action, change)) = event {
        let title = input.title.trim();
        let event_title = if let Some(parent_id) = &input.parent_id {
            let parent_title: String = tx.query_row(
                "SELECT title FROM long_term_work WHERE id=?1", [parent_id], |row| row.get(0),
            ).map_err(|error| error.to_string())?;
            format!("{action}长期子事项：{parent_title} / {title}{change}")
        } else {
            format!("{action}长期任务：{title}{change}")
        };
        db::append_event(&tx, event_type, "long_term_work", &id, work_date, "summary", &event_title, None, &db::new_id())?;
    }
    let items = list(&tx)?;
    tx.commit().map_err(|error| error.to_string())?;
    Ok(items)
}

#[tauri::command]
pub(crate) fn list_long_term_work(database: State<'_, Database>) -> Result<Vec<LongTermItem>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    list(&connection)
}

#[tauri::command]
pub(crate) fn save_long_term_work(database: State<'_, Database>, input: SaveLongTermInput) -> Result<Vec<LongTermItem>, String> {
    let mut connection = database.0.lock().map_err(|error| error.to_string())?;
    let work_date = Local::now().date_naive().to_string();
    save_with_event(&mut connection, input, &work_date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{commands, model::{CreateTaskInput, StartFocusInput, SwitchFocusInput, WorkEntryInput, CompleteFocusInput}};

    fn daily_task(parent_id: Option<String>, workload: Option<&str>) -> CreateTaskInput {
        CreateTaskInput {
            work_date: "2026-11-23".into(), title: "设备选型".into(), importance: "important".into(),
            urgency: "urgent".into(), parent_id, planned_start: None, planned_end: None,
            workload: workload.map(str::to_owned),
        }
    }

    #[test]
    fn activity_is_logged_on_the_action_date_and_rendered_in_the_daily_note() {
        let mut connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        let first_date = "2026-11-23";
        let completion_date = "2026-11-24";
        let parent = SaveLongTermInput { id: None, parent_id: None, title: "设备选型".into(), workload: None, completed: false };
        let items = save_with_event(&mut connection, parent, first_date).unwrap();
        let parent = SaveLongTermInput { id: Some(items[0].id.clone()), parent_id: None, title: "设备选型".into(), workload: None, completed: true };
        let child = |id, completed| SaveLongTermInput {
            id, parent_id: parent.id.clone(), title: "收集规格".into(), workload: Some("light".into()), completed,
        };
        let items = save_with_event(&mut connection, child(None, false), first_date).unwrap();
        let child_id = items[1].id.clone();
        save_with_event(&mut connection, child(Some(child_id.clone()), true), completion_date).unwrap();
        save_with_event(&mut connection, child(Some(child_id.clone()), true), completion_date).unwrap();
        save_with_event(&mut connection, child(Some(child_id.clone()), false), completion_date).unwrap();
        save_with_event(&mut connection, SaveLongTermInput { workload: Some("large".into()), ..child(Some(child_id), false) }, completion_date).unwrap();
        save_with_event(&mut connection, parent, completion_date).unwrap();

        let first = db::read_day(&connection, first_date).unwrap();
        assert!(first.tasks.is_empty());
        assert_eq!(first.timeline.iter().map(|event| event.title.as_str()).collect::<Vec<_>>(), vec![
            "新增长期任务：设备选型",
            "新增长期子事项：设备选型 / 收集规格，工作量：轻松（<1 h）",
        ]);
        let day = db::read_day(&connection, completion_date).unwrap();
        assert!(day.tasks.is_empty() && day.schedules.is_empty());
        assert_eq!(day.timeline.iter().map(|event| event.title.as_str()).collect::<Vec<_>>(), vec![
            "完成长期子事项：设备选型 / 收集规格",
            "重开长期子事项：设备选型 / 收集规格",
            "调整长期子事项：设备选型 / 收集规格，工作量：轻松（<1 h） → 较大（1–2 h）",
            "完成长期任务：设备选型",
        ]);
        let markdown = crate::obsidian::render_managed(&day, &[]);
        assert!(markdown.split_once("## 今日记录").unwrap().1.contains("完成长期任务：设备选型"));
    }

    #[test]
    fn moving_a_group_keeps_ids_and_history_and_rolls_back_if_any_child_fails() {
        let mut connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        let parent = commands::create_task_core(&mut connection, daily_task(None, None)).unwrap().tasks.remove(0);
        let child = commands::create_task_core(&mut connection, daily_task(Some(parent.id.clone()), Some("large"))).unwrap().tasks.remove(1);
        let input = || MoveToLongTermInput {
            work_date: "2026-11-23".into(), instance_id: parent.id.clone(),
            parent_id: None, title: "设备选型计划".into(), workload: None,
        };
        connection.execute("UPDATE tasks SET title=?1 WHERE id=?2", params!["事".repeat(201), child.permanent_task_id]).unwrap();
        assert!(move_from_day(&mut connection, input()).is_err());
        assert!(list(&connection).unwrap().is_empty());
        assert_eq!(db::read_day(&connection, "2026-11-23").unwrap().tasks.len(), 2);
        connection.execute("UPDATE tasks SET title='收集规格' WHERE id=?1", [&child.permanent_task_id]).unwrap();
        connection.execute("UPDATE task_day_instances SET day_status='completed' WHERE id=?1", [&child.id]).unwrap();

        let day = move_from_day(&mut connection, input()).unwrap();
        assert!(day.tasks.is_empty());
        assert_eq!(day.timeline.len(), 3);
        assert_eq!(day.timeline[2].event_type, "task.moved_to_long_term");
        let items = list(&connection).unwrap();
        assert_eq!(items[0].id, parent.permanent_task_id);
        assert_eq!(items[1].parent_id.as_deref(), Some(parent.permanent_task_id.as_str()));
        assert_eq!(items[1].id, child.permanent_task_id);
        assert_eq!(items[1].workload.as_deref(), Some("large"));
        assert!(items[1].completed);
        assert!(move_from_day(&mut connection, input()).is_err());
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM task_day_instances", [], |row| row.get::<_, i64>(0)).unwrap(), 2);
        let next = commands::create_task_core(&mut connection, daily_task(None, None)).unwrap();
        assert_eq!(next.tasks[0].display_code, "#2");
    }

    #[test]
    fn child_can_move_into_an_existing_long_term_parent() {
        let mut connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        save(&connection, &SaveLongTermInput {
            id: None, parent_id: None, title: "既有计划".into(), workload: None, completed: false,
        }).unwrap();
        let destination = list(&connection).unwrap().remove(0);
        let parent = commands::create_task_core(&mut connection, daily_task(None, None)).unwrap().tasks.remove(0);
        let child = commands::create_task_core(&mut connection, daily_task(Some(parent.id.clone()), Some("light"))).unwrap().tasks.remove(1);
        let day = move_from_day(&mut connection, MoveToLongTermInput {
            work_date: "2026-11-23".into(), instance_id: child.id, parent_id: Some(destination.id.clone()),
            title: "收集规格".into(), workload: Some("very_large".into()),
        }).unwrap();
        assert_eq!(day.tasks.len(), 1);
        assert_eq!(day.tasks[0].id, parent.id);
        assert!(day.timeline.last().unwrap().title.contains("工作量：轻松（<1 h） → 极大（2–3 h）"));
        let moved = list(&connection).unwrap().remove(1);
        assert_eq!(moved.parent_id, Some(destination.id));
        assert_eq!(moved.workload.as_deref(), Some("very_large"));
    }

    #[test]
    fn long_term_focus_reuses_timer_and_preserves_daily_numbering() {
        let mut connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        let work_date = "2026-11-23";
        save(&connection, &SaveLongTermInput {
            id: None, parent_id: None, title: "设备校核".into(), workload: None, completed: false,
        }).unwrap();
        let parent = list(&connection).unwrap().remove(0);
        let target = format!("long-term:{}", parent.id);
        let day = commands::start_focus_core(&mut connection, StartFocusInput {
            work_date: work_date.into(), task_id: target.clone(), planned_seconds: 1500,
        }).unwrap();
        assert!(day.tasks.is_empty());
        let focus = day.focus.unwrap();
        assert_eq!(focus.long_term_id.as_deref(), Some(parent.id.as_str()));
        assert!(day.timeline.last().unwrap().title.contains("长期 设备校核"));
        let day = commands::add_work_entry_core(&mut connection, WorkEntryInput {
            work_date: work_date.into(), content: "补齐数据".into(), entry_type: "idea".into(),
            review_level: "normal".into(), task_id: Some(focus.task_id.clone()),
        }).unwrap();
        assert_eq!(day.timeline.last().unwrap().title, "长期 · 设备校核：补齐数据");
        let daily = commands::create_task_core(&mut connection, daily_task(None, None)).unwrap().tasks.remove(0);
        assert_eq!(daily.display_code, "#1");
        let switch = |task_id| SwitchFocusInput { work_date: work_date.into(), task_id };
        let day = commands::switch_focus_core(&mut connection, switch(daily.id)).unwrap();
        assert!(day.focus.unwrap().long_term_id.is_none());
        let day = commands::switch_focus_core(&mut connection, switch(target)).unwrap();
        assert_eq!(day.focus.unwrap().task_id, focus.task_id);
        let day = commands::complete_focus_core(&mut connection, CompleteFocusInput {
            work_date: work_date.into(), reason: "early_complete".into(),
        }).unwrap();
        assert!(day.focus.is_none());
        assert_eq!(day.tasks.len(), 1);
        assert!(!list(&connection).unwrap()[0].completed);
        let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        let report = crate::report_details::tasks(&connection, date, date).unwrap();
        assert_eq!(report.len(), 1);
    }

    #[test]
    fn board_persists_independently_of_daily_tasks() {
        let connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        let input = |parent_id, workload| SaveLongTermInput {
            id: None, title: "工作安排".into(), parent_id, workload, completed: false,
        };
        assert!(save(&connection, &input(None, Some("light".into()))).is_err());
        save(&connection, &input(None, None)).unwrap();
        let parent = list(&connection).unwrap().remove(0);
        assert!(save(&connection, &input(Some(parent.id.clone()), Some("over_3h".into()))).is_err());
        save(&connection, &input(Some(parent.id.clone()), Some("large".into()))).unwrap();
        let child = list(&connection).unwrap().remove(1);
        assert!(save(&connection, &input(Some(child.id.clone()), None)).is_err());
        assert!(save(&connection, &SaveLongTermInput { id: Some(child.id.clone()), ..input(None, None) }).is_err());
        save(&connection, &SaveLongTermInput {
            id: Some(child.id.clone()), completed: true, title: "更新后".into(),
            ..input(Some(parent.id), Some("very_large".into()))
        }).unwrap();
        db::initialize(&connection).unwrap();
        let items = list(&connection).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].id, child.id);
        assert_eq!(items[1].title, "更新后");
        assert_eq!(items[1].workload.as_deref(), Some("very_large"));
        assert!(items[1].completed);
        let day = db::read_day(&connection, "2026-11-23").unwrap();
        assert!(day.tasks.is_empty() && day.timeline.is_empty());
    }
}
