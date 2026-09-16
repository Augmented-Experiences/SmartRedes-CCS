// App de escritorio nativa SmartSuite (Tauri v2)
//
// Arranca el backend FastAPI empaquetado (sidecar `backend`), prepara
// Ollama (API HTTP local: sistema en 11434, binario portable, o descarga zip/tgz)
// y muestra el progreso en la pantalla de carga. Cuando todo está listo, la
// ventana carga la UI real sin reiniciar. Al cerrar, detiene el sidecar y
// únicamente el Ollama que esta app haya arrancado.
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use smartsuite_ollama as ollama_portable;

use std::io::{BufRead, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

// --- Config por-herramienta (generada por scripts/configure.mjs) ---
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tier {
    max_ram_gb: f64,
    model: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    product_name: String,
    data_dir_name: String,
    ollama_tiers: Vec<Tier>,
    /// Reservado por el kit (SmartRedes de escritorio no descarga modelos de imagen).
    #[serde(default)]
    #[allow(dead_code)]
    extra_models: Vec<String>,
}

static APP_CONFIG_JSON: &str = include_str!("../appconfig.json");

fn app_config() -> &'static AppConfig {
    static CFG: OnceLock<AppConfig> = OnceLock::new();
    CFG.get_or_init(|| serde_json::from_str(APP_CONFIG_JSON).expect("appconfig.json inválido"))
}

fn product_name() -> &'static str {
    app_config().product_name.as_str()
}

/// Proceso hijo del backend (para terminarlo al salir).
struct BackendState(Mutex<Option<CommandChild>>);

/// Ollama arrancado por esta instancia (PID propio; nunca un Ollama de sistema).
enum OwnedOllama {
    Child(Child),
    Pid(u32),
}

impl OwnedOllama {
    fn pid(&self) -> u32 {
        match self {
            OwnedOllama::Child(child) => child.id(),
            OwnedOllama::Pid(pid) => *pid,
        }
    }
}

struct OllamaState(Mutex<Option<OwnedOllama>>);

/// Estado compartido que se muestra en la pantalla de carga.
#[derive(Clone, serde::Serialize)]
struct Status {
    /// "starting" | "ollama" | "download-engine" | "extract" | "start" | "downloading" | "ready" | "warning"
    phase: String,
    message: String,
    /// 0–100, o -1 si es indeterminado.
    percent: i32,
    /// URL de la UI real (cuando el backend responde).
    backend_url: Option<String>,
    /// El backend ya responde: se puede entrar.
    can_continue: bool,
    /// El paso de Ollama terminó (éxito, omitido o fallo no fatal).
    ollama_done: bool,
}

impl Status {
    fn initial() -> Self {
        Status {
            phase: "starting".into(),
            message: "Iniciando servicios…".into(),
            percent: -1,
            backend_url: None,
            can_continue: false,
            ollama_done: false,
        }
    }
}

struct AppStatus(Mutex<Status>);

/// Puerto HTTP del sidecar FastAPI (fijado en `setup`).
struct BackendPort(u16);

/// Ruta que devuelve 200 cuando FastAPI está sirviendo (SmartGastos/SmartCaja).
const BACKEND_HEALTH_PATH: &str = "/api/health";
/// Entrada de la UI web empaquetada (StaticFiles en `server/app.py`).
const BACKEND_UI_PATH: &str = "/ui/";

/// Actualiza el estado compartido y lo emite a la pantalla de carga.
fn update_status(app: &tauri::AppHandle, f: impl FnOnce(&mut Status)) {
    let snapshot = {
        let state = app.state::<AppStatus>();
        let mut s = state.0.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    let _ = app.emit("status", snapshot);
}

/// Comando invocable desde la pantalla de carga para obtener el estado actual
/// (evita perder actualizaciones si la página carga tarde).
#[tauri::command]
fn current_status(state: tauri::State<AppStatus>) -> Status {
    state.0.lock().unwrap().clone()
}

fn remember_owned_ollama(app: &tauri::AppHandle, owned: OwnedOllama) {
    let pid = owned.pid();
    ollama_portable::write_owned_pid(&user_data_dir(), pid);
    ollama_log(&format!("Ollama propio registrado (PID {pid})."));
    app.state::<OllamaState>().0.lock().unwrap().replace(owned);
}

fn home_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    }
}

/// Carpeta de datos por-usuario (coincide con la del backend).
fn user_data_dir() -> PathBuf {
    let app = app_config().data_dir_name.as_str();
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join("AppData").join("Roaming"));
        base.join(app)
    }
    #[cfg(target_os = "macos")]
    {
        home_dir()
            .join("Library")
            .join("Application Support")
            .join(app)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".local").join("share"));
        base.join(app)
    }
}

fn log_dir() -> PathBuf {
    let dir = user_data_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Añade una línea de estado al log de Ollama.
fn ollama_log(line: &str) {
    let path = log_dir().join("ollama.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{}", line);
    }
}

/// Elige un puerto libre para el backend (prefiere 7860, luego efímero).
fn pick_port() -> u16 {
    for p in [7860u16, 7861, 7862, 7863] {
        if TcpListener::bind(("127.0.0.1", p)).is_ok() {
            return p;
        }
    }
    TcpListener::bind(("127.0.0.1", 0))
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .unwrap_or(7860)
}

/// Espera hasta que el puerto acepte conexiones (Ollama u otros servicios TCP).
fn wait_for_port(port: u16, attempts: u32) -> bool {
    for _ in 0..attempts {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

fn backend_health_url(port: u16) -> String {
    format!("http://127.0.0.1:{}{}", port, BACKEND_HEALTH_PATH)
}

fn backend_ui_url(port: u16) -> String {
    format!("http://127.0.0.1:{}{}", port, BACKEND_UI_PATH)
}

/// Espera hasta que `GET /api/health` responda 200 (no basta con TCP al puerto).
fn wait_for_backend_ready(port: u16, attempts: u32) -> bool {
    let url = backend_health_url(port);
    for _ in 0..attempts {
        if ureq::get(&url)
            .call()
            .map(|r| r.status() == 200)
            .unwrap_or(false)
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

fn apply_backend_ready(app: &tauri::AppHandle, port: u16) {
    let url = backend_ui_url(port);
    update_status(app, |s| {
        s.backend_url = Some(url);
        s.can_continue = true;
        // No tapar descarga/extracción/pull de Ollama: el splash espera ollama_done.
        if s.ollama_done {
            s.phase = "ready".into();
            if !s.message.to_lowercase().contains("no se pudo") {
                s.message = "Servicios listos.".into();
            }
        }
    });
}

/// Modelo de Ollama recomendado según la RAM total del equipo y la config
/// por-herramienta (tiers en appconfig.json).
fn model_for_ram() -> String {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let gb = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let cfg = app_config();
    for t in &cfg.ollama_tiers {
        if t.max_ram_gb > 0.0 && gb < t.max_ram_gb {
            return t.model.clone();
        }
    }
    cfg.ollama_tiers
        .last()
        .map(|t| t.model.clone())
        .unwrap_or_else(|| "llama3.2:3b".to_string())
}

/// Descarga el modelo vía la API de Ollama (`/api/pull`) mostrando el progreso.
/// Devuelve true si el modelo quedó disponible.
fn pull_model_with_progress(app: &tauri::AppHandle, agent: &ureq::Agent, port: u16, model: &str) -> bool {
    let body = format!("{{\"name\":\"{}\"}}", model);
    let url = format!("{}/api/pull", ollama_portable::api_base(port));
    let resp = agent
        .post(&url)
        .set("Content-Type", "application/json")
        .send_string(&body);
    let resp = match resp {
        Ok(r) => r,
        Err(_) => return false,
    };
    let reader = std::io::BufReader::new(resp.into_reader());
    let mut ok = false;
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("error").is_some() {
            ok = false;
            break;
        }
        let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
        let total = v.get("total").and_then(|t| t.as_u64());
        let completed = v.get("completed").and_then(|c| c.as_u64());
        let pct: i32 = match (total, completed) {
            (Some(t), Some(c)) if t > 0 => ((c.min(t) * 100) / t) as i32,
            _ => -1,
        };
        let msg = if pct >= 0 {
            format!("Descargando el modelo {} — {}%", model, pct)
        } else {
            format!("Preparando el modelo {} ({})…", model, status)
        };
        ollama_log(&msg);
        update_status(app, |s| {
            s.phase = "downloading".into();
            s.message = msg.clone();
            s.percent = pct;
        });
        if status == "success" {
            ok = true;
        }
    }
    ok
}

fn wait_for_ollama_api(agent: &ureq::Agent, port: u16, attempts: u32) -> bool {
    wait_for_port(port, attempts) && {
        for _ in 0..attempts {
            if ollama_portable::api_healthy(agent, port) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        ollama_portable::api_healthy(agent, port)
    }
}

fn start_owned_ollama(
    app: &tauri::AppHandle,
    binary: &Path,
    user_data: &Path,
    portable: bool,
    port: u16,
) -> bool {
    update_status(app, |s| {
        s.phase = "start".into();
        s.message = "Iniciando el servicio de IA…".into();
        s.percent = -1;
    });
    ollama_log(&format!(
        "Iniciando '{}' serve en 127.0.0.1:{port} (portable={portable})",
        binary.display()
    ));
    let mut cmd = ollama_portable::ollama_command(binary);
    if portable {
        ollama_portable::configure_portable_serve(&mut cmd, binary, user_data, port);
    } else {
        cmd.arg("serve");
        cmd.env("OLLAMA_HOST", format!("127.0.0.1:{port}"));
    }
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    match cmd.spawn() {
        Ok(child) => {
            remember_owned_ollama(app, OwnedOllama::Child(child));
            let agent = ollama_portable::http_agent();
            if wait_for_ollama_api(&agent, port, 40) {
                true
            } else {
                ollama_log("Ollama arrancó pero la API HTTP no respondió a tiempo.");
                false
            }
        }
        Err(error) => {
            ollama_log(&format!("No se pudo iniciar ollama serve: {error}"));
            false
        }
    }
}

fn ensure_portable_binary(
    app: &tauri::AppHandle,
    agent: &ureq::Agent,
    user_data: &Path,
) -> Result<PathBuf, String> {
    let home = ollama_portable::ollama_home(user_data);
    if let Some(existing) = ollama_portable::find_portable_binary(&home) {
        ollama_portable::ensure_executable(&existing);
        ollama_log(&format!(
            "Binario portable reutilizado: {}",
            existing.display()
        ));
        return Ok(existing);
    }

    let mut last_error = "no se encontró un archivo portable de Ollama".to_string();
    for spec in ollama_portable::archive_candidates() {
        ollama_log(&format!("Probando archivo portable {}", spec.url));
        if spec.filename.to_ascii_lowercase().contains("setup")
            || spec.filename.to_ascii_lowercase().ends_with(".msi")
            || spec.filename.to_ascii_lowercase().contains("ollamasetup")
        {
            continue;
        }

        update_status(app, |s| {
            s.phase = "download-engine".into();
            s.message = format!("Descargando Ollama ({})…", spec.filename);
            s.percent = 0;
        });

        let archive = home.join("cache").join(spec.filename);
        match ollama_portable::download_file(agent, spec.url, &archive, |copied, total| {
            let pct = match total {
                Some(t) if t > 0 => ((copied.min(t) * 100) / t) as i32,
                _ => -1,
            };
            let msg = if pct >= 0 {
                format!("Descargando Ollama — {pct}%")
            } else {
                format!("Descargando Ollama ({copied} bytes)…")
            };
            update_status(app, |s| {
                s.phase = "download-engine".into();
                s.message = msg;
                s.percent = pct;
            });
        }) {
            Ok(()) => {}
            Err(error) => {
                ollama_log(&format!("Descarga fallida ({}): {error}", spec.url));
                last_error = error;
                continue;
            }
        }

        update_status(app, |s| {
            s.phase = "extract".into();
            s.message = "Extrayendo Ollama…".into();
            s.percent = -1;
        });
        ollama_log(&format!("Extrayendo {} en {}", archive.display(), home.display()));
        if let Err(error) = ollama_portable::extract_archive(&archive, &home) {
            ollama_log(&format!("Extracción fallida: {error}"));
            last_error = error;
            continue;
        }

        if let Some(binary) = ollama_portable::find_portable_binary(&home) {
            ollama_portable::ensure_executable(&binary);
            ollama_log(&format!("Binario portable listo: {}", binary.display()));
            return Ok(binary);
        }
        last_error = "el archivo de Ollama no contenía el binario esperado".into();
    }
    Err(last_error)
}

/// Prepara Ollama (sistema en 11434, portable previo, o descarga zip/tgz) sin reiniciar.
fn bootstrap_ollama(app: tauri::AppHandle, backend_port: u16) {
    std::thread::spawn(move || {
        update_status(&app, |s| {
            s.phase = "ollama".into();
            s.message = "Verificando el motor de IA (Ollama)…".into();
            s.percent = -1;
        });
        ollama_log(&format!(
            "== {}: preparando el motor de IA (Ollama portable) ==",
            product_name()
        ));

        let agent = ollama_portable::http_agent();
        let data_dir = user_data_dir();
        let port = ollama_portable::DEFAULT_PORT;
        let mut ready = false;

        if ollama_portable::api_healthy(&agent, port) {
            if let Some(pid) = ollama_portable::read_owned_pid(&data_dir) {
                if ollama_portable::pid_looks_like_ollama(pid) {
                    ollama_log(&format!(
                        "Reutilizando Ollama portable ya en ejecución (PID {pid})."
                    ));
                    remember_owned_ollama(&app, OwnedOllama::Pid(pid));
                } else {
                    ollama_portable::clear_owned_pid(&data_dir);
                    ollama_log("Ollama de sistema ya escuchaba en 11434; no se tomará posesión.");
                }
            } else {
                ollama_log("Ollama de sistema ya escuchaba en 11434; no se detendrá al salir.");
            }
            ready = true;
        } else if let Some(pid) = ollama_portable::read_owned_pid(&data_dir) {
            if ollama_portable::pid_looks_like_ollama(pid) {
                ollama_log(&format!(
                    "PID propio {pid} sigue vivo pero la API no responde; se reinicia."
                ));
                ollama_portable::kill_owned_pid(pid);
            }
            ollama_portable::clear_owned_pid(&data_dir);
        }

        if !ready {
            let home = ollama_portable::ollama_home(&data_dir);
            if let Some(portable) = ollama_portable::find_portable_binary(&home) {
                ollama_portable::ensure_executable(&portable);
                ready = start_owned_ollama(&app, &portable, &data_dir, true, port);
            }
        }

        if !ready {
            if let Some(system_bin) = ollama_portable::path_has_ollama() {
                ollama_log("Ollama está en PATH; se arranca sin instalador MSI.");
                ready = start_owned_ollama(&app, &system_bin, &data_dir, false, port);
            }
        }

        if !ready {
            match ensure_portable_binary(&app, &agent, &data_dir) {
                Ok(binary) => {
                    ready = start_owned_ollama(&app, &binary, &data_dir, true, port);
                }
                Err(error) => {
                    ollama_log(&format!("No se pudo obtener Ollama portable: {error}"));
                    update_status(&app, |s| {
                        s.phase = "warning".into();
                        s.message = format!(
                            "No se pudo preparar Ollama ({error}). La app abrirá; la IA quedará desconectada."
                        );
                        s.percent = -1;
                    });
                }
            }
        }

        if ready {
            ollama_log("Servicio Ollama disponible (API HTTP).");
            let ram_hint = model_for_ram();
            let model = ollama_portable::TEXT_MODEL.to_string();
            if ram_hint != model {
                ollama_log(&format!(
                    "RAM sugeriría {ram_hint}; SmartRedes usa el modelo de texto {model}."
                ));
            }
            if ollama_portable::model_already_pulled(&agent, port, &model) {
                update_status(&app, |s| {
                    s.phase = "downloading".into();
                    s.message = format!("Modelo {model} listo.");
                    s.percent = 100;
                });
            } else {
                update_status(&app, |s| {
                    s.phase = "downloading".into();
                    s.message = format!("Descargando el modelo {model} (solo la primera vez)…");
                    s.percent = -1;
                });
                let ok = pull_model_with_progress(&app, &agent, port, &model);
                if ok {
                    ollama_log(&format!("Modelo '{model}' listo."));
                    update_status(&app, |s| {
                        s.message = format!("Modelo {model} listo.");
                        s.percent = 100;
                    });
                } else {
                    ollama_log(&format!("No se pudo descargar '{model}' automáticamente."));
                    update_status(&app, |s| {
                        s.phase = "warning".into();
                        s.message = format!(
                            "No se pudo descargar {model} automáticamente; podrás reintentar desde la app."
                        );
                        s.percent = -1;
                    });
                }
            }
        }

        let backend_ready = app.state::<AppStatus>().0.lock().unwrap().can_continue;
        if !backend_ready && wait_for_backend_ready(backend_port, 600) {
            apply_backend_ready(&app, backend_port);
        }

        update_status(&app, |s| {
            s.ollama_done = true;
            if s.can_continue && s.backend_url.is_some() {
                s.phase = "ready".into();
            }
        });
    });
}

/// Detiene el sidecar y únicamente el Ollama que esta instancia arrancó.
fn shutdown_services(app: &tauri::AppHandle) {
    if let Some(child) = app.state::<BackendState>().0.lock().unwrap().take() {
        let _ = child.kill();
    }
    if let Some(owned) = app.state::<OllamaState>().0.lock().unwrap().take() {
        let pid = owned.pid();
        ollama_log(&format!("Deteniendo Ollama propio (PID {pid})."));
        match owned {
            OwnedOllama::Child(mut child) => {
                ollama_portable::kill_owned_pid(pid);
                let _ = child.kill();
                let _ = child.wait();
            }
            OwnedOllama::Pid(pid) => {
                if ollama_portable::pid_looks_like_ollama(pid) {
                    ollama_portable::kill_owned_pid(pid);
                }
            }
        }
        ollama_portable::clear_owned_pid(&user_data_dir());
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(BackendState(Mutex::new(None)))
        .manage(OllamaState(Mutex::new(None)))
        .manage(AppStatus(Mutex::new(Status::initial())))
        .invoke_handler(tauri::generate_handler![current_status])
        .setup(|app| {
            let handle = app.handle().clone();
            let port = pick_port();
            app.manage(BackendPort(port));

            // 1) Lanzar el backend empaquetado (sidecar), pasándole el puerto y la
            //    carpeta de datos por entorno (compatible con las apps del SmartSuite).
            let data_dir = user_data_dir().to_string_lossy().to_string();
            let (mut rx, child) = app
                .shell()
                .sidecar("backend")
                .expect("no se encontró el sidecar 'backend'")
                .env("PORT", port.to_string())
                .env("DATA_DIR", data_dir)
                .env(
                    "OLLAMA_URL",
                    ollama_portable::api_base(ollama_portable::DEFAULT_PORT),
                )
                .spawn()
                .expect("no se pudo iniciar el backend");
            app.state::<BackendState>()
                .0
                .lock()
                .unwrap()
                .replace(child);

            // Drenar la salida del backend (evita bloqueos del buffer).
            tauri::async_runtime::spawn(async move {
                while let Some(event) = rx.recv().await {
                    if let CommandEvent::Stderr(bytes) | CommandEvent::Stdout(bytes) = event {
                        let _ = String::from_utf8_lossy(&bytes);
                    }
                }
            });

            // 2) Preparar Ollama con progreso en pantalla.
            bootstrap_ollama(handle.clone(), port);

            // 3) Cuando FastAPI responda en /api/health, habilitar el ingreso a la UI.
            let ready_handle = handle.clone();
            std::thread::spawn(move || {
                // Hasta ~30 min (arranque lento + descargas Ollama en paralelo).
                if wait_for_backend_ready(port, 3600) {
                    apply_backend_ready(&ready_handle, port);
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al construir la app de escritorio")
        .run(|app_handle, event| {
            match event {
                RunEvent::WindowEvent {
                    event: WindowEvent::CloseRequested { api, .. },
                    ..
                } => {
                    api.prevent_close();
                    shutdown_services(app_handle);
                    app_handle.exit(0);
                }
                RunEvent::Exit => shutdown_services(app_handle),
                _ => {}
            }
        });
}
