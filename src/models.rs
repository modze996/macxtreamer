use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    English,
    German,
}

impl Default for Language {
    fn default() -> Self {
        Language::English
    }
}

impl Language {
    #[allow(dead_code)]
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::German => "de",
        }
    }
    
    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::German => "Deutsch",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerProfile {
    pub name: String,
    pub address: String,
    pub username: String,
    pub password: String,
}

impl Default for ServerProfile {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            address: String::new(),
            username: String::new(),
            password: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
        #[serde(default)]
        pub column_config_serialized: Option<String>, // CSV-Liste der ColumnKeys für Persistenz
    // Legacy fields for backward compatibility (will be migrated to profiles)
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    
    // New multi-server support
    #[serde(default)]
    pub server_profiles: Vec<ServerProfile>,
    #[serde(default)]
    pub active_profile_index: usize,
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
    pub mpv_custom_path: String, // manueller Pfad zu mpv falls nicht im PATH/AppBundle
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
    pub wisdom_gate_endpoint: String, // API endpoint for Wisdom-Gate
    #[serde(default)]
    pub wisdom_gate_cache_content: String,  // Cached recommendations content
    #[serde(default)]
    pub wisdom_gate_cache_timestamp: u64,   // Timestamp when cache was created (Unix timestamp)
    #[serde(default)]
    pub ai_provider: String,          // AI provider selection: "wisdom-gate", "perplexity", "cognora", "gemini", "openai"
    #[serde(default)]
    pub perplexity_api_key: String,   // API key for Perplexity
    #[serde(default)]
    pub perplexity_model: String,     // Model selection for Perplexity
    #[serde(default)]
    pub cognora_api_key: String,      // API key for Cognora Toolkit
    #[serde(default)]
    pub cognora_model: String,        // Model selection for Cognora
    #[serde(default)]
    pub gemini_api_key: String,       // API key for Gemini
    #[serde(default)]
    pub gemini_model: String,         // Model selection for Gemini
    #[serde(default)]
    pub openai_api_key: String,       // API key for OpenAI
    #[serde(default)]
    pub openai_model: String,         // Model selection for OpenAI
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
    #[serde(default)]
    pub default_search_languages: Vec<String>, // Default-Sprachen für Suchfilter (z.B. ["EN", "DE", "MULTI"])
    #[serde(default)]
    pub filter_by_language: bool, // Legacy: Filter content by language in main view (kept for backward compatibility)
    #[serde(default)]
    pub filter_live_language: bool, // Persistiert: LiveTV Sprachfilter aktiv
    #[serde(default)]
    pub filter_vod_language: bool,  // Persistiert: VOD Sprachfilter aktiv
    #[serde(default)]
    pub filter_series_language: bool, // Persistiert: Serien Sprachfilter aktiv
    #[serde(default)]
    pub ai_panel_tab: String, // Persistiert: Aktuell ausgewählter Tab in AI Panel ("recommendations" | "recently_added")
    #[serde(default)]
    pub language: Language, // UI Language (English | German)
    #[serde(default)]
    pub check_for_updates: bool, // Auto-check for updates on startup
    #[serde(default)]
    pub last_update_check: u64, // Unix timestamp of last update check
    // SOCKS5 Proxy / VPN Settings
    #[serde(default)]
    pub proxy_enabled: bool, // Enable proxy for network traffic
    #[serde(default)]
    pub proxy_type: String, // "socks5" or "http"
    #[serde(default)]
    pub proxy_host: String, // proxy server hostname (e.g., "proxy.privadovpn.com")
    #[serde(default)]
    pub proxy_port: u16, // proxy port (1080 for socks5, 8118 for privoxy/http)
    #[serde(default)]
    pub proxy_username: String, // proxy auth username (optional)
    #[serde(default)]
    pub proxy_password: String, // proxy auth password (optional)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            column_config_serialized: None,
            address: String::new(),
            username: String::new(),
            password: String::new(),
            server_profiles: Vec::new(),  // Start with empty list - profiles will be loaded from config file
            active_profile_index: 0,
            player_command: String::new(),
            theme: "dark".to_string(),
            cover_ttl_days: 7,
            cover_parallel: 6,
            font_scale: 1.15,
            reuse_vlc: true,
            vlc_network_caching_ms: 8000,
            vlc_live_caching_ms: 6000,
            vlc_prefetch_buffer_bytes: 1024 * 1024,
            vlc_file_caching_ms: 5000,
            vlc_mux_caching_ms: 3000,
            vlc_http_reconnect: true,
            vlc_timeout_ms: 10000,
            vlc_extra_args: String::new(),
            vlc_profile_bias: 50,
            vlc_verbose: false,
            vlc_diagnose_on_start: false,
            vlc_continuous_diagnostics: false,
            use_mpv: false,
            mpv_extra_args: String::new(),
            mpv_cache_secs_override: 0,
            mpv_readahead_secs_override: 0,
            mpv_keep_open: false,
            mpv_live_auto_retry: false,
            mpv_live_retry_max: 3,
            mpv_live_retry_delay_ms: 2000,
            mpv_verbose: false,
            mpv_custom_path: String::new(),
            download_dir: String::new(),
            cover_uploads_per_frame: 3,
            cover_decode_parallel: 2,
            texture_cache_limit: 512,
            category_parallel: 6,
            cover_height: 60.0,
            enable_downloads: false,
            max_parallel_downloads: 1,
            wisdom_gate_api_key: String::new(),
            wisdom_gate_prompt: default_wisdom_gate_prompt(),
            wisdom_gate_model: "wisdom-ai-dsr1".to_string(), // Default Wisdom Gate Modell
            wisdom_gate_endpoint: "https://wisdom-gate.juheapi.com/v1/chat/completions".to_string(),
            wisdom_gate_cache_content: String::new(),
            wisdom_gate_cache_timestamp: 0,
            ai_provider: "wisdom-gate".to_string(),
            perplexity_api_key: String::new(),
            perplexity_model: "sonar".to_string(),
            cognora_api_key: String::new(),
            cognora_model: "cognora-3".to_string(),
            gemini_api_key: String::new(),
            gemini_model: "gemini-2.0-flash-exp".to_string(),
            openai_api_key: String::new(),
            openai_model: "gpt-4o".to_string(),
            vlc_diag_history: String::new(),
            low_cpu_mode: true,
            ultra_low_flicker_mode: false,
            bottom_panel_height: 200.0,
            left_panel_width: 300.0,
            download_retry_max: 3,
            download_retry_delay_ms: 1000,
            default_search_languages: vec!["EN".to_string(), "DE".to_string(), "MULTI".to_string()],
            filter_by_language: true,
            filter_live_language: false,
            filter_vod_language: false,
            filter_series_language: false,
            ai_panel_tab: "recommendations".to_string(),
            language: Language::English,
            check_for_updates: true,
            last_update_check: 0,
            proxy_enabled: false,
            proxy_type: "socks5".to_string(),
            proxy_host: String::new(),
            proxy_port: 1080,
            proxy_username: String::new(),
            proxy_password: String::new(),
        }
    }
}

impl Config {
    /// Get the currently active server profile
    pub fn active_profile(&self) -> &ServerProfile {
        if self.server_profiles.is_empty() {
            // Should never happen, but provide a safe default
            static DEFAULT: ServerProfile = ServerProfile {
                name: String::new(),
                address: String::new(),
                username: String::new(),
                password: String::new(),
            };
            return &DEFAULT;
        }
        let idx = self.active_profile_index.min(self.server_profiles.len() - 1);
        &self.server_profiles[idx]
    }
    
    /// Get mutable reference to currently active server profile
    pub fn active_profile_mut(&mut self) -> &mut ServerProfile {
        // Don't add a default profile here - it should be added during initialization
        // If for some reason there are no profiles, return the first one after ensuring index is valid
        if !self.server_profiles.is_empty() {
            let idx = self.active_profile_index.min(self.server_profiles.len() - 1);
            self.active_profile_index = idx; // Ensure index is valid
            &mut self.server_profiles[idx]
        } else {
            // Emergency fallback - should never happen in normal operation
            eprintln!("⚠️ WARNING: active_profile_mut() called with no profiles - this should not happen!");
            self.server_profiles.push(ServerProfile::default());
            self.active_profile_index = 0;
            &mut self.server_profiles[0]
        }
    }
    
    /// Migrate legacy single-server config to profiles if needed
    pub fn migrate_to_profiles(&mut self) {
        // If we have legacy data but no profiles, migrate
        if self.server_profiles.is_empty() && (!self.address.is_empty() || !self.username.is_empty()) {
            self.server_profiles.push(ServerProfile {
                name: "Imported".to_string(),
                address: self.address.clone(),
                username: self.username.clone(),
                password: self.password.clone(),
            });
            self.active_profile_index = 0;
        }
        // Ensure at least one profile exists
        if self.server_profiles.is_empty() {
            self.server_profiles.push(ServerProfile::default());
        }
        // Sync active profile to legacy fields
        self.sync_active_profile();
    }
    
    /// Sync active profile data to legacy fields for API compatibility
    pub fn sync_active_profile(&mut self) {
        let addr = self.active_profile().address.clone();
        let user = self.active_profile().username.clone();
        let pass = self.active_profile().password.clone();
        self.address = addr;
        self.username = user;
        self.password = pass;
    }
    
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
    #[serde(default)]
    pub audio_languages: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavItem {
    pub id: String,
    pub info: String,
    pub name: String,
    #[serde(default)]
    pub item_type: String, // "Movie", "Series", "Channel", "Episode"
    #[serde(default)]
    pub stream_url: Option<String>,
    #[serde(default)]
    pub container_extension: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub series_id: Option<String>, // For episodes: reference to parent series
}

impl Default for FavItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            info: String::new(),
            name: String::new(),
            item_type: String::new(),
            stream_url: None,
            container_extension: None,
            cover: None,
            series_id: None,
        }
    }
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
    pub audio_languages: Option<String>,
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
