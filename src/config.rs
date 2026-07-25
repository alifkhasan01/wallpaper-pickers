use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Folder tempat wallpaper disimpan
    pub wallpaper_dir: String,
    /// Tipe transisi awww: simple, fade, wipe, wave, grow, center, outer, random
    pub transition_type: String,
    /// Durasi transisi (detik)
    pub transition_duration: f32,
    /// FPS transisi
    pub transition_fps: u32,
    /// Ukuran thumbnail (px)
    pub thumb_size: u32,
    /// Jumlah kolom grid
    pub columns: u32,
}

impl Default for Config {
    fn default() -> Self {
        let wallpaper_dir = dirs::picture_dir()
            .map(|p| p.join("Wallpapers"))
            .unwrap_or_else(|| PathBuf::from("~/Pictures/Wallpapers"))
            .to_string_lossy()
            .to_string();

        Self {
            wallpaper_dir,
            transition_type: "wipe".to_string(),
            transition_duration: 1.0,
            transition_fps: 60,
            thumb_size: 220,
            columns: 4,
        }
    }
}

impl Config {
    fn config_path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Gagal menemukan config dir")?
            .join("wallpicker");
        fs::create_dir_all(&dir)?;
        Ok(dir.join("config.json"))
    }

    pub fn cache_dir() -> Result<PathBuf> {
        let dir = dirs::cache_dir()
            .context("Gagal menemukan cache dir")?
            .join("wallpicker")
            .join("thumbs");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            let cfg = Self::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Gagal baca config di {:?}", path))?;
        let cfg: Config = serde_json::from_str(&content)
            .context("Config JSON tidak valid, cek format file")?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)
            .with_context(|| format!("Gagal simpan config ke {:?}", path))?;
        Ok(())
    }
}
