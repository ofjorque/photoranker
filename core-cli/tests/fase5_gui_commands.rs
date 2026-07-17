//! Prueba de integración para los comandos agregados en Fase 5
//! (`get-thumbnail`, `get-quality-metrics`, `list-bursts`) — ver
//! docs/fase5-gui.md, "Acceso a miniaturas y métricas de calidad desde la
//! GUI".

use image::{Rgb, RgbImage};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Aísla `~/.photoranker/` del proceso real de quien corre los tests (ver
/// mismo helper en fase1_integration.rs y `PHOTORANKER_HOME` en config.rs).
fn test_home() -> &'static Path {
    use std::sync::OnceLock;
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("photoranker_test_home_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    })
}

fn run_cli(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_photoranker"))
        .args(args)
        .env("PHOTORANKER_HOME", test_home())
        .output()
        .expect("no se pudo ejecutar photoranker");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "JSON inválido: {e}\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn write_solid_jpeg(path: &Path, v: u8) {
    let img = RgbImage::from_pixel(64, 64, Rgb([v, v, v]));
    img.save(path).expect("no se pudo guardar el JPEG");
}

/// Ver fase1_integration.rs: la estructura de cuadrantes grandes sobrevive al
/// downsample del pHash y permite formar una ráfaga determinista en el test.
fn write_quadrant_jpeg(path: &Path, bright_quadrant: u8, noise_seed: u32) {
    let mut img = RgbImage::new(64, 64);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let q = u8::from(x >= 32) + 2 * u8::from(y >= 32);
        let v = if q == bright_quadrant { 240 } else { 20 };
        *pixel = Rgb([v, v, v]);
    }
    for (i, pixel) in img.pixels_mut().enumerate() {
        if (i as u32 + noise_seed).is_multiple_of(37) {
            pixel.0[0] = pixel.0[0].saturating_add(10);
        }
    }
    img.save(path)
        .expect("no se pudo guardar el JPEG sintético");
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("photoranker_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn get_thumbnail_and_quality_metrics_happy_path() {
    let tmp = TempDir::new("fase5_happy");
    let folder = &tmp.0;
    write_solid_jpeg(&folder.join("a.jpg"), 128);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();

    let init_result = run_cli(&["init", "--path", &path_arg]);
    assert_eq!(init_result["status"], "ok", "init falló: {init_result}");

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let image_id: i64 = conn
        .query_row("SELECT id FROM images LIMIT 1", [], |r| r.get(0))
        .unwrap();
    drop(conn);
    let id_arg = image_id.to_string();

    let thumb = run_cli(&["get-thumbnail", "--image-id", &id_arg, "--db", &db_arg]);
    assert_eq!(thumb["status"], "ok", "get-thumbnail falló: {thumb}");
    let b64 = thumb["data"]["thumbnail_b64"]
        .as_str()
        .expect("thumbnail_b64 debe ser string");
    assert!(!b64.is_empty());
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .expect("thumbnail_b64 debe ser base64 válido");
    assert_eq!(
        &decoded[0..2],
        &[0xFF, 0xD8],
        "debe ser un JPEG (SOI marker)"
    );

    let metrics = run_cli(&[
        "get-quality-metrics",
        "--image-id",
        &id_arg,
        "--db",
        &db_arg,
    ]);
    assert_eq!(
        metrics["status"], "ok",
        "get-quality-metrics falló: {metrics}"
    );
    assert!(metrics["data"]["metrics"]["sharpness"].is_number());
    assert_eq!(metrics["data"]["metrics"]["orientation"], "square");

    let preview = run_cli(&["get-preview", "--image-id", &id_arg, "--db", &db_arg]);
    assert_eq!(preview["status"], "ok", "get-preview falló: {preview}");
    let preview_b64 = preview["data"]["preview_b64"]
        .as_str()
        .expect("preview_b64 debe ser string");
    assert!(!preview_b64.is_empty());
    let preview_decoded = base64::engine::general_purpose::STANDARD
        .decode(preview_b64)
        .expect("preview_b64 debe ser base64 válido");
    assert_eq!(
        &preview_decoded[0..2],
        &[0xFF, 0xD8],
        "debe ser un JPEG (SOI marker)"
    );
}

#[test]
fn get_preview_reports_image_not_found() {
    let tmp = TempDir::new("fase5_preview_not_found");
    let folder = &tmp.0;
    write_solid_jpeg(&folder.join("a.jpg"), 128);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();

    run_cli(&["init", "--path", &path_arg]);

    let missing = run_cli(&["get-preview", "--image-id", "999999", "--db", &db_arg]);
    assert_eq!(missing["status"], "error");
    assert_eq!(missing["code"], "IMAGE_NOT_FOUND");
}

#[test]
fn list_bursts_returns_pending_bursts_with_members_and_omits_completed() {
    let tmp = TempDir::new("fase5_list_bursts");
    let folder = &tmp.0;
    write_quadrant_jpeg(&folder.join("burst_a.jpg"), 0, 1);
    write_quadrant_jpeg(&folder.join("burst_b.jpg"), 0, 2);
    write_quadrant_jpeg(&folder.join("solo.jpg"), 3, 0);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();

    run_cli(&["init", "--path", &path_arg]);
    run_cli(&["burst-detect", "--threshold", "0.15", "--db", &db_arg]);

    let listed = run_cli(&["list-bursts", "--db", &db_arg]);
    assert_eq!(listed["status"], "ok", "list-bursts falló: {listed}");
    let bursts = listed["data"].as_array().expect("data debe ser array");
    assert_eq!(bursts.len(), 1, "solo hay 1 burst pendiente: {bursts:?}");
    let images = bursts[0]["images"].as_array().unwrap();
    assert_eq!(images.len(), 2);

    // Resolver el burst y confirmar que list-bursts deja de reportarlo.
    let burst_id = bursts[0]["id"].as_i64().unwrap();
    let id_a = images[0]["id"].as_i64().unwrap();
    let id_b = images[1]["id"].as_i64().unwrap();
    run_cli(&[
        "burst-tournament",
        "--burst-id",
        &burst_id.to_string(),
        "--db",
        &db_arg,
        "--ranking",
        &format!("{id_a}:1"),
        &format!("{id_b}:2"),
    ]);

    let listed_after = run_cli(&["list-bursts", "--db", &db_arg]);
    assert_eq!(
        listed_after["data"].as_array().unwrap().len(),
        0,
        "un burst completado no debe seguir apareciendo"
    );
}

#[test]
fn tournament_next_and_status_exclude_failed_thumbnails() {
    // Regresión: tournament-next/tournament-status no filtraban
    // thumbnail_status, así que fotos con miniatura fallida (comunes con RAW
    // no soportados por rawloader, ver fase1-ingesta.md sección 3) terminaban
    // en el torneo con mu/sigma default para siempre — nunca convergen ni se
    // comparan, contaminando "activas"/convergencia. Ver fase3-torneo.md,
    // "Queda excluida de torneos... hasta resolverse manualmente".
    let tmp = TempDir::new("fase5_thumb_status");
    let folder = &tmp.0;
    write_solid_jpeg(&folder.join("a.jpg"), 100);
    write_solid_jpeg(&folder.join("b.jpg"), 150);
    std::fs::write(folder.join("broken.jpg"), b"no es una imagen valida").unwrap();

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();

    let init_result = run_cli(&["init", "--path", &path_arg]);
    assert_eq!(init_result["data"]["inserted_ok"], 2);
    assert_eq!(init_result["data"]["inserted_failed"], 1);

    let status = run_cli(&["tournament-status", "--db", &db_arg]);
    assert_eq!(status["status"], "ok", "tournament-status falló: {status}");
    assert_eq!(
        status["data"]["active_images"], 2,
        "la imagen con miniatura fallida no debe contar como activa"
    );

    let next = run_cli(&["tournament-next", "--db", &db_arg]);
    assert_eq!(next["status"], "ok", "tournament-next falló: {next}");
    let images = next["data"]["images"]
        .as_array()
        .expect("con 2 imágenes ok debe formarse un grupo");
    assert_eq!(
        images.len(),
        2,
        "solo las 2 imágenes ok deben entrar al grupo"
    );
    for img in images {
        let name = img["file_path"].as_str().unwrap();
        assert!(
            !name.contains("broken"),
            "broken.jpg no debe aparecer en un grupo de torneo: {name}"
        );
    }
}

#[test]
fn get_thumbnail_reports_image_not_found() {
    let tmp = TempDir::new("fase5_error");
    let folder = &tmp.0;
    write_solid_jpeg(&folder.join("a.jpg"), 64);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();

    run_cli(&["init", "--path", &path_arg]);

    let missing = run_cli(&["get-thumbnail", "--image-id", "999999", "--db", &db_arg]);
    assert_eq!(missing["status"], "error");
    assert_eq!(missing["code"], "IMAGE_NOT_FOUND");

    let missing_metrics = run_cli(&[
        "get-quality-metrics",
        "--image-id",
        "999999",
        "--db",
        &db_arg,
    ]);
    assert_eq!(missing_metrics["status"], "error");
    assert_eq!(missing_metrics["code"], "IMAGE_NOT_FOUND");
}
