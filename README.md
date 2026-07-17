# 📸 PhotoRanker
> Curación fotográfica cuantitativa, inteligente y sin "cajas negras"

Volver de un viaje o una sesión de fotos con miles de tomas y tener que elegir cuáles guardar es agotador: mirarlas una por una, comparar a ojo, dudar. PhotoRanker automatiza la parte tediosa. Agrupa las fotos por similitud (ráfagas, encuadres repetidos), las hace competir entre sí de a pocas por vez hasta armar un ranking objetivo, y las clasifica en grupos según los criterios que cada quien defina — sin ningún modelo de IA opaco decidiendo por detrás: todo el criterio es estadística explicable, auditable y entendible.

Es una aplicación de escritorio para Windows con dos caras: un CLI (`photoranker`) que hace todo el trabajo, y una GUI que es simplemente su vidriera visual. Por dentro combina Rust (el motor), R (`clustMD`, para agrupar fotos por variables mixtas) y el algoritmo TrueSkill para el ranking competitivo. El resultado se exporta como archivos `.xmp` — nunca toca los RAW originales, y es compatible con Darktable. Todo se puede operar sin soltar el teclado.

## Instalación

**La forma más simple:** descargar el instalador desde [Releases](https://github.com/ofjorque/photoranker/releases) (`.msi` o `.exe`) — trae la GUI y el CLI ya empaquetados juntos. Lo único que hay que instalar aparte es **R ≥ 4.3** con los paquetes que usa el motor de agrupamiento (la lista completa está en `docs/fase0-scaffolding.md`).

**Para compilarlo desde el código** (desarrollo, o para meterle mano):

1. Instalar R ≥ 4.3 con los paquetes de `docs/fase0-scaffolding.md`, Rust ≥ 1.75 y Node ≥ 18 LTS.
2. Clonar este repositorio.
3. Para el CLI: `cd core-cli && cargo run -- --help`.
4. Para la GUI: `cd gui && npm install && npm run tauri dev`.

## Cómo se usa

Desde el CLI, un flujo típico se ve así:

```bash
photoranker init --path "C:\Fotos\Boda_Juan"
photoranker burst-detect --threshold 0.10
photoranker cluster --preview
photoranker tournament-next
photoranker export-xmp
```

La GUI hace exactamente lo mismo, solo que con clics en vez de comandos — es una envoltura visual, sin ninguna lógica propia por detrás. El catálogo completo de comandos está en `docs/cli-reference.md`.

## Para profundizar

Este README es la puerta de entrada. Toda la documentación técnica del proyecto — arquitectura, esquema de base de datos, convenciones de código, y el detalle de cómo se construyó cada parte — vive en `docs/`:

- [`docs/architecture.md`](docs/architecture.md) — cómo está armado por dentro y por qué
- [`docs/database.md`](docs/database.md) — el esquema completo (base local + índice global)
- [`docs/conventions.md`](docs/conventions.md) — convenciones de código, formato de datos, testing
- [`docs/config.md`](docs/config.md) — todos los parámetros configurables
- [`docs/cli-reference.md`](docs/cli-reference.md) — catálogo completo de comandos

Para quien tenga curiosidad por la historia de cómo se construyó, fase por fase (desde el esqueleto inicial hasta las mejoras más recientes), están los `docs/faseN-*.md`, del 0 al 8.

## Cómo hacer un release

Esto es para quien mantenga el proyecto, no para quien solo lo usa.

Pushear un tag `vX.Y.Z` dispara [`.github/workflows/release.yml`](.github/workflows/release.yml): compila todo, empaqueta el CLI junto a la GUI, y deja un **Release en borrador** en GitHub con los instaladores de Windows adjuntos. No publica nada de forma automática — el borrador queda esperando revisión.

1. Actualizar la versión en los 3 lugares que tienen que coincidir: `core-cli/Cargo.toml`, `gui/package.json`, `gui/src-tauri/tauri.conf.json`.
2. Ejecutar las verificaciones a mano antes de crear el tag (el workflow no corre `cargo test` porque necesitaría R instalado en el runner):
   ```bash
   cd core-cli && cargo test && cargo clippy --all-targets && cargo fmt --check
   cd ../gui && npx tsc --noEmit && npm run build
   ```
3. Confirmar (commit) el cambio de versión y subirlo a `master`.
4. Crear y subir el tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.
5. Esperar a que termine el workflow (pestaña **Actions**) — tarda varios minutos.
6. Ir a **Releases**, abrir el borrador, escribir las notas, y publicarlo.

## Licencia

GPL-3.0. Quien tome este código para armar otra cosa, esa obra derivada también tiene que ser de código abierto y bajo los mismos términos.
