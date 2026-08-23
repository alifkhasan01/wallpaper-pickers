use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::config::Config;

fn cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".cache")
        .join("wallpaper")
}

fn write_cache(path: &Path) -> Result<()> {
    let dir = cache_dir();
    fs::create_dir_all(&dir)?;

    let current = dir.join("current");
    fs::write(&current, path.to_string_lossy().as_bytes())?;

    let link = dir.join("hyprlock-bg");
    let _ = fs::remove_file(&link);
    unix_fs::symlink(path, &link)?;

    Ok(())
}

pub fn ensure_daemon_running() -> Result<()> {
    let status = Command::new("awww")
        .arg("query")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let running = matches!(status, Ok(s) if s.success());

    if !running {
        Command::new("awww-daemon")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        thread::sleep(Duration::from_millis(400));
    }

    Ok(())
}

pub fn set_wallpaper(path: &Path, cfg: &Config) -> Result<()> {
    ensure_daemon_running()?;

    let output = Command::new("awww")
        .arg("img")
        .arg(path)
        .arg("--transition-type")
        .arg(&cfg.transition_type)
        .arg("--transition-duration")
        .arg(cfg.transition_duration.to_string())
        .arg("--transition-fps")
        .arg(cfg.transition_fps.to_string())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("awww gagal set wallpaper: {}", stderr.trim());
    }

    write_cache(path)?;

    Ok(())
}

/// Wallpaper yang sedang aktif — tanya langsung ke daemon via `awww query`,
/// fallback ke cache `~/.cache/wallpaper/current` (ditulis saat set).
pub fn get_current_wallpaper() -> Option<PathBuf> {
    if let Ok(output) = Command::new("awww")
        .arg("query")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some(p) = extract_path(line) {
                    let path = PathBuf::from(&p);
                    if path.exists() {
                        return Some(path);
                    }
                }
            }
        }
    }

    // Fallback: cache lokal yang ditulis write_cache()
    if let Ok(s) = fs::read_to_string(cache_dir().join("current")) {
        let s = s.trim();
        if !s.is_empty() {
            let path = PathBuf::from(s);
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

/// Ambil path absolut dari satu baris output `awww query`.
/// Mendukung format `path=/x/y.jpg`, `"output with image /x/y.jpg"`, dll.
fn extract_path(line: &str) -> Option<String> {
    let candidate = match line.find("path=") {
        Some(i) => line[i + 5..].to_string(),
        None => line[line.find('/')?..].to_string(),
    };

    let candidate = candidate
        .trim()
        .trim_matches('"')
        .trim_end_matches(',')
        .trim_matches('"')
        .to_string();

    if candidate.is_empty() || !candidate.starts_with('/') {
        return None;
    }
    Some(candidate)
}

pub fn check_binaries_available() -> Result<()> {
    for bin in ["awww", "awww-daemon"] {
        let found = Command::new("which")
            .arg(bin)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !found {
            bail!(
                "Binary `{}` tidak ketemu di PATH. Install awww dulu (AUR: awww, atau awww-git).",
                bin
            );
        }
    }
    Ok(())
}

pub fn start_background_slideshow() -> Result<()> {
    let exe = std::env::current_exe().context("Gagal menentukan path binary")?;

    let child = Command::new(&exe)
        .arg("--slideshow-bg")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()?;

    let pid_path = Config::slideshow_pid_path()?;
    fs::write(&pid_path, child.id().to_string())?;

    Ok(())
}

pub fn stop_background_slideshow() -> Result<()> {
    let pid_path = Config::slideshow_pid_path()?;
    if pid_path.exists() {
        let pid_str = fs::read_to_string(&pid_path)?;
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            let _ = Command::new("kill")
                .arg(pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = fs::remove_file(&pid_path);
    }
    Ok(())
}

pub fn is_background_slideshow_running() -> bool {
    let pid_path = match Config::slideshow_pid_path() {
        Ok(p) => p,
        Err(_) => return false,
    };
    if !pid_path.exists() {
        return false;
    }
    let pid_str = match fs::read_to_string(&pid_path) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return false,
    };
    let pid: i32 = match pid_str.parse() {
        Ok(p) => p,
        Err(_) => return false,
    };
    Path::new(&format!("/proc/{}", pid)).exists()
}
