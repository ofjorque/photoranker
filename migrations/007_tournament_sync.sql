-- Ver docs/fase3-torneo.md, "Sincronización con el índice global (en lotes, no por grupo)".
-- La CLI es un proceso nuevo por cada invocación (ver conventions.md, "API interna"),
-- así que la "cola en memoria" descrita en el spec se persiste aquí entre llamadas a
-- tournament-result: pending_global_sync guarda el último (mu, rejected) conocido por
-- imagen desde el último flush, y project_meta.pending_sync_count cuenta cuántos
-- resultados de grupo se han acumulado desde ese flush. Al llegar a global_sync_every
-- (o al forzarse antes de una lectura: ranking/tournament-status/export-xmp), se hace
-- el upsert por lote hacia global_ratings y se vacía esta tabla.
ALTER TABLE project_meta ADD COLUMN pending_sync_count INTEGER NOT NULL DEFAULT 0;

CREATE TABLE pending_global_sync (
    image_id INTEGER PRIMARY KEY,
    mu REAL NOT NULL,
    rejected INTEGER NOT NULL,
    queued_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (image_id) REFERENCES images(id)
);

-- stall_counter: rondas consecutivas sin bajar sigma más de un 5% (ver criterio de
-- estancamiento, fase3-torneo.md). No hay tabla de "rondas" separada; se cuenta por
-- imagen porque el estancamiento se evalúa por imagen, no de forma global.
ALTER TABLE images ADD COLUMN stall_counter INTEGER NOT NULL DEFAULT 0;
