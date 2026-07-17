# Fase 8 — Mejoras avanzadas (post-MVP, arquitectura real)

> Ver también: `fase6-fuera-de-alcance.md` (de donde salen estos 4 ítems), `fase3-torneo.md` (scope de torneo, vista de `variable-tag`), `conventions.md` ("Modelo de concurrencia").

A diferencia de `fase7-mejoras-post-mvp.md` (envoltura visual pura, sin lógica nueva), los 4 ítems de esta fase sí tocan backend/arquitectura real: comparar datos entre bases de datos distintas, concurrencia real, cómo arma sus pools el torneo, y un modo de interacción nuevo en el TUI. `fase6-fuera-de-alcance.md` los marcaba como permanentemente fuera de alcance; se "liberan" acá porque el usuario del proyecto quiere dejar la puerta abierta a implementarlos más adelante — pero, a diferencia de fase7, **ninguno tiene todavía una dirección de diseño concreta**. Nada de esto se implementa sin definirlo primero (ver CLAUDE.md: "no tomes decisiones de arquitectura por tu cuenta").

## 1. Detección de duplicados entre carpetas/viajes

Hoy cada carpeta tiene su propia `.photoranker.sqlite` totalmente independiente (ver `architecture.md`) — no existe ningún mecanismo que compare imágenes entre carpetas distintas.

**Pendiente de definición**: ¿qué cuenta como "duplicado" (mismo `hash`/pHash exacto vs. similar por debajo de un umbral, como `burst_threshold` pero cross-carpeta)? ¿Se compara contra el índice global (`~/.photoranker/global_index.sqlite`, que hoy solo guarda `mu` por imagen, ver `database.md`) o hace falta agregar el pHash ahí también? ¿Qué hace la GUI con un duplicado detectado — solo lo informa, o permite alguna acción (excluir, fusionar métricas)?

## 2. Acotar el pool de torneo por subcarpeta (`--scope=subfolder`)

El MVP usa siempre la carpeta raíz completa como pool único de `tournament-next` (ver `fase3-torneo.md`). La idea es permitir competir solo dentro de una subcarpeta (ej. comparar fotos de "Día 1" sin mezclarlas con "Día 2" de un mismo viaje).

**Pendiente de definición**: ¿el scope se pasa por flag del CLI en cada llamada (`tournament-next --scope="Día 1"`) o es un modo persistente de la sesión? ¿Cómo interactúa con el índice global (¿los resultados siguen sincronizando `mu` globalmente aunque el pool esté acotado)? ¿La GUI necesita un selector de subcarpeta nuevo, y dónde vive?

## 3. Lock manager propio para concurrencia GUI+CLI simultánea

Hoy la disciplina es "un comando a la vez": la GUI nunca lanza dos subprocesos de escritura en paralelo sobre la misma BD, y no hay lock manager propio — SQLite en modo WAL alcanza (ver `conventions.md`, "Modelo de concurrencia"). Esto se vuelve insuficiente si en algún momento se quiere soportar, por ejemplo, dos ventanas de la GUI abiertas sobre la misma carpeta, o la GUI corriendo mientras alguien usa el CLI a mano sobre la misma BD.

**Pendiente de definición**: ¿el caso de uso real que lo justifica ya apareció (dos instancias de la GUI, GUI+CLI manual, etc.) o es preventivo? ¿El lock es a nivel de archivo (un `.lock` junto al `.sqlite`) o a nivel de fila/tabla? ¿Qué pasa con el proceso que pierde la carrera — espera, falla con un error explícito, o hace polling?

## 4. Vista en grilla (múltiples imágenes a la vez) en `variable-tag`

El modo TUI `variable-tag` es exclusivamente "una imagen a la vez" en el MVP (ver `fase3-torneo.md`). Una vista en grilla permitiría clasificar varias imágenes de un vistazo (útil para variables nominales binarias tipo "¿hay animales?").

**Pendiente de definición**: ¿cuántas imágenes por pantalla, y cómo se decide con el detector automático Kitty/Sixel/ASCII de `ratatui-image` (ver `fase1-ingesta.md`)? ¿La navegación/asignación por teclado cambia (hoy es número = valor, `Espacio` = saltar) o se mantiene igual pero repetida por celda? ¿Esto es solo para el TUI, o también aplicaría al clasificador de la GUI (`Variables.tsx`, tab "Clasificar")?

## Checklist de implementación

- [ ] Definir alcance de detección de duplicados entre carpetas y luego implementarlo.
- [ ] Definir el mecanismo de `--scope=subfolder` para el torneo y luego implementarlo.
- [ ] Definir si hace falta un lock manager propio (y de qué tipo) y luego implementarlo.
- [ ] Definir la interacción de la vista en grilla de `variable-tag` y luego implementarla.

## Siguiente fase

Ninguna planeada todavía.
