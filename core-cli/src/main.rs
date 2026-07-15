//! PhotoRanker — curación fotográfica cuantitativa, inteligente y sin "cajas negras".
//!
//! Los subcomandos se agregan a partir de docs/fase1-ingesta.md en adelante —
//! ver docs/cli-reference.md. Formato de salida y códigos de error: docs/conventions.md.

mod commands;
mod config;
mod db;
mod error;
mod exif;
mod phash;
mod quality;
mod thumbnail;

use clap::{Parser, Subcommand};
use error::{AppError, AppResult};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "photoranker", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Escanea una carpeta y extrae miniaturas + pHash (incremental, idempotente).
    Init {
        #[arg(long)]
        path: PathBuf,
    },
    /// Marca como `missing` las fotos que ya no existen en disco.
    Prune {
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Detecta ráfagas por distancia normalizada de pHash.
    #[command(name = "burst-detect")]
    BurstDetect {
        #[arg(long)]
        threshold: Option<f64>,
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Resuelve el minitorneo de una ráfaga (formato id:posición).
    #[command(name = "burst-tournament")]
    BurstTournament {
        #[arg(long = "burst-id")]
        burst_id: i64,
        #[arg(long, num_args = 1.., required = true)]
        ranking: Vec<String>,
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Crea una variable personalizada (ordinal o nominal).
    #[command(name = "variable-create")]
    VariableCreate {
        #[arg(long)]
        name: String,
        #[arg(long = "type")]
        var_type: String,
        #[arg(long)]
        min: Option<f64>,
        #[arg(long)]
        max: Option<f64>,
        /// Solo para nominales: "Etiqueta:codigo,Etiqueta:codigo,..."
        #[arg(long)]
        categories: Option<String>,
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Lista las variables personalizadas definidas.
    #[command(name = "variable-list")]
    VariableList {
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Asigna valores de una variable a imágenes (formato id:valor).
    #[command(name = "variable-set")]
    VariableSet {
        #[arg(long)]
        variable: String,
        #[arg(long, num_args = 1.., required = true)]
        values: Vec<String>,
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Modo TUI: asigna una variable recorriendo las imágenes por teclado.
    #[command(name = "variable-tag")]
    VariableTag {
        #[arg(long)]
        variable: String,
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Clustering vía clustMD (R): `--preview` para BIC por k, `--k` para comprometer.
    Cluster {
        #[arg(long)]
        preview: bool,
        #[arg(long)]
        k: Option<u32>,
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Renombra un cluster antes de exportarlo como tag.
    #[command(name = "cluster-rename")]
    ClusterRename {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        name: String,
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Arma el siguiente grupo de comparación del torneo principal.
    #[command(name = "tournament-next")]
    TournamentNext {
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Envía el resultado de un grupo (formato id:posición, permite empates).
    #[command(name = "tournament-result")]
    TournamentResult {
        #[arg(long = "group-id")]
        group_id: String,
        #[arg(long, num_args = 1.., required = true)]
        ranking: Vec<String>,
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Ranking en vivo por mu descendente (desempate: sigma asc, luego id).
    Ranking {
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Progreso de la sesión de torneo y motivo de parada.
    #[command(name = "tournament-status")]
    TournamentStatus {
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

fn parse_id_number_pairs(raw: &[String], arg_name: &str) -> AppResult<Vec<(i64, f64)>> {
    raw.iter()
        .map(|entry| {
            let (id_str, value_str) = entry.split_once(':').ok_or_else(|| {
                AppError::InvalidArgument(format!(
                    "'{entry}' en --{arg_name} debe tener formato id:valor"
                ))
            })?;
            let id: i64 = id_str
                .parse()
                .map_err(|_| AppError::InvalidArgument(format!("id inválido en '{entry}'")))?;
            let value: f64 = value_str
                .parse()
                .map_err(|_| AppError::InvalidArgument(format!("valor inválido en '{entry}'")))?;
            Ok((id, value))
        })
        .collect()
}

fn parse_categories(raw: &str) -> AppResult<Vec<commands::variable::NewCategory>> {
    raw.split(',')
        .map(|entry| {
            let (label, code_str) = entry.rsplit_once(':').ok_or_else(|| {
                AppError::InvalidArgument(format!(
                    "'{entry}' en --categories debe tener formato Etiqueta:codigo"
                ))
            })?;
            let code: i64 = code_str
                .trim()
                .parse()
                .map_err(|_| AppError::InvalidArgument(format!("código inválido en '{entry}'")))?;
            Ok(commands::variable::NewCategory {
                code,
                label: label.trim().to_string(),
            })
        })
        .collect()
}

fn run(cli: Cli) -> AppResult<Value> {
    match cli.command {
        Commands::Init { path } => {
            let cfg = config::load_or_init()?;
            commands::init::run(&path, &cfg)
        }
        Commands::Prune { db } => {
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            commands::prune::run(&mut conn, &db_path)
        }
        Commands::BurstDetect { threshold, db } => {
            let cfg = config::load_or_init()?;
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            commands::burst_detect::run(&mut conn, threshold.unwrap_or(cfg.burst_threshold))
        }
        Commands::BurstTournament {
            burst_id,
            ranking,
            db,
        } => {
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            let pairs = parse_id_number_pairs(&ranking, "ranking")?
                .into_iter()
                .map(|(id, pos)| (id, pos as i64))
                .collect::<Vec<_>>();
            commands::burst_tournament::run(&mut conn, &db_path, burst_id, &pairs)
        }
        Commands::VariableCreate {
            name,
            var_type,
            min,
            max,
            categories,
            db,
        } => {
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            let cats = match &categories {
                Some(raw) => parse_categories(raw)?,
                None => Vec::new(),
            };
            commands::variable::create(&mut conn, &name, &var_type, min, max, &cats)
        }
        Commands::VariableList { db } => {
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let conn = db::open_local(&db_path)?;
            commands::variable::list(&conn)
        }
        Commands::VariableSet {
            variable,
            values,
            db,
        } => {
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            let pairs = parse_id_number_pairs(&values, "values")?;
            commands::variable::set(&mut conn, &variable, &pairs)
        }
        Commands::VariableTag { variable, db } => {
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            commands::variable_tag::run(&mut conn, &variable)
        }
        Commands::Cluster { preview, k, db } => {
            if preview && k.is_some() {
                return Err(AppError::InvalidArgument(
                    "--preview y --k son mutuamente excluyentes".to_string(),
                ));
            }
            let cfg = config::load_or_init()?;
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            if preview {
                commands::cluster::preview(&conn, &db_path, &cfg)
            } else {
                commands::cluster::commit(&mut conn, &db_path, &cfg, k)
            }
        }
        Commands::ClusterRename { id, name, db } => {
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            commands::cluster::rename(&mut conn, id, &name)
        }
        Commands::TournamentNext { db } => {
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            commands::tournament::next(&mut conn)
        }
        Commands::TournamentResult {
            group_id,
            ranking,
            db,
        } => {
            let cfg = config::load_or_init()?;
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            let pairs = parse_id_number_pairs(&ranking, "ranking")?
                .into_iter()
                .map(|(id, pos)| (id, pos as i64))
                .collect::<Vec<_>>();
            commands::tournament::result(&mut conn, &db_path, &cfg, &group_id, &pairs)
        }
        Commands::Ranking { db } => {
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            commands::tournament::ranking(&mut conn, &db_path)
        }
        Commands::TournamentStatus { db } => {
            let cfg = config::load_or_init()?;
            let db_path = db::resolve_local_db_path(db.as_deref())?;
            let mut conn = db::open_local(&db_path)?;
            commands::tournament::status(&mut conn, &db_path, &cfg)
        }
    }
}

fn init_tracing() {
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

fn main() {
    init_tracing();
    let cli = Cli::parse();

    let (envelope, is_ok) = match run(cli) {
        Ok(data) => (json!({"status": "ok", "data": data}), true),
        Err(err) => {
            tracing::error!(error = %err, "comando falló");
            (
                json!({"status": "error", "code": err.code(), "message": err.to_string()}),
                false,
            )
        }
    };

    println!("{envelope}");
    std::process::exit(if is_ok { 0 } else { 1 });
}
