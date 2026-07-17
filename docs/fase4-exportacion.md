# Fase 4 — Exportación a XMP

> Ver también: `database.md` (tabla `global_ratings`, JOIN de herencia semántica), `conventions.md` (backup con `VACUUM INTO`), `config.md` (`min_global_sample`).

## Exportación a XMP (Transaccional y Diferida)

Al finalizar, un comando único (`export-xmp`) compila los resultados desde la BD local y el índice global, y escribe masivamente los archivos `.xmp`:

- **Estrellas (`xmp:Rating`)**: se calculan por **cuantiles sobre el índice global** (`~/.photoranker/global_index.sqlite`, excluyendo `rejected=1`), no sobre un rango fijo de `mu`. Cada imagen cae en el intervalo de su posición relativa (rango porcentual) dentro de la distribución completa de `mu`:

| Rango porcentual (posición dentro de la distribución) | Estrellas |
|---|---|
| 0% – 10% (inferior) | ★ |
| 10% – 35% | ★★ |
| 35% – 75% | ★★★ |
| 75% – 95% | ★★★★ |
| 95% – 100% (superior) | ★★★★★ |

  Esto hace que el significado de "3 estrellas" sea consistente entre distintas carpetas/sesiones, en lugar de depender de un valor absoluto de `mu` que puede exceder el rango teórico 0–50.

  **Mínimo de datos para que el cuantil sea significativo**: si `global_ratings` tiene menos de `min_global_sample = 20` imágenes no rechazadas, no se calculan cuantiles — en su lugar se usa un mapeo provisional directo sobre `mu` (0–10→★, 10–20→★★, 20–30→★★★, 30–40→★★★★, 40–50→★★★★★, con clamping en los extremos). En cuanto se supera el umbral, `export-xmp` cambia automáticamente al modo por cuantiles.

  **Query de referencia** (SQLite soporta `PERCENT_RANK()` como función de ventana desde 3.25+; esta es la consulta exacta que `export-xmp` debe ejecutar contra `global_ratings`, no una reimplementación manual del cálculo). **Nota de sentido**: se ordena `mu ASC` a propósito — `mu` bajo significa que el torneo la clasificó peor, así que el percentil más bajo (0%–10%) corresponde a las imágenes con **peor** `mu`, que reciben 1 estrella. "Inferior" en la tabla de arriba se refiere a posición en la distribución de `mu`, no a "percentil matemáticamente inferior" en algún otro sentido — mu bajo = peor ranking = menos estrellas:

  ```sql
  SELECT file_path, mu,
    CASE
      WHEN PERCENT_RANK() OVER (ORDER BY mu ASC) <= 0.10 THEN 1
      WHEN PERCENT_RANK() OVER (ORDER BY mu ASC) <= 0.35 THEN 2
      WHEN PERCENT_RANK() OVER (ORDER BY mu ASC) <= 0.75 THEN 3
      WHEN PERCENT_RANK() OVER (ORDER BY mu ASC) <= 0.95 THEN 4
      ELSE 5
    END AS stars
  FROM global_ratings
  WHERE rejected = 0;
  ```

- **Etiquetas de color (`xmp:Label`)**: se preservan intactas.
- **Clusters**: se exportan como tags planos (no jerárquicos) en `dc:subject` (ej. "Retratos nocturnos"). El `cluster_id` usado es el argmax de `image_clusters.probability` para esa imagen (ver `fase2-clustering.md`).
- **Descartes**: las imágenes con `rejected = 1` reciben `<xmp:Rating>-1</xmp:Rating>` para que Darktable las muestre como rechazadas. **Herencia de `cluster_id`/`dc:subject`**: se copian desde la imagen representante del burst **solo si esta ya tiene `cluster_id` asignado** (es decir, si se corrió `cluster` antes de exportar). Si la ganadora nunca fue clusterizada (`cluster_id IS NULL`), la rechazada también queda sin cluster/tag — no se genera un tag "vacío" ni se fuerza un cluster. La herencia se resuelve como una copia del `cluster_id` (referencia en BD) al momento de construir el XMP; el `dc:subject` final se deriva de ese `cluster_id` igual que para cualquier otra imagen, no se copia como string suelto. El `JOIN` exacto para resolverla:

  ```sql
  SELECT rejected_img.id AS rejected_image_id, winner.cluster_id
  FROM images AS rejected_img
  JOIN burst_members ON burst_members.image_id = rejected_img.id
  JOIN bursts ON bursts.id = burst_members.burst_id
  JOIN images AS winner ON winner.id = bursts.representative_image_id
  WHERE rejected_img.rejected = 1;
  ```

**Nombre del archivo sidecar (crítico para que Darktable lo reconozca)**: se usa la **convención Darktable**, no la de Adobe — el nombre del `.xmp` es el **nombre completo del archivo original, incluyendo su extensión, más `.xmp`** al final. Ejemplo: para `IMG_1234.CR2`, el sidecar es `IMG_1234.CR2.xmp` (**no** `IMG_1234.xmp`, que es la convención Adobe/Lightroom y que Darktable **no** reconoce). El sidecar se escribe siempre en la misma carpeta que el archivo original. Esta regla debe ir codificada en una sola función auxiliar (`fn xmp_sidecar_path(original: &Path) -> PathBuf`) usada por todo el código de exportación, para que no haya dos lugares que puedan implementarla de forma distinta.

**Política de escritura: merge seguro, no sobrescritura total.** Si ya existe un `.xmp` para la imagen (ej. editado previamente en Darktable/Lightroom, con palabras clave, copyright, u otras etiquetas de color no gestionadas por PhotoRanker), `export-xmp` **lee el archivo existente, preserva íntegramente cualquier namespace/etiqueta que no gestione**, y solo actualiza o inyecta `xmp:Rating` y los `<rdf:li>` dentro de `dc:subject` (agregando el tag del cluster sin borrar tags de `dc:subject` que el usuario haya agregado por fuera de PhotoRanker). Se usa el crate `quick-xml` para este parseo/merge dirigido, en vez de regenerar el XML desde cero. Si el `.xmp` no existe aún, se crea uno nuevo con la estructura mínima de abajo (con el nombre de archivo de la regla anterior).

**Imágenes excluidas de la exportación**: las que tienen `thumbnail_status = 'failed'` **no reciben `.xmp`** — no se les asigna el `mu=25` por defecto disfrazado de rating real, porque nunca participaron en el torneo. Quedan fuera del conteo de "N archivos escritos" hasta que se resuelva su miniatura (`retry-thumbnail`, ver `fase1-ingesta.md`) y participen en al menos una ronda.

**Ejemplo de sidecar `.xmp` generado** (formato estándar Darktable/Adobe XMP, namespaces `x`, `rdf`, `xmp`, `dc`):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmp:Rating="4"
    xmp:Label="Green">
   <dc:subject>
    <rdf:Bag>
     <rdf:li>Retratos nocturnos</rdf:li>
    </rdf:Bag>
   </dc:subject>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
```

Para una imagen rechazada, `xmp:Rating="-1"` y `dc:subject` toma el tag heredado de su representante (si existe).

**Pares RAW+JPEG** (ver `fase1-ingesta.md`, "RAW + JPEG del mismo disparo cuentan como 1 sola foto"): si `images.paired_path` no es `NULL`, se escribe un sidecar `.xmp` para `file_path` **y** otro para `paired_path`, ambos con el mismo rating/label/cluster — así Darktable/Lightroom ven el rating reflejado sin importar cuál de las dos versiones abran. El campo `written` de la salida JSON cuenta archivos `.xmp` escritos, no filas de `images` — un par fusionado suma 2, no 1.

Todo el proceso es no destructivo: nunca se modifica el RAW, solo los sidecars XMP (ver `architecture.md`, Principios del Proyecto).

## Checklist de implementación

- [x] Implementar `export-xmp`: cálculo de cuantiles desde `global_ratings` (tabla de estrellas 10/25/40/20/5; si `global_ratings` tiene menos de `min_global_sample` imágenes, usar el mapeo fijo provisional sobre `mu`), mapeo `rejected=1 → -1`, tags planos en `dc:subject`, siguiendo el formato de sidecar de arriba. **Nombre del sidecar**: `<nombre_completo_original>.xmp` (convención Darktable, ej. `IMG_1234.CR2.xmp`), vía una única función auxiliar reutilizada en todo el módulo. **Excluir de la exportación** las imágenes con `thumbnail_status='failed'` o `missing=1` (ver comando `prune`, `fase1-ingesta.md`). **Merge seguro con `quick-xml`**: si ya existe un `.xmp`, preservar todo lo que no gestiona PhotoRanker y solo actualizar `xmp:Rating`/`dc:subject`.
- [x] Implementar la herencia de `cluster_id`/`dc:subject` desde la ganadora del burst hacia sus rechazadas, **solo si la ganadora tiene `cluster_id` asignado**; si no, la rechazada queda sin cluster.
- [x] Implementar `list-failed-thumbnails` y `retry-thumbnail`.
- [x] Implementar `resync-global --path` para reparar `source_db_path` si una carpeta fue movida/renombrada (ver `database.md` — es cosmético, no crítico).

## Siguiente fase

`fase5-gui.md`
