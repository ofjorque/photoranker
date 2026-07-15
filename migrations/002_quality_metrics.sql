-- Ver docs/database.md y docs/fase1-ingesta.md (cálculo de cada métrica).
CREATE TABLE image_quality_metrics (
    image_id INTEGER PRIMARY KEY,
    sharpness REAL,
    brightness REAL,
    contrast REAL,
    overexposed_pct REAL,
    underexposed_pct REAL,
    saturation REAL,
    colorfulness REAL,
    entropy REAL,
    average_r INTEGER,
    average_g INTEGER,
    average_b INTEGER,
    orientation TEXT CHECK(orientation IN ('portrait','landscape','square')),
    FOREIGN KEY (image_id) REFERENCES images(id)
);
