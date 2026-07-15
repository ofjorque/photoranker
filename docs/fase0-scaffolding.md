# Fase 0 — Scaffolding

> Ver también: `conventions.md` (estructura del repositorio, crates oficiales), `database.md` (esquema a migrar), `config.md` (parámetros a inicializar).

## Requisitos e Instalación (Windows)

### 1. Dependencias del Sistema

- R para Windows (https://cran.r-project.org/bin/windows/base/)
- Instalar el paquete `clustMD` y sus dependencias de acceso a SQLite:
  ```R
  install.packages(c("clustMD", "RSQLite", "DBI"))
  ```
- Asegúrate de que `Rscript.exe` esté en el PATH del sistema (o configura `rscript_path` en `config.toml`, ver `config.md`).

### 2. Instalación de la App

(Próximamente: descarga el instalador `.msi` desde la pestaña de Releases, que incluirá la interfaz Tauri y el CLI)

## Desarrollo Local

### Requisitos de Compilación

**Versiones mínimas:**
- Rust ≥ 1.75 (toolchain estable, vía rustup) — requerido por las versiones actuales de `ratatui-image` y `rusqlite_migration`.
- R ≥ 4.3 — versión mínima probada con `clustMD` para variables mixtas sin warnings de compatibilidad.
- SQLite ≥ 3.35 (viene incluido vía el feature `bundled` de `rusqlite`, así que no depende de la versión del sistema) — necesario para `PERCENT_RANK()` y otras funciones de ventana usadas en el cálculo de cuantiles (ver `fase4-exportacion.md`).
- Node.js ≥ 18 LTS (para Tauri).

**Herramientas:** Node.js/npm, Tauri CLI (`npm install -g @tauri-apps/cli`), R con `clustMD`. Ver `conventions.md` para la tabla completa de crates de Rust.

### Pasos

1. Clona el repositorio:
   ```bash
   git clone https://github.com/tu-usuario/PhotoRanker.git
   cd PhotoRanker
   ```
2. Compilar y probar solo el CLI (Core):
   ```bash
   cd core-cli
   cargo run -- --help
   ```
3. Levantar el entorno gráfico completo (Tauri):
   ```bash
   npm install
   npm run tauri dev
   ```

## Checklist de implementación

- [ ] Crear proyecto Rust (`core-cli`) con estructura de subcomandos (`clap` con feature `derive`).
- [ ] Definir el esquema SQLite completo de `database.md` como migraciones versionadas con `rusqlite_migration` en `migrations/001_*.sql`, `002_*.sql`, etc. **Cada migración que agregue una columna consultada en `WHERE`/`ORDER BY` debe crear su índice correspondiente en el mismo archivo** (ver los `CREATE INDEX` ya listados en `database.md`: `images(mu)`, `images(sigma)`, `images(rejected)`, `image_clusters(cluster_id)`, `burst_members(image_id)`).
- [ ] **Habilitar `PRAGMA journal_mode=WAL;` al abrir cualquier conexión SQLite** (tanto en Rust vía `rusqlite` como en R vía `RSQLite`), antes de cualquier otra operación. Es obligatorio: sin esto, el subproceso de R falla con "database is locked" al leer/escribir sobre el mismo archivo que Rust mantiene abierto (ver "Modelo de concurrencia" en `conventions.md`).
- [ ] Configurar `tracing`/`tracing-subscriber` con salida a stderr y nivel controlado por `RUST_LOG` (ver "Logging" en `conventions.md`).
- [ ] Crear `config.toml` en `~/.photoranker/config.toml` (usando el crate `directories` para la ruta multiplataforma) con los defaults documentados en `config.md`.
- [ ] Crear el índice global vacío (`~/.photoranker/global_index.sqlite`) si no existe, con la tabla `global_ratings` (también en modo WAL) — ver `database.md`.

## Siguiente fase

`fase1-ingesta.md`
