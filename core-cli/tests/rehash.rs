//! Prueba de integración para `rehash` y para la corrección del algoritmo de
//! pHash (`HashAlg::Gradient`, no `Mean`) — ver docs/fase1-ingesta.md,
//! "Recálculo de pHash". Bug real detectado en uso real: `Mean`+`preproc_dct()`
//! dejaba que el coeficiente DC dominara el promedio, colapsando casi todos
//! los hashes a ~0-16 bits en 1 (sobre 64) sin importar el contenido real de
//! la foto, y agrupando fotos sin relación como si fueran la misma ráfaga.

use image::{Rgb, RgbImage};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn test_home() -> &'static Path {
    use std::sync::OnceLock;
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!(
            "photoranker_test_home_rehash_{}",
            std::process::id()
        ));
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

/// Imagen con cuadrantes grandes (misma fixture que phash.rs, "sobrevive" al
/// downsample para DCT/pHash — ruido por-píxel se promedia y desaparece).
fn quadrant_jpeg(path: &Path, bright_quadrant: u8) {
    let mut img = RgbImage::new(256, 256);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let q = (if x < 128 { 0 } else { 1 }) + (if y < 128 { 0 } else { 2 });
        let v = if q == bright_quadrant { 240 } else { 20 };
        *p = Rgb([v, v, v]);
    }
    img.save(path).expect("no se pudo guardar el JPEG");
}

fn popcount_hex(hash: &str) -> u32 {
    (0..hash.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hash[i..i + 2], 16)
                .unwrap()
                .count_ones()
        })
        .sum()
}

#[test]
fn distinct_quadrant_photos_are_not_grouped_into_a_burst() {
    let tmp = TempDir::new("rehash_distinct");
    let folder = &tmp.0;
    quadrant_jpeg(&folder.join("a.jpg"), 0);
    quadrant_jpeg(&folder.join("b.jpg"), 3); // cuadrante opuesto — foto claramente distinta

    let path_arg = folder.to_string_lossy().to_string();
    run_cli(&["init", "--path", &path_arg]);

    let db_path = folder.join(".photoranker.sqlite");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let hashes: Vec<String> = conn
        .prepare("SELECT hash FROM images ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert_eq!(hashes.len(), 2);

    // Con el algoritmo roto (Mean+DCT) estos hashes colapsaban a ~0-2 bits en
    // 1 sobre 64; con Gradient+DCT deben tener densidad de bits razonable.
    for h in &hashes {
        let ones = popcount_hex(h);
        assert!(
            (10..55).contains(&ones),
            "hash {h} tiene una densidad de bits sospechosa ({ones}/64) — posible regresión al bug de Mean+DCT"
        );
    }
    assert_ne!(
        hashes[0], hashes[1],
        "cuadrantes opuestos no deben hashear igual"
    );

    let db_arg = db_path.to_string_lossy().to_string();
    let burst_result = run_cli(&["burst-detect", "--db", &db_arg]);
    assert_eq!(
        burst_result["status"], "ok",
        "burst-detect falló: {burst_result}"
    );
    assert_eq!(
        burst_result["data"]["bursts_created"], 0,
        "dos fotos con cuadrantes opuestos no deben agruparse como ráfaga: {burst_result}"
    );
}

#[test]
fn rehash_recomputes_hash_from_stored_thumbnail_and_restores_discriminative_power() {
    let tmp = TempDir::new("rehash_recompute");
    let folder = &tmp.0;
    quadrant_jpeg(&folder.join("a.jpg"), 0);
    quadrant_jpeg(&folder.join("b.jpg"), 3);

    let path_arg = folder.to_string_lossy().to_string();
    run_cli(&["init", "--path", &path_arg]);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();

    // Simula hashes calculados con el algoritmo viejo/roto: ambos colapsados
    // al mismo valor degenerado, como pasaba en uso real con Mean+DCT.
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute("UPDATE images SET hash = '0100000000000000'", [])
            .unwrap();
    }

    let rehash_result = run_cli(&["rehash", "--db", &db_arg]);
    assert_eq!(
        rehash_result["status"], "ok",
        "rehash falló: {rehash_result}"
    );
    assert_eq!(rehash_result["data"]["candidates"], 2);
    assert_eq!(rehash_result["data"]["rehashed"], 2);
    assert_eq!(rehash_result["data"]["failed_to_decode"], 0);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let hashes: Vec<String> = conn
        .prepare("SELECT hash FROM images ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert_eq!(hashes.len(), 2);
    assert!(
        hashes.iter().all(|h| h != "0100000000000000"),
        "rehash debe reemplazar el hash simulado viejo: {hashes:?}"
    );
    assert_ne!(
        hashes[0], hashes[1],
        "tras rehash, dos fotos distintas deben volver a tener hashes distintos"
    );
}
