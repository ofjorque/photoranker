## run_clustmd.R — clustering de mezcla para variables mixtas (ver
## docs/fase2-clustering.md, docs/database.md, docs/config.md).
##
## Invocado por Rust vía std::process::Command con argumentos posicionales
## (la ruta de la BD se pasa como .arg() separado, nunca concatenada a mano
## — ver docs/conventions.md, "Interfaz con R").
##
## Uso:
##   Rscript run_clustmd.R <db_path> <seed> preview <k_min> <k_max> <null_threshold>
##   Rscript run_clustmd.R <db_path> <seed> commit  <k>            <null_threshold>
##
## Salida: una sola línea JSON a stdout (mismo sobre que el resto del CLI,
## ver docs/conventions.md):
##   {"status":"ok", ...}  o  {"status":"error","code":"...","message":"..."}
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
  if (length(args) < 5) fail("commit requiere <k> <null_threshold>")
  k_commit <- suppressWarnings(as.integer(args[4]))
  variable_null_threshold <- suppressWarnings(as.numeric(args[5]))
  if (is.na(k_commit) || k_commit < 1) fail("k inválido")
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
## variable de usuario. Como excluir esa única variable categórica (y
## clusterizar solo con las continuas + nominales de 3+ niveles, si las
## hay) es preferible a que el comando falle en el flujo básico, se excluye
## y se reporta en `excluded_solo_categorical`.
excluded_solo_categorical <- character(0)
if (length(ord_cols) == 1) {
  excluded_solo_categorical <- names(ord_cols)
  ord_cols <- list()
}

if (length(included_continuous) == 0 && length(ord_cols) == 0 && length(nom_cols) == 0) {
  fail("no quedan variables utilizables tras excluir por varianza cero / NULLs excesivos / bug de clustMD con bloque categórico de tamaño 1")
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

## Prueba cada estructura de covarianza de `candidate_models` para un G dado
## y devuelve la de mayor BIChat (ver nota al inicio del archivo sobre por
## qué no hay un único modelo fijo).
fit_one <- function(g) {
  best <- NULL
  for (model in candidate_models) {
    fit <- fit_one_model(g, model)
    if (!is.null(fit) && (is.null(best) || fit$BIChat > best$BIChat)) {
      best <- fit
    }
  }
  best
}

## Escribe clusters/image_clusters/images.cluster_id. Una corrida de
## `cluster` compromete una única partición vigente sobre las imágenes
## activas — reemplaza cualquier clustering previo en vez de acumularlo.
write_clusters <- function(fit, g_used) {
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
    dbExecute(
      con, "UPDATE images SET cluster_id = ? WHERE id = ?",
      params = list(cluster_ids[fit$cl[row_i]], image_id)
    )
  }

  cluster_ids
}

if (mode == "preview") {
  bic_by_k <- list()
  for (g in cluster_min:cluster_max) {
    fit <- fit_one(g)
    if (!is.null(fit)) {
      bic_by_k[[as.character(g)]] <- fit$BIChat
    }
  }
  if (length(bic_by_k) == 0) fail("ningún valor de k en el rango solicitado convergió")

  result <- list(
    status = "ok",
    bic_by_k = bic_by_k,
    excluded_variables = excluded_variables,
    excluded_zero_variance = excluded_zero_variance,
    excluded_solo_categorical = excluded_solo_categorical,
    n_used = nrow(model_data)
  )
  cat(toJSON(result, auto_unbox = TRUE))
} else {
  fit <- fit_one(k_commit)
  if (is.null(fit)) fail(sprintf("clustMD no convergió para k=%d", k_commit))

  cluster_ids <- dbWithTransaction(con, write_clusters(fit, k_commit))

  result <- list(
    status = "ok",
    clusters = k_commit,
    cluster_ids = as.list(as.integer(cluster_ids)),
    n_assigned = nrow(model_data),
    excluded_variables = excluded_variables,
    excluded_zero_variance = excluded_zero_variance,
    excluded_solo_categorical = excluded_solo_categorical
  )
  cat(toJSON(result, auto_unbox = TRUE))
}
