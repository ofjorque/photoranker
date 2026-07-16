//! `cluster --preview` / `cluster --k <N>` / `cluster-rename` — ver
//! docs/fase2-clustering.md, "Interfaz con R (clustMD)". Rust nunca calcula el
//! clustering; solo invoca `r/run_clustmd.R` como subproceso síncrono y
//! traduce su salida JSON al sobre estándar del CLI (docs/conventions.md).

use crate::config::Config;
use crate::error::{AppError, AppResult};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

/// Cuántas imágenes representativas devolver por cluster en `list-clusters`
/// (ver fase5-gui.md, "mostrar unas 3 o 4 fotos más representativas de cada
/// cluster para etiquetarlo").
const REPRESENTATIVE_IMAGES_PER_CLUSTER: usize = 4;

/// Ruta al script R, resuelta en tiempo de compilación relativa al crate
/// (mismo patrón que `include_str!` usa para las migraciones en `db/mod.rs`).
const R_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../r/run_clustmd.R");

/// Ejecuta `run_clustmd.R` con los argumentos posicionales dados y devuelve
/// su payload ya validado (`status` removido). La conexión abierta se recibe
/// solo para dejar constancia de que debe seguir viva (WAL) mientras el
/// subproceso de R lee/escribe el mismo archivo — ver "Modelo de
/// concurrencia" en docs/conventions.md.
fn run_r_script(
    _conn: &Connection,
    cfg: &Config,
    db_path: &Path,
    args: &[String],
) -> AppResult<Value> {
    let mut command = Command::new(&cfg.rscript_path);
    command.arg(R_SCRIPT).arg(db_path);
    for arg in args {
        command.arg(arg);
    }

    let output = command.output().map_err(|e| {
        AppError::RSubprocessFailed(format!(
            "no se pudo ejecutar '{}' (¿Rscript.exe en el PATH? ver rscript_path en config.toml): {e}",
            cfg.rscript_path
        ))
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        tracing::debug!(stderr = %stderr, "salida de stderr de run_clustmd.R");
    }

    let parsed: Value = serde_json::from_str(stdout.trim()).map_err(|e| {
        AppError::RSubprocessFailed(format!(
            "salida de run_clustmd.R no es JSON válido: {e}\nstdout: {stdout}\nstderr: {stderr}"
        ))
    })?;

    match parsed.get("status").and_then(Value::as_str) {
        Some("ok") => Ok(parsed),
        Some("error") => {
            let message = parsed
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("error desconocido en run_clustmd.R");
            Err(AppError::RSubprocessFailed(message.to_string()))
        }
        _ => Err(AppError::RSubprocessFailed(format!(
            "salida de run_clustmd.R sin campo 'status' reconocible: {stdout}"
        ))),
    }
}

fn strip_status(mut value: Value) -> Value {
    if let Value::Object(map) = &mut value {
        map.remove("status");
    }
    value
}

fn preview_raw(conn: &Connection, db_path: &Path, cfg: &Config) -> AppResult<Value> {
    run_r_script(
        conn,
        cfg,
        db_path,
        &[
            cfg.clustmd_seed.to_string(),
            "preview".to_string(),
            cfg.cluster_min.to_string(),
            cfg.cluster_max.to_string(),
            cfg.variable_null_threshold.to_string(),
        ],
    )
}

/// `cluster --preview`: corre clustMD para el rango `cluster_min..cluster_max`
/// y devuelve el BIC de cada k, sin comprometer resultados (ver
/// fase2-clustering.md).
pub fn preview(conn: &Connection, db_path: &Path, cfg: &Config) -> AppResult<Value> {
    let result = preview_raw(conn, db_path, cfg)?;
    Ok(strip_status(result))
}

/// Elige k por argmax(BIChat). clustMD reporta el BIC con la convención
/// estilo mclust (mayor = mejor ajuste) — ver la nota extensa en
/// `r/run_clustmd.R` sobre por qué esto no es "mínimo BIC" pese al texto
/// original de fase2-clustering.md (decisión explícita, confirmada con el
/// usuario del proyecto).
fn best_k_from_bic(preview: &Value) -> AppResult<u32> {
    let bic_by_k = preview
        .get("bic_by_k")
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::RSubprocessFailed("preview sin 'bic_by_k'".to_string()))?;

    if bic_by_k.is_empty() {
        return Err(AppError::RSubprocessFailed(
            "ningún valor de k convergió durante el preview; no se puede elegir automáticamente"
                .to_string(),
        ));
    }

    let mut best: Option<(u32, f64)> = None;
    for (k_str, bic_value) in bic_by_k {
        let k: u32 = k_str.parse().map_err(|_| {
            AppError::RSubprocessFailed(format!("k inválido en bic_by_k: '{k_str}'"))
        })?;
        let bic = bic_value.as_f64().ok_or_else(|| {
            AppError::RSubprocessFailed(format!("BIC no numérico para k={k_str}"))
        })?;
        if best.is_none_or(|(_, best_bic)| bic > best_bic) {
            best = Some((k, bic));
        }
    }

    best.map(|(k, _)| k)
        .ok_or_else(|| AppError::RSubprocessFailed("bic_by_k vacío".to_string()))
}

fn commit_raw(
    conn: &Connection,
    db_path: &Path,
    cfg: &Config,
    k: u32,
    probability_threshold: f64,
) -> AppResult<Value> {
    run_r_script(
        conn,
        cfg,
        db_path,
        &[
            cfg.clustmd_seed.to_string(),
            "commit".to_string(),
            k.to_string(),
            cfg.variable_null_threshold.to_string(),
            probability_threshold.to_string(),
        ],
    )
}

/// `cluster --k <N>` (o `cluster` sin `--k`, que primero corre un preview
/// interno y elige el k de mayor BIC como fallback automático, ver
/// fase2-clustering.md). `probability_threshold` (`None` = usar el default de
/// `config.toml`, `cluster_probability_threshold`): si la probabilidad
/// argmax de una imagen a su cluster asignado no lo supera, queda con
/// `cluster_id = NULL` en vez de forzarla al cluster de todos modos — ver
/// "Umbral de probabilidad de pertenencia" en fase2-clustering.md. El script
/// R consulta `cached_cluster_fits` antes de reajustar (ver
/// r/run_clustmd.R); el resultado indica `from_cache` para que quede
/// visible cuándo se evitó una corrida nueva de `clustMD`.
pub fn commit(
    conn: &mut Connection,
    db_path: &Path,
    cfg: &Config,
    k: Option<u32>,
    probability_threshold: Option<f64>,
) -> AppResult<Value> {
    let chosen_k = match k {
        Some(k) => k,
        None => {
            let preview = preview_raw(conn, db_path, cfg)?;
            best_k_from_bic(&preview)?
        }
    };
    let threshold = probability_threshold.unwrap_or(cfg.cluster_probability_threshold);
    let result = commit_raw(conn, db_path, cfg, chosen_k, threshold)?;
    Ok(strip_status(result))
}

/// `cluster-rename --id <N> --name "<nombre>"` — bautiza un cluster antes de
/// exportarlo como tag en `dc:subject` (ver fase4-exportacion.md).
pub fn rename(conn: &mut Connection, id: i64, name: &str) -> AppResult<Value> {
    let updated = conn.execute(
        "UPDATE clusters SET name = ?1 WHERE id = ?2",
        params![name, id],
    )?;
    if updated == 0 {
        return Err(AppError::ClusterNotFound(id));
    }
    Ok(serde_json::json!({ "id": id, "name": name }))
}

/// `list-clusters`: solo lectura, sin backup — devuelve cada cluster ya
/// comprometido (`cluster --k`) junto con sus `member_count` y hasta
/// `REPRESENTATIVE_IMAGES_PER_CLUSTER` imágenes de mayor `probability`
/// (argmax de pertenencia), para que la GUI pueda mostrarlas y ayudar a
/// elegir el nombre antes de `cluster-rename` (ver fase5-gui.md).
/// (cluster_id, cluster_name, image_id, probability, file_path)
type ClusterMemberRow = (i64, Option<String>, i64, Option<f64>, String);

pub fn list(conn: &Connection) -> AppResult<Value> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, ic.image_id, ic.probability, i.file_path \
         FROM clusters c \
         JOIN image_clusters ic ON ic.cluster_id = c.id \
         JOIN images i ON i.id = ic.image_id \
         ORDER BY c.id, ic.probability DESC, ic.image_id ASC",
    )?;
    let rows: Vec<ClusterMemberRow> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut clusters: Vec<Value> = Vec::new();
    let mut current_id: Option<i64> = None;
    let mut current_name: Option<String> = None;
    let mut current_images: Vec<Value> = Vec::new();
    let mut current_count: usize = 0;

    fn push_cluster(
        clusters: &mut Vec<Value>,
        id: i64,
        name: &Option<String>,
        images: &[Value],
        count: usize,
    ) {
        clusters.push(json!({
            "id": id,
            "name": name,
            "member_count": count,
            "representative_images": images,
        }));
    }

    for (cluster_id, name, image_id, probability, file_path) in rows {
        if current_id != Some(cluster_id) {
            if let Some(id) = current_id {
                push_cluster(
                    &mut clusters,
                    id,
                    &current_name,
                    &current_images,
                    current_count,
                );
            }
            current_id = Some(cluster_id);
            current_name = name;
            current_images = Vec::new();
            current_count = 0;
        }
        current_count += 1;
        if current_images.len() < REPRESENTATIVE_IMAGES_PER_CLUSTER {
            current_images.push(json!({
                "id": image_id,
                "file_path": file_path,
                "probability": probability,
            }));
        }
    }
    if let Some(id) = current_id {
        push_cluster(
            &mut clusters,
            id,
            &current_name,
            &current_images,
            current_count,
        );
    }

    Ok(json!(clusters))
}
