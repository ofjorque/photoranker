# ⚙️ `config.toml` Completo

> Documento de referencia — consúltalo cuando cualquier fase necesite leer un parámetro de configuración. Ver también `conventions.md` (dónde y cómo se lee), `database.md` (`project_meta.config_snapshot`).

Ubicación: `~/.photoranker/config.toml`. Todos los parámetros documentados, con sus defaults:

```toml
burst_threshold = 0.10        # distancia pHash normalizada (sobre 64 bits) para agrupar ráfagas — ver fase1-ingesta.md
sigma_stop_threshold = 2.0     # convergencia individual del torneo — ver fase3-torneo.md
convergence_fraction = 0.95    # fracción de imágenes activas que deben converger para detener la sesión
stall_rounds = 20              # rondas sin mejora >5% antes de marcar stalled
max_rounds_multiplier = 3      # tope de rondas = este valor × nº de imágenes activas
global_sync_every = 10         # cada cuántos resultados de grupo se hace upsert por lote al índice global
cluster_min = 2               # rango mínimo de k para clustMD --preview — ver fase2-clustering.md
cluster_max = 10               # rango máximo de k para clustMD --preview
preview_size = 512            # resolución máxima (lado mayor) de la miniatura normalizada — ver fase1-ingesta.md
trueskill_beta = 4.1667      # beta de TrueSkillConfig, inyectado explícitamente (no el default silencioso del crate) — ver fase3-torneo.md (migrado desde weng_lin_beta)
min_global_sample = 20        # mínimo de imágenes en el índice global para usar cuantiles en vez del mapeo fijo de mu — ver fase4-exportacion.md
variable_null_threshold = 0.20 # si una variable personalizada tiene >20% de NULL entre imágenes activas, se excluye del clustering
cluster_probability_threshold = 0.0 # 0.0 = deshabilitado; si >0, una imagen cuya probabilidad de pertenencia (argmax) al cluster asignado no lo supere queda con cluster_id=NULL — ver fase2-clustering.md
rscript_path = "Rscript"      # override si Rscript.exe no está en PATH, ej. "C:\\Program Files\\R\\R-4.3.0\\bin\\Rscript.exe"
clustmd_seed = 42              # semilla fija para set.seed() en run_clustmd.R, garantiza resultados deterministas
theme = "dark"                # tema embebido de la GUI ("dark" o "light") — ver fase5-gui.md
theme_path = ""                # ruta opcional a un .css externo que sobreescribe variables del tema (ej. "~/.photoranker/theme.css"); vacío = sin override
keyboard_layout = "qwerty"    # layout asumido para atajos de teclado
```

## Ver también

- `conventions.md` — modelo de concurrencia, formato JSON, guía de estilo.
- `fase0-scaffolding.md` — creación inicial de este archivo con el crate `directories`.
