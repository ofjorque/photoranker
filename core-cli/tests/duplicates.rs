//! Pruebas de integración para `list-duplicates` (ver
//! docs/fase8-mejoras-avanzadas.md, "Detección de duplicados entre
//! carpetas/viajes"). Siembra `global_ratings` directamente por SQL (mismo
//! patrón que cluster_and_variable_gui_commands.rs con `cached_cluster_fits`)
//! para no depender de una sesión completa de torneo solo para tener `hash`
//! sincronizado en dos "carpetas" distintas.

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
fn list_duplicates_flags_matching_hash_from_another_project_and_ignores_own() {
    let tmp = TempDir::new("duplicates");
    let folder = &tmp.0;
    write_solid_jpeg(&folder.join("a.jpg"), 10);
    write_solid_jpeg(&folder.join("b.jpg"), 200);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();
    run_cli(&["init", "--path", &path_arg]);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let project_id: String = conn
        .query_row("SELECT project_id FROM project_meta LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    let (image_a, hash_a): (i64, String) = conn
        .query_row(
            "SELECT id, hash FROM images WHERE file_path LIKE '%a.jpg' LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    drop(conn);

    // `init` no toca el índice global — hace falta correr algún comando que
    // sí lo abra (crea la tabla `global_ratings` con su columna `hash`) antes
    // de poder sembrar filas directamente por SQL.
    run_cli(&["reset-global-index"]);

    // Siembra global_ratings directamente: mi propia imagen (necesaria para
    // el lado "mine" de la comparación) + un "otro proyecto" con el MISMO
    // hash (duplicado exacto esperado) + un tercero con hash bien distinto
    // (no debería aparecer).
    let global_db_path = test_home().join("global_index.sqlite");
    let gconn = rusqlite::Connection::open(&global_db_path).unwrap();
    gconn
        .execute(
            "INSERT INTO global_ratings (project_id, source_db_path, image_id, file_path, mu, rejected, hash) \
             VALUES (?1, ?2, ?3, ?4, 25.0, 0, ?5)",
            rusqlite::params![project_id, db_path.display().to_string(), image_a, "a.jpg", hash_a],
        )
        .unwrap();
    gconn
        .execute(
            "INSERT INTO global_ratings (project_id, source_db_path, image_id, file_path, mu, rejected, hash) \
             VALUES ('other-project-dup', '/otro/viaje/.photoranker.sqlite', 1, 'otro/viaje/foto_repetida.jpg', 25.0, 0, ?1)",
            rusqlite::params![hash_a],
        )
        .unwrap();
    // XOR con 0xAA (10101010) en cada byte: flipea exactamente la mitad de
    // los bits de CUALQUIER hash de entrada, sin importar su valor real —
    // a diferencia de "todo unos", que podría terminar casualmente cerca
    // del hash original si este ya tiene mayoría de bits en 1. Garantiza
    // distancia normalizada exactamente 0.5, bien por encima del
    // duplicate_threshold default (0.10).
    let far_hash: String = (0..hash_a.len())
        .step_by(2)
        .map(|i| {
            let byte = u8::from_str_radix(&hash_a[i..i + 2], 16).unwrap();
            format!("{:02x}", byte ^ 0xAA)
        })
        .collect();
    gconn
        .execute(
            "INSERT INTO global_ratings (project_id, source_db_path, image_id, file_path, mu, rejected, hash) \
             VALUES ('other-project-different', '/otro/viaje2/.photoranker.sqlite', 1, 'otro/viaje2/otra_foto.jpg', 25.0, 0, ?1)",
            rusqlite::params![far_hash],
        )
        .unwrap();
    drop(gconn);

    let result = run_cli(&["list-duplicates", "--db", &db_arg]);
    assert_eq!(result["status"], "ok", "list-duplicates falló: {result}");
    let matches = result["data"].as_array().unwrap();
    assert_eq!(
        matches.len(),
        1,
        "solo el duplicado de mismo hash debe aparecer, no el de hash lejano ni el propio: {matches:?}"
    );
    assert_eq!(matches[0]["local_image_id"], image_a);
    assert_eq!(matches[0]["other_project_id"], "other-project-dup");
    assert_eq!(matches[0]["exact"], true);
    assert_eq!(matches[0]["distance"], 0.0);
}
