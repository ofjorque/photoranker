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

const RAW_EXTENSIONS: &[&str] = &[
    "cr2", "cr3", "nef", "arw", "rw2", "orf", "dng", "pef", "raf",
];
const JPEG_EXTENSIONS: &[&str] = &["jpg", "jpeg"];
const OTHER_SUPPORTED_EXTENSIONS: &[&str] = &["png", "tif", "tiff", "heic", "heif"];

struct ProcessedFile {
    file_path: String,
    paired_path: Option<String>,
    thumbnail_bytes: Option<Vec<u8>>,
    thumbnail_status: &'static str,
    hash: Option<String>,
    exif_json: String,
    iso: Option<u32>,
    aperture: Option<f64>,
    focal_length: Option<f64>,
    quality: Option<quality::QualityMetrics>,
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn is_supported(path: &Path) -> bool {
    let ext = extension_of(path);
    RAW_EXTENSIONS.contains(&ext.as_str())
        || JPEG_EXTENSIONS.contains(&ext.as_str())
        || OTHER_SUPPORTED_EXTENSIONS.contains(&ext.as_str())
}

/// `true` si `entry` es una subcarpeta (no la raíz del escaneo) cuyo nombre
/// coincide (case-insensitive) con alguno de `exclude_dirs` — ej. "Selected"
/// o "exported", carpetas que el usuario crea a mano para juntar fotos ya
/// elegidas/exportadas y que no deben volver a escanearse como fuente (ver
/// docs/config.md, `exclude_dirs`). `depth() > 0` excluye la raíz misma: si
/// el usuario apunta `init --path` directo a una carpeta llamada "Selected",
/// el escaneo debe funcionar igual en vez de devolver cero archivos.
fn is_excluded_dir(entry: &walkdir::DirEntry, exclude_dirs: &[String]) -> bool {
    entry.depth() > 0
        && entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .map(|name| exclude_dirs.iter().any(|d| d.eq_ignore_ascii_case(name)))
            .unwrap_or(false)
}

fn scan_files(root: &Path, exclude_dirs: &[String]) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !is_excluded_dir(entry, exclude_dirs))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file() && is_supported(entry.path()))
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

/// Una "unidad" a procesar: normalmente un archivo suelto, o un par RAW+JPEG
/// del mismo disparo fusionado en un solo registro (ver
/// fase1-ingesta.md, "RAW + JPEG del mismo disparo cuentan como 1 sola foto").
struct ScanUnit {
    /// Va a `images.file_path` — el RAW si hay par, el archivo mismo si no.
    primary: PathBuf,
    /// De dónde se extrae la miniatura/pHash/métricas — el JPEG si hay par
    /// (más confiable que decodificar el RAW), el archivo mismo si no.
    thumbnail_source: PathBuf,
    /// Va a `images.paired_path` — `Some(jpeg)` solo si hubo emparejamiento.
    paired_path: Option<PathBuf>,
}

/// Agrupa `files` por carpeta + nombre base (sin extensión, case-insensitive)
/// y fusiona en un único `ScanUnit` los grupos de exactamente 2 archivos
/// donde uno es RAW y el otro JPEG. Cualquier otro caso (archivo suelto, o un
/// grupo ambiguo de 3+ archivos con el mismo nombre base) se procesa como
/// unidades independientes — fusionar solo el caso inequívoco RAW+JPEG evita
/// tener que adivinar en casos raros (ej. dos RAW distintos con igual
/// nombre en carpetas... no debería pasar dado que ya se agrupa por carpeta,
/// pero un IMG_1234.CR2 + IMG_1234.CR3 sí caería acá y queda sin fusionar).
fn group_into_units(files: Vec<PathBuf>) -> Vec<ScanUnit> {
    use std::collections::HashMap;
    let mut groups: HashMap<(PathBuf, String), Vec<PathBuf>> = HashMap::new();
    for f in files {
        let parent = f.parent().unwrap_or(Path::new("")).to_path_buf();
        let stem = f
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        groups.entry((parent, stem)).or_default().push(f);
    }

    let mut units = Vec::new();
    for (_, group) in groups {
        if group.len() == 2 {
            let (a, b) = (&group[0], &group[1]);
            let (ext_a, ext_b) = (extension_of(a), extension_of(b));
            let pair = if RAW_EXTENSIONS.contains(&ext_a.as_str())
                && JPEG_EXTENSIONS.contains(&ext_b.as_str())
            {
                Some((a.clone(), b.clone()))
            } else if RAW_EXTENSIONS.contains(&ext_b.as_str())
                && JPEG_EXTENSIONS.contains(&ext_a.as_str())
            {
                Some((b.clone(), a.clone()))
            } else {
                None
            };
            if let Some((raw_path, jpeg_path)) = pair {
                units.push(ScanUnit {
                    primary: raw_path,
                    thumbnail_source: jpeg_path.clone(),
                    paired_path: Some(jpeg_path),
                });
                continue;
            }
        }
        for f in group {
            units.push(ScanUnit {
                thumbnail_source: f.clone(),
                primary: f,
                paired_path: None,
            });
        }
    }
    units
}

fn process_unit(unit: &ScanUnit, preview_size: u32) -> ProcessedFile {
    let file_path = unit.primary.to_string_lossy().to_string();
    let paired_path = unit
        .paired_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());
    // Log de progreso (stderr, ver conventions.md "Logging") — la GUI lo
    // muestra en vivo durante init vía streaming de stdout/stderr (ver
    // docs/fase5-gui.md, agregado por feedback de uso real: "que salga el
    // nombre de la carpeta/archivo que se está escaneando"). rayon procesa
    // en paralelo, así que estas líneas se intercalan entre hilos — es
    // esperable, la GUI solo muestra la más reciente.
    tracing::info!(file = %file_path, "procesando imagen");

    // El EXIF (iso/aperture/focal_length) se lee del RAW/archivo primario; si
    // no trae nada legible y hay un JPEG emparejado, se completa desde ahí
    // (misma escena, el JPEG lo genera la cámara al momento de la toma —
    // ver exif::read, que ya degrada a ExifData::default() sin fallar).
    let mut exif_data = exif::read(&unit.primary);
    if unit.paired_path.is_some()
        && exif_data.iso.is_none()
        && exif_data.aperture.is_none()
        && exif_data.focal_length.is_none()
    {
        let companion_exif = exif::read(&unit.thumbnail_source);
        if companion_exif.iso.is_some()
            || companion_exif.aperture.is_some()
            || companion_exif.focal_length.is_some()
        {
            exif_data = companion_exif;
        }
    }
    let exif_json = serde_json::to_string(&exif_data).unwrap_or_else(|_| "{}".to_string());

    match thumbnail::extract_normalized(&unit.thumbnail_source, &exif_data, preview_size) {
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
                paired_path,
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
                paired_path,
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

/// Todos los `file_path` ya conocidos por la BD, sea como `images.file_path`
/// o como `images.paired_path` — necesario para que la fusión RAW+JPEG no
/// rompa la idempotencia de `init`: si un JPEG ya quedó fusionado con su RAW
/// en una corrida anterior, no debe volver a insertarse solo en la próxima
/// corrida solo porque su propio `file_path` nunca apareció en `images.file_path`.
fn existing_file_paths(conn: &Connection) -> AppResult<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT file_path FROM images \
         UNION \
         SELECT paired_path FROM images WHERE paired_path IS NOT NULL",
    )?;
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
    tracing::info!(path = %scan_path.display(), "escaneando carpeta");
    let db_path = scan_path.join(db::LOCAL_DB_FILENAME);
    let mut conn = db::open_local(&db_path)?;

    let existing = existing_file_paths(&conn)?;
    let all_files = scan_files(scan_path, &config.exclude_dirs);
    let new_files: Vec<PathBuf> = all_files
        .into_iter()
        .filter(|p| !existing.contains(&p.to_string_lossy().to_string()))
        .collect();
    let scanned_count = new_files.len();
    tracing::info!(nuevos = scanned_count, "archivos nuevos encontrados");

    let units = group_into_units(new_files);
    let paired_count = units.iter().filter(|u| u.paired_path.is_some()).count();

    let processed: Vec<ProcessedFile> = units
        .par_iter()
        .map(|unit| process_unit(unit, config.preview_size))
        .collect();

    let mut ok_count = 0u32;
    let mut failed_count = 0u32;

    let tx = conn.transaction()?;
    ensure_project_meta(&tx, config)?;
    for file in &processed {
        tx.execute(
            "INSERT OR IGNORE INTO images (file_path, paired_path, hash, thumbnail, thumbnail_status, exif_json, iso, aperture, focal_length)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                file.file_path,
                file.paired_path,
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
        "scanned": scanned_count,
        "inserted_ok": ok_count,
        "inserted_failed": failed_count,
        "skipped_existing": existing.len(),
        "paired_raw_jpeg": paired_count,
    }))
}
