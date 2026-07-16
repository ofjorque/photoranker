//! Prueba de integración de extremo a extremo para Fase 1 (ver
//! docs/conventions.md, "Prueba de aceptación de referencia"): genera una
//! pequeña biblioteca sintética de JPEGs (sin RAW real disponible en este
//! entorno) y ejerce init -> burst-detect -> burst-tournament ->
//! variable-create/list/set -> prune contra el binario `photoranker`.

use image::{Rgb, RgbImage};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Aísla `~/.photoranker/` (config.toml + global_index.sqlite, COMPARTIDO
/// entre carpetas reales del usuario) del proceso real de quien corre los
/// tests — sin esto, `cargo test` termina leyendo/escribiendo/vaciando el
/// índice global real de la máquina (ver `PHOTORANKER_HOME` en config.rs).
/// Un solo directorio por proceso de test (no por test individual): los
/// tests dentro de un mismo binario corren en hilos del mismo proceso, y
/// compartir esta carpeta es seguro porque cada test genera su propio
/// `project_id` (UUID) vía `init`, así que nunca colisionan en `global_ratings`.
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

/// Imagen de cuadrantes grandes: la estructura macro sobrevive al downsample
/// 64x64 -> 16x16 que hace el preprocesamiento DCT del pHash (ver
/// src/phash.rs, `similar_quadrant_images_are_close_and_distinct_ones_are_far`
/// — un patrón de alta frecuencia por-píxel, como ruido o un tablero de
/// ajedrez, se promedia y desaparece en ese downsample y no sirve para
/// distinguir imágenes).
fn write_quadrant_jpeg(path: &Path, bright_quadrant: u8, noise_seed: u32) {
    let mut img = RgbImage::new(64, 64);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let q = u8::from(x >= 32) + 2 * u8::from(y >= 32);
        let v = if q == bright_quadrant { 240 } else { 20 };
        *pixel = Rgb([v, v, v]);
    }
    // Ruido determinista leve para variar el pHash entre "casi idénticas" sin
    // cambiar la distancia de forma relevante (simula una ráfaga real).
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
fn full_fase1_flow() {
    let tmp = TempDir::new("fase1");
    let folder = &tmp.0;

    // 3 fotos "de ráfaga" (mismo encuadre, ruido leve) + 1 claramente distinta.
    write_quadrant_jpeg(&folder.join("burst_a.jpg"), 0, 1);
    write_quadrant_jpeg(&folder.join("burst_b.jpg"), 0, 2);
    write_quadrant_jpeg(&folder.join("burst_c.jpg"), 0, 3);
    write_quadrant_jpeg(&folder.join("different.jpg"), 3, 0);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();

    // init: debe insertar 4 imágenes, todas con miniatura ok (son JPEGs directos).
    let init_result = run_cli(&["init", "--path", &path_arg]);
    assert_eq!(init_result["status"], "ok", "init falló: {init_result}");
    assert_eq!(init_result["data"]["scanned"], 4);
    assert_eq!(init_result["data"]["inserted_ok"], 4);
    assert_eq!(init_result["data"]["inserted_failed"], 0);

    // init de nuevo debe ser incremental (no reprocesa nada).
    let init_again = run_cli(&["init", "--path", &path_arg]);
    assert_eq!(init_again["data"]["scanned"], 0);
    assert_eq!(init_again["data"]["skipped_existing"], 4);

    // burst-detect: las 3 fotos de ráfaga deben agruparse; la distinta, no.
    let burst_result = run_cli(&["burst-detect", "--threshold", "0.15", "--db", &db_arg]);
    assert_eq!(
        burst_result["status"], "ok",
        "burst-detect falló: {burst_result}"
    );
    assert_eq!(burst_result["data"]["bursts_created"], 1);
    assert_eq!(burst_result["data"]["images_grouped"], 3);

    // Averiguar los ids de la ráfaga vía variable-list como acceso indirecto no
    // sirve; en vez de eso, abrimos la BD sqlite directamente para leer burst_members.
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let burst_id: i64 = conn
        .query_row("SELECT id FROM bursts LIMIT 1", [], |r| r.get(0))
        .unwrap();
    let mut stmt = conn
        .prepare("SELECT image_id FROM burst_members WHERE burst_id = ?1 ORDER BY image_id")
        .unwrap();
    let members: Vec<i64> = stmt
        .query_map([burst_id], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert_eq!(members.len(), 3);
    drop(stmt);
    drop(conn);

    // burst-tournament: la primera imagen gana, las otras dos quedan rechazadas.
    let ranking = vec![
        format!("{}:1", members[0]),
        format!("{}:2", members[1]),
        format!("{}:3", members[2]),
    ];
    let mut args = vec![
        "burst-tournament".to_string(),
        "--burst-id".to_string(),
        burst_id.to_string(),
        "--db".to_string(),
        db_arg.clone(),
        "--ranking".to_string(),
    ];
    args.extend(ranking);
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let tournament_result = run_cli(&args_ref);
    assert_eq!(
        tournament_result["status"], "ok",
        "burst-tournament falló: {tournament_result}"
    );
    assert_eq!(
        tournament_result["data"]["representative_image_id"],
        members[0]
    );
    assert_eq!(tournament_result["data"]["rejected"], 2);

    // Verificar backup automático (VACUUM INTO) antes de la operación destructiva.
    assert!(folder.join(".photoranker.sqlite.bak").exists());

    // variable-create (ordinal) + variable-list + variable-set.
    let create_result = run_cli(&[
        "variable-create",
        "--name",
        "Grado de nostalgia",
        "--type",
        "ordinal",
        "--min",
        "1",
        "--max",
        "5",
        "--db",
        &db_arg,
    ]);
    assert_eq!(
        create_result["status"], "ok",
        "variable-create falló: {create_result}"
    );

    let list_result = run_cli(&["variable-list", "--db", &db_arg]);
    assert_eq!(list_result["data"].as_array().unwrap().len(), 1);

    let set_arg = format!("{}:4", members[0]);
    let set_result = run_cli(&[
        "variable-set",
        "--variable",
        "Grado de nostalgia",
        "--values",
        &set_arg,
        "--db",
        &db_arg,
    ]);
    assert_eq!(
        set_result["status"], "ok",
        "variable-set falló: {set_result}"
    );
    assert_eq!(set_result["data"]["values_set"], 1);

    // variable-set con valor fuera de rango debe fallar con INVALID_ARGUMENT.
    let out_of_range = format!("{}:99", members[0]);
    let bad_set = run_cli(&[
        "variable-set",
        "--variable",
        "Grado de nostalgia",
        "--values",
        &out_of_range,
        "--db",
        &db_arg,
    ]);
    assert_eq!(bad_set["status"], "error");
    assert_eq!(bad_set["code"], "INVALID_ARGUMENT");

    // prune: borrar un archivo y confirmar que queda missing=1.
    std::fs::remove_file(folder.join("different.jpg")).unwrap();
    let prune_result = run_cli(&["prune", "--db", &db_arg]);
    assert_eq!(prune_result["status"], "ok", "prune falló: {prune_result}");
    assert_eq!(prune_result["data"]["marked_missing"], 1);
}
