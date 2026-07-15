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
