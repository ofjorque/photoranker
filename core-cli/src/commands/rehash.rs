//! `photoranker rehash --db` — ver docs/fase1-ingesta.md, "Recálculo de
//! pHash". No es parte del flujo normal de `init`; existe para corregir
//! `images.hash` ya guardado cuando cambia el algoritmo de pHash (ver el
//! propio `phash.rs`), sin tener que volver a escanear la carpeta ni tocar
//! los archivos originales en disco.

use crate::error::AppResult;
use crate::phash;
use rayon::prelude::*;
use rusqlite::{Connection, params};
use serde_json::json;

/// Recalcula `hash` para toda imagen con miniatura ya extraída
/// (`thumbnail_status='ok'`), decodificando `images.thumbnail` (el mismo
/// JPEG normalizado que ya se usó para el hash original) — no vuelve a leer
/// el archivo fuente ni toca `exif_json`/`image_quality_metrics`/`mu`/
/// `sigma`/`rejected`/`cluster_id`, así que no dispara `db::backup` (mismo
/// criterio que `retry-thumbnail`, ver commands/thumbnails.rs).
pub fn run(conn: &mut Connection) -> AppResult<serde_json::Value> {
    let rows: Vec<(i64, Vec<u8>)> = {
        let mut stmt = conn.prepare(
            "SELECT id, thumbnail FROM images WHERE thumbnail_status = 'ok' AND thumbnail IS NOT NULL",
        )?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect()
    };

    // rayon calcula en memoria; el hilo principal es el único que escribe a
    // SQLite, en una única transacción (ver conventions.md, "Concurrencia").
    let computed: Vec<(i64, Option<String>)> = rows
        .par_iter()
        .map(|(id, thumbnail_bytes)| {
            let hash = image::load_from_memory(thumbnail_bytes)
                .ok()
                .map(|img| phash::compute(&img));
            (*id, hash)
        })
        .collect();

    let tx = conn.transaction()?;
    let mut rehashed = 0u32;
    let mut failed_to_decode = 0u32;
    for (id, hash) in &computed {
        match hash {
            Some(h) => {
                tx.execute("UPDATE images SET hash = ?1 WHERE id = ?2", params![h, id])?;
                rehashed += 1;
            }
            None => {
                tracing::warn!(
                    image_id = id,
                    "rehash: no se pudo decodificar la miniatura guardada"
                );
                failed_to_decode += 1;
            }
        }
    }
    tx.commit()?;

    Ok(json!({
        "candidates": rows.len(),
        "rehashed": rehashed,
        "failed_to_decode": failed_to_decode,
    }))
}
