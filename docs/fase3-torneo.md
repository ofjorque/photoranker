# Fase 3 — Torneo Principal

> Ver también: `database.md` (tablas `images`, `pending_tournament_groups`, `tournament_matches`, `global_ratings`), `conventions.md` (formato JSON, códigos de error), `config.md` (`sigma_stop_threshold`, `convergence_fraction`, `stall_rounds`, `max_rounds_multiplier`, `global_sync_every`, `trueskill_beta`).

## Torneos Jerárquicos (TrueSkill vía `skillratings`)

**Enmienda al spec v1.0 (feedback de uso real, post-Fase 5)**: esta sección
describía originalmente Weng-Lin. Probando con carpetas de pocas imágenes
(ej. 6 fotos) se detectó que la sesión de torneo nunca converge en la
práctica: con `sigma` inicial `8.33` y umbral `sigma_stop_threshold=2.0`, una
simulación de 60 rondas mostró que Weng-Lin solo baja `sigma` a ~3.0–3.6, muy
lejos del umbral, mientras que `max_rounds = max_rounds_multiplier ×
imágenes_activas` da un tope de apenas 18 rondas para 6 fotos — la sesión
queda "atascada" mostrando 0% de convergencia indefinidamente. Se evaluaron
las alternativas del crate `skillratings`: Elo/Glicko-2/Glicko-Boost/etc. son
por pares (no soportan un grupo de N imágenes con empates en una sola
llamada, como requiere la UX actual); **TrueSkill** es la única alternativa
que expone una función multi-equipo nativa (`trueskill_multi_team`) con la
misma forma de entrada/salida. Se migró a TrueSkill, con la advertencia
explícita (confirmada empíricamente antes de dar el cambio por bueno) de que
al ser también bayesiano no estaba garantizado que resolviera la convergencia
lenta por sí solo — la misma simulación de 60 rondas con TrueSkill mostró
`sigma` bajando a ~1.1–1.9 (cruzando el umbral de 2.0 individualmente ya
desde la ronda ~15–20, aunque el 95% de convergencia simultánea con solo 6
imágenes activas sigue necesitando más de las 18 rondas del tope por
defecto) — una mejora real, aunque no una solución completa por sí sola para
carpetas muy chicas. **Nota**: TrueSkill está patentado (ver la documentación
del propio crate, `skillratings::trueskill`), que recomienda evitarlo en
proyectos comerciales — PhotoRanker es de uso personal, pero queda señalado
por si ese contexto cambia.

El sistema organiza torneos inteligentes dentro de la estructura de carpetas:

1. **Inicialización**: todas las imágenes comienzan con `mu = 25.0`, `sigma = 8.33` (valores por defecto de `TrueSkillRating::new()`, que coinciden con los que ya usaba `WengLinRating::new()`).
2. **Selección del grupo** (`tournament-next`, identificado por un `group_id` nuevo generado como UUID v4 en cada llamada, devuelto en la respuesta JSON y que el cliente debe reenviar tal cual en `tournament-result`): se ordenan las imágenes candidatas (excluyendo `rejected=1`, `stalled=1` y `missing=1`) con la cláusula exacta `ORDER BY sigma DESC, CASE WHEN last_compared_at IS NULL THEN 0 ELSE 1 END ASC, last_compared_at ASC` — es decir, primero `sigma` descendente, y como desempate, las imágenes que **nunca** han participado (`last_compared_at IS NULL`) tienen máxima prioridad, seguidas por las que llevan más tiempo sin participar (`last_compared_at` ascendente). Este orden explícito evita que la IA implemente el `ORDER BY ... ASC` de forma ambigua (en SQLite, `NULL` ya ordena primero en `ASC` por defecto, pero se especifica el `CASE` igual para que el comportamiento no dependa de una convención implícita del motor). Para armar cada grupo de 5:
   - Se toma la primera imagen no agrupada de la cola de prioridad como semilla.
   - Sobre un array de imágenes ordenado por `mu` (búsqueda binaria), se buscan las 3 más cercanas a la semilla con diferencia absoluta de `mu` ≤ 5.
   - Si no se completan 4 imágenes (semilla + 3), se relaja el umbral en pasos de +2 hasta lograrlo.
   - Se añade una 5ª imagen aleatoria cuyo `mu` difiera en al menos 10 puntos del promedio del grupo. **Fallback**: si no existe ninguna imagen activa restante que cumpla esa diferencia de 10 puntos (torneo ya casi convergido, con `mu`s muy parejos), se relaja también esta condición — se toma cualquier imagen activa restante sin la restricción de diferencia mínima, en vez de dejar el grupo con 4. Esta condición es un objetivo de calibración, no un requisito estricto que pueda bloquear la formación del grupo.
   - **Nota de implementación**: esto es agrupación en 1 dimensión (solo `mu`), por lo que basta con ordenar + búsqueda binaria — no se requiere k-means ni ningún algoritmo de clustering iterativo para este paso.
   - **Grupos incompletos**: el tamaño del grupo es **dinámico, de 2 a 5 imágenes** — `trueskill_multi_team` soporta cualquier número de equipos. Si la carpeta tiene menos de 5 imágenes activas disponibles para completar el grupo, se forma con las que haya (mínimo 2; con solo 1 imagen activa restante no hay torneo posible y `tournament-next` devuelve `status="ok"` con `data=null` indicando que no hay grupo que formar). **Nunca se duplica un `image_id` dentro del mismo `rating_groups`** — repetir la misma imagen como si fuera dos equipos distintos corrompería el cálculo de `trueskill_multi_team` (compara una imagen contra sí misma).
   - Al confirmar el resultado (`tournament-result`), se actualiza `last_compared_at = CURRENT_TIMESTAMP` (SQLite; **no** existe una función `now()`) para las imágenes del grupo.
   - El grupo se registra en `pending_tournament_groups` (ver `database.md`) para que `tournament-result` pueda validarlo después.
3. **Comparación (por teclado)**: el usuario ordena las imágenes de mejor a peor usando las teclas `1`–`5`, permitiendo empates (ver "Interacción por Teclado" abajo).
4. **Actualización**: se llama a `trueskill_multi_team` del crate `skillratings`, tratando cada foto como un equipo de un solo integrante, usando el `beta` leído desde `config.toml` (nunca el default hardcodeado del crate):

```rust
use skillratings::{
    trueskill::{trueskill_multi_team, TrueSkillConfig, TrueSkillRating},
    MultiTeamOutcome,
};

// user ranking con empates: [(image_id, rank_position)]
// ej: [(42,1), (17,1), (58,2), (3,3), (99,4)]

let team_42 = vec![TrueSkillRating { rating: mu_42, uncertainty: sigma_42 }];
let team_17 = vec![TrueSkillRating { rating: mu_17, uncertainty: sigma_17 }];
// ...

let rating_groups = vec![
    (&team_42[..], MultiTeamOutcome::new(1)),
    (&team_17[..], MultiTeamOutcome::new(1)), // empate en 1°
    (&team_58[..], MultiTeamOutcome::new(2)),
    (&team_3[..],  MultiTeamOutcome::new(3)),
    (&team_99[..], MultiTeamOutcome::new(4)),
];

// El beta se lee de config.toml, NO se usa TrueSkillConfig::new() (que traería
// el default del crate e ignoraría lo configurado por el usuario).
let config = TrueSkillConfig { beta: settings.trueskill_beta, ..Default::default() };
// `weights=None`: cada equipo es siempre una sola imagen, peso implícito 1.0
// parejo — el único caso en que devuelve Err() es con pesos explícitos mal
// formados, que este proyecto no usa.
let updated = trueskill_multi_team(&rating_groups, &config, None)?;
```

   Una sola llamada actualiza `mu`/`sigma` de las imágenes del grupo. No se descompone en enfrentamientos por pares.

   **Valores por defecto de `skillratings::TrueSkillConfig`** (documentados aquí para que quede explícito qué se hereda si no se toca `config.toml`): `beta ≈ 4.1667` (25/6 — mismo valor que Weng-Lin usaba, controla cuánta diferencia de `mu` se necesita para predecir ~80% de probabilidad de ganar), `draw_probability = 0.1` (probabilidad de empate asumida) y `dynamics_factor ≈ 0.0833` (25/300 — factor aditivo que permite que la incertidumbre no baje a cero indefinidamente). El resto de los campos de `TrueSkillConfig` se dejan en su default (`..Default::default()`); solo `beta` se expone como configurable en el MVP, igual que antes con Weng-Lin.

5. **Criterio de parada**: se detiene cuando ocurre cualquiera de estos casos:
   - **Convergencia**: al menos el `convergence_fraction` (default `0.95`) de las imágenes activas tiene `sigma < sigma_stop_threshold` (default `2.0`). Se usa 95% y no 100% a propósito: exigir que *todas* converjan es frágil si 1-2 imágenes quedan sistemáticamente como "la 5ª aleatoria" y su `sigma` baja muy lento — el mecanismo de `stalled` ya cubre esos casos individuales, pero el 95% agrega un colchón adicional para que la sesión no dependa de las últimas imágenes más tercas.
   - **Estancamiento**: si el `sigma` de una imagen no baja más de un 5% en `stall_rounds` rondas consecutivas (default `20`), se marca `stalled = 1` y se **excluye del pool activo** de selección de grupos, pero conserva su `mu`/`sigma` actual y sigue apareciendo en el ranking final.
   - **Timeout de rondas**: el máximo de rondas por sesión se calcula como `max_rounds_multiplier × número_de_imágenes_activas` (default del multiplicador: `3`; ej. con 2.400 fotos activas, tope de 7.200 rondas), no un valor fijo — así el límite escala con el tamaño de la carpeta. Al alcanzarlo, se detiene y se reporta cuántas imágenes seguían activas/estancadas.
   - Detención manual del usuario en cualquier momento.

   El comando `tournament-status` reporta el motivo de parada (`converged`/`stalled`/`timeout`/`manual`) y el detalle por imagen.
6. **Sincronización con el índice global (en lotes, no por grupo)**: para evitar abrir/escribir/cerrar `~/.photoranker/global_index.sqlite` en cada resultado de grupo (lo que generaría contención si la GUI lo lee al mismo tiempo para mostrar el ranking en vivo), los resultados `(image_id, mu, rejected)` se acumulan en una cola en memoria y se hace un **upsert por lote** cada `global_sync_every` resultados de grupo (default `10`) o al finalizar la sesión de torneo (lo que ocurra primero). Cualquier comando que **lea** del índice global (`ranking`, `export-xmp`, `tournament-status`) primero fuerza un flush de la cola pendiente, para no mostrar datos desactualizados.

## Interacción por Teclado

Toda la selección de rankings (torneo principal y minitorneo de ráfagas, ver `fase1-ingesta.md`) es navegable sin mouse:

- **Navegación**: flechas o `Tab` mueven el foco entre las miniaturas mostradas (borde resaltado indica foco).
- **Asignar posición**: con una imagen en foco, presionar `1`–`5` (o `1`–`N` en ráfagas de tamaño variable) le asigna esa posición de ranking.
  - Si la posición ya está ocupada por otra imagen, ambas quedan **empatadas** en esa posición.
  - Cada imagen muestra su número asignado (o "empate en 2°" si comparte posición) como feedback inmediato.
- **Confirmar grupo**: `Enter` envía el resultado del grupo. **Está bloqueado hasta que todas las imágenes tengan una posición asignada** — si faltan, se muestra un mensaje como "Faltan 2 imágenes por ordenar" y no se envía nada.
- **Deshacer**: `Backspace` o `R` reinicia las posiciones asignadas del grupo actual antes de confirmar.

**Etiquetado masivo de variables subjetivas (`variable-tag`, modo TUI)** — ver `fase1-ingesta.md` para el contexto completo:

Asignar variables subjetivas a miles de fotos requiere verlas. El CLI incluye un **modo TUI** que renderiza la `images.thumbnail` normalizada en la terminal usando el crate `ratatui-image` (que ya encapsula la detección automática del backend, sin que el CLI tenga que implementar los protocolos a mano), con esta prioridad fija: **protocolo gráfico Kitty** → **Sixel** → **fallback ASCII/halfblock** (siempre funciona). La detección es automática al iniciar `variable-tag`, sin flag manual. La miniatura se renderiza ocupando como máximo el 60% del alto de la terminal, preservando aspect ratio.

- `photoranker variable-tag --variable "Grado de nostalgia"` recorre las imágenes **una por una** (modo "slideshow", no grilla — decisión del MVP: más simple de implementar y de testear, sin bugs de layout multi-imagen en el TUI. Una vista en grilla queda fuera del MVP, ver `fase6-fuera-de-alcance.md`).
- Para variables **ordinales**: presionar el número directo asigna el valor y avanza automáticamente.
- Para variables **nominales**: presionar el número del `code` de la categoría asigna y avanza.
- `Espacio` salta la imagen sin asignar (queda `NULL`, y `clustMD` la trata como faltante, ver `fase2-clustering.md`).
- `Backspace` retrocede a la imagen anterior para corregir.
- `Q` sale guardando el progreso (se puede reanudar después).

## Checklist de implementación

- [x] **Verificar antes de construir nada más**: escribir un test unitario aislado que llame `trueskill_multi_team` (migrado desde `weng_lin_multi_team`, ver nota de enmienda arriba) con un `rating_groups` donde dos equipos comparten el mismo `MultiTeamOutcome` (ej. ambos con rango 1) y confirmar que el crate `skillratings` los trata como empate real (mismo `mu`/`sigma` resultante para ambos, o al menos un tratamiento simétrico) — no asumir este comportamiento sin probarlo, ya que toda la UX de empates por teclado depende de que esto funcione así. *(`trueskill_multi_team_treats_ties_symmetrically`, en `commands/tournament.rs`.)*
- [x] Implementar `tournament-next`: generar `group_id` (UUID v4), seleccionar grupo de hasta 5 por `sigma` descendente + `last_compared_at` ascendente (desempate) + `mu` similar + 1 aleatoria (excluyendo `rejected=1`, `stalled=1` y `missing=1`); tamaño dinámico 2-5 si no hay suficientes imágenes disponibles, **sin duplicar `image_id`** dentro del mismo grupo. Registrar el grupo en `pending_tournament_groups`.
- [x] Implementar detección de estancamiento: marcar `stalled=1` si `sigma` no baja >5% en `stall_rounds` rondas, y timeout por `max_rounds_multiplier × imágenes activas`. Implementar criterio de convergencia por `convergence_fraction` (95%, no 100%).
- [x] Implementar la cola en memoria de sincronización al índice global: upsert por lote cada `global_sync_every` resultados (o al finalizar sesión), con flush forzado antes de cualquier lectura (`ranking`, `tournament-status`, `export-xmp`).
- [x] Implementar `tournament-result --group-id --ranking id:posición ...` usando `skillratings::trueskill_multi_team` con `beta` inyectado desde `config.toml`; actualizar `last_compared_at` de las imágenes del grupo. **Validación estricta antes de calcular** (si falla cualquiera, devolver `status="error"`, `code="INVALID_RANKING"`, sin tocar la BD): (a) el `group_id` existe y sigue pendiente (no fue ya resuelto antes); (b) el conjunto de `image_id` en `--ranking` coincide **exactamente** (mismo conjunto, sin faltantes ni sobrantes) con las imágenes que `tournament-next` generó para ese `group_id`; (c) las posiciones son enteros contiguos empezando en 1, permitiendo empates (ej. `1,1,2,3,4` es válido; `1,2,4,5` no, porque salta el 3).
- [x] Registrar cada resultado en `tournament_matches` (log/auditoría).
- [x] Implementar sincronización por lotes (con reintento ante `SQLITE_BUSY`) hacia `global_ratings` cada `global_sync_every` resultados, no por resultado individual.
- [x] Implementar `ranking` (cálculo en vivo por `mu` descendente, desempate por `sigma` ascendente y luego `image_id`).
- [x] Implementar `tournament-status`: devolver progreso del torneo (nº de imágenes con `sigma` sobre el umbral, % de convergencia, y si ya se cumplió el criterio de parada).

## Deshacer / reiniciar (agregado en Fase 5 por feedback de uso real)

Probando la GUI contra una biblioteca real apareció la necesidad de corregir errores humanos sin perder todo el progreso — no estaba cubierto por el diseño original de Fase 3:

- `photoranker tournament-undo [--db]`: revierte el **grupo resuelto más reciente que todavía no se haya deshecho** (`mu`/`sigma`/`stall_counter`/`stalled`/`last_compared_at` vuelven al valor que tenían justo antes de ese grupo). `tournament-result` ahora guarda ese estado "antes" en `tournament_matches` (migración `008_tournament_undo.sql`) precisamente para poder revertir sin tener que invertir la fórmula de `trueskill_multi_team` (no es invertible en general). Cada fila deshecha se marca `undone=1`, así que deshacer dos veces seguidas sin un nuevo resultado de por medio devuelve `NOTHING_TO_UNDO`. No toca `rejected` (el torneo principal nunca lo modifica). Si el grupo ya se había sincronizado al índice global, ese `mu` queda desactualizado hasta la próxima vez que la imagen participe — límite aceptado, igual que `resync-global`.
- `photoranker tournament-reset [--db]`: reinicia **todo** el torneo principal de una carpeta — todas las imágenes con `missing=0` vuelven a `mu=25.0`/`sigma=8.33`, se limpia `stalled`/`stall_counter`/`last_compared_at`. **No toca `rejected`**: las decisiones de `burst-tournament` se conservan a propósito. `tournament_matches`/`pending_tournament_groups` se conservan como auditoría histórica, no se borran.
- `photoranker reset-global-index`: vacía por completo `~/.photoranker/global_index.sqlite` (**todas** las carpetas del usuario, no solo una — acción destructiva y explícita, distinta de `tournament-reset`). Los cuantiles de estrellas (`fase4-exportacion.md`) vuelven al modo `fixed_provisional` hasta que suficientes imágenes vuelvan a sincronizarse.

**Nota de aislamiento para tests**: como el índice global es un archivo único compartido entre todas las carpetas de un usuario real (`~/.photoranker/global_index.sqlite`), los tests de integración deben fijar la variable de entorno `PHOTORANKER_HOME` (ver `config.rs`) a un directorio temporal antes de invocar el CLI — de lo contrario `cargo test` termina leyendo/escribiendo/vaciando el índice global real de quien lo corre. Ver `test_home()` en `core-cli/tests/*.rs`.

## Siguiente fase

`fase4-exportacion.md`
