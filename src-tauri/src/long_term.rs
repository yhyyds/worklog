use crate::{db, Database};
use rusqlite::{params, Connection};
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

fn save(connection: &Connection, input: SaveLongTermInput) -> Result<(), String> {
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
    if let Some(id) = input.id {
        let changed = connection.execute(
            "UPDATE long_term_work SET title=?1,workload=?2,completed=?3 WHERE id=?4 AND parent_id IS ?5",
            params![title, input.workload, input.completed, id, input.parent_id],
        ).map_err(|error| error.to_string())?;
        if changed == 0 { return Err("长期任务不存在或父项不匹配".into()); }
    } else {
        connection.execute(
            "INSERT INTO long_term_work(id,parent_id,title,workload,completed) VALUES(?1,?2,?3,?4,?5)",
            params![db::new_id(), input.parent_id, title, input.workload, input.completed],
        ).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn list_long_term_work(database: State<'_, Database>) -> Result<Vec<LongTermItem>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    list(&connection)
}

#[tauri::command]
pub(crate) fn save_long_term_work(database: State<'_, Database>, input: SaveLongTermInput) -> Result<Vec<LongTermItem>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    save(&connection, input)?;
    list(&connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_persists_independently_of_daily_tasks() {
        let connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        let input = |parent_id, workload| SaveLongTermInput {
            id: None, title: "工作安排".into(), parent_id, workload, completed: false,
        };
        assert!(save(&connection, input(None, Some("light".into()))).is_err());
        save(&connection, input(None, None)).unwrap();
        let parent = list(&connection).unwrap().remove(0);
        assert!(save(&connection, input(Some(parent.id.clone()), Some("over_3h".into()))).is_err());
        save(&connection, input(Some(parent.id.clone()), Some("large".into()))).unwrap();
        let child = list(&connection).unwrap().remove(1);
        assert!(save(&connection, input(Some(child.id.clone()), None)).is_err());
        assert!(save(&connection, SaveLongTermInput { id: Some(child.id.clone()), ..input(None, None) }).is_err());
        save(&connection, SaveLongTermInput {
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
