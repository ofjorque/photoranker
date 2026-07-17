//! Pruebas de integración para `list-clusters` y `get-variable-values` —
//! agregados por feedback de uso real sobre la GUI de Fase 5 (ver
//! docs/fase5-gui.md, "clasificación visual" y "fotos representativas por
//! cluster").

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
fn list_clusters_returns_representative_images_ordered_by_probability() {
    let tmp = TempDir::new("list_clusters");
    let folder = &tmp.0;
    for i in 0..6 {
        write_solid_jpeg(&folder.join(format!("img{i}.jpg")), (i * 10) as u8);
    }

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();
    run_cli(&["init", "--path", &path_arg]);

    // Arma el estado de clustering directamente por SQL (igual que
    // fase4_integration.rs) para no depender de Rscript/clustMD en este test.
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let image_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM images ORDER BY id").unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    conn.execute("INSERT INTO clusters (name) VALUES (NULL)", [])
        .unwrap();
    let cluster_id = conn.last_insert_rowid();
    // 5 imágenes en el cluster, con probabilidades distintas; solo las 4 de
    // mayor probabilidad deben aparecer como representativas.
    let probs = [0.95, 0.4, 0.9, 0.6, 0.8];
    for (i, &p) in probs.iter().enumerate() {
        conn.execute(
            "INSERT INTO image_clusters (image_id, cluster_id, probability) VALUES (?1, ?2, ?3)",
            rusqlite::params![image_ids[i], cluster_id, p],
        )
        .unwrap();
        conn.execute(
            "UPDATE images SET cluster_id = ?1 WHERE id = ?2",
            rusqlite::params![cluster_id, image_ids[i]],
        )
        .unwrap();
    }
    drop(conn);

    let listed = run_cli(&["list-clusters", "--db", &db_arg]);
    assert_eq!(listed["status"], "ok", "list-clusters falló: {listed}");
    let clusters = listed["data"].as_array().unwrap();
    assert_eq!(clusters.len(), 1);
    assert_eq!(clusters[0]["member_count"], 5);
    let reps = clusters[0]["representative_images"].as_array().unwrap();
    assert_eq!(reps.len(), 4, "debe recortar a 4 representativas, no 5");
    let probabilities: Vec<f64> = reps
        .iter()
        .map(|r| r["probability"].as_f64().unwrap())
        .collect();
    assert_eq!(
        probabilities,
        vec![0.95, 0.9, 0.8, 0.6],
        "deben venir ordenadas por probability desc, la de 0.4 queda afuera"
    );
}

#[test]
fn list_cluster_images_returns_all_members_not_just_representatives() {
    let tmp = TempDir::new("list_cluster_images");
    let folder = &tmp.0;
    for i in 0..6 {
        write_solid_jpeg(&folder.join(format!("img{i}.jpg")), (i * 10) as u8);
    }

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();
    run_cli(&["init", "--path", &path_arg]);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let image_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM images ORDER BY id").unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    conn.execute("INSERT INTO clusters (name) VALUES (NULL)", [])
        .unwrap();
    let cluster_id = conn.last_insert_rowid();
    // 5 imágenes en el cluster (más que REPRESENTATIVE_IMAGES_PER_CLUSTER=4
    // de list-clusters) — a diferencia de ese comando, acá deben venir las 5.
    let probs = [0.95, 0.4, 0.9, 0.6, 0.8];
    for (i, &p) in probs.iter().enumerate() {
        conn.execute(
            "INSERT INTO image_clusters (image_id, cluster_id, probability) VALUES (?1, ?2, ?3)",
            rusqlite::params![image_ids[i], cluster_id, p],
        )
        .unwrap();
    }
    drop(conn);

    let cluster_id_arg = cluster_id.to_string();
    let listed = run_cli(&[
        "list-cluster-images",
        "--id",
        &cluster_id_arg,
        "--db",
        &db_arg,
    ]);
    assert_eq!(
        listed["status"], "ok",
        "list-cluster-images falló: {listed}"
    );
    let images = listed["data"].as_array().unwrap();
    assert_eq!(
        images.len(),
        5,
        "deben venir las 5 imágenes, no recortadas a 4"
    );
    let probabilities: Vec<f64> = images
        .iter()
        .map(|r| r["probability"].as_f64().unwrap())
        .collect();
    assert_eq!(
        probabilities,
        vec![0.95, 0.9, 0.8, 0.6, 0.4],
        "deben venir ordenadas por probability desc"
    );

    let missing = run_cli(&["list-cluster-images", "--id", "999999", "--db", &db_arg]);
    assert_eq!(missing["status"], "error");
    assert_eq!(missing["code"], "CLUSTER_NOT_FOUND");
}

#[test]
fn get_variable_values_reports_null_for_untagged_and_excludes_rejected() {
    let tmp = TempDir::new("var_values");
    let folder = &tmp.0;
    write_solid_jpeg(&folder.join("a.jpg"), 10);
    write_solid_jpeg(&folder.join("b.jpg"), 20);
    write_solid_jpeg(&folder.join("c.jpg"), 30);

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();
    run_cli(&["init", "--path", &path_arg]);

    run_cli(&[
        "variable-create",
        "--name",
        "Nostalgia",
        "--type",
        "ordinal",
        "--min",
        "1",
        "--max",
        "5",
        "--db",
        &db_arg,
    ]);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM images ORDER BY id").unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    // Marcar la tercera imagen como rejected: no debe aparecer en la lista.
    conn.execute("UPDATE images SET rejected = 1 WHERE id = ?1", [ids[2]])
        .unwrap();
    drop(conn);

    run_cli(&[
        "variable-set",
        "--variable",
        "Nostalgia",
        "--db",
        &db_arg,
        "--values",
        &format!("{}:4", ids[0]),
    ]);

    let values = run_cli(&[
        "get-variable-values",
        "--variable",
        "Nostalgia",
        "--db",
        &db_arg,
    ]);
    assert_eq!(
        values["status"], "ok",
        "get-variable-values falló: {values}"
    );
    let rows = values["data"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "la imagen rejected debe quedar excluida");
    assert_eq!(rows[0]["id"], ids[0]);
    assert_eq!(rows[0]["value"], 4.0);
    assert_eq!(rows[1]["id"], ids[1]);
    assert!(
        rows[1]["value"].is_null(),
        "sin valor asignado debe ser null"
    );

    let missing_var = run_cli(&[
        "get-variable-values",
        "--variable",
        "No Existe",
        "--db",
        &db_arg,
    ]);
    assert_eq!(missing_var["status"], "error");
    assert_eq!(missing_var["code"], "VARIABLE_NOT_FOUND");
}
