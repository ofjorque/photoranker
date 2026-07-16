-- Ver docs/fase3-torneo.md, "Deshacer el último resultado enviado"
-- (tournament-undo, agregado en Fase 5 por feedback de uso real: "me
-- equivoqué y ya mandé el grupo"). tournament-result ahora guarda, por cada
-- fila que ya insertaba en tournament_matches, el estado previo de la imagen
-- (mu/sigma/stall_counter/stalled/last_compared_at) para poder revertirlo
-- exactamente sin tener que invertir la fórmula de weng_lin_multi_team (no es
-- invertible en general). `undone` marca qué grupos ya fueron deshechos, para
-- que tournament-undo siempre opere sobre "el grupo resuelto más reciente
-- todavía no deshecho" y no pueda deshacerse dos veces.
ALTER TABLE tournament_matches ADD COLUMN mu_before REAL;
ALTER TABLE tournament_matches ADD COLUMN sigma_before REAL;
ALTER TABLE tournament_matches ADD COLUMN stall_counter_before INTEGER;
ALTER TABLE tournament_matches ADD COLUMN stalled_before INTEGER;
ALTER TABLE tournament_matches ADD COLUMN last_compared_at_before DATETIME;
ALTER TABLE tournament_matches ADD COLUMN undone INTEGER NOT NULL DEFAULT 0;
