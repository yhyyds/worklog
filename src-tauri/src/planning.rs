use crate::{
    db,
    growth::{self, LongTermGoal},
    Database,
};
use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::State;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Occurrence {
    pub id: String,
    pub date: String,
    pub status: String,
    pub task_id: String,
}
pub fn occurrences(c: &Connection, id: &str) -> Result<Vec<Occurrence>, String> {
    let mut q=c.prepare("SELECT o.id,o.scheduled_date,t.status,o.task_id FROM goal_action_occurrences o JOIN tasks t ON t.id=o.task_id WHERE o.action_id=?1 AND o.active=1 ORDER BY o.scheduled_date").map_err(|e|e.to_string())?;
    let result = q
        .query_map([id], |r| {
            Ok(Occurrence {
                id: r.get(0)?,
                date: r.get(1)?,
                status: r.get(2)?,
                task_id: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}
pub fn no_carry_tasks(c: &Connection) -> Result<HashSet<String>, String> {
    let mut q=c.prepare("SELECT task_id FROM goal_action_occurrences WHERE action_kind='repeating' UNION SELECT t.id FROM tasks t JOIN goal_action_occurrences o ON o.task_id=t.parent_task_id WHERE o.action_kind='repeating'").map_err(|e|e.to_string())?;
    let result = q
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|e| e.to_string());
    result
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePlan {
    pub action_id: String,
    pub title: String,
    pub action_kind: String,
    pub required: bool,
    pub target_count: i64,
    pub importance: String,
    pub urgency: String,
    pub dates: Vec<String>,
}

// Retire plan membership, never delete a task UUID, focus session or event.
fn retire(c: &Connection, o: &Occurrence, today: &str) -> Result<(), String> {
    let touched:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM task_day_instances d WHERE d.task_id=?1 AND (d.work_date<?2 OR d.day_status<>'not_started' OR EXISTS(SELECT 1 FROM focus_segments s WHERE s.task_instance_id=d.id) OR EXISTS(SELECT 1 FROM focus_sessions f WHERE f.primary_task_instance_id=d.id) OR EXISTS(SELECT 1 FROM task_day_instances child WHERE child.parent_instance_id=d.id)))",params![o.task_id,today],|r|r.get(0)).map_err(|e|e.to_string())?;
    if touched {
        return Err("已有执行记录的安排会保留；只能移除今天及未来尚未开始的安排".into());
    }
    c.execute("INSERT OR IGNORE INTO goal_removed_instances SELECT id FROM task_day_instances WHERE task_id=?1",[&o.task_id]).map_err(|e|e.to_string())?;
    c.execute(
        "UPDATE task_day_instances SET day_status='cancelled',updated_at_utc=?2 WHERE task_id=?1",
        params![o.task_id, db::now_iso()],
    )
    .map_err(|e| e.to_string())?;
    c.execute(
        "UPDATE tasks SET status='cancelled',updated_at_utc=?2 WHERE id=?1",
        params![o.task_id, db::now_iso()],
    )
    .map_err(|e| e.to_string())?;
    c.execute("DELETE FROM task_inbox WHERE task_id=?1", [&o.task_id])
        .map_err(|e| e.to_string())?;
    c.execute(
        "UPDATE goal_action_occurrences SET active=0 WHERE id=?1",
        [&o.id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub(crate) fn save_core(c: &mut Connection, input: SavePlan) -> Result<Vec<LongTermGoal>, String> {
    let today = Local::now().date_naive().to_string();
    if input.title.trim().is_empty() {
        return Err("任务名称不能为空".into());
    }
    if !["one_off", "repeating"].contains(&input.action_kind.as_str())
        || !["important", "secondary"].contains(&input.importance.as_str())
        || !["urgent", "relaxed"].contains(&input.urgency.as_str())
    {
        return Err("任务类型或优先级无效".into());
    }
    let (baseline,start,end):(i64,String,String)=c.query_row("SELECT a.completed_count,p.start_date,p.end_date FROM goal_actions a JOIN goal_phases p ON p.id=a.phase_id LEFT JOIN goal_action_options x ON x.action_id=a.id WHERE a.id=?1 AND x.deleted_at IS NULL",[&input.action_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|"任务不存在或已删除".to_string())?;
    let dates: HashSet<_> = input.dates.iter().cloned().collect();
    if dates.len() != input.dates.len() {
        return Err("执行日期不能重复".into());
    }
    if !(1..=3660).contains(&input.target_count)
        || input.target_count < baseline + dates.len() as i64
    {
        return Err("计划次数不能少于已有完成次数与已安排次数之和".into());
    }
    let old = occurrences(c, &input.action_id)?;
    for date in &input.dates {
        if NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map(|d| d.to_string() != *date)
            .unwrap_or(true)
        {
            return Err("日期格式无效".into());
        }
        if !old.iter().any(|o| o.date == *date) && (date < &today || date < &start || date > &end) {
            return Err("新增安排请选择阶段内的今天或未来日期".into());
        }
    }
    let tx = c.transaction().map_err(|e| e.to_string())?;
    let now = db::now_iso();
    for o in &old {
        if !dates.contains(&o.date) {
            retire(&tx, o, &today)?;
        }
    }
    tx.execute("UPDATE goal_actions SET title=?2,action_kind=?3,required=?4,target_count=?5,updated_at_utc=?6 WHERE id=?1",params![input.action_id,input.title.trim(),input.action_kind,input.required,input.target_count,now]).map_err(|e|e.to_string())?;
    tx.execute("INSERT INTO goal_action_options(action_id,importance,urgency) VALUES(?1,?2,?3) ON CONFLICT(action_id) DO UPDATE SET importance=excluded.importance,urgency=excluded.urgency",params![input.action_id,input.importance,input.urgency]).map_err(|e|e.to_string())?;
    for date in &input.dates {
        if let Some(o) = old.iter().find(|o| o.date == *date) {
            // Completed/past task titles and priorities remain as their historical record.
            let editable:bool=tx.query_row("SELECT NOT EXISTS(SELECT 1 FROM task_day_instances d WHERE task_id=?1 AND (work_date<?2 OR day_status<>'not_started' OR EXISTS(SELECT 1 FROM focus_segments s WHERE s.task_instance_id=d.id) OR EXISTS(SELECT 1 FROM focus_sessions f WHERE f.primary_task_instance_id=d.id)))",params![o.task_id,today],|r|r.get(0)).map_err(|e|e.to_string())?;
            if editable {
                tx.execute(
                    "UPDATE tasks SET title=?2,updated_at_utc=?3 WHERE id=?1",
                    params![o.task_id, input.title.trim(), now],
                )
                .map_err(|e| e.to_string())?;
                tx.execute("UPDATE task_day_instances SET importance=?2,urgency=?3,updated_at_utc=?4 WHERE task_id=?1",params![o.task_id,input.importance,input.urgency,now]).map_err(|e|e.to_string())?;
                tx.execute(
                    "UPDATE task_inbox SET importance=?2,urgency=?3 WHERE task_id=?1",
                    params![o.task_id, input.importance, input.urgency],
                )
                .map_err(|e| e.to_string())?;
                tx.execute(
                    "UPDATE goal_action_occurrences SET action_kind=?2 WHERE id=?1",
                    params![o.id, input.action_kind],
                )
                .map_err(|e| e.to_string())?;
            }
            continue;
        }
        let task = db::new_id();
        let instance = db::new_id();
        let top: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(top_level_no),0)+1 FROM task_day_instances WHERE work_date=?1",
                [date],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO tasks(id,title,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?3)",
            params![task, input.title.trim(), now],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO task_day_instances(id,task_id,work_date,display_code,top_level_no,importance,urgency,sort_order,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",params![instance,task,date,format!("#{top}"),top,input.importance,input.urgency,top*1000,now]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO goal_action_occurrences(id,action_id,task_id,scheduled_date,action_kind) VALUES(?1,?2,?3,?4,?5)",params![db::new_id(),input.action_id,task,date,input.action_kind]).map_err(|e|e.to_string())?;
        db::append_event(
            &tx,
            "goal.task_scheduled",
            "task",
            &task,
            date,
            "detail",
            &format!("安排目标任务：{}", input.title.trim()),
            None,
            &db::new_id(),
        )?;
    }
    db::append_event(
        &tx,
        "goal.action_updated",
        "goal_action",
        &input.action_id,
        &today,
        "detail",
        &format!("修改目标任务：{}", input.title.trim()),
        None,
        &db::new_id(),
    )?;
    tx.commit().map_err(|e| e.to_string())?;
    growth::list_goals_core(c)
}
fn delete_core(c: &mut Connection, id: &str) -> Result<Vec<LongTermGoal>, String> {
    let today = Local::now().date_naive().to_string();
    let rows = occurrences(c, id)?;
    let tx = c.transaction().map_err(|e| e.to_string())?;
    let title: String = tx
        .query_row("SELECT title FROM goal_actions WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .map_err(|_| "任务不存在".to_string())?;
    // Keep started and historical occurrences; remove only pristine pending plans.
    for o in rows {
        if o.date >= today && o.status == "not_started" {
            let safe:bool=tx.query_row("SELECT NOT EXISTS(SELECT 1 FROM task_day_instances d WHERE d.task_id=?1 AND (d.work_date<?2 OR d.day_status<>'not_started' OR EXISTS(SELECT 1 FROM focus_segments s WHERE s.task_instance_id=d.id) OR EXISTS(SELECT 1 FROM focus_sessions f WHERE f.primary_task_instance_id=d.id) OR EXISTS(SELECT 1 FROM task_day_instances child WHERE child.parent_instance_id=d.id)))",params![o.task_id,today],|r|r.get(0)).map_err(|e|e.to_string())?;
            if safe {
                retire(&tx, &o, &today)?;
            }
        }
    }
    tx.execute("INSERT INTO goal_action_options(action_id,deleted_at) VALUES(?1,?2) ON CONFLICT(action_id) DO UPDATE SET deleted_at=excluded.deleted_at",params![id,db::now_iso()]).map_err(|e|e.to_string())?;
    db::append_event(
        &tx,
        "goal.action_deleted",
        "goal_action",
        id,
        &today,
        "detail",
        &format!("删除目标任务：{title}"),
        None,
        &db::new_id(),
    )?;
    tx.commit().map_err(|e| e.to_string())?;
    growth::list_goals_core(c)
}
#[tauri::command]
pub fn save_goal_action_plan(
    database: State<'_, Database>,
    input: SavePlan,
) -> Result<Vec<LongTermGoal>, String> {
    let mut c = database.0.lock().map_err(|e| e.to_string())?;
    save_core(&mut c, input)
}
#[tauri::command(rename_all = "camelCase")]
pub fn delete_goal_action(
    database: State<'_, Database>,
    action_id: String,
) -> Result<Vec<LongTermGoal>, String> {
    let mut c = database.0.lock().map_err(|e| e.to_string())?;
    delete_core(&mut c, &action_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Connection, SavePlan) {
        let c = Connection::open_in_memory().unwrap();
        db::initialize(&c).unwrap();
        let today = Local::now().date_naive().to_string();
        c.execute("INSERT INTO long_term_goals(id,title,cycle_days,start_date,created_at_utc,updated_at_utc) VALUES('g','目标',30,?1,'now','now')",[&today]).unwrap();
        c.execute("INSERT INTO goal_phases(id,goal_id,title,start_date,end_date,created_at_utc,updated_at_utc) VALUES('p','g','阶段',?1,'2099-12-31','now','now')",[&today]).unwrap();
        c.execute("INSERT INTO goal_actions(id,phase_id,title,action_kind,required,target_count,created_at_utc,updated_at_utc) VALUES('a','p','任务','repeating',1,2,'now','now')",[]).unwrap();
        (
            c,
            SavePlan {
                action_id: "a".into(),
                title: "新任务".into(),
                action_kind: "repeating".into(),
                required: true,
                target_count: 2,
                importance: "secondary".into(),
                urgency: "urgent".into(),
                dates: vec![today],
            },
        )
    }
    #[test]
    fn scheduling_is_idempotent_and_daily_completion_updates_goal() {
        let (mut c, input) = fixture();
        save_core(&mut c, input.clone()).unwrap();
        save_core(&mut c, input.clone()).unwrap();
        let d = db::read_day(&c, &input.dates[0]).unwrap();
        assert_eq!(d.tasks.len(), 1);
        assert_eq!(d.tasks[0].importance, "secondary");
        crate::commands::set_task_status_core(
            &mut c,
            crate::model::SetTaskStatusInput {
                work_date: input.dates[0].clone(),
                instance_id: d.tasks[0].id.clone(),
                status: "completed".into(),
            },
        )
        .unwrap();
        let g = growth::list_goals_core(&c).unwrap();
        assert_eq!(g[0].progress_percent, 50);
        assert_eq!(g[0].phases[0].actions[0].completed_count, 1);
        delete_core(&mut c, "a").unwrap();
        assert_eq!(
            db::read_day(&c, &input.dates[0]).unwrap().tasks[0].status,
            "completed"
        );
    }
    #[test]
    fn deletion_keeps_events_and_removes_unstarted_daily_plans() {
        let (mut c, input) = fixture();
        save_core(&mut c, input.clone()).unwrap();
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap();
        delete_core(&mut c, "a").unwrap();
        assert!(db::read_day(&c, &input.dates[0]).unwrap().tasks.is_empty());
        let after: i64 = c
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert!(after > n);
        assert!(growth::list_goals_core(&c).unwrap()[0].phases[0]
            .actions
            .is_empty());
    }
    #[test]
    fn invalid_plan_is_atomic_and_legacy_progress_is_retained() {
        let (mut c, mut input) = fixture();
        c.execute("UPDATE goal_actions SET completed_count=1 WHERE id='a'", [])
            .unwrap();
        save_core(&mut c, input.clone()).unwrap();
        assert_eq!(growth::list_goals_core(&c).unwrap()[0].progress_percent, 50);
        input.dates.push("2020-01-01".into());
        assert!(save_core(&mut c, input).is_err());
        assert_eq!(occurrences(&c, "a").unwrap().len(), 1);
        db::initialize(&c).unwrap();
        assert_eq!(growth::list_goals_core(&c).unwrap()[0].progress_percent, 50);
    }
}
