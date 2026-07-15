-- Ver docs/database.md y docs/fase1-ingesta.md (detección de ráfagas).
CREATE TABLE bursts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    representative_image_id INTEGER,
    status TEXT DEFAULT 'pending' CHECK(status IN ('pending','completed')),
    FOREIGN KEY (representative_image_id) REFERENCES images(id)
);

CREATE TABLE burst_members (
    burst_id INTEGER NOT NULL,
    image_id INTEGER NOT NULL,
    similarity_score REAL,
    PRIMARY KEY (burst_id, image_id)
);
CREATE INDEX idx_burst_members_image_id ON burst_members(image_id);
