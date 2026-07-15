//! `export-xmp` — ver docs/fase4-exportacion.md.

use crate::commands::tournament::flush_pending_sync;
use crate::config::Config;
use crate::db;
use crate::error::AppResult;
use crate::xmp;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;

/// Mapeo provisional directo sobre `mu` (0-50, con clamping en los extremos),
/// usado mientras `global_ratings` no alcanza `min_global_sample` filas no
/// rechazadas (fase4-exportacion.md).
fn stars_from_mu_fixed(mu: f64) -> i32 {
    if mu < 10.0 {
        1
    } else if mu < 20.0 {
        2
    } else if mu < 30.0 {
        3
    } else if mu < 40.0 {
        4
    } else {
        5
    }
}

struct ExportRow {
    id: i64,
    file_path: String,
    mu: f64,
    sigma: f64,
    rejected: bool,
    own_cluster_id: Option<i64>,
}

/// Modo de cálculo de estrellas, decidido una sola vez por corrida de
/// `export-xmp` según el tamaño de `global_ratings` (fase4-exportacion.md).
enum StarsMode {
    /// `(project_id, image_id) -> estrellas`, ya resuelto vía `PERCENT_RANK()`
    /// sobre todo el índice global.
    Quantile(HashMap<(String, i64), i32>),
    FixedProvisional,
}

/// Ejecuta exactamente la consulta de referencia de fase4-exportacion.md
/// (mismo `CASE`/`PERCENT_RANK()`), agregando `project_id`/`image_id` a la
/// proyección para poder mapear el resultado de vuelta a las imágenes locales.
fn quantile_stars_by_project_and_image(
    global_conn: &Connection,
) -> AppResult<HashMap<(String, i64), i32>> {
    let mut stmt = global_conn.prepare(
        "SELECT project_id, image_id,
           CASE
             WHEN PERCENT_RANK() OVER (ORDER BY mu ASC) <= 0.10 THEN 1
             WHEN PERCENT_RANK() OVER (ORDER BY mu ASC) <= 0.35 THEN 2
             WHEN PERCENT_RANK() OVER (ORDER BY mu ASC) <= 0.75 THEN 3
             WHEN PERCENT_RANK() OVER (ORDER BY mu ASC) <= 0.95 THEN 4
             ELSE 5
           END AS stars
         FROM global_ratings
         WHERE rejected = 0",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (project_id, image_id, stars) = row?;
        map.insert((project_id, image_id), stars as i32);
    }
    Ok(map)
}

fn resolve_stars_mode(global_conn: &Connection, cfg: &Config) -> AppResult<StarsMode> {
    let non_rejected_count: i64 = global_conn.query_row(
        "SELECT COUNT(*) FROM global_ratings WHERE rejected = 0",
        [],
        |r| r.get(0),
    )?;
    if non_rejected_count < cfg.min_global_sample as i64 {
        Ok(StarsMode::FixedProvisional)
    } else {
        Ok(StarsMode::Quantile(quantile_stars_by_project_and_image(
            global_conn,
        )?))
    }
}

/// Estrellas para una imagen activa (no rechazada). Si está en modo cuantil
/// pero la imagen todavía no fue sincronizada al índice global (nunca
/// participó en una ronda de torneo, sigue en el `mu` por defecto), se usa el
/// mapeo fijo como respaldo solo para esa imagen — el spec no cubre este caso
/// explícitamente; queda señalado en la salida (`fallback_fixed_mapping`).
fn stars_for_active_image(
    mode: &StarsMode,
    project_id: &str,
    image_id: i64,
    mu: f64,
) -> (i32, bool) {
    match mode {
        StarsMode::FixedProvisional => (stars_from_mu_fixed(mu), false),
        StarsMode::Quantile(map) => match map.get(&(project_id.to_string(), image_id)) {
            Some(percentile_stars) => (*percentile_stars, false),
            None => (stars_from_mu_fixed(mu), true),
        },
    }
}

/// `cluster_id` heredado de la ganadora del burst para imágenes rechazadas
/// (ver fase4-exportacion.md, "Herencia de cluster_id/dc:subject"): join
/// exacto documentado, `image_id -> Option<cluster_id de la ganadora>`. Solo
/// cubre rechazadas que pertenecen a un burst resuelto.
fn inherited_cluster_ids(conn: &Connection) -> AppResult<HashMap<i64, Option<i64>>> {
    let mut stmt = conn.prepare(
        "SELECT rejected_img.id, winner.cluster_id
         FROM images AS rejected_img
         JOIN burst_members ON burst_members.image_id = rejected_img.id
         JOIN bursts ON bursts.id = burst_members.burst_id
         JOIN images AS winner ON winner.id = bursts.representative_image_id
         WHERE rejected_img.rejected = 1",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (id, cluster_id) = row?;
        map.insert(id, cluster_id);
    }
    Ok(map)
}

fn cluster_names(conn: &Connection) -> AppResult<HashMap<i64, Option<String>>> {
    let mut stmt = conn.prepare("SELECT id, name FROM clusters")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (id, name) = row?;
        map.insert(id, name);
    }
    Ok(map)
}

/// `export-xmp`: compila estrellas + `dc:subject` desde la BD local y el
/// índice global, y escribe/fusiona los sidecars `.xmp` (ver
/// fase4-exportacion.md, incluyendo la excepción de imágenes con
/// `thumbnail_status='failed'` o `missing=1`, que quedan fuera).
pub fn run(conn: &mut Connection, db_path: &Path, cfg: &Config) -> AppResult<Value> {
    flush_pending_sync(conn, db_path)?;

    let project_id: String =
        conn.query_row("SELECT project_id FROM project_meta LIMIT 1", [], |r| {
            r.get(0)
        })?;

    let rows: Vec<ExportRow> = {
        let mut stmt = conn.prepare(
            "SELECT id, file_path, mu, sigma, rejected, cluster_id FROM images \
             WHERE missing = 0 AND thumbnail_status != 'failed'",
        )?;
        stmt.query_map([], |row| {
            Ok(ExportRow {
                id: row.get(0)?,
                file_path: row.get(1)?,
                mu: row.get(2)?,
                sigma: row.get(3)?,
                rejected: row.get::<_, i64>(4)? != 0,
                own_cluster_id: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect()
    };

    let excluded_failed_thumbnail: i64 = conn.query_row(
        "SELECT COUNT(*) FROM images WHERE thumbnail_status = 'failed'",
        [],
        |r| r.get(0),
    )?;
    let excluded_missing: i64 =
        conn.query_row("SELECT COUNT(*) FROM images WHERE missing = 1", [], |r| {
            r.get(0)
        })?;

    let inherited = inherited_cluster_ids(conn)?;
    let names = cluster_names(conn)?;

    let global_conn = db::open_global()?;
    let mode = resolve_stars_mode(&global_conn, cfg)?;

    // Snapshot de posición, mismo orden que `ranking` (mu desc, sigma asc, id
    // asc), sobre todo lo que recibe un .xmp (ver database.md, "rank_order").
    let mut ordered_ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    {
        let by_id: HashMap<i64, (&f64, &f64)> =
            rows.iter().map(|r| (r.id, (&r.mu, &r.sigma))).collect();
        ordered_ids.sort_by(|a, b| {
            let (mu_a, sigma_a) = by_id[a];
            let (mu_b, sigma_b) = by_id[b];
            mu_b.partial_cmp(mu_a)
                .unwrap()
                .then(sigma_a.partial_cmp(sigma_b).unwrap())
                .then(a.cmp(b))
        });
    }
    let rank_order: HashMap<i64, i64> = ordered_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i as i64 + 1))
        .collect();

    let mut stars_breakdown: HashMap<i32, i64> = HashMap::new();
    let mut fallback_fixed_mapping_used = 0i64;
    let mut updates: Vec<(i64, i32, i64)> = Vec::with_capacity(rows.len());

    for row in &rows {
        let (stars, used_fallback) = if row.rejected {
            (-1, false)
        } else {
            stars_for_active_image(&mode, &project_id, row.id, row.mu)
        };
        if used_fallback {
            fallback_fixed_mapping_used += 1;
        }
        *stars_breakdown.entry(stars).or_insert(0) += 1;

        let effective_cluster_id = if row.rejected {
            inherited.get(&row.id).copied().flatten()
        } else {
            row.own_cluster_id
        };
        let subject_tags: Vec<String> = effective_cluster_id
            .and_then(|cid| names.get(&cid).cloned().flatten())
            .into_iter()
            .collect();

        xmp::write_sidecar(Path::new(&row.file_path), stars, &subject_tags)?;
        updates.push((row.id, stars, rank_order[&row.id]));
    }

    if !updates.is_empty() {
        db::backup(conn, db_path)?;
        let tx = conn.transaction()?;
        for (id, stars, order) in &updates {
            tx.execute(
                "UPDATE images SET rating = ?1, rank_order = ?2 WHERE id = ?3",
                params![stars, order, id],
            )?;
        }
        tx.commit()?;
    }

    Ok(json!({
        "written": updates.len(),
        "excluded_failed_thumbnail": excluded_failed_thumbnail,
        "excluded_missing": excluded_missing,
        "mode": match mode { StarsMode::Quantile(_) => "quantile", StarsMode::FixedProvisional => "fixed_provisional" },
        "fallback_fixed_mapping_used": fallback_fixed_mapping_used,
        "stars_breakdown": stars_breakdown.iter().map(|(k, v)| (k.to_string(), json!(*v))).collect::<serde_json::Map<String, Value>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ejercita la consulta SQL real (no una reimplementación en Rust, ver
    /// fase4-exportacion.md: "no una reimplementación manual del cálculo") con
    /// 20 filas sintéticas para que `PERCENT_RANK()` reparta exactamente en
    /// los cortes de la tabla de fase4-exportacion.md.
    #[test]
    fn quantile_query_matches_fase4_star_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE global_ratings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id TEXT NOT NULL,
                image_id INTEGER NOT NULL,
                mu REAL NOT NULL,
                rejected INTEGER DEFAULT 0
            )",
            [],
        )
        .unwrap();
        // 20 filas con mu 1..20 (percent_rank = (rank-1)/19), project_id fijo,
        // image_id = mu para poder verificar el mapeo de vuelta fácilmente.
        for mu in 1..=20 {
            conn.execute(
                "INSERT INTO global_ratings (project_id, image_id, mu, rejected) VALUES ('p', ?1, ?1, 0)",
                params![mu],
            )
            .unwrap();
        }
        // Fila rechazada: debe quedar excluida del cálculo de percentiles.
        conn.execute(
            "INSERT INTO global_ratings (project_id, image_id, mu, rejected) VALUES ('p', 999, 5, 1)",
            [],
        )
        .unwrap();

        let stars = quantile_stars_by_project_and_image(&conn).unwrap();

        assert_eq!(stars.len(), 20);
        assert!(!stars.contains_key(&("p".to_string(), 999)));
        // percent_rank=0 (mu=1, rank 1) -> <=0.10 -> 1 estrella.
        assert_eq!(stars[&("p".to_string(), 1)], 1);
        // percent_rank=1 (mu=20, último) -> 5 estrellas.
        assert_eq!(stars[&("p".to_string(), 20)], 5);
        // rank intermedio, percent_rank=0.5 -> dentro de 0.35..0.75 -> 3 estrellas.
        assert_eq!(stars[&("p".to_string(), 11)], 3);
    }

    #[test]
    fn fixed_mapping_matches_fase4_table_with_clamping() {
        assert_eq!(stars_from_mu_fixed(-5.0), 1);
        assert_eq!(stars_from_mu_fixed(0.0), 1);
        assert_eq!(stars_from_mu_fixed(9.9), 1);
        assert_eq!(stars_from_mu_fixed(15.0), 2);
        assert_eq!(stars_from_mu_fixed(25.0), 3);
        assert_eq!(stars_from_mu_fixed(35.0), 4);
        assert_eq!(stars_from_mu_fixed(45.0), 5);
        assert_eq!(stars_from_mu_fixed(60.0), 5);
    }
}
