//! Tipos de error tipados del CLI (ver "Formato JSON estándar de salida" en docs/conventions.md).

use thiserror::Error;

/// Error de aplicación con un código `SCREAMING_SNAKE_CASE` estable, usado para
/// serializar `{"status":"error","code":"...","message":"..."}` en stdout.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("No se encontró ninguna .photoranker.sqlite en el directorio actual ni en sus padres")]
    DbNotFound,

    #[error("Error de base de datos: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Error de migración: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("Error de E/S: {0}")]
    Io(#[from] std::io::Error),

    #[error("Error de configuración: {0}")]
    Config(String),

    #[error("Burst {0} no existe")]
    BurstNotFound(i64),

    #[error("Imagen {0} no existe")]
    ImageNotFound(i64),

    #[error("Ranking inválido: {0}")]
    InvalidRanking(String),

    #[error("La miniatura de la imagen {0} falló en su extracción (thumbnail_status='failed')")]
    ThumbnailFailed(i64),

    #[error("Variable '{0}' no existe")]
    VariableNotFound(String),

    #[error("Cluster {0} no existe")]
    ClusterNotFound(i64),

    #[error("Falló el subproceso de R: {0}")]
    RSubprocessFailed(String),

    #[error("Ranking incompleto: {0}")]
    #[allow(dead_code)]
    IncompleteRanking(String),

    #[error("Argumento inválido: {0}")]
    InvalidArgument(String),

    #[error("Error de XMP: {0}")]
    XmpParseError(String),

    #[error("No hay ningún grupo de torneo para deshacer")]
    NothingToUndo,

    #[error("Error interno de TrueSkill: {0}")]
    TrueSkillError(String),
}

impl From<quick_xml::Error> for AppError {
    fn from(e: quick_xml::Error) -> Self {
        AppError::XmpParseError(e.to_string())
    }
}

impl AppError {
    /// Código `SCREAMING_SNAKE_CASE` estable para el sobre JSON de error.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::DbNotFound => "DB_NOT_FOUND",
            AppError::Database(_) => "DATABASE_ERROR",
            AppError::Migration(_) => "MIGRATION_ERROR",
            AppError::Io(_) => "IO_ERROR",
            AppError::Config(_) => "CONFIG_ERROR",
            AppError::BurstNotFound(_) => "BURST_NOT_FOUND",
            AppError::ImageNotFound(_) => "IMAGE_NOT_FOUND",
            AppError::InvalidRanking(_) => "INVALID_RANKING",
            AppError::ThumbnailFailed(_) => "THUMBNAIL_FAILED",
            AppError::VariableNotFound(_) => "VARIABLE_NOT_FOUND",
            AppError::ClusterNotFound(_) => "CLUSTER_NOT_FOUND",
            AppError::RSubprocessFailed(_) => "R_SUBPROCESS_FAILED",
            AppError::IncompleteRanking(_) => "INCOMPLETE_RANKING",
            AppError::InvalidArgument(_) => "INVALID_ARGUMENT",
            AppError::XmpParseError(_) => "XMP_PARSE_ERROR",
            AppError::NothingToUndo => "NOTHING_TO_UNDO",
            AppError::TrueSkillError(_) => "TRUESKILL_ERROR",
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
