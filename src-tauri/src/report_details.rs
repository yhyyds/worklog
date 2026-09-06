use chrono::{Duration, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;

// Same plan membership and leaf-task definition for summary and drill-down.
pub const PLAN_FILTER: &str = "d.day_status<>'cancelled' AND (d.day_status='completed' OR NOT EXISTS(SELECT 1 FROM inbox_removed_instances r WHERE r.instance_id=d.id)) AND NOT EXISTS(SELECT 1 FROM task_day_instances child WHERE child.parent_instance_id=d.id AND child.day_status<>'cancelled' AND (child.day_status='completed' OR NOT EXISTS(SELECT 1 FROM inbox_removed_instances r WHERE r.instance_id=child.id)))";

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HabitDetail {
    pub id: String,
    pub title: String,
    pub days: Vec<String>,
    pub completed: i64,
    pub missed: i64,
    pub prerequisite_missed: i64,
    pub pending: i64,
    pub current_streak: Option<i64>,
    pub longest_streak: i64,
    pub week_longest_streak: i64,
    pub breaks: i64,
    pub previous_completed: i64,
    pub previous_reviewed: i64,
    pub streak_through: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportTask {
    pub id: String,
    pub title: String,
    pub date: String,
    pub status: String,
    pub importance: String,
    pub urgency: String,
    pub carried: bool,
}

#[derive(Debug, Serialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FocusDetail {
    pub sessions: i64,
    pub completed_sessions: i64,
    pub abandoned_sessions: i64,
    pub average_minutes: i64,
    pub longest_minutes: i64,
    pub pauses: i64,
    pub switches: i64,
}

pub fn tasks(connection: &Connection, start: NaiveDate, end: NaiveDate) -> Result<Vec<ReportTask>, String> {
    let mut statement = connection.prepare(&format!("SELECT d.id,t.title,d.work_date,d.day_status,d.importance,d.urgency,d.carry_from_instance_id IS NOT NULL FROM task_day_instances d JOIN tasks t ON t.id=d.task_id WHERE d.work_date BETWEEN ?1 AND ?2 AND {PLAN_FILTER} ORDER BY d.work_date,d.sort_order,d.id")).map_err(|e|e.to_string())?;
    let rows = statement.query_map(params![start.to_string(),end.to_string()],|r|Ok(ReportTask {id:r.get(0)?,title:r.get(1)?,date:r.get(2)?,status:r.get(3)?,importance:r.get(4)?,urgency:r.get(5)?,carried:r.get(6)?})).map_err(|e|e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}

pub fn focus(connection: &Connection, start: NaiveDate, end: NaiveDate) -> Result<FocusDetail, String> {
    let mut result = connection.query_row(
        "SELECT COUNT(*),COALESCE(SUM(status='completed'),0),COALESCE(SUM(status='abandoned'),0),COALESCE(AVG(seconds),0),COALESCE(MAX(seconds),0) FROM (SELECT f.status,COALESCE(SUM(s.allocated_seconds),0) seconds FROM focus_sessions f LEFT JOIN focus_segments s ON s.focus_session_id=f.id WHERE f.work_date BETWEEN ?1 AND ?2 AND f.status IN ('completed','abandoned') GROUP BY f.id)",
        params![start.to_string(),end.to_string()],|r| Ok(FocusDetail {sessions:r.get(0)?,completed_sessions:r.get(1)?,abandoned_sessions:r.get(2)?,average_minutes:(r.get::<_,f64>(3)?/60.).round() as i64,longest_minutes:(r.get::<_,i64>(4)?+30)/60,..Default::default()}),
    ).map_err(|e|e.to_string())?;
    (result.pauses,result.switches) = connection.query_row("SELECT COALESCE(SUM(event_type='focus.paused'),0),COALESCE(SUM(event_type='focus.task_switched'),0) FROM events WHERE work_date BETWEEN ?1 AND ?2",params![start.to_string(),end.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
    Ok(result)
}

// Unknown dates neither prove a streak nor a break. A break is a confirmed
// completed day immediately followed by a confirmed unsuccessful day.
fn streaks(rows: &[(NaiveDate,bool)], start: NaiveDate, end: NaiveDate) -> (i64,i64,i64,Option<i64>) {
    let mut last = None;
    let mut run = 0; let mut longest = 0; let mut week_run = 0; let mut week_longest = 0; let mut breaks = 0;
    for &(date,done) in rows.iter().filter(|(date,_)|*date<=end) {
        let adjacent = last.is_some_and(|previous| previous + Duration::days(1) == date);
        if !adjacent { run=0; week_run=0; }
        if done {
            run+=1;
            if date>=start { week_run+=1; }
            longest=longest.max(run); week_longest=week_longest.max(week_run);
        } else {
            if date>=start && run>0 { breaks+=1; }
            run=0; week_run=0;
        }
        last=Some(date);
    }
    (longest,week_longest,breaks,if last==Some(end) {Some(run)} else {None})
}

pub fn habits(connection: &Connection, start: NaiveDate, end: NaiveDate, today: NaiveDate) -> Result<Vec<HabitDetail>,String> {
    let mut query=connection.prepare("SELECT id,title,start_date,archived_date FROM habits WHERE start_date<=?1 AND (archived_date IS NULL OR archived_date>?2) ORDER BY sort_order,created_at_utc,id").map_err(|e|e.to_string())?;
    let defs=query.query_map(params![end.to_string(),start.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    let mut query=connection.prepare("SELECT habit_id,occurrence_date,raw_completed,effective_completed FROM habit_occurrences WHERE occurrence_date<=?1 ORDER BY occurrence_date").map_err(|e|e.to_string())?;
    let records=query.query_map([end.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,bool>(2)?,r.get::<_,bool>(3)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    let mut grouped: HashMap<String,Vec<(NaiveDate,bool,bool)>>=HashMap::new();
    for (id,date,raw,effective) in records {
        let date=NaiveDate::parse_from_str(&date,"%Y-%m-%d").map_err(|e|e.to_string())?;
        grouped.entry(id).or_default().push((date,raw,effective));
    }
    defs.into_iter().map(|(id,title,first,archived)| {
        let first=NaiveDate::parse_from_str(&first,"%Y-%m-%d").map_err(|e|e.to_string())?;
        let archived=archived.map(|s|NaiveDate::parse_from_str(&s,"%Y-%m-%d").map_err(|e|e.to_string())).transpose()?;
        let rows=grouped.remove(&id).unwrap_or_default();
        let lookup: HashMap<_,_>=rows.iter().map(|&(date,raw,done)|(date,(raw,done))).collect();
        let due = |date|date>=first && archived.map_or(true,|last|date<last);
        let days: Vec<String>=(0..7).map(|offset| {
            let date=start+Duration::days(offset);
            if !due(date) { "inactive" } else if date>=today || date>end { "upcoming" }
            else { match lookup.get(&date) {Some((_,true))=>"done",Some((true,false))=>"prerequisite",Some((false,false))=>"missed",None=>"pending"} }.into()
        }).collect();
        let cutoff=end.min(today-Duration::days(1)).min(archived.map(|d|d-Duration::days(1)).unwrap_or(end));
        let history: Vec<_>=rows.iter().filter(|(d,_,_)|due(*d)).map(|&(date,_,done)|(date,done)).collect();
        let (longest_streak,week_longest_streak,breaks,current_streak)=streaks(&history,start,cutoff);
        let previous_start=start-Duration::days(7); let previous_end=end-Duration::days(7);
        let previous:Vec<_>=rows.iter().filter(|(d,_,_)|*d>=previous_start && *d<=previous_end && due(*d)).collect();
        Ok(HabitDetail {id,title,completed:days.iter().filter(|s|*s=="done").count() as i64,missed:days.iter().filter(|s|*s=="missed").count() as i64,prerequisite_missed:days.iter().filter(|s|*s=="prerequisite").count() as i64,pending:days.iter().filter(|s|*s=="pending").count() as i64,current_streak,longest_streak,week_longest_streak,breaks,previous_completed:previous.iter().filter(|(_,_,done)|*done).count() as i64,previous_reviewed:previous.len() as i64,streak_through:if cutoff>=first {Some(cutoff.to_string())} else {None},days})
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn date(day:u32)->NaiveDate {NaiveDate::from_ymd_opt(2026,9,day).unwrap()}
    #[test] fn streak_spans_weeks_and_counts_only_confirmed_breaks() {
        let rows=vec![(date(5),true),(date(6),true),(date(7),true),(date(8),false),(date(9),false),(date(10),true),(date(11),true)];
        assert_eq!(streaks(&rows,date(7),date(11)),(3,2,1,Some(2)));
    }
    #[test] fn unknown_day_does_not_create_a_break_or_bridge_a_streak() {
        let rows=vec![(date(6),true),(date(8),false),(date(9),true)];
        assert_eq!(streaks(&rows,date(7),date(10)),(1,1,0,None));
    }
    #[test] fn habits_distinguish_missing_prerequisite_from_not_reviewed() {
        let c=Connection::open_in_memory().unwrap(); crate::db::initialize(&c).unwrap();
        c.execute("INSERT INTO habits(id,title,start_date,created_at_utc,updated_at_utc) VALUES('h','早睡','2026-09-01','now','now')",[]).unwrap();
        c.execute("INSERT INTO habit_occurrences VALUES('h','2026-09-07',1,1,'[]','now'),('h','2026-09-08',1,0,'[]','now')",[]).unwrap();
        let result=habits(&c,date(7),date(10),date(10)).unwrap(); let h=&result[0];
        assert_eq!(h.days,vec!["done","prerequisite","pending","upcoming","upcoming","upcoming","upcoming"]);
        assert_eq!((h.completed,h.missed,h.prerequisite_missed,h.pending,h.breaks),(1,0,1,1,1));
        assert_eq!(h.current_streak,None);
    }
    #[test] fn historical_report_does_not_include_later_streaks() {
        let rows=vec![(date(7),true),(date(8),true),(date(9),false),(date(10),true),(date(11),true),(date(12),true)];
        assert_eq!(streaks(&rows,date(7),date(9)),(2,2,1,Some(0)));
    }
    #[test] fn focus_averages_sessions_not_segments_and_excludes_running_rounds() {
        let c=Connection::open_in_memory().unwrap(); crate::db::initialize(&c).unwrap();
        c.execute("INSERT INTO tasks(id,title,created_at_utc,updated_at_utc) VALUES('t','任务','now','now')",[]).unwrap();
        c.execute("INSERT INTO task_day_instances(id,task_id,work_date,display_code,top_level_no,importance,urgency,created_at_utc,updated_at_utc) VALUES('d','t','2026-09-07','#1',1,'important','urgent','now','now')",[]).unwrap();
        c.execute("INSERT INTO focus_sessions(id,work_date,status,primary_task_instance_id,planned_seconds,remaining_seconds,started_at_utc,active_guard) VALUES('f','2026-09-07','completed','d',1800,0,'now',NULL),('g','2026-09-07','abandoned','d',1800,1200,'now',NULL),('r','2026-09-07','running','d',1800,1800,'now',1)",[]).unwrap();
        c.execute("INSERT INTO focus_segments(id,focus_session_id,task_instance_id,started_at_utc,allocated_seconds) VALUES('s1','f','d','now',600),('s2','f','d','now',1200),('s3','g','d','now',600)",[]).unwrap();
        let result=focus(&c,date(7),date(13)).unwrap();
        assert_eq!((result.sessions,result.completed_sessions,result.abandoned_sessions,result.average_minutes,result.longest_minutes),(2,1,1,20,30));
    }
}
