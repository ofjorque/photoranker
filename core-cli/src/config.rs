//! Carga y defaults de `~/.photoranker/config.toml` (ver docs/config.md).

use crate::error::{AppError, AppResult};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub burst_threshold: f64,
    pub duplicate_threshold: f64,
    pub sigma_stop_threshold: f64,
    pub convergence_fraction: f64,
    pub stall_rounds: u32,
    pub max_rounds_multiplier: u32,
    pub global_sync_every: u32,
    pub cluster_min: u32,
    pub cluster_max: u32,
    pub preview_size: u32,
    pub preview_zoom_size: u32,
    pub trueskill_beta: f64,
    pub min_global_sample: u32,
    pub variable_null_threshold: f64,
    pub cluster_probability_threshold: f64,
    pub rscript_path: String,
    pub clustmd_seed: u64,
    pub theme: String,
    pub keyboard_layout: String,
    pub exclude_dirs: Vec<String>,
    pub language: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            burst_threshold: 0.10,
            duplicate_threshold: 0.10,
            sigma_stop_threshold: 2.0,
            convergence_fraction: 0.95,
            stall_rounds: 20,
            max_rounds_multiplier: 3,
            global_sync_every: 10,
            cluster_min: 2,
            cluster_max: 10,
            preview_size: 512,
            preview_zoom_size: 2048,
            trueskill_beta: 4.1667,
            min_global_sample: 20,
            variable_null_threshold: 0.20,
            cluster_probability_threshold: 0.0,
            rscript_path: "Rscript".to_string(),
            clustmd_seed: 42,
            theme: "dark".to_string(),
            keyboard_layout: "qwerty".to_string(),
            exclude_dirs: vec!["Selected".to_string(), "exported".to_string()],
            language: "es".to_string(),
        }
    }
}

/// Directorio `~/.photoranker/` (o equivalente por plataforma vía el crate `directories`).
///
/// `PHOTORANKER_HOME`, si está definida, reemplaza esta ruta por completo —
/// **exclusivamente para aislar los tests de integración** de
/// `~/.photoranker/config.toml` y, sobre todo, de `global_index.sqlite`, que
/// es un archivo compartido entre todas las carpetas del usuario real (ver
/// "Índice global compartido" en conventions.md). Sin este override, correr
/// `cargo test` termina leyendo/escribiendo/vaciando el índice global real de
/// quien compile el proyecto — ver tests/fase*.rs, que la fijan vía
/// `Command::env`.
pub fn photoranker_dir() -> AppResult<PathBuf> {
    if let Ok(over) = std::env::var("PHOTORANKER_HOME") {
        return Ok(PathBuf::from(over));
    }
    let dirs = ProjectDirs::from("", "", "PhotoRanker").ok_or_else(|| {
        AppError::Config(
            "No se pudo determinar el directorio de configuración del usuario".to_string(),
        )
    })?;
    Ok(dirs.config_dir().to_path_buf())
}

pub fn config_path() -> AppResult<PathBuf> {
    Ok(photoranker_dir()?.join("config.toml"))
}

pub fn global_index_path() -> AppResult<PathBuf> {
    Ok(photoranker_dir()?.join("global_index.sqlite"))
}

/// Carga `config.toml`, creándolo con los defaults documentados si no existe.
pub fn load_or_init() -> AppResult<Config> {
    let path = config_path()?;
    if !path.exists() {
        let dir = photoranker_dir()?;
        std::fs::create_dir_all(&dir)?;
        let defaults = Config::default();
        write_config(&path, &defaults)?;
        return Ok(defaults);
    }
    read_config(&path)
}

fn read_config(path: &Path) -> AppResult<Config> {
    let contents = std::fs::read_to_string(path)?;
    toml::from_str(&contents).map_err(|e| AppError::Config(format!("config.toml inválido: {e}")))
}

fn write_config(path: &Path, config: &Config) -> AppResult<()> {
    let serialized = toml::to_string_pretty(config)
        .map_err(|e| AppError::Config(format!("no se pudo serializar config.toml: {e}")))?;
    std::fs::write(path, serialized)?;
    Ok(())
}
