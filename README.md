# 📸 PhotoRanker
> **Curación fotográfica cuantitativa, inteligente y sin "cajas negras"**

> **Especificación v1.0 — congelada.** Este documento fue revisado en 9 rondas cruzadas (ChatGPT, Gemini, DeepSeek, Kimi, Qwen) y no quedan huecos técnicos abiertos. A partir de aquí, cambios grandes de arquitectura durante la implementación del MVP deberían evitarse — si surge una necesidad real de cambio, se documenta como una decisión explícita, no como una improvisación de quien esté programando.

PhotoRanker es una suite de herramientas de escritorio (CLI + GUI) diseñada para fotógrafos que buscan organizar, clasificar y rankear grandes volúmenes de imágenes de manera lógica, matemática y sin fatiga de decisión.

Usa un núcleo en **Rust** que se comunica con **R** (`clustMD`) para agrupar tus fotos por variables mixtas, las ordena de forma competitiva con el algoritmo estadístico **TrueSkill** (crate `skillratings`, migrado desde Weng-Lin por feedback de uso real — ver `docs/fase3-torneo.md`), y exporta el resultado de forma 100% no destructiva a sidecars `.xmp` compatibles con **Darktable**. Todo navegable por teclado. Ver `docs/architecture.md` para el detalle de principios y arquitectura.

## Instalación rápida

1. Instala R ≥ 4.3 con `clustMD`, `RSQLite`, `DBI`; Rust ≥ 1.75; Node ≥ 18 LTS.
2. `git clone` este repositorio.
3. `cd core-cli && cargo run -- --help`

Detalle completo en `docs/fase0-scaffolding.md`.

## Uso rápido

```bash
photoranker init --path "C:\Fotos\Boda_Juan"
photoranker burst-detect --threshold 0.10
photoranker cluster --preview
photoranker tournament-next
photoranker export-xmp
```

Catálogo completo de comandos en `docs/cli-reference.md`.

## Documentación técnica completa

Este README es solo la puerta de entrada. La especificación completa vive en `docs/`, dividida en dos capas:

**Capa de referencia** (consúltala desde cualquier fase):
- [`docs/architecture.md`](docs/architecture.md) — principios, no-objetivos, arquitectura CLI-First + GUI
- [`docs/database.md`](docs/database.md) — esquema SQL completo (BD local + índice global)
- [`docs/conventions.md`](docs/conventions.md) — crates oficiales, formato JSON, concurrencia, estilo de código, testing, definición de MVP
- [`docs/config.md`](docs/config.md) — `config.toml` completo documentado
- [`docs/cli-reference.md`](docs/cli-reference.md) — catálogo de todos los comandos

**Capa de fases** (roadmap secuencial — sigue este orden):
- [`docs/fase0-scaffolding.md`](docs/fase0-scaffolding.md)
- [`docs/fase1-ingesta.md`](docs/fase1-ingesta.md) — ingesta, ráfagas, métricas de calidad, `prune`
- [`docs/fase2-clustering.md`](docs/fase2-clustering.md) — `clustMD` en R
- [`docs/fase3-torneo.md`](docs/fase3-torneo.md) — TrueSkill, interacción por teclado
- [`docs/fase4-exportacion.md`](docs/fase4-exportacion.md) — XMP
- [`docs/fase5-gui.md`](docs/fase5-gui.md) — GUI Tauri
- [`docs/fase6-fuera-de-alcance.md`](docs/fase6-fuera-de-alcance.md) — explícitamente fuera del MVP
- [`docs/fase7-mejoras-post-mvp.md`](docs/fase7-mejoras-post-mvp.md) — mejoras de UX/GUI post-MVP, sí implementables
- [`docs/fase8-mejoras-avanzadas.md`](docs/fase8-mejoras-avanzadas.md) — mejoras de arquitectura liberadas de fase 6 (duplicados, scope de torneo, lock manager, vista en grilla)

**Nota para Claude Code / IA implementadora**: lee `docs/conventions.md` completo antes de escribir código de cualquier fase — es la "constitución" del proyecto. Luego, para cada fase, lee su archivo `docs/faseN-*.md` junto con `docs/database.md` y `docs/config.md` (referenciados desde cada fase según lo que necesite).

## Cómo hacer un release

Un push de un tag `vX.Y.Z` dispara [`.github/workflows/release.yml`](.github/workflows/release.yml): compila `core-cli` en modo release, lo empaqueta junto a la GUI (`bundle.resources` en `tauri.conf.json`, así `photoranker.exe` queda al lado del ejecutable de la GUI en producción — ver `resolve_cli_path()` en `gui/src-tauri/src/lib.rs`) y crea un **Release borrador** en GitHub con el instalador de Windows (`.msi` + `.exe`) adjunto. Nada se publica automáticamente — el borrador queda para revisar y publicar a mano.

Pasos:

1. **Actualizá la versión** en los 3 lugares que deben coincidir: `core-cli/Cargo.toml` (`version`), `gui/package.json` (`version`), `gui/src-tauri/tauri.conf.json` (`version`).
2. **Corré los checks localmente** antes de taggear (el workflow de release no corre `cargo test`, porque `fase2_integration` necesita R instalado y eso no está en el runner de CI):
   ```bash
   cd core-cli && cargo test && cargo clippy --all-targets && cargo fmt --check
   cd ../gui && npx tsc --noEmit && npm run build
   ```
3. **Commiteá** el bump de versión (`git commit -am "Release vX.Y.Z"`) y pusheá a `master`.
4. **Taggeá y pusheá el tag**:
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```
5. Esperá a que termine el workflow (pestaña **Actions** del repo en GitHub) — arma el `.msi`/`.exe` desde cero, tarda varios minutos.
6. Andá a **Releases** en GitHub, abrí el borrador que se creó, escribí las notas de la versión, y publicalo.

Primer release (`v0.1.0`): además de lo anterior, hace falta que el repo ya tenga un remoto de GitHub configurado (`git remote add origin <url>` si todavía no existe) y el primer push a `master` (`git push -u origin master`) antes de taggear.

## Licencia

GNU General Public License v3.0 (GPL-3.0). PhotoRanker es software libre. Si modificas, bifurcas o utilizas el código de este proyecto para construir otra aplicación, tu obra derivada debe ser de código abierto y distribuirse bajo los mismos términos de la GPL-3.0.
