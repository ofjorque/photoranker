//! `list-duplicates` — detección de posibles duplicados entre carpetas vía
//! el índice global (ver docs/fase8-mejoras-avanzadas.md, "Detección de
//! duplicados"). Compara el pHash (`images.hash`, ya calculado en `init` y
//! sincronizado a `global_ratings.hash` junto con `mu`) de las imágenes
//! activas de la carpeta actual contra las de todas las demás carpetas ya
//! sincronizadas — distancia 0.0 se reporta como `exact` (mismo hash),
//! el resto por debajo de `threshold` como similar. Solo lectura, no toca
//! ninguna BD (ni local ni global).

use crate::error::AppResult;
use crate::phash;
use rusqlite::{Connection, params};
use serde_json::{Value, json};

struct GlobalRow {
    project_id: String,
    image_id: i64,
    file_path: String,
    source_db_path: Option<String>,
    hash: String,
}

/// `list-duplicates --db <ruta> [--threshold <f64>]`.
pub fn list(local_conn: &Connection, global_conn: &Connection, threshold: f64) -> AppResult<Value> {
    let project_id: String =
        local_conn.query_row("SELECT project_id FROM project_meta LIMIT 1", [], |r| {
            r.get(0)
        })?;

    let mine = load_rows(global_conn, &project_id, SameProject::Yes)?;
    let others = load_rows(global_conn, &project_id, SameProject::No)?;

    let mut matches = Vec::new();
    for a in &mine {
        for b in &others {
            let Some(distance) = phash::normalized_distance(&a.hash, &b.hash) else {
                continue;
            };
            if distance <= threshold {
                matches.push(json!({
                    "local_image_id": a.image_id,
                    "local_file_path": a.file_path,
                    "other_project_id": b.project_id,
                    "other_file_path": b.file_path,
                    "other_source_db_path": b.source_db_path,
                    "distance": distance,
                    "exact": distance == 0.0,
                }));
            }
        }
    }

    Ok(json!(matches))
}

enum SameProject {
    Yes,
    No,
}

/// Solo imágenes activas (`rejected = 0`) con hash ya sincronizado — las
/// filas viejas de instalaciones previas a agregar la columna `hash` (ver
/// `db::add_hash_column_if_missing`) todavía no tienen valor ahí hasta su
/// próxima sincronización, y no pueden compararse.
fn load_rows(
    global_conn: &Connection,
    project_id: &str,
    same_project: SameProject,
) -> AppResult<Vec<GlobalRow>> {
    let mut stmt = match same_project {
        SameProject::Yes => global_conn.prepare(
            "SELECT project_id, image_id, file_path, source_db_path, hash \
             FROM global_ratings WHERE project_id = ?1 AND rejected = 0 AND hash IS NOT NULL",
        )?,
        SameProject::No => global_conn.prepare(
            "SELECT project_id, image_id, file_path, source_db_path, hash \
             FROM global_ratings WHERE project_id != ?1 AND rejected = 0 AND hash IS NOT NULL",
        )?,
    };
    let rows = stmt
        .query_map(params![project_id], |r| {
            Ok(GlobalRow {
                project_id: r.get(0)?,
                image_id: r.get(1)?,
                file_path: r.get(2)?,
                source_db_path: r.get(3)?,
                hash: r.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}
