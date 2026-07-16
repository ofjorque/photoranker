//! `burst-exclude` / `burst-undo` / `list-bursts-resolved` — ver
//! docs/fase1-ingesta.md, "Excluir/deshacer bursts" (agregado por feedback de
//! uso real: "esta imagen no es parte de una ráfaga").

use image::{Rgb, RgbImage};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Misma fixture que fase5_gui_commands.rs: la estructura de cuadrantes
/// grandes sobrevive al downsample del pHash y arma una ráfaga determinista.
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
        // Sufijo único además de `process::id()`: todos los tests de este
        // archivo corren en el mismo proceso (cargo test los ejecuta en
        // threads del mismo binario), así que dos tests con el mismo `name`
        // colisionarían en la misma carpeta si solo se usara el pid.
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "photoranker_test_{name}_{}_{unique}",
            std::process::id()
        ));
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

/// Arma una carpeta con un burst de 3 imágenes (mismo cuadrante brillante,
/// con ruido leve para diferenciar el hash) + una imagen suelta que no debe
/// agruparse. Devuelve (tmp, db_path, burst_id, image_ids del burst).
fn setup_burst_of_three() -> (TempDir, PathBuf, i64, Vec<i64>) {
    let tmp = TempDir::new("burst_exclude_undo");
    let folder = &tmp.0;
    write_quadrant_jpeg(&folder.join("burst_a.jpg"), 0, 1);
    write_quadrant_jpeg(&folder.join("burst_b.jpg"), 0, 2);
    write_quadrant_jpeg(&folder.join("burst_c.jpg"), 0, 3);
    write_quadrant_jpeg(&folder.join("solo.jpg"), 3, 0);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();

    run_cli(&["init", "--path", &path_arg]);
    run_cli(&["burst-detect", "--threshold", "0.15", "--db", &db_arg]);

    let listed = run_cli(&["list-bursts", "--db", &db_arg]);
    let bursts = listed["data"].as_array().expect("data debe ser array");
    assert_eq!(
        bursts.len(),
        1,
        "debe formarse exactamente 1 burst de 3: {bursts:?}"
    );
    let burst_id = bursts[0]["id"].as_i64().unwrap();
    let image_ids: Vec<i64> = bursts[0]["images"]
        .as_array()
        .unwrap()
        .iter()
        .map(|img| img["id"].as_i64().unwrap())
        .collect();
    assert_eq!(image_ids.len(), 3);

    (tmp, db_path, burst_id, image_ids)
}

#[test]
fn burst_exclude_removes_image_and_keeps_burst_pending() {
    let (_tmp, db_path, burst_id, image_ids) = setup_burst_of_three();
    let db_arg = db_path.to_string_lossy().to_string();

    let excluded_id = image_ids[0].to_string();
    let result = run_cli(&[
        "burst-exclude",
        "--burst-id",
        &burst_id.to_string(),
        "--image-id",
        &excluded_id,
        "--db",
        &db_arg,
    ]);
    assert_eq!(result["status"], "ok", "burst-exclude falló: {result}");
    assert_eq!(result["data"]["burst_dissolved"], false);

    let listed = run_cli(&["list-bursts", "--db", &db_arg]);
    let bursts = listed["data"].as_array().unwrap();
    assert_eq!(bursts.len(), 1, "el burst sigue pendiente con 2 miembros");
    let remaining_images = bursts[0]["images"].as_array().unwrap();
    assert_eq!(remaining_images.len(), 2);
    assert!(
        !remaining_images
            .iter()
            .any(|img| img["id"].as_i64().unwrap().to_string() == excluded_id),
        "la imagen excluida no debe seguir en el burst"
    );
}

#[test]
fn burst_exclude_dissolves_burst_when_only_one_member_remains() {
    let (_tmp, db_path, burst_id, image_ids) = setup_burst_of_three();
    let db_arg = db_path.to_string_lossy().to_string();

    let result = run_cli(&[
        "burst-exclude",
        "--burst-id",
        &burst_id.to_string(),
        "--image-id",
        &image_ids[0].to_string(),
        "--image-id",
        &image_ids[1].to_string(),
        "--db",
        &db_arg,
    ]);
    assert_eq!(result["status"], "ok", "burst-exclude falló: {result}");
    assert_eq!(
        result["data"]["burst_dissolved"], true,
        "con 1 solo miembro restante el burst debe disolverse: {result}"
    );

    let listed = run_cli(&["list-bursts", "--db", &db_arg]);
    assert_eq!(
        listed["data"].as_array().unwrap().len(),
        0,
        "el burst disuelto no debe seguir apareciendo como pendiente"
    );
}

#[test]
fn burst_exclude_rejects_already_resolved_burst() {
    let (_tmp, db_path, burst_id, image_ids) = setup_burst_of_three();
    let db_arg = db_path.to_string_lossy().to_string();

    run_cli(&[
        "burst-tournament",
        "--burst-id",
        &burst_id.to_string(),
        "--db",
        &db_arg,
        "--ranking",
        &format!("{}:1", image_ids[0]),
        &format!("{}:2", image_ids[1]),
        &format!("{}:2", image_ids[2]),
    ]);

    let result = run_cli(&[
        "burst-exclude",
        "--burst-id",
        &burst_id.to_string(),
        "--image-id",
        &image_ids[1].to_string(),
        "--db",
        &db_arg,
    ]);
    assert_eq!(result["status"], "error");
    assert_eq!(result["code"], "INVALID_RANKING");
}

#[test]
fn burst_undo_full_restores_pending_and_rejected_state() {
    let (_tmp, db_path, burst_id, image_ids) = setup_burst_of_three();
    let db_arg = db_path.to_string_lossy().to_string();
    let winner = image_ids[0];
    let loser_a = image_ids[1];
    let loser_b = image_ids[2];

    let resolve = run_cli(&[
        "burst-tournament",
        "--burst-id",
        &burst_id.to_string(),
        "--db",
        &db_arg,
        "--ranking",
        &format!("{winner}:1"),
        &format!("{loser_a}:2"),
        &format!("{loser_b}:2"),
    ]);
    assert_eq!(resolve["status"], "ok", "burst-tournament falló: {resolve}");
    assert_eq!(resolve["data"]["representative_image_id"], winner);

    let resolved_list = run_cli(&["list-bursts-resolved", "--db", &db_arg]);
    assert_eq!(resolved_list["data"].as_array().unwrap().len(), 1);

    let undo = run_cli(&[
        "burst-undo",
        "--burst-id",
        &burst_id.to_string(),
        "--db",
        &db_arg,
    ]);
    assert_eq!(undo["status"], "ok", "burst-undo falló: {undo}");
    assert_eq!(undo["data"]["burst_status"], "pending");

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    for id in [winner, loser_a, loser_b] {
        let rejected: i64 = conn
            .query_row("SELECT rejected FROM images WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            rejected, 0,
            "imagen {id} debe volver a rejected=0 tras deshacer"
        );
    }

    let listed = run_cli(&["list-bursts", "--db", &db_arg]);
    assert_eq!(
        listed["data"].as_array().unwrap().len(),
        1,
        "el burst deshecho debe volver a aparecer como pendiente"
    );

    let resolved_after_undo = run_cli(&["list-bursts-resolved", "--db", &db_arg]);
    assert_eq!(resolved_after_undo["data"].as_array().unwrap().len(), 0);
}

#[test]
fn burst_undo_single_image_restores_just_that_image() {
    let (_tmp, db_path, burst_id, image_ids) = setup_burst_of_three();
    let db_arg = db_path.to_string_lossy().to_string();
    let winner = image_ids[0];
    let loser_a = image_ids[1];
    let loser_b = image_ids[2];

    run_cli(&[
        "burst-tournament",
        "--burst-id",
        &burst_id.to_string(),
        "--db",
        &db_arg,
        "--ranking",
        &format!("{winner}:1"),
        &format!("{loser_a}:2"),
        &format!("{loser_b}:2"),
    ]);

    let undo = run_cli(&[
        "burst-undo",
        "--burst-id",
        &burst_id.to_string(),
        "--image-id",
        &loser_a.to_string(),
        "--db",
        &db_arg,
    ]);
    assert_eq!(undo["status"], "ok", "burst-undo falló: {undo}");
    assert_eq!(undo["data"]["burst_status"], "completed");

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let rejected_a: i64 = conn
        .query_row(
            "SELECT rejected FROM images WHERE id = ?1",
            [loser_a],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        rejected_a, 0,
        "la imagen deshecha individualmente debe volver a rejected=0"
    );
    let rejected_b: i64 = conn
        .query_row(
            "SELECT rejected FROM images WHERE id = ?1",
            [loser_b],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rejected_b, 1, "el resto del burst debe seguir resuelto");
    let member_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM burst_members WHERE burst_id = ?1",
            [burst_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(member_count, 2, "la imagen deshecha sale de burst_members");
}

#[test]
fn burst_undo_rejects_excluding_the_representative_image() {
    let (_tmp, db_path, burst_id, image_ids) = setup_burst_of_three();
    let db_arg = db_path.to_string_lossy().to_string();
    let winner = image_ids[0];

    run_cli(&[
        "burst-tournament",
        "--burst-id",
        &burst_id.to_string(),
        "--db",
        &db_arg,
        "--ranking",
        &format!("{winner}:1"),
        &format!("{}:2", image_ids[1]),
        &format!("{}:2", image_ids[2]),
    ]);

    let undo = run_cli(&[
        "burst-undo",
        "--burst-id",
        &burst_id.to_string(),
        "--image-id",
        &winner.to_string(),
        "--db",
        &db_arg,
    ]);
    assert_eq!(undo["status"], "error");
    assert_eq!(undo["code"], "INVALID_RANKING");
}
