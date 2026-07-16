# 💻 Referencia CLI Completa

> Documento de referencia — catálogo de todos los comandos, agrupados por la fase donde se implementan. Ver también `conventions.md` (formato JSON, kebab-case), cada `faseN-*.md` para el detalle de comportamiento de sus comandos.

Todos los comandos (excepto `init`) buscan automáticamente `.photoranker.sqlite` en el directorio actual (o hacia arriba), salvo que se pase `--db <ruta>` explícito. Todos los comandos devuelven JSON estructurado (ver `conventions.md`).

```bash
# --- Fase 1: Ingesta y ráfagas (ver fase1-ingesta.md) ---

# Escanear carpeta y extraer miniaturas + pHash
photoranker init --path "C:\Fotos\Boda_Juan"

# Marcar como 'missing' las fotos que ya no existen en disco (borradas/renombradas)
photoranker prune

# Detectar ráfagas (basado en pHash, distancia normalizada 0-1)
photoranker burst-detect --threshold 0.10

# Listar los bursts pendientes de minitorneo, con sus imágenes miembro
photoranker list-bursts

# Crear una variable personalizada nominal con sus categorías
photoranker variable-create --name "Presencia de animales" --type nominal --categories "No:0,Sí:1"

# Crear una variable personalizada ordinal (slider)
photoranker variable-create --name "Grado de nostalgia" --type ordinal --min 1 --max 5

# Ver variables personalizadas definidas
photoranker variable-list

# Asignar valores de una variable a imágenes (formato id:valor)
photoranker variable-set --variable "Grado de nostalgia" --values 42:4 17:2 58:5

# Modo TUI: asignar una variable recorriendo las imágenes por teclado (ver miniaturas)
photoranker variable-tag --variable "Grado de nostalgia"

# Ver el valor actual de una variable por imagen activa (para edición visual en la GUI)
photoranker get-variable-values --variable "Grado de nostalgia"

# Minitorneo de ráfaga (formato id:posición, permite empates)
photoranker burst-tournament --burst-id 1 --ranking 12:1 8:2 4:3

# --- Fase 2: Clustering (ver fase2-clustering.md) ---

# Ver el BIC por número de clusters, sin comprometer resultados
photoranker cluster --preview

# Comprometer clustering con un número de clusters elegido
photoranker cluster --k 4

# Renombrar un cluster antes de exportarlo como tag
photoranker cluster-rename --id 3 --name "Retratos nocturnos"

# Listar clusters comprometidos con sus fotos más representativas (mayor probability)
photoranker list-clusters

# --- Fase 3: Torneo principal (ver fase3-torneo.md) ---

# Solicitar el siguiente grupo de imágenes para comparar
photoranker tournament-next

# Enviar el ranking del usuario (formato id:posición, permite empates)
photoranker tournament-result --group-id abc123 --ranking 42:1 17:1 58:2 3:3 99:4

# Ver el ranking actual con puntuaciones y estrellas
# (calculado en vivo ordenando por mu descendente; empates en mu se
# desempatan por sigma ascendente y luego por image_id, para ser determinista.
# rank_order en la BD es solo un snapshot que escribe export-xmp, no lo lee este comando)
photoranker ranking

# Ver el progreso del torneo (cuántas imágenes siguen sobre el umbral de sigma)
photoranker tournament-status

# Deshacer el último grupo de torneo enviado (mu/sigma vuelven al valor previo)
photoranker tournament-undo

# Reiniciar el torneo principal de esta carpeta (mu/sigma a default, no toca rejected)
photoranker tournament-reset

# Vaciar por completo el índice global (todas las carpetas, acción destructiva)
photoranker reset-global-index

# --- Fase 4: Exportación (ver fase4-exportacion.md) ---

# Listar imágenes excluidas por falla de miniatura
photoranker list-failed-thumbnails

# Reintentar extracción de miniatura de una imagen
photoranker retry-thumbnail --image-id 123

# Exportar resultados a XMP
photoranker export-xmp --db "C:\Fotos\Boda_Juan\.photoranker.sqlite"

# Reparar source_db_path si una carpeta fue movida/renombrada (cosmético, ver database.md)
photoranker resync-global --path "D:\Fotos\Boda_Juan"

# --- Fase 5: GUI (ver fase5-gui.md) ---

# Miniatura normalizada en base64 (único punto por el que la GUI recibe bytes de imagen)
photoranker get-thumbnail --image-id 42

# Métricas objetivas de calidad de una imagen (panel de referencia de la GUI)
photoranker get-quality-metrics --image-id 42
```

## Ver también

- `conventions.md` — formato JSON de entrada/salida, convención kebab-case, códigos de error.
- `fase1-ingesta.md`, `fase2-clustering.md`, `fase3-torneo.md`, `fase4-exportacion.md` — comportamiento detallado de cada comando.
