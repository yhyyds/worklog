use crate::{db, growth, report_details, Database};
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{collections::HashSet, fs, path::Path};
use tauri::State;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DailyMetric {
    pub date: String,
    pub planned_tasks: i64,
    pub completed_tasks: i64,
    pub focus_minutes: i64,
    pub habit_effective: i64,
    pub habit_total: i64,
    pub habit_pending: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportQuote {
    pub id: String,
    pub text: String,
    pub author: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportGoal {
    pub title: String,
    pub progress_percent: i64,
    pub trophy: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyReport {
    pub week_start: String,
    pub week_end: String,
    pub daily: Vec<DailyMetric>,
    pub planned_tasks: i64,
    pub completed_tasks: i64,
    pub focus_minutes: i64,
    pub completion_rate: i64,
    pub habit_effective: i64,
    pub habit_total: i64,
    pub habit_rate: i64,
    pub focus_change_percent: Option<i64>,
    pub completed_change_percent: Option<i64>,
    pub baseline_weeks: usize,
    pub comparison_days: usize,
    pub habit_pending: i64,
    pub scenario: String,
    pub headline: String,
    pub observation: String,
    pub quote: ReportQuote,
    pub goals: Vec<ReportGoal>,
    pub habits: Vec<report_details::HabitDetail>,
    pub tasks: Vec<report_details::ReportTask>,
    pub focus_detail: report_details::FocusDetail,
    pub history: Vec<WeekComparison>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WeekComparison { pub week_start: String, pub planned: i64, pub completed: i64, pub focus_minutes: i64, pub habit_completed: i64, pub habit_reviewed: i64 }

#[derive(Clone, Copy)]
struct QuoteDef { id: &'static str, text: &'static str, author: &'static str, source: &'static str, tags: &'static [&'static str] }

const QUOTES: &[QuoteDef] = &[
    QuoteDef { id: "xunzi-01", text: "不积跬步，无以至千里；不积小流，无以成江海。", author: "荀子", source: "《劝学》", tags: &["steady", "recovery"] },
    QuoteDef { id: "xunzi-02", text: "锲而不舍，金石可镂。", author: "荀子", source: "《劝学》", tags: &["strong", "steady"] },
    QuoteDef { id: "xunzi-03", text: "君子生非异也，善假于物也。", author: "荀子", source: "《劝学》", tags: &["recalibrate"] },
    QuoteDef { id: "laozi-01", text: "千里之行，始于足下。", author: "老子", source: "《道德经·第六十四章》", tags: &["recovery", "steady"] },
    QuoteDef { id: "laozi-02", text: "慎终如始，则无败事。", author: "老子", source: "《道德经·第六十四章》", tags: &["strong", "steady"] },
    QuoteDef { id: "laozi-03", text: "知人者智，自知者明。", author: "老子", source: "《道德经·第三十三章》", tags: &["recalibrate", "recovery"] },
    QuoteDef { id: "laozi-04", text: "合抱之木，生于毫末；九层之台，起于累土。", author: "老子", source: "《道德经·第六十四章》", tags: &["steady"] },
    QuoteDef { id: "lunyu-01", text: "工欲善其事，必先利其器。", author: "孔子", source: "《论语·卫灵公》", tags: &["recalibrate"] },
    QuoteDef { id: "lunyu-02", text: "欲速则不达，见小利则大事不成。", author: "孔子", source: "《论语·子路》", tags: &["recalibrate"] },
    QuoteDef { id: "lunyu-03", text: "逝者如斯夫，不舍昼夜。", author: "孔子", source: "《论语·子罕》", tags: &["steady", "recovery"] },
    QuoteDef { id: "lunyu-04", text: "知之者不如好之者，好之者不如乐之者。", author: "孔子", source: "《论语·雍也》", tags: &["strong"] },
    QuoteDef { id: "lunyu-05", text: "君子求诸己，小人求诸人。", author: "孔子", source: "《论语·卫灵公》", tags: &["recalibrate"] },
    QuoteDef { id: "lunyu-06", text: "过而不改，是谓过矣。", author: "孔子", source: "《论语·卫灵公》", tags: &["recalibrate", "recovery"] },
    QuoteDef { id: "lunyu-07", text: "岁寒，然后知松柏之后凋也。", author: "孔子", source: "《论语·子罕》", tags: &["strong", "steady"] },
    QuoteDef { id: "mengzi-01", text: "行有不得者，皆反求诸己。", author: "孟子", source: "《孟子·离娄上》", tags: &["recalibrate"] },
    QuoteDef { id: "mengzi-02", text: "不以规矩，不能成方圆。", author: "孟子", source: "《孟子·离娄上》", tags: &["recalibrate", "steady"] },
    QuoteDef { id: "mengzi-03", text: "有为者辟若掘井，掘井九轫而不及泉，犹为弃井也。", author: "孟子", source: "《孟子·尽心上》", tags: &["steady", "recovery"] },
    QuoteDef { id: "zhuangzi-01", text: "且夫水之积也不厚，则其负大舟也无力。", author: "庄子", source: "《庄子·逍遥游》", tags: &["steady", "recalibrate"] },
    QuoteDef { id: "shijing-01", text: "靡不有初，鲜克有终。", author: "《诗经》", source: "《大雅·荡》", tags: &["recalibrate", "steady"] },
    QuoteDef { id: "shijing-02", text: "如切如磋，如琢如磨。", author: "《诗经》", source: "《卫风·淇奥》", tags: &["strong", "steady"] },
    QuoteDef { id: "shijing-03", text: "他山之石，可以攻玉。", author: "《诗经》", source: "《小雅·鹤鸣》", tags: &["recalibrate"] },
    QuoteDef { id: "zhouyi-01", text: "天行健，君子以自强不息。", author: "《周易》", source: "《乾卦》", tags: &["strong", "recovery"] },
    QuoteDef { id: "zhouyi-02", text: "穷则变，变则通，通则久。", author: "《周易》", source: "《系辞下》", tags: &["recalibrate", "recovery"] },
    QuoteDef { id: "zhouyi-03", text: "君子藏器于身，待时而动。", author: "《周易》", source: "《系辞下》", tags: &["steady"] },
    QuoteDef { id: "liji-01", text: "凡事豫则立，不豫则废。", author: "《礼记》", source: "《中庸》", tags: &["recalibrate", "steady"] },
    QuoteDef { id: "liji-02", text: "学然后知不足，教然后知困。", author: "《礼记》", source: "《学记》", tags: &["recovery", "steady"] },
    QuoteDef { id: "liji-03", text: "苟日新，日日新，又日新。", author: "《礼记》", source: "《大学》", tags: &["recovery", "strong"] },
    QuoteDef { id: "hanyu-01", text: "业精于勤，荒于嬉；行成于思，毁于随。", author: "韩愈", source: "《进学解》", tags: &["strong", "recalibrate"] },
    QuoteDef { id: "sushi-01", text: "博观而约取，厚积而薄发。", author: "苏轼", source: "《稼说送张琥》", tags: &["steady", "strong"] },
    QuoteDef { id: "tao-01", text: "及时当勉励，岁月不待人。", author: "陶渊明", source: "《杂诗》", tags: &["recovery", "steady"] },
    QuoteDef { id: "luyou-01", text: "纸上得来终觉浅，绝知此事要躬行。", author: "陆游", source: "《冬夜读书示子聿》", tags: &["recalibrate", "steady"] },
    QuoteDef { id: "liuxi-01", text: "千淘万漉虽辛苦，吹尽狂沙始到金。", author: "刘禹锡", source: "《浪淘沙》", tags: &["strong", "steady"] },
    QuoteDef { id: "dufu-01", text: "会当凌绝顶，一览众山小。", author: "杜甫", source: "《望岳》", tags: &["strong"] },
    QuoteDef { id: "libai-01", text: "长风破浪会有时，直挂云帆济沧海。", author: "李白", source: "《行路难》", tags: &["recovery", "strong"] },
    QuoteDef { id: "quyuan-01", text: "路漫漫其修远兮，吾将上下而求索。", author: "屈原", source: "《离骚》", tags: &["steady", "recovery"] },
    QuoteDef { id: "wangbo-01", text: "穷且益坚，不坠青云之志。", author: "王勃", source: "《滕王阁序》", tags: &["recovery", "strong"] },
    QuoteDef { id: "caocao-01", text: "老骥伏枥，志在千里；烈士暮年，壮心不已。", author: "曹操", source: "《龟虽寿》", tags: &["recovery", "strong"] },
    QuoteDef { id: "zhugeliang-01", text: "非学无以广才，非志无以成学。", author: "诸葛亮", source: "《诫子书》", tags: &["steady"] },
    QuoteDef { id: "zhugeliang-02", text: "淫慢则不能励精，险躁则不能治性。", author: "诸葛亮", source: "《诫子书》", tags: &["recalibrate"] },
    QuoteDef { id: "zhuxi-01", text: "问渠那得清如许？为有源头活水来。", author: "朱熹", source: "《观书有感》", tags: &["recovery", "strong"] },
];

fn daily_metric(connection: &Connection, date: NaiveDate) -> Result<DailyMetric, String> {
    let text = date.format("%Y-%m-%d").to_string();
    let (planned_tasks, completed_tasks): (i64, i64) = connection.query_row(
        &format!("SELECT COUNT(*),COALESCE(SUM(CASE WHEN day_status='completed' THEN 1 ELSE 0 END),0) FROM task_day_instances d WHERE work_date=?1 AND {}", report_details::PLAN_FILTER),
        [&text], |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|error| error.to_string())?;
    let focus_seconds: i64 = connection.query_row(
        "SELECT COALESCE(SUM(s.allocated_seconds),0) FROM focus_segments s JOIN focus_sessions f ON f.id=s.focus_session_id WHERE f.work_date=?1",
        [&text], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    let habit_total: i64 = connection.query_row(
        "SELECT COUNT(*) FROM habits WHERE start_date<=?1 AND (archived_date IS NULL OR archived_date>?1)",
        [&text], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    let habit_effective: i64 = connection.query_row(
        "SELECT COALESCE(SUM(effective_completed),0) FROM habit_occurrences WHERE occurrence_date=?1",
        [&text], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    let reviewed: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM habit_reviews WHERE review_date=?1)", [&text], |row| row.get(0)).map_err(|error| error.to_string())?;
    Ok(DailyMetric { date: text, planned_tasks, completed_tasks, focus_minutes: (focus_seconds + 30) / 60, habit_effective, habit_total: if reviewed { habit_total } else { 0 }, habit_pending: if reviewed { 0 } else { habit_total } })
}

// Sharing uses only the filtered metrics and never the private quote cache.
pub fn share_quote(week: &str, scenario: &str) -> ReportQuote {
    let candidates:Vec<_>=QUOTES.iter().filter(|q|q.tags.contains(&scenario)).collect();
    let seed=week.bytes().fold(0usize,|s,b|s.wrapping_mul(31).wrapping_add(b as usize));
    let q=candidates[seed%candidates.len()];
    ReportQuote{id:q.id.into(),text:q.text.into(),author:q.author.into(),source:q.source.into()}
}

fn week_metrics(connection: &Connection, start: NaiveDate) -> Result<Vec<DailyMetric>, String> {
    (0..7).map(|offset| daily_metric(connection, start + Duration::days(offset))).collect()
}

fn totals(metrics: &[DailyMetric]) -> (i64, i64, i64) {
    metrics.iter().fold((0, 0, 0), |sum, day| (sum.0 + day.planned_tasks, sum.1 + day.completed_tasks, sum.2 + day.focus_minutes))
}

fn median(mut values: Vec<i64>) -> i64 {
    if values.is_empty() { return 0; }
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 0 { values[middle - 1] + (values[middle] - values[middle - 1]) / 2 } else { values[middle] }
}

fn percent_change(current: i64, previous: i64) -> Option<i64> {
    if previous <= 0 { None } else { Some(((current - previous) * 100) / previous) }
}

fn select_quote(connection: &Connection, week_start: &str, scenario: &str) -> Result<ReportQuote, String> {
    if let Some(existing) = connection.query_row("SELECT quote_id FROM quote_usage WHERE week_start=?1 LIMIT 1", [week_start], |row| row.get::<_, String>(0)).optional().map_err(|error| error.to_string())? {
        if let Some(quote) = QUOTES.iter().find(|quote| quote.id == existing) {
            return Ok(ReportQuote { id: quote.id.into(), text: quote.text.into(), author: quote.author.into(), source: quote.source.into() });
        }
    }
    let recent: HashSet<String> = {
        let mut statement = connection.prepare("SELECT quote_id FROM quote_usage ORDER BY week_start DESC LIMIT 26").map_err(|error| error.to_string())?;
        let values = statement.query_map([], |row| row.get(0)).map_err(|error| error.to_string())?.collect::<Result<HashSet<_>, _>>().map_err(|error| error.to_string())?;
        values
    };
    let mut candidates: Vec<_> = QUOTES.iter().filter(|quote| quote.tags.contains(&scenario) && !recent.contains(quote.id)).collect();
    if candidates.is_empty() { candidates = QUOTES.iter().filter(|quote| !recent.contains(quote.id)).collect(); }
    if candidates.is_empty() { candidates = QUOTES.iter().collect(); }
    let seed = week_start.bytes().fold(0usize, |sum, value| sum.wrapping_mul(31).wrapping_add(value as usize));
    let quote = candidates[seed % candidates.len()];
    connection.execute("INSERT OR REPLACE INTO quote_usage(quote_id,week_start,scenario,used_at_utc) VALUES(?1,?2,?3,?4)", params![quote.id, week_start, scenario, db::now_iso()]).map_err(|error| error.to_string())?;
    Ok(ReportQuote { id: quote.id.into(), text: quote.text.into(), author: quote.author.into(), source: quote.source.into() })
}

fn report_core(connection: &Connection, week_start: &str) -> Result<WeeklyReport, String> {
    report_at(connection, week_start, Local::now().date_naive())
}

fn report_at(connection: &Connection, week_start: &str, today: NaiveDate) -> Result<WeeklyReport, String> {
    let start = NaiveDate::parse_from_str(week_start, "%Y-%m-%d").map_err(|_| "周开始日期必须为 YYYY-MM-DD".to_string())?;
    if start.weekday() != Weekday::Mon || start > today { return Err("请选择不晚于今天的周一".into()); }
    let comparison_days = ((today - start).num_days() + 1).min(7) as usize;
    let mut daily = week_metrics(connection, start)?;
    for day in daily.iter_mut().skip(comparison_days) {
        day.planned_tasks = 0; day.completed_tasks = 0; day.focus_minutes = 0;
        day.habit_effective = 0; day.habit_total = 0; day.habit_pending = 0;
    }
    // Habit reviews happen the following day; today's habits are not overdue.
    if let Some(day) = daily.iter_mut().find(|d| d.date == today.to_string()) { day.habit_pending = 0; }
    let (planned_tasks, completed_tasks, focus_minutes) = totals(&daily);
    let habit_total: i64 = daily.iter().map(|day| day.habit_total).sum();
    let habit_effective: i64 = daily.iter().map(|day| day.habit_effective).sum();
    let habit_pending = daily.iter().map(|day| day.habit_pending).sum();
    let completion_rate = if planned_tasks > 0 { completed_tasks * 100 / planned_tasks } else { 0 };
    let habit_rate = if habit_total > 0 { habit_effective * 100 / habit_total } else { 0 };
    let previous = totals(&week_metrics(connection, start - Duration::days(7))?[..comparison_days]);
    let mut historical_focus = Vec::new();
    let mut historical_completed = Vec::new();
    for offset in 1..=8 {
        let values = totals(&week_metrics(connection, start - Duration::days(offset * 7))?[..comparison_days]);
        if values.0 > 0 || values.2 > 0 { historical_completed.push(values.1); historical_focus.push(values.2); }
    }
    let baseline_weeks = historical_focus.len();
    let focus_baseline = median(historical_focus);
    let completed_baseline = median(historical_completed);
    let (scenario, headline, observation) = if planned_tasks == 0 && focus_minutes == 0 && habit_total == 0 {
        ("recovery", "本周暂无记录", "".to_string())
    } else if completion_rate >= 100 && (habit_total == 0 || habit_rate >= 80) {
        ("strong", "本周计划已完成", format!("完成 {completed_tasks} 项，专注 {focus_minutes} 分钟。"))
    } else if baseline_weeks >= 3 && focus_baseline > 0 && completed_baseline > 0 && focus_minutes * 100 > focus_baseline * 120 && completed_tasks * 100 < completed_baseline * 80 {
        ("recalibrate", "本周投入了更多时间", "专注时间高于近期中位数，完成项较少。可以回看未完成的任务，确认是否需要拆分或调整计划。".to_string())
    } else if completed_tasks > previous.1 && previous.1 > 0 {
        ("recovery", "本周完成的事项更多了", format!("比上周同期多完成 {} 项。", completed_tasks-previous.1))
    } else {
        ("steady", "本周回顾", format!("完成 {completed_tasks} 项，还有 {} 项未完成。",planned_tasks-completed_tasks))
    };
    let quote = select_quote(connection, week_start, scenario)?;
    // Goal counters do not yet have historical snapshots; never project today's
    // progress into a past weekly report.
    let goals = if today <= start + Duration::days(6) {
        growth::list_goals_core(connection)?.into_iter().take(3).map(|goal| ReportGoal { title: goal.title, progress_percent: goal.progress_percent, trophy: goal.trophy }).collect()
    } else { Vec::new() };
    let end = start + Duration::days(comparison_days as i64 - 1);
    let habits = report_details::habits(connection,start,end,today)?;
    let tasks = report_details::tasks(connection,start,end)?;
    let focus_detail = report_details::focus(connection,start,end)?;
    let mut history = Vec::new();
    for offset in (1..=4).rev() {
        let date = start - Duration::days(offset * 7);
        let metrics = week_metrics(connection,date)?;
        let sample = &metrics[..comparison_days];
        let (planned,completed,focus_minutes) = totals(sample);
        history.push(WeekComparison {week_start:date.to_string(),planned,completed,focus_minutes,habit_completed:sample.iter().map(|d|d.habit_effective).sum(),habit_reviewed:sample.iter().map(|d|d.habit_total).sum()});
    }
    Ok(WeeklyReport {
        week_start: week_start.into(), week_end: (start + Duration::days(6)).format("%Y-%m-%d").to_string(), daily,
        planned_tasks, completed_tasks, focus_minutes, completion_rate, habit_effective, habit_total, habit_rate,
        focus_change_percent: percent_change(focus_minutes, previous.2), completed_change_percent: percent_change(completed_tasks, previous.1),
        baseline_weeks, comparison_days, habit_pending, scenario: scenario.into(), headline: headline.to_string(), observation, quote, goals, habits, tasks, focus_detail, history,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_weekly_report(database: State<'_, Database>, week_start: String) -> Result<WeeklyReport, String> {
    let connection = database.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    report_core(&connection, &week_start)
}

#[tauri::command(rename_all = "camelCase")]
pub fn save_weekly_report_image(path: String, bytes: Vec<u8>) -> Result<String, String> {
    let path = Path::new(&path);
    if !path.is_absolute() || path.extension().and_then(|value| value.to_str()).map(|value| !value.eq_ignore_ascii_case("png")).unwrap_or(true) {
        return Err("周报图片必须保存为绝对路径下的 PNG 文件".to_string());
    }
    if bytes.len() > 20 * 1024 * 1024 || !bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
        return Err("导出的 PNG 数据无效或超过 20 MB".to_string());
    }
    fs::write(path, bytes).map_err(|error| format!("无法保存周报图片：{error}"))?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_uses_a_stable_quote_for_the_same_week() {
        let connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
        let first = report_at(&connection, "2026-09-07", today).unwrap();
        let second = report_at(&connection, "2026-09-07", today).unwrap();
        assert_eq!(first.quote, second.quote);
        assert!(!first.quote.source.is_empty());
    }

    #[test]
    fn quote_ids_are_unique() {
        let ids: HashSet<_> = QUOTES.iter().map(|quote| quote.id).collect();
        assert_eq!(ids.len(), QUOTES.len());
    }

    #[test]
    fn median_handles_even_odd_and_empty_samples() {
        assert_eq!(median(vec![]), 0);
        assert_eq!(median(vec![30, 10]), 20);
        assert_eq!(median(vec![50, 10, 30]), 30);
    }

    #[test]
    fn unfinished_week_excludes_future_days_and_pending_habits() {
        let connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        connection.execute("INSERT INTO habits(id,title,start_date,created_at_utc,updated_at_utc) VALUES('h','阅读','2026-09-07','now','now')", []).unwrap();
        let report = report_at(&connection, "2026-09-07", NaiveDate::from_ymd_opt(2026, 9, 9).unwrap()).unwrap();
        assert_eq!(report.comparison_days, 3);
        assert_eq!(report.habit_total, 0);
        assert_eq!(report.habit_pending, 2);
        assert_eq!(report.daily[3].habit_pending, 0);
        assert_eq!(report.headline,"本周暂无记录");
    }

    #[test]
    fn future_weeks_and_non_mondays_are_rejected() {
        let connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
        assert!(report_at(&connection, "2026-09-14", today).is_err());
        assert!(report_at(&connection, "2026-09-08", today).is_err());
    }
}
