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
    // Enhanced defaults for VLC buffering - optimized for live TV stability with stuttering fix
    cfg.vlc_network_caching_ms = 25000;  // 25 seconds network buffering for stutter-free live TV
    cfg.vlc_live_caching_ms = 15000;     // 15 seconds additional live-specific caching
    cfg.vlc_prefetch_buffer_bytes = 64 * 1024 * 1024; // 64 MiB prefetch buffer for maximum stability
    cfg.vlc_file_caching_ms = 3000; // default moderate VOD file caching
    cfg.vlc_mux_caching_ms = 1500; // default small mux caching
    cfg.vlc_http_reconnect = true; // attempt reconnects by default
    cfg.vlc_timeout_ms = 15000; // 15s HTTP timeout
    cfg.vlc_extra_args = String::new(); // empty by default
    cfg.vlc_profile_bias = 50; // middle ground default
    cfg.vlc_verbose = false;
    cfg.vlc_diagnose_on_start = false;
    cfg.vlc_continuous_diagnostics = false;
    cfg.use_mpv = false; // default to VLC unless user opts in
    cfg.mpv_extra_args = String::new();
    cfg.mpv_cache_secs_override = 0;
    cfg.mpv_readahead_secs_override = 0;
    cfg.mpv_keep_open = true; // sinnvoll für Live
    cfg.mpv_live_auto_retry = true;
    cfg.mpv_live_retry_max = 5;
    cfg.mpv_live_retry_delay_ms = 4000;
    cfg.mpv_verbose = false;
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
                "vlc_network_caching_ms" => cfg.vlc_network_caching_ms = v.trim().parse::<u32>().unwrap_or(25000),
                "vlc_live_caching_ms" => cfg.vlc_live_caching_ms = v.trim().parse::<u32>().unwrap_or(15000),
                "vlc_prefetch_buffer_bytes" => cfg.vlc_prefetch_buffer_bytes = v.trim().parse::<u64>().unwrap_or(64 * 1024 * 1024),
                "vlc_file_caching_ms" => cfg.vlc_file_caching_ms = v.trim().parse::<u32>().unwrap_or(3000),
                "vlc_mux_caching_ms" => cfg.vlc_mux_caching_ms = v.trim().parse::<u32>().unwrap_or(1500),
                "vlc_http_reconnect" => cfg.vlc_http_reconnect = v.trim().parse::<u8>().map(|n| n != 0).unwrap_or(true),
                "vlc_timeout_ms" => cfg.vlc_timeout_ms = v.trim().parse::<u32>().unwrap_or(15000),
                "vlc_extra_args" => cfg.vlc_extra_args = v.trim().to_string(),
                "vlc_profile_bias" => cfg.vlc_profile_bias = v.trim().parse::<u32>().unwrap_or(50).min(100),
                "vlc_verbose" => cfg.vlc_verbose = v.trim().parse::<u8>().map(|n| n!=0).unwrap_or(false),
                "vlc_diagnose_on_start" => cfg.vlc_diagnose_on_start = v.trim().parse::<u8>().map(|n| n!=0).unwrap_or(false),
                "vlc_continuous_diagnostics" => cfg.vlc_continuous_diagnostics = v.trim().parse::<u8>().map(|n| n!=0).unwrap_or(false),
                "use_mpv" => cfg.use_mpv = v.trim().parse::<u8>().map(|n| n!=0).unwrap_or(false),
                "mpv_extra_args" => cfg.mpv_extra_args = v.trim().to_string(),
                "mpv_cache_secs_override" => cfg.mpv_cache_secs_override = v.trim().parse::<u32>().unwrap_or(0),
                "mpv_readahead_secs_override" => cfg.mpv_readahead_secs_override = v.trim().parse::<u32>().unwrap_or(0),
                "mpv_keep_open" => cfg.mpv_keep_open = v.trim().parse::<u8>().map(|n| n!=0).unwrap_or(true),
                "mpv_live_auto_retry" => cfg.mpv_live_auto_retry = v.trim().parse::<u8>().map(|n| n!=0).unwrap_or(true),
                "mpv_live_retry_max" => cfg.mpv_live_retry_max = v.trim().parse::<u32>().unwrap_or(5),
                "mpv_live_retry_delay_ms" => cfg.mpv_live_retry_delay_ms = v.trim().parse::<u32>().unwrap_or(4000),
                "mpv_verbose" => cfg.mpv_verbose = v.trim().parse::<u8>().map(|n| n!=0).unwrap_or(false),
                "enable_downloads" => cfg.enable_downloads = v.trim().parse::<u8>().map(|n| n != 0).unwrap_or(false),
                "max_parallel_downloads" => cfg.max_parallel_downloads = v.trim().parse::<u32>().unwrap_or(1),
                "wisdom_gate_api_key" => cfg.wisdom_gate_api_key = v.trim().to_string(),
                "wisdom_gate_prompt" => cfg.wisdom_gate_prompt = v.trim().to_string(),
                "wisdom_gate_model" => cfg.wisdom_gate_model = v.trim().to_string(),
                "wisdom_gate_endpoint" => cfg.wisdom_gate_endpoint = v.trim().to_string(),
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
                "vlc_diag_history" => cfg.vlc_diag_history = v.trim().to_string(),
                "low_cpu_mode" => cfg.low_cpu_mode = v.trim().parse::<u8>().map(|n| n!=0).unwrap_or(false),
                "ultra_low_flicker_mode" => cfg.ultra_low_flicker_mode = v.trim().parse::<u8>().map(|n| n!=0).unwrap_or(false),
                "bottom_panel_height" => cfg.bottom_panel_height = v.trim().parse::<f32>().unwrap_or(0.0),
                "left_panel_width" => cfg.left_panel_width = v.trim().parse::<f32>().unwrap_or(0.0),
                "active_profile_index" => cfg.active_profile_index = v.trim().parse::<usize>().unwrap_or(0),
                "server_profile" => {
                    // Format: name|address|username|password
                    let parts: Vec<&str> = v.split('|').collect();
                    if parts.len() == 4 {
                        cfg.server_profiles.push(crate::models::ServerProfile {
                            name: parts[0].to_string(),
                            address: parts[1].to_string(),
                            username: parts[2].to_string(),
                            password: parts[3].to_string(),
                        });
                    }
                },
                _ => {}
            }
        }
    }
    
    // Migrate legacy config to profiles if needed (only if no profiles exist yet)
    let had_no_profiles = cfg.server_profiles.is_empty();
    if had_no_profiles {
        cfg.migrate_to_profiles();
    } else {
        // If profiles exist, just sync the active one to legacy fields
        // Ensure active_profile_index is valid
        if cfg.active_profile_index >= cfg.server_profiles.len() {
            cfg.active_profile_index = 0;
        }
        cfg.sync_active_profile();
    }
    
    // Ensure at least one profile exists after migration/sync
    if cfg.server_profiles.is_empty() {
        cfg.server_profiles.push(crate::models::ServerProfile::default());
        cfg.active_profile_index = 0;
    }
    
    // Only save if we had no profiles before and now have them (first migration)
    let needs_save = had_no_profiles && !cfg.server_profiles.is_empty();
    
    if cfg.download_dir.trim().is_empty() {
        cfg.wisdom_gate_prompt = crate::models::default_wisdom_gate_prompt();
    }
    if cfg.wisdom_gate_model.trim().is_empty() {
        cfg.wisdom_gate_model = "gpt-3.5-turbo".to_string(); // Default model - actually available
    }
    if cfg.wisdom_gate_endpoint.trim().is_empty() {
        cfg.wisdom_gate_endpoint = "https://api.wisdom-gate.ai/v1/chat/completions".to_string();
    }
    
    // Save immediately after migration to persist profiles
    if needs_save {
        let _ = save_config(&cfg);
    }
    
    Ok(cfg)
}

pub fn save_config(cfg: &Config) -> Result<(), io::Error> {
    let path = config_file_path();
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let mut f = fs::File::create(path)?;
    
    // Create a cleaned copy of profiles without empty Default profiles
    let mut cleaned_profiles: Vec<&crate::models::ServerProfile> = Vec::new();
    let mut old_to_new_index: Vec<usize> = Vec::new();
    
    for (_old_idx, profile) in cfg.server_profiles.iter().enumerate() {
        // Skip empty Default profiles
        if profile.name == "Default" && profile.address.is_empty() && profile.username.is_empty() && profile.password.is_empty() {
            old_to_new_index.push(usize::MAX); // Mark as removed
            continue;
        }
        old_to_new_index.push(cleaned_profiles.len());
        cleaned_profiles.push(profile);
    }
    
    // If no profiles remain, save at least one default
    if cleaned_profiles.is_empty() {
        writeln!(f, "server_profile=Default|||")?;
        writeln!(f, "active_profile_index=0")?;
    } else {
        // Save cleaned profiles
        for profile in &cleaned_profiles {
            writeln!(f, "server_profile={}|{}|{}|{}", profile.name, profile.address, profile.username, profile.password)?;
        }
        
        // Map the active_profile_index to the cleaned list
        let valid_index = if cfg.active_profile_index < old_to_new_index.len() {
            let new_idx = old_to_new_index[cfg.active_profile_index];
            if new_idx != usize::MAX {
                new_idx
            } else {
                0 // Active profile was removed, default to first
            }
        } else {
            0
        };
        writeln!(f, "active_profile_index={}", valid_index)?;
    }
    
    // Save active profile data to legacy fields for backward compatibility
    let active = cfg.active_profile();
    writeln!(f, "address={}", active.address)?;
    writeln!(f, "username={}", active.username)?;
    writeln!(f, "password={}", active.password)?;
    
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
    writeln!(f, "vlc_file_caching_ms={}", cfg.vlc_file_caching_ms)?;
    writeln!(f, "vlc_mux_caching_ms={}", cfg.vlc_mux_caching_ms)?;
    writeln!(f, "vlc_http_reconnect={}", if cfg.vlc_http_reconnect { 1 } else { 0 })?;
    writeln!(f, "vlc_timeout_ms={}", cfg.vlc_timeout_ms)?;
    if !cfg.vlc_extra_args.trim().is_empty() { writeln!(f, "vlc_extra_args={}", cfg.vlc_extra_args)?; }
    writeln!(f, "vlc_profile_bias={}", cfg.vlc_profile_bias)?;
    writeln!(f, "vlc_verbose={}", if cfg.vlc_verbose {1} else {0})?;
    writeln!(f, "vlc_diagnose_on_start={}", if cfg.vlc_diagnose_on_start {1} else {0})?;
    writeln!(f, "vlc_continuous_diagnostics={}", if cfg.vlc_continuous_diagnostics {1} else {0})?;
    // mpv Parameter (einmalig, Duplikate entfernt)
    writeln!(f, "use_mpv={}", if cfg.use_mpv {1} else {0})?;
    if !cfg.mpv_extra_args.trim().is_empty() { writeln!(f, "mpv_extra_args={}", cfg.mpv_extra_args)?; }
    if cfg.mpv_cache_secs_override != 0 { writeln!(f, "mpv_cache_secs_override={}", cfg.mpv_cache_secs_override)?; }
    if cfg.mpv_readahead_secs_override != 0 { writeln!(f, "mpv_readahead_secs_override={}", cfg.mpv_readahead_secs_override)?; }
    writeln!(f, "mpv_keep_open={}", if cfg.mpv_keep_open {1} else {0})?;
    writeln!(f, "mpv_live_auto_retry={}", if cfg.mpv_live_auto_retry {1} else {0})?;
    writeln!(f, "mpv_live_retry_max={}", cfg.mpv_live_retry_max)?;
    writeln!(f, "mpv_live_retry_delay_ms={}", cfg.mpv_live_retry_delay_ms)?;
    writeln!(f, "mpv_verbose={}", if cfg.mpv_verbose {1} else {0})?;
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
    if !cfg.wisdom_gate_endpoint.is_empty() { writeln!(f, "wisdom_gate_endpoint={}", cfg.wisdom_gate_endpoint)?; }
    if !cfg.wisdom_gate_cache_content.is_empty() { 
        // Encode cache content as base64 to handle multiline text (save_config)
        let encoded = general_purpose::STANDARD.encode(cfg.wisdom_gate_cache_content.as_bytes());
        writeln!(f, "wisdom_gate_cache_content={}", encoded)?; 
    }
    if cfg.wisdom_gate_cache_timestamp > 0 { writeln!(f, "wisdom_gate_cache_timestamp={}", cfg.wisdom_gate_cache_timestamp)?; }
    if !cfg.vlc_diag_history.trim().is_empty() { writeln!(f, "vlc_diag_history={}", cfg.vlc_diag_history)?; }
    writeln!(f, "low_cpu_mode={}", if cfg.low_cpu_mode {1} else {0})?;
    writeln!(f, "ultra_low_flicker_mode={}", if cfg.ultra_low_flicker_mode {1} else {0})?; // Duplikat entfernt
    if cfg.bottom_panel_height > 0.0 { writeln!(f, "bottom_panel_height={:.1}", cfg.bottom_panel_height)?; }
    if cfg.left_panel_width > 0.0 { writeln!(f, "left_panel_width={:.1}", cfg.left_panel_width)?; }
    
    Ok(())
}

pub fn write_config(cfg: &Config) -> Result<(), io::Error> {
    // Use save_config which includes server profiles
    save_config(cfg)
}
