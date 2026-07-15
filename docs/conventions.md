# 🧭 Convenciones de Desarrollo

> Documento de referencia — **léelo antes de escribir código de cualquier fase**. Es la "constitución" del proyecto. Ver también `architecture.md`, `database.md`, `config.md`.

## Estructura del repositorio

```
PhotoRanker/
├── core-cli/
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands/        -- un módulo por subcomando (init.rs, cluster.rs, tournament.rs, ...)
│   │   ├── db/               -- acceso SQLite, migraciones
│   │   ├── quality/          -- métricas objetivas de calidad
│   │   └── error.rs          -- tipos de error (thiserror)
│   └── Cargo.toml
├── gui/
│   ├── src/
│   └── package.json
├── r/
│   └── run_clustmd.R
├── migrations/
│   ├── 001_init.sql
│   ├── 002_quality_metrics.sql
│   └── ...
├── docs/
├── tests/
│   └── sample_library/       -- fixtures: RAW/JPEG de prueba, BD de ejemplo
└── examples/
```

## Crates oficiales de Rust

Para evitar que se propongan librerías distintas entre módulos, estas son las únicas a usar salvo justificación explícita:

| Crate | Uso |
|---|---|
| `clap` | Parseo de subcomandos y flags del CLI (feature `derive`) |
| `rusqlite` | Acceso a SQLite (feature `bundled`) |
| `rusqlite_migration` | Migraciones versionadas de la BD (ver "Versionado de la base de datos") |
| `image`, `imageproc` | Decodificación y métricas objetivas de calidad |
| `rawloader` | Decode reducido de RAW cuando no hay preview JPEG embebido (Rust puro, sin dependencias de C) |
| `img_hash` | Cálculo de pHash (en vez de implementarlo a mano) |
| `kamadak-exif` | Lectura de metadatos EXIF |
| `walkdir` | Escaneo recursivo de carpetas |
| `rayon` | Paralelización de CPU (cálculo de miniaturas/hashes/métricas en `init`) |
| `directories` | Rutas estándar multiplataforma para `~/.photoranker/` (config e índice global) |
| `serde`, `serde_json` | Serialización de la salida JSON de todos los comandos |
| `quick-xml` | Parseo y merge seguro de sidecars `.xmp` existentes en `export-xmp` |
| `thiserror` | Definición de errores tipados por módulo |
| `anyhow` | Propagación de errores en los bordes (`main.rs`) |
| `tracing`, `tracing-subscriber` | Logging estructurado (ver "Logging") |
| `skillratings` | Motor de torneo Weng-Lin |
| `ratatui`, `ratatui-image` | Modo TUI (`variable-tag`); `ratatui-image` maneja la detección de backend (Kitty/Sixel/ASCII) |

La interfaz con R se hace vía `std::process::Command` de la librería estándar (sin crate adicional).

## Convención de nombres de subcomandos

Todos los subcomandos multi-palabra usan **kebab-case** (`burst-detect`, `export-xmp`, `variable-create`, `tournament-next`, `cluster-rename`, `retry-thumbnail`, `resync-global`). Los de una sola palabra (`init`, `ranking`, `prune`) no llevan guion por no tener nada que separar — es la misma convención, no una excepción. Ver `cli-reference.md` para el catálogo completo.

## Formato JSON estándar de salida

Todo comando imprime **una sola línea JSON** a stdout con este sobre:

```json
// Éxito
{"status":"ok","data":{ }}

// Error
{"status":"error","code":"BURST_NOT_FOUND","message":"Burst 15 no existe."}
```

- `data` contiene el payload específico del comando (puede ser objeto, arreglo, o `null`).
- El **exit code** del proceso es `0` si `status="ok"`, `1` si `status="error"` — así un script o la GUI pueden decidir sin parsear el JSON si solo necesitan éxito/fallo.
- `stderr` se reserva exclusivamente para logs de depuración (nunca para el resultado); la GUI no debe parsear `stderr`.
- Los `code` de error son constantes en `SCREAMING_SNAKE_CASE`, catalogados en `core-cli/src/error.rs`. Ejemplos: `DB_NOT_FOUND`, `BURST_NOT_FOUND`, `IMAGE_NOT_FOUND`, `INVALID_RANKING`, `THUMBNAIL_FAILED`, `VARIABLE_NOT_FOUND`, `CLUSTER_NOT_FOUND`, `R_SUBPROCESS_FAILED`, `INCOMPLETE_RANKING`.

## Modelo de concurrencia

- **Nunca hay dos procesos escribiendo simultáneamente** sobre la misma `.photoranker.sqlite`. SQLite se abre en modo **WAL** (`PRAGMA journal_mode=WAL;`, ejecutado al abrir cualquier conexión — ver `fase0-scaffolding.md`), pero la garantía real de seguridad viene del diseño: cada comando es una operación transaccional que se ejecuta de principio a fin (abre conexión → transacción → commit → cierra conexión) antes de que el proceso termine y libere el archivo. El modo WAL es además **necesario** porque en `cluster`, Rust mantiene el archivo abierto mientras el subproceso de R (`Rscript`) lee/escribe sobre el mismo archivo; sin WAL esto falla con "database is locked".
- **Índice global compartido**: a diferencia de la BD por carpeta, `~/.photoranker/global_index.sqlite` puede recibir escrituras desde **instancias distintas del CLI corriendo sobre carpetas distintas al mismo tiempo** (ej. dos sesiones de torneo en paralelo, una por viaje). Para esto: también en modo WAL, más `PRAGMA busy_timeout=5000;` (espera hasta 5s si el archivo está bloqueado) y el *upsert* se reintenta hasta 3 veces con backoff simple si devuelve `SQLITE_BUSY`. Esto sí es necesario en el MVP (a diferencia del lock manager de `fase6-fuera-de-alcance.md`, que es para la BD local).
- La GUI nunca lanza dos subprocesos de escritura en paralelo sobre la misma BD local; encola las llamadas.
- No se implementa un lock manager propio para la BD local en el MVP — esta disciplina de "un comando a la vez" es suficiente.

## Modelo de ejecución (paralelismo, no async)

PhotoRanker **no usa un runtime asíncrono** (nada de `tokio`/`async-std`). El trabajo pesado (`init`: extracción de miniaturas, pHash, métricas de calidad sobre miles de fotos) es **CPU-bound**, no I/O-bound, así que el paralelismo correcto es de **hilos de CPU vía `rayon`** (`par_iter()` sobre la lista de archivos), no async. `main.rs` es completamente síncrono. **`rayon` nunca toca SQLite directamente**: cada hilo procesa un archivo y devuelve su resultado (miniatura, hash, métricas) a un `Vec` acumulado en el hilo principal; solo después, fuera de `rayon`, una única conexión `rusqlite` hace todas las escrituras en una transacción — SQLite no soporta escrituras concurrentes desde múltiples hilos sobre la misma conexión, y abrir una conexión por hilo generaría contención de locks sin ningún beneficio real. La única excepción es el subproceso de R, que se invoca de forma síncrona y bloqueante con `std::process::Command::output()` — el comando `cluster` simplemente espera a que termine.

## Logging

- Se usa `tracing` + `tracing-subscriber` (no `println!` para logs).
- Los logs van **exclusivamente a stderr** (nunca a stdout, que está reservado para el JSON de salida — ver "Formato JSON estándar de salida" arriba).
- Nivel por defecto `info`; controlable vía variable de entorno `RUST_LOG` (ej. `RUST_LOG=debug photoranker cluster --preview`).
- Se debe loguear explícitamente: fallas de extracción de miniatura, fallas del subproceso de R (`Rscript` no encontrado, error de `clustMD`, JSON de salida malformado), y cada transición de estado del torneo (convergencia/estancamiento/timeout).
- Opcionalmente, con `--log-file <ruta>` se puede redirigir el log a un archivo además de stderr (útil para depurar sesiones largas de torneo).

## Guía de estilo (Rust)

- Prohibido `unwrap()` y `expect()` fuera de tests — todo error se propaga como `Result<T, AppError>` con `thiserror`.
- `anyhow` solo se usa en el borde (`main.rs`) para convertir a mensaje final.
- Documentación `rustdoc` en toda función pública.
- Pruebas unitarias por módulo (mínimo: casos felices + 1 caso de error por comando).
- `cargo clippy` sin warnings y `cargo fmt` aplicado antes de cada commit.

## Versionado de la base de datos

- Migraciones numeradas en `migrations/001_*.sql`, `002_*.sql`, etc.
- **Nunca se modifica una migración ya aplicada** — cualquier cambio de esquema es una migración nueva.
- **Única fuente de verdad de la versión de esquema**: `PRAGMA user_version`, gestionado internamente por `rusqlite_migration` — no existe una tabla `schema_version` separada ni una columna redundante en `project_meta` (ver nota en `database.md`). El CLI revisa `user_version` al abrir cualquier `.photoranker.sqlite` y aplica las migraciones pendientes automáticamente.

## API interna (cómo la GUI llama al CLI)

- La GUI (Tauri) invoca `photoranker.exe` como **subproceso**, un comando por llamada (sin modo interactivo, excepto `variable-tag` que es su propio proceso TUI independiente).
- Comunicación exclusivamente por **stdout (JSON de una línea) + exit code**, según el formato estándar de arriba. No se usan archivos temporales ni pipes adicionales.
- La GUI nunca lee `.photoranker.sqlite` directamente — siempre pasa por el CLI, para mantener una única fuente de lógica.

## Cómo probar

- `cargo test` (unitarios + integración contra `tests/sample_library/`, que incluye un set pequeño de RAW/JPEG de prueba y una BD de ejemplo).
- **Unit tests obligatorios** para toda lógica puramente matemática/determinista, sin necesidad de archivos reales: selección de grupo de 5 por `mu`/`sigma` (`fase3-torneo.md`), mapeo de percentiles a estrellas (`fase4-exportacion.md`), z-score de variables continuas (`fase2-clustering.md`), y el criterio de parada (convergencia/estancamiento/timeout).
- **Fixtures de imágenes de prueba** en `tests/sample_library/`: un puñado de JPEG pequeños (no hace falta RAW real) con EXIF sintético variado (distintos ISO/velocidad/apertura), más 1-2 archivos corruptos a propósito para probar el fallback de miniatura fallida (`thumbnail_status='failed'`).
- `cargo clippy` y `cargo fmt --check` como parte del CI.

### Prueba de aceptación de referencia (flujo funcional completo, especificación end-to-end)

```
Dado:        100 imágenes en una carpeta (2 de ellas con RAW corrupto a propósito)
Cuando:      se ejecuta init
Entonces:    images tiene 100 filas; 98 con thumbnail_status='ok', 2 con 'failed'

Cuando:      se ejecuta burst-detect --threshold 0.10
Entonces:    se generan 12 bursts sobre las 98 imágenes con miniatura ok (las 2 'failed' quedan fuera, no tienen thumbnail que hashear)

Cuando:      se resuelven los 12 minitorneos de ráfaga
Entonces:    quedan 87 imágenes activas con rejected=0 (de las 98 procesables)

Cuando:      se ejecuta cluster --k 5
Entonces:    clusters tiene 5 filas; las 87 imágenes activas tienen cluster_id asignado

Cuando:      se corren rondas de tournament-next / tournament-result hasta convergencia
Entonces:    tournament-status reporta status="converged"

Cuando:      se ejecuta export-xmp
Entonces:    se escriben 98 archivos .xmp (87 con rating 1-5, 11 con rating -1); las 2 imágenes con thumbnail_status='failed' NO reciben .xmp (quedan excluidas hasta resolver con retry-thumbnail)
```

## Definición de MVP

El MVP se considera terminado cuando **todo el flujo funciona desde el CLI**, sin necesidad de GUI:

✅ `init` (incremental) · ✅ `burst-detect` + `burst-tournament` · ✅ `cluster` (`--preview`/`--k`) · ✅ `variable-create`/`variable-set`/`variable-tag` · ✅ `tournament-next`/`tournament-result`/`tournament-status` · ✅ `export-xmp`

La GUI (`fase5-gui.md`) es la envoltura visual de lo anterior — no agrega lógica nueva, y puede ser mínima en su primera versión. Todo lo listado en `fase6-fuera-de-alcance.md` (multiplataforma, i18n, duplicados, etc.) queda explícitamente fuera del MVP.

## Ver también

- `architecture.md` — arquitectura general.
- `database.md` — esquema SQL completo.
- `config.md` — `config.toml` documentado.
- `cli-reference.md` — catálogo de comandos.
