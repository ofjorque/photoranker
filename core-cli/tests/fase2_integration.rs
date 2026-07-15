//! Prueba de integración de extremo a extremo para Fase 2 (ver
//! docs/fase2-clustering.md): genera una pequeña biblioteca sintética de
//! JPEGs con dos grupos de brillo claramente distintos y ejerce
//! init -> cluster --preview -> cluster --k -> cluster-rename contra el
//! binario `photoranker`, invocando de verdad `Rscript` + `r/run_clustmd.R`
//! (ver fase0-scaffolding.md: R con clustMD/RSQLite/DBI es un requisito de
//! entorno documentado, no un mock).

use image::{Rgb, RgbImage};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_cli(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_photoranker"))
        .args(args)
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

/// Generador congruencial simple (determinista, sin dependencias) para dar
/// ruido real por-imagen — clustMD ajusta matrices de covarianza por
/// cluster, y una fixture demasiado degenerada (varianza intra-grupo ~0,
/// filas casi duplicadas) produce matrices casi singulares que el EM
/// resuelve con NaN silenciosos en vez de un error de R.
fn lcg_bytes(seed: u32, n: usize) -> Vec<u8> {
    let mut state = seed.wrapping_mul(2654435761).wrapping_add(1);
    (0..n)
        .map(|_| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        })
        .collect()
}

/// Imagen de brillo uniforme con ruido real por-pixel y una pequeña
/// variación de nivel base entre imágenes del mismo grupo — separa los dos
/// grupos por brillo medio (`image_quality_metrics.brightness`/
/// `average_r/g/b`) sin dejar el resto de las métricas casi constantes.
fn write_solid_jpeg(path: &Path, base_level: u8, seed: u32) {
    let mut img = RgbImage::new(48, 48);
    let noise = lcg_bytes(seed, (48 * 48) as usize);
    let level_jitter = (seed % 15) as i16 - 7;
    for (i, pixel) in img.pixels_mut().enumerate() {
        let n = (noise[i] % 21) as i16 - 10;
        let v = (base_level as i16 + level_jitter + n).clamp(0, 255) as u8;
        *pixel = Rgb([v, v, v]);
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
fn full_fase2_flow() {
    let tmp = TempDir::new("fase2");
    let folder = &tmp.0;

    // 20 imágenes oscuras + 20 claras: dos grupos bien separados en brillo.
    // Con muestras muy chicas (se probó con 6+6) algunas de las estructuras
    // de covarianza de clustMD colapsan a un único cluster pese a la
    // separación obvia, por el tamaño de muestra chico combinado con
    // columnas de ruido (sharpness/contrast/entropy) poco informativas y
    // brightness/average_r/g/b casi colineales entre sí. Se verificó
    // empíricamente que 20+20 converge de forma estable a la partición
    // correcta (ver run_clustmd.R, `candidate_models`).
    for i in 0..20 {
        write_solid_jpeg(&folder.join(format!("dark_{i}.jpg")), 20, i);
    }
    for i in 0..20 {
        write_solid_jpeg(&folder.join(format!("bright_{i}.jpg")), 220, i);
    }

    let db_path = folder.join(".photoranker.sqlite");
    let db_arg = db_path.to_string_lossy().to_string();
    let path_arg = folder.to_string_lossy().to_string();

    let init_result = run_cli(&["init", "--path", &path_arg]);
    assert_eq!(init_result["status"], "ok", "init falló: {init_result}");
    assert_eq!(init_result["data"]["scanned"], 40);
    assert_eq!(init_result["data"]["inserted_ok"], 40);

    // cluster --preview: no debe escribir nada en `clusters`, solo reportar BIC por k.
    let preview_result = run_cli(&["cluster", "--preview", "--db", &db_arg]);
    assert_eq!(
        preview_result["status"], "ok",
        "cluster --preview falló: {preview_result}"
    );
    let bic_by_k = preview_result["data"]["bic_by_k"]
        .as_object()
        .expect("bic_by_k debe ser un objeto");
    assert!(!bic_by_k.is_empty(), "bic_by_k no debería estar vacío");

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let clusters_after_preview: i64 = conn
        .query_row("SELECT COUNT(*) FROM clusters", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        clusters_after_preview, 0,
        "--preview no debe comprometer resultados"
    );
    drop(conn);

    // cluster --k 2: sí debe comprometer resultados y separar los 2 grupos de brillo.
    let commit_result = run_cli(&["cluster", "--k", "2", "--db", &db_arg]);
    assert_eq!(
        commit_result["status"], "ok",
        "cluster --k falló: {commit_result}"
    );
    assert_eq!(commit_result["data"]["clusters"], 2);
    assert_eq!(commit_result["data"]["n_assigned"], 40);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let cluster_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM clusters ORDER BY id").unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    assert_eq!(cluster_ids.len(), 2);

    // Cada imagen "dark_*" debe compartir cluster_id, distinto del de "bright_*".
    let dark_cluster: Vec<Option<i64>> = {
        let mut stmt = conn
            .prepare("SELECT cluster_id FROM images WHERE file_path LIKE '%dark_%' ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    let bright_cluster: Vec<Option<i64>> = {
        let mut stmt = conn
            .prepare("SELECT cluster_id FROM images WHERE file_path LIKE '%bright_%' ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    assert!(dark_cluster.iter().all(|c| c.is_some()));
    assert!(bright_cluster.iter().all(|c| c.is_some()));
    assert_eq!(
        dark_cluster
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        1,
        "todas las oscuras deben quedar en el mismo cluster: {dark_cluster:?}"
    );
    assert_eq!(
        bright_cluster
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        1,
        "todas las claras deben quedar en el mismo cluster: {bright_cluster:?}"
    );
    assert_ne!(
        dark_cluster[0], bright_cluster[0],
        "los dos grupos de brillo deben quedar en clusters distintos"
    );

    let image_clusters_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM image_clusters", [], |r| r.get(0))
        .unwrap();
    assert_eq!(image_clusters_rows, 40 * 2); // posterior completo (2 clusters) por imagen
    drop(conn);

    // cluster-rename: bautiza el primer cluster.
    let rename_result = run_cli(&[
        "cluster-rename",
        "--id",
        &cluster_ids[0].to_string(),
        "--name",
        "Oscuras",
        "--db",
        &db_arg,
    ]);
    assert_eq!(
        rename_result["status"], "ok",
        "cluster-rename falló: {rename_result}"
    );

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let name: Option<String> = conn
        .query_row(
            "SELECT name FROM clusters WHERE id = ?1",
            [cluster_ids[0]],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(name.as_deref(), Some("Oscuras"));
    drop(conn);

    // cluster-rename sobre un id inexistente debe fallar con CLUSTER_NOT_FOUND.
    let bad_rename = run_cli(&[
        "cluster-rename",
        "--id",
        "999999",
        "--name",
        "Nada",
        "--db",
        &db_arg,
    ]);
    assert_eq!(bad_rename["status"], "error");
    assert_eq!(bad_rename["code"], "CLUSTER_NOT_FOUND");
}
