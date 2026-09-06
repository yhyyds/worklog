use crate::{db, Database};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub color: String,
    pub share_mode: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Classification {
    pub entity_id: String,
    pub entity_kind: String,
    pub category_id: Option<String>,
    pub share_name: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub categories: Vec<Category>,
    pub classifications: Vec<Classification>,
}
pub fn catalog(c: &Connection) -> Result<Catalog, String> {
    let mut q = c
        .prepare("SELECT id,name,color,share_mode FROM growth_categories ORDER BY name,id")
        .map_err(|e| e.to_string())?;
    let categories = q
        .query_map([], |r| {
            Ok(Category {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                share_mode: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut q = c
        .prepare("SELECT entity_id,entity_kind,category_id,share_name FROM growth_classifications")
        .map_err(|e| e.to_string())?;
    let classifications = q
        .query_map([], |r| {
            Ok(Classification {
                entity_id: r.get(0)?,
                entity_kind: r.get(1)?,
                category_id: r.get(2)?,
                share_name: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(Catalog {
        categories,
        classifications,
    })
}
fn save_core(c: &Connection, mut input: Category) -> Result<Catalog, String> {
    input.name = input.name.trim().into();
    if input.name.is_empty() || input.name.chars().count() > 40 {
        return Err("分类名称须为 1–40 个字".into());
    }
    if input.color.len() != 7
        || !input.color.starts_with('#')
        || !input.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("请选择有效颜色".into());
    }
    if !["public", "anonymous", "excluded"].contains(&input.share_mode.as_str()) {
        return Err("分享方式无效".into());
    }
    if input.id.is_empty() {
        input.id = db::new_id();
    }
    c.execute("INSERT INTO growth_categories VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET name=excluded.name,color=excluded.color,share_mode=excluded.share_mode",params![input.id,input.name,input.color,input.share_mode]).map_err(|e|format!("分类保存失败（名称不能重复）：{e}"))?;
    catalog(c)
}
fn assign_core(c: &Connection, input: Classification) -> Result<Catalog, String> {
    let table = match input.entity_kind.as_str() {
        "habit" => "habits",
        "goal" => "long_term_goals",
        _ => return Err("分类对象无效".into()),
    };
    let exists: bool = c
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"),
            [&input.entity_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !exists {
        return Err("事项不存在".into());
    }
    c.execute("INSERT INTO growth_classifications VALUES(?1,?2,?3,?4) ON CONFLICT(entity_id) DO UPDATE SET category_id=excluded.category_id,share_name=excluded.share_name",params![input.entity_id,input.entity_kind,input.category_id,input.share_name]).map_err(|e|e.to_string())?;
    catalog(c)
}
#[tauri::command]
pub fn get_growth_catalog(database: State<'_, Database>) -> Result<Catalog, String> {
    let c = database.0.lock().map_err(|e| e.to_string())?;
    catalog(&c)
}
#[tauri::command]
pub fn save_growth_category(
    database: State<'_, Database>,
    input: Category,
) -> Result<Catalog, String> {
    let c = database.0.lock().map_err(|e| e.to_string())?;
    save_core(&c, input)
}
#[tauri::command]
pub fn assign_growth_category(
    database: State<'_, Database>,
    input: Classification,
) -> Result<Catalog, String> {
    let c = database.0.lock().map_err(|e| e.to_string())?;
    assign_core(&c, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn categories_validate_and_migration_preserves_records() {
        let c = Connection::open_in_memory().unwrap();
        db::initialize(&c).unwrap();
        let input = Category {
            id: "c".into(),
            name: "生活".into(),
            color: "#22aa33".into(),
            share_mode: "anonymous".into(),
        };
        save_core(&c, input.clone()).unwrap();
        db::initialize(&c).unwrap();
        assert_eq!(catalog(&c).unwrap().categories.len(), 1);
        let mut bad = input;
        bad.color = "javascript:x".into();
        assert!(save_core(&c, bad).is_err());
    }
}
