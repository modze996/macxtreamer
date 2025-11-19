use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub address: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub player_command: String,
    #[serde(default)]
    pub theme: String, // "dark" | "light"
    #[serde(default)]
    pub cover_ttl_days: u32,      // 1 Woche default
    #[serde(default)]
    pub cover_parallel: u32,      // 6 default
    #[serde(default)]
    pub font_scale: f32,          // 1.0..=?, default increased (e.g., 1.15)
    #[serde(default)]
    pub reuse_vlc: bool,          // default true
    #[serde(default)]
    pub vlc_network_caching_ms: u32, // network-caching in ms for VLC (live)
    #[serde(default)]
    pub vlc_live_caching_ms: u32,    // live-caching in ms for VLC
    #[serde(default)]
    pub vlc_prefetch_buffer_bytes: u64, // prefetch buffer size in bytes
    #[serde(default)]
    pub vlc_file_caching_ms: u32,    // file/on-demand caching in ms (VOD)
    #[serde(default)]
    pub vlc_mux_caching_ms: u32,     // demux/mux layer caching in ms (advanced)
    #[serde(default)]
    pub vlc_http_reconnect: bool,    // attempt HTTP reconnect on drop
    #[serde(default)]
    pub vlc_timeout_ms: u32,         // HTTP timeout ms
    #[serde(default)]
    pub vlc_extra_args: String,      // additional raw VLC args appended before URL
    #[serde(default)]
    pub vlc_profile_bias: u32,       // 0..100 (0 = minimale Latenz, 100 = maximale Stabilität)
    #[serde(default)]
    pub vlc_verbose: bool,           // enable -vvv when diagnosing
    #[serde(default)]
    pub vlc_diagnose_on_start: bool, // capture VLC output once per start
    #[serde(default)]
    pub vlc_continuous_diagnostics: bool, // keep a background verbose VLC to adapt caching
    #[serde(default)]
    pub use_mpv: bool, // prefer mpv over VLC when launching player
    #[serde(default)]
    pub mpv_extra_args: String, // additional raw mpv args
    #[serde(default)]
    pub mpv_cache_secs_override: u32, // 0 = auto derive from bias
    #[serde(default)]
    pub mpv_readahead_secs_override: u32, // 0 = auto
    #[serde(default)]
    pub mpv_keep_open: bool, // hält Fenster nach EOF offen (Live Stabilität)
    #[serde(default)]
    pub mpv_live_auto_retry: bool, // bei frühem EOF bei Live automatisch neu starten
    #[serde(default)]
    pub mpv_live_retry_max: u32, // max Versuche
    #[serde(default)]
    pub mpv_live_retry_delay_ms: u32, // Pause zwischen Versuchen
    #[serde(default)]
    pub mpv_verbose: bool, // ausführliche stderr Ausgabe von mpv erfassen
    #[serde(default)]
    pub download_dir: String,     // default ~/Downloads/macxtreamer
    #[serde(default)]
    pub cover_uploads_per_frame: u32, // default 3
    #[serde(default)]
    pub cover_decode_parallel: u32,   // default 2
    #[serde(default)]
    pub texture_cache_limit: u32,     // default 512
    #[serde(default)]
    pub category_parallel: u32,       // default 6
    #[serde(default)]
    pub cover_height: f32,            // default 60.0
    #[serde(default)]
    pub enable_downloads: bool,       // default false
    #[serde(default)]
    pub max_parallel_downloads: u32,  // default 1
    #[serde(default)]
    pub wisdom_gate_api_key: String,  // API key for Wisdom-Gate
    #[serde(default)]
    pub wisdom_gate_prompt: String,   // Custom prompt for AI recommendations
    #[serde(default)]
    pub wisdom_gate_model: String,    // Model selection for Wisdom-Gate
    #[serde(default)]
    pub wisdom_gate_cache_content: String,  // Cached recommendations content
    #[serde(default)]
    pub wisdom_gate_cache_timestamp: u64,   // Timestamp when cache was created (Unix timestamp)
    #[serde(default)]
    pub vlc_diag_history: String, // Semikolon-separierte Liste angewandter Vorschläge: ts:net:live:file;...
    #[serde(default)]
    pub low_cpu_mode: bool, // Aktiviert zusätzliche Drosselung (Repaint & Diagnose-Sleep)
    #[serde(default)]
    pub ultra_low_flicker_mode: bool, // Noch aggressiveres Repaint-Gating (optional)
    #[serde(default)]
    pub bottom_panel_height: f32, // persistierte Höhe des Bottom Panels
    #[serde(default)]
    pub left_panel_width: f32, // persistierte Breite der linken AI Seitenleiste
    #[serde(default)]
    pub download_retry_max: u32, // maximale Versuche für einen Download (Resume)
    #[serde(default)]
    pub download_retry_delay_ms: u32, // Wartezeit zwischen Versuchen
}

impl Config {
    /// Check if the cache is still valid (less than 24 hours old)
    pub fn is_wisdom_gate_cache_valid(&self) -> bool {
        if self.wisdom_gate_cache_content.is_empty() || self.wisdom_gate_cache_timestamp == 0 {
            return false;
        }
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let cache_age_hours = (now - self.wisdom_gate_cache_timestamp) / 3600;
        cache_age_hours < 24  // Cache is valid for 24 hours
    }
    
    /// Update the cache with new content
    pub fn update_wisdom_gate_cache(&mut self, content: String) {
        self.wisdom_gate_cache_content = content;
        self.wisdom_gate_cache_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
    
    /// Get cache age in hours for display
    pub fn get_wisdom_gate_cache_age_hours(&self) -> u64 {
        if self.wisdom_gate_cache_timestamp == 0 {
            return 0;
        }
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        (now - self.wisdom_gate_cache_timestamp) / 3600
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub container_extension: String,
    #[serde(default)]
    pub plot: String,
    #[serde(default)]
    pub stream_url: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub year: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub rating_5based: Option<f32>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub director: Option<String>,
    #[serde(default)]
    pub cast: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Episode {
    pub episode_id: String,
    pub name: String,
    #[serde(default)]
    pub container_extension: String,
    #[serde(default)]
    pub stream_url: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentItem {
    pub id: String,
    pub name: String,
    pub info: String,
    pub stream_url: String,
    #[serde(default)]
    pub container_extension: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FavItem {
    pub id: String,
    pub info: String,
    pub name: String,
    pub stream_url: Option<String>,
    #[serde(default)]
    pub container_extension: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub name: String,
    pub id: String,
    pub info: String,
    pub container_extension: Option<String>,
    pub stream_url: Option<String>,
    pub cover_url: Option<String>,
    pub year: Option<String>,
    pub release_date: Option<String>,
    pub rating_5based: Option<f32>,
    pub genre: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchItem {
    pub id: String,
    pub name: String,
    pub info: String,
    pub container_extension: String,
    pub cover: Option<String>,
    pub year: Option<String>,
    pub release_date: Option<String>,
    pub rating_5based: Option<f32>,
    pub genre: Option<String>,
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WisdomGateRecommendation {
    pub content: String,  // Raw AI response content
}

impl Default for WisdomGateRecommendation {
    fn default() -> Self {
        Self {
            content: String::new(),
        }
    }
}

// Default Wisdom-Gate prompt for streaming recommendations
pub fn default_wisdom_gate_prompt() -> String {
    "Was sind die besten Streaming-Empfehlungen für heute in Deutschland? Bitte nenne aktuelle Filme und Serien auf Netflix, Amazon Prime, Disney+, etc. mit kurzer Beschreibung.".to_string()
}
