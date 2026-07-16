//! Prueba de integración de extremo a extremo para Fase 4 (ver
//! docs/fase4-exportacion.md): ejerce `export-xmp`, `list-failed-thumbnails`,
//! `retry-thumbnail` y `resync-global` contra el binario `photoranker`.
//!
//! El estado de `bursts`/`burst_members`/`clusters` se arma directamente por
//! SQL (en vez de vía `burst-detect`/`cluster`, que dependen de similitud de
//! pHash real o de R) para aislar el comportamiento de `export-xmp` en sí.
//!
//! Nota histórica: `export-xmp` decide su modo (`quantile` vs
//! `fixed_provisional`) según el tamaño de `~/.photoranker/global_index.sqlite`.
//! Hasta que se agregó `PHOTORANKER_HOME` (ver config.rs) ese archivo era el
//! *real*, compartido con el resto del sistema del usuario — un test corrido
//! sin querer contra la máquina de un usuario real (ver incidente
//! documentado en el historial del proyecto) podía vaciar/contaminar datos
//! reales. `test_home()` aísla cada proceso de test en su propio directorio
//! temporal, así que este test ahora sí controla el estado del índice global
//! de punta a punta y puede asertar `fixed_provisional` con confianza (el
//! índice aislado siempre arranca vacío).

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

fn write_solid_jpeg(path: &Path, level: u8) {
    let mut img = RgbImage::new(32, 32);
    for pixel in img.pixels_mut() {
        *pixel = Rgb([level, level, level]);
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

fn image_id_by_path_suffix(conn: &rusqlite::Connection, suffix: &str) -> i64 {
    conn.query_row(
        "SELECT id FROM images WHERE file_path LIKE ?1",
        [format!("%{suffix}")],
        |r| r.get(0),
    )
    .unwrap_or_else(|e| panic!("no se encontró imagen que termine en '{suffix}': {e}"))
}

#[test]
fn full_fase4_flow() {
    let tmp = TempDir::new("fase4");
    let folder = &tmp.0;

    write_solid_jpeg(&folder.join("winner.jpg"), 220);
    write_solid_jpeg(&folder.join("active.jpg"), 30);
    write_solid_jpeg(&folder.join("rejected.jpg"), 200);
    // Bytes deliberadamente inválidos como JPEG/RAW: agota los 3 fallbacks de
    // thumbnail::extract_normalized y deja thumbnail_status='failed'.
    std::fs::write(folder.join("broken.jpg"), b"no es una imagen valida").unwrap();

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();

    let init_result = run_cli(&["init", "--path", &path_arg]);
    assert_eq!(init_result["status"], "ok", "init falló: {init_result}");
    assert_eq!(init_result["data"]["scanned"], 4);
    assert_eq!(init_result["data"]["inserted_ok"], 3);
    assert_eq!(init_result["data"]["inserted_failed"], 1);

    // --- Arma el estado de burst/cluster directamente por SQL ---
    let winner_id;
    let rejected_id;
    let broken_id;
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        winner_id = image_id_by_path_suffix(&conn, "winner.jpg");
        let active_id = image_id_by_path_suffix(&conn, "active.jpg");
        rejected_id = image_id_by_path_suffix(&conn, "rejected.jpg");
        broken_id = image_id_by_path_suffix(&conn, "broken.jpg");

        conn.execute("UPDATE images SET mu = 45.0 WHERE id = ?1", [winner_id])
            .unwrap();
        conn.execute("UPDATE images SET mu = 5.0 WHERE id = ?1", [active_id])
            .unwrap();
        conn.execute(
            "UPDATE images SET rejected = 1 WHERE id = ?1",
            [rejected_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO clusters (name) VALUES ('Retratos nocturnos')",
            [],
        )
        .unwrap();
        let cluster_id = conn.last_insert_rowid();
        conn.execute(
            "UPDATE images SET cluster_id = ?1 WHERE id = ?2",
            rusqlite::params![cluster_id, winner_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO bursts (representative_image_id, status) VALUES (?1, 'completed')",
            [winner_id],
        )
        .unwrap();
        let burst_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO burst_members (burst_id, image_id, similarity_score) VALUES (?1, ?2, 0.0), (?1, ?3, 0.05)",
            rusqlite::params![burst_id, winner_id, rejected_id],
        )
        .unwrap();
    }

    // --- list-failed-thumbnails ---
    let failed_list = run_cli(&["list-failed-thumbnails", "--db", &db_arg]);
    assert_eq!(failed_list["status"], "ok");
    let failed_array = failed_list["data"].as_array().unwrap();
    assert_eq!(failed_array.len(), 1);
    assert_eq!(failed_array[0]["id"], broken_id);

    // --- retry-thumbnail sobre un archivo que sigue corrupto ---
    let retry_result = run_cli(&[
        "retry-thumbnail",
        "--image-id",
        &broken_id.to_string(),
        "--db",
        &db_arg,
    ]);
    assert_eq!(retry_result["status"], "error");
    assert_eq!(retry_result["code"], "THUMBNAIL_FAILED");

    // --- export-xmp ---
    let export_result = run_cli(&["export-xmp", "--db", &db_arg]);
    assert_eq!(
        export_result["status"], "ok",
        "export-xmp falló: {export_result}"
    );
    assert_eq!(export_result["data"]["written"], 3);
    assert_eq!(export_result["data"]["excluded_failed_thumbnail"], 1);
    assert_eq!(export_result["data"]["excluded_missing"], 0);
    let mode = export_result["data"]["mode"].as_str().unwrap().to_string();

    // Sidecars: convención Darktable (nombre completo + .xmp), y broken.jpg
    // (thumbnail_status='failed') no debe recibir ninguno.
    let winner_xmp = std::fs::read_to_string(folder.join("winner.jpg.xmp")).unwrap();
    let active_xmp = std::fs::read_to_string(folder.join("active.jpg.xmp")).unwrap();
    let rejected_xmp = std::fs::read_to_string(folder.join("rejected.jpg.xmp")).unwrap();
    assert!(!folder.join("broken.jpg.xmp").exists());

    // Invariantes válidas en cualquier modo.
    assert!(rejected_xmp.contains("xmp:Rating=\"-1\""));
    assert!(
        rejected_xmp.contains("<rdf:li>Retratos nocturnos</rdf:li>"),
        "la rechazada debe heredar el tag de la ganadora del burst: {rejected_xmp}"
    );
    assert!(winner_xmp.contains("<rdf:li>Retratos nocturnos</rdf:li>"));
    assert!(
        !active_xmp.contains("dc:subject"),
        "una imagen sin cluster no debe llevar dc:subject: {active_xmp}"
    );

    if mode == "fixed_provisional" {
        // mu=45 -> 5 estrellas; mu=5 -> 1 estrella (ver mapeo fijo de fase4-exportacion.md).
        assert!(winner_xmp.contains("xmp:Rating=\"5\""), "{winner_xmp}");
        assert!(active_xmp.contains("xmp:Rating=\"1\""), "{active_xmp}");
    }

    // rank_order/rating quedan escritos en la BD local para lo exportado.
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let (rating, rank_order): (i64, Option<i64>) = conn
            .query_row(
                "SELECT rating, rank_order FROM images WHERE id = ?1",
                [winner_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(rank_order.is_some());
        if mode == "fixed_provisional" {
            assert_eq!(rating, 5);
        }

        let (broken_rating, broken_rank): (Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT rating, rank_order FROM images WHERE id = ?1",
                [broken_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(
            broken_rating.is_none() && broken_rank.is_none(),
            "una imagen con thumbnail_status='failed' no debe recibir rating/rank_order"
        );
    }

    // --- resync-global: cosmético, no debe fallar aunque no haya filas que tocar ---
    let resync_result = run_cli(&["resync-global", "--path", &path_arg]);
    assert_eq!(
        resync_result["status"], "ok",
        "resync-global falló: {resync_result}"
    );
    assert!(resync_result["data"]["project_id"].is_string());
}
