//! Pruebas de integración para `tournament-undo`, `tournament-reset` y
//! `reset-global-index` — agregados por feedback de uso real, ver
//! docs/fase3-torneo.md, "Deshacer el último resultado enviado" / "Reiniciar
//! el torneo completo de la carpeta".

use image::{Rgb, RgbImage};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Aísla `~/.photoranker/` del proceso real de quien corre los tests — crítico
/// acá en particular, ya que `reset-global-index` vacía ese archivo por
/// completo (ver `PHOTORANKER_HOME` en config.rs; sin este aislamiento este
/// test vaciaría el índice global real de la máquina que corre `cargo test`).
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

fn setup_project(name: &str, n: usize) -> (TempDir, PathBuf, String, String) {
    let tmp = TempDir::new(name);
    let folder = tmp.0.clone();
    for i in 0..n {
        write_solid_jpeg(&folder.join(format!("img{i}.jpg")), (50 + i * 10) as u8);
    }
    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();
    run_cli(&["init", "--path", &path_arg]);
    (tmp, db_path, db_arg, path_arg)
}

#[test]
fn tournament_undo_reverts_last_group_and_only_that_group() {
    let (_tmp, db_path, db_arg, _path_arg) = setup_project("undo", 5);

    // Nada que deshacer todavía.
    let no_group = run_cli(&["tournament-undo", "--db", &db_arg]);
    assert_eq!(no_group["status"], "error");
    assert_eq!(no_group["code"], "NOTHING_TO_UNDO");

    let next1 = run_cli(&["tournament-next", "--db", &db_arg]);
    let images1 = next1["data"]["images"].as_array().unwrap();
    let group1 = next1["data"]["group_id"].as_str().unwrap();
    let ranking1: Vec<String> = images1
        .iter()
        .enumerate()
        .map(|(i, img)| format!("{}:{}", img["id"].as_i64().unwrap(), i + 1))
        .collect();
    let mut args1 = vec![
        "tournament-result".to_string(),
        "--group-id".to_string(),
        group1.to_string(),
        "--db".to_string(),
        db_arg.clone(),
        "--ranking".to_string(),
    ];
    args1.extend(ranking1);
    let args1_ref: Vec<&str> = args1.iter().map(String::as_str).collect();
    run_cli(&args1_ref);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let mu_after_group1: f64 = conn
        .query_row(
            "SELECT mu FROM images WHERE id = ?1",
            [images1[0]["id"].as_i64().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    drop(conn);
    assert_ne!(mu_after_group1, 25.0, "el ganador debe haber subido de mu");

    let undo = run_cli(&["tournament-undo", "--db", &db_arg]);
    assert_eq!(undo["status"], "ok", "tournament-undo falló: {undo}");
    assert_eq!(undo["data"]["group_id"], group1);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let mu_after_undo: f64 = conn
        .query_row(
            "SELECT mu FROM images WHERE id = ?1",
            [images1[0]["id"].as_i64().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    drop(conn);
    assert_eq!(
        mu_after_undo, 25.0,
        "tras deshacer, mu debe volver al default"
    );

    // Deshacer el mismo grupo dos veces debe fallar (ya no queda nada pendiente de deshacer).
    let undo_again = run_cli(&["tournament-undo", "--db", &db_arg]);
    assert_eq!(undo_again["status"], "error");
    assert_eq!(undo_again["code"], "NOTHING_TO_UNDO");
}

#[test]
fn tournament_reset_restores_defaults_without_touching_rejected() {
    let (_tmp, db_path, db_arg, _path_arg) = setup_project("reset", 4);

    let next = run_cli(&["tournament-next", "--db", &db_arg]);
    let images = next["data"]["images"].as_array().unwrap();
    let group_id = next["data"]["group_id"].as_str().unwrap();
    let ranking: Vec<String> = images
        .iter()
        .enumerate()
        .map(|(i, img)| format!("{}:{}", img["id"].as_i64().unwrap(), i + 1))
        .collect();
    let mut args = vec![
        "tournament-result".to_string(),
        "--group-id".to_string(),
        group_id.to_string(),
        "--db".to_string(),
        db_arg.clone(),
        "--ranking".to_string(),
    ];
    args.extend(ranking);
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    run_cli(&args_ref);

    // Marcar una imagen como rejected manualmente (simula una decisión de burst-tournament).
    let rejected_id = images[0]["id"].as_i64().unwrap();
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "UPDATE images SET rejected = 1 WHERE id = ?1",
            [rejected_id],
        )
        .unwrap();
    }

    let reset = run_cli(&["tournament-reset", "--db", &db_arg]);
    assert_eq!(reset["status"], "ok", "tournament-reset falló: {reset}");
    assert_eq!(reset["data"]["images_reset"], 4);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let (mu, sigma, rejected): (f64, f64, i64) = conn
        .query_row(
            "SELECT mu, sigma, rejected FROM images WHERE id = ?1",
            [rejected_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(mu, 25.0);
    assert_eq!(sigma, 8.33);
    assert_eq!(rejected, 1, "tournament-reset no debe tocar rejected");
}

#[test]
fn reset_global_index_empties_shared_index() {
    let reset = run_cli(&["reset-global-index"]);
    assert_eq!(reset["status"], "ok", "reset-global-index falló: {reset}");
    assert!(reset["data"]["rows_deleted"].is_number());
}
