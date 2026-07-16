//! `photoranker burst-tournament --burst-id --ranking id:posición ...` — ver
//! docs/fase1-ingesta.md, "Detección y minitorneo de ráfagas".

use crate::db;
use crate::error::{AppError, AppResult};
use rusqlite::{Connection, params};
use serde_json::json;
use std::collections::HashSet;
use std::path::Path;

pub fn run(
    conn: &mut Connection,
    db_path: &Path,
    burst_id: i64,
    ranking: &[(i64, i64)],
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
    if status == "completed" {
        return Err(AppError::InvalidRanking(format!(
            "Burst {burst_id} ya fue resuelto por un minitorneo anterior"
        )));
    }

    let members: HashSet<i64> = {
        let mut stmt = conn.prepare("SELECT image_id FROM burst_members WHERE burst_id = ?1")?;
        stmt.query_map(params![burst_id], |row| row.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect()
    };

    let ranking_ids: HashSet<i64> = ranking.iter().map(|(id, _)| *id).collect();
    if ranking_ids != members {
        return Err(AppError::InvalidRanking(format!(
            "El ranking recibido no coincide con los miembros del burst {burst_id}"
        )));
    }

    let winners: Vec<i64> = ranking
        .iter()
        .filter(|(_, pos)| *pos == 1)
        .map(|(id, _)| *id)
        .collect();
    if winners.len() != 1 {
        return Err(AppError::InvalidRanking(
            "El minitorneo de ráfaga necesita exactamente una Campeona en la posición 1"
                .to_string(),
        ));
    }
    let winner = winners[0];

    db::backup(conn, db_path)?;

    let tx = conn.transaction()?;
    for &(image_id, _) in ranking {
        // Snapshot de `rejected` previo a la resolución (migración
        // 011_burst_exclusion.sql) — permite que `burst-undo` restaure el
        // valor real anterior en vez de asumir que siempre era 0 (ver
        // fase1-ingesta.md, "Excluir/deshacer bursts").
        let rejected_before: i64 = tx.query_row(
            "SELECT rejected FROM images WHERE id = ?1",
            params![image_id],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE burst_members SET rejected_before = ?1 WHERE burst_id = ?2 AND image_id = ?3",
            params![rejected_before, burst_id, image_id],
        )?;
        let rejected = i32::from(image_id != winner);
        tx.execute(
            "UPDATE images SET rejected = ?1 WHERE id = ?2",
            params![rejected, image_id],
        )?;
    }
    tx.execute(
        "UPDATE bursts SET representative_image_id = ?1, status = 'completed' WHERE id = ?2",
        params![winner, burst_id],
    )?;
    tx.commit()?;

    Ok(json!({
        "burst_id": burst_id,
        "representative_image_id": winner,
        "rejected": ranking.len() as i64 - 1,
    }))
}
