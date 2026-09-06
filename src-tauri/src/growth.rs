use crate::{db, Database};
use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use tauri::State;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Habit {
    pub id: String,
    pub title: String,
    pub start_date: String,
    pub prerequisite_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateHabitInput {
    pub title: String,
    pub start_date: String,
    pub prerequisite_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HabitReviewItem {
    pub habit_id: String,
    pub title: String,
    pub prerequisite_ids: Vec<String>,
    pub prerequisite_titles: Vec<String>,
    pub raw_completed: bool,
    pub effective_completed: bool,
    pub blocked_by_titles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HabitReview {
    pub review_date: String,
    pub finalized: bool,
    pub items: Vec<HabitReviewItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteHabitReviewInput {
    pub review_date: String,
    pub completed_habit_ids: Vec<String>,
}

fn valid_date(value: &str) -> Result<(), String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| "日期必须为 YYYY-MM-DD".to_string())
}

fn list_habits_core(connection: &Connection) -> Result<Vec<Habit>, String> {
    let mut statement = connection
        .prepare("SELECT id,title,start_date FROM habits WHERE active=1 ORDER BY sort_order,created_at_utc")
        .map_err(|error| error.to_string())?;
    let base = statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    base.into_iter()
        .map(|(id, title, start_date)| {
            let mut dependencies = connection
                .prepare("SELECT prerequisite_habit_id FROM habit_dependencies WHERE habit_id=?1 ORDER BY prerequisite_habit_id")
                .map_err(|error| error.to_string())?;
            let prerequisite_ids = dependencies
                .query_map([&id], |row| row.get(0))
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<String>, _>>()
                .map_err(|error| error.to_string())?;
            Ok(Habit { id, title, start_date, prerequisite_ids })
        })
        .collect()
}

fn habits_for_date(connection: &Connection, date: &str) -> Result<Vec<Habit>, String> {
    let mut statement = connection.prepare(
        "SELECT id,title,start_date FROM habits WHERE start_date<=?1 AND (archived_date IS NULL OR archived_date>?1) ORDER BY sort_order,created_at_utc"
    ).map_err(|error| error.to_string())?;
    let base = statement.query_map([date], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
        .map_err(|error| error.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
    base.into_iter().map(|(id, title, start_date)| {
        let mut dependencies = connection.prepare("SELECT prerequisite_habit_id FROM habit_dependencies WHERE habit_id=?1 ORDER BY prerequisite_habit_id").map_err(|error| error.to_string())?;
        let prerequisite_ids = dependencies.query_map([&id], |row| row.get(0)).map_err(|error| error.to_string())?.collect::<Result<Vec<String>, _>>().map_err(|error| error.to_string())?;
        Ok(Habit { id, title, start_date, prerequisite_ids })
    }).collect()
}

fn create_habit_core(connection: &mut Connection, input: CreateHabitInput) -> Result<Vec<Habit>, String> {
    let title = input.title.trim();
    if title.is_empty() { return Err("打卡名称不能为空".to_string()); }
    valid_date(&input.start_date)?;
    let unique: HashSet<_> = input.prerequisite_ids.iter().collect();
    if unique.len() != input.prerequisite_ids.len() { return Err("前置打卡不能重复".to_string()); }
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    for prerequisite in &input.prerequisite_ids {
        let exists: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM habits WHERE id=?1 AND active=1 AND start_date<=?2", params![prerequisite, input.start_date], |row| row.get(0),
        ).map_err(|error| error.to_string())?;
        if exists == 0 { return Err("前置打卡必须有效，且开始日期不能晚于当前打卡".to_string()); }
    }
    let sort_order: i64 = transaction.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM habits", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    let id = db::new_id();
    let now = db::now_iso();
    transaction.execute(
        "INSERT INTO habits(id,title,start_date,sort_order,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?4,?5,?5)",
        params![id, title, input.start_date, sort_order, now],
    ).map_err(|error| error.to_string())?;
    for prerequisite in input.prerequisite_ids {
        transaction.execute(
            "INSERT INTO habit_dependencies(habit_id,prerequisite_habit_id) VALUES(?1,?2)",
            params![id, prerequisite],
        ).map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    list_habits_core(connection)
}

fn archive_habit_core(connection: &Connection, habit_id: &str) -> Result<Vec<Habit>, String> {
    let dependents: i64 = connection.query_row(
        "SELECT COUNT(*) FROM habit_dependencies d JOIN habits h ON h.id=d.habit_id WHERE d.prerequisite_habit_id=?1 AND h.active=1", [habit_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if dependents > 0 { return Err("仍有启用中的打卡依赖这一项，请先停用这些后置打卡".into()); }
    let changed = connection.execute(
        "UPDATE habits SET active=0,archived_date=?1,updated_at_utc=?2 WHERE id=?3 AND active=1",
        params![Local::now().date_naive().format("%Y-%m-%d").to_string(), db::now_iso(), habit_id],
    ).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("打卡不存在或已停用".to_string()); }
    list_habits_core(connection)
}

fn read_review_core(connection: &Connection, review_date: &str) -> Result<HabitReview, String> {
    valid_date(review_date)?;
    let finalized = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM habit_reviews WHERE review_date=?1)", [review_date], |row| row.get::<_, i64>(0),
    ).map_err(|error| error.to_string())? != 0;
    let habits = habits_for_date(connection, review_date)?;
    let titles: HashMap<_, _> = habits.iter().map(|habit| (habit.id.clone(), habit.title.clone())).collect();
    let mut items = Vec::new();
    for habit in habits {
        let occurrence: Option<(i64, i64, String)> = connection.query_row(
            "SELECT raw_completed,effective_completed,dependency_snapshot_json FROM habit_occurrences WHERE habit_id=?1 AND occurrence_date=?2",
            params![habit.id, review_date], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional().map_err(|error| error.to_string())?;
        let blocked_ids: Vec<String> = occurrence.as_ref()
            .and_then(|(_, _, value)| serde_json::from_str(value).ok())
            .unwrap_or_default();
        items.push(HabitReviewItem {
            habit_id: habit.id,
            title: habit.title,
            prerequisite_titles: habit.prerequisite_ids.iter().filter_map(|id| titles.get(id).cloned()).collect(),
            prerequisite_ids: habit.prerequisite_ids,
            raw_completed: occurrence.as_ref().is_some_and(|value| value.0 != 0),
            effective_completed: occurrence.as_ref().is_some_and(|value| value.1 != 0),
            blocked_by_titles: blocked_ids.iter().filter_map(|id| titles.get(id).cloned()).collect(),
        });
    }
    Ok(HabitReview { review_date: review_date.to_string(), finalized, items })
}

fn effective_completion(
    id: &str,
    raw: &HashSet<String>,
    dependencies: &HashMap<String, Vec<String>>,
    memo: &mut HashMap<String, bool>,
    visiting: &mut HashSet<String>,
) -> Result<bool, String> {
    if let Some(value) = memo.get(id) { return Ok(*value); }
    if !visiting.insert(id.to_string()) { return Err("打卡依赖存在循环".to_string()); }
    let value = raw.contains(id) && dependencies.get(id).into_iter().flatten().try_fold(true, |valid, prerequisite| {
        Ok::<_, String>(valid && effective_completion(prerequisite, raw, dependencies, memo, visiting)?)
    })?;
    visiting.remove(id);
    memo.insert(id.to_string(), value);
    Ok(value)
}

fn complete_review_core(connection: &mut Connection, input: CompleteHabitReviewInput) -> Result<HabitReview, String> {
    valid_date(&input.review_date)?;
    if input.review_date >= Local::now().date_naive().format("%Y-%m-%d").to_string() {
        return Err("请在次日回顾，今天及未来的打卡不能提前结算".into());
    }
    let habits = habits_for_date(connection, &input.review_date)?;
    if habits.is_empty() { return Err("这一天没有需要结算的打卡".into()); }
    let ids: HashSet<_> = habits.iter().map(|habit| habit.id.clone()).collect();
    let raw: HashSet<_> = input.completed_habit_ids.into_iter().collect();
    if raw.iter().any(|id| !ids.contains(id)) { return Err("完成列表包含不存在的打卡".to_string()); }
    let dependencies: HashMap<_, _> = habits.iter().map(|habit| (habit.id.clone(), habit.prerequisite_ids.clone())).collect();
    let mut memo = HashMap::new();
    for id in &ids {
        effective_completion(id, &raw, &dependencies, &mut memo, &mut HashSet::new())?;
    }

    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let now = db::now_iso();
    if transaction.execute(
        "INSERT OR IGNORE INTO habit_reviews(review_date,reviewed_at_utc) VALUES(?1,?2)",
        params![input.review_date, now],
    ).map_err(|error| error.to_string())? == 0 {
        return Err("这一天的打卡已经结算，不能重复改写".to_string());
    }
    for habit in &habits {
        let blocked: Vec<_> = habit.prerequisite_ids.iter().filter(|id| !memo.get(*id).copied().unwrap_or(false)).cloned().collect();
        transaction.execute(
            "INSERT INTO habit_occurrences(habit_id,occurrence_date,raw_completed,effective_completed,dependency_snapshot_json,reviewed_at_utc) VALUES(?1,?2,?3,?4,?5,?6)",
            params![habit.id, input.review_date, raw.contains(&habit.id), memo.get(&habit.id).copied().unwrap_or(false), json!(blocked).to_string(), now],
        ).map_err(|error| error.to_string())?;
    }
    let effective_count = memo.values().filter(|value| **value).count();
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    db::append_event(
        &transaction, "habit.reviewed", "habit_review", &input.review_date, &today, "summary",
        &format!("{} 打卡已回顾：有效完成{effective_count}/{}项", input.review_date, habits.len()),
        Some(&format!("归属日期：{}\n{}", input.review_date, habits.iter().map(|habit| format!("{}：{}", habit.title, if memo.get(&habit.id).copied().unwrap_or(false) { "有效完成" } else if raw.contains(&habit.id) { "已做，但前置未完成" } else { "未完成" })).collect::<Vec<_>>().join("\n"))), &db::new_id(),
    )?;
    transaction.commit().map_err(|error| error.to_string())?;
    read_review_core(connection, &input.review_date)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GoalAction {
    pub id: String,
    pub title: String,
    pub action_kind: String,
    pub required: bool,
    pub target_count: i64,
    pub completed_count: i64,
    pub importance: String,
    pub urgency: String,
    pub manual_completed_count: i64,
    pub tracked: bool,
    pub occurrences: Vec<crate::planning::Occurrence>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GoalPhase {
    pub id: String,
    pub title: String,
    pub start_date: String,
    pub end_date: String,
    pub brainstorm_md: String,
    pub actions: Vec<GoalAction>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LongTermGoal {
    pub id: String,
    pub title: String,
    pub description_md: String,
    pub cycle_days: i64,
    pub start_date: String,
    pub status: String,
    pub phases: Vec<GoalPhase>,
    pub progress_percent: i64,
    pub trophy: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGoalInput { pub title: String, pub description_md: String, pub cycle_days: i64, pub start_date: String }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePhaseInput { pub goal_id: String, pub title: String, pub start_date: String, pub end_date: String, pub brainstorm_md: String }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePhaseNoteInput { pub phase_id: String, pub brainstorm_md: String }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGoalActionInput { pub phase_id: String, pub title: String, pub action_kind: String, pub required: bool, pub target_count: i64 }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetGoalActionProgressInput { pub action_id: String, pub completed_count: i64 }

pub(crate) fn list_goals_core(connection: &Connection) -> Result<Vec<LongTermGoal>, String> {
    let mut statement = connection.prepare(
        "SELECT id,title,description_md,cycle_days,start_date,status FROM long_term_goals WHERE status<>'archived' ORDER BY created_at_utc DESC"
    ).map_err(|error| error.to_string())?;
    let goals = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?)))
        .map_err(|error| error.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
    goals.into_iter().map(|(id, title, description_md, cycle_days, start_date, status)| {
        let mut phase_statement = connection.prepare(
            "SELECT id,title,start_date,end_date,brainstorm_md FROM goal_phases WHERE goal_id=?1 ORDER BY sort_order,created_at_utc"
        ).map_err(|error| error.to_string())?;
        let phase_rows = phase_statement.query_map([&id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?)))
            .map_err(|error| error.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
        let mut phases = Vec::new();
        let mut required_total = 0;
        let mut required_completed = 0;
        let mut optional_total = 0;
        let mut optional_completed = 0;
        for (phase_id, phase_title, phase_start, phase_end, brainstorm_md) in phase_rows {
            let mut action_statement = connection.prepare(
                "SELECT a.id,a.title,a.action_kind,a.required,a.target_count,a.completed_count,COALESCE(x.importance,'important'),COALESCE(x.urgency,'relaxed') FROM goal_actions a LEFT JOIN goal_action_options x ON x.action_id=a.id WHERE a.phase_id=?1 AND x.deleted_at IS NULL ORDER BY a.sort_order,a.created_at_utc"
            ).map_err(|error| error.to_string())?;
            let mut actions = action_statement.query_map([&phase_id], |row| Ok(GoalAction {
                id: row.get(0)?, title: row.get(1)?, action_kind: row.get(2)?, required: row.get::<_, i64>(3)? != 0,
                target_count: row.get(4)?, completed_count: row.get(5)?,
                importance:row.get(6)?,urgency:row.get(7)?,manual_completed_count:row.get(5)?,tracked:false,occurrences:vec![],
            })).map_err(|error| error.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
            for action in &mut actions {
                action.occurrences=crate::planning::occurrences(connection,&action.id)?;
                action.tracked=connection.query_row("SELECT EXISTS(SELECT 1 FROM goal_action_occurrences WHERE action_id=?1)",[&action.id],|r|r.get(0)).map_err(|e|e.to_string())?;
                action.completed_count+=action.occurrences.iter().filter(|o|o.status=="completed").count() as i64;
                if action.required { required_total += action.target_count; required_completed += action.completed_count.min(action.target_count); }
                else { optional_total += action.target_count; optional_completed += action.completed_count.min(action.target_count); }
            }
            phases.push(GoalPhase { id: phase_id, title: phase_title, start_date: phase_start, end_date: phase_end, brainstorm_md, actions });
        }
        let required_done = required_total > 0 && required_completed >= required_total;
        let progress_percent = if required_total > 0 { ((required_completed + optional_completed) * 100) / required_total } else { 0 };
        let trophy = if required_done && optional_total > 0 && optional_completed >= optional_total { Some("gold".to_string()) }
            else if required_done && optional_completed > 0 { Some("silver".to_string()) }
            else if required_done { Some("bronze".to_string()) } else { None };
        Ok(LongTermGoal { id, title, description_md, cycle_days, start_date, status, phases, progress_percent, trophy })
    }).collect()
}

fn create_goal_core(connection: &Connection, input: CreateGoalInput) -> Result<Vec<LongTermGoal>, String> {
    if input.title.trim().is_empty() { return Err("长期目标名称不能为空".to_string()); }
    if !(1..=3660).contains(&input.cycle_days) { return Err("循环周期必须在 1–3660 天之间".to_string()); }
    valid_date(&input.start_date)?;
    let now = db::now_iso();
    connection.execute(
        "INSERT INTO long_term_goals(id,title,description_md,cycle_days,start_date,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?4,?5,?6,?6)",
        params![db::new_id(), input.title.trim(), input.description_md.trim(), input.cycle_days, input.start_date, now],
    ).map_err(|error| error.to_string())?;
    list_goals_core(connection)
}

fn create_phase_core(connection: &Connection, input: CreatePhaseInput) -> Result<Vec<LongTermGoal>, String> {
    if input.title.trim().is_empty() { return Err("阶段名称不能为空".to_string()); }
    valid_date(&input.start_date)?; valid_date(&input.end_date)?;
    if input.start_date > input.end_date { return Err("阶段结束日期不能早于开始日期".to_string()); }
    let goal_exists: i64 = connection.query_row("SELECT COUNT(*) FROM long_term_goals WHERE id=?1 AND status='active'", [&input.goal_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    if goal_exists == 0 { return Err("长期目标不存在或已结束".to_string()); }
    let sort_order: i64 = connection.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM goal_phases WHERE goal_id=?1", [&input.goal_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    let now = db::now_iso();
    connection.execute(
        "INSERT INTO goal_phases(id,goal_id,title,start_date,end_date,brainstorm_md,sort_order,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",
        params![db::new_id(), input.goal_id, input.title.trim(), input.start_date, input.end_date, input.brainstorm_md.trim(), sort_order, now],
    ).map_err(|error| error.to_string())?;
    list_goals_core(connection)
}

fn save_phase_note_core(connection: &Connection, input: SavePhaseNoteInput) -> Result<Vec<LongTermGoal>, String> {
    if connection.execute(
        "UPDATE goal_phases SET brainstorm_md=?1,updated_at_utc=?2 WHERE id=?3",
        params![input.brainstorm_md, db::now_iso(), input.phase_id],
    ).map_err(|error| error.to_string())? == 0 { return Err("阶段不存在".to_string()); }
    list_goals_core(connection)
}

fn create_action_core(connection: &Connection, input: CreateGoalActionInput) -> Result<Vec<LongTermGoal>, String> {
    if input.title.trim().is_empty() { return Err("计划事项不能为空".to_string()); }
    if !["one_off", "repeating"].contains(&input.action_kind.as_str()) { return Err("事项类型无效".to_string()); }
    if !(1..=3660).contains(&input.target_count) { return Err("计划次数必须在 1–3660 之间".to_string()); }
    let phase_exists: i64 = connection.query_row("SELECT COUNT(*) FROM goal_phases WHERE id=?1", [&input.phase_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    if phase_exists == 0 { return Err("目标阶段不存在".to_string()); }
    let sort_order: i64 = connection.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM goal_actions WHERE phase_id=?1", [&input.phase_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    let now = db::now_iso();
    connection.execute(
        "INSERT INTO goal_actions(id,phase_id,title,action_kind,required,target_count,sort_order,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",
        params![db::new_id(), input.phase_id, input.title.trim(), input.action_kind, input.required, input.target_count, sort_order, now],
    ).map_err(|error| error.to_string())?;
    list_goals_core(connection)
}

fn set_action_progress_core(connection: &Connection, input: SetGoalActionProgressInput) -> Result<Vec<LongTermGoal>, String> {
    let linked:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM goal_action_occurrences WHERE action_id=?1) OR EXISTS(SELECT 1 FROM goal_action_options WHERE action_id=?1 AND deleted_at IS NOT NULL)",[&input.action_id],|r|r.get(0)).map_err(|e|e.to_string())?;
    if linked{return Err("已排期任务请在当天的任务列表中记录完成情况".into());}
    if input.completed_count < 0 { return Err("完成次数不能小于零".to_string()); }
    let target: i64 = connection.query_row("SELECT target_count FROM goal_actions WHERE id=?1", [&input.action_id], |row| row.get(0)).map_err(|_| "计划事项不存在".to_string())?;
    if input.completed_count > target { return Err("完成次数不能超过计划次数".to_string()); }
    connection.execute("UPDATE goal_actions SET completed_count=?1,updated_at_utc=?2 WHERE id=?3", params![input.completed_count, db::now_iso(), input.action_id]).map_err(|error| error.to_string())?;
    list_goals_core(connection)
}

#[tauri::command] pub fn list_habits(database: State<'_, Database>) -> Result<Vec<Habit>, String> { let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; list_habits_core(&connection) }
#[tauri::command] pub fn create_habit(database: State<'_, Database>, input: CreateHabitInput) -> Result<Vec<Habit>, String> { let mut connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; create_habit_core(&mut connection, input) }
#[tauri::command(rename_all = "camelCase")] pub fn archive_habit(database: State<'_, Database>, habit_id: String) -> Result<Vec<Habit>, String> { let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; archive_habit_core(&connection, &habit_id) }
#[tauri::command(rename_all = "camelCase")] pub fn get_habit_review(database: State<'_, Database>, review_date: String) -> Result<HabitReview, String> { let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; read_review_core(&connection, &review_date) }
#[tauri::command] pub fn complete_habit_review(database: State<'_, Database>, input: CompleteHabitReviewInput) -> Result<HabitReview, String> { let mut connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; complete_review_core(&mut connection, input) }
#[tauri::command] pub fn list_long_term_goals(database: State<'_, Database>) -> Result<Vec<LongTermGoal>, String> { let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; list_goals_core(&connection) }
#[tauri::command] pub fn create_long_term_goal(database: State<'_, Database>, input: CreateGoalInput) -> Result<Vec<LongTermGoal>, String> { let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; create_goal_core(&connection, input) }
#[tauri::command] pub fn create_goal_phase(database: State<'_, Database>, input: CreatePhaseInput) -> Result<Vec<LongTermGoal>, String> { let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; create_phase_core(&connection, input) }
#[tauri::command] pub fn save_goal_phase_note(database: State<'_, Database>, input: SavePhaseNoteInput) -> Result<Vec<LongTermGoal>, String> { let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; save_phase_note_core(&connection, input) }
#[tauri::command] pub fn create_goal_action(database: State<'_, Database>, input: CreateGoalActionInput) -> Result<Vec<LongTermGoal>, String> { let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; create_action_core(&connection, input) }
#[tauri::command] pub fn set_goal_action_progress(database: State<'_, Database>, input: SetGoalActionProgressInput) -> Result<Vec<LongTermGoal>, String> { let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?; set_action_progress_core(&connection, input) }

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> Connection { let connection = Connection::open_in_memory().unwrap(); db::initialize(&connection).unwrap(); connection }

    #[test]
    fn missing_prerequisite_invalidates_a_completed_dependent_habit() {
        let mut connection = connection();
        let habits = create_habit_core(&mut connection, CreateHabitInput { title: "到家洗澡".into(), start_date: "2026-09-01".into(), prerequisite_ids: vec![] }).unwrap();
        let prerequisite = habits[0].id.clone();
        let habits = create_habit_core(&mut connection, CreateHabitInput { title: "十点睡觉".into(), start_date: "2026-09-01".into(), prerequisite_ids: vec![prerequisite] }).unwrap();
        let dependent = habits.iter().find(|habit| habit.title == "十点睡觉").unwrap().id.clone();
        let review = complete_review_core(&mut connection, CompleteHabitReviewInput { review_date: "2026-09-02".into(), completed_habit_ids: vec![dependent] }).unwrap();
        let sleep = review.items.iter().find(|item| item.title == "十点睡觉").unwrap();
        assert!(sleep.raw_completed);
        assert!(!sleep.effective_completed);
        assert_eq!(sleep.blocked_by_titles, vec!["到家洗澡"]);
    }

    #[test]
    fn prerequisite_cannot_be_archived_while_dependents_are_active() {
        let mut connection = connection();
        let habits = create_habit_core(&mut connection, CreateHabitInput { title: "前置".into(), start_date: "2026-01-01".into(), prerequisite_ids: vec![] }).unwrap();
        let id = habits[0].id.clone();
        create_habit_core(&mut connection, CreateHabitInput { title: "后置".into(), start_date: "2026-01-01".into(), prerequisite_ids: vec![id.clone()] }).unwrap();
        assert!(archive_habit_core(&connection, &id).is_err());
        assert_eq!(list_habits_core(&connection).unwrap().len(), 2);
    }

    #[test]
    fn review_cannot_finalize_today_or_an_empty_day() {
        let mut connection = connection();
        assert!(complete_review_core(&mut connection, CompleteHabitReviewInput { review_date: Local::now().date_naive().to_string(), completed_habit_ids: vec![] }).is_err());
        assert!(complete_review_core(&mut connection, CompleteHabitReviewInput { review_date: "2026-01-01".into(), completed_habit_ids: vec![] }).is_err());
        let count: i64 = connection.query_row("SELECT COUNT(*) FROM habit_reviews", [], |r|r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn required_and_optional_actions_award_progress_over_one_hundred() {
        let connection = connection();
        let goals = create_goal_core(&connection, CreateGoalInput { title: "读懂原理图".into(), description_md: "".into(), cycle_days: 30, start_date: "2026-09-01".into() }).unwrap();
        let goal_id = goals[0].id.clone();
        let goals = create_phase_core(&connection, CreatePhaseInput { goal_id, title: "电工识图".into(), start_date: "2026-09-01".into(), end_date: "2026-09-07".into(), brainstorm_md: "".into() }).unwrap();
        let phase_id = goals[0].phases[0].id.clone();
        let goals = create_action_core(&connection, CreateGoalActionInput { phase_id: phase_id.clone(), title: "认识元件".into(), action_kind: "repeating".into(), required: true, target_count: 5 }).unwrap();
        let required_id = goals[0].phases[0].actions[0].id.clone();
        let goals = create_action_core(&connection, CreateGoalActionInput { phase_id, title: "扩展阅读".into(), action_kind: "one_off".into(), required: false, target_count: 1 }).unwrap();
        let optional_id = goals[0].phases[0].actions.iter().find(|item| !item.required).unwrap().id.clone();
        set_action_progress_core(&connection, SetGoalActionProgressInput { action_id: required_id, completed_count: 5 }).unwrap();
        let goals = set_action_progress_core(&connection, SetGoalActionProgressInput { action_id: optional_id, completed_count: 1 }).unwrap();
        assert_eq!(goals[0].progress_percent, 120);
        assert_eq!(goals[0].trophy.as_deref(), Some("gold"));
    }
}
