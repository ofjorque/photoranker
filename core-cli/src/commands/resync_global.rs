//! `resync-global --path <carpeta>` — ver docs/database.md,
//! "resync-global se simplifica": cosmético, no crítico para la corrección de
//! los cuantiles (que se apoyan en `project_id`, estable aunque la carpeta se
//! mueva o renombre). Solo refresca `source_db_path` en el índice global.

use crate::db;
use crate::error::AppResult;
use rusqlite::params;
use serde_json::json;
use std::path::Path;

pub fn run(path: &Path) -> AppResult<serde_json::Value> {
    let db_path = path.join(db::LOCAL_DB_FILENAME);
    let local_conn = db::open_local(&db_path)?;
    let project_id: String =
        local_conn.query_row("SELECT project_id FROM project_meta LIMIT 1", [], |r| {
            r.get(0)
        })?;

    let global_conn = db::open_global()?;
    let updated = global_conn.execute(
        "UPDATE global_ratings SET source_db_path = ?1, updated_at = CURRENT_TIMESTAMP \
         WHERE project_id = ?2",
        params![db_path.display().to_string(), project_id],
    )?;

    Ok(json!({
        "project_id": project_id,
        "source_db_path": db_path.display().to_string(),
        "rows_updated": updated,
    }))
}
