use crate::{db, Database};
use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxTask {
    task_id: String,
    title: String,
    group_id: String,
    parent_task_id: Option<String>,
    importance: String,
    urgency: String,
    entered_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalUnfinishedTask {
    instance_id: String,
    permanent_task_id: String,
    parent_instance_id: Option<String>,
    work_date: String,
    display_code: String,
    title: String,
    status: String,
    importance: String,
    urgency: String,
    reschedulable: bool,
    blocked_reason: Option<String>,
}

fn list_core(connection: &Connection) -> Result<Vec<InboxTask>, String> {
    let mut query = connection.prepare("SELECT t.id,t.title,i.group_id,i.parent_task_id,i.importance,i.urgency,i.entered_at_utc FROM task_inbox i JOIN tasks t ON t.id=i.task_id ORDER BY i.entered_at_utc,i.group_id,i.parent_task_id IS NOT NULL,t.created_at_utc").map_err(|e| e.to_string())?;
    let rows = query.query_map([], |r| Ok(InboxTask { task_id:r.get(0)?, title:r.get(1)?, group_id:r.get(2)?, parent_task_id:r.get(3)?, importance:r.get(4)?, urgency:r.get(5)?, entered_at:r.get(6)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())
}

fn parse_date(value: &str) -> Result<NaiveDate, String> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| "日期必须为 YYYY-MM-DD".to_string())?;
    if date.format("%Y-%m-%d").to_string() != value {
        return Err("日期必须为 YYYY-MM-DD".to_string());
    }
    Ok(date)
}

fn list_historical_core(connection: &Connection, before_date: &str) -> Result<Vec<HistoricalUnfinishedTask>, String> {
    parse_date(before_date)?;
    let no_carry = crate::planning::no_carry_tasks(connection)?;
    let mut query = connection.prepare(
        "SELECT i.id,t.id,i.parent_instance_id,i.work_date,i.display_code,t.title,
                i.day_status,i.importance,i.urgency,
                EXISTS(SELECT 1 FROM focus_sessions f WHERE f.primary_task_instance_id=i.id AND f.active_guard=1)
         FROM task_day_instances i
         JOIN tasks t ON t.id=i.task_id
         WHERE i.work_date<?1
           AND i.day_status IN ('not_started','in_progress','waiting','blocked')
           AND NOT EXISTS(SELECT 1 FROM inbox_removed_instances r WHERE r.instance_id=i.id)
           AND NOT EXISTS(SELECT 1 FROM goal_removed_instances r WHERE r.instance_id=i.id)
           AND NOT EXISTS(
             SELECT 1 FROM task_day_instances newer
             WHERE newer.task_id=i.task_id AND newer.work_date>i.work_date
               AND NOT EXISTS(SELECT 1 FROM inbox_removed_instances r WHERE r.instance_id=newer.id)
               AND NOT EXISTS(SELECT 1 FROM goal_removed_instances r WHERE r.instance_id=newer.id)
           )
         ORDER BY i.work_date,i.top_level_no,COALESCE(i.child_no,0)"
    ).map_err(|error| error.to_string())?;
    let rows = query.query_map([before_date], |row| {
        Ok((
            row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?,
            row.get::<_, String>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?,
            row.get::<_, bool>(9)?,
        ))
    }).map_err(|error| error.to_string())?;
    rows.map(|row| {
        let (instance_id, permanent_task_id, parent_instance_id, work_date, display_code, title, status, importance, urgency, active_focus) = row.map_err(|error| error.to_string())?;
        let blocked_reason = if no_carry.contains(&permanent_task_id) {
            Some("重复目标任务请在“成长”中调整日期".to_string())
        } else if active_focus {
            Some("请先结束该任务正在进行的专注".to_string())
        } else {
            None
        };
        Ok(HistoricalUnfinishedTask {
            instance_id, permanent_task_id, parent_instance_id, work_date, display_code,
            title, status, importance, urgency,
            reschedulable: blocked_reason.is_none(), blocked_reason,
        })
    }).collect()
}

fn reschedule_historical_core(
    connection: &mut Connection,
    source_instance_id: &str,
    target_date: &str,
    today: &str,
) -> Result<Vec<HistoricalUnfinishedTask>, String> {
    let target = parse_date(target_date)?;
    let current = parse_date(today)?;
    if target < current {
        return Err("请重新安排到今天或未来，不改写过去的计划".into());
    }
    let historical = list_historical_core(connection, today)?;
    let source = historical.iter().find(|task| task.instance_id == source_instance_id)
        .cloned().ok_or("该历史任务已完成、已重新安排或不再可见")?;
    let parent_is_visible = source.parent_instance_id.as_ref()
        .is_some_and(|parent| historical.iter().any(|task| &task.instance_id == parent));
    let source_is_group_root = source.parent_instance_id.is_none() || !parent_is_visible;
    let selected: Vec<_> = historical.iter().filter(|task| {
        task.instance_id == source.instance_id
            || (source_is_group_root && task.parent_instance_id.as_deref() == Some(source.instance_id.as_str()))
    }).cloned().collect();
    if let Some(blocked) = selected.iter().find(|task| !task.reschedulable) {
        return Err(blocked.blocked_reason.clone().unwrap_or_else(|| "该任务不能重新安排".into()));
    }

    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    for task in &selected {
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_day_instances WHERE task_id=?1 AND work_date=?2)",
            params![task.permanent_task_id, target_date], |row| row.get(0),
        ).map_err(|error| error.to_string())?;
        if exists {
            return Err(format!("{} 已包含“{}”，没有重复安排", target_date, task.title));
        }
    }

    let mut next_top: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(top_level_no),0) FROM task_day_instances WHERE work_date=?1 AND parent_instance_id IS NULL",
        [target_date], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    let mut destinations: HashMap<String, (String, String, i64)> = HashMap::new();
    let now = db::now_iso();
    for task in &selected {
        let parent_destination = task.parent_instance_id.as_ref()
            .and_then(|parent| destinations.get(parent)).cloned();
        let (parent_instance_id, display_code, top_level_no, child_no) = if let Some((parent_id, parent_code, parent_top)) = parent_destination {
            let child: i64 = transaction.query_row(
                "SELECT COALESCE(MAX(child_no),0)+1 FROM task_day_instances WHERE parent_instance_id=?1",
                [&parent_id], |row| row.get(0),
            ).map_err(|error| error.to_string())?;
            (Some(parent_id), format!("{parent_code}.{child}"), parent_top, Some(child))
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
        transaction.execute(
            "INSERT INTO task_day_instances(
                id,task_id,work_date,parent_instance_id,carry_from_instance_id,display_code,
                top_level_no,child_no,importance,urgency,day_status,sort_order,created_at_utc,updated_at_utc
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13)",
            params![instance_id, task.permanent_task_id, target_date, parent_instance_id,
                task.instance_id, display_code, top_level_no, child_no, task.importance,
                task.urgency, next_status, top_level_no * 1000 + child_no.unwrap_or(0), now],
        ).map_err(|error| error.to_string())?;
        transaction.execute(
            "UPDATE tasks SET status=?1,updated_at_utc=?2,row_version=row_version+1 WHERE id=?3",
            params![next_status, now, task.permanent_task_id],
        ).map_err(|error| error.to_string())?;
        transaction.execute(
            "UPDATE task_day_instances SET day_status=CASE WHEN day_status IN ('not_started','in_progress') THEN 'deferred' ELSE day_status END,updated_at_utc=?1 WHERE id=?2",
            params![now, task.instance_id],
        ).map_err(|error| error.to_string())?;
        destinations.insert(task.instance_id.clone(), (instance_id, display_code, top_level_no));
    }
    let transaction_id = db::new_id();
    let count = selected.len();
    db::append_event(
        &transaction, "task.historical_rescheduled", "task", &source.permanent_task_id,
        &source.work_date, "summary", &format!("重新安排{}：{}", source.display_code, source.title),
        Some(&format!("移至{target_date}；原日期记录保留。")), &transaction_id,
    )?;
    db::append_event(
        &transaction, "task.historical_received", "task", &source.permanent_task_id,
        target_date, "detail", &format!("从{}重新安排{count}项任务", source.work_date),
        None, &transaction_id,
    )?;
    transaction.commit().map_err(|error| error.to_string())?;
    list_historical_core(connection, today)
}

fn create_core(connection: &mut Connection, title: &str) -> Result<Vec<InboxTask>, String> {
    if title.trim().is_empty() { return Err("请输入待办内容".into()); }
    let tx = connection.transaction().map_err(|e| e.to_string())?;
    let id = db::new_id(); let now = db::now_iso();
    tx.execute("INSERT INTO tasks(id,title,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?3)", params![id,title.trim(),now]).map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO task_inbox(task_id,group_id,importance,urgency,entered_at_utc) VALUES(?1,?1,'secondary','relaxed',?2)", params![id,now]).map_err(|e| e.to_string())?;
    db::append_event(&tx,"task.inbox_created","task",&id,&Local::now().format("%Y-%m-%d").to_string(),"detail",&format!("收入待办箱：{}",title.trim()),None,&db::new_id())?;
    tx.commit().map_err(|e| e.to_string())?;
    list_core(connection)
}

fn move_core(connection: &mut Connection, instance_id: &str, work_date: &str) -> Result<Vec<InboxTask>, String> {
    let day = db::read_day(connection, work_date)?;
    let root = day.tasks.iter().find(|t| t.id == instance_id).ok_or("任务不在所选日期，或已收入待办箱")?;
    let selected: Vec<_> = day.tasks.iter().filter(|t| t.id == instance_id || t.parent_id.as_deref() == Some(instance_id)).collect();
    let no_carry=crate::planning::no_carry_tasks(connection)?;
    if selected.iter().any(|t|no_carry.contains(&t.permanent_task_id)){return Err("重复任务不能收入待办箱。未来尚未开始的安排可在长期目标中修改日期".into());}
    // Keeping completed children in the group preserves their completion fact.
    let tx = connection.transaction().map_err(|e| e.to_string())?;
    let now = db::now_iso();
    for task in &selected {
        let active: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM focus_sessions WHERE primary_task_instance_id=?1 AND active_guard=1)",[&task.id],|r|r.get(0)).map_err(|e|e.to_string())?;
        if active { return Err("请先结束这项任务的专注，再收入待办箱".into()); }
        let parent = if task.id == instance_id { None } else { Some(root.permanent_task_id.clone()) };
        tx.execute("INSERT INTO task_inbox(task_id,group_id,source_instance_id,parent_task_id,importance,urgency,entered_at_utc) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![task.permanent_task_id,root.permanent_task_id,task.id,parent,task.importance,task.urgency,now]).map_err(|e|format!("该任务已经在待办箱中：{e}"))?;
        tx.execute("INSERT INTO inbox_removed_instances(instance_id,removed_at_utc) VALUES(?1,?2)",params![task.id,now]).map_err(|e|e.to_string())?;
    }
    db::append_event(&tx,"task.moved_to_inbox","task",&root.permanent_task_id,work_date,"summary",&format!("收入待办箱：{}",root.title),Some("暂不安排日期；原始事件、专注记录和完成状态保留。子任务随父任务一起收纳。"),&db::new_id())?;
    tx.commit().map_err(|e|e.to_string())?;
    list_core(connection)
}

fn schedule_core(connection: &mut Connection, group_id: &str, work_date: &str) -> Result<Vec<InboxTask>, String> {
    let date = NaiveDate::parse_from_str(work_date,"%Y-%m-%d").map_err(|_|"日期无效")?;
    if date < Local::now().date_naive() { return Err("请安排到今天或未来，不改写过去的计划".into()); }
    let items: Vec<_> = list_core(connection)?.into_iter().filter(|t|t.group_id==group_id).collect();
    if items.is_empty() { return Err("待办不存在，或已经安排".into()); }
    let tx = connection.transaction().map_err(|e|e.to_string())?;
    let now = db::now_iso();
    let mut root_instance = None;
    let mut root_top = 0;
    for item in items {
        let source: Option<String> = tx.query_row("SELECT source_instance_id FROM task_inbox WHERE task_id=?1",[&item.task_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        let existing: Option<(String,bool)> = tx.query_row("SELECT id,EXISTS(SELECT 1 FROM inbox_removed_instances r WHERE r.instance_id=d.id) FROM task_day_instances d WHERE task_id=?1 AND work_date=?2",params![item.task_id,work_date],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
        let instance = if let Some((id,removed)) = existing {
            if !removed { return Err(format!("所选日期已包含“{}”，没有重复安排",item.title)); }
            let (parent,top,child,code) = if item.parent_task_id.is_some() {
                let parent = root_instance.clone().ok_or("待办组缺少父任务")?;
                let child: i64 = tx.query_row("SELECT COALESCE(MAX(child_no),0)+1 FROM task_day_instances WHERE parent_instance_id=?1",[&parent],|r|r.get(0)).map_err(|e|e.to_string())?;
                (Some(parent),root_top,Some(child),format!("#{root_top}.{child}"))
            } else {
                root_top = tx.query_row("SELECT COALESCE(MAX(top_level_no),0)+1 FROM task_day_instances WHERE work_date=?1",[work_date],|r|r.get(0)).map_err(|e|e.to_string())?;
                (None,root_top,None,format!("#{root_top}"))
            };
            tx.execute("UPDATE task_day_instances SET parent_instance_id=?1,top_level_no=?2,child_no=?3,display_code=?4,sort_order=?5,planned_start_minute=NULL,planned_end_minute=NULL,updated_at_utc=?6 WHERE id=?7",params![parent,top,child,code,top*1000+child.unwrap_or(0),now,id]).map_err(|e|e.to_string())?;
            tx.execute("DELETE FROM inbox_removed_instances WHERE instance_id=?1",[&id]).map_err(|e|e.to_string())?;
            id
        } else {
            let (parent,top,child,code) = if item.parent_task_id.is_some() {
                let parent = root_instance.clone().ok_or("待办组缺少父任务")?;
                let child: i64 = tx.query_row("SELECT COALESCE(MAX(child_no),0)+1 FROM task_day_instances WHERE parent_instance_id=?1",[&parent],|r|r.get(0)).map_err(|e|e.to_string())?;
                (Some(parent),root_top,Some(child),format!("#{root_top}.{child}"))
            } else {
                root_top = tx.query_row("SELECT COALESCE(MAX(top_level_no),0)+1 FROM task_day_instances WHERE work_date=?1",[work_date],|r|r.get(0)).map_err(|e|e.to_string())?;
                (None,root_top,None,format!("#{root_top}"))
            };
            let status: String = tx.query_row("SELECT status FROM tasks WHERE id=?1",[&item.task_id],|r|r.get(0)).map_err(|e|e.to_string())?;
            let id=db::new_id();
            tx.execute("INSERT INTO task_day_instances(id,task_id,work_date,parent_instance_id,carry_from_instance_id,display_code,top_level_no,child_no,importance,urgency,day_status,sort_order,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13)",params![id,item.task_id,work_date,parent,source,code,top,child,item.importance,item.urgency,status,top*1000+child.unwrap_or(0),now]).map_err(|e|e.to_string())?;
            id
        };
        if item.parent_task_id.is_none() { root_instance=Some(instance); }
        tx.execute("DELETE FROM task_inbox WHERE task_id=?1",[&item.task_id]).map_err(|e|e.to_string())?;
        db::append_event(&tx,"task.inbox_scheduled","task",&item.task_id,work_date,"summary",&format!("从待办箱安排：{}",item.title),None,&db::new_id())?;
    }
    tx.commit().map_err(|e|e.to_string())?;
    list_core(connection)
}

#[tauri::command] pub fn list_inbox(database: State<'_,Database>) -> Result<Vec<InboxTask>,String> { let c=database.0.lock().map_err(|_|"数据库繁忙")?; list_core(&c) }
#[tauri::command] pub fn create_inbox_task(database: State<'_,Database>, title:String) -> Result<Vec<InboxTask>,String> { let mut c=database.0.lock().map_err(|_|"数据库繁忙")?; create_core(&mut c,&title) }
#[tauri::command(rename_all="camelCase")] pub fn move_task_to_inbox(database: State<'_,Database>, instance_id:String, work_date:String) -> Result<Vec<InboxTask>,String> { let mut c=database.0.lock().map_err(|_|"数据库繁忙")?; move_core(&mut c,&instance_id,&work_date) }
#[tauri::command(rename_all="camelCase")] pub fn schedule_inbox_task(database: State<'_,Database>, group_id:String, work_date:String) -> Result<Vec<InboxTask>,String> { let mut c=database.0.lock().map_err(|_|"数据库繁忙")?; schedule_core(&mut c,&group_id,&work_date) }
#[tauri::command(rename_all="camelCase")] pub fn list_historical_unfinished(database: State<'_,Database>, before_date:String) -> Result<Vec<HistoricalUnfinishedTask>,String> { let c=database.0.lock().map_err(|_|"数据库繁忙")?; list_historical_core(&c,&before_date) }
#[tauri::command(rename_all="camelCase")] pub fn reschedule_historical_task(database: State<'_,Database>, source_instance_id:String, target_date:String) -> Result<Vec<HistoricalUnfinishedTask>,String> { let mut c=database.0.lock().map_err(|_|"数据库繁忙")?; let today=Local::now().format("%Y-%m-%d").to_string(); reschedule_historical_core(&mut c,&source_instance_id,&target_date,&today) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{commands,model::CreateTaskInput};
    fn connection()->Connection { let c=Connection::open_in_memory().unwrap(); db::initialize(&c).unwrap(); c }
    #[test] fn capture_then_schedule_keeps_permanent_id_and_prevents_duplicates() {
        let mut c=connection(); let date=Local::now().format("%Y-%m-%d").to_string();
        let items=create_core(&mut c,"尚未定日期的想法").unwrap(); let id=items[0].task_id.clone();
        assert!(db::read_day(&c,&date).unwrap().tasks.is_empty());
        schedule_core(&mut c,&id,&date).unwrap();
        assert_eq!(db::read_day(&c,&date).unwrap().tasks[0].permanent_task_id,id);
        assert!(schedule_core(&mut c,&id,&date).is_err());
    }
    #[test] fn moving_and_restoring_a_parent_keeps_children_and_events() {
        let mut c=connection(); let date=Local::now().format("%Y-%m-%d").to_string();
        let input=|title:&str,parent_id|CreateTaskInput { work_date:date.clone(),title:title.into(),parent_id,importance:"important".into(),urgency:"urgent".into(),planned_start:None,planned_end:None };
        let day=commands::create_task_core(&mut c,input("父任务",None)).unwrap(); let root=day.tasks[0].clone();
        commands::create_task_core(&mut c,input("子任务",Some(root.id.clone()))).unwrap();
        assert_eq!(move_core(&mut c,&root.id,&date).unwrap().len(),2);
        assert!(db::read_day(&c,&date).unwrap().tasks.is_empty());
        assert_eq!(db::read_day(&c,&date).unwrap().timeline.len(),3);
        schedule_core(&mut c,&root.permanent_task_id,&date).unwrap();
        let day=db::read_day(&c,&date).unwrap();
        assert_eq!(day.tasks.len(),2); assert_eq!(day.tasks[1].parent_id.as_deref(),Some(root.id.as_str()));
    }
    #[test] fn separately_parked_child_is_promoted_when_restored() {
        let mut c=connection(); let date=Local::now().format("%Y-%m-%d").to_string();
        let input=|title:&str,parent_id|CreateTaskInput { work_date:date.clone(),title:title.into(),parent_id,importance:"important".into(),urgency:"urgent".into(),planned_start:None,planned_end:None };
        let day=commands::create_task_core(&mut c,input("父任务",None)).unwrap(); let root=day.tasks[0].clone();
        let day=commands::create_task_core(&mut c,input("子任务",Some(root.id.clone()))).unwrap(); let child=day.tasks[1].clone();
        move_core(&mut c,&child.id,&date).unwrap();
        move_core(&mut c,&root.id,&date).unwrap();
        schedule_core(&mut c,&child.permanent_task_id,&date).unwrap();
        let day=db::read_day(&c,&date).unwrap();
        assert_eq!(day.tasks.len(),1); assert!(day.tasks[0].parent_id.is_none());
        assert_eq!(day.tasks[0].permanent_task_id,child.permanent_task_id);
    }
    #[test] fn active_focus_blocks_parking_without_partial_writes() {
        let mut c=connection(); let date=Local::now().format("%Y-%m-%d").to_string();
        let day=commands::create_task_core(&mut c,CreateTaskInput { work_date:date.clone(),title:"专注任务".into(),parent_id:None,importance:"important".into(),urgency:"urgent".into(),planned_start:None,planned_end:None }).unwrap();
        let id=day.tasks[0].id.clone();
        commands::start_focus_core(&mut c,crate::model::StartFocusInput { work_date:date.clone(),task_id:id.clone(),planned_seconds:1500 }).unwrap();
        assert!(move_core(&mut c,&id,&date).is_err());
        assert!(list_core(&c).unwrap().is_empty());
        assert_eq!(db::read_day(&c,&date).unwrap().tasks.len(),1);
        assert!(db::read_day(&c,&date).unwrap().focus.is_some());
    }
    #[test] fn historical_parent_reschedule_keeps_ids_and_omits_completed_children() {
        let mut c=connection();
        let today=Local::now().date_naive(); let old=(today-chrono::Duration::days(2)).to_string(); let target=today.to_string();
        let input=|title:&str,parent_id|CreateTaskInput { work_date:old.clone(),title:title.into(),parent_id,importance:"important".into(),urgency:"urgent".into(),planned_start:None,planned_end:None };
        let parent=commands::create_task_core(&mut c,input("历史父任务",None)).unwrap().tasks[0].clone();
        let done=commands::create_task_core(&mut c,input("已完成子任务",Some(parent.id.clone()))).unwrap().tasks[1].clone();
        let open=commands::create_task_core(&mut c,input("未完成子任务",Some(parent.id.clone()))).unwrap().tasks[2].clone();
        commands::set_task_status_core(&mut c,crate::model::SetTaskStatusInput{work_date:old.clone(),instance_id:done.id,status:"completed".into()}).unwrap();
        let historical=list_historical_core(&c,&target).unwrap();
        assert_eq!(historical.iter().map(|task|task.title.as_str()).collect::<Vec<_>>(),vec!["历史父任务","未完成子任务"]);
        reschedule_historical_core(&mut c,&parent.id,&target,&target).unwrap();
        let next=db::read_day(&c,&target).unwrap();
        assert_eq!(next.tasks.len(),2);
        assert_eq!(next.tasks[0].permanent_task_id,parent.permanent_task_id);
        assert_eq!(next.tasks[1].permanent_task_id,open.permanent_task_id);
        assert_eq!(next.tasks[1].parent_id.as_deref(),Some(next.tasks[0].id.as_str()));
        assert!(list_historical_core(&c,&target).unwrap().is_empty());
        assert_eq!(db::read_day(&c,&old).unwrap().timeline.iter().filter(|event|event.event_type=="task.historical_rescheduled").count(),1);
    }
    #[test] fn historical_reschedule_is_atomic_when_destination_already_contains_task() {
        let mut c=connection();
        let today=Local::now().date_naive(); let old=(today-chrono::Duration::days(1)).to_string(); let target=today.to_string();
        let task=commands::create_task_core(&mut c,CreateTaskInput { work_date:old.clone(),title:"重复安排检查".into(),parent_id:None,importance:"important".into(),urgency:"urgent".into(),planned_start:None,planned_end:None }).unwrap().tasks[0].clone();
        c.execute("INSERT INTO task_day_instances(id,task_id,work_date,display_code,top_level_no,importance,urgency,sort_order,created_at_utc,updated_at_utc) VALUES('existing',?1,?2,'#1',1,'important','urgent',1000,'now','now')",params![task.permanent_task_id,target]).unwrap();
        assert!(reschedule_historical_core(&mut c,&task.id,&target,&target).is_err());
        assert_eq!(c.query_row("SELECT day_status FROM task_day_instances WHERE id=?1",[task.id],|row|row.get::<_,String>(0)).unwrap(),"not_started");
    }
    #[test] fn repeating_goal_task_remains_visible_but_requires_growth_planning() {
        let mut c=connection();
        let today=Local::now().date_naive(); let old=(today-chrono::Duration::days(1)).to_string(); let target=today.to_string();
        let task=commands::create_task_core(&mut c,CreateTaskInput { work_date:old.clone(),title:"重复目标任务".into(),parent_id:None,importance:"important".into(),urgency:"urgent".into(),planned_start:None,planned_end:None }).unwrap().tasks[0].clone();
        c.execute_batch("INSERT INTO long_term_goals(id,title,cycle_days,start_date,created_at_utc,updated_at_utc) VALUES('g','目标',30,'2026-01-01','now','now'); INSERT INTO goal_phases(id,goal_id,title,start_date,end_date,created_at_utc,updated_at_utc) VALUES('p','g','阶段','2026-01-01','2099-12-31','now','now'); INSERT INTO goal_actions(id,phase_id,title,action_kind,required,target_count,created_at_utc,updated_at_utc) VALUES('a','p','任务','repeating',1,2,'now','now');").unwrap();
        c.execute("INSERT INTO goal_action_occurrences(id,action_id,task_id,scheduled_date,action_kind) VALUES('o','a',?1,?2,'repeating')",params![task.permanent_task_id,old]).unwrap();
        let historical=list_historical_core(&c,&target).unwrap();
        assert_eq!(historical.len(),1); assert!(!historical[0].reschedulable);
        assert!(historical[0].blocked_reason.as_deref().unwrap().contains("成长"));
        assert!(reschedule_historical_core(&mut c,&task.id,&target,&target).is_err());
    }
}
