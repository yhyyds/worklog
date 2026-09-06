use crate::{
    categories::{self, Catalog},
    db, report_details, reports, Database,
};
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use tauri::State;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub planned: i64,
    pub completed: i64,
    pub focus_minutes: i64,
    pub habit_done: i64,
    pub habit_reviewed: i64,
    pub habit_pending: i64,
    pub habit_breaks: i64,
    pub habit_best_streak: i64,
    pub goal_required: i64,
    pub goal_done: i64,
    pub goal_optional_done: i64,
    pub goal_optional: i64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySummary {
    pub name: String,
    pub color: String,
    pub counts: Counts,
}
#[derive(Serialize)]
pub struct DaySummary {
    pub date: String,
    pub counts: Counts,
}
#[derive(Serialize)]
pub struct NamedSummary {
    pub name: String,
    pub kind: String,
    pub done: i64,
    pub total: i64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub week_start: String,
    pub week_end: String,
    pub through: String,
    pub sharing: bool,
    pub counts: Counts,
    pub categories: Vec<CategorySummary>,
    pub daily: Vec<DaySummary>,
    pub history: Vec<DaySummary>,
    pub names: Vec<NamedSummary>,
    pub quote: reports::ReportQuote,
}
#[derive(Clone)]
struct Label {
    key: String,
    name: String,
    color: String,
    mode: String,
    share_name: bool,
}
fn label(catalog: &Catalog, id: &str) -> Label {
    let assignment = catalog.classifications.iter().find(|c| c.entity_id == id);
    let category = assignment
        .and_then(|a| a.category_id.as_ref())
        .and_then(|id| catalog.categories.iter().find(|c| &c.id == id));
    match category {
        Some(c) => Label {
            key: c.id.clone(),
            name: c.name.clone(),
            color: c.color.clone(),
            mode: c.share_mode.clone(),
            share_name: assignment.is_some_and(|a| a.share_name),
        },
        None => Label {
            key: String::new(),
            name: "未分类".into(),
            color: "#87938b".into(),
            mode: "anonymous".into(),
            share_name: false,
        },
    }
}
fn allowed(l: &Label, sharing: bool) -> bool {
    !sharing || l.mode != "excluded"
}
fn apply(
    total: &mut Counts,
    groups: &mut BTreeMap<String, (Label, Counts)>,
    l: &Label,
    f: impl Fn(&mut Counts),
) {
    f(total);
    f(&mut groups
        .entry(l.key.clone())
        .or_insert_with(|| (l.clone(), Counts::default()))
        .1);
}
struct Mappings {
    tasks: HashMap<String, (String, bool)>,
    instances: HashMap<String, String>,
}
fn mappings(c: &Connection) -> Result<Mappings, String> {
    let mut q=c.prepare("SELECT t.id,COALESCE(p.goal_id,pp.goal_id,''),COALESCE(a.required,pa.required,0) FROM tasks t LEFT JOIN goal_action_occurrences o ON o.task_id=t.id LEFT JOIN goal_actions a ON a.id=o.action_id LEFT JOIN goal_phases p ON p.id=a.phase_id LEFT JOIN goal_action_occurrences po ON po.task_id=t.parent_task_id LEFT JOIN goal_actions pa ON pa.id=po.action_id LEFT JOIN goal_phases pp ON pp.id=pa.phase_id").map_err(|e|e.to_string())?;
    let tasks = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, bool>(2)?),
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<HashMap<_, _>, _>>()
        .map_err(|e| e.to_string())?;
    let mut q = c
        .prepare("SELECT id,task_id FROM task_day_instances")
        .map_err(|e| e.to_string())?;
    let instances = q
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<HashMap<_, _>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(Mappings { tasks, instances })
}
fn period(
    c: &Connection,
    catalog: &Catalog,
    maps: &Mappings,
    start: NaiveDate,
    end: NaiveDate,
    today: NaiveDate,
    sharing: bool,
) -> Result<(Counts, BTreeMap<String, (Label, Counts)>, Vec<NamedSummary>), String> {
    let mut total = Counts::default();
    let mut groups = BTreeMap::new();
    let mut names = vec![];
    let mut named_goals: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for t in report_details::tasks(c, start, end)? {
        let (goal, required) = maps
            .instances
            .get(&t.id)
            .and_then(|id| maps.tasks.get(id))
            .cloned()
            .unwrap_or_default();
        let l = label(catalog, &goal);
        if !allowed(&l, sharing) {
            continue;
        }
        let done = i64::from(t.status == "completed");
        apply(&mut total, &mut groups, &l, |v| {
            v.planned += 1;
            v.completed += done;
            if !goal.is_empty() {
                if required {
                    v.goal_required += 1;
                    v.goal_done += done;
                } else {
                    v.goal_optional += 1;
                    v.goal_optional_done += done;
                }
            }
        });
        if !goal.is_empty() && (!sharing || (l.mode == "public" && l.share_name)) {
            let e = named_goals.entry(goal).or_default();
            e.0 += done;
            e.1 += 1;
        }
    }
    for h in report_details::habits(c, start, end, today)? {
        let l = label(catalog, &h.id);
        if !allowed(&l, sharing) {
            continue;
        }
        let days = (end - start).num_days() + 1;
        let statuses = &h.days[..days.min(7) as usize];
        let done = statuses.iter().filter(|s| s.as_str() == "done").count() as i64;
        let reviewed = statuses
            .iter()
            .filter(|s| ["done", "missed", "prerequisite"].contains(&s.as_str()))
            .count() as i64;
        let pending = statuses.iter().filter(|s| s.as_str() == "pending").count() as i64;
        apply(&mut total, &mut groups, &l, |v| {
            v.habit_done += done;
            v.habit_reviewed += reviewed;
            v.habit_pending += pending;
            v.habit_breaks += h.breaks;
            v.habit_best_streak = v.habit_best_streak.max(h.week_longest_streak);
        });
        if !sharing || (l.mode == "public" && l.share_name) {
            names.push(NamedSummary {
                name: h.title,
                kind: "打卡".into(),
                done,
                total: reviewed,
            });
        }
    }
    let mut q=c.prepare("SELECT s.task_instance_id,SUM(s.allocated_seconds) FROM focus_segments s JOIN focus_sessions f ON f.id=s.focus_session_id WHERE f.work_date BETWEEN ?1 AND ?2 GROUP BY s.task_instance_id").map_err(|e|e.to_string())?;
    let rows = q
        .query_map(params![start.to_string(), end.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut seconds: BTreeMap<String, (Label, i64)> = BTreeMap::new();
    for (instance, secs) in rows {
        let goal = maps
            .instances
            .get(&instance)
            .and_then(|id| maps.tasks.get(id))
            .map(|p| p.0.as_str())
            .unwrap_or("");
        let l = label(catalog, goal);
        if allowed(&l, sharing) {
            seconds.entry(l.key.clone()).or_insert((l, 0)).1 += secs;
        }
    }
    // Round only after aggregation; excluded segments cannot affect visible totals.
    let total_seconds: i64 = seconds.values().map(|(_, s)| *s).sum();
    total.focus_minutes = (total_seconds + 30) / 60;
    for (key, (l, secs)) in seconds {
        groups
            .entry(key)
            .or_insert((l, Counts::default()))
            .1
            .focus_minutes = (secs + 30) / 60;
    }
    for (id, (done, count)) in named_goals {
        let name = c
            .query_row("SELECT title FROM long_term_goals WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        names.push(NamedSummary {
            name,
            kind: "目标任务".into(),
            done,
            total: count,
        });
    }
    Ok((total, groups, names))
}
fn overview_at(
    c: &Connection,
    start: &str,
    sharing: bool,
    today: NaiveDate,
) -> Result<Overview, String> {
    let start =
        NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|_| "请选择有效日期".to_string())?;
    if start.weekday() != Weekday::Mon || start > today {
        return Err("请选择不晚于今天的周一".into());
    }
    let end = (start + Duration::days(6)).min(today);
    let catalog = categories::catalog(c)?;
    let maps = mappings(c)?;
    let (counts, groups, names) = period(c, &catalog, &maps, start, end, today, sharing)?;
    let categories = groups
        .into_values()
        .filter(|(l, _)| !sharing || l.mode == "public")
        .map(|(l, counts)| CategorySummary {
            name: l.name,
            color: l.color,
            counts,
        })
        .collect();
    let mut daily = vec![];
    for offset in 0..7 {
        let d = start + Duration::days(offset);
        let counts = if d > end {
            Counts::default()
        } else {
            period(c, &catalog, &maps, d, d, today, sharing)?.0
        };
        daily.push(DaySummary {
            date: d.to_string(),
            counts,
        });
    }
    let mut history = vec![];
    for offset in (1..=4).rev() {
        let days = Duration::days(offset * 7);
        let counts = period(c, &catalog, &maps, start - days, end - days, today, sharing)?.0;
        history.push(DaySummary {
            date: (start - days).to_string(),
            counts,
        });
    }
    let scenario = if counts.planned > 0 && counts.completed >= counts.planned {
        "strong"
    } else if counts.planned == 0 && counts.habit_reviewed == 0 {
        "recovery"
    } else {
        "steady"
    };
    Ok(Overview {
        week_start: start.to_string(),
        week_end: (start + Duration::days(6)).to_string(),
        through: end.to_string(),
        sharing,
        counts,
        categories,
        daily,
        history,
        names,
        quote: reports::share_quote(&start.to_string(), scenario),
    })
}
#[tauri::command(rename_all = "camelCase")]
pub fn get_report_overview(
    database: State<'_, Database>,
    week_start: String,
    sharing: bool,
) -> Result<Overview, String> {
    let c = database.0.lock().map_err(|e| e.to_string())?;
    overview_at(&c, &week_start, sharing, Local::now().date_naive())
}
#[tauri::command]
pub fn get_share_preference(database: State<'_, Database>) -> Result<bool, String> {
    let c = database.0.lock().map_err(|e| e.to_string())?;
    let s: Option<String> = c
        .query_row(
            "SELECT value_json FROM app_settings WHERE key='weekly_share_mode'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(s.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(true))
}
#[tauri::command]
pub fn save_share_preference(database: State<'_, Database>, sharing: bool) -> Result<(), String> {
    let c = database.0.lock().map_err(|e| e.to_string())?;
    c.execute("INSERT INTO app_settings VALUES('weekly_share_mode',?1,?2) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at_utc=excluded.updated_at_utc",params![sharing.to_string(),db::now_iso()]).map_err(|e|e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        db::initialize(&c).unwrap();
        c.execute_batch("INSERT INTO growth_categories VALUES('c','非常私密分类','#123456','excluded'),('p','生活','#33aa77','public'); INSERT INTO habits(id,title,start_date,created_at_utc,updated_at_utc) VALUES('h','私密打卡','2026-09-01','now','now'),('v','公开但未授权名称','2026-09-01','now','now'); INSERT INTO growth_classifications VALUES('h','habit','c',1),('v','habit','p',0); INSERT INTO habit_occurrences VALUES('h','2026-09-07',1,1,'[]','now'),('v','2026-09-07',0,0,'[]','now');").unwrap();
        c
    }
    #[test]
    fn excluded_names_colors_and_counts_never_reach_share_dto() {
        let c = fixture();
        let today = NaiveDate::from_ymd_opt(2026, 9, 8).unwrap();
        let r = overview_at(&c, "2026-09-07", true, today).unwrap();
        let json = serde_json::to_string(&r).unwrap();
        for secret in ["非常私密分类", "#123456", "私密打卡", "公开但未授权名称"]
        {
            assert!(!json.contains(secret));
        }
        assert_eq!(r.counts.habit_done, 0);
        assert_eq!(r.counts.habit_reviewed, 1);
        assert_eq!(r.categories.len(), 1);
        assert_eq!(r.categories[0].name, "生活");
        assert!(r.names.is_empty());
        assert_eq!(
            overview_at(&c, "2026-09-07", false, today)
                .unwrap()
                .counts
                .habit_done,
            1
        );
    }
    #[test]
    fn anonymous_is_total_only_and_explicit_public_names_are_opt_in() {
        let c = fixture();
        c.execute(
            "UPDATE growth_categories SET share_mode='anonymous' WHERE id='c'",
            [],
        )
        .unwrap();
        c.execute(
            "UPDATE growth_classifications SET share_name=1 WHERE entity_id='v'",
            [],
        )
        .unwrap();
        let r = overview_at(
            &c,
            "2026-09-07",
            true,
            NaiveDate::from_ymd_opt(2026, 9, 8).unwrap(),
        )
        .unwrap();
        assert_eq!(r.counts.habit_done, 1);
        assert_eq!(r.categories.len(), 1);
        assert_eq!(r.names.len(), 1);
        assert_eq!(r.names[0].name, "公开但未授权名称");
        assert!(!serde_json::to_string(&r).unwrap().contains("私密打卡"));
    }
    #[test]
    fn hidden_change_does_not_change_public_history_or_quote() {
        let c = fixture();
        let today = NaiveDate::from_ymd_opt(2026, 9, 8).unwrap();
        let before =
            serde_json::to_string(&overview_at(&c, "2026-09-07", true, today).unwrap()).unwrap();
        c.execute(
            "UPDATE habit_occurrences SET effective_completed=0 WHERE habit_id='h'",
            [],
        )
        .unwrap();
        let after =
            serde_json::to_string(&overview_at(&c, "2026-09-07", true, today).unwrap()).unwrap();
        assert_eq!(before, after);
    }
    #[test]
    fn excluded_goal_children_and_mixed_focus_segments_are_filtered_before_totals() {
        let c = fixture();
        c.execute_batch("INSERT INTO long_term_goals(id,title,cycle_days,start_date,created_at_utc,updated_at_utc) VALUES('g','隐私目标',30,'2026-09-01','now','now');
  INSERT INTO goal_phases(id,goal_id,title,start_date,end_date,created_at_utc,updated_at_utc) VALUES('phase','g','私人笔记','2026-09-01','2026-09-30','now','now');
  INSERT INTO goal_actions(id,phase_id,title,action_kind,required,target_count,created_at_utc,updated_at_utc) VALUES('a','phase','私密行动','one_off',1,1,'now','now');
  INSERT INTO growth_classifications VALUES('g','goal','c',1);
  INSERT INTO tasks(id,title,created_at_utc,updated_at_utc) VALUES('private','保密父任务','now','now'),('public','普通任务','now','now');
  INSERT INTO tasks(id,title,parent_task_id,created_at_utc,updated_at_utc) VALUES('child','保密子任务','private','now','now');
  INSERT INTO goal_action_occurrences VALUES('o','a','private','2026-09-07','one_off',1);
  INSERT INTO task_day_instances(id,task_id,work_date,display_code,top_level_no,importance,urgency,created_at_utc,updated_at_utc) VALUES('d','private','2026-09-07','#1',1,'important','urgent','now','now'),('v','public','2026-09-07','#2',2,'important','urgent','now','now');
  INSERT INTO task_day_instances(id,task_id,parent_instance_id,work_date,display_code,top_level_no,child_no,importance,urgency,created_at_utc,updated_at_utc) VALUES('cd','child','d','2026-09-07','#1.1',1,1,'important','urgent','now','now');
  INSERT INTO focus_sessions(id,work_date,status,primary_task_instance_id,planned_seconds,remaining_seconds,started_at_utc,active_guard) VALUES('f','2026-09-07','completed','v',4200,0,'2026-09-07T01:00:00Z',NULL);
  INSERT INTO focus_segments(id,focus_session_id,task_instance_id,started_at_utc,allocated_seconds) VALUES('s1','f','cd','now',3600),('s2','f','v','now',600);").unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 9, 8).unwrap();
        let public = overview_at(&c, "2026-09-07", true, today).unwrap();
        let private = overview_at(&c, "2026-09-07", false, today).unwrap();
        assert_eq!(
            (
                public.counts.planned,
                public.counts.focus_minutes,
                public.counts.goal_required
            ),
            (1, 10, 0)
        );
        assert_eq!(
            (
                private.counts.planned,
                private.counts.focus_minutes,
                private.counts.goal_required
            ),
            (2, 70, 1)
        );
        assert_eq!(public.daily[0].counts.focus_minutes, 10);
        let json = serde_json::to_string(&public).unwrap();
        for secret in ["保密", "私人笔记", "隐私目标", "01:00:00"] {
            assert!(!json.contains(secret));
        }
    }
}
