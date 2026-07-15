# Fase 5 — GUI (Tauri)

> Ver también: `conventions.md` (API interna GUI↔CLI), `fase3-torneo.md` (mecánica de teclado a replicar visualmente), `fase1-ingesta.md` (métricas de calidad a mostrar), `fase2-clustering.md` (scree plot de BIC).

La GUI es la envoltura visual de todo lo implementado en las fases 0-4 — no agrega lógica nueva (ver "Definición de MVP" en `conventions.md`). Invoca `photoranker.exe` como subproceso según el contrato de "API interna" de `conventions.md`.

## Checklist de implementación

- [ ] Envolver todos los comandos anteriores como llamadas de subproceso desde Tauri.
- [ ] Implementar navegación e interacción por teclado (flechas/Tab para foco, `1`–`5` para asignar posición, `Enter` para confirmar con validación de completitud, `Backspace`/`R` para reset) — replicando exactamente la mecánica descrita en `fase3-torneo.md`.
- [ ] Feedback visual: foco = borde azul; posición asignada = badge verde con número; empate = badge naranjo con ícono "=".
- [ ] Scree plot de BIC para `cluster --preview` (ver `fase2-clustering.md`).
- [ ] Panel de referencia con métricas objetivas de calidad por imagen (valores e íconos de advertencia para baja nitidez o clipping, ver `fase1-ingesta.md`).

## Siguiente fase

`fase6-fuera-de-alcance.md` (no implementar; documentar los límites del MVP)
