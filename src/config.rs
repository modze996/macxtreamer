use std::io::{self, Write};
use std::fs;
use std::path::PathBuf;
use crate::models::Config;
use base64::{Engine as _, engine::general_purpose};

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
    // Enhanced defaults for VLC buffering - optimized for live TV stability (10+ seconds total buffering)
    cfg.vlc_network_caching_ms = 10000;  // 10 seconds network buffering for live TV
    cfg.vlc_live_caching_ms = 5000;      // Additional 5 seconds live-specific caching
    cfg.vlc_prefetch_buffer_bytes = 16 * 1024 * 1024; // 16 MiB prefetch buffer for stability
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
                "vlc_network_caching_ms" => cfg.vlc_network_caching_ms = v.trim().parse::<u32>().unwrap_or(10000),
                "vlc_live_caching_ms" => cfg.vlc_live_caching_ms = v.trim().parse::<u32>().unwrap_or(5000),
                "vlc_prefetch_buffer_bytes" => cfg.vlc_prefetch_buffer_bytes = v.trim().parse::<u64>().unwrap_or(16 * 1024 * 1024),
                "enable_downloads" => cfg.enable_downloads = v.trim().parse::<u8>().map(|n| n != 0).unwrap_or(false),
                "max_parallel_downloads" => cfg.max_parallel_downloads = v.trim().parse::<u32>().unwrap_or(1),
                "wisdom_gate_api_key" => cfg.wisdom_gate_api_key = v.trim().to_string(),
                "wisdom_gate_prompt" => cfg.wisdom_gate_prompt = v.trim().to_string(),
                "wisdom_gate_model" => cfg.wisdom_gate_model = v.trim().to_string(),
                "wisdom_gate_cache_content" => {
                    // Decode base64 content for multiline support
                    if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(v.trim()) {
                        if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                            cfg.wisdom_gate_cache_content = decoded_str;
                        } else {
                            cfg.wisdom_gate_cache_content = v.trim().to_string(); // Fallback to raw
                        }
                    } else {
                        cfg.wisdom_gate_cache_content = v.trim().to_string(); // Fallback to raw
                    }
                },
                "wisdom_gate_cache_timestamp" => cfg.wisdom_gate_cache_timestamp = v.trim().parse::<u64>().unwrap_or(0),
                _ => {}
            }
        }
    }
    if cfg.download_dir.trim().is_empty() {
        if let Ok(home) = std::env::var("HOME") {
            cfg.download_dir = format!("{}/Downloads/macxtreamer", home);
        }
    }
    
    // Set default Wisdom-Gate values if empty
    if cfg.wisdom_gate_prompt.trim().is_empty() {
        cfg.wisdom_gate_prompt = crate::models::default_wisdom_gate_prompt();
    }
    if cfg.wisdom_gate_model.trim().is_empty() {
        cfg.wisdom_gate_model = "wisdom-ai-dsv3".to_string(); // Default model - actually available
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
    // Persist VLC buffer options
    writeln!(f, "vlc_network_caching_ms={}", cfg.vlc_network_caching_ms)?;
    writeln!(f, "vlc_live_caching_ms={}", cfg.vlc_live_caching_ms)?;
    writeln!(f, "vlc_prefetch_buffer_bytes={}", cfg.vlc_prefetch_buffer_bytes)?;
    if cfg.cover_uploads_per_frame != 0 { writeln!(f, "cover_uploads_per_frame={}", cfg.cover_uploads_per_frame)?; }
    if cfg.cover_decode_parallel != 0 { writeln!(f, "cover_decode_parallel={}", cfg.cover_decode_parallel)?; }
    if cfg.texture_cache_limit != 0 { writeln!(f, "texture_cache_limit={}", cfg.texture_cache_limit)?; }
    if cfg.category_parallel != 0 { writeln!(f, "category_parallel={}", cfg.category_parallel)?; }
    if cfg.cover_height != 0.0 { writeln!(f, "cover_height={:.1}", cfg.cover_height)?; }
    writeln!(f, "enable_downloads={}", if cfg.enable_downloads { 1 } else { 0 })?;
    if cfg.max_parallel_downloads != 0 { writeln!(f, "max_parallel_downloads={}", cfg.max_parallel_downloads)?; }
    
    // Save Wisdom-Gate configuration
    if !cfg.wisdom_gate_api_key.is_empty() { writeln!(f, "wisdom_gate_api_key={}", cfg.wisdom_gate_api_key)?; }
    if !cfg.wisdom_gate_prompt.is_empty() { writeln!(f, "wisdom_gate_prompt={}", cfg.wisdom_gate_prompt)?; }
    if !cfg.wisdom_gate_model.is_empty() { writeln!(f, "wisdom_gate_model={}", cfg.wisdom_gate_model)?; }
    if !cfg.wisdom_gate_cache_content.is_empty() { 
        // Encode cache content as base64 to handle multiline text (save_config)
        let encoded = general_purpose::STANDARD.encode(cfg.wisdom_gate_cache_content.as_bytes());
        writeln!(f, "wisdom_gate_cache_content={}", encoded)?; 
    }
    if cfg.wisdom_gate_cache_timestamp > 0 { writeln!(f, "wisdom_gate_cache_timestamp={}", cfg.wisdom_gate_cache_timestamp)?; }
    
    Ok(())
}

pub fn write_config(cfg: &Config) -> Result<(), io::Error> {
    // Try to write to primary config location first, fallback to local file
    let primary = config_file_path();
    
    // Create directory if it doesn't exist
    if let Some(parent) = primary.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    
    match write_config_to_file(&primary, cfg) {
        Ok(()) => Ok(()),
        Err(_) => write_config_to_file(&PathBuf::from("xtream_config.txt"), cfg),
    }
}

fn write_config_to_file(path: &PathBuf, cfg: &Config) -> Result<(), io::Error> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    
    if !cfg.address.trim().is_empty() { writeln!(f, "address={}", cfg.address)?; }
    if !cfg.username.trim().is_empty() { writeln!(f, "username={}", cfg.username)?; }
    if !cfg.password.trim().is_empty() { writeln!(f, "password={}", cfg.password)?; }
    if !cfg.player_command.trim().is_empty() { writeln!(f, "player_command={}", cfg.player_command)?; }
    if !cfg.theme.trim().is_empty() { writeln!(f, "theme={}", cfg.theme)?; }
    if cfg.cover_ttl_days != 0 { writeln!(f, "cover_ttl_days={}", cfg.cover_ttl_days)?; }
    if cfg.cover_parallel != 0 { writeln!(f, "cover_parallel={}", cfg.cover_parallel)?; }
    if cfg.font_scale != 0.0 { writeln!(f, "font_scale={:.2}", cfg.font_scale)?; }
    if !cfg.download_dir.is_empty() { writeln!(f, "download_dir={}", cfg.download_dir)?; }
    writeln!(f, "reuse_vlc={}", if cfg.reuse_vlc { 1 } else { 0 })?;
    writeln!(f, "vlc_network_caching_ms={}", cfg.vlc_network_caching_ms)?;
    writeln!(f, "vlc_live_caching_ms={}", cfg.vlc_live_caching_ms)?;
    writeln!(f, "vlc_prefetch_buffer_bytes={}", cfg.vlc_prefetch_buffer_bytes)?;
    if cfg.cover_uploads_per_frame != 0 { writeln!(f, "cover_uploads_per_frame={}", cfg.cover_uploads_per_frame)?; }
    if cfg.cover_decode_parallel != 0 { writeln!(f, "cover_decode_parallel={}", cfg.cover_decode_parallel)?; }
    if cfg.texture_cache_limit != 0 { writeln!(f, "texture_cache_limit={}", cfg.texture_cache_limit)?; }
    if cfg.category_parallel != 0 { writeln!(f, "category_parallel={}", cfg.category_parallel)?; }
    if cfg.cover_height != 0.0 { writeln!(f, "cover_height={:.1}", cfg.cover_height)?; }
    writeln!(f, "enable_downloads={}", if cfg.enable_downloads { 1 } else { 0 })?;
    if cfg.max_parallel_downloads != 0 { writeln!(f, "max_parallel_downloads={}", cfg.max_parallel_downloads)?; }
    
    // Save Wisdom-Gate configuration
    if !cfg.wisdom_gate_api_key.is_empty() { writeln!(f, "wisdom_gate_api_key={}", cfg.wisdom_gate_api_key)?; }
    if !cfg.wisdom_gate_prompt.is_empty() { writeln!(f, "wisdom_gate_prompt={}", cfg.wisdom_gate_prompt)?; }
    if !cfg.wisdom_gate_model.is_empty() { writeln!(f, "wisdom_gate_model={}", cfg.wisdom_gate_model)?; }
    if !cfg.wisdom_gate_cache_content.is_empty() { 
        // Encode cache content as base64 to handle multiline text (write_config_to_file)
        let encoded = general_purpose::STANDARD.encode(cfg.wisdom_gate_cache_content.as_bytes());
        writeln!(f, "wisdom_gate_cache_content={}", encoded)?; 
    }
    if cfg.wisdom_gate_cache_timestamp > 0 { writeln!(f, "wisdom_gate_cache_timestamp={}", cfg.wisdom_gate_cache_timestamp)?; }
    
    Ok(())
}
