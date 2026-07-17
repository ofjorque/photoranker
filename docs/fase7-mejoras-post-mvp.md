# Fase 7 — Mejoras post-MVP de la GUI

> Ver también: `fase5-gui.md` (la GUI que esta fase extiende), `fase1-ingesta.md` (bursts), `fase2-clustering.md` (clusters), `cli-reference.md` (catálogo de comandos), `conventions.md` ("Definición de MVP").

El MVP (fases 0–5) está funcionalmente completo. Esta fase recoge mejoras de UX/GUI identificadas con la app ya en uso real, priorizadas sobre `fase6-fuera-de-alcance.md` porque **no** contradicen ningún no-objetivo documentado ahí — a diferencia de esa fase, esto **sí** es implementable, no solo para documentar.

Regla general heredada de `fase5-gui.md`: la GUI es la envoltura visual del CLI, no agrega lógica de negocio nueva. Los ítems que si necesitan un comando CLI nuevo lo dicen explícitamente (comandos de solo lectura, mismo patrón que `get-variable-values`/`list-clusters`, agregados en su momento "tras una segunda ronda de feedback").

## 1. Menú contextual

Agregar menú contextual (click derecho) con componente `context-menu` de shadcn/ui, en 4 superficies (definido en conjunto con el usuario):

- **Torneo**: tarjetas de `RankingBoard` (torneo principal y minitorneo de ráfagas).
- **Ráfagas**: miniaturas del panel de exclusión ("¿alguna de estas fotos no es parte de la ráfaga?").
- **Clusters**: imágenes representativas de `ClusterView` y las del Sheet "ver todas".
- **Exportar**: filas de la tabla de ranking en vivo de `ExportView`.

**Sin acciones nuevas**: el menú contextual solo replica las acciones que ya existen como botón en cada superficie (ej. "ver detalles" vía `PhotoDetailsDrawer`, "excluir" en Ráfagas) — decisión explícita para no agregar lógica nueva, solo un atajo de interacción. No agrega "copiar ruta" ni "abrir carpeta contenedora" (esa última hubiera requerido un comando Tauri nuevo).

## 2. Ver todas las imágenes de un cluster + representatividad

`list-clusters` (ver `cli-reference.md`) solo devuelve hasta 4 imágenes representativas por cluster (`representative_images`, mayor `probability`). La tabla `image_clusters` ya guarda `probability` para **todas** las imágenes del cluster, no solo las representativas — falta exponerlas.

**Nuevo comando CLI de solo lectura** (mismo espíritu que `get-variable-values`, no toca `mu`/`sigma`/`rejected`/`cluster_id`):

```
photoranker list-cluster-images --id <cluster_id>
```

`data = [{"id": <image_id>, "file_path": "...", "probability": <f64>}, ...]` — todas las imágenes del cluster (no solo top-4), orden `probability` descendente. `CLUSTER_NOT_FOUND` si el id no existe.

En la GUI, al seleccionar un cluster en `ClusterView` (tab "list"), mostrar la lista completa (ej. en un `Drawer`/`Sheet`) con la probabilidad de cada imagen visible — no solo las 4 representativas actuales.

## 3. Más tabs en la carga del proyecto

**Definido en conjunto**: reorganizar `HomeView.tsx` (hoy una sola página larga con Cards apiladas) en `Tabs`, mismo contenido, sin lógica nueva:

- **"Ingesta"**: Card de carpeta + `init` (input de ruta, botón "elegir carpeta", botón `init`) y Card de acciones (`prune`, `burst-detect`, ir a Ráfagas, ir a Torneo) — lo que ya existe arriba de la zona peligrosa hoy.
- **"Mantenimiento"**: el Card de resultado de la última acción (`result`, JSON crudo) — hoy fijo debajo de las acciones.
- **"Zona peligrosa"**: el Card `border-destructive/50` de deshacer/reiniciar torneo, y el Card de índice global (`resync-global`/`reset-global-index`) — ambos ya visualmente separados como "zona peligrosa" hoy, solo pasan a vivir en su propio tab en vez de siempre visibles.

## 4. Ráfagas de 2 imágenes: "esto no es una ráfaga"

**Ya soportado por el CLI, falta solo en la GUI.** `burst-exclude` (ver `fase1-ingesta.md`, "Excluir/deshacer bursts") ya disuelve el burst completo si tras excluir quedan 1 o 0 miembros — no hace falta ningún comando nuevo.

El bloqueo es puramente de `gui/src/views/Bursts.tsx`: el panel de exclusión hoy está condicionado a `pendingBurst.images.length > 2`, así que un burst de exactamente 2 imágenes nunca lo muestra. Cambio: agregar una acción explícita ("Esto no es una ráfaga") visible también cuando `images.length === 2`, que llame `burst-exclude` con ambos `image_id` — el CLI ya la disuelve sin que la GUI tenga que decidir nada especial para el caso de 2.

## 5. UX de variables nominales

El formulario actual (`VariableBuilder.tsx`) arma categorías nominales como texto libre `"etiqueta:código,etiqueta:código"` (ej. `"No:0,Sí:1"`) — feedback de uso real: se siente arcaico.

**Definido en conjunto**: lista editable de chips, uno por categoría — etiqueta editable + código autogenerado (0, 1, 2… en el orden de la lista, no editable a mano para evitar duplicados/huecos), botón "+ agregar categoría", reordenable por drag (mismo `@dnd-kit` que ya usa el componente para los bloques ordinal/nominal), tacho para borrar cada fila. Al confirmar, se serializa internamente al mismo formato `"etiqueta:código,..."` que ya espera `cli.variableCreate` — el contrato con el CLI no cambia, es puramente la interacción de armado.

```
┌─ Categorías ───────────────────┐
│ [≡] No           código 0  [x]│
│ [≡] Sí           código 1  [x]│
│                                │
│      + Agregar categoría      │
└────────────────────────────────┘
```

## 6. Crear varias variables custom de una sola pasada

Sin cambios de CLI: `variable-create` sigue siendo de a una variable, pero `VariableBuilder.tsx` puede dejar que el usuario arme varias definiciones en la UI (cola local) y dispare `cli.variableCreate` en secuencia al confirmar — orquestación pura del lado GUI, mismo patrón que ya usa `runCliAction` en otras vistas para encadenar llamadas.

## 7. Listado de fotos × todas sus variables (tarjetas)

Sin comando nuevo, para la cantidad típica de variables custom de una carpeta: `variable-list` ya da todas las variables definidas, y se puede llamar `get-variable-values` una vez por variable y cruzar los resultados por `image_id` en la GUI (tabla/grilla de tarjetas: foto + valor de cada variable). Si en la práctica el número de variables custom crece mucho y esto se vuelve lento (muchas llamadas secuenciales), ahí sí valdría un comando bulk dedicado — no se resuelve preventivamente en el MVP de esta fase.

## Fuera de alcance de esta fase

- **Asignación por lotes de variables en la GUI** (tab "batch" de `Variables.tsx`, wrapper de `variable-set` vía texto `id:valor` pegado): feedback de uso real indica que no se entiende como interacción. Se retira **solo de la GUI** — `variable-set` sigue disponible y sin cambios en el CLI para quien lo use por línea de comandos.

## Checklist de implementación

- [x] Implementar menú contextual (`context-menu` de shadcn/ui) en Torneo, Ráfagas, Clusters y Exportar, replicando solo acciones ya existentes.
- [x] Implementar `list-cluster-images --id <N>` en el CLI (comando de solo lectura) y consumirlo desde `ClusterView`.
- [x] Reorganizar `HomeView.tsx` en tabs ("Ingesta" / "Mantenimiento" / "Zona peligrosa"), mismo contenido.
- [x] Habilitar la acción "esto no es una ráfaga" en `Bursts.tsx` para bursts de 2 imágenes (llama `burst-exclude` existente, sin cambios de CLI).
- [x] Implementar la lista editable de chips para categorías nominales en `VariableBuilder.tsx` (drag para reordenar, código autogenerado).
- [x] Permitir encolar y crear varias variables custom en una sola pasada desde `VariableBuilder.tsx`.
- [x] Implementar vista de tarjetas "fotos × variables" cruzando `variable-list` + `get-variable-values`.
- [x] Quitar el tab de asignación por lotes de `Variables.tsx` (GUI únicamente).

## Siguiente fase

`fase8-mejoras-avanzadas.md` — a diferencia de esta fase (envoltura visual pura), toca backend/arquitectura real; los 4 ítems que trae vienen liberados de `fase6-fuera-de-alcance.md` y todavía no tienen dirección de diseño definida.
