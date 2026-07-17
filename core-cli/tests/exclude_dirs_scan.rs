//! Prueba de integración para `exclude_dirs` — ver docs/config.md y
//! docs/fase1-ingesta.md. `init` no debe descender a subcarpetas cuyo
//! nombre coincida (case-insensitive, cualquier profundidad) con alguna de
//! `exclude_dirs` (default: "Selected", "exported"), típicamente carpetas
//! que el usuario arma a mano con fotos ya elegidas/exportadas y que no
//! deben volver a contarse como fuente.

use image::{Rgb, RgbImage};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Aísla `~/.photoranker/` de quien corre los tests (ver mismo helper en
/// fase1_integration.rs / raw_jpeg_pairing.rs y `PHOTORANKER_HOME` en
/// config.rs). Cada test de este archivo usa su propio subdirectorio (no un
/// `OnceLock` compartido) porque el segundo test necesita escribir un
/// `config.toml` con `exclude_dirs` distinto al default, y no debe pisar el
/// `config.toml` que ya haya creado otro test del mismo binario.
fn fresh_home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "photoranker_test_home_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_cli(home: &Path, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_photoranker"))
        .args(args)
        .env("PHOTORANKER_HOME", home)
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

#[test]
fn init_skips_default_excluded_dirs_case_insensitive_any_depth() {
    let home = fresh_home("exclude_default");
    let tmp = TempDir::new("exclude_default_lib");
    let folder = &tmp.0;

    write_solid_jpeg(&folder.join("top_level.jpg"), 10);

    let selected = folder.join("Selected");
    std::fs::create_dir_all(&selected).unwrap();
    write_solid_jpeg(&selected.join("chosen.jpg"), 20);

    let exported = folder.join("exported");
    std::fs::create_dir_all(&exported).unwrap();
    write_solid_jpeg(&exported.join("done.jpg"), 30);

    // Variante de mayúsculas a mayor profundidad, para probar
    // case-insensitive + cualquier profundidad, no solo el nivel superior.
    let nested_selected = folder.join("2024").join("SELECTED");
    std::fs::create_dir_all(&nested_selected).unwrap();
    write_solid_jpeg(&nested_selected.join("deep.jpg"), 40);

    let path_arg = folder.to_string_lossy().to_string();
    let init_result = run_cli(&home, &["init", "--path", &path_arg]);
    assert_eq!(init_result["status"], "ok", "init falló: {init_result}");
    assert_eq!(
        init_result["data"]["scanned"], 1,
        "solo top_level.jpg debe contarse — Selected/exported/SELECTED se podan enteras: {init_result}"
    );
    assert_eq!(init_result["data"]["inserted_ok"], 1);

    let db_path = folder.join(".photoranker.sqlite");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let file_paths: Vec<String> = conn
        .prepare("SELECT file_path FROM images")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert_eq!(file_paths.len(), 1);
    assert!(file_paths[0].to_lowercase().contains("top_level"));
}

#[test]
fn init_respects_custom_exclude_dirs_from_config_toml() {
    let home = fresh_home("exclude_custom");
    // `exclude_dirs` custom: solo "Descartadas" — "Selected" (el default) NO
    // debe excluirse acá, prueba que el valor viene de config.toml y no del
    // default hardcodeado.
    std::fs::write(
        home.join("config.toml"),
        "exclude_dirs = [\"Descartadas\"]\n",
    )
    .unwrap();

    let tmp = TempDir::new("exclude_custom_lib");
    let folder = &tmp.0;

    let selected = folder.join("Selected");
    std::fs::create_dir_all(&selected).unwrap();
    write_solid_jpeg(&selected.join("chosen.jpg"), 20);

    let discarded = folder.join("Descartadas");
    std::fs::create_dir_all(&discarded).unwrap();
    write_solid_jpeg(&discarded.join("no.jpg"), 30);

    let path_arg = folder.to_string_lossy().to_string();
    let init_result = run_cli(&home, &["init", "--path", &path_arg]);
    assert_eq!(init_result["status"], "ok", "init falló: {init_result}");
    assert_eq!(
        init_result["data"]["scanned"], 1,
        "con exclude_dirs=[\"Descartadas\"] del config.toml, Selected/ SÍ debe escanearse: {init_result}"
    );

    let db_path = folder.join(".photoranker.sqlite");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let file_paths: Vec<String> = conn
        .prepare("SELECT file_path FROM images")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert_eq!(file_paths.len(), 1);
    assert!(file_paths[0].to_lowercase().contains("chosen"));
}
