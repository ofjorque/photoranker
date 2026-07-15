//! Backend Tauri de PhotoRanker: la GUI nunca implementa lógica de negocio ni
//! lee `.photoranker.sqlite` directamente (ver docs/conventions.md, "API
//! interna"); este módulo solo (a) invoca `photoranker(.exe)` como subproceso
//! y devuelve el sobre JSON de stdout tal cual, y (b) lee `config.toml` para
//! resolver el tema embebido/override (ver docs/fase5-gui.md, "Theming").

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tauri_plugin_dialog::DialogExt;

/// Localiza el ejecutable `photoranker` (el único "cerebro" del proyecto).
/// Orden de búsqueda: override explícito (`PHOTORANKER_CLI`, útil en
/// desarrollo) -> junto al ejecutable de la GUI (empaquetado de producción)
/// -> build de desarrollo de `core-cli` relativa a este crate -> `PATH`.
fn resolve_cli_path() -> Result<PathBuf, String> {
    let exe_name = if cfg!(windows) {
        "photoranker.exe"
    } else {
        "photoranker"
    };

    if let Ok(over) = std::env::var("PHOTORANKER_CLI") {
        let p = PathBuf::from(over);
        if p.is_file() {
            return Ok(p);
        }
    }

    if let Ok(current) = std::env::current_exe()
        && let Some(dir) = current.parent()
    {
        let candidate = dir.join(exe_name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    // `cargo tauri dev` corre este binario desde src-tauri/target/debug/, dos
    // niveles bajo la raíz del repo; core-cli/target/{debug,release} es su vecino.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for profile in ["debug", "release"] {
        let candidate = manifest_dir
            .join("..")
            .join("..")
            .join("core-cli")
            .join("target")
            .join(profile)
            .join(exe_name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(exe_name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(format!(
        "No se encontró el ejecutable '{exe_name}' del CLI. Compílalo con \
         'cargo build' en core-cli/, o define PHOTORANKER_CLI con su ruta."
    ))
}

/// Invoca `photoranker <args>` como subproceso y devuelve el sobre JSON de
/// stdout (`{"status":"ok","data":...}` o `{"status":"error",...}`) sin
/// interpretarlo — la GUI decide qué hacer con `status` en TypeScript.
#[tauri::command]
fn run_photoranker(args: Vec<String>) -> Result<serde_json::Value, String> {
    let cli_path = resolve_cli_path()?;
    let output = Command::new(&cli_path)
        .args(&args)
        .output()
        .map_err(|e| format!("No se pudo ejecutar '{}': {e}", cli_path.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("El CLI no produjo salida JSON. stderr: {stderr}"));
    }

    serde_json::from_str(trimmed)
        .map_err(|e| format!("JSON inválido del CLI: {e}\nstdout: {trimmed}"))
}

#[derive(Serialize, Deserialize, Default)]
struct ThemeConfig {
    theme: String,
    theme_path: String,
}

/// Lee `theme`/`theme_path` de `~/.photoranker/config.toml` (mismo archivo
/// que gestiona el CLI, ver docs/config.md). Es un archivo de configuración
/// de texto plano, no la base de datos de una carpeta de fotos — leerlo
/// directamente no viola la regla de "la GUI nunca lee .photoranker.sqlite".
/// Si el archivo aún no existe (ningún comando del CLI corrió todavía), se
/// usan los defaults documentados en config.md.
#[tauri::command]
fn read_theme_config() -> ThemeConfig {
    let Some(dirs) = ProjectDirs::from("", "", "photoranker") else {
        return ThemeConfig {
            theme: "dark".into(),
            theme_path: String::new(),
        };
    };
    let config_path = dirs.config_dir().join("config.toml");
    let Ok(text) = std::fs::read_to_string(&config_path) else {
        return ThemeConfig {
            theme: "dark".into(),
            theme_path: String::new(),
        };
    };
    let parsed: toml::Value =
        toml::from_str(&text).unwrap_or_else(|_| toml::Value::Table(Default::default()));
    let theme = parsed
        .get("theme")
        .and_then(|v| v.as_str())
        .unwrap_or("dark")
        .to_string();
    let theme_path = parsed
        .get("theme_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    ThemeConfig { theme, theme_path }
}

/// Lee el contenido del CSS de override del usuario (`theme_path`). Fallback
/// silencioso a `None` si no existe o no se puede leer (ver fase5-gui.md,
/// "Mecanismo de override por el usuario": un CSS de usuario mal formado o
/// ausente nunca debe romper la app).
#[tauri::command]
fn read_theme_override(path: String) -> Option<String> {
    if path.trim().is_empty() {
        return None;
    }
    let expanded = shellexpand_home(&path);
    std::fs::read_to_string(expanded).ok()
}

fn shellexpand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"))
        && let Some(home) = directories::UserDirs::new()
    {
        return home.home_dir().join(rest);
    }
    PathBuf::from(path)
}

/// Abre el selector nativo de carpetas (para `init --path`, ver
/// docs/fase1-ingesta.md). Bloqueante: la GUI nunca lanza dos subprocesos de
/// escritura en paralelo sobre la misma BD, y elegir carpeta es una acción
/// sincrónica del usuario.
#[tauri::command]
fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|p| p.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            run_photoranker,
            read_theme_config,
            read_theme_override,
            pick_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
