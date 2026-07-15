-- Ver docs/database.md ("Base de Datos local (SQLite, por carpeta)").
-- project_meta: una sola fila por BD local, creada en el primer `init` (idempotente, nunca se regenera).
CREATE TABLE project_meta (
    project_id TEXT PRIMARY KEY,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    config_snapshot TEXT
);

-- images: entidad central. cluster_id referencia clusters(id) (migración 005_clustering.sql);
-- SQLite permite una FK hacia una tabla que aún no existe, la constraint solo se valida al insertar.
CREATE TABLE images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT UNIQUE NOT NULL,
    hash TEXT,
    thumbnail BLOB,
    thumbnail_status TEXT DEFAULT 'ok' CHECK(thumbnail_status IN ('ok','failed')),
    exif_json TEXT,
    iso INTEGER,
    aperture REAL,
    focal_length REAL,
    mu REAL DEFAULT 25.0,
    sigma REAL DEFAULT 8.33,
    rating INTEGER,
    rank_order INTEGER,
    rejected INTEGER DEFAULT 0,
    stalled INTEGER DEFAULT 0,
    missing INTEGER DEFAULT 0,
    last_compared_at DATETIME,
    cluster_id INTEGER,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id)
);

CREATE INDEX idx_images_mu ON images(mu);
CREATE INDEX idx_images_sigma ON images(sigma);
CREATE INDEX idx_images_rejected ON images(rejected);
