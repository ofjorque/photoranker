-- Ver docs/database.md y docs/fase2-clustering.md (clustMD).
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
