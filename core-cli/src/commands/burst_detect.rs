//! `photoranker burst-detect --threshold` — ver docs/fase1-ingesta.md,
//! "Detección y minitorneo de ráfagas".

use crate::db;
use crate::error::{AppError, AppResult};
use crate::phash;
use rusqlite::{Connection, params};
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;

struct DisjointSet {
    parent: Vec<usize>,
}

impl DisjointSet {
    fn new(n: usize) -> Self {
        DisjointSet {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra] = rb;
        }
    }
}

/// Agrupa por distancia normalizada de pHash (single-linkage / componentes
/// conexas) las imágenes activas (`missing=0`, con `hash` calculado) que aún no
/// pertenecen a ninguna ráfaga — así `burst-detect` es incremental, igual que
/// `init`: correrlo de nuevo tras agregar fotos nuevas no vuelve a agrupar lo
/// que ya se agrupó antes. Solo se registran como `bursts` las componentes de
/// tamaño >= 2 (una "ráfaga" de una sola foto no requiere minitorneo).
pub fn run(conn: &mut Connection, threshold: f64) -> AppResult<serde_json::Value> {
    let candidates: Vec<(i64, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, hash FROM images
             WHERE missing = 0 AND hash IS NOT NULL
               AND id NOT IN (SELECT image_id FROM burst_members)",
        )?;
        stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect()
    };

    let n = candidates.len();
    let mut dsu = DisjointSet::new(n);
    let mut pairwise_distance: HashMap<(usize, usize), f64> = HashMap::new();

    for i in 0..n {
        for j in (i + 1)..n {
            if let Some(distance) = phash::normalized_distance(&candidates[i].1, &candidates[j].1)
                && distance < threshold
            {
                dsu.union(i, j);
                pairwise_distance.insert((i, j), distance);
            }
        }
    }

    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = dsu.find(i);
        groups.entry(root).or_default().push(i);
    }

    let tx = conn.transaction()?;
    let mut bursts_created = 0u32;
    let mut images_grouped = 0u32;

    for members in groups.values() {
        if members.len() < 2 {
            continue;
        }
        tx.execute("INSERT INTO bursts (status) VALUES ('pending')", [])?;
        let burst_id = tx.last_insert_rowid();

        for &idx in members {
            let (image_id, _) = &candidates[idx];
            let best_similarity = members
                .iter()
                .filter(|&&other| other != idx)
                .filter_map(|&other| {
                    let key = if idx < other {
                        (idx, other)
                    } else {
                        (other, idx)
                    };
                    pairwise_distance.get(&key).copied()
                })
                .fold(f64::MAX, f64::min);
            tx.execute(
                "INSERT INTO burst_members (burst_id, image_id, similarity_score) VALUES (?1, ?2, ?3)",
                params![burst_id, image_id, best_similarity],
            )?;
            images_grouped += 1;
        }
        bursts_created += 1;
    }
    tx.commit()?;

    Ok(json!({
        "candidates_considered": n,
        "bursts_created": bursts_created,
        "images_grouped": images_grouped,
    }))
}

/// `list-bursts`: solo lectura, sin backup (igual que `list-failed-thumbnails`
/// — no está en la lista de comandos que disparan `db::backup`, ver checklist
/// de fase1-ingesta.md). Devuelve los bursts `pending` (los `completed` ya
/// fueron resueltos por `burst-tournament` y no necesitan volver a mostrarse)
/// junto con sus imágenes miembro, para que la GUI pueda armar el minitorneo
/// sin tener que hacer una llamada aparte por burst (ver fase5-gui.md).
pub fn list_pending(conn: &Connection) -> AppResult<serde_json::Value> {
    let mut burst_stmt = conn.prepare("SELECT id FROM bursts WHERE status = 'pending'")?;
    let burst_ids: Vec<i64> = burst_stmt
        .query_map([], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(burst_stmt);

    let mut member_stmt = conn.prepare(
        "SELECT images.id, images.file_path FROM burst_members
         JOIN images ON images.id = burst_members.image_id
         WHERE burst_members.burst_id = ?1
         ORDER BY images.id",
    )?;

    let bursts: Vec<serde_json::Value> = burst_ids
        .into_iter()
        .map(|burst_id| {
            let members: Vec<serde_json::Value> = member_stmt
                .query_map(params![burst_id], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "file_path": r.get::<_, String>(1)?,
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(json!({ "id": burst_id, "images": members }))
        })
        .collect::<AppResult<Vec<_>>>()?;

    Ok(json!(bursts))
}

/// `list-bursts-resolved`: solo lectura, sin backup — bursts ya resueltos por
/// `burst-tournament` (`status='completed'`), para la sección de "deshacer"
/// de la GUI (ver fase1-ingesta.md, "Excluir/deshacer bursts").
pub fn list_resolved(conn: &Connection) -> AppResult<serde_json::Value> {
    let mut burst_stmt = conn.prepare(
        "SELECT id, representative_image_id FROM bursts WHERE status = 'completed' ORDER BY id DESC",
    )?;
    let bursts_meta: Vec<(i64, Option<i64>)> = burst_stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(burst_stmt);

    let mut member_stmt = conn.prepare(
        "SELECT images.id, images.file_path, images.rejected FROM burst_members
         JOIN images ON images.id = burst_members.image_id
         WHERE burst_members.burst_id = ?1
         ORDER BY images.id",
    )?;

    let bursts: Vec<serde_json::Value> = bursts_meta
        .into_iter()
        .map(|(burst_id, representative_image_id)| {
            let members: Vec<serde_json::Value> = member_stmt
                .query_map(params![burst_id], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "file_path": r.get::<_, String>(1)?,
                        "rejected": r.get::<_, i64>(2)? != 0,
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(json!({
                "id": burst_id,
                "representative_image_id": representative_image_id,
                "images": members,
            }))
        })
        .collect::<AppResult<Vec<_>>>()?;

    Ok(json!(bursts))
}

/// `burst-exclude --burst-id --image-id ...`: saca imagen(es) de un burst
/// **pendiente** (antes de resolver `burst-tournament`) — quedan como
/// imágenes normales, ya elegibles para `tournament-next` sin ningún paso
/// adicional (el pool del torneo principal no filtra por `burst_members`).
/// Si tras excluir queda 1 solo miembro (o 0), el burst completo se disuelve
/// — una "ráfaga" de una sola foto no es válida (ver fase1-ingesta.md,
/// "Excluir/deshacer bursts").
pub fn exclude(
    conn: &mut Connection,
    burst_id: i64,
    image_ids: &[i64],
) -> AppResult<serde_json::Value> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM bursts WHERE id = ?1",
            params![burst_id],
            |r| r.get(0),
        )
        .ok();
    let Some(status) = status else {
        return Err(AppError::BurstNotFound(burst_id));
    };
    if status != "pending" {
        return Err(AppError::InvalidRanking(format!(
            "Burst {burst_id} ya fue resuelto por un minitorneo — usar burst-undo en vez de burst-exclude"
        )));
    }

    let tx = conn.transaction()?;
    for image_id in image_ids {
        tx.execute(
            "DELETE FROM burst_members WHERE burst_id = ?1 AND image_id = ?2",
            params![burst_id, image_id],
        )?;
    }
    let remaining: i64 = tx.query_row(
        "SELECT COUNT(*) FROM burst_members WHERE burst_id = ?1",
        params![burst_id],
        |r| r.get(0),
    )?;
    let dissolved = remaining < 2;
    if dissolved {
        tx.execute(
            "DELETE FROM burst_members WHERE burst_id = ?1",
            params![burst_id],
        )?;
        tx.execute("DELETE FROM bursts WHERE id = ?1", params![burst_id])?;
    }
    tx.commit()?;

    Ok(json!({
        "burst_id": burst_id,
        "excluded": image_ids,
        "burst_dissolved": dissolved,
    }))
}

/// `burst-undo --burst-id [--image-id ...]`: revierte una resolución de
/// `burst-tournament` (`status='completed'`, ver migración
/// `011_burst_exclusion.sql` para `rejected_before`).
///
/// - Sin `image_ids`: deshace el burst completo — todos los miembros
///   recuperan su `rejected_before`, y el burst vuelve a `status='pending'`
///   con `representative_image_id=NULL` (queda disponible para resolver de
///   nuevo).
/// - Con `image_ids`: deshace solo esas imágenes (recuperan `rejected_before`
///   y salen de `burst_members`, igual que `burst-exclude`); el resto del
///   burst permanece `completed`. Si alguna de las imágenes es la
///   representativa (`representative_image_id`), es un error explícito — no
///   hay una "segunda mejor" obvia, hay que deshacer el burst completo.
pub fn undo(
    conn: &mut Connection,
    db_path: &Path,
    burst_id: i64,
    image_ids: Option<&[i64]>,
) -> AppResult<serde_json::Value> {
    let (status, representative_image_id): (String, Option<i64>) = conn
        .query_row(
            "SELECT status, representative_image_id FROM bursts WHERE id = ?1",
            params![burst_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| AppError::BurstNotFound(burst_id))?;
    if status != "completed" {
        return Err(AppError::InvalidRanking(format!(
            "Burst {burst_id} no está resuelto (status='{status}'); no hay nada que deshacer"
        )));
    }

    db::backup(conn, db_path)?;

    match image_ids {
        None => {
            let members: Vec<(i64, Option<i64>)> = {
                let mut stmt = conn.prepare(
                    "SELECT image_id, rejected_before FROM burst_members WHERE burst_id = ?1",
                )?;
                stmt.query_map(params![burst_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .filter_map(|r| r.ok())
                    .collect()
            };
            let tx = conn.transaction()?;
            for (image_id, rejected_before) in &members {
                tx.execute(
                    "UPDATE images SET rejected = ?1 WHERE id = ?2",
                    params![rejected_before.unwrap_or(0), image_id],
                )?;
            }
            tx.execute(
                "UPDATE bursts SET status = 'pending', representative_image_id = NULL WHERE id = ?1",
                params![burst_id],
            )?;
            tx.commit()?;
            Ok(json!({
                "burst_id": burst_id,
                "reverted_images": members.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                "burst_status": "pending",
            }))
        }
        Some(ids) => {
            if let Some(rep) = representative_image_id
                && ids.contains(&rep)
            {
                return Err(AppError::InvalidRanking(format!(
                    "La imagen {rep} es la representativa del burst {burst_id}; deshacer el burst completo (sin --image-id) en vez de excluirla sola"
                )));
            }

            let tx = conn.transaction()?;
            let mut reverted = Vec::with_capacity(ids.len());
            for image_id in ids {
                let rejected_before: Option<i64> = tx
                    .query_row(
                        "SELECT rejected_before FROM burst_members WHERE burst_id = ?1 AND image_id = ?2",
                        params![burst_id, image_id],
                        |r| r.get(0),
                    )
                    .map_err(|_| {
                        AppError::InvalidRanking(format!(
                            "La imagen {image_id} no pertenece al burst {burst_id}"
                        ))
                    })?;
                tx.execute(
                    "UPDATE images SET rejected = ?1 WHERE id = ?2",
                    params![rejected_before.unwrap_or(0), image_id],
                )?;
                tx.execute(
                    "DELETE FROM burst_members WHERE burst_id = ?1 AND image_id = ?2",
                    params![burst_id, image_id],
                )?;
                reverted.push(*image_id);
            }
            tx.commit()?;
            Ok(json!({
                "burst_id": burst_id,
                "reverted_images": reverted,
                "burst_status": "completed",
            }))
        }
    }
}
