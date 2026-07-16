## run_clustmd.R — clustering de mezcla para variables mixtas (ver
## docs/fase2-clustering.md, docs/database.md, docs/config.md).
##
## Invocado por Rust vía std::process::Command con argumentos posicionales
## (la ruta de la BD se pasa como .arg() separado, nunca concatenada a mano
## — ver docs/conventions.md, "Interfaz con R").
##
## Uso:
##   Rscript run_clustmd.R <db_path> <seed> preview <k_min> <k_max> <null_threshold>
##   Rscript run_clustmd.R <db_path> <seed> commit  <k>            <null_threshold> <prob_threshold>
##
## Salida: una sola línea JSON a stdout (mismo sobre que el resto del CLI,
## ver docs/conventions.md):
##   {"status":"ok", ...}  o  {"status":"error","code":"...","message":"..."}
##
## NOTA sobre `cached_cluster_fits` (feedback de uso real: "cuando uno escoge
## el modelo, la clusterización debería ser rápida y no volver a correr el
## código"): `preview` persiste en SQLite + un .rds por disco el mejor modelo
## ajustado (mayor BIChat) de cada k explorado. `commit` primero consulta esa
## tabla por (k, data_fingerprint) — un hash determinista de las imágenes
## activas + columnas usadas + seed, calculado con `tools::md5sum` sobre un
## archivo temporal (no se agrega `digest` como dependencia nueva) — y si hay
## coincidencia, hace `readRDS()` en vez de volver a llamar `clustMD()`. El
## fingerprint deliberadamente NO incluye `prob_threshold`: ese umbral solo
## afecta el paso final de asignación (`write_clusters`), no el ajuste EM en
## sí, así que cambiarlo no debería invalidar la caché. Ver
## docs/fase2-clustering.md, "Caché de modelos ajustados".
##
## NOTA sobre la convención de BIChat: clustMD reporta el BIC con la misma
## convención que mclust (proporcional a 2*logLik menos una penalización de
## complejidad), donde un valor MAYOR (menos negativo) indica mejor ajuste.
## Se verificó empíricamente con datos sintéticos de 2 clusters bien
## separados: G=1 dio BIChat=-1154 (peor ajuste posible, colapsa los 2
## grupos en 1), G=2 dio -126 (el ajuste correcto), G=3..5 fueron
## decreciendo monótonamente. La selección automática de k (lado Rust,
## cuando `cluster` corre sin --k) usa por lo tanto argmax(BIChat), pese a
## que el texto original de fase2-clustering.md dice "mínimo BIC" — seguir
## eso al pie de la letra elegiría sistemáticamente el peor resultado
## posible. Decisión explícita confirmada con el usuario del proyecto.
##
## NOTA sobre la estructura de covarianza (`model` de clustMD): no hay un
## único modelo fijo. Se probaron los 6 (EII/VII/EEI/VEI/EVI/VVI/BD) contra
## datos reales (40 fotos, 2 grupos de brillo obvios, con ruido en el resto
## de las métricas): "VVI" colapsó todo en 1 cluster pese a la separación
## obvia, "EVI" y "BD" dieron BIChat=NaN, y solo EII/VII/EEI/VEI convergieron
## de forma correcta y estable (EEI con el mejor BIC por lejos). Como la
## estructura óptima depende de los datos de cada carpeta, para cada G se
## prueban los 4 modelos que demostraron converger de forma fiable
## (candidate_models más abajo) y se usa el de mayor BIChat — no un solo
## modelo fijo.

candidate_models <- c("EII", "VII", "EEI", "VEI")

suppressPackageStartupMessages({
  library(DBI)
  library(RSQLite)
  library(clustMD)
  library(jsonlite)
})

fail <- function(message, code = "R_SUBPROCESS_FAILED") {
  cat(toJSON(list(status = "error", code = code, message = message), auto_unbox = TRUE))
  quit(save = "no", status = 1)
}

# Moda de un vector categórico (mínimo valor entre empates, para ser
# determinista). clustMD no exporta una utilidad equivalente en su API
# pública, así que se implementa localmente en vez de usar `:::` sobre un
# símbolo interno de otro paquete.
r_mode <- function(x) {
  x <- x[!is.na(x)]
  if (length(x) == 0) {
    return(NA)
  }
  counts <- table(x)
  best <- names(counts)[counts == max(counts)]
  sort(best)[1]
}

args <- commandArgs(trailingOnly = TRUE)
if (length(args) < 3) {
  fail("argumentos insuficientes: se esperaba <db_path> <seed> <preview|commit> ...")
}

db_path <- args[1]
seed <- suppressWarnings(as.integer(args[2]))
mode <- args[3]

if (is.na(seed)) fail("semilla inválida")
if (!(mode %in% c("preview", "commit"))) fail(paste("modo desconocido:", mode))

if (mode == "preview") {
  if (length(args) < 6) fail("preview requiere <k_min> <k_max> <null_threshold>")
  cluster_min <- suppressWarnings(as.integer(args[4]))
  cluster_max <- suppressWarnings(as.integer(args[5]))
  variable_null_threshold <- suppressWarnings(as.numeric(args[6]))
  if (is.na(cluster_min) || is.na(cluster_max) || cluster_min < 1 || cluster_max < cluster_min) {
    fail("k_min/k_max inválidos")
  }
} else {
  if (length(args) < 6) fail("commit requiere <k> <null_threshold> <prob_threshold>")
  k_commit <- suppressWarnings(as.integer(args[4]))
  variable_null_threshold <- suppressWarnings(as.numeric(args[5]))
  prob_threshold <- suppressWarnings(as.numeric(args[6]))
  if (is.na(k_commit) || k_commit < 1) fail("k inválido")
  if (is.na(prob_threshold) || prob_threshold < 0 || prob_threshold > 1) {
    fail("prob_threshold inválido (debe estar entre 0 y 1)")
  }
}
if (is.na(variable_null_threshold)) fail("variable_null_threshold inválido")

set.seed(seed)

con <- dbConnect(RSQLite::SQLite(), db_path)
on.exit(dbDisconnect(con), add = TRUE)
invisible(dbExecute(con, "PRAGMA journal_mode = WAL;"))
invisible(dbExecute(con, "PRAGMA busy_timeout = 5000;"))

## --- Lectura de datos (solo imágenes activas, ver fase2-clustering.md) ---

images <- dbGetQuery(con, "SELECT id, iso, aperture, focal_length FROM images WHERE rejected = 0 AND missing = 0")
if (nrow(images) == 0) fail("no hay imágenes activas (rejected=0, missing=0) para clusterizar")

metrics <- dbGetQuery(con, paste(
  "SELECT image_id, sharpness, brightness, contrast, overexposed_pct,",
  "underexposed_pct, saturation, colorfulness, entropy,",
  "average_r, average_g, average_b, orientation FROM image_quality_metrics"
))

data <- merge(images, metrics, by.x = "id", by.y = "image_id", all.x = TRUE)

user_vars <- dbGetQuery(con, "SELECT id, name, var_type, position FROM user_variables ORDER BY position")
values <- dbGetQuery(con, "SELECT image_id, variable_id, value FROM image_variable_values")

if (nrow(user_vars) > 0) {
  for (i in seq_len(nrow(user_vars))) {
    vid <- user_vars$id[i]
    vname <- user_vars$name[i]
    col <- values[values$variable_id == vid, c("image_id", "value"), drop = FALSE]
    names(col) <- c("id", vname)
    data <- merge(data, col, by = "id", all.x = TRUE)
  }
}

n_active <- nrow(data)

continuous_cols <- c(
  "iso", "aperture", "focal_length",
  "sharpness", "brightness", "contrast",
  "overexposed_pct", "underexposed_pct",
  "saturation", "colorfulness", "entropy",
  "average_r", "average_g", "average_b"
)

ordinal_user <- if (nrow(user_vars) > 0) user_vars$name[user_vars$var_type == "ordinal"] else character(0)
nominal_user <- if (nrow(user_vars) > 0) user_vars$name[user_vars$var_type == "nominal"] else character(0)

## --- (a) Exclusión por umbral de NULL (solo variables personalizadas) ---

excluded_variables <- character(0)
included_ordinal <- character(0)
included_nominal <- character(0)

for (vname in ordinal_user) {
  null_frac <- sum(is.na(data[[vname]])) / n_active
  if (null_frac > variable_null_threshold) {
    excluded_variables <- c(excluded_variables, vname)
  } else {
    included_ordinal <- c(included_ordinal, vname)
  }
}
for (vname in nominal_user) {
  null_frac <- sum(is.na(data[[vname]])) / n_active
  if (null_frac > variable_null_threshold) {
    excluded_variables <- c(excluded_variables, vname)
  } else {
    included_nominal <- c(included_nominal, vname)
  }
}

## --- (b) Exclusión por varianza cero (todas las variables, incl. orientation) ---

excluded_zero_variance <- character(0)
included_continuous <- character(0)
for (col in continuous_cols) {
  sdv <- suppressWarnings(sd(data[[col]], na.rm = TRUE))
  if (is.na(sdv) || sdv == 0) {
    excluded_zero_variance <- c(excluded_zero_variance, col)
  } else {
    included_continuous <- c(included_continuous, col)
  }
}

orientation_levels <- unique(na.omit(data$orientation))
include_orientation <- length(orientation_levels) > 1
if (!include_orientation) {
  excluded_zero_variance <- c(excluded_zero_variance, "orientation")
}

for (vname in included_nominal) {
  levels_present <- unique(na.omit(data[[vname]]))
  if (length(levels_present) <= 1) {
    excluded_zero_variance <- c(excluded_zero_variance, vname)
    included_nominal <- setdiff(included_nominal, vname)
  }
}
for (vname in included_ordinal) {
  sdv <- suppressWarnings(sd(data[[vname]], na.rm = TRUE))
  if (is.na(sdv) || sdv == 0) {
    excluded_zero_variance <- c(excluded_zero_variance, vname)
    included_ordinal <- setdiff(included_ordinal, vname)
  }
}

if (length(included_continuous) == 0 && !include_orientation &&
  length(included_ordinal) == 0 && length(included_nominal) == 0) {
  fail("no quedan variables utilizables tras excluir por varianza cero / NULLs excesivos")
}

## clustMD's Usage documenta el bloque de en medio como "binary (coded 1 and
## 2) and ordinal variables" — es decir, las categóricas de exactamente 2
## niveles van junto a las ordinales (OrdIndx), no al bloque nominal final.
## Se verificó empíricamente: pasar una variable de 2 niveles por el bloque
## nominal hace que `clustMD` falle dentro de `z.nom.diag` ("dim(X) debe
## tener una longitud positiva"), porque ese camino del E-step Monte Carlo
## asume >=3 niveles. Las nominales con 3+ niveles sí toman el bloque
## nominal normalmente.
binary_nominal <- character(0)
multi_nominal <- character(0)
if (include_orientation) {
  if (length(orientation_levels) == 2) {
    binary_nominal <- c(binary_nominal, "orientation")
  } else {
    multi_nominal <- c(multi_nominal, "orientation")
  }
}
for (vname in included_nominal) {
  levels_present <- unique(na.omit(data[[vname]]))
  if (length(levels_present) == 2) {
    binary_nominal <- c(binary_nominal, vname)
  } else {
    multi_nominal <- c(multi_nominal, vname)
  }
}

## --- Construcción de la matriz mixta (continuas | ordinales+binarias | nominales 3+) ---
## clustMD exige ese orden exacto de columnas (ver Usage de ?clustMD).

cont_mat <- if (length(included_continuous) > 0) {
  as.matrix(scale(data[, included_continuous, drop = FALSE]))
} else {
  matrix(nrow = nrow(data), ncol = 0)
}

ord_cols <- list()
for (vname in included_ordinal) {
  ord_cols[[vname]] <- as.integer(factor(data[[vname]]))
}
for (vname in binary_nominal) {
  col_values <- if (vname == "orientation") data$orientation else data[[vname]]
  ord_cols[[vname]] <- as.integer(factor(col_values))
}
nom_cols <- list()
for (vname in multi_nominal) {
  col_values <- if (vname == "orientation") data$orientation else data[[vname]]
  nom_cols[[vname]] <- as.integer(factor(col_values))
}

## Bug conocido de clustMD 1.2: el bloque ordinal+binario falla siempre
## (error interno en z.moments_diag) cuando tiene exactamente 1 variable —
## se confirmó empíricamente con datos sintéticos, reproducible con los 5
## métodos de `startCL`, independiente de los datos. El caso más común de
## disparar esto es una carpeta con solo `orientation` (binaria) y ninguna
## variable de usuario, pero también dispara con cualquier variable de
## usuario recién creada si `orientation` termina excluida por varianza
## cero (ej. carpeta donde todas las fotos comparten orientación) — un caso
## nada raro, y que la primera versión de este workaround manejaba
## **descartando la variable del usuario en silencio**, reportado como
## feedback de uso real: "la clasificación siempre se elimina".
##
## Fix: en vez de excluir la única variable, se duplica su columna (mismo
## contenido, un segundo nombre) para que el bloque tenga ancho 2 sin
## alterar semánticamente los datos — el bug de clustMD es sobre el
## *ancho* del bloque, no sobre qué variable lo ocupa. Verificado
## empíricamente con datos sintéticos: no crashea, y el centroide
## resultante refleja la variable duplicada con normalidad (ver
## docs/fase2-clustering.md). Se reporta igual en
## `duplicated_solo_categorical` para que quede visible en la salida JSON
## que se aplicó este workaround (no se oculta que pasó).
duplicated_solo_categorical <- character(0)
if (length(ord_cols) == 1) {
  solo_name <- names(ord_cols)
  ord_cols[[paste0(solo_name, "__dup")]] <- ord_cols[[solo_name]]
  duplicated_solo_categorical <- solo_name
}

if (length(included_continuous) == 0 && length(ord_cols) == 0 && length(nom_cols) == 0) {
  fail("no quedan variables utilizables tras excluir por varianza cero / NULLs excesivos")
}

ord_mat <- if (length(ord_cols) > 0) as.matrix(as.data.frame(ord_cols)) else matrix(nrow = nrow(data), ncol = 0)
nom_mat <- if (length(nom_cols) > 0) as.matrix(as.data.frame(nom_cols)) else matrix(nrow = nrow(data), ncol = 0)

full_mat <- cbind(cont_mat, ord_mat, nom_mat)
colnames(full_mat) <- c(included_continuous, names(ord_cols), names(nom_cols))

## --- (c) Listwise deletion: filas con NA en alguna columna incluida quedan
## fuera del ajuste del modelo (y por lo tanto con cluster_id = NULL) ---

complete_rows <- complete.cases(full_mat)
model_data <- full_mat[complete_rows, , drop = FALSE]
model_ids <- data$id[complete_rows]

if (nrow(model_data) < 2) {
  fail("menos de 2 imágenes completas tras excluir NULLs (listwise deletion); no se puede clusterizar")
}

CnsIndx <- length(included_continuous)
OrdIndx <- CnsIndx + length(ord_cols)
included_all <- c(included_continuous, included_ordinal, included_nominal)

## --- Caché de modelos ajustados (ver nota al inicio del archivo) ---

cache_dir <- file.path(dirname(db_path), ".photoranker_cluster_cache")
dir.create(cache_dir, showWarnings = FALSE, recursive = TRUE)

# Fingerprint determinista de los datos de entrada del ajuste (imágenes
# activas incluidas + columnas usadas + sus valores + seed): si cualquiera de
# estos cambia entre una corrida de `preview`/`commit` y la siguiente, el
# fingerprint cambia y la caché existente simplemente deja de matchear (no
# hace falta invalidación explícita). Usa `tools::md5sum` (paquete base)
# sobre un archivo temporal en vez de agregar `digest` como dependencia nueva.
data_fingerprint <- local({
  tmp <- tempfile()
  on.exit(unlink(tmp), add = TRUE)
  con_tmp <- file(tmp, "wb")
  writeLines(paste(model_ids, collapse = ","), con_tmp)
  writeLines(paste(colnames(full_mat), collapse = ","), con_tmp)
  writeLines(paste(seed, variable_null_threshold, sep = ","), con_tmp)
  write.table(round(model_data, 8), con_tmp)
  close(con_tmp)
  unname(tools::md5sum(tmp))
})

# Persiste el mejor modelo ajustado para un `g` dado (llamado desde `preview`
# tras elegir el ganador entre `candidate_models`, y desde `commit` cuando no
# hay caché aprovechable). Reemplaza cualquier fila previa con el mismo
# (k, fingerprint) en vez de acumular duplicados entre corridas repetidas.
cache_fit <- function(g, model_name, fit) {
  rds_path <- file.path(
    cache_dir,
    sprintf("%s_k%d_%s.rds", data_fingerprint, g, model_name)
  )
  saveRDS(fit, rds_path)
  dbExecute(
    con, "DELETE FROM cached_cluster_fits WHERE k = ?1 AND data_fingerprint = ?2",
    params = list(g, data_fingerprint)
  )
  dbExecute(
    con, "INSERT INTO cached_cluster_fits (k, model, bic, data_fingerprint, rds_path) \
          VALUES (?1, ?2, ?3, ?4, ?5)",
    params = list(g, model_name, fit$BIChat, data_fingerprint, rds_path)
  )
}

# Busca en la caché un ajuste ya hecho para (g, fingerprint actual). Devuelve
# `NULL` si no hay coincidencia o si el archivo .rds ya no existe en disco
# (ej. se borró la carpeta de caché manualmente) — en ese caso se reajusta
# como si nunca hubiera existido, no se falla.
cached_fit_for <- function(g) {
  row <- dbGetQuery(
    con, "SELECT rds_path FROM cached_cluster_fits WHERE k = ?1 AND data_fingerprint = ?2 LIMIT 1",
    params = list(g, data_fingerprint)
  )
  if (nrow(row) == 0 || !file.exists(row$rds_path[1])) {
    return(NULL)
  }
  readRDS(row$rds_path[1])
}

fit_one_model <- function(g, model) {
  tryCatch(
    {
      fit <- NULL
      # clustMD dibuja una barra de progreso txtProgressBar con file="" (va a
      # stdout por defecto) — stdout está reservado exclusivamente para la
      # única línea JSON de salida (ver docs/conventions.md), así que se
      # descarta con capture.output() en vez de dejarla contaminar la salida.
      invisible(utils::capture.output(
        fit <- clustMD(
          X = model_data,
          G = g,
          CnsIndx = CnsIndx,
          OrdIndx = OrdIndx,
          Nnorms = 1000,
          MaxIter = 500,
          model = model,
          store.params = FALSE,
          scale = FALSE,
          startCL = "hc_mclust",
          autoStop = FALSE
        ),
        file = nullfile()
      ))
      if (is.null(fit) || !is.finite(fit$BIChat)) NULL else fit
    },
    error = function(e) {
      cat(sprintf("run_clustmd.R: G=%d modelo=%s falló: %s\n", g, model, conditionMessage(e)), file = stderr())
      NULL
    }
  )
}

## Prueba cada estructura de covarianza de `candidate_models` para un G dado y
## devuelve la de mayor BIChat (ver nota al inicio del archivo sobre por qué
## no hay un único modelo fijo), junto con el nombre del modelo ganador (para
## poder cachearlo con su metadata completa).
fit_one <- function(g) {
  best <- NULL
  best_model_name <- NA_character_
  for (model in candidate_models) {
    fit <- fit_one_model(g, model)
    if (!is.null(fit) && (is.null(best) || fit$BIChat > best$BIChat)) {
      best <- fit
      best_model_name <- model
    }
  }
  list(fit = best, model = best_model_name)
}

## Escribe clusters/image_clusters/images.cluster_id. Una corrida de
## `cluster` compromete una única partición vigente sobre las imágenes
## activas — reemplaza cualquier clustering previo en vez de acumularlo.
## `prob_threshold` (0 = deshabilitado): si la probabilidad de pertenencia
## (argmax) de una imagen a su cluster asignado no lo supera, `images.cluster_id`
## queda en NULL en vez del cluster argmax — la fila de `image_clusters` con la
## probabilidad real se sigue insertando igual (ver fase2-clustering.md,
## "Umbral de probabilidad de pertenencia").
write_clusters <- function(fit, g_used, prob_threshold = 0) {
  dbExecute(con, "DELETE FROM image_clusters")
  dbExecute(con, "DELETE FROM clusters")
  dbExecute(con, "UPDATE images SET cluster_id = NULL")

  cluster_ids <- integer(g_used)
  for (i in seq_len(g_used)) {
    members <- model_ids[fit$cl == i]
    member_rows <- match(members, data$id)
    centroid <- list()
    for (vname in included_all) {
      raw <- data[[vname]][member_rows]
      centroid[[vname]] <- if (is.numeric(raw)) mean(raw, na.rm = TRUE) else r_mode(raw)
    }
    if (include_orientation) {
      centroid[["orientation"]] <- r_mode(data$orientation[member_rows])
    }
    dbExecute(
      con, "INSERT INTO clusters (name, centroid_json) VALUES (NULL, ?)",
      params = list(toJSON(centroid, auto_unbox = TRUE, na = "null"))
    )
    cluster_ids[i] <- dbGetQuery(con, "SELECT last_insert_rowid() AS id")$id
  }

  below_threshold_count <- 0L
  for (row_i in seq_along(model_ids)) {
    image_id <- model_ids[row_i]
    for (g in seq_len(g_used)) {
      prob <- fit$tau[row_i, g]
      if (!is.na(prob)) {
        dbExecute(
          con, "INSERT INTO image_clusters (image_id, cluster_id, probability) VALUES (?, ?, ?)",
          params = list(image_id, cluster_ids[g], prob)
        )
      }
    }
    assigned_cluster <- fit$cl[row_i]
    assigned_probability <- fit$tau[row_i, assigned_cluster]
    if (!is.na(assigned_probability) && assigned_probability < prob_threshold) {
      below_threshold_count <- below_threshold_count + 1L
      dbExecute(con, "UPDATE images SET cluster_id = NULL WHERE id = ?", params = list(image_id))
    } else {
      dbExecute(
        con, "UPDATE images SET cluster_id = ? WHERE id = ?",
        params = list(cluster_ids[assigned_cluster], image_id)
      )
    }
  }

  list(cluster_ids = cluster_ids, below_threshold = below_threshold_count)
}

if (mode == "preview") {
  bic_by_k <- list()
  for (g in cluster_min:cluster_max) {
    result_g <- fit_one(g)
    if (!is.null(result_g$fit)) {
      bic_by_k[[as.character(g)]] <- result_g$fit$BIChat
      cache_fit(g, result_g$model, result_g$fit)
    }
  }
  if (length(bic_by_k) == 0) fail("ningún valor de k en el rango solicitado convergió")

  result <- list(
    status = "ok",
    bic_by_k = bic_by_k,
    excluded_variables = excluded_variables,
    excluded_zero_variance = excluded_zero_variance,
    duplicated_solo_categorical = duplicated_solo_categorical,
    n_used = nrow(model_data)
  )
  cat(toJSON(result, auto_unbox = TRUE))
} else {
  # Consulta la caché antes de reajustar (ver nota al inicio del archivo) —
  # así elegir un k ya explorado en un `--preview` anterior es una consulta,
  # no una corrida nueva de EM.
  fit <- cached_fit_for(k_commit)
  from_cache <- !is.null(fit)
  if (is.null(fit)) {
    result_k <- fit_one(k_commit)
    fit <- result_k$fit
    if (is.null(fit)) fail(sprintf("clustMD no convergió para k=%d", k_commit))
    cache_fit(k_commit, result_k$model, fit)
  }

  write_result <- dbWithTransaction(con, write_clusters(fit, k_commit, prob_threshold))
  cluster_ids <- write_result$cluster_ids

  result <- list(
    status = "ok",
    clusters = k_commit,
    cluster_ids = as.list(as.integer(cluster_ids)),
    n_assigned = nrow(model_data),
    excluded_variables = excluded_variables,
    excluded_zero_variance = excluded_zero_variance,
    duplicated_solo_categorical = duplicated_solo_categorical,
    from_cache = from_cache,
    below_probability_threshold = write_result$below_threshold
  )
  cat(toJSON(result, auto_unbox = TRUE))
}
