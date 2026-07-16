-- Caché de modelos clustMD ya ajustados (feedback de uso real: "cuando uno
-- escoge el modelo, la clusterización debería ser rápida y no volver a correr
-- el código"). `cluster --preview` persiste el mejor modelo ajustado por cada
-- k explorado; `cluster --k <N>` primero consulta esta tabla por
-- (k, data_fingerprint) antes de invocar clustMD de nuevo — ver
-- docs/fase2-clustering.md, "Caché de modelos ajustados".
CREATE TABLE cached_cluster_fits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    k INTEGER NOT NULL,
    model TEXT NOT NULL,
    bic REAL NOT NULL,
    data_fingerprint TEXT NOT NULL,
    rds_path TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_cached_cluster_fits_lookup ON cached_cluster_fits(k, data_fingerprint);
