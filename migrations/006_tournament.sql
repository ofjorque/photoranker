-- Ver docs/database.md y docs/fase3-torneo.md (tournament-next/tournament-result).
-- pending_tournament_groups: soft-delete, tournament-result marca resolved=1, nunca se borran filas.
CREATE TABLE pending_tournament_groups (
    group_id TEXT NOT NULL,
    image_id INTEGER NOT NULL,
    resolved INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (group_id, image_id)
);

-- tournament_matches: log/auditoría de resultados, no es el mecanismo de cálculo del rating.
CREATE TABLE tournament_matches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id TEXT NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    image_id INTEGER NOT NULL,
    rank_position INTEGER NOT NULL
);
