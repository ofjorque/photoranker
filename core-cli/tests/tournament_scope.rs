//! Prueba de integración para `tournament-next --scope=<subfolder>` (ver
//! docs/fase8-mejoras-avanzadas.md, "Acotar el pool de torneo por
//! subcarpeta").

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
fn tournament_next_with_scope_only_pulls_images_from_matching_subfolder() {
    let tmp = TempDir::new("tournament_scope");
    let folder = &tmp.0;
    let dia1 = folder.join("Dia1");
    let dia2 = folder.join("Dia2");
    std::fs::create_dir_all(&dia1).unwrap();
    std::fs::create_dir_all(&dia2).unwrap();
    write_solid_jpeg(&dia1.join("a.jpg"), 10);
    write_solid_jpeg(&dia1.join("b.jpg"), 20);
    write_solid_jpeg(&dia2.join("c.jpg"), 30);
    write_solid_jpeg(&dia2.join("d.jpg"), 40);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();
    run_cli(&["init", "--path", &path_arg]);

    let scoped = run_cli(&["tournament-next", "--scope", "Dia1", "--db", &db_arg]);
    assert_eq!(scoped["status"], "ok", "tournament-next falló: {scoped}");
    let images = scoped["data"]["images"].as_array().unwrap();
    assert!(
        !images.is_empty(),
        "el grupo con scope no debería estar vacío"
    );
    for img in images {
        let file_path = img["file_path"].as_str().unwrap();
        assert!(
            file_path.contains("Dia1"),
            "con --scope=Dia1 no debería aparecer una imagen fuera de esa subcarpeta: {file_path}"
        );
    }

    // Sin scope, el pool completo (4 imágenes) está disponible — alguna del
    // otro día puede entrar al grupo.
    let unscoped = run_cli(&["tournament-next", "--db", &db_arg]);
    assert_eq!(unscoped["status"], "ok");
    let all_images = unscoped["data"]["images"].as_array().unwrap();
    assert_eq!(
        all_images.len(),
        4,
        "sin scope, con solo 4 imágenes activas, el grupo dinámico debe incluirlas todas"
    );
}

#[test]
fn tournament_next_with_scope_matching_nothing_returns_null() {
    let tmp = TempDir::new("tournament_scope_empty");
    let folder = &tmp.0;
    write_solid_jpeg(&folder.join("a.jpg"), 10);
    write_solid_jpeg(&folder.join("b.jpg"), 20);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();
    run_cli(&["init", "--path", &path_arg]);

    let scoped = run_cli(&[
        "tournament-next",
        "--scope",
        "CarpetaQueNoExiste",
        "--db",
        &db_arg,
    ]);
    assert_eq!(scoped["status"], "ok");
    assert!(
        scoped["data"].is_null(),
        "sin candidatos en el scope, data debe ser null"
    );
}
