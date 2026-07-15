//! `photoranker prune` — ver docs/fase1-ingesta.md, "Fotos borradas o renombradas".

use crate::db;
use crate::error::AppResult;
use rusqlite::{Connection, params};
use serde_json::json;
use std::path::Path;

pub fn run(conn: &mut Connection, db_path: &Path) -> AppResult<serde_json::Value> {
    let project_id: Option<String> = conn
        .query_row("SELECT project_id FROM project_meta LIMIT 1", [], |r| {
            r.get(0)
        })
        .ok();

    let tx = conn.transaction()?;

    let mut stmt = tx.prepare("SELECT id, file_path FROM images WHERE missing = 0")?;
    let missing_ids: Vec<i64> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .filter(|(_, file_path)| !Path::new(file_path).is_file())
        .map(|(id, _)| id)
        .collect();
    drop(stmt);

    for id in &missing_ids {
        tx.execute("UPDATE images SET missing = 1 WHERE id = ?1", params![id])?;
    }
    tx.commit()?;

    if let Some(project_id) = project_id
        && !missing_ids.is_empty()
        && let Ok(global_conn) = db::open_global()
    {
        for id in &missing_ids {
            let _ = global_conn.execute(
                "DELETE FROM global_ratings WHERE project_id = ?1 AND image_id = ?2",
                params![project_id, id],
            );
        }
    }

    Ok(json!({
        "db_path": db_path.display().to_string(),
        "marked_missing": missing_ids.len(),
    }))
}
