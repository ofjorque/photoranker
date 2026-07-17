//! Lock de archivo preventivo para comandos de escritura sobre la misma BD
//! local, agregado por feedback de uso real: la disciplina de "un comando a
//! la vez" de `conventions.md` ("Modelo de concurrencia") asume que la GUI es
//! la única que encola llamadas — no cubre el caso de dos ventanas de la GUI
//! abiertas sobre la misma carpeta, o la GUI corriendo mientras alguien usa
//! el CLI a mano. Ver docs/fase8-mejoras-avanzadas.md, "Lock manager propio".
//!
//! Solo lo adquieren los comandos de escritura (ver `db::open_local_locked`);
//! los de solo lectura siguen sin tocarlo — WAL ya permite lectores
//! concurrentes con un escritor en curso, así que no hace falta bloquearlos,
//! y bloquearlos igual haría que una lectura rápida (ej. `list-clusters`)
//! fallara por timeout mientras un `cluster --k` largo todavía está
//! escribiendo.

use crate::error::{AppError, AppResult};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const LOCK_FILENAME: &str = ".photoranker.lock";
const RETRY_INTERVAL: Duration = Duration::from_millis(100);
/// Mismo orden de magnitud que `busy_timeout=5000` ya usado para el índice
/// global (conventions.md) — si otro comando sigue escribiendo después de
/// esto, se falla explícito en vez de colgarse.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);
/// Un lock más viejo que esto se considera abandonado (el proceso dueño
/// murió sin liberarlo, ej. kill -9) y se descarta en vez de esperarlo.
const STALE_AFTER: Duration = Duration::from_secs(60);

/// Guarda de RAII: mientras vive, el lock está tomado. Se libera solo
/// (borra el archivo) al salir de scope — nunca cruza un `std::process::exit`
/// sin soltarse primero, porque vive dentro del `match` de `run()` en
/// main.rs, que retorna antes de que `main()` llame a `exit`.
pub struct FileLock {
    path: PathBuf,
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Adquiere el lock de escritura de la BD local en `db_path` (bloqueante,
/// con reintento corto hasta `ACQUIRE_TIMEOUT`). `AppError::DbLocked` si
/// otro proceso lo sigue teniendo al agotarse el timeout.
pub fn acquire(db_path: &Path) -> AppResult<FileLock> {
    let lock_path = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(LOCK_FILENAME);

    let started = Instant::now();
    loop {
        match File::options()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut f) => {
                // Contenido informativo (para depuración manual, ej. `cat
                // .photoranker.lock`); no se lee de vuelta por código, la
                // detección de lock abandonado usa la fecha de modificación
                // del archivo (ver `is_stale`), no este PID.
                let _ = write!(f, "{}", std::process::id());
                return Ok(FileLock { path: lock_path });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if is_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    continue;
                }
                if started.elapsed() >= ACQUIRE_TIMEOUT {
                    return Err(AppError::DbLocked);
                }
                std::thread::sleep(RETRY_INTERVAL);
            }
            Err(e) => return Err(AppError::Io(e)),
        }
    }
}

fn is_stale(lock_path: &Path) -> bool {
    match fs::metadata(lock_path).and_then(|m| m.modified()) {
        Ok(modified) => SystemTime::now()
            .duration_since(modified)
            .map(|age| age > STALE_AFTER)
            .unwrap_or(false),
        // Si no se puede leer la metadata (ej. otro proceso lo borró justo
        // ahora), no tratarlo como abandonado — el próximo intento de
        // `create_new` ya resuelve la carrera de forma atómica.
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn temp_db_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "photoranker_lock_test_{name}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("fake.photoranker.sqlite")
    }

    #[test]
    fn second_acquire_fails_fast_while_first_is_held() {
        let db_path = temp_db_path("contended");
        let _first = acquire(&db_path).expect("primer lock debe adquirirse");

        let started = Instant::now();
        let second = acquire(&db_path);
        assert!(
            matches!(second, Err(AppError::DbLocked)),
            "el segundo acquire debe fallar con DbLocked mientras el primero sigue vivo"
        );
        assert!(
            started.elapsed() >= ACQUIRE_TIMEOUT,
            "debe haber esperado el timeout completo antes de rendirse"
        );
    }

    #[test]
    fn lock_is_released_on_drop_and_reacquirable() {
        let db_path = temp_db_path("release");
        {
            let _first = acquire(&db_path).expect("primer lock debe adquirirse");
        } // _first se dropea acá, debería borrar el archivo

        let second = acquire(&db_path);
        assert!(
            second.is_ok(),
            "tras liberar el primero, el segundo debe adquirirse sin esperar"
        );
    }

    #[test]
    fn stale_lock_is_discarded_instead_of_waited_out() {
        let db_path = temp_db_path("stale");
        let lock_path = db_path.parent().unwrap().join(LOCK_FILENAME);
        let lock_file = File::create(&lock_path).unwrap();

        // Retrocede la fecha de modificación más allá de STALE_AFTER (vía
        // std, sin crate nuevo), en vez de dormir 60s real en el test.
        let old_time = SystemTime::now() - STALE_AFTER - Duration::from_secs(1);
        lock_file.set_modified(old_time).unwrap();
        drop(lock_file);

        let started = Instant::now();
        let acquired = acquire(&db_path);
        assert!(
            acquired.is_ok(),
            "un lock abandonado debe descartarse, no esperarse"
        );
        assert!(
            started.elapsed() < ACQUIRE_TIMEOUT,
            "no debería haber esperado el timeout completo para un lock stale"
        );
    }
}
