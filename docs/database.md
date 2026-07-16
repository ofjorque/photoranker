# 📐 Base de Datos: Esquema Completo

> Documento de referencia — consúltalo desde cualquier fase que lea o escriba en SQLite. Ver también `architecture.md`, `conventions.md` (modelo de concurrencia, versionado de migraciones).

## Base de Datos local (SQLite, por carpeta)

```sql
CREATE TABLE project_meta (
    project_id TEXT PRIMARY KEY,  -- UUID v4 generado una sola vez en el primer init; init no lo regenera después
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    config_snapshot TEXT           -- JSON con los parámetros de config.toml vigentes al momento del primer init
                                    -- (burst_threshold, cluster_min/max, trueskill_beta, sigma_stop_threshold, etc.)
                                    -- — permite reproducir exactamente el comportamiento original de esta
                                    -- biblioteca aunque config.toml cambie después globalmente.
);
-- Nota: la versión de esquema NO se guarda aquí. La única fuente de verdad es
-- PRAGMA user_version, gestionado internamente por rusqlite_migration — evita
-- tener dos lugares que puedan desincronizarse (ver "Versionado de la base de
-- datos" en conventions.md).

CREATE TABLE images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT UNIQUE NOT NULL,
    paired_path TEXT,             -- companero RAW+JPEG del mismo disparo (migración 009); NULL si no tiene par, ver fase1-ingesta.md
    hash TEXT,
    thumbnail BLOB,              -- JPEG embebido o decode reducido del RAW
    thumbnail_status TEXT DEFAULT 'ok' CHECK(thumbnail_status IN ('ok','failed')),
    exif_json TEXT,               -- JSON plano completo (respaldo/debug), ver ejemplo abajo
    iso INTEGER,                   -- columna dedicada, extraída de exif_json en init — la usa clustMD directo, sin parsear JSON en R
    aperture REAL,                 -- columna dedicada, ídem
    focal_length REAL,             -- columna dedicada, ídem
    mu REAL DEFAULT 25.0,
    sigma REAL DEFAULT 8.33,
    rating INTEGER,               -- estrellas 1-5 (o -1 si rejected); escrito por export-xmp
    rank_order INTEGER,           -- snapshot de posición al momento de export-xmp (ver comando `ranking`, que es en vivo)
    rejected INTEGER DEFAULT 0,
    stalled INTEGER DEFAULT 0,    -- 1 si sigma no converge tras N rondas (ver criterio de parada, fase3-torneo.md)
    missing INTEGER DEFAULT 0,    -- 1 si file_path ya no existe en disco (ver comando `prune`, fase1-ingesta.md)
    last_compared_at DATETIME,    -- última vez que participó en un grupo de tournament-result; NULL si nunca
    cluster_id INTEGER,           -- argmax de image_clusters.probability
    FOREIGN KEY (cluster_id) REFERENCES clusters(id)
);

-- Índices de rendimiento (además de las PK/UNIQUE ya implícitas). Toda migración
-- que agregue una columna consultada frecuentemente en WHERE/ORDER BY debe
-- crear su índice correspondiente en el mismo archivo de migración.
CREATE INDEX idx_images_mu ON images(mu);
CREATE INDEX idx_images_sigma ON images(sigma);
CREATE INDEX idx_images_rejected ON images(rejected);
```

**Estructura de `exif_json`** (objeto plano, claves fijas, sin anidamiento — se guarda completo como respaldo/debug, pero **no se parsea en R**):

```json
{"iso": 400, "shutter_speed": "1/250", "aperture": 2.8, "focal_length": 35, "lens": "24-70mm f/2.8"}
```

`iso`, `aperture` y `focal_length` se extraen de este JSON **una sola vez, en Rust, durante `init`**, y se guardan en las columnas dedicadas `images.iso`/`images.aperture`/`images.focal_length` (ver tabla arriba). `run_clustmd.R` lee esas columnas directamente por SQL — **nunca parsea `exif_json`**, evitando la necesidad de `jsonlite` en R y manteniendo el diseño relacional. `shutter_speed` y `lens` solo quedan dentro de `exif_json` (no tienen columna dedicada) y no entran al clustering en el MVP, por ser difíciles de tratar como continuas sin parsing adicional. Ver `fase2-clustering.md` para el detalle de `run_clustmd.R`.

```sql
CREATE TABLE bursts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    representative_image_id INTEGER,  -- ganadora del minitorneo
    status TEXT DEFAULT 'pending' CHECK(status IN ('pending','completed')),
    FOREIGN KEY (representative_image_id) REFERENCES images(id)
);

CREATE TABLE burst_members (
    burst_id INTEGER NOT NULL,
    image_id INTEGER NOT NULL,
    similarity_score REAL,   -- distancia normalizada 0-1
    rejected_before INTEGER, -- snapshot de images.rejected previo a burst-tournament
                             -- (migración 011_burst_exclusion.sql), para poder
                             -- deshacer con burst-undo sin asumir que siempre era 0
    PRIMARY KEY (burst_id, image_id)
);
CREATE INDEX idx_burst_members_image_id ON burst_members(image_id);

CREATE TABLE image_quality_metrics (
    image_id INTEGER PRIMARY KEY,
    sharpness REAL,            -- Varianza del Laplaciano
    brightness REAL,           -- Luminancia media (0-255)
    contrast REAL,             -- Desviación estándar de luminancia
    overexposed_pct REAL,      -- % píxeles > 250
    underexposed_pct REAL,     -- % píxeles < 5
    saturation REAL,           -- Promedio canal S (HSV)
    colorfulness REAL,         -- Métrica de Hasler-Süsstrunk
    entropy REAL,              -- Entropía de Shannon del histograma de luminancia
    average_r INTEGER,        -- 0-255
    average_g INTEGER,        -- 0-255
    average_b INTEGER,        -- 0-255
    orientation TEXT CHECK(orientation IN ('portrait','landscape','square')),
    FOREIGN KEY (image_id) REFERENCES images(id)
);
-- Ver fase1-ingesta.md para el detalle de cálculo de cada métrica.

CREATE TABLE user_variables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    var_type TEXT NOT NULL CHECK(var_type IN ('ordinal','nominal')),
    position INTEGER UNIQUE NOT NULL,  -- orden para matriz de clustMD
    min_value REAL,
    max_value REAL
);

CREATE TABLE variable_categories (
    variable_id INTEGER NOT NULL,
    code INTEGER NOT NULL,       -- valor numérico guardado en image_variable_values.value
    label TEXT NOT NULL,         -- lo que ve el usuario (ej. "Interior", "Exterior")
    PRIMARY KEY (variable_id, code),
    FOREIGN KEY (variable_id) REFERENCES user_variables(id)
);

CREATE TABLE image_variable_values (
    image_id INTEGER NOT NULL,
    variable_id INTEGER NOT NULL,
    value REAL,   -- código numérico; para nominales, se mapea vía variable_categories
    PRIMARY KEY (image_id, variable_id)
);

CREATE TABLE clusters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT,
    centroid_json TEXT
);

CREATE TABLE image_clusters (
    image_id INTEGER NOT NULL,
    cluster_id INTEGER NOT NULL,
    probability REAL,
    PRIMARY KEY (image_id, cluster_id)
);
CREATE INDEX idx_image_clusters_cluster_id ON image_clusters(cluster_id);

-- Registra qué imágenes componían un grupo generado por tournament-next, para
-- que tournament-result pueda validar (a) que el group_id existe y sigue
-- pendiente (resolved=0), y (b) que el conjunto de image_id recibido coincide
-- exactamente. Decisión fija: soft-delete — tournament-result marca
-- resolved=1 al procesar el grupo con éxito; NUNCA se borran filas (sirve
-- como auditoría adicional a tournament_matches).
CREATE TABLE pending_tournament_groups (
    group_id TEXT NOT NULL,
    image_id INTEGER NOT NULL,
    resolved INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (group_id, image_id)
);

-- Log/auditoría de resultados de torneo (no es el mecanismo de cálculo;
-- el cálculo real ocurre en una sola llamada a trueskill_multi_team, ver
-- fase3-torneo.md). group_id es un UUID v4 generado por el CLI en cada
-- llamada a tournament-next (no por el cliente/GUI); el mismo valor debe
-- reenviarse en tournament-result.
CREATE TABLE tournament_matches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id TEXT NOT NULL,       -- UUID v4
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    image_id INTEGER NOT NULL,
    rank_position INTEGER NOT NULL  -- empates comparten el mismo número
);
```

## Índice Global (SQLite, único por instalación de usuario)

Ubicación: `~/.photoranker/global_index.sqlite` (Linux/Mac) o `%APPDATA%\PhotoRanker\global_index.sqlite` (Windows). Se crea automáticamente en el primer comando que lo necesite.

```sql
CREATE TABLE global_ratings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,       -- vincula con project_meta.project_id de la BD local (estable aunque se mueva/renombre la carpeta)
    source_db_path TEXT,            -- última ruta conocida de la carpeta, solo referencia/debug — NO es la clave de unión
    image_id INTEGER NOT NULL,      -- id dentro de esa BD local
    file_path TEXT NOT NULL,        -- ruta absoluta de la foto, para referencia/debug
    mu REAL NOT NULL,
    rejected INTEGER DEFAULT 0,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, image_id)
);
```

- **`project_id`**: cada `.photoranker.sqlite` local tiene su propia tabla `project_meta` (una fila, `project_id TEXT PRIMARY KEY`) con un UUID v4 generado **una sola vez**, en el primer `init` de esa carpeta — `init` siendo idempotente, nunca lo regenera en corridas posteriores. Es la clave real de unión con el índice global, precisamente porque **viaja con el archivo de BD local** aunque el usuario mueva o renombre la carpeta de fotos — a diferencia de una ruta absoluta, que se rompe de inmediato en ese caso.
- Se actualiza (upsert vía `INSERT ... ON CONFLICT(project_id, image_id) DO UPDATE SET mu=excluded.mu, rejected=excluded.rejected, source_db_path=excluded.source_db_path, updated_at=CURRENT_TIMESTAMP`) **por lotes** (ver "Sincronización con el índice global" en `fase3-torneo.md`), manteniendo siempre el `mu` más reciente.
- **`resync-global` se simplifica**: como el `project_id` no depende de la ruta, los cuantiles siguen siendo correctos incluso si el usuario nunca corre `resync-global` tras mover una carpeta. El comando solo sirve para refrescar el campo informativo `source_db_path` (abre la BD local en la ruta nueva, lee su `project_id`, actualiza `source_db_path` donde corresponda) — es cosmético, no crítico para la corrección de los cálculos.
- `ranking` y `export-xmp` calculan percentiles de estrellas consultando esta tabla (excluyendo `rejected=1`), no la BD local. Ver `fase4-exportacion.md` para la query exacta de cuantiles.

## Ver también

- `architecture.md` — arquitectura general y por qué existen dos bases de datos.
- `conventions.md` — modelo de concurrencia (WAL, `busy_timeout`), versionado de migraciones, backup con `VACUUM INTO`.
- `fase0-scaffolding.md` — creación inicial de este esquema vía migraciones.
