//! Portable Ollama bootstrap (no MSI / system installer).
//!
//! The engine and model store are shared by SmartGastos, SmartRedes, and
//! SmartCaja under `SmartSuite/ollama`. Per-app markers stay in the product
//! data dir. Official zip/tgz from ollama.com is downloaded only into the
//! shared engine home.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Text model SmartRedes already requires (`server/app.py` REQUIRED_MODEL).
pub const TEXT_MODEL: &str = "llama3.1:8b";

pub const DEFAULT_PORT: u16 = 11434;

pub struct ArchiveSpec {
    pub url: &'static str,
    pub filename: &'static str,
}

/// Official Ollama archives. Prefer zip/tgz; Linux latest may only ship tar.zst.
pub fn archive_candidates() -> Vec<ArchiveSpec> {
    #[cfg(all(windows, target_arch = "aarch64"))]
    {
        return vec![
            ArchiveSpec {
                url: "https://ollama.com/download/ollama-windows-arm64.zip",
                filename: "ollama-windows-arm64.zip",
            },
            ArchiveSpec {
                url: "https://github.com/ollama/ollama/releases/latest/download/ollama-windows-arm64.zip",
                filename: "ollama-windows-arm64.zip",
            },
        ];
    }
    #[cfg(all(windows, not(target_arch = "aarch64")))]
    {
        return vec![
            ArchiveSpec {
                url: "https://ollama.com/download/ollama-windows-amd64.zip",
                filename: "ollama-windows-amd64.zip",
            },
            ArchiveSpec {
                url: "https://github.com/ollama/ollama/releases/latest/download/ollama-windows-amd64.zip",
                filename: "ollama-windows-amd64.zip",
            },
        ];
    }
    #[cfg(target_os = "macos")]
    {
        return vec![
            ArchiveSpec {
                url: "https://ollama.com/download/ollama-darwin.tgz",
                filename: "ollama-darwin.tgz",
            },
            ArchiveSpec {
                url: "https://github.com/ollama/ollama/releases/latest/download/ollama-darwin.tgz",
                filename: "ollama-darwin.tgz",
            },
        ];
    }
    #[cfg(all(not(windows), not(target_os = "macos"), target_arch = "aarch64"))]
    {
        return vec![
            ArchiveSpec {
                url: "https://ollama.com/download/ollama-linux-arm64.tgz",
                filename: "ollama-linux-arm64.tgz",
            },
            ArchiveSpec {
                url: "https://github.com/ollama/ollama/releases/latest/download/ollama-linux-arm64.tar.zst",
                filename: "ollama-linux-arm64.tar.zst",
            },
            ArchiveSpec {
                url: "https://github.com/ollama/ollama/releases/download/v0.13.5/ollama-linux-arm64.tgz",
                filename: "ollama-linux-arm64.tgz",
            },
        ];
    }
    #[cfg(all(not(windows), not(target_os = "macos"), not(target_arch = "aarch64")))]
    {
        vec![
            ArchiveSpec {
                url: "https://ollama.com/download/ollama-linux-amd64.tgz",
                filename: "ollama-linux-amd64.tgz",
            },
            ArchiveSpec {
                url: "https://github.com/ollama/ollama/releases/latest/download/ollama-linux-amd64.tar.zst",
                filename: "ollama-linux-amd64.tar.zst",
            },
            ArchiveSpec {
                url: "https://github.com/ollama/ollama/releases/download/v0.13.5/ollama-linux-amd64.tgz",
                filename: "ollama-linux-amd64.tgz",
            },
        ]
    }
}

pub fn archive_spec() -> ArchiveSpec {
    archive_candidates()
        .into_iter()
        .next()
        .expect("al menos un archivo oficial de Ollama")
}

const SUITE_DIR_NAME: &str = "SmartSuite";
const LEGACY_APP_NAMES: &[&str] = &["SmartGastos", "SmartRedes", "SmartCaja"];

/// Per-app Ollama state (`active_model.txt`, `active_profile.json`).
pub fn ollama_home(user_data: &Path) -> PathBuf {
    user_data.join("ollama")
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

/// Same base directory rules as the app data dir, without the product folder.
pub fn platform_data_base() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join("AppData").join("Roaming"))
    }
    #[cfg(target_os = "macos")]
    {
        home_dir()
            .join("Library")
            .join("Application Support")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".local").join("share"))
    }
}

pub fn shared_engine_home_under(data_base: &Path) -> PathBuf {
    data_base.join(SUITE_DIR_NAME).join("ollama")
}

pub fn legacy_engine_homes_under(data_base: &Path) -> Vec<PathBuf> {
    LEGACY_APP_NAMES
        .iter()
        .map(|name| data_base.join(name).join("ollama"))
        .collect()
}

/// `%APPDATA%\SmartSuite\ollama` (or the platform equivalent).
pub fn engine_home() -> PathBuf {
    shared_engine_home_under(&platform_data_base())
}

pub fn legacy_engine_homes() -> Vec<PathBuf> {
    legacy_engine_homes_under(&platform_data_base())
}

pub fn shared_models_dir() -> PathBuf {
    engine_home().join("models")
}

/// Shared model directory. `user_data` is unused; app markers stay in `ollama_home`.
pub fn models_dir(_user_data: &Path) -> PathBuf {
    shared_models_dir()
}

fn pid_file_in(engine: &Path) -> PathBuf {
    engine.join("owned.pid")
}

pub fn owned_pid_path(_user_data: &Path) -> PathBuf {
    pid_file_in(&engine_home())
}

pub fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent("SmartRedes-CCS/1.0 (portable-ollama)")
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(180))
        .timeout_write(Duration::from_secs(60))
        .build()
}

pub fn api_base(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

pub fn api_healthy(agent: &ureq::Agent, port: u16) -> bool {
    let url = format!("{}/api/tags", api_base(port));
    agent
        .get(&url)
        .timeout(Duration::from_secs(3))
        .call()
        .map(|r| r.status() == 200)
        .unwrap_or(false)
}

pub fn tags_have_model(body: &str, model: &str) -> bool {
    let v: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let Some(models) = v.get("models").and_then(|m| m.as_array()) else {
        return false;
    };
    models.iter().any(|m| {
        m.get("name")
            .and_then(|n| n.as_str())
            .map(|n| n == model || n.starts_with(&format!("{model}-")) || n.starts_with(&format!("{model}:")))
            .unwrap_or(false)
            || m.get("model")
                .and_then(|n| n.as_str())
                .map(|n| n == model)
                .unwrap_or(false)
    })
}

pub fn model_already_pulled(agent: &ureq::Agent, port: u16, model: &str) -> bool {
    let url = format!("{}/api/tags", api_base(port));
    match agent.get(&url).call() {
        Ok(resp) if resp.status() == 200 => {
            let body = resp.into_string().unwrap_or_default();
            tags_have_model(&body, model)
        }
        _ => false,
    }
}

/// Locate a previously extracted portable binary under the product ollama dir.
pub fn find_portable_binary(home: &Path) -> Option<PathBuf> {
    if !home.exists() {
        return None;
    }
    let exe = if cfg!(windows) { "ollama.exe" } else { "ollama" };
    let candidates = [
        home.join("bin").join(exe),
        home.join(exe),
        home.join("runtime").join(exe),
        home.join("ollama").join("bin").join(exe),
        home.join("ollama").join(exe),
    ];
    for path in candidates {
        if is_runnable(&path) {
            return Some(path);
        }
    }
    walk_for_binary(home, exe, 0)
}

fn is_runnable(path: &Path) -> bool {
    path.is_file()
}

fn walk_for_binary(dir: &Path, exe: &str, depth: u8) -> Option<PathBuf> {
    if depth > 4 {
        return None;
    }
    let entries = fs::read_dir(dir).ok()?;
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if fname == "active_model.txt" || fname == "active_profile.json" {
                continue;
            }
            if fname == exe {
                return Some(path);
            }
        } else if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "models" || name == "cache" || name == "logs" {
                continue;
            }
            dirs.push(path);
        }
    }
    for path in dirs {
        if let Some(found) = walk_for_binary(&path, exe, depth + 1) {
            return Some(found);
        }
    }
    None
}

pub fn path_has_ollama() -> Option<PathBuf> {
    let mut cmd = Command::new("ollama");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let ok = cmd
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        Some(PathBuf::from("ollama"))
    } else {
        None
    }
}

fn curl_command() -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("curl.exe");
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new("curl")
    }
}

fn content_length_hint(url: &str) -> Option<u64> {
    let output = curl_command()
        .args(["-sI", "-L", "--max-time", "20", url])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("content-length:"))
        .filter_map(|line| line.split(':').nth(1)?.trim().parse::<u64>().ok())
        .last()
}

pub fn download_file(
    _agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("no se pudo crear {parent:?}: {e}"))?;
    }
    let tmp = dest.with_extension("partial");
    let _ = fs::remove_file(&tmp);
    let total = content_length_hint(url);
    on_progress(0, total);

    let mut child = curl_command()
        .args([
            "-fL",
            "--retry",
            "3",
            "--retry-delay",
            "2",
            "--connect-timeout",
            "30",
            "-o",
        ])
        .arg(&tmp)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("no se pudo ejecutar curl para descargar Ollama: {e}"))?;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let _ = fs::remove_file(&tmp);
                    return Err(format!(
                        "curl no pudo descargar Ollama ({status}) desde {url}"
                    ));
                }
                break;
            }
            Ok(None) => {
                let copied = fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
                on_progress(copied, total);
                std::thread::sleep(Duration::from_millis(400));
            }
            Err(error) => {
                let _ = child.kill();
                return Err(format!("error esperando la descarga de Ollama: {error}"));
            }
        }
    }

    let copied = fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    on_progress(copied, total.or(Some(copied)));
    if copied == 0 {
        let _ = fs::remove_file(&tmp);
        return Err("la descarga de Ollama llegó vacía".into());
    }
    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    fs::rename(&tmp, dest).map_err(|e| format!("no se pudo finalizar la descarga: {e}"))?;
    Ok(())
}

pub fn extract_archive(archive: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("no se pudo crear {dest:?}: {e}"))?;
    let mut cmd = Command::new("tar");
    cmd.arg("-xf").arg(archive).arg("-C").arg(dest);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    match cmd.status() {
        Ok(status) if status.success() => return Ok(()),
        Ok(_) | Err(_) => {}
    }

    let name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".tar.zst") || name.ends_with(".zst") {
        let mut zstd = Command::new("zstd");
        zstd.args(["-d", "-c"]).arg(archive);
        #[cfg(windows)]
        zstd.creation_flags(CREATE_NO_WINDOW);
        zstd.stdout(Stdio::piped());
        if let Ok(mut decoder) = zstd.spawn() {
            let mut tar = Command::new("tar");
            tar.args(["-xf", "-", "-C"]).arg(dest);
            #[cfg(windows)]
            tar.creation_flags(CREATE_NO_WINDOW);
            if let Some(stdout) = decoder.stdout.take() {
                tar.stdin(stdout);
                if let Ok(status) = tar.status() {
                    let _ = decoder.wait();
                    if status.success() {
                        return Ok(());
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    {
        if name.ends_with(".zip") {
            return expand_zip_powershell(archive, dest);
        }
    }
    Err(format!(
        "no se pudo extraer {} (tar/zstd no disponibles o archivo dañado)",
        archive.display()
    ))
}

#[cfg(windows)]
fn expand_zip_powershell(archive: &Path, dest: &Path) -> Result<(), String> {
    let archive = archive.to_string_lossy().replace('\'', "''");
    let dest = dest.to_string_lossy().replace('\'', "''");
    let script = format!(
        "Expand-Archive -Force -LiteralPath '{archive}' -DestinationPath '{dest}'"
    );
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    let status = cmd
        .status()
        .map_err(|e| format!("Expand-Archive falló: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Expand-Archive salió con {status}"))
    }
}

pub fn ensure_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(perms.mode() | 0o755);
            let _ = fs::set_permissions(path, perms);
        }
    }
    let _ = path;
}

fn write_pid_at(engine: &Path, pid: u32) {
    let path = pid_file_in(engine);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, format!("{pid}\n"));
}

fn read_pid_at(engine: &Path) -> Option<u32> {
    let text = fs::read_to_string(pid_file_in(engine)).ok()?;
    text.trim().parse().ok()
}

fn clear_pid_at(engine: &Path) {
    let _ = fs::remove_file(pid_file_in(engine));
}

pub fn write_owned_pid(_user_data: &Path, pid: u32) {
    write_pid_at(&engine_home(), pid);
}

pub fn read_owned_pid(_user_data: &Path) -> Option<u32> {
    read_pid_at(&engine_home())
}

pub fn clear_owned_pid(_user_data: &Path) {
    clear_pid_at(&engine_home());
}

pub fn pid_looks_like_ollama(pid: u32) -> bool {
    if !pid_is_running(pid) {
        return false;
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("tasklist");
        cmd.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        return match cmd.output() {
            Ok(output) => {
                String::from_utf8_lossy(&output.stdout)
                    .to_ascii_lowercase()
                    .contains("ollama")
            }
            Err(_) => false,
        };
    }
    #[cfg(unix)]
    {
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .output();
        match output {
            Ok(out) => String::from_utf8_lossy(&out.stdout)
                .to_ascii_lowercase()
                .contains("ollama"),
            Err(_) => false,
        }
    }
}

pub fn pid_is_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("tasklist");
        cmd.args(["/FI", &format!("PID eq {pid}"), "/NH"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        return match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                output.status.success()
                    && stdout.contains(&pid.to_string())
                    && !stdout.to_ascii_lowercase().contains("no tasks")
            }
            Err(_) => false,
        };
    }
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Kill only this PID (and its children). Never `pkill ollama` / image-wide taskkill.
pub fn kill_owned_pid(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.status();
    }
    #[cfg(unix)]
    {
        let grouped = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !grouped {
            let _ = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        std::thread::sleep(Duration::from_millis(400));
        let _ = Command::new("kill")
            .args(["-KILL", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn path_key(path: &Path) -> String {
    let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut text = canon.to_string_lossy().to_string();
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        text = rest.to_string();
    }
    #[cfg(windows)]
    {
        text.replace('/', "\\").to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        text
    }
}

/// True when this exact binary path is the image of a running process.
pub fn binary_image_running(binary: &Path) -> bool {
    let want = path_key(binary);
    running_ollama_images().iter().any(|running| path_key(running) == want)
}

fn running_ollama_images() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-CimInstance Win32_Process -Filter \"Name='ollama.exe'\" | Select-Object -ExpandProperty ExecutablePath",
        ]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = match cmd.output() {
            Ok(output) if output.status.success() => output,
            _ => return Vec::new(),
        };
        return String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect();
    }
    #[cfg(target_os = "linux")]
    {
        let mut found = Vec::new();
        let Ok(entries) = fs::read_dir("/proc") else {
            return found;
        };
        for entry in entries.flatten() {
            let exe = entry.path().join("exe");
            if let Ok(target) = fs::read_link(&exe) {
                if target.file_name().and_then(|n| n.to_str()) == Some("ollama") {
                    found.push(target);
                }
            }
        }
        found
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ps")
            .args(["-axww", "-o", "command="])
            .output();
        let Ok(output) = output else {
            return Vec::new();
        };
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let token = line.split_whitespace().next()?;
                let path = Path::new(token);
                if path.file_name().and_then(|n| n.to_str()) == Some("ollama") {
                    Some(path.to_path_buf())
                } else {
                    None
                }
            })
            .collect()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Vec::new()
    }
}

fn engine_file_stays(rel: &Path) -> bool {
    for component in rel.components() {
        if let std::path::Component::Normal(name) = component {
            if name == "models" || name == "logs" {
                return true;
            }
        }
    }
    matches!(
        rel.file_name().and_then(|n| n.to_str()),
        Some("active_model.txt")
            | Some("active_profile.json")
            | Some("owned.pid")
            | Some("migrate.lock")
            | Some("desktop_status.json")
            | Some("config.json")
    )
}

fn move_engine_tree(from: &Path, to: &Path) {
    move_engine_dir(from, from, to);
}

fn move_engine_dir(root: &Path, dir: &Path, dest_root: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        if engine_file_stays(rel) {
            continue;
        }
        if path.is_dir() {
            move_engine_dir(root, &path, dest_root);
            let _ = fs::remove_dir(&path);
        } else if path.is_file() {
            let target = dest_root.join(rel);
            if target.exists() {
                continue;
            }
            if let Some(parent) = target.parent() {
                if fs::create_dir_all(parent).is_err() {
                    continue;
                }
            }
            if fs::rename(&path, &target).is_err() {
                // Locked or busy: leave the source for a later launch.
            }
        }
    }
}

/// Use a portable binary from the shared home, or adopt the first legacy home
/// that has one. A running legacy binary stays in place.
pub fn adopt_legacy_engine(engine: &Path, legacy_homes: &[PathBuf]) -> Option<PathBuf> {
    adopt_legacy_engine_with(engine, legacy_homes, binary_image_running)
}

fn adopt_legacy_engine_with(
    engine: &Path,
    legacy_homes: &[PathBuf],
    mut is_running: impl FnMut(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(binary) = find_portable_binary(engine) {
        return Some(binary);
    }
    for home in legacy_homes {
        let Some(binary) = find_portable_binary(home) else {
            continue;
        };
        if is_running(&binary) {
            return Some(binary);
        }
        move_engine_tree(home, engine);
        if let Some(moved) = find_portable_binary(engine) {
            return Some(moved);
        }
        return Some(binary);
    }
    None
}

/// Zip download is unnecessary when the shared home or any legacy home already
/// has a portable binary.
pub fn engine_download_needed(engine: &Path, legacy_homes: &[PathBuf]) -> bool {
    if find_portable_binary(engine).is_some() {
        return false;
    }
    !legacy_homes.iter().any(|home| find_portable_binary(home).is_some())
}

fn dirs_match(left: &Path, right: &Path) -> bool {
    path_key(left) == path_key(right)
}

fn model_sources(engine: &Path, legacy_homes: &[PathBuf], app_data: &Path) -> Vec<PathBuf> {
    let dest = engine.join("models");
    let mut sources: Vec<PathBuf> = Vec::new();
    for home in legacy_homes {
        let models = home.join("models");
        if models.is_dir() && !dirs_match(&models, &dest) && !sources.iter().any(|p| dirs_match(p, &models))
        {
            sources.push(models);
        }
    }
    let app_models = ollama_home(app_data).join("models");
    if app_models.is_dir()
        && !dirs_match(&app_models, &dest)
        && !sources.iter().any(|p| dirs_match(p, &app_models))
    {
        sources.push(app_models);
    }
    sources
}

fn file_len(path: &Path) -> Option<u64> {
    fs::metadata(path).ok().map(|meta| meta.len())
}

fn reconcile_model_file(src: &Path, dest: &Path) {
    let Some(src_len) = file_len(src) else {
        return;
    };
    if dest.exists() {
        if file_len(dest) == Some(src_len) {
            let _ = fs::remove_file(src);
        }
        return;
    }
    if let Some(parent) = dest.parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if fs::rename(src, dest).is_err() {
        // Source is locked. Leave it and keep going.
    }
}

fn import_tree(src_dir: &Path, dest_dir: &Path) {
    let Ok(entries) = fs::read_dir(src_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let target = dest_dir.join(entry.file_name());
        if path.is_dir() {
            import_tree(&path, &target);
            let _ = fs::remove_dir(&path);
        } else if path.is_file() {
            reconcile_model_file(&path, &target);
        }
    }
}

fn import_blobs_and_manifests(source_models: &Path, dest_models: &Path) {
    for name in ["blobs", "manifests"] {
        let src = source_models.join(name);
        if src.is_dir() {
            import_tree(&src, &dest_models.join(name));
        }
    }
}

#[cfg(unix)]
extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}

fn try_lock_file(path: &Path) -> Option<File> {
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return None;
        }
    }
    #[cfg(windows)]
    {
        return OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .share_mode(0)
            .open(path)
            .ok();
    }
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .ok()?;
        // LOCK_EX | LOCK_NB
        let rc = unsafe { flock(file.as_raw_fd(), 2 | 4) };
        if rc == 0 {
            Some(file)
        } else {
            None
        }
    }
}

fn wait_for_migrate_lock(path: &Path, wait: Duration) -> Option<File> {
    let start = Instant::now();
    loop {
        if let Some(file) = try_lock_file(path) {
            return Some(file);
        }
        if start.elapsed() >= wait {
            return None;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Move legacy `blobs` and `manifests` into the shared models dir.
/// If another app holds `migrate.lock`, wait up to `wait` and then return
/// without failing.
pub fn import_models_into(
    engine: &Path,
    legacy_homes: &[PathBuf],
    app_data: &Path,
    wait: Duration,
) {
    let sources = model_sources(engine, legacy_homes, app_data);
    if sources.is_empty() {
        return;
    }
    let Some(_lock) = wait_for_migrate_lock(&engine.join("migrate.lock"), wait) else {
        return;
    };
    let dest = engine.join("models");
    for source in sources {
        import_blobs_and_manifests(&source, &dest);
    }
}

pub fn import_shared_models(app_data: &Path) {
    import_models_into(
        &engine_home(),
        &legacy_engine_homes(),
        app_data,
        Duration::from_secs(60),
    );
}

pub fn ollama_command(binary: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(binary);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    cmd
}

/// Env for every `ollama serve` this app starts, including one found on PATH.
pub fn apply_serve_env(cmd: &mut Command, port: u16) {
    let models = shared_models_dir();
    let _ = fs::create_dir_all(&models);
    cmd.env("OLLAMA_HOST", format!("127.0.0.1:{port}"));
    cmd.env("OLLAMA_MODELS", &models);
}

/// Apply env so a portable serve stores models in the shared SmartSuite dir.
pub fn configure_portable_serve(cmd: &mut Command, binary: &Path, _user_data: &Path, port: u16) {
    cmd.arg("serve");
    apply_serve_env(cmd, port);
    if let Some(bin_dir) = binary.parent() {
        let path_var = if cfg!(windows) { "PATH" } else { "PATH" };
        let sep = if cfg!(windows) { ";" } else { ":" };
        let current = std::env::var_os(path_var).unwrap_or_default();
        let mut new_path = bin_dir.as_os_str().to_os_string();
        new_path.push(sep);
        if let Some(prefix) = bin_dir.parent() {
            new_path.push(prefix);
            new_path.push(sep);
        }
        new_path.push(current);
        cmd.env(path_var, new_path);
        #[cfg(unix)]
        {
            if let Some(prefix) = bin_dir.parent() {
                let lib = prefix.join("lib");
                if lib.is_dir() {
                    let mut ld = lib.into_os_string();
                    ld.push(":");
                    if let Some(old) = std::env::var_os("LD_LIBRARY_PATH") {
                        ld.push(old);
                    }
                    cmd.env("LD_LIBRARY_PATH", ld);
                }
            }
        }
        let _ = cmd.current_dir(bin_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn official_archive_is_portable_not_msi() {
        let specs = archive_candidates();
        assert!(!specs.is_empty());
        for spec in &specs {
            assert!(!spec.filename.to_ascii_lowercase().contains("setup"));
            assert!(!spec.filename.to_ascii_lowercase().contains("msi"));
            assert!(!spec.url.to_ascii_lowercase().contains("ollamasetup"));
        }
        let spec = archive_spec();
        assert!(spec.url.contains("ollama.com/download") || spec.url.contains("github.com/ollama"));
        #[cfg(windows)]
        {
            assert!(spec.filename.ends_with(".zip"), "{}", spec.filename);
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            assert!(
                spec.filename.ends_with(".tgz")
                    || spec.filename.ends_with(".tar.gz")
                    || spec.filename.ends_with(".tar.zst"),
                "{}",
                spec.filename
            );
            assert!(
                specs.iter().any(|s| s.filename.ends_with(".tgz")),
                "Linux must keep a tgz fallback"
            );
        }
    }

    #[test]
    fn text_model_is_product_default() {
        assert_eq!(TEXT_MODEL, "llama3.1:8b");
    }

    #[test]
    fn tags_detect_llama31() {
        let body = r#"{"models":[{"name":"llama3.1:8b","size":1}]}"#;
        assert!(tags_have_model(body, "llama3.1:8b"));
        assert!(!tags_have_model(body, "moondream"));
        assert!(!tags_have_model("not-json", "llama3.1:8b"));
    }

    #[test]
    fn find_binary_prefers_bin_dir() {
        let root = std::env::temp_dir().join(format!(
            "smartredes-ollama-find-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("models")).unwrap();
        let exe = if cfg!(windows) { "ollama.exe" } else { "ollama" };
        fs::write(root.join("bin").join(exe), b"fake").unwrap();
        fs::write(root.join("models").join(exe), b"ignore").unwrap();
        let found = find_portable_binary(&root).expect("binary");
        assert_eq!(found, root.join("bin").join(exe));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn extract_tgz_roundtrip() {
        if cfg!(windows) {
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "smartredes-ollama-tgz-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let src = root.join("src");
        let dest = root.join("dest");
        fs::create_dir_all(src.join("bin")).unwrap();
        let mut f = File::create(src.join("bin").join("ollama")).unwrap();
        f.write_all(b"#!/bin/sh\necho ok\n").unwrap();
        drop(f);
        let archive = root.join("ollama-linux-amd64.tgz");
        let status = Command::new("tar")
            .args([
                "-czf",
                archive.to_str().unwrap(),
                "-C",
                src.to_str().unwrap(),
                "bin",
            ])
            .status()
            .expect("tar create");
        assert!(status.success());
        extract_archive(&archive, &dest).expect("extract");
        let found = find_portable_binary(&dest).expect("found after extract");
        assert!(found.ends_with("ollama"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn download_file_via_curl_file_url() {
        let root = std::env::temp_dir().join(format!(
            "smartredes-ollama-dl-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let src = root.join("src.bin");
        fs::write(&src, b"hello-portable-ollama").unwrap();
        let dest = root.join("out.bin");
        let url = format!("file://{}", src.display());
        download_file(&http_agent(), &url, &dest, |_, _| {}).expect("curl file download");
        assert_eq!(fs::read(&dest).unwrap(), b"hello-portable-ollama");
        let _ = fs::remove_dir_all(&root);
    }

    fn exe_name() -> &'static str {
        if cfg!(windows) { "ollama.exe" } else { "ollama" }
    }

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "smartredes-ollama-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn engine_home_is_smartsuite_and_active_model_stays_in_app() {
        let base = scratch("paths");
        let engine = shared_engine_home_under(&base);
        assert_eq!(engine, base.join("SmartSuite").join("ollama"));
        assert_eq!(
            engine.join("models"),
            base.join("SmartSuite").join("ollama").join("models")
        );
        let app = base.join("SmartRedes");
        let marker = ollama_home(&app).join("active_model.txt");
        assert_eq!(
            marker,
            base.join("SmartRedes").join("ollama").join("active_model.txt")
        );
        assert_eq!(
            pid_file_in(&engine),
            base.join("SmartSuite").join("ollama").join("owned.pid")
        );
        let homes = legacy_engine_homes_under(&base);
        assert_eq!(homes[0], base.join("SmartGastos").join("ollama"));
        assert_eq!(homes[1], base.join("SmartRedes").join("ollama"));
        assert_eq!(homes[2], base.join("SmartCaja").join("ollama"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn owned_pid_roundtrip_under_engine_home() {
        let base = scratch("pid");
        let engine = shared_engine_home_under(&base);
        write_pid_at(&engine, 4242);
        assert_eq!(read_pid_at(&engine), Some(4242));
        assert!(pid_file_in(&engine).is_file());
        assert!(!base.join("SmartRedes").join("ollama").join("owned.pid").exists());
        clear_pid_at(&engine);
        assert_eq!(read_pid_at(&engine), None);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn active_model_txt_is_not_a_binary() {
        let root = scratch("marker");
        fs::write(root.join("active_model.txt"), b"llama3.2:3b\n").unwrap();
        fs::create_dir_all(root.join("models")).unwrap();
        fs::write(root.join("models").join(exe_name()), b"ignore").unwrap();
        assert!(find_portable_binary(&root).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn legacy_binary_suppresses_engine_download() {
        let base = scratch("legacy-bin");
        let engine = shared_engine_home_under(&base);
        let legacy = legacy_engine_homes_under(&base);
        let redes = &legacy[1];
        fs::create_dir_all(redes.join("runtime")).unwrap();
        fs::write(redes.join("runtime").join(exe_name()), b"portable").unwrap();
        fs::write(redes.join("active_model.txt"), b"llama3.2:3b\n").unwrap();
        assert!(!engine_download_needed(&engine, &legacy));
        assert!(find_portable_binary(&engine).is_none());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn adopt_moves_stopped_legacy_engine_and_keeps_app_state() {
        let base = scratch("adopt");
        let engine = shared_engine_home_under(&base);
        let legacy = legacy_engine_homes_under(&base);
        let redes = &legacy[1];
        fs::create_dir_all(redes.join("runtime")).unwrap();
        fs::write(redes.join("runtime").join(exe_name()), b"portable").unwrap();
        fs::create_dir_all(redes.join("models").join("blobs")).unwrap();
        fs::write(redes.join("models").join("blobs").join("sha"), b"blob").unwrap();
        fs::write(redes.join("active_model.txt"), b"llama3.2:3b\n").unwrap();
        fs::write(redes.join("active_profile.json"), b"{}\n").unwrap();
        fs::create_dir_all(redes.join("logs")).unwrap();
        fs::write(redes.join("logs").join("ollama.log"), b"log").unwrap();

        let found = adopt_legacy_engine_with(&engine, &legacy, |_| false).expect("binary");
        assert_eq!(found, engine.join("runtime").join(exe_name()));
        assert!(redes.join("active_model.txt").is_file());
        assert!(redes.join("active_profile.json").is_file());
        assert!(redes.join("logs").join("ollama.log").is_file());
        assert!(redes.join("models").join("blobs").join("sha").is_file());
        assert!(!engine.join("models").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn running_legacy_binary_stays_in_place() {
        let base = scratch("running");
        let engine = shared_engine_home_under(&base);
        let legacy = legacy_engine_homes_under(&base);
        let redes = &legacy[1];
        fs::create_dir_all(redes.join("bin")).unwrap();
        let binary = redes.join("bin").join(exe_name());
        fs::write(&binary, b"portable").unwrap();
        let found = adopt_legacy_engine_with(&engine, &legacy, |_| true).expect("binary");
        assert_eq!(found, binary);
        assert!(!engine.join("bin").join(exe_name()).exists());
        assert!(!engine_download_needed(&engine, &legacy));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn import_moves_blobs_and_reconciles_size() {
        let base = scratch("models");
        let engine = shared_engine_home_under(&base);
        let legacy = legacy_engine_homes_under(&base);
        let redes = legacy[1].join("models");
        fs::create_dir_all(redes.join("blobs")).unwrap();
        fs::create_dir_all(redes.join("manifests").join("library")).unwrap();
        fs::write(redes.join("blobs").join("new"), b"aaaa").unwrap();
        fs::write(redes.join("blobs").join("same"), b"bbbb").unwrap();
        fs::write(redes.join("blobs").join("diff"), b"cccc").unwrap();
        fs::write(redes.join("manifests").join("library").join("llama"), b"manifest").unwrap();
        fs::create_dir_all(engine.join("models").join("blobs")).unwrap();
        fs::write(engine.join("models").join("blobs").join("same"), b"bbbb").unwrap();
        fs::write(engine.join("models").join("blobs").join("diff"), b"zz").unwrap();

        import_models_into(&engine, &legacy, &base.join("SmartRedes"), Duration::from_secs(5));

        assert_eq!(
            fs::read(engine.join("models").join("blobs").join("new")).unwrap(),
            b"aaaa"
        );
        assert!(!redes.join("blobs").join("new").exists());
        assert!(!redes.join("blobs").join("same").exists());
        assert_eq!(
            fs::read(engine.join("models").join("blobs").join("diff")).unwrap(),
            b"zz"
        );
        assert_eq!(fs::read(redes.join("blobs").join("diff")).unwrap(), b"cccc");
        assert_eq!(
            fs::read(engine.join("models").join("manifests").join("library").join("llama")).unwrap(),
            b"manifest"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn migrate_lock_waits_then_continues() {
        let base = scratch("lock");
        let engine = shared_engine_home_under(&base);
        let legacy = legacy_engine_homes_under(&base);
        let blobs = legacy[0].join("models").join("blobs");
        fs::create_dir_all(&blobs).unwrap();
        fs::write(blobs.join("sha"), b"data").unwrap();
        let lock_path = engine.join("migrate.lock");
        let held = try_lock_file(&lock_path).expect("lock");
        import_models_into(&engine, &legacy, &base.join("SmartGastos"), Duration::ZERO);
        assert!(blobs.join("sha").is_file());
        assert!(!engine.join("models").join("blobs").join("sha").exists());
        drop(held);
        import_models_into(&engine, &legacy, &base.join("SmartGastos"), Duration::from_secs(2));
        assert_eq!(
            fs::read(engine.join("models").join("blobs").join("sha")).unwrap(),
            b"data"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[test]
    fn locked_model_file_is_left_in_place() {
        use std::os::windows::fs::OpenOptionsExt;
        let base = scratch("locked-blob");
        let engine = shared_engine_home_under(&base);
        let legacy = legacy_engine_homes_under(&base);
        let blobs = legacy[2].join("models").join("blobs");
        fs::create_dir_all(&blobs).unwrap();
        let src = blobs.join("sha");
        fs::write(&src, b"locked-blob").unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&src)
            .expect("hold blob");
        import_models_into(&engine, &legacy, &base.join("SmartCaja"), Duration::from_secs(2));
        assert!(src.is_file(), "locked source stays");
        assert!(!engine.join("models").join("blobs").join("sha").exists());
        drop(held);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn existing_tag_suppresses_pull() {
        let body = r#"{"models":[{"name":"llama3.1:8b","model":"llama3.1:8b"}]}"#;
        assert!(tags_have_model(body, "llama3.1:8b"));
        assert!(!tags_have_model(body, "llama3.2:3b"));
        assert!(!tags_have_model(body, "moondream"));
    }
}
