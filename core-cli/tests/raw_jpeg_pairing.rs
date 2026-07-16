//! Prueba de integración para el emparejamiento RAW+JPEG de `init` — ver
//! docs/fase1-ingesta.md, "RAW + JPEG del mismo disparo cuentan como 1 sola
//! foto" (agregado por feedback de uso real: un disparo en RAW+JPEG no debe
//! contar como 2 fotos en el torneo).

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
fn init_merges_raw_and_jpeg_with_same_stem_into_one_row() {
    let tmp = TempDir::new("raw_jpeg_pair");
    let folder = &tmp.0;

    // El contenido del ".CR3" no importa: cuando hay JPEG emparejado, la
    // miniatura/pHash/métricas se extraen del JPEG, no del RAW (ver
    // fase1-ingesta.md) — bytes arbitrarios simulan un RAW que rawloader no
    // sabría decodificar de todas formas, sin necesitar un CR3 real.
    std::fs::write(folder.join("IMG_0001.CR3"), b"contenido raw arbitrario").unwrap();
    write_solid_jpeg(&folder.join("IMG_0001.JPG"), 128);
    // Foto suelta, sin par, para confirmar que no se fusiona nada de más.
    write_solid_jpeg(&folder.join("IMG_0002.JPG"), 64);

    let db_path = folder.join(".photoranker.sqlite");
    let path_arg = folder.to_string_lossy().to_string();

    let init_result = run_cli(&["init", "--path", &path_arg]);
    assert_eq!(init_result["status"], "ok", "init falló: {init_result}");
    assert_eq!(
        init_result["data"]["scanned"], 3,
        "scanned cuenta archivos crudos, no filas"
    );
    assert_eq!(
        init_result["data"]["inserted_ok"], 2,
        "2 filas lógicas: el par fusionado + la suelta"
    );
    assert_eq!(init_result["data"]["paired_raw_jpeg"], 1);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let mut stmt = conn
        .prepare("SELECT file_path, paired_path, thumbnail_status FROM images ORDER BY id")
        .unwrap();
    let rows: Vec<(String, Option<String>, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    drop(conn);

    assert_eq!(rows.len(), 2, "debe haber exactamente 2 filas en images");
    let paired_row = rows
        .iter()
        .find(|(fp, ..)| fp.to_lowercase().ends_with(".cr3"))
        .expect("debe existir una fila con file_path=el .CR3");
    assert!(
        paired_row
            .1
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .ends_with(".jpg"),
        "paired_path debe apuntar al .JPG: {paired_row:?}"
    );
    assert_eq!(
        paired_row.2, "ok",
        "la miniatura debe salir del JPEG emparejado, no del RAW ilegible"
    );

    let solo_row = rows
        .iter()
        .find(|(fp, ..)| fp.to_lowercase().contains("img_0002"))
        .expect("debe existir la fila suelta");
    assert!(
        solo_row.1.is_none(),
        "la foto sin par no debe tener paired_path"
    );

    // Reinicializar debe ser idempotente: ni el RAW ni el JPEG del par deben
    // volver a contarse como "nuevos" (ver existing_file_paths incluyendo
    // paired_path).
    let init_again = run_cli(&["init", "--path", &path_arg]);
    assert_eq!(init_again["data"]["scanned"], 0);
    assert_eq!(init_again["data"]["skipped_existing"], 3);
}

#[test]
fn export_xmp_writes_sidecar_for_both_files_of_a_pair() {
    let tmp = TempDir::new("raw_jpeg_export");
    let folder = &tmp.0;
    std::fs::write(folder.join("IMG_0001.CR3"), b"contenido raw arbitrario").unwrap();
    write_solid_jpeg(&folder.join("IMG_0001.JPG"), 128);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();
    run_cli(&["init", "--path", &path_arg]);

    let export = run_cli(&["export-xmp", "--db", &db_arg]);
    assert_eq!(export["status"], "ok", "export-xmp falló: {export}");
    assert_eq!(
        export["data"]["written"], 2,
        "un par RAW+JPEG debe escribir 2 sidecars"
    );

    assert!(folder.join("IMG_0001.CR3.xmp").exists());
    assert!(folder.join("IMG_0001.JPG.xmp").exists());

    let raw_xmp = std::fs::read_to_string(folder.join("IMG_0001.CR3.xmp")).unwrap();
    let jpeg_xmp = std::fs::read_to_string(folder.join("IMG_0001.JPG.xmp")).unwrap();
    // Mismo rating en ambos (ver fase4-exportacion.md: "con el mismo rating/label/cluster").
    let extract_rating = |xml: &str| -> String {
        xml.split("xmp:Rating=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(extract_rating(&raw_xmp), extract_rating(&jpeg_xmp));
    assert!(!extract_rating(&raw_xmp).is_empty());
}
