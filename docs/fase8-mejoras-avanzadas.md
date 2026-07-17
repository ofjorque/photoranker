# Fase 8 — Mejoras avanzadas (post-MVP, arquitectura real)

> Ver también: `fase6-fuera-de-alcance.md` (de donde salen estos 4 ítems), `fase3-torneo.md` (scope de torneo), `fase7-mejoras-post-mvp.md` (clasificador de `Variables.tsx` que gana la vista en grilla), `database.md` (índice global), `conventions.md` ("Modelo de concurrencia").

A diferencia de `fase7-mejoras-post-mvp.md` (envoltura visual pura, sin lógica nueva), los 4 ítems de esta fase sí tocan backend/arquitectura real: comparar datos entre bases de datos distintas, concurrencia real, y cómo arma sus pools el torneo. `fase6-fuera-de-alcance.md` los marcaba como permanentemente fuera de alcance; se "liberan" acá porque el usuario del proyecto quiere dejar la puerta abierta a implementarlos más adelante. Las 4 direcciones de diseño de abajo ya están definidas en conjunto con el usuario — falta implementarlas.

## 1. Detección de duplicados entre carpetas/viajes

Hoy cada carpeta tiene su propia `.photoranker.sqlite` totalmente independiente (ver `architecture.md`) — no existe ningún mecanismo que compare imágenes entre carpetas distintas.

**Definido en conjunto**:
- **Qué cuenta como duplicado**: ambos criterios — (a) `hash` idéntico (copia exacta del mismo archivo en dos carpetas) y (b) pHash bajo un umbral (mismo mecanismo que `burst_threshold`, pero cross-carpeta — detecta el mismo disparo reexportado o recortado, no solo copias byte-a-byte).
- **Dónde se compara**: se extiende `global_ratings` (índice global, `database.md`) con columnas `hash` y `phash`, llenadas en el mismo momento en que ya se sincroniza `mu` (ver "Sincronización con el índice global" en `fase3-torneo.md`) — así la comparación es una sola query contra una tabla que ya integra todas las carpetas, sin abrir cada `.photoranker.sqlite` local ni depender de `source_db_path` (que es solo informativo y puede estar desactualizado).
- **Qué hace la GUI**: solo informa — un aviso/badge "posible duplicado en `<carpeta>`" en la carpeta donde aparece. Nada de exclusión ni fusión automática de métricas; el usuario decide manualmente en cada carpeta.

## 2. Acotar el pool de torneo por subcarpeta (`--scope=subfolder`)

El MVP usa siempre la carpeta raíz completa como pool único de `tournament-next` (ver `fase3-torneo.md`). La idea es permitir competir solo dentro de una subcarpeta (ej. comparar fotos de "Día 1" sin mezclarlas con "Día 2" de un mismo viaje).

**Definido en conjunto**:
- **Mecanismo**: ambos — el CLI expone `tournament-next --scope="Día 1"` (uso manual, explícito por llamada) y la GUI setea ese mismo flag automáticamente según lo que el usuario eligió en su selector.
- **Selector en la GUI**: vive en `TournamentView`, como un dropdown junto a las estadísticas de progreso — aplica de inmediato al próximo `tournament-next` (no persiste entre sesiones, ni se guarda en `config.toml`).
- **Índice global**: sin cambios — los resultados siguen sincronizando `mu` globalmente igual que hoy; el scope solo acota qué imágenes entran al pool local de comparación, no qué se sincroniza.

## 3. Lock manager propio para concurrencia GUI+CLI simultánea

Hoy la disciplina es "un comando a la vez": la GUI nunca lanza dos subprocesos de escritura en paralelo sobre la misma BD, y no hay lock manager propio — SQLite en modo WAL alcanza (ver `conventions.md`, "Modelo de concurrencia"). Esto se vuelve insuficiente si en algún momento se quiere soportar, por ejemplo, dos ventanas de la GUI abiertas sobre la misma carpeta, o la GUI corriendo mientras alguien usa el CLI a mano sobre la misma BD.

**Definido en conjunto**: es preventivo (no hay un incidente real todavía) — diseño de archivo `<carpeta>/.photoranker.lock` junto al `.sqlite`, adquirido por cualquier comando de escritura al arrancar. Si ya existe un lock vigente de otro proceso, el segundo **espera con reintento/backoff corto** (mismo espíritu que `busy_timeout` de SQLite) en vez de fallar de inmediato — transparente para el usuario, solo agrega una espera breve. Queda para el momento de implementación: el timeout máximo de esa espera y qué código de error explícito devuelve si se agota (no debe colgarse indefinidamente si el proceso dueño del lock murió sin liberarlo).

## 4. Vista en grilla (múltiples imágenes a la vez) al clasificar variables

**Definido en conjunto**: solo en la GUI — el clasificador de `Variables.tsx` (tab "Clasificar", hoy una imagen a la vez con navegación por teclado) gana una vista en grilla alternativa. El modo TUI `variable-tag` **se mantiene sin cambios**, exclusivamente "una por una" (a diferencia de lo que decía la versión anterior de este documento, que lo planteaba al revés).

Dirección propuesta para la implementación (ajustable, no bloqueante para arrancar): grilla responsiva similar a `RankingBoard` (`repeat(auto-fit, minmax(...))`), navegación por click/flechas para enfocar una celda y las mismas teclas numéricas ya usadas en el modo uno-a-uno para asignar el valor a la celda enfocada — mismo modelo mental, distinta densidad visual.

## Checklist de implementación

- [ ] Agregar `hash`/`phash` a `global_ratings` (índice global) y llenarlos en la sincronización existente.
- [ ] Implementar detección de duplicados (exacto + pHash con umbral) y el aviso de solo-lectura en la GUI.
- [ ] Implementar `tournament-next --scope=<subfolder>` en el CLI.
- [ ] Implementar el selector de scope en `TournamentView` (GUI).
- [ ] Implementar el lock de archivo (`.photoranker.lock`) con espera/backoff en las escrituras del CLI.
- [ ] Implementar la vista en grilla en el clasificador de `Variables.tsx` (GUI).

## Siguiente fase

Ninguna planeada todavía.
