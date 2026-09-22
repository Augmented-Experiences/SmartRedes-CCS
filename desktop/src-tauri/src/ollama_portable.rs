//! Portable Ollama bootstrap (no MSI / system installer).
//!
//! Official zip/tgz from ollama.com is downloaded into the product data dir,
//! extracted, and served locally. Only a process this app started is owned.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

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

pub fn ollama_home(user_data: &Path) -> PathBuf {
    user_data.join("ollama")
}

pub fn models_dir(user_data: &Path) -> PathBuf {
    ollama_home(user_data).join("models")
}

pub fn owned_pid_path(user_data: &Path) -> PathBuf {
    ollama_home(user_data).join("owned.pid")
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
            if path.file_name().and_then(|n| n.to_str()) == Some(exe) {
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

pub fn write_owned_pid(user_data: &Path, pid: u32) {
    let path = owned_pid_path(user_data);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, format!("{pid}\n"));
}

pub fn read_owned_pid(user_data: &Path) -> Option<u32> {
    let text = fs::read_to_string(owned_pid_path(user_data)).ok()?;
    text.trim().parse().ok()
}

pub fn clear_owned_pid(user_data: &Path) {
    let _ = fs::remove_file(owned_pid_path(user_data));
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

pub fn ollama_command(binary: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(binary);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    cmd
}

/// Apply env so a portable serve stores models under the product data dir.
pub fn configure_portable_serve(cmd: &mut Command, binary: &Path, user_data: &Path, port: u16) {
    cmd.arg("serve");
    cmd.env("OLLAMA_HOST", format!("127.0.0.1:{port}"));
    cmd.env("OLLAMA_MODELS", models_dir(user_data));
    let _ = fs::create_dir_all(models_dir(user_data));
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

    #[test]
    fn owned_pid_roundtrip() {
        let root = std::env::temp_dir().join(format!(
            "smartredes-ollama-pid-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        write_owned_pid(&root, 4242);
        assert_eq!(read_owned_pid(&root), Some(4242));
        clear_owned_pid(&root);
        assert_eq!(read_owned_pid(&root), None);
        let _ = fs::remove_dir_all(&root);
    }
}
