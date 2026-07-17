# 📸 PhotoRanker
> **Curación fotográfica cuantitativa, inteligente y sin "cajas negras"**

PhotoRanker es una suite de herramientas de escritorio (CLI + GUI) diseñada para fotógrafos que buscan organizar, clasificar y ordenar grandes volúmenes de imágenes de manera lógica, matemática y sin fatiga de decisión.

Utiliza un núcleo en **Rust** que se comunica con **R** (`clustMD`) para agrupar las fotos por variables mixtas, las ordena de forma competitiva con el algoritmo estadístico **TrueSkill** (biblioteca `skillratings`, migrada desde Weng-Lin por retroalimentación de uso real — ver `docs/fase3-torneo.md`), y exporta el resultado de forma 100% no destructiva a archivos complementarios `.xmp` compatibles con **Darktable**. Toda la interacción es navegable por teclado. Consulta `docs/architecture.md` para el detalle de principios y arquitectura.

**Estado del proyecto**: el MVP (fases 0 a 5: ingesta, clustering, torneo, exportación y GUI) está completo y en uso, junto con dos rondas adicionales de mejoras (`docs/fase7-mejoras-post-mvp.md` y `docs/fase8-mejoras-avanzadas.md`). El repositorio es público en [github.com/ofjorque/photoranker](https://github.com/ofjorque/photoranker).

## Instalación

### Opción 1: instalador (Windows)

Descargar el instalador más reciente desde la sección [Releases](https://github.com/ofjorque/photoranker/releases) del repositorio (`.msi` o `.exe`). Incluye la GUI y el CLI empaquetados juntos; solo falta tener **R ≥ 4.3** instalado por separado con los paquetes que usa el motor de clustering (ver `docs/fase0-scaffolding.md` para la lista completa).

### Opción 2: desde el código fuente (desarrollo)

1. Instalar R ≥ 4.3 con los paquetes listados en `docs/fase0-scaffolding.md`; Rust ≥ 1.75; Node ≥ 18 LTS.
2. Clonar este repositorio.
3. CLI: `cd core-cli && cargo run -- --help`.
4. GUI: `cd gui && npm install && npm run tauri dev`.

Detalle completo en `docs/fase0-scaffolding.md`.

## Uso rápido

Vía CLI:

```bash
photoranker init --path "C:\Fotos\Boda_Juan"
photoranker burst-detect --threshold 0.10
photoranker cluster --preview
photoranker tournament-next
photoranker export-xmp
```

Catálogo completo de comandos en `docs/cli-reference.md`. La GUI envuelve exactamente estos mismos comandos, sin lógica adicional (ver `docs/architecture.md`).

## Documentación técnica completa

Este README es solo la puerta de entrada. La especificación completa vive en `docs/`, dividida en dos capas:

**Capa de referencia** (consultar desde cualquier fase):
- [`docs/architecture.md`](docs/architecture.md) — principios, no objetivos, arquitectura CLI-first + GUI
- [`docs/database.md`](docs/database.md) — esquema SQL completo (base de datos local + índice global)
- [`docs/conventions.md`](docs/conventions.md) — bibliotecas oficiales, formato JSON, concurrencia, estilo de código, pruebas, definición de MVP
- [`docs/config.md`](docs/config.md) — `config.toml` completo documentado
- [`docs/cli-reference.md`](docs/cli-reference.md) — catálogo de todos los comandos

**Capa de fases** (roadmap secuencial, todas implementadas salvo la excepción indicada):
- [`docs/fase0-scaffolding.md`](docs/fase0-scaffolding.md) — estructura inicial del proyecto
- [`docs/fase1-ingesta.md`](docs/fase1-ingesta.md) — ingesta, ráfagas, métricas de calidad, `prune`
- [`docs/fase2-clustering.md`](docs/fase2-clustering.md) — `clustMD` en R
- [`docs/fase3-torneo.md`](docs/fase3-torneo.md) — TrueSkill, interacción por teclado
- [`docs/fase4-exportacion.md`](docs/fase4-exportacion.md) — exportación a XMP
- [`docs/fase5-gui.md`](docs/fase5-gui.md) — GUI en Tauri
- [`docs/fase6-fuera-de-alcance.md`](docs/fase6-fuera-de-alcance.md) — explícitamente fuera del alcance del MVP (no implementada; documenta límites)
- [`docs/fase7-mejoras-post-mvp.md`](docs/fase7-mejoras-post-mvp.md) — mejoras de UX/GUI post-MVP (menú contextual, categorías nominales, tabs de inicio, entre otras)
- [`docs/fase8-mejoras-avanzadas.md`](docs/fase8-mejoras-avanzadas.md) — mejoras de arquitectura liberadas de la fase 6 (duplicados entre carpetas, scope de torneo por subcarpeta, lock manager de archivo, vista en grilla del clasificador)

**Nota para Claude Code / IA implementadora**: leer `docs/conventions.md` completo antes de escribir código de cualquier fase — es la "constitución" del proyecto. Luego, para cada fase, leer su archivo `docs/faseN-*.md` junto con `docs/database.md` y `docs/config.md` (referenciados desde cada fase según lo que necesite).

## Cómo hacer un release

Un push de un tag `vX.Y.Z` dispara [`.github/workflows/release.yml`](.github/workflows/release.yml): compila `core-cli` en modo release, lo empaqueta junto a la GUI (`bundle.resources` en `tauri.conf.json`, de modo que `photoranker.exe` queda junto al ejecutable de la GUI en producción — ver `resolve_cli_path()` en `gui/src-tauri/src/lib.rs`) y crea un **Release en borrador** en GitHub con el instalador de Windows (`.msi` + `.exe`) adjunto. Nada se publica automáticamente: el borrador queda pendiente de revisión y publicación manual.

Pasos:

1. **Actualizar la versión** en los 3 lugares que deben coincidir: `core-cli/Cargo.toml` (`version`), `gui/package.json` (`version`), `gui/src-tauri/tauri.conf.json` (`version`).
2. **Ejecutar las verificaciones localmente** antes de crear el tag (el workflow de release no ejecuta `cargo test`, porque `fase2_integration` necesita R instalado y eso no está disponible en el ejecutor de CI):
   ```bash
   cd core-cli && cargo test && cargo clippy --all-targets && cargo fmt --check
   cd ../gui && npx tsc --noEmit && npm run build
   ```
3. **Confirmar** (commit) el incremento de versión (`git commit -am "Release vX.Y.Z"`) y subirlo (push) a `master`.
4. **Crear y subir el tag**:
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```
5. Esperar a que termine el workflow (pestaña **Actions** del repositorio en GitHub) — genera el `.msi`/`.exe` desde cero, lo que toma varios minutos.
6. Ir a **Releases** en GitHub, abrir el borrador creado, escribir las notas de la versión y publicarlo.

## Licencia

GNU General Public License v3.0 (GPL-3.0). PhotoRanker es software libre. Quien modifique, bifurque o utilice el código de este proyecto para construir otra aplicación debe distribuir esa obra derivada como código abierto y bajo los mismos términos de la GPL-3.0.
