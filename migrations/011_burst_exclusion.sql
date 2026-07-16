-- Excluir imagen(es) de un burst antes o después de resolver burst-tournament
-- (feedback de uso real: "esta imagen no es parte de una ráfaga"). Se
-- necesita el `rejected` previo a la resolución para poder deshacerla sin
-- asumir que siempre era 0 — mismo espíritu que el snapshot `_before` de
-- tournament_matches (migración 008), pero un solo campo escalar en vez de
-- una fórmula no invertible. Ver docs/fase1-ingesta.md, "Excluir/deshacer bursts".
ALTER TABLE burst_members ADD COLUMN rejected_before INTEGER;
