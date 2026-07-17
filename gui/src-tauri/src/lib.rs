//! Backend Tauri de PhotoRanker: la GUI nunca implementa lógica de negocio ni
//! lee `.photoranker.sqlite` directamente (ver docs/conventions.md, "API
//! interna"); este módulo solo (a) invoca `photoranker(.exe)` como subproceso
//! y devuelve el sobre JSON de stdout tal cual, y (b) lee `config.toml` para
//! resolver el tema embebido/override (ver docs/fase5-gui.md, "Theming").

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
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

/// Procesos `photoranker` lanzados por `run_photoranker_async`, indexados por
/// el `op_id` que genera el frontend (ver api/asyncCli.ts) — permite que
/// `cancel_photoranker` los mate por PID sin depender de un ID generado acá
/// (evita una carrera entre "Rust genera el id" y "el evento ya llegó al
/// frontend antes de que conozca ese id").
#[derive(Default)]
struct RunningOps(Mutex<HashMap<String, Arc<Mutex<Child>>>>);

/// `op_id`s cancelados explícitamente por el usuario — así el hilo que
/// espera la salida del proceso puede distinguir "lo maté yo" de "se cayó
/// solo" al armar el evento `photoranker-done` (Windows no expone esa
/// distinción en `ExitStatus`).
#[derive(Default)]
struct CancelledOps(Mutex<HashSet<String>>);

#[derive(Clone, Serialize)]
struct ProcessLogEvent {
    op_id: String,
    stream: &'static str,
    line: String,
}

#[derive(Clone, Serialize)]
struct ProcessDoneEvent {
    op_id: String,
    /// Sobre JSON final (última línea no vacía de stdout), si se pudo parsear.
    envelope: Option<serde_json::Value>,
    /// Mensaje de error de transporte (el proceso no pudo esperarse, etc.) —
    /// distinto de un envelope `{"status":"error",...}`, que sí es `envelope`.
    error: Option<String>,
    cancelled: bool,
}

/// Versión con streaming + cancelación de `run_photoranker`, hoy usada solo
/// para `init` (ver docs/fase5-gui.md) — la GUI muestra en vivo el archivo
/// que se está procesando (stderr trae los logs de `tracing`) y puede
/// cancelar sin esperar a que termine. El sobre JSON final se emite en el
/// evento `photoranker-done`, no en el valor de retorno de este comando — el
/// valor de retorno es solo un ack de que el proceso arrancó. `op_id` lo
/// genera el frontend (`crypto.randomUUID()`) para que pueda suscribirse a
/// los eventos ANTES de invocar este comando, sin ventana de carrera.
///
/// **Deliberadamente NO se usa para `cluster --preview`/`--k`**: ese comando
/// bloquea esperando a `Rscript` como subproceso *hijo* de `photoranker`, y
/// `cancel_photoranker` mata por PID — en Windows, matar el proceso padre no
/// mata a sus hijos (no hay cascada sin Job Objects), así que cancelar
/// dejaría un `Rscript.exe` huérfano reteniendo el lock WAL de la BD. `init`
/// es seguro de cancelar porque su paralelismo es con hilos (`rayon`) dentro
/// del mismo proceso, no subprocesos.
#[tauri::command]
fn run_photoranker_async(
    app: AppHandle,
    running: State<RunningOps>,
    op_id: String,
    args: Vec<String>,
) -> Result<(), String> {
    let cli_path = resolve_cli_path()?;
    let mut child = Command::new(&cli_path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("No se pudo ejecutar '{}': {e}", cli_path.display()))?;

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    let child_arc = Arc::new(Mutex::new(child));
    running
        .0
        .lock()
        .unwrap()
        .insert(op_id.clone(), child_arc.clone());

    let last_stdout_line = Arc::new(Mutex::new(String::new()));

    let stdout_app = app.clone();
    let stdout_op_id = op_id.clone();
    let last_line_writer = last_stdout_line.clone();
    let stdout_thread = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if !line.trim().is_empty() {
                *last_line_writer.lock().unwrap() = line.clone();
            }
            let _ = stdout_app.emit(
                "photoranker-log",
                ProcessLogEvent {
                    op_id: stdout_op_id.clone(),
                    stream: "stdout",
                    line,
                },
            );
        }
    });

    let stderr_app = app.clone();
    let stderr_op_id = op_id.clone();
    let stderr_thread = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = stderr_app.emit(
                "photoranker-log",
                ProcessLogEvent {
                    op_id: stderr_op_id.clone(),
                    stream: "stderr",
                    line,
                },
            );
        }
    });

    let wait_app = app.clone();
    let wait_op_id = op_id.clone();
    std::thread::spawn(move || {
        let _ = stdout_thread.join();
        let _ = stderr_thread.join();
        let status = child_arc.lock().unwrap().wait();

        wait_app
            .state::<RunningOps>()
            .0
            .lock()
            .unwrap()
            .remove(&wait_op_id);
        let cancelled = wait_app
            .state::<CancelledOps>()
            .0
            .lock()
            .unwrap()
            .remove(&wait_op_id);

        let line = last_stdout_line.lock().unwrap().clone();
        let envelope = serde_json::from_str::<serde_json::Value>(&line).ok();
        let error = status.err().map(|e| e.to_string());

        let _ = wait_app.emit(
            "photoranker-done",
            ProcessDoneEvent {
                op_id: wait_op_id,
                envelope,
                error,
                cancelled,
            },
        );
    });

    Ok(())
}

/// Mata el proceso `photoranker` de `op_id` si todavía está corriendo (ver
/// `run_photoranker_async`). Devuelve `false` si ya había terminado — no es
/// un error, la GUI simplemente no llega a tiempo de cancelar.
#[tauri::command]
fn cancel_photoranker(
    running: State<RunningOps>,
    cancelled: State<CancelledOps>,
    op_id: String,
) -> Result<bool, String> {
    cancelled.0.lock().unwrap().insert(op_id.clone());
    let child = running.0.lock().unwrap().get(&op_id).cloned();
    match child {
        Some(child) => {
            child.lock().unwrap().kill().map_err(|e| e.to_string())?;
            Ok(true)
        }
        None => {
            cancelled.0.lock().unwrap().remove(&op_id);
            Ok(false)
        }
    }
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

/// Nombre fijo del archivo de override que genera la propia pantalla de
/// Ajustes (ver `views/Settings.ts`) — distinto de cualquier `theme_path`
/// que el usuario haya apuntado a mano a un archivo propio, para poder
/// avisarle antes de tomar control de esa clave (ver docs/fase5-gui.md).
const GUI_ACCENT_FILENAME: &str = "gui-accent.css";

fn photoranker_config_dir() -> Result<PathBuf, String> {
    ProjectDirs::from("", "", "photoranker")
        .map(|dirs| dirs.config_dir().to_path_buf())
        .ok_or_else(|| "No se pudo determinar el directorio de configuración".to_string())
}

/// Actualiza solo la clave `theme` de `config.toml`, preservando el resto de
/// claves que gestiona el CLI (`core-cli/src/config.rs`) — nunca reescribe
/// el archivo entero con un `ThemeConfig` parcial, que borraría el resto de
/// la configuración del usuario.
#[tauri::command]
fn write_theme_config(theme: String) -> Result<(), String> {
    let dir = photoranker_config_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let config_path = dir.join("config.toml");

    let mut parsed: toml::Value = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_else(|| toml::Value::Table(Default::default()));

    let table = parsed
        .as_table_mut()
        .ok_or_else(|| "config.toml no tiene la forma esperada (tabla TOML)".to_string())?;
    table.insert("theme".to_string(), toml::Value::String(theme));

    let serialized = toml::to_string_pretty(&parsed).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, serialized).map_err(|e| e.to_string())
}

/// Escribe el CSS de acento generado por la pantalla de Ajustes a un archivo
/// fijo (`gui-accent.css`) y actualiza `theme_path` en `config.toml` para que
/// apunte ahí — reutiliza el mecanismo de override ya existente
/// (`read_theme_override`) en vez de inventar un segundo camino de theming.
#[tauri::command]
fn write_theme_override(css: String) -> Result<(), String> {
    let dir = photoranker_config_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let accent_path = dir.join(GUI_ACCENT_FILENAME);
    std::fs::write(&accent_path, css).map_err(|e| e.to_string())?;

    let config_path = dir.join("config.toml");
    let mut parsed: toml::Value = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_else(|| toml::Value::Table(Default::default()));
    let table = parsed
        .as_table_mut()
        .ok_or_else(|| "config.toml no tiene la forma esperada (tabla TOML)".to_string())?;
    table.insert(
        "theme_path".to_string(),
        toml::Value::String(accent_path.to_string_lossy().to_string()),
    );

    let serialized = toml::to_string_pretty(&parsed).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, serialized).map_err(|e| e.to_string())
}

/// Para que la pantalla de Ajustes pueda advertir antes de pisar un
/// `theme_path` que el usuario haya apuntado a mano a un archivo propio (ver
/// docs/fase5-gui.md, nota de diseño de la pantalla de Ajustes).
#[tauri::command]
fn theme_path_is_gui_managed(theme_path: String) -> bool {
    if theme_path.trim().is_empty() {
        return true;
    }
    let expanded = shellexpand_home(&theme_path);
    expanded.file_name().and_then(|n| n.to_str()) == Some(GUI_ACCENT_FILENAME)
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
        .manage(RunningOps::default())
        .manage(CancelledOps::default())
        .invoke_handler(tauri::generate_handler![
            run_photoranker,
            run_photoranker_async,
            cancel_photoranker,
            read_theme_config,
            read_theme_override,
            write_theme_config,
            write_theme_override,
            theme_path_is_gui_managed,
            pick_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
