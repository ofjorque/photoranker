//! `photoranker init --path <carpeta>` — ver docs/fase1-ingesta.md, secciones 1-3.

use crate::config::Config;
use crate::error::AppResult;
use crate::{db, exif, phash, quality, thumbnail};
use rayon::prelude::*;
use rusqlite::{Connection, params};
use serde_json::json;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "tif", "tiff", "heic", "heif", "cr2", "cr3", "nef", "arw", "rw2", "orf",
    "dng", "pef", "raf",
];

struct ProcessedFile {
    file_path: String,
    thumbnail_bytes: Option<Vec<u8>>,
    thumbnail_status: &'static str,
    hash: Option<String>,
    exif_json: String,
    iso: Option<u32>,
    aperture: Option<f64>,
    focal_length: Option<f64>,
    quality: Option<quality::QualityMetrics>,
}

fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn scan_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file() && is_supported(entry.path()))
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn process_file(path: &Path, preview_size: u32) -> ProcessedFile {
    let file_path = path.to_string_lossy().to_string();
    let exif_data = exif::read(path);
    let exif_json = serde_json::to_string(&exif_data).unwrap_or_else(|_| "{}".to_string());

    match thumbnail::extract_normalized(path, &exif_data, preview_size) {
        Ok(img) => {
            let hash = phash::compute(&img);
            let metrics = quality::compute(&img);
            let mut buf = Vec::new();
            let thumbnail_bytes = img
                .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
                .ok()
                .map(|_| buf);

            ProcessedFile {
                file_path,
                thumbnail_bytes,
                thumbnail_status: "ok",
                hash: Some(hash),
                exif_json,
                iso: exif_data.iso,
                aperture: exif_data.aperture,
                focal_length: exif_data.focal_length,
                quality: Some(metrics),
            }
        }
        Err(_) => {
            tracing::warn!(file = %file_path, "falló la extracción de miniatura");
            ProcessedFile {
                file_path,
                thumbnail_bytes: None,
                thumbnail_status: "failed",
                hash: None,
                exif_json,
                iso: exif_data.iso,
                aperture: exif_data.aperture,
                focal_length: exif_data.focal_length,
                quality: None,
            }
        }
    }
}

fn existing_file_paths(conn: &Connection) -> AppResult<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT file_path FROM images")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut set = std::collections::HashSet::new();
    for row in rows {
        set.insert(row?);
    }
    Ok(set)
}

fn ensure_project_meta(tx: &rusqlite::Transaction, config: &Config) -> AppResult<()> {
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM project_meta", [], |r| r.get(0))?;
    if count == 0 {
        let project_id = uuid::Uuid::new_v4().to_string();
        let snapshot = serde_json::to_string(config).unwrap_or_else(|_| "{}".to_string());
        tx.execute(
            "INSERT INTO project_meta (project_id, config_snapshot) VALUES (?1, ?2)",
            params![project_id, snapshot],
        )?;
    }
    Ok(())
}

pub fn run(scan_path: &Path, config: &Config) -> AppResult<serde_json::Value> {
    let db_path = scan_path.join(db::LOCAL_DB_FILENAME);
    let mut conn = db::open_local(&db_path)?;

    let existing = existing_file_paths(&conn)?;
    let all_files = scan_files(scan_path);
    let new_files: Vec<PathBuf> = all_files
        .into_iter()
        .filter(|p| !existing.contains(&p.to_string_lossy().to_string()))
        .collect();

    let processed: Vec<ProcessedFile> = new_files
        .par_iter()
        .map(|path| process_file(path, config.preview_size))
        .collect();

    let mut ok_count = 0u32;
    let mut failed_count = 0u32;

    let tx = conn.transaction()?;
    ensure_project_meta(&tx, config)?;
    for file in &processed {
        tx.execute(
            "INSERT OR IGNORE INTO images (file_path, hash, thumbnail, thumbnail_status, exif_json, iso, aperture, focal_length)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                file.file_path,
                file.hash,
                file.thumbnail_bytes,
                file.thumbnail_status,
                file.exif_json,
                file.iso,
                file.aperture,
                file.focal_length,
            ],
        )?;

        if file.thumbnail_status == "ok" {
            ok_count += 1;
        } else {
            failed_count += 1;
        }

        if let Some(metrics) = &file.quality {
            let image_id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO image_quality_metrics (
                    image_id, sharpness, brightness, contrast, overexposed_pct, underexposed_pct,
                    saturation, colorfulness, entropy, average_r, average_g, average_b, orientation
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
        }
    }
    tx.commit()?;

    Ok(json!({
        "scanned": processed.len(),
        "inserted_ok": ok_count,
        "inserted_failed": failed_count,
        "skipped_existing": existing.len(),
    }))
}
