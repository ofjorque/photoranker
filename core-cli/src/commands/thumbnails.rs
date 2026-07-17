//! `list-failed-thumbnails` / `retry-thumbnail` / `get-thumbnail` /
//! `get-quality-metrics` — ver docs/fase1-ingesta.md, "Fotos con miniatura
//! fallida", docs/fase4-exportacion.md y docs/fase5-gui.md (panel de
//! referencia de calidad y miniaturas de la GUI).

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::{exif, phash, quality, thumbnail};
use base64::Engine;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::io::Cursor;
use std::path::Path;

/// `list-failed-thumbnails`: solo lectura, sin backup (ver "Modelo de
/// concurrencia" / checklist de fase1-ingesta.md — no está en la lista de
/// comandos que disparan `db::backup`).
pub fn list_failed(conn: &Connection) -> AppResult<Value> {
    let mut stmt =
        conn.prepare("SELECT id, file_path FROM images WHERE thumbnail_status = 'failed'")?;
    let rows: Vec<Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "file_path": row.get::<_, String>(1)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(json!(rows))
}

/// `retry-thumbnail --image-id <id>`: reintenta la extracción de miniatura +
/// pHash + métricas de calidad de una imagen con `thumbnail_status='failed'`
/// (u `ok`, de forma idempotente). No dispara `db::backup` — no toca `mu`,
/// `sigma`, `rejected` ni `cluster_id` (ver conventions.md, "Modelo de
/// concurrencia"). Si la extracción vuelve a fallar, devuelve
/// `AppError::ThumbnailFailed` (`code="THUMBNAIL_FAILED"`).
pub fn retry(conn: &mut Connection, cfg: &Config, image_id: i64) -> AppResult<Value> {
    let file_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM images WHERE id = ?1",
            params![image_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(file_path) = file_path else {
        return Err(AppError::ImageNotFound(image_id));
    };

    let path = Path::new(&file_path);
    let exif_data = exif::read(path);
    let exif_json = serde_json::to_string(&exif_data).unwrap_or_else(|_| "{}".to_string());

    let Ok(img) = thumbnail::extract_normalized(path, &exif_data, cfg.preview_size) else {
        tracing::warn!(file = %file_path, "retry-thumbnail: la extracción volvió a fallar");
        conn.execute(
            "UPDATE images SET thumbnail_status = 'failed', exif_json = ?1, \
             iso = ?2, aperture = ?3, focal_length = ?4 WHERE id = ?5",
            params![
                exif_json,
                exif_data.iso,
                exif_data.aperture,
                exif_data.focal_length,
                image_id
            ],
        )?;
        return Err(AppError::ThumbnailFailed(image_id));
    };

    let hash = phash::compute(&img);
    let metrics = quality::compute(&img);
    let mut buf = Vec::new();
    let thumbnail_bytes = img
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
        .ok()
        .map(|_| buf);

    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE images SET thumbnail = ?1, thumbnail_status = 'ok', hash = ?2, exif_json = ?3, \
         iso = ?4, aperture = ?5, focal_length = ?6 WHERE id = ?7",
        params![
            thumbnail_bytes,
            hash,
            exif_json,
            exif_data.iso,
            exif_data.aperture,
            exif_data.focal_length,
            image_id,
        ],
    )?;
    tx.execute(
        "INSERT INTO image_quality_metrics (
            image_id, sharpness, brightness, contrast, overexposed_pct, underexposed_pct,
            saturation, colorfulness, entropy, average_r, average_g, average_b, orientation
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(image_id) DO UPDATE SET
            sharpness = excluded.sharpness, brightness = excluded.brightness,
            contrast = excluded.contrast, overexposed_pct = excluded.overexposed_pct,
            underexposed_pct = excluded.underexposed_pct, saturation = excluded.saturation,
            colorfulness = excluded.colorfulness, entropy = excluded.entropy,
            average_r = excluded.average_r, average_g = excluded.average_g,
            average_b = excluded.average_b, orientation = excluded.orientation",
        params![
            image_id,
            metrics.sharpness,
            metrics.brightness,
            metrics.contrast,
            metrics.overexposed_pct,
            metrics.underexposed_pct,
            metrics.saturation,
            metrics.colorfulness,
            metrics.entropy,
            metrics.average_r,
            metrics.average_g,
            metrics.average_b,
            metrics.orientation.as_db_str(),
        ],
    )?;
    tx.commit()?;

    Ok(json!({
        "id": image_id,
        "thumbnail_status": "ok",
    }))
}

/// `get-thumbnail --image-id <id>`: única vía por la que la GUI (Tauri)
/// obtiene los bytes de `images.thumbnail` (JPEG normalizado) sin leer
/// `.photoranker.sqlite` directamente (ver "API interna" en conventions.md
/// y docs/fase5-gui.md). Solo lectura, sin backup. Devuelve `THUMBNAIL_FAILED`
/// si `thumbnail_status='failed'` (no hay bytes que devolver).
pub fn get_thumbnail(conn: &Connection, image_id: i64) -> AppResult<Value> {
    let row: Option<(Option<Vec<u8>>, String)> = conn
        .query_row(
            "SELECT thumbnail, thumbnail_status FROM images WHERE id = ?1",
            params![image_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((thumbnail, status)) = row else {
        return Err(AppError::ImageNotFound(image_id));
    };
    let Some(bytes) = thumbnail.filter(|_| status == "ok") else {
        return Err(AppError::ThumbnailFailed(image_id));
    };

    Ok(json!({
        "id": image_id,
        "thumbnail_b64": base64::engine::general_purpose::STANDARD.encode(bytes),
    }))
}

/// `get-preview --image-id <id>`: re-decodifica el archivo original al vuelo
/// a `preview_zoom_size` (más grande que `preview_size`, ver config.md) para
/// el zoom del Lightbox de la GUI — el `thumbnail` guardado en DB se queda en
/// `preview_size` para no inflar cada `.photoranker.sqlite`. Solo lectura, sin
/// backup, no toca `thumbnail`/`thumbnail_status`: si la extracción falla acá
/// no se marca la imagen como fallida, ya que `get-thumbnail`/`init` ya
/// resolvieron esa extracción con éxito antes.
pub fn get_preview(conn: &Connection, cfg: &Config, image_id: i64) -> AppResult<Value> {
    let file_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM images WHERE id = ?1",
            params![image_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(file_path) = file_path else {
        return Err(AppError::ImageNotFound(image_id));
    };

    let path = Path::new(&file_path);
    let exif_data = exif::read(path);
    let Ok(img) = thumbnail::extract_normalized(path, &exif_data, cfg.preview_zoom_size) else {
        return Err(AppError::ThumbnailFailed(image_id));
    };

    let mut buf = Vec::new();
    if img
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
        .is_err()
    {
        return Err(AppError::ThumbnailFailed(image_id));
    }

    Ok(json!({
        "id": image_id,
        "preview_b64": base64::engine::general_purpose::STANDARD.encode(buf),
    }))
}

/// `get-quality-metrics --image-id <id>`: expone `image_quality_metrics`
/// para el panel de referencia de calidad de la GUI (ver fase1-ingesta.md
/// sección 2, fase5-gui.md checklist). Solo lectura, sin backup.
pub fn get_quality_metrics(conn: &Connection, image_id: i64) -> AppResult<Value> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM images WHERE id = ?1)",
        params![image_id],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(AppError::ImageNotFound(image_id));
    }

    let metrics = conn
        .query_row(
            "SELECT sharpness, brightness, contrast, overexposed_pct, underexposed_pct, \
             saturation, colorfulness, entropy, average_r, average_g, average_b, orientation \
             FROM image_quality_metrics WHERE image_id = ?1",
            params![image_id],
            |r| {
                Ok(json!({
                    "sharpness": r.get::<_, f64>(0)?,
                    "brightness": r.get::<_, f64>(1)?,
                    "contrast": r.get::<_, f64>(2)?,
                    "overexposed_pct": r.get::<_, f64>(3)?,
                    "underexposed_pct": r.get::<_, f64>(4)?,
                    "saturation": r.get::<_, f64>(5)?,
                    "colorfulness": r.get::<_, f64>(6)?,
                    "entropy": r.get::<_, f64>(7)?,
                    "average_r": r.get::<_, u8>(8)?,
                    "average_g": r.get::<_, u8>(9)?,
                    "average_b": r.get::<_, u8>(10)?,
                    "orientation": r.get::<_, String>(11)?,
                }))
            },
        )
        .optional()?
        .unwrap_or(Value::Null);

    Ok(json!({
        "id": image_id,
        "metrics": metrics,
    }))
}
