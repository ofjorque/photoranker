# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# PhotoRanker

Este proyecto se construye siguiendo estrictamente la especificación en `README.md` y `docs/` (v1.0, congelada). No tomes decisiones de arquitectura por tu cuenta: si algo no está documentado, pregunta antes de asumir.

**Estado actual del repo**: solo existe la especificación (`README.md` + `docs/`). Todavía no hay `core-cli/`, `gui/`, `migrations/` ni ningún `Cargo.toml` — el primer código a escribir es el de `docs/fase0-scaffolding.md`.

## Cómo navegar la documentación

- `README.md` — punto de entrada, índice a todo lo demás.
- `docs/conventions.md` — **léelo completo antes de escribir cualquier código**. Crates oficiales, formato JSON, concurrencia, estilo de código, testing, definición de MVP.
- `docs/architecture.md`, `docs/database.md`, `docs/config.md`, `docs/cli-reference.md` — referencia transversal, consúltalos cuando la tarea lo requiera.
- `docs/fase0-scaffolding.md` a `docs/fase6-fuera-de-alcance.md` — roadmap secuencial. Cada archivo de fase enlaza a los docs de referencia que necesita.

## Reglas no negociables

- Sigue el roadmap por fases EN ORDEN (`fase0` → `fase1` → ...). No implementes nada de una fase posterior sin permiso explícito.
- Antes de tocar código de una fase, lee su `docs/faseN-*.md` completo junto con `docs/conventions.md`.
- Cada migración SQL nueva va en `migrations/`, nunca se edita una ya existente (ver `docs/conventions.md`, "Versionado de la base de datos").
- Sin `unwrap()` ni `expect()` fuera de tests. Errores tipados con `thiserror`, `anyhow` solo en el borde (`main.rs`).
- No uses `tokio`/async — `main.rs` es completamente síncrono; paralelismo de CPU vía `rayon` (`par_iter()`) únicamente donde el spec lo indica (extracción de miniaturas/pHash/métricas en `init`). `rayon` nunca toca SQLite directamente: cada hilo devuelve su resultado a un `Vec`, y solo el hilo principal escribe en una única transacción `rusqlite`.
- Corre `cargo clippy` (sin warnings) y `cargo fmt` antes de dar por terminada una tarea.
- Si el spec y el código ya escrito entran en conflicto, el spec manda — señálamelo en vez de improvisar una reconciliación silenciosa.

## Arquitectura (resumen — detalle en `docs/architecture.md`)

CLI-first: `photoranker.exe` (Rust, en `core-cli/`) es el único "cerebro". La GUI (Tauri, en `gui/`) es una envoltura visual que invoca el CLI como subproceso, un comando por llamada, y nunca lee `.photoranker.sqlite` directamente.

- **Cada carpeta de fotos** tiene su propia `*.photoranker.sqlite` (fuente de verdad local, vía `rusqlite`, modo WAL obligatorio).
- **Clustering** delega a R (`clustMD`) vía `std::process::Command` sobre `r/run_clustmd.R`, síncrono y bloqueante — sin crate adicional de interop.
- **Torneo** usa el algoritmo Weng-Lin (crate `skillratings`).
- **Exportación** es 100% no destructiva: solo sidecars `.xmp` (crate `quick-xml`), nunca se toca el RAW.
- **Índice global** en `~/.photoranker/global_index.sqlite` (vía crate `directories`) guarda solo `mu` por imagen de todas las carpetas, para percentiles consistentes entre sesiones; puede recibir escrituras concurrentes de instancias distintas del CLI (WAL + `busy_timeout=5000` + reintento con backoff).

## Convenciones de código clave (detalle en `docs/conventions.md`)

- Subcomandos multi-palabra en kebab-case (`burst-detect`, `export-xmp`, `tournament-next`).
- Todo comando imprime **una sola línea JSON** a stdout: `{"status":"ok","data":{...}}` o `{"status":"error","code":"...","message":"..."}`. Exit code 0/1 según status. `stderr` es solo para logs (`tracing`), nunca para el resultado.
- Códigos de error en `SCREAMING_SNAKE_CASE`, catalogados en `core-cli/src/error.rs`.
- Migraciones numeradas (`rusqlite_migration`), versión de esquema única fuente de verdad: `PRAGMA user_version` (sin tabla `schema_version` separada).

## Comandos

```bash
cd core-cli && cargo run -- --help   # CLI (una vez exista el crate)
cargo test                           # unitarios + integración (contra tests/sample_library/)
cargo clippy                         # lint, debe quedar sin warnings
cargo fmt --check                    # formato
```
