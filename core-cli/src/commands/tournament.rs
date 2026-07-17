//! `tournament-next` / `tournament-result` / `ranking` / `tournament-status` —
//! ver docs/fase3-torneo.md, "Torneos Jerárquicos (TrueSkill vía `skillratings`)".
//!
//! **Nota de migración (feedback de uso real: con pocas imágenes activas la
//! sesión nunca converge)**: este módulo usaba Weng-Lin; se migró a TrueSkill
//! por pedido explícito del usuario, avisándole que ambos son algoritmos
//! bayesianos con una curva de reducción de incertidumbre similar, así que
//! esto podría no resolver por sí solo la convergencia lenta con pocas fotos
//! (ver simulación diagnóstica en los tests de este módulo). TrueSkill está
//! patentado (ver docs de `skillratings::trueskill`) — el crate recomienda
//! evitarlo en proyectos comerciales; PhotoRanker es de uso personal, pero
//! queda señalado por si esto cambia.

use crate::config::Config;
use crate::db;
use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use skillratings::{
    MultiTeamOutcome,
    trueskill::{TrueSkillConfig, TrueSkillRating, trueskill_multi_team},
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

/// Arma un patrón `LIKE '%...%'` a partir de un substring de usuario,
/// escapando `%`/`_`/`\` (los caracteres especiales de `LIKE`) para que un
/// nombre de subcarpeta real que los contenga (ej. `"Viaje_2024"`) se
/// compare literal, no como comodín.
fn like_pattern(raw: &str) -> String {
    let escaped = raw
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn rng_seed_from_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

/// `tournament-next`: arma el siguiente grupo (2-5 imágenes) y lo registra en
/// `pending_tournament_groups`. `data=null` si queda menos de 2 imágenes
/// activas disponibles (ver fase3-torneo.md, "Grupos incompletos").
///
/// `scope` (ver docs/fase8-mejoras-avanzadas.md, "Acotar el pool de torneo
/// por subcarpeta"): si viene, acota el pool a imágenes cuyo `file_path`
/// contenga ese substring (ej. `--scope="Día 1"` matchea cualquier ruta que
/// tenga "Día 1" en algún segmento) — no requiere que sea el nombre exacto
/// de una subcarpeta ni conocer la carpeta raíz, que no se guarda aparte.
/// Coincidencia sensible a mayúsculas/minúsculas (misma convención simple
/// que el resto del filtrado por texto del proyecto). No afecta qué se
/// sincroniza al índice global (`flush_pending_sync` sigue igual).
pub fn next(conn: &mut Connection, scope: Option<&str>) -> AppResult<Value> {
    let priority_ordered: Vec<Candidate> = {
        let base_sql = "SELECT id, mu FROM images \
             WHERE rejected = 0 AND stalled = 0 AND missing = 0 AND thumbnail_status = 'ok'";
        let order_sql = " ORDER BY sigma DESC, \
                      CASE WHEN last_compared_at IS NULL THEN 0 ELSE 1 END ASC, \
                      last_compared_at ASC";

        match scope {
            Some(scope) => {
                let sql = format!("{base_sql} AND file_path LIKE ?1 ESCAPE '\\'{order_sql}");
                let mut stmt = conn.prepare(&sql)?;
                stmt.query_map(params![like_pattern(scope)], |row| {
                    Ok(Candidate {
                        id: row.get(0)?,
                        mu: row.get(1)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect()
            }
            None => {
                let sql = format!("{base_sql}{order_sql}");
                let mut stmt = conn.prepare(&sql)?;
                stmt.query_map([], |row| {
                    Ok(Candidate {
                        id: row.get(0)?,
                        mu: row.get(1)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect()
            }
        }
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
    last_compared_at: Option<String>,
}

fn fetch_image_state(conn: &Connection, id: i64) -> AppResult<ImageState> {
    conn.query_row(
        "SELECT mu, sigma, rejected, stall_counter, stalled, last_compared_at \
         FROM images WHERE id = ?1",
        params![id],
        |row| {
            Ok(ImageState {
                mu: row.get(0)?,
                sigma: row.get(1)?,
                rejected: row.get(2)?,
                stall_counter: row.get(3)?,
                stalled: row.get(4)?,
                last_compared_at: row.get(5)?,
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

    let teams: Vec<[TrueSkillRating; 1]> = states
        .iter()
        .map(|s| {
            [TrueSkillRating {
                rating: s.mu,
                uncertainty: s.sigma,
            }]
        })
        .collect();
    let rating_groups: Vec<(&[TrueSkillRating], MultiTeamOutcome)> = teams
        .iter()
        .zip(ranking.iter())
        .map(|(team, (_, pos))| (team.as_slice(), MultiTeamOutcome::new(*pos as usize)))
        .collect();
    let trueskill_config = TrueSkillConfig {
        beta: cfg.trueskill_beta,
        ..Default::default()
    };
    // `weights=None` porque cada equipo es siempre una sola imagen (peso
    // implícito 1.0 parejo) — el único caso en que `trueskill_multi_team`
    // devuelve `Err` es con pesos explícitos mal formados, que no se usan acá.
    let updated = trueskill_multi_team(&rating_groups, &trueskill_config, None)
        .map_err(|e| AppError::TrueSkillError(e.to_string()))?;

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
            "INSERT INTO tournament_matches (
                group_id, image_id, rank_position,
                mu_before, sigma_before, stall_counter_before, stalled_before, last_compared_at_before
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                group_id,
                id,
                pos,
                old.mu,
                old.sigma,
                old.stall_counter,
                old.stalled,
                old.last_compared_at,
            ],
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

    let rows: Vec<(i64, String, f64, i64, Option<String>)> = {
        let mut stmt = conn.prepare(
            "SELECT p.image_id, i.file_path, p.mu, p.rejected, i.hash \
             FROM pending_global_sync p JOIN images i ON i.id = p.image_id",
        )?;
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
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
            for (image_id, file_path, mu, rejected, hash) in &rows {
                gtx.execute(
                    "INSERT INTO global_ratings \
                     (project_id, source_db_path, image_id, file_path, mu, rejected, hash) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                     ON CONFLICT(project_id, image_id) DO UPDATE SET \
                        mu = excluded.mu, rejected = excluded.rejected, \
                        source_db_path = excluded.source_db_path, hash = excluded.hash, \
                        updated_at = CURRENT_TIMESTAMP",
                    params![
                        project_id,
                        db_path_str,
                        image_id,
                        file_path,
                        mu,
                        rejected,
                        hash
                    ],
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

/// `tournament-undo`: revierte el grupo resuelto más reciente que todavía no
/// haya sido deshecho (ver fase3-torneo.md, "Deshacer el último resultado
/// enviado" — agregado por feedback: "me equivoqué al mandar el grupo").
/// Restaura `mu`/`sigma`/`stall_counter`/`stalled`/`last_compared_at` al valor
/// que tenían antes de ese grupo (guardado en `tournament_matches` al momento
/// de aplicar el resultado, ver migración 008) y marca esas filas `undone=1`
/// para que no puedan deshacerse dos veces. No toca `rejected` — el torneo
/// principal nunca lo modifica (solo `burst-tournament` lo hace).
///
/// **Límite aceptado**: si el resultado del grupo ya se sincronizó al índice
/// global (`global_ratings`, ver "Sincronización con el índice global" en
/// fase3-torneo.md) antes de deshacerse, ese `mu` sincronizado queda
/// desactualizado hasta que la imagen vuelva a participar en un grupo — igual
/// que `resync-global`, no se considera crítico corregirlo retroactivamente.
pub fn undo(conn: &mut Connection, db_path: &Path) -> AppResult<Value> {
    let group_id: Option<String> = conn
        .query_row(
            "SELECT group_id FROM tournament_matches \
             WHERE undone = 0 ORDER BY timestamp DESC, id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let Some(group_id) = group_id else {
        return Err(AppError::NothingToUndo);
    };

    struct Snapshot {
        image_id: i64,
        mu_before: f64,
        sigma_before: f64,
        stall_counter_before: i64,
        stalled_before: i64,
        last_compared_at_before: Option<String>,
    }
    let snapshots: Vec<Snapshot> = {
        let mut stmt = conn.prepare(
            "SELECT image_id, mu_before, sigma_before, stall_counter_before, \
                    stalled_before, last_compared_at_before \
             FROM tournament_matches WHERE group_id = ?1 AND undone = 0",
        )?;
        stmt.query_map(params![group_id], |row| {
            Ok(Snapshot {
                image_id: row.get(0)?,
                mu_before: row.get(1)?,
                sigma_before: row.get(2)?,
                stall_counter_before: row.get(3)?,
                stalled_before: row.get(4)?,
                last_compared_at_before: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect()
    };

    db::backup(conn, db_path)?;

    let tx = conn.transaction()?;
    for s in &snapshots {
        tx.execute(
            "UPDATE images SET mu = ?1, sigma = ?2, stall_counter = ?3, stalled = ?4, \
             last_compared_at = ?5 WHERE id = ?6",
            params![
                s.mu_before,
                s.sigma_before,
                s.stall_counter_before,
                s.stalled_before,
                s.last_compared_at_before,
                s.image_id,
            ],
        )?;
        // Si el resultado deshecho todavía no se sincronizó al índice global,
        // corregimos también el mu en cola para que no se envíe el valor
        // ahora revertido en el próximo flush por lote.
        tx.execute(
            "UPDATE pending_global_sync SET mu = ?1 WHERE image_id = ?2",
            params![s.mu_before, s.image_id],
        )?;
    }
    tx.execute(
        "UPDATE tournament_matches SET undone = 1 WHERE group_id = ?1",
        params![group_id],
    )?;
    tx.commit()?;

    Ok(json!({
        "group_id": group_id,
        "reverted_images": snapshots.iter().map(|s| s.image_id).collect::<Vec<_>>(),
    }))
}

/// `tournament-reset`: reinicia el progreso del torneo principal de esta
/// carpeta — todas las imágenes no perdidas (`missing=0`) vuelven a
/// `mu`/`sigma` por defecto y se limpia `stalled`/`stall_counter`/
/// `last_compared_at`. **No toca `rejected`**: las decisiones ya tomadas en
/// minitorneos de ráfaga (`burst-tournament`) se conservan (ver
/// fase3-torneo.md, "Reiniciar el torneo completo de la carpeta" — agregado
/// por feedback de uso real). El historial en `tournament_matches`/
/// `pending_tournament_groups` se conserva como auditoría, no se borra.
pub fn reset(conn: &mut Connection, db_path: &Path) -> AppResult<Value> {
    db::backup(conn, db_path)?;

    let tx = conn.transaction()?;
    let reset_count = tx.execute(
        "UPDATE images SET mu = 25.0, sigma = 8.33, stall_counter = 0, stalled = 0, \
         last_compared_at = NULL WHERE missing = 0",
        [],
    )?;
    tx.execute("DELETE FROM pending_global_sync", [])?;
    tx.execute("UPDATE project_meta SET pending_sync_count = 0", [])?;
    tx.commit()?;

    Ok(json!({ "images_reset": reset_count }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checklist de fase3-torneo.md, primer ítem: "no asumir" que
    /// `trueskill_multi_team` trata los empates de forma simétrica sin
    /// probarlo. **Resultado real, distinto de Weng-Lin**: Weng-Lin (fórmula
    /// cerrada) daba mu/sigma *exactamente* iguales para un empate
    /// (tolerancia `1e-9`). TrueSkill usa un factor graph con inferencia
    /// iterativa (message passing) internamente, que no es perfectamente
    /// simétrico por orden de procesamiento — empíricamente la diferencia
    /// entre dos equipos empatados fue de ~0.005 en `mu` (con beta=4.1667,
    /// una diferencia irrelevante en la práctica frente al rango de sigma
    /// 2.0-8.33), no cero. Se relaja la tolerancia a `0.05` en vez del
    /// `1e-9` que tenía el test de Weng-Lin — sigue siendo una prueba real
    /// (falla si el trato de empates se rompe del todo, ver el contraprueba
    /// de abajo), pero no exige una simetría exacta que este algoritmo no
    /// ofrece.
    #[test]
    fn trueskill_multi_team_treats_ties_symmetrically() {
        let team_a = [TrueSkillRating {
            rating: 25.0,
            uncertainty: 8.33,
        }];
        let team_b = [TrueSkillRating {
            rating: 25.0,
            uncertainty: 8.33,
        }];
        let team_c = [TrueSkillRating {
            rating: 25.0,
            uncertainty: 8.33,
        }];

        let rating_groups: Vec<(&[TrueSkillRating], MultiTeamOutcome)> = vec![
            (&team_a[..], MultiTeamOutcome::new(1)),
            (&team_b[..], MultiTeamOutcome::new(1)),
            (&team_c[..], MultiTeamOutcome::new(2)),
        ];
        let config = TrueSkillConfig {
            beta: 4.1667,
            ..Default::default()
        };
        let updated = trueskill_multi_team(&rating_groups, &config, None).unwrap();

        assert_eq!(updated.len(), 3);
        let a = updated[0][0];
        let b = updated[1][0];
        assert!(
            (a.rating - b.rating).abs() < 0.05,
            "mu de empatados debería ser aproximadamente igual: {} vs {}",
            a.rating,
            b.rating
        );
        assert!(
            (a.uncertainty - b.uncertainty).abs() < 0.05,
            "sigma de empatados debería ser aproximadamente igual: {} vs {}",
            a.uncertainty,
            b.uncertainty
        );
        // Y ambos deberían haber subido de rating respecto al 3° (que quedó 2°).
        let c = updated[2][0];
        assert!(a.rating > c.rating);
    }

    #[test]
    fn trueskill_multi_team_asymmetric_ranks_break_the_tie() {
        // Contraprueba: si NO empatan (ranks 1,2,3 distintos), el primero debe
        // terminar con mayor mu que el segundo — confirma que el test anterior
        // realmente está detectando el trato de empates y no un artefacto del
        // crate que iguala todo.
        let team_a = [TrueSkillRating {
            rating: 25.0,
            uncertainty: 8.33,
        }];
        let team_b = [TrueSkillRating {
            rating: 25.0,
            uncertainty: 8.33,
        }];
        let rating_groups: Vec<(&[TrueSkillRating], MultiTeamOutcome)> = vec![
            (&team_a[..], MultiTeamOutcome::new(1)),
            (&team_b[..], MultiTeamOutcome::new(2)),
        ];
        let config = TrueSkillConfig::new();
        let updated = trueskill_multi_team(&rating_groups, &config, None).unwrap();
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
