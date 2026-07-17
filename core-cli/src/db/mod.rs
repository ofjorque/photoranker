//! Acceso SQLite: apertura de conexiones (WAL obligatorio), migraciones versionadas,
//! búsqueda de la BD local y el índice global (ver docs/database.md, docs/conventions.md).

use crate::error::{AppError, AppResult};
use crate::lock::{self, FileLock};
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub const LOCAL_DB_FILENAME: &str = ".photoranker.sqlite";

static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::new(vec![
        M::up(include_str!("../../../migrations/001_init.sql")),
        M::up(include_str!("../../../migrations/002_quality_metrics.sql")),
        M::up(include_str!("../../../migrations/003_bursts.sql")),
        M::up(include_str!("../../../migrations/004_variables.sql")),
        M::up(include_str!("../../../migrations/005_clustering.sql")),
        M::up(include_str!("../../../migrations/006_tournament.sql")),
        M::up(include_str!("../../../migrations/007_tournament_sync.sql")),
        M::up(include_str!("../../../migrations/008_tournament_undo.sql")),
        M::up(include_str!("../../../migrations/009_raw_jpeg_pairing.sql")),
        M::up(include_str!("../../../migrations/010_cluster_cache.sql")),
        M::up(include_str!("../../../migrations/011_burst_exclusion.sql")),
    ])
});

/// Habilita WAL (obligatorio antes de cualquier otra operación, ver conventions.md).
fn enable_wal(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(())
}

/// Abre (creando el archivo si hace falta) la BD local en `path`, habilita WAL
/// y aplica las migraciones pendientes.
pub fn open_local(path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    enable_wal(&conn)?;
    MIGRATIONS.to_latest(&mut conn)?;
    Ok(conn)
}

/// Igual que `open_local`, pero además adquiere el lock de archivo
/// preventivo de la carpeta (ver `lock.rs`) antes de abrir la conexión —
/// usado por los comandos que **escriben** en la BD local. El `FileLock`
/// devuelto debe mantenerse vivo mientras dure la operación; se libera solo
/// al salir de scope.
pub fn open_local_locked(path: &Path) -> AppResult<(Connection, FileLock)> {
    let file_lock = lock::acquire(path)?;
    let conn = open_local(path)?;
    Ok((conn, file_lock))
}

/// Busca `.photoranker.sqlite` en `start_dir` o en sus directorios padres.
pub fn find_local_db(start_dir: &Path) -> AppResult<PathBuf> {
    let mut dir = Some(start_dir.to_path_buf());
    while let Some(d) = dir {
        let candidate = d.join(LOCAL_DB_FILENAME);
        if candidate.is_file() {
            return Ok(candidate);
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    Err(AppError::DbNotFound)
}

/// Resuelve la ruta de la BD local a usar: `--db` explícito si se pasó, o
/// búsqueda hacia arriba desde el directorio actual.
pub fn resolve_local_db_path(explicit: Option<&Path>) -> AppResult<PathBuf> {
    match explicit {
        Some(p) => Ok(p.to_path_buf()),
        None => {
            let cwd = std::env::current_dir()?;
            find_local_db(&cwd)
        }
    }
}

/// Abre la conexión al índice global (`~/.photoranker/global_index.sqlite`),
/// creándolo si hace falta, en modo WAL + `busy_timeout=5000` (ver conventions.md,
/// "Índice global compartido").
pub fn open_global() -> AppResult<Connection> {
    let path = crate::config::global_index_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    enable_wal(&conn)?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS global_ratings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id TEXT NOT NULL,
            source_db_path TEXT,
            image_id INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            mu REAL NOT NULL,
            rejected INTEGER DEFAULT 0,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(project_id, image_id)
        )",
        [],
    )?;
    add_hash_column_if_missing(&conn)?;
    Ok(conn)
}

/// `global_ratings` no usa `rusqlite_migration` (es un `CREATE TABLE IF NOT
/// EXISTS` a mano desde el día 1, ver arriba) — así que agregar una columna a
/// una tabla que ya existía en instalaciones previas necesita su propio
/// chequeo idempotente: `ALTER TABLE ADD COLUMN` no tiene una forma
/// "IF NOT EXISTS" en SQLite, falla si la columna ya está. Agregada para
/// detección de duplicados entre carpetas (ver
/// docs/fase8-mejoras-avanzadas.md) — copia del mismo pHash ya calculado en
/// `images.hash` de cada carpeta, sincronizado junto con `mu` (ver
/// `commands::tournament::flush_pending_sync`).
fn add_hash_column_if_missing(conn: &Connection) -> AppResult<()> {
    let has_hash: bool = conn
        .prepare("SELECT 1 FROM pragma_table_info('global_ratings') WHERE name = 'hash'")?
        .exists([])?;
    if !has_hash {
        conn.execute("ALTER TABLE global_ratings ADD COLUMN hash TEXT", [])?;
    }
    Ok(())
}

/// Backup no destructivo vía `VACUUM INTO`, ejecutado sobre la conexión abierta
/// (nunca una copia de archivo a nivel de SO, ver fase1-ingesta.md). Se llama
/// antes de cualquier comando que modifique `mu`/`sigma`/`rejected`/`cluster_id`
/// o escriba en el índice global.
pub fn backup(conn: &Connection, db_path: &Path) -> AppResult<()> {
    let backup_path = format!("{}.bak", db_path.display());
    // `VACUUM INTO` exige que el destino no exista todavía; como este backup
    // es un único snapshot rotativo (no un historial), se descarta el
    // anterior antes de escribir el nuevo — si no, cualquier segundo comando
    // mutante sobre la misma BD (ej. dos tournament-result seguidos) fallaría
    // siempre con "output file already exists".
    if Path::new(&backup_path).exists() {
        std::fs::remove_file(&backup_path)?;
    }
    conn.execute("VACUUM INTO ?1", [backup_path])?;
    Ok(())
}
