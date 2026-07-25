use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::ImageFormat;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

use crate::config::Config;

const EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "tiff"];

#[derive(Debug, Clone)]
pub struct WallpaperEntry {
    pub full_path: PathBuf,
    pub file_name: String,
    pub thumb_path: PathBuf,
}

pub fn scan_wallpapers(dir: &str) -> Result<Vec<PathBuf>> {
    let base = Path::new(dir);
    if !base.exists() {
        anyhow::bail!("Folder wallpaper tidak ditemukan: {}", dir);
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(base)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                files.push(path.to_path_buf());
            }
        }
    }
    files.sort();
    Ok(files)
}

fn cache_key(path: &Path) -> Result<String> {
    let meta = fs::metadata(path)?;
    let mtime = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(mtime.to_le_bytes());
    let hash = hasher.finalize();
    Ok(format!("{:x}", hash)[..24].to_string())
}

pub fn get_or_create_thumbnail(path: &Path, size: u32) -> Result<PathBuf> {
    let cache_dir = Config::cache_dir()?;
    let key = cache_key(path)?;
    let thumb_path = cache_dir.join(format!("{}_{}.png", key, size));

    if thumb_path.exists() {
        return Ok(thumb_path);
    }

    let img = image::ImageReader::open(path)
        .with_context(|| format!("Gagal buka gambar {:?}", path))?
        .with_guessed_format()
        .with_context(|| format!("Gagal deteksi format {:?}", path))?
        .decode()
        .with_context(|| format!("Gagal decode gambar {:?}", path))?;

    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        anyhow::bail!("Gambar kosong: {:?}", path);
    }

    let (new_w, new_h) = if w > h {
        (size, (size as u64 * h as u64 / w as u64).max(1) as u32)
    } else {
        ((size as u64 * w as u64 / h as u64).max(1) as u32, size)
    };

    let thumb = img.resize_exact(new_w, new_h, FilterType::Triangle);

    let file = fs::File::create(&thumb_path)
        .with_context(|| format!("Gagal buat file thumbnail {:?}", thumb_path))?;
    let writer = BufWriter::new(file);
    thumb.write_to(writer, ImageFormat::Png)
        .with_context(|| format!("Gagal simpan thumbnail ke {:?}", thumb_path))?;

    Ok(thumb_path)
}

pub fn build_entry(path: &Path, thumb_size: u32) -> Option<WallpaperEntry> {
    let thumb_path = get_or_create_thumbnail(path, thumb_size).ok()?;
    let file_name = path.file_name()?.to_string_lossy().to_string();
    Some(WallpaperEntry {
        full_path: path.to_path_buf(),
        file_name,
        thumb_path,
    })
}


