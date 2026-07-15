# Fase 1 — Ingesta y Ráfagas

> Ver también: `database.md` (tablas `images`, `bursts`, `burst_members`, `image_quality_metrics`), `conventions.md` (patrón `rayon`+SQLite, crates oficiales), `config.md` (`burst_threshold`, `preview_size`).

## 1. Ingesta Inteligente y Limpieza de Ráfagas (Minitorneo manual)

Al importar una carpeta, el CLI de Rust extrae la miniatura JPEG incrustada en el EXIF (sin necesidad de revelar el RAW). Si el archivo trae múltiples miniaturas embebidas (común en Canon/Nikon), **se elige siempre la de mayor resolución**. Antes de guardarla, se corrige la rotación según el tag EXIF `Orientation` (para que quede "de pie" independiente de cómo la haya guardado la cámara) y se reescala a un máximo de `preview_size` (default `512`px en el lado mayor, ver `config.md`) — así el tamaño en BD y el costo de las métricas de calidad quedan acotados. Esta miniatura normalizada (rotada + reescalada) es la que se guarda en `images.thumbnail` y sobre la que se calculan **tanto el pHash como las métricas de calidad** (sección 2 abajo) — nunca sobre el RAW ni sobre una miniatura sin normalizar.

Luego calcula el Hashing Perceptual (pHash) sobre esa miniatura ya normalizada: **pHash clásico basado en DCT, de 64 bits** (crate `img_hash`, configuración por defecto: reducción a 32×32 en escala de grises, DCT, se conservan los 8×8 coeficientes de baja frecuencia → 64 bits de hash). La "longitud del hash en bits" de la fórmula de distancia normalizada es, por lo tanto, **64**.

**`init` es incremental e idempotente**: correrlo de nuevo sobre una carpeta ya inicializada (ej. porque agregaste 200 fotos nuevas) **no destruye nada**. Solo inserta las imágenes cuyo `file_path` aún no existe en `images` (con `mu`/`sigma` por defecto), y deja intactas las ya existentes junto con sus `mu`, `sigma`, `rejected`, clusters y valores de variables. **`init` nunca borra ni marca nada** — solo agrega.

### Fotos borradas o renombradas (`photoranker prune`)

`init` no detecta archivos que desaparecieron de la carpeta (borrados o renombrados fuera de PhotoRanker) — para eso existe un comando separado, `prune`, con responsabilidad única:
- Recorre `images` y marca `missing = 1` en las filas cuyo `file_path` ya no existe en disco.
- Las imágenes `missing = 1` se excluyen de torneos, clustering y `export-xmp` (igual que `rejected`, pero es un estado distinto: no fue una decisión del usuario, fue que el archivo desapareció).
- Se elimina su fila correspondiente de `global_ratings` (vía `project_id` + `image_id`) al ejecutar `prune`, para no seguir contaminando los cuantiles globales con un `mu` fantasma.
- **Renombrados**: `prune` **no intenta reconciliar automáticamente** un archivo renombrado con su historial anterior (sería necesario emparejar por `hash`/pHash, lo cual es ambiguo si hay fotos similares reales en la ráfaga). Un archivo renombrado aparece como `missing=1` en la entrada vieja y como una imagen nueva (con `mu`/`sigma` por defecto) en el siguiente `init` — se pierde su historial de ranking. Esto es una limitación aceptada y documentada del MVP, no un bug a resolver con matching automático.

### Detección y minitorneo de ráfagas

- **Distancia y agrupación de ráfagas**: se usa distancia de Hamming normalizada entre 0 y 1 (`distancia = hamming_distance / 64`). El método de agrupación es **single-linkage (cierre transitivo)**: se construye un grafo donde dos imágenes quedan unidas por una arista si su distancia está por debajo del umbral (`< 0.10` por defecto, `burst_threshold` en `config.md`), y cada **componente conexo** de ese grafo es una ráfaga — es decir, si A~B y B~C (aunque A y C por sí solas superen el umbral), A, B y C quedan en la misma ráfaga. Se eligió single-linkage porque modela bien una ráfaga real de disparo continuo, donde el encuadre va derivando gradualmente foto a foto.
- La GUI (o el CLI en modo texto/TUI) presenta cada grupo al usuario.
- El usuario realiza un **minitorneo por teclado** entre los miembros de la ráfaga para elegir la Campeona (misma mecánica de teclado que el torneo principal, ver `fase3-torneo.md`).
- Las fotos descartadas se marcan como `rejected = 1` en la base de datos local y no participan en torneos posteriores.
- **Herencia semántica (post-clustering)**: la herencia de etiquetas de la ganadora hacia las rechazadas **no ocurre en este paso**, porque los clusters aún no existen. Ocurre recién en `export-xmp` (ver `fase4-exportacion.md`): para cada imagen rechazada, se busca su `representative_image_id` (vía `bursts`/`burst_members`) y se copia el `cluster_id` y el `dc:subject` de la ganadora.

## 2. Métricas Objetivas de Calidad (calculadas automáticamente)

Además del EXIF y las variables subjetivas que tú asignas a mano (ver `fase2-clustering.md`), PhotoRanker calcula automáticamente un conjunto de métricas objetivas sobre la miniatura ya extraída y normalizada (sección 1) — no requiere decodificar el RAW de nuevo. Todas se calculan **sobre la imagen completa** (no sobre recortes centrales ni regiones), son deterministas y sin modelos de ML, usando los crates `image` e `imageproc` de Rust. Los umbrales de exposición (`>250`/`<5`) se aplican directamente sobre los valores de luminancia de 8 bits del JPEG de la miniatura (gamma sRGB tal cual vienen, sin conversión a espacio lineal — es una aproximación intencionalmente simple):

| Métrica | Qué mide | Cálculo |
|---|---|---|
| `sharpness` | Nitidez/foco | Varianza del Laplaciano sobre la imagen en escala de grises |
| `brightness` | Brillo general | Luminancia media (0–255) |
| `contrast` | Rango dinámico | Desviación estándar de la luminancia |
| `overexposed_pct` / `underexposed_pct` | Clipping | % de píxeles >250 y % de píxeles <5, respectivamente |
| `saturation` | Viveza de color | Promedio del canal S en espacio HSV |
| `colorfulness` | Qué tan colorida es la foto | Métrica de Hasler–Süsstrunk: con `rg = R-G` y `yb = 0.5·(R+G)-B`, `colorfulness = sqrt(σ_rg² + σ_yb²) + 0.3·sqrt(μ_rg² + μ_yb²)` |
| `entropy` | Complejidad/detalle visual | Entropía de Shannon del histograma de luminancia (256 bins) |
| `average_r` / `average_g` / `average_b` | Color promedio | Promedio RGB de la miniatura (**no** es "color dominante" en sentido estricto — una foto con sujeto rojo pequeño sobre fondo negro dará un promedio oscuro, no rojo; se nombra `average_*` para ser matemáticamente honesto. No se implementa clustering de color (ej. k-means sobre píxeles) para obtener el verdadero color dominante, para no sobre-ingenierizar esta métrica secundaria) |
| `orientation` | Vertical/horizontal/cuadrada | Calculado directo de `ancho/alto` de la miniatura |

Estas métricas se calculan **una sola vez en `init`**, se guardan en `image_quality_metrics` (ver `database.md`), y se usan de dos formas:

1. **Como variables continuas adicionales para `clustMD`** (ver `fase2-clustering.md`), de modo que el clustering también agrupa por calidad/composición, no solo por parámetros de cámara.
2. **Como panel de referencia en la GUI** (ver `fase5-gui.md`), mostrando valores o íconos de advertencia (ej. baja nitidez, sobreexposición) al ver cada imagen.

`orientation` es la única categórica; se pasa a `clustMD` como `factor()` fijo (`portrait`/`landscape`/`square`), sin pasar por `variable_categories` porque no es editable por el usuario — es un valor computado, no una variable definida por el usuario.

## 3. Manejo de fallas en extracción de miniatura

Si el RAW no trae preview JPEG embebido, se intenta un decode reducido del RAW usando el crate **`rawloader`** (Rust puro — se prefiere sobre bindings a `libraw`/`dcraw` porque estos requieren un toolchain de compilación en C que suele ser problemático en Windows). `rawloader` tiene **cobertura de formatos más limitada que `libraw`** (puede no soportar modelos de cámara muy nuevos o poco comunes) — esto es una limitación aceptada del MVP, no un bug a resolver con un tercer fallback. Si `rawloader` no soporta el formato específico de la cámara o el decode falla:

- La imagen queda marcada con `thumbnail_status = 'failed'`.
- **Queda excluida de torneos y detección de ráfagas** hasta resolverse manualmente.
- El CLI expone un listado de imágenes excluidas y un comando de reintento: `photoranker retry-thumbnail --image-id <id>`.
- **No se implementa un segundo motor de decode como respaldo.** Si el reintento sigue fallando, la recomendación para el usuario es convertir el archivo manualmente a DNG o TIFF con otra herramienta (ej. Adobe DNG Converter) y volver a intentar — esto queda documentado como limitación conocida, no como algo que el CLI deba resolver automáticamente.

## Checklist de implementación

- [ ] Implementar `init --path`: escaneo recursivo, extracción de miniatura EXIF, fallback de decode RAW, pHash, poblar `images`. **Debe ser incremental**: usar `INSERT OR IGNORE` sobre `file_path` (que ya es `UNIQUE`) para garantizar idempotencia sin verificación manual previa, preservando `mu`/`sigma`/`rejected`/clusters/variables de las imágenes existentes. **Patrón de paralelización**: `rayon` (`par_iter()` sobre la lista de archivos) se usa **solo** para el trabajo CPU-bound (decode, pHash, métricas) y cada hilo devuelve su resultado a un `Vec` en memoria — **nunca se comparte una conexión `rusqlite` entre hilos** (ver `conventions.md`). Una vez recolectados todos los resultados, una **única transacción SQLite secuencial** (fuera de `rayon`) hace todos los `INSERT OR IGNORE`. Si `project_meta` está vacía, generar un `project_id` (UUID v4) y guardar un `config_snapshot` (JSON de los parámetros vigentes de `config.toml`) una sola vez; si ya existe, no tocar ninguno de los dos.
- [ ] Implementar `prune`: marcar `missing=1` en `images` cuyo `file_path` ya no existe en disco; excluirlas de torneos/clustering/export; eliminar su fila de `global_ratings`. Sin reconciliación automática de renombrados (limitación aceptada del MVP).
- [ ] Calcular métricas objetivas de calidad (nitidez, brillo, contraste, clipping, saturación, colorido, entropía, color promedio, orientación) sobre la miniatura extraída y poblar `image_quality_metrics`.
- [ ] Implementar `burst-detect --threshold`: agrupar por distancia normalizada de pHash (single-linkage / componentes conexas), excluyendo `missing=1`, poblar `bursts`/`burst_members`.
- [ ] Implementar `burst-tournament --burst-id --ranking id:posición ...`: marcar `rejected=1` en perdedoras, `representative_image_id` en la ganadora. Sin herencia de etiquetas todavía (ocurre en Fase 4).
- [ ] Implementar backup automático (`.photoranker.sqlite.bak`) antes de operaciones destructivas — específicamente antes de cualquier comando que modifique `mu`, `sigma`, `rejected`, `cluster_id`, o escriba en `global_index.sqlite`: `burst-tournament`, `cluster --k`, `tournament-result`, `export-xmp`. Comandos de solo lectura (`ranking`, `tournament-status`, `list-failed-thumbnails`) y `cluster --preview` (no comete cambios) **no** disparan backup. **El backup debe usar `VACUUM INTO 'archivo.bak'`** (ejecutado sobre la conexión SQLite abierta), no una copia de archivo a nivel de sistema operativo — en modo WAL, copiar el `.sqlite` directamente puede omitir cambios que aún viven en el archivo `-wal` y producir un backup inconsistente o desactualizado.
- [ ] Implementar `variable-create`, `variable-list` y `variable-set` para que el usuario defina y asigne variables personalizadas (ordinales/nominales) antes de correr el clustering. Ver `fase2-clustering.md` para el modelo de datos.
- [ ] Implementar `variable-tag` (modo TUI): renderizar `images.thumbnail` con `ratatui-image` (detección automática Kitty/Sixel/ASCII), navegación por teclado, asignación por número, `Espacio` para saltar, `Backspace` para retroceder, `Q` para salir guardando progreso. Detalle completo del modo TUI en `fase3-torneo.md` (sección de Interacción por Teclado, que aplica igual aquí).

## Siguiente fase

`fase2-clustering.md`
