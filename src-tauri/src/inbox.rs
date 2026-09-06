use crate::{db, Database};
use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
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

fn list_core(connection: &Connection) -> Result<Vec<InboxTask>, String> {
    let mut query = connection.prepare("SELECT t.id,t.title,i.group_id,i.parent_task_id,i.importance,i.urgency,i.entered_at_utc FROM task_inbox i JOIN tasks t ON t.id=i.task_id ORDER BY i.entered_at_utc,i.group_id,i.parent_task_id IS NOT NULL,t.created_at_utc").map_err(|e| e.to_string())?;
    let rows = query.query_map([], |r| Ok(InboxTask { task_id:r.get(0)?, title:r.get(1)?, group_id:r.get(2)?, parent_task_id:r.get(3)?, importance:r.get(4)?, urgency:r.get(5)?, entered_at:r.get(6)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())
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
}
