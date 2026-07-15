//! `photoranker burst-detect --threshold` — ver docs/fase1-ingesta.md,
//! "Detección y minitorneo de ráfagas".

use crate::error::AppResult;
use crate::phash;
use rusqlite::{Connection, params};
use serde_json::json;
use std::collections::HashMap;

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
