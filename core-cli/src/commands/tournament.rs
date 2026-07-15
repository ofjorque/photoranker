//! `tournament-next` / `tournament-result` / `ranking` / `tournament-status` —
//! ver docs/fase3-torneo.md, "Torneos Jerárquicos (Weng-Lin vía `skillratings`)".

use crate::config::Config;
use crate::db;
use crate::error::{AppError, AppResult};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use skillratings::{
    MultiTeamOutcome,
    weng_lin::{WengLinConfig, WengLinRating, weng_lin_multi_team},
};
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Imagen candidata a formar parte de un grupo, ya en el orden de prioridad de
/// selección (`sigma` desc, `last_compared_at` asc con `NULL` primero).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub id: i64,
    pub mu: f64,
}

/// PRNG determinista minúsculo (xorshift64) para elegir la 5ª imagen del
/// grupo sin depender de un crate adicional (`rand` no está en la lista de
/// crates oficiales de conventions.md) — el `seed` es lo único que decide el
/// resultado, lo que además hace la función testeable.
fn next_pseudo_random(seed: u64) -> u64 {
    let mut x = seed | 1;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

/// Selección pura del grupo de 2-5 imágenes para `tournament-next` (ver
/// fase3-torneo.md, paso 2). `priority_ordered` ya viene ordenado por
/// prioridad de selección; esta función solo decide *cuáles* entran al grupo.
/// Nunca duplica un `id`. Sin acceso a la BD para poder testearla de forma
/// aislada (ver "Cómo probar" en conventions.md).
pub fn select_group(priority_ordered: &[Candidate], rng_seed: u64) -> Vec<i64> {
    if priority_ordered.len() < 2 {
        return Vec::new();
    }

    let seed = priority_ordered[0];
    let mut remaining: Vec<Candidate> = priority_ordered[1..].to_vec();
    remaining.sort_by(|a, b| (a.mu - seed.mu).abs().total_cmp(&(b.mu - seed.mu).abs()));

    let mut threshold = 5.0_f64;
    let neighbor_count = loop {
        let within = remaining
            .iter()
            .take_while(|c| (c.mu - seed.mu).abs() <= threshold)
            .count();
        if within >= 3 || within == remaining.len() {
            break within.min(3);
        }
        threshold += 2.0;
    };

    let mut chosen: Vec<Candidate> = Vec::with_capacity(5);
    chosen.push(seed);
    chosen.extend(remaining.drain(0..neighbor_count));

    // `remaining` ahora son las imágenes que no entraron como semilla/vecinas;
    // de ahí sale la 5ª imagen "aleatoria" con mu suficientemente distinto.
    if !remaining.is_empty() {
        let avg_mu: f64 = chosen.iter().map(|c| c.mu).sum::<f64>() / chosen.len() as f64;
        let qualifying: Vec<&Candidate> = remaining
            .iter()
            .filter(|c| (c.mu - avg_mu).abs() >= 10.0)
            .collect();
        let pool: Vec<&Candidate> = if qualifying.is_empty() {
            remaining.iter().collect()
        } else {
            qualifying
        };
        let idx = (next_pseudo_random(rng_seed) as usize) % pool.len();
        chosen.push(*pool[idx]);
    }

    let mut seen = HashSet::new();
    chosen
        .into_iter()
        .filter(|c| seen.insert(c.id))
        .map(|c| c.id)
        .collect()
}

fn rng_seed_from_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

/// `tournament-next`: arma el siguiente grupo (2-5 imágenes) y lo registra en
/// `pending_tournament_groups`. `data=null` si queda menos de 2 imágenes
/// activas disponibles (ver fase3-torneo.md, "Grupos incompletos").
pub fn next(conn: &mut Connection) -> AppResult<Value> {
    let priority_ordered: Vec<Candidate> = {
        let mut stmt = conn.prepare(
            "SELECT id, mu FROM images \
             WHERE rejected = 0 AND stalled = 0 AND missing = 0 AND thumbnail_status = 'ok' \
             ORDER BY sigma DESC, \
                      CASE WHEN last_compared_at IS NULL THEN 0 ELSE 1 END ASC, \
                      last_compared_at ASC",
        )?;
        stmt.query_map([], |row| {
            Ok(Candidate {
                id: row.get(0)?,
                mu: row.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect()
    };

    let group_ids = select_group(&priority_ordered, rng_seed_from_clock());
    if group_ids.is_empty() {
        return Ok(Value::Null);
    }

    let group_id = uuid::Uuid::new_v4().to_string();

    let images: Vec<Value> = {
        let mut stmt = conn.prepare("SELECT id, file_path, mu, sigma FROM images WHERE id = ?1")?;
        group_ids
            .iter()
            .map(|id| {
                stmt.query_row(params![id], |row| {
                    Ok(json!({
                        "id": row.get::<_, i64>(0)?,
                        "file_path": row.get::<_, String>(1)?,
                        "mu": row.get::<_, f64>(2)?,
                        "sigma": row.get::<_, f64>(3)?,
                    }))
                })
                .map_err(AppError::from)
            })
            .collect::<AppResult<Vec<_>>>()?
    };

    let tx = conn.transaction()?;
    for id in &group_ids {
        tx.execute(
            "INSERT INTO pending_tournament_groups (group_id, image_id) VALUES (?1, ?2)",
            params![group_id, id],
        )?;
    }
    tx.commit()?;

    Ok(json!({
        "group_id": group_id,
        "images": images,
    }))
}

struct ImageState {
    mu: f64,
    sigma: f64,
    rejected: i64,
    stall_counter: i64,
    stalled: i64,
}

fn fetch_image_state(conn: &Connection, id: i64) -> AppResult<ImageState> {
    conn.query_row(
        "SELECT mu, sigma, rejected, stall_counter, stalled FROM images WHERE id = ?1",
        params![id],
        |row| {
            Ok(ImageState {
                mu: row.get(0)?,
                sigma: row.get(1)?,
                rejected: row.get(2)?,
                stall_counter: row.get(3)?,
                stalled: row.get(4)?,
            })
        },
    )
    .map_err(|_| AppError::ImageNotFound(id))
}

/// Valida y aplica el resultado de un grupo (ver fase3-torneo.md, checklist:
/// "Validación estricta antes de calcular"). Cualquier fallo de validación
/// devuelve `AppError::InvalidRanking` (`code="INVALID_RANKING"`) sin tocar la BD.
pub fn result(
    conn: &mut Connection,
    db_path: &Path,
    cfg: &Config,
    group_id: &str,
    ranking: &[(i64, i64)],
) -> AppResult<Value> {
    let group_members: HashSet<i64> = {
        let mut stmt =
            conn.prepare("SELECT image_id FROM pending_tournament_groups WHERE group_id = ?1")?;
        stmt.query_map(params![group_id], |row| row.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect()
    };
    if group_members.is_empty() {
        return Err(AppError::InvalidRanking(format!(
            "group_id '{group_id}' no existe"
        )));
    }

    let resolved: i64 = conn.query_row(
        "SELECT resolved FROM pending_tournament_groups WHERE group_id = ?1 LIMIT 1",
        params![group_id],
        |r| r.get(0),
    )?;
    if resolved != 0 {
        return Err(AppError::InvalidRanking(format!(
            "group_id '{group_id}' ya fue resuelto por un tournament-result anterior"
        )));
    }

    let ranking_ids: HashSet<i64> = ranking.iter().map(|(id, _)| *id).collect();
    if ranking_ids != group_members {
        return Err(AppError::InvalidRanking(
            "el conjunto de image_id en --ranking no coincide con las imágenes del grupo"
                .to_string(),
        ));
    }

    let mut positions: Vec<i64> = ranking.iter().map(|(_, pos)| *pos).collect();
    positions.sort_unstable();
    positions.dedup();
    for (i, pos) in positions.iter().enumerate() {
        if *pos != (i as i64 + 1) {
            return Err(AppError::InvalidRanking(
                "las posiciones deben ser enteros contiguos empezando en 1 (se permiten empates)"
                    .to_string(),
            ));
        }
    }

    let states: Vec<ImageState> = ranking
        .iter()
        .map(|(id, _)| fetch_image_state(conn, *id))
        .collect::<AppResult<Vec<_>>>()?;

    let teams: Vec<[WengLinRating; 1]> = states
        .iter()
        .map(|s| {
            [WengLinRating {
                rating: s.mu,
                uncertainty: s.sigma,
            }]
        })
        .collect();
    let rating_groups: Vec<(&[WengLinRating], MultiTeamOutcome)> = teams
        .iter()
        .zip(ranking.iter())
        .map(|(team, (_, pos))| (team.as_slice(), MultiTeamOutcome::new(*pos as usize)))
        .collect();
    let weng_lin_config = WengLinConfig {
        beta: cfg.weng_lin_beta,
        ..Default::default()
    };
    let updated = weng_lin_multi_team(&rating_groups, &weng_lin_config);

    db::backup(conn, db_path)?;

    let tx = conn.transaction()?;
    for (i, (id, pos)) in ranking.iter().enumerate() {
        let new_rating = updated[i][0];
        let old = &states[i];
        let improvement = if old.sigma > 0.0 {
            (old.sigma - new_rating.uncertainty) / old.sigma
        } else {
            0.0
        };
        let new_stall_counter = if improvement > 0.05 {
            0
        } else {
            old.stall_counter + 1
        };
        let new_stalled = if old.stalled == 1 || new_stall_counter >= cfg.stall_rounds as i64 {
            1
        } else {
            0
        };

        tx.execute(
            "UPDATE images SET mu = ?1, sigma = ?2, last_compared_at = CURRENT_TIMESTAMP, \
             stall_counter = ?3, stalled = ?4 WHERE id = ?5",
            params![
                new_rating.rating,
                new_rating.uncertainty,
                new_stall_counter,
                new_stalled,
                id
            ],
        )?;
        tx.execute(
            "INSERT INTO tournament_matches (group_id, image_id, rank_position) VALUES (?1, ?2, ?3)",
            params![group_id, id, pos],
        )?;
        tx.execute(
            "INSERT INTO pending_global_sync (image_id, mu, rejected) VALUES (?1, ?2, ?3) \
             ON CONFLICT(image_id) DO UPDATE SET mu = excluded.mu, rejected = excluded.rejected, \
             queued_at = CURRENT_TIMESTAMP",
            params![id, new_rating.rating, old.rejected],
        )?;
    }
    tx.execute(
        "UPDATE pending_tournament_groups SET resolved = 1 WHERE group_id = ?1",
        params![group_id],
    )?;
    tx.execute(
        "UPDATE project_meta SET pending_sync_count = pending_sync_count + 1",
        [],
    )?;
    tx.commit()?;

    let pending_count: i64 = conn.query_row(
        "SELECT pending_sync_count FROM project_meta LIMIT 1",
        [],
        |r| r.get(0),
    )?;
    let synced = if pending_count >= cfg.global_sync_every as i64 {
        flush_pending_sync(conn, db_path)?
    } else {
        0
    };

    Ok(json!({
        "group_id": group_id,
        "updated": ranking.iter().zip(updated.iter()).map(|((id, pos), rating)| {
            json!({
                "id": id,
                "rank_position": pos,
                "mu": rating[0].rating,
                "sigma": rating[0].uncertainty,
            })
        }).collect::<Vec<_>>(),
        "global_sync": { "flushed": synced, "pending": if synced > 0 { 0 } else { pending_count } },
    }))
}

/// Vuelca `pending_global_sync` hacia `~/.photoranker/global_index.sqlite` en
/// una sola transacción por lote, con reintento ante `SQLITE_BUSY` (ver
/// "Modelo de concurrencia" en conventions.md). Devuelve cuántas filas se
/// sincronizaron (0 si no había nada pendiente).
pub(crate) fn flush_pending_sync(conn: &mut Connection, db_path: &Path) -> AppResult<usize> {
    let project_id: String =
        conn.query_row("SELECT project_id FROM project_meta LIMIT 1", [], |r| {
            r.get(0)
        })?;

    let rows: Vec<(i64, String, f64, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT p.image_id, i.file_path, p.mu, p.rejected \
             FROM pending_global_sync p JOIN images i ON i.id = p.image_id",
        )?;
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect()
    };

    if rows.is_empty() {
        conn.execute("UPDATE project_meta SET pending_sync_count = 0", [])?;
        return Ok(0);
    }

    let db_path_str = db_path.display().to_string();
    let mut global_conn = db::open_global()?;

    let mut attempt = 0;
    loop {
        let outcome: rusqlite::Result<()> = (|| {
            let gtx = global_conn.transaction()?;
            for (image_id, file_path, mu, rejected) in &rows {
                gtx.execute(
                    "INSERT INTO global_ratings \
                     (project_id, source_db_path, image_id, file_path, mu, rejected) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                     ON CONFLICT(project_id, image_id) DO UPDATE SET \
                        mu = excluded.mu, rejected = excluded.rejected, \
                        source_db_path = excluded.source_db_path, updated_at = CURRENT_TIMESTAMP",
                    params![project_id, db_path_str, image_id, file_path, mu, rejected],
                )?;
            }
            gtx.commit()
        })();

        match outcome {
            Ok(()) => break,
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::DatabaseBusy && attempt < 3 =>
            {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(50 * attempt as u64));
            }
            Err(e) => return Err(AppError::from(e)),
        }
    }

    conn.execute("DELETE FROM pending_global_sync", [])?;
    conn.execute("UPDATE project_meta SET pending_sync_count = 0", [])?;
    Ok(rows.len())
}

/// `ranking`: orden en vivo por `mu` descendente, desempate por `sigma`
/// ascendente y luego `image_id` (determinista, ver cli-reference.md).
/// Fuerza un flush de la cola de sincronización pendiente antes de leer (ver
/// fase3-torneo.md, "Sincronización con el índice global").
pub fn ranking(conn: &mut Connection, db_path: &Path) -> AppResult<Value> {
    flush_pending_sync(conn, db_path)?;

    let mut stmt = conn.prepare(
        "SELECT id, file_path, mu, sigma, rejected, stalled FROM images \
         WHERE missing = 0 ORDER BY mu DESC, sigma ASC, id ASC",
    )?;
    let rows: Vec<Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "file_path": row.get::<_, String>(1)?,
                "mu": row.get::<_, f64>(2)?,
                "sigma": row.get::<_, f64>(3)?,
                "rejected": row.get::<_, i64>(4)? != 0,
                "stalled": row.get::<_, i64>(5)? != 0,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(json!(rows))
}

/// `tournament-status`: progreso de la sesión y motivo de parada
/// (`converged`/`stalled`/`timeout`/`in_progress`) — ver fase3-torneo.md,
/// "Criterio de parada". Fuerza flush de la cola pendiente antes de leer.
pub fn status(conn: &mut Connection, db_path: &Path, cfg: &Config) -> AppResult<Value> {
    flush_pending_sync(conn, db_path)?;

    let total_images: i64 =
        conn.query_row("SELECT COUNT(*) FROM images WHERE missing = 0", [], |r| {
            r.get(0)
        })?;
    let active_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM images \
         WHERE rejected = 0 AND stalled = 0 AND missing = 0 AND thumbnail_status = 'ok'",
        [],
        |r| r.get(0),
    )?;
    let stalled_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM images WHERE stalled = 1", [], |r| {
            r.get(0)
        })?;
    let converged_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM images \
         WHERE rejected = 0 AND stalled = 0 AND missing = 0 AND thumbnail_status = 'ok' \
           AND sigma < ?1",
        params![cfg.sigma_stop_threshold],
        |r| r.get(0),
    )?;
    let rounds_completed: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT group_id) FROM pending_tournament_groups WHERE resolved = 1",
        [],
        |r| r.get(0),
    )?;

    let max_rounds = cfg.max_rounds_multiplier as i64 * active_count;
    let convergence_ratio = if active_count > 0 {
        converged_count as f64 / active_count as f64
    } else {
        1.0
    };

    // Prioridad: sin imágenes activas o umbral de convergencia alcanzado
    // ("converged") > tope de rondas alcanzado ("timeout") > menos de 2
    // imágenes activas restantes, ninguna puede seguir formando grupos
    // ("stalled") > sesión aún en curso.
    let reason = if active_count == 0 || convergence_ratio >= cfg.convergence_fraction {
        "converged"
    } else if max_rounds > 0 && rounds_completed >= max_rounds {
        "timeout"
    } else if active_count < 2 {
        "stalled"
    } else {
        "in_progress"
    };

    Ok(json!({
        "total_images": total_images,
        "active_images": active_count,
        "stalled_images": stalled_count,
        "converged_images": converged_count,
        "convergence_ratio": convergence_ratio,
        "rounds_completed": rounds_completed,
        "max_rounds": max_rounds,
        "status": reason,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checklist de fase3-torneo.md, primer ítem: "no asumir" que
    /// `weng_lin_multi_team` trata los empates de forma simétrica sin
    /// probarlo. Dos equipos con el mismo `MultiTeamOutcome` (empate en 1°)
    /// y el mismo rating de entrada deben terminar con el mismo mu/sigma.
    #[test]
    fn weng_lin_multi_team_treats_ties_symmetrically() {
        let team_a = [WengLinRating {
            rating: 25.0,
            uncertainty: 8.33,
        }];
        let team_b = [WengLinRating {
            rating: 25.0,
            uncertainty: 8.33,
        }];
        let team_c = [WengLinRating {
            rating: 25.0,
            uncertainty: 8.33,
        }];

        let rating_groups: Vec<(&[WengLinRating], MultiTeamOutcome)> = vec![
            (&team_a[..], MultiTeamOutcome::new(1)),
            (&team_b[..], MultiTeamOutcome::new(1)),
            (&team_c[..], MultiTeamOutcome::new(2)),
        ];
        let config = WengLinConfig {
            beta: 4.1667,
            ..Default::default()
        };
        let updated = weng_lin_multi_team(&rating_groups, &config);

        assert_eq!(updated.len(), 3);
        let a = updated[0][0];
        let b = updated[1][0];
        assert!(
            (a.rating - b.rating).abs() < 1e-9,
            "mu de empatados debería ser idéntico: {} vs {}",
            a.rating,
            b.rating
        );
        assert!(
            (a.uncertainty - b.uncertainty).abs() < 1e-9,
            "sigma de empatados debería ser idéntico: {} vs {}",
            a.uncertainty,
            b.uncertainty
        );
        // Y ambos deberían haber subido de rating respecto al 3° (que quedó 2°).
        let c = updated[2][0];
        assert!(a.rating > c.rating);
    }

    #[test]
    fn weng_lin_multi_team_asymmetric_ranks_break_the_tie() {
        // Contraprueba: si NO empatan (ranks 1,2,3 distintos), el primero debe
        // terminar con mayor mu que el segundo — confirma que el test anterior
        // realmente está detectando el trato de empates y no un artefacto del
        // crate que iguala todo.
        let team_a = [WengLinRating {
            rating: 25.0,
            uncertainty: 8.33,
        }];
        let team_b = [WengLinRating {
            rating: 25.0,
            uncertainty: 8.33,
        }];
        let rating_groups: Vec<(&[WengLinRating], MultiTeamOutcome)> = vec![
            (&team_a[..], MultiTeamOutcome::new(1)),
            (&team_b[..], MultiTeamOutcome::new(2)),
        ];
        let config = WengLinConfig::new();
        let updated = weng_lin_multi_team(&rating_groups, &config);
        assert!(updated[0][0].rating > updated[1][0].rating);
    }

    fn candidates(pairs: &[(i64, f64)]) -> Vec<Candidate> {
        pairs.iter().map(|&(id, mu)| Candidate { id, mu }).collect()
    }

    #[test]
    fn select_group_returns_empty_with_fewer_than_two_candidates() {
        assert_eq!(select_group(&candidates(&[]), 1), Vec::<i64>::new());
        assert_eq!(
            select_group(&candidates(&[(1, 25.0)]), 1),
            Vec::<i64>::new()
        );
    }

    #[test]
    fn select_group_never_duplicates_ids_and_caps_at_five() {
        let pool: Vec<(i64, f64)> = (1..=20).map(|i| (i, 20.0 + i as f64)).collect();
        let group = select_group(&candidates(&pool), 42);
        assert!(group.len() >= 2 && group.len() <= 5);
        let unique: HashSet<i64> = group.iter().cloned().collect();
        assert_eq!(unique.len(), group.len());
    }

    #[test]
    fn select_group_picks_seed_and_prefers_close_mu_neighbors() {
        // Semilla mu=25; vecinos cercanos en 26,27,28; uno lejano en 60 que
        // debería quedar disponible para la 5ª posición, no como "vecino".
        let pool = candidates(&[(1, 25.0), (2, 26.0), (3, 27.0), (4, 28.0), (5, 60.0)]);
        let group = select_group(&pool, 7);
        assert_eq!(group.len(), 5);
        assert_eq!(group[0], 1); // la semilla siempre es la primera de priority_ordered
        assert!(group[1..4].iter().all(|id| [2, 3, 4].contains(id)));
        assert_eq!(group[4], 5); // única candidata que queda para la 5ª posición
    }

    #[test]
    fn select_group_relaxes_threshold_when_not_enough_close_neighbors() {
        // Ningún candidato está a <=5 de la semilla; con la relajación en
        // pasos de +2 debería terminar incluyendo a los 3 más cercanos igual.
        let pool = candidates(&[(1, 0.0), (2, 20.0), (3, 22.0), (4, 24.0)]);
        let group = select_group(&pool, 3);
        assert_eq!(group.len(), 4);
        assert_eq!(group[0], 1);
    }

    #[test]
    fn select_group_falls_back_when_no_image_differs_enough_for_fifth_slot() {
        // Todas las mu están muy parejas (torneo casi convergido): la 5ª
        // imagen no puede cumplir la diferencia de 10, pero igual se agrega.
        let pool = candidates(&[(1, 25.0), (2, 25.5), (3, 26.0), (4, 24.5), (5, 25.2)]);
        let group = select_group(&pool, 99);
        assert_eq!(group.len(), 5);
        let unique: HashSet<i64> = group.iter().cloned().collect();
        assert_eq!(unique.len(), 5);
    }

    #[test]
    fn select_group_dynamic_size_when_pool_smaller_than_five() {
        let pool = candidates(&[(1, 25.0), (2, 26.0), (3, 60.0)]);
        let group = select_group(&pool, 5);
        assert_eq!(group.len(), 3);
    }
}
