use std::io::{self, Write};
use std::fs;
use std::path::PathBuf;
use crate::models::Config;

fn config_file_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(format!("{}/Library/Application Support/MacXtreamer/xtream_config.txt", home))
}

pub fn read_config() -> Result<Config, io::Error> {
    // Primär aus ~/Library/Application Support/... lesen, bei Bedarf auf lokale Datei zurückfallen
    let primary = config_file_path();
    let content = match fs::read_to_string(&primary) {
        Ok(s) => s,
        Err(_e) => fs::read_to_string("xtream_config.txt")?,
    };
    let mut cfg = Config::default();
    cfg.reuse_vlc = true; // default
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "address" => cfg.address = v.trim().to_string(),
                "username" => cfg.username = v.trim().to_string(),
                "password" => cfg.password = v.trim().to_string(),
                "player_command" => cfg.player_command = v.trim().to_string(),
                "theme" => cfg.theme = v.trim().to_string(),
                "cover_ttl_days" => cfg.cover_ttl_days = v.trim().parse::<u32>().unwrap_or(7),
                "cover_parallel" => cfg.cover_parallel = v.trim().parse::<u32>().unwrap_or(6),
                "font_scale" => cfg.font_scale = v.trim().parse::<f32>().unwrap_or(1.15),
                "download_dir" => cfg.download_dir = v.trim().to_string(),
                "reuse_vlc" => cfg.reuse_vlc = v.trim().parse::<u8>().map(|n| n != 0).unwrap_or(true),
                "cover_uploads_per_frame" => cfg.cover_uploads_per_frame = v.trim().parse::<u32>().unwrap_or(3),
                "cover_decode_parallel" => cfg.cover_decode_parallel = v.trim().parse::<u32>().unwrap_or(2),
                "texture_cache_limit" => cfg.texture_cache_limit = v.trim().parse::<u32>().unwrap_or(512),
                "category_parallel" => cfg.category_parallel = v.trim().parse::<u32>().unwrap_or(6),
                "cover_height" => cfg.cover_height = v.trim().parse::<f32>().unwrap_or(60.0),
                "enable_downloads" => cfg.enable_downloads = v.trim().parse::<u8>().map(|n| n != 0).unwrap_or(false),
                _ => {}
            }
        }
    }
    if cfg.download_dir.trim().is_empty() {
        if let Ok(home) = std::env::var("HOME") {
            cfg.download_dir = format!("{}/Downloads/macxtreamer", home);
        }
    }
    Ok(cfg)
}

pub fn save_config(cfg: &Config) -> Result<(), io::Error> {
    let path = config_file_path();
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let mut f = fs::File::create(path)?;
    if !cfg.address.is_empty() { writeln!(f, "address={}", cfg.address)?; }
    if !cfg.username.is_empty() { writeln!(f, "username={}", cfg.username)?; }
    if !cfg.password.is_empty() { writeln!(f, "password={}", cfg.password)?; }
    if !cfg.player_command.is_empty() { writeln!(f, "player_command={}", cfg.player_command)?; }
    if !cfg.theme.is_empty() { writeln!(f, "theme={}", cfg.theme)?; }
    if cfg.cover_ttl_days != 0 { writeln!(f, "cover_ttl_days={}", cfg.cover_ttl_days)?; }
    if cfg.cover_parallel != 0 { writeln!(f, "cover_parallel={}", cfg.cover_parallel)?; }
    if cfg.font_scale != 0.0 { writeln!(f, "font_scale={:.2}", cfg.font_scale)?; }
    if !cfg.download_dir.is_empty() { writeln!(f, "download_dir={}", cfg.download_dir)?; }
    writeln!(f, "reuse_vlc={}", if cfg.reuse_vlc { 1 } else { 0 })?;
    if cfg.cover_uploads_per_frame != 0 { writeln!(f, "cover_uploads_per_frame={}", cfg.cover_uploads_per_frame)?; }
    if cfg.cover_decode_parallel != 0 { writeln!(f, "cover_decode_parallel={}", cfg.cover_decode_parallel)?; }
    if cfg.texture_cache_limit != 0 { writeln!(f, "texture_cache_limit={}", cfg.texture_cache_limit)?; }
    if cfg.category_parallel != 0 { writeln!(f, "category_parallel={}", cfg.category_parallel)?; }
    if cfg.cover_height != 0.0 { writeln!(f, "cover_height={:.1}", cfg.cover_height)?; }
    writeln!(f, "enable_downloads={}", if cfg.enable_downloads { 1 } else { 0 })?;
    Ok(())
}
