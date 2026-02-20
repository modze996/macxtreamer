
use eframe::egui::{self, Color32, RichText};
use egui_extras::TableBuilder;

// Hilfsfunktion: Spalten-Konfiguration als CSV speichern
use image::GenericImageView;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
// removed unused AsyncReadExt import after refactor
use tokio::sync::Semaphore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnKey {
    Cover,
    Name,
    ID,
    Info,
    Year,
    ReleaseDate,
    Rating,
    Genre,
    Languages,
    Path,
    CurrentProgram,
    Actions,
}

mod ai_panel;
mod api;
mod app_state;
mod cache;
mod config;
mod download_utils;
mod downloads;
mod helpers;
mod icon;
mod images;
mod logger;
mod models;
mod network;
mod player;
mod search;
mod storage;
mod ui_helpers;
mod i18n;
mod updater;

use ai_panel::render_ai_panel;
use i18n::t;
use api::{fetch_categories, fetch_items, fetch_series_episodes};
use app_state::{Msg, SearchStatus, SortKey, ViewState};
use cache::{clear_all_caches};
use config::{read_config, save_config};
use download_utils::{DownloadMeta, DownloadState, ScannedDownload, expand_download_dir};
use downloads::{BulkOptions, sanitize_filename};
use helpers::{file_path_to_uri, format_file_size};
use logger::log_line;
use models::{Category, Config, FavItem, Item, Language, RecentItem, Row};
use ui_helpers::{colored_text_by_type, render_loading_spinner};
impl ColumnKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            ColumnKey::Cover => "cover",
            ColumnKey::Name => "name",
            ColumnKey::ID => "id",
            ColumnKey::Info => "info",
            ColumnKey::Year => "year",
            ColumnKey::ReleaseDate => "release_date",
            ColumnKey::Rating => "rating",
            ColumnKey::Genre => "genre",
            ColumnKey::Languages => "languages",
            ColumnKey::Path => "path",
            ColumnKey::CurrentProgram => "current_program",
            ColumnKey::Actions => "actions",
        }
    }
    pub fn from_str(s: &str) -> Option<ColumnKey> {
        match s {
            "cover" => Some(ColumnKey::Cover),
            "name" => Some(ColumnKey::Name),
            "id" => Some(ColumnKey::ID),
            "info" => Some(ColumnKey::Info),
            "year" => Some(ColumnKey::Year),
            "release_date" => Some(ColumnKey::ReleaseDate),
            "rating" => Some(ColumnKey::Rating),
            "genre" => Some(ColumnKey::Genre),
            "languages" => Some(ColumnKey::Languages),
            "path" => Some(ColumnKey::Path),
            "current_program" => Some(ColumnKey::CurrentProgram),
            "actions" => Some(ColumnKey::Actions),
            _ => None,
        }
    }
}
use player::{build_url_by_type, start_player};
use once_cell::sync::OnceCell;
static GLOBAL_TX: OnceCell<Sender<Msg>> = OnceCell::new();
use search::search_items_with_language_filter;
use storage::{add_to_recently, load_favorites, load_recently_played, toggle_favorite, is_favorite, load_search_history, save_search_history};

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    let icon = icon::generate_icon(256);
    let viewport = egui::ViewportBuilder::default()
        .with_maximized(true) // Start im maximierten Modus
        .with_icon(icon);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "MacXtreamer",
        options,
        Box::new(|cc| {
            // Setup fonts with extended Unicode support
            setup_custom_fonts(&cc.egui_ctx);
            Box::new(MacXtreamer::new())
        }),
    )
}

fn setup_custom_fonts(ctx: &egui::Context) {
    use egui::FontFamily;
    
    let mut fonts = egui::FontDefinitions::default();
    
    // Load system fonts that support extended Unicode characters
    #[cfg(target_os = "macos")]
    {
        let mut loaded_fonts = Vec::new();
        
        // Try to load Arial Unicode MS - has comprehensive Unicode coverage including modifier letters
        if let Ok(font_data) = std::fs::read("/System/Library/Fonts/Supplemental/Arial Unicode.ttf") {
            // Explicitly enable extended Unicode ranges including modifier letters (U+02B0-02FF)
            // and other special characters
            let font_data_with_tweak = egui::FontData::from_owned(font_data).tweak(
                egui::FontTweak {
                    scale: 1.0,
                    y_offset_factor: 0.0,
                    y_offset: 0.0,
                    baseline_offset_factor: 0.0,
                }
            );
            
            fonts.font_data.insert("ArialUnicode".to_owned(), font_data_with_tweak);
            fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "ArialUnicode".to_owned());
            fonts.families.get_mut(&FontFamily::Monospace).unwrap().insert(0, "ArialUnicode".to_owned());
            loaded_fonts.push("Arial Unicode");
        }
        
        // Also try Menlo which has good Unicode support
        if let Ok(font_data) = std::fs::read("/System/Library/Fonts/Menlo.ttc") {
            fonts.font_data.insert("Menlo".to_owned(), egui::FontData::from_owned(font_data));
            fonts.families.get_mut(&FontFamily::Proportional).unwrap().push("Menlo".to_owned());
            loaded_fonts.push("Menlo");
        }
        
        // Helvetica as additional fallback
        if let Ok(font_data) = std::fs::read("/System/Library/Fonts/Helvetica.ttc") {
            fonts.font_data.insert("Helvetica".to_owned(), egui::FontData::from_owned(font_data));
            fonts.families.get_mut(&FontFamily::Proportional).unwrap().push("Helvetica".to_owned());
            loaded_fonts.push("Helvetica");
        }
        
        if !loaded_fonts.is_empty() {
            println!("✅ Fonts geladen: {}", loaded_fonts.join(", "));
        } else {
            println!("⚠️ No additional fonts loaded - some Unicode characters may not render correctly");
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        // For Linux/Windows, try common Unicode fonts
        let unicode_fonts = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
            "C:\\Windows\\Fonts\\arialuni.ttf",
            "C:\\Windows\\Fonts\\seguisym.ttf",
        ];
        
        for font_path in &unicode_fonts {
            if let Ok(font_data) = std::fs::read(font_path) {
                let font_name = Path::new(font_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unicode");
                fonts.font_data.insert(
                    font_name.to_owned(),
                    egui::FontData::from_owned(font_data)
                );
                fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, font_name.to_owned());
                println!("✅ Font geladen: {}", font_name);
                break;
            }
        }
    }
    
    ctx.set_fonts(fonts);
    
    // Force immediate font atlas rebuild with full Unicode support
    ctx.request_repaint();
    println!("✅ Font configuration applied with extended Unicode ranges");
}

#[derive(Clone)]
struct Toast {
    message: String,
    toast_type: ToastType,
    created_at: std::time::Instant,
}

#[derive(Clone, Copy, PartialEq)]
enum ToastType {
    Info,
    Success,
    Warning,
    #[allow(dead_code)]
    Error,
}

struct MacXtreamer {
        // Sichtbare und sortierte Spalten
        column_config: Vec<ColumnKey>,
    // Config/State
    config: Config,
    config_draft: Option<Config>,
    playlists: Vec<Category>,
    vod_categories: Vec<Category>,
    series_categories: Vec<Category>,
    content_rows: Vec<Row>,
    all_movies: Vec<Item>,
    all_series: Vec<Item>,
    all_channels: Vec<Item>,
    recently: Vec<RecentItem>,
    favorites: Vec<FavItem>,


    // UI assets
    textures: HashMap<String, egui::TextureHandle>,
    pending_covers: HashSet<String>,
    // Queue of cover bytes waiting to be decoded & uploaded as textures (budgeted per frame)
    pending_texture_uploads: VecDeque<(String, Vec<u8>, u32, u32)>,
    // URLs currently queued for upload, and for background decode, to avoid duplicates
    pending_texture_urls: HashSet<String>,
    pending_decode_urls: HashSet<String>,
    decode_sem: Arc<Semaphore>,
    cover_sem: Arc<Semaphore>,
    cover_height: f32,

    // UI State
    search_text: String,
    search_history: Vec<String>,
    show_search_history: bool,
    search_status: SearchStatus,
    search_language_filter: Vec<String>, // Selected languages for search filtering
    show_language_filter: bool, // Show language filter dropdown
    is_loading: bool,
    loading_done: usize,
    loading_total: usize,
    last_error: Option<String>,
    show_config: bool,
    pending_save_config: bool,
    show_server_manager: bool,
    new_profile_name: String,
    selected_playlist: Option<usize>,
    selected_vod: Option<usize>,
    selected_series: Option<usize>,
    last_live_cat_id: Option<String>,
    last_vod_cat_id: Option<String>,
    last_series_cat_id: Option<String>,
    current_theme: String,
    theme_applied: bool,
    font_scale_applied: bool,
    current_font_scale: f32, // Track the intended font scale independently
    indexing: bool,
    sort_key: Option<SortKey>,
    sort_asc: bool,
    // Neue Checkboxen für Sprachfilterung pro Kategorie
    filter_live_language: bool,
    filter_vod_language: bool,
    filter_series_language: bool,

    // Async messaging
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    show_error_dialog: bool,
    loading_error: String,
    initial_config_pending: bool,
    downloads: HashMap<String, DownloadState>,
    download_order: Vec<String>,
    download_meta: HashMap<String, DownloadMeta>,
    // Map item-id -> category path for displaying in search results
    index_paths: HashMap<String, String>,
    confirm_bulk: Option<(String, String)>,
    bulk_opts_draft: BulkOptions,
    bulk_options_by_series: HashMap<String, BulkOptions>,
    pending_bulk_downloads: Vec<(String, String, String, Option<String>)>,
    http_client: reqwest::Client,
    last_download_scan: Option<std::time::Instant>,
    should_check_downloads: bool,
    should_start_search: bool,
    current_view: Option<ViewState>,
    view_stack: Vec<ViewState>,
    wisdom_gate_recommendations: Option<String>,
    _wisdom_gate_last_fetch: Option<std::time::Instant>,
    ai_panel_tab: String,
    recently_added_items: Vec<Item>,
    vlc_diag_lines: VecDeque<String>,
    vlc_diag_suggestion: Option<(u32,u32,u32)>,
    has_vlc: bool,
    has_mpv: bool,
    vlc_version: Option<String>,
    mpv_version: Option<String>,
    detected_vlc_path: Option<String>,
    detected_mpv_path: Option<String>,
    vlc_fail_count: u32,
    mpv_fail_count: u32,
    active_diag_stop: Option<Arc<AtomicBool>>,
    command_preview: String,
    last_frame_time: std::time::Instant,
    avg_frame_ms: f32,
    last_forced_repaint: std::time::Instant,
    pending_repaint_due_to_msg: bool,
    pending_player_redetect: bool,
    /// Wallclock time of the previous frame — used to detect system suspend/resume
    last_frame_wallclock: std::time::SystemTime,
    /// Set to `Some(deadline)` for a few seconds after a suspected wake from sleep/hibernate.
    /// While active, repaints are throttled to ~5 FPS to prevent the flicker burst.
    post_wake_cooldown_until: Option<std::time::Instant>,
    
    // EPG data: stream_id -> current program info
    epg_data: HashMap<String, String>,
    epg_loading: HashSet<String>, // Track which channels are loading EPG
    
    // Update system
    available_update: Option<updater::UpdateInfo>,
    checking_for_updates: bool,
    update_check_deadline: Option<std::time::Instant>,
    update_downloading: bool,
    update_installing: bool,
    update_progress: String,
    /// True when update check was triggered automatically on startup → auto-install without dialog.
    startup_auto_install: bool,
    
    // Toast notifications
    toasts: Vec<Toast>,
}

impl MacXtreamer {
    fn new() -> Self {
        let read_result = read_config();
        let (config, had_file) = match read_result {
            Ok(c) => (c, true),
            Err(_) => {
                let mut cfg = Config::default();
                // Ensure profiles are initialized
                cfg.migrate_to_profiles();
                // Save immediately to persist default config
                let _ = save_config(&cfg);
                (cfg, false)
            },
        };
        
        // Check for cached recommendations
        let cached_recommendations = if config.is_wisdom_gate_cache_valid() && !config.wisdom_gate_cache_content.is_empty() {
            let cache_age = config.get_wisdom_gate_cache_age_hours();
            println!("📦 Loading cached AI recommendations (age: {}h)", cache_age);
            Some(format!("📦 **Cached Recommendations** (updated {}h ago)\n\n{}", 
                cache_age, &config.wisdom_gate_cache_content))
        } else {
            None
        };
        
        let (tx, rx) = mpsc::channel();
    let _ = GLOBAL_TX.set(tx.clone());
    // Capture persisted per-category filter flags before moving `config` into the struct
    let persisted_filter_live = config.filter_live_language;
    let persisted_filter_vod = config.filter_vod_language;
    let persisted_filter_series = config.filter_series_language;
    let default_search_langs = config.default_search_languages.clone();
    let persisted_ai_panel_tab = config.ai_panel_tab.clone();
        let mut app = Self {
                        column_config: vec![
                            ColumnKey::Cover,
                            ColumnKey::Name,
                            ColumnKey::ReleaseDate,
                            ColumnKey::Rating,
                            ColumnKey::Genre,
                            ColumnKey::Languages,
                            ColumnKey::CurrentProgram,
                            ColumnKey::Actions,
                        ],
            config,
            config_draft: None,
            playlists: vec![],
            vod_categories: vec![],
            series_categories: vec![],
            content_rows: vec![],
            all_movies: vec![],
            all_series: vec![],
            all_channels: vec![],
            recently: load_recently_played(),
            favorites: load_favorites(),
            // Initialize per-category filter flags from persisted config
            filter_live_language: persisted_filter_live,
            filter_vod_language: persisted_filter_vod,
            filter_series_language: persisted_filter_series,


            textures: HashMap::new(),
            pending_covers: HashSet::new(),
            pending_texture_uploads: VecDeque::new(),
            pending_texture_urls: HashSet::new(),
            pending_decode_urls: HashSet::new(),
            decode_sem: Arc::new(Semaphore::new(2)),
            cover_sem: Arc::new(Semaphore::new(6)),
            cover_height: 60.0,
            search_text: String::new(),
            search_history: load_search_history(),
            show_search_history: false,
            search_status: SearchStatus::Idle,
            search_language_filter: default_search_langs,
            show_language_filter: false,
            is_loading: false,
            loading_done: 0,
            loading_total: 0,
            last_error: None,
            show_config: false,
            pending_save_config: false,
            show_server_manager: false,
            new_profile_name: String::new(),
            selected_playlist: None,
            selected_vod: None,
            selected_series: None,
            last_live_cat_id: None,
            last_vod_cat_id: None,
            last_series_cat_id: None,
            current_theme: "".into(),
            theme_applied: false,
            font_scale_applied: false,
            current_font_scale: 1.15,
            indexing: false,
            sort_key: None,
            sort_asc: true,
            tx,
            rx,
            show_error_dialog: false,
            loading_error: String::new(),
            initial_config_pending: false,
            downloads: HashMap::new(),
            download_order: Vec::new(),
            download_meta: HashMap::new(),
            index_paths: HashMap::new(),
            confirm_bulk: None,
            bulk_opts_draft: BulkOptions { only_not_downloaded: true, season: None, max_count: 0 },
            bulk_options_by_series: HashMap::new(),
            pending_bulk_downloads: Vec::new(),
            http_client: reqwest::Client::builder()
                .pool_idle_timeout(Duration::from_secs(300))
                .pool_max_idle_per_host(2)
                .tcp_nodelay(true)
                .tcp_keepalive(Some(Duration::from_secs(60)))
                .timeout(Duration::from_secs(7200))
                .connect_timeout(Duration::from_secs(30))
                .user_agent("VLC/3.0.18 LibVLC/3.0.18")
                .danger_accept_invalid_certs(true)
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            last_download_scan: None,
            should_check_downloads: false,
            should_start_search: false,
            current_view: None,
            view_stack: Vec::new(),
            wisdom_gate_recommendations: cached_recommendations,
            _wisdom_gate_last_fetch: None,
            ai_panel_tab: persisted_ai_panel_tab,
            recently_added_items: Vec::new(),
            vlc_diag_lines: VecDeque::with_capacity(128),
            vlc_diag_suggestion: None,
            has_vlc: false,
            has_mpv: false,
            vlc_version: None,
            mpv_version: None,
            detected_vlc_path: None,
            detected_mpv_path: None,
            vlc_fail_count: 0,
            mpv_fail_count: 0,
            active_diag_stop: None,
            command_preview: String::new(),
            last_frame_time: std::time::Instant::now(),
            avg_frame_ms: 0.0,
            last_forced_repaint: std::time::Instant::now(),
            pending_repaint_due_to_msg: false,
            pending_player_redetect: false,
            last_frame_wallclock: std::time::SystemTime::now(),
            post_wake_cooldown_until: None,
            
            // EPG data
            epg_data: HashMap::new(),
            epg_loading: HashSet::new(),
            
            // Update system  
            available_update: None,
            checking_for_updates: false,
            update_check_deadline: None,
            toasts: Vec::new(),
            update_downloading: false,
            update_installing: false,
            update_progress: String::new(),
            startup_auto_install: false,
        };

        // Konfig prüfen – falls unvollständig, Config Dialog anzeigen
        if !app.config_is_complete() {
            app.initial_config_pending = true;
            if !had_file {
                app.show_config = true;
            }
        } else {
            // Direkt Kategorien laden wenn Konfig vollständig
            app.reload_categories();
            // Resume eventuell vorhandene unvollständige Downloads (.part Dateien)
            app.resume_incomplete_downloads();
        }

        // SYNCHRON Player Erkennung direkt beim Start plus späteres Thread-Refresh
        app.perform_player_detection();
        {
            let tx_detect = app.tx.clone();
            let cfg_for_custom = app.config.clone();
            std::thread::spawn(move || {
                // Kleines Delay um GUI ersten Frame zu erlauben
                std::thread::sleep(std::time::Duration::from_millis(250));
                let (has_vlc, has_mpv, vlc_path, mpv_path, vlc_version, mpv_version) = MacXtreamer::detect_players(&cfg_for_custom);
                let _ = tx_detect.send(Msg::PlayerDetection { has_vlc, has_mpv, vlc_version, mpv_version, vlc_path, mpv_path });
            });
        }

        // Auto-check for updates on startup (if enabled) — always silent on startup, auto-installs if newer found
        if app.config.check_for_updates {
            app.startup_auto_install = true;
            app.check_for_updates();
        }

        app
    }
    
    
    fn add_toast(&mut self, message: String, toast_type: ToastType) {
        self.toasts.push(Toast {
            message,
            toast_type,
            created_at: std::time::Instant::now(),
        });
    }
    
    fn render_toasts(&mut self, ctx: &egui::Context) {
        const TOAST_DURATION: f32 = 5.0; // seconds
        const TOAST_FADE_OUT: f32 = 1.0; // seconds fade-out
        
        let now = std::time::Instant::now();
        
        // Remove expired toasts
        self.toasts.retain(|toast| {
            now.duration_since(toast.created_at).as_secs_f32() < TOAST_DURATION + TOAST_FADE_OUT
        });
        
        let has_active_toasts = !self.toasts.is_empty();
        
        // Render toasts
        if has_active_toasts {
            let screen_rect = ctx.screen_rect();
            let toast_width = 400.0;
            let toast_spacing = 10.0;
            let margin = 20.0;
            
            for (i, toast) in self.toasts.iter().enumerate() {
                let elapsed = now.duration_since(toast.created_at).as_secs_f32();
                let alpha = if elapsed > TOAST_DURATION {
                    1.0 - ((elapsed - TOAST_DURATION) / TOAST_FADE_OUT)
                } else {
                    1.0
                };
                
                if alpha <= 0.0 {
                    continue;
                }
                
                let y_offset = margin + (i as f32) * (60.0 + toast_spacing);
                let pos = egui::pos2(
                    screen_rect.right() - toast_width - margin,
                    screen_rect.top() + y_offset,
                );
                
                let (bg_color, text_color) = match toast.toast_type {
                    ToastType::Info => (
                        Color32::from_rgba_unmultiplied(60, 120, 180, (200.0 * alpha) as u8),
                        Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8),
                    ),
                    ToastType::Success => (
                        Color32::from_rgba_unmultiplied(60, 180, 80, (200.0 * alpha) as u8),
                        Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8),
                    ),
                    ToastType::Warning => (
                        Color32::from_rgba_unmultiplied(220, 180, 60, (200.0 * alpha) as u8),
                        Color32::from_rgba_unmultiplied(40, 40, 40, (255.0 * alpha) as u8),
                    ),
                    ToastType::Error => (
                        Color32::from_rgba_unmultiplied(200, 60, 60, (200.0 * alpha) as u8),
                        Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8),
                    ),
                };
                
                egui::Area::new(egui::Id::new(format!("toast_{}", i)))
                    .fixed_pos(pos)
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(bg_color)
                            .rounding(8.0)
                            .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                            .shadow(egui::epaint::Shadow {
                                extrusion: 8.0,
                                color: Color32::from_black_alpha((50.0 * alpha) as u8),
                            })
                            .show(ui, |ui| {
                                ui.set_max_width(toast_width - 32.0);
                                ui.style_mut().wrap = Some(true);
                                ui.label(RichText::new(&toast.message).color(text_color));
                            });
                    });
            }
            
            // Request repaint only if toasts are still animating (not at the end of their lifetime)
            let min_elapsed = self.toasts.iter()
                .map(|t| now.duration_since(t.created_at).as_secs_f32())
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0);
            
            if min_elapsed < TOAST_DURATION + TOAST_FADE_OUT {
                ctx.request_repaint_after(Duration::from_millis(100)); // Repaint less frequently (every 100ms)
            }
        }
    }

    fn reload_categories(&mut self) {
        if !self.config_is_complete() {
            return;
        }
        self.is_loading = true;
        self.loading_total = 3;
        self.loading_done = 0;
        self.last_error = None;
        let cfg_base = self.config.clone();
        let cfg_live = cfg_base.clone();
        let cfg_vod = cfg_base.clone();
        let cfg_series = cfg_base;
        let tx_live = self.tx.clone();
        let tx_vod = self.tx.clone();
        let tx_series = self.tx.clone();
        tokio::spawn(async move {
            let r = fetch_categories(&cfg_live, "get_live_categories").await;
            let _ = tx_live.send(Msg::LiveCategories(r.map_err(|e| e.to_string())));
        });
        tokio::spawn(async move {
            let r = fetch_categories(&cfg_vod, "get_vod_categories").await;
            let _ = tx_vod.send(Msg::VodCategories(r.map_err(|e| e.to_string())));
        });
        tokio::spawn(async move {
            let r = fetch_categories(&cfg_series, "get_series_categories").await;
            let _ = tx_series.send(Msg::SeriesCategories(r.map_err(|e| e.to_string())));
        });
        

    }

    // Statische Erkennung von VLC/mpv (auch für Wiederverwendung im Thread)
    fn detect_players(cfg: &Config) -> (bool,bool,Option<String>,Option<String>,Option<String>,Option<String>) {
        use std::process::Command; use std::path::Path;
        let which = |name: &str| -> Option<String> {
            Command::new("which").arg(name).output().ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let vlc_path = which("vlc");
        let mut mpv_path = if !cfg.mpv_custom_path.trim().is_empty() { Some(cfg.mpv_custom_path.trim().to_string()) } else { which("mpv") };
        if mpv_path.as_ref().map(|p| !Path::new(p).exists()).unwrap_or(false) { mpv_path = None; }
        if mpv_path.is_none() {
            let candidates = [
                "/opt/homebrew/bin/mpv",
                "/usr/local/bin/mpv",
                "/usr/bin/mpv",
                "/Applications/mpv.app/Contents/MacOS/mpv",
            ];
            for c in candidates.iter() { if Path::new(c).exists() { mpv_path = Some(c.to_string()); break; } }
        }
        let mpv_version = mpv_path.as_ref().and_then(|p| Command::new(p).arg("--version").output().ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.lines().next().unwrap_or("").to_string()));
        let vlc_version = vlc_path.as_ref().and_then(|p| Command::new(p).arg("--version").output().ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.lines().next().unwrap_or("").to_string()));
        (vlc_path.is_some(), mpv_path.is_some(), vlc_path, mpv_path, vlc_version, mpv_version)
    }

    fn perform_player_detection(&mut self) {
        let (has_vlc, has_mpv, vlc_path, mpv_path, vlc_version, mpv_version) = Self::detect_players(&self.config);
        self.has_vlc = has_vlc; self.has_mpv = has_mpv; self.detected_vlc_path = vlc_path; self.detected_mpv_path = mpv_path; self.vlc_version = vlc_version; self.mpv_version = mpv_version;
        if self.config.use_mpv && !self.has_mpv { self.config.use_mpv = false; self.last_error = Some("mpv not found – falling back to VLC".into()); self.pending_save_config = true; }
        if !self.config.use_mpv && self.has_mpv && !self.has_vlc { self.config.use_mpv = true; self.pending_save_config = true; }
    }

    fn config_is_complete(&self) -> bool {
        !self.config.address.trim().is_empty()
            && !self.config.username.trim().is_empty()
            && !self.config.password.trim().is_empty()
    }

    fn check_for_updates(&mut self) {
        if !self.checking_for_updates {
            self.checking_for_updates = true;
            // set watchdog deadline to auto-clear the UI if something goes wrong
            self.update_check_deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs(20));
            println!("🔄 Starting update check...");
            
            // Update last check timestamp
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.config.last_update_check = now;
            self.pending_save_config = true;
            
            let tx = self.tx.clone();
            
            tokio::spawn(async move {
                // Use current app version for comparison
                let current_version = env!("CARGO_PKG_VERSION");
                println!("📦 Current version: {}", current_version);
                println!("🌐 Checking GitHub for updates...");

                // Wrap the updater call in a timeout so the UI doesn't stay stuck
                let timeout_dur = tokio::time::Duration::from_secs(15);
                match tokio::time::timeout(timeout_dur, updater::check_for_updates(current_version)).await {
                    Ok(Ok(update_info)) => {
                        println!("✅ Update check successful!");
                        println!("   Latest version: {}", update_info.latest_version);
                        println!("   Update available: {}", update_info.update_available);

                        if update_info.update_available {
                            println!("📥 Sending UpdateAvailable message");
                            let _ = tx.send(Msg::UpdateAvailable(update_info));
                        } else {
                            println!("✔️ Sending NoUpdateAvailable message");
                            let _ = tx.send(Msg::NoUpdateAvailable);
                        }
                    }
                    Ok(Err(e)) => {
                        println!("❌ Update check failed: {}", e);
                        let _ = tx.send(Msg::UpdateError(format!("Update check failed: {}", e)));
                    }
                    Err(_) => {
                        println!("⏱️ Update check timed out after {}s", timeout_dur.as_secs());
                        let _ = tx.send(Msg::UpdateError(format!("Update check timed out after {}s", timeout_dur.as_secs())));
                    }
                }
            });
        }
    }
    
    fn start_update_download(&mut self, update_info: updater::UpdateInfo) {
        if let Some(ref download_url) = update_info.download_url {
            self.update_downloading = true;
            self.update_progress = "Starting download...".to_string();
            
            let tx = self.tx.clone();
            let url = download_url.clone();
            let version = update_info.latest_version.clone();
            
            let (prog_tx, mut prog_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_prog = tx.clone();
            tokio::spawn(async move {
                while let Some(msg) = prog_rx.recv().await {
                    let _ = tx_prog.send(Msg::UpdateProgress(msg));
                }
            });
            
            tokio::spawn(async move {
                println!("📥 Starting DMG download from: {}", url);
                match updater::download_and_install_update(&url, &version, Some(prog_tx)).await {
                    Ok(msg) => {
                        println!("✅ {}", msg);
                        let _ = tx.send(Msg::UpdateInstalled);
                    }
                    Err(e) => {
                        println!("❌ Update installation failed: {}", e);
                        let _ = tx.send(Msg::UpdateError(format!("Installation failed: {}", e)));
                    }
                }
            });
        } else {
            self.add_toast("⚠️ No DMG asset found in this release.".to_string(), ToastType::Warning);
        }
    }

    fn effective_config(&self) -> &Config {
        if let Some(d) = self.config_draft.as_ref() { d } else { &self.config }
    }

    fn clear_caches_and_reload(&mut self) {
        println!("🧹 [Reload] Clearing all caches and reloading...");
        // In-Memory Texturen und Cover Warteschlangen leeren
        self.textures.clear();
        self.pending_covers.clear();
        // Dateisystem Cache leeren (Images, ggf. andere)
        clear_all_caches();
        // Suchindex leeren
        self.all_movies.clear();
        self.all_series.clear();
        self.all_channels.clear();
        self.index_paths.clear();
        // Kategorien neu laden falls Konfig vollständig
        if self.config_is_complete() { 
            println!("✅ [Reload] Config complete, reloading categories...");
            self.reload_categories();
            // Don't call spawn_build_index() here - let auto-build handle it after categories load
            println!("⏳ [Reload] Auto-build will trigger after categories finish loading...");
        } else {
            println!("⚠️ [Reload] Config incomplete, skipping reload");
        }
    }

    fn create_and_play_m3u(&mut self, entries: &[(String,String)]) -> Result<(), String> {
        use std::io::Write;
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_else(|_| std::time::Duration::from_secs(0)).as_secs();
        let path = std::env::temp_dir().join(format!("macxtreamer_playlist_{}.m3u", ts));
        
        // Debug: Log playlist creation
        println!("[DEBUG] Creating binge watch playlist with {} episodes", entries.len());
        
        {
            let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
            writeln!(file, "#EXTM3U").ok();
            for (title, url) in entries { 
                println!("[DEBUG] Adding episode: {} -> {}", title, url);
                // Use proper M3U format with EXTINF duration
                writeln!(file, "#EXTINF:3600,{}", title).ok();
                writeln!(file, "{}", url).map_err(|e| e.to_string())?; 
            }
            // Explicitly flush and close the file
            file.flush().map_err(|e| e.to_string())?;
        } // File is dropped and closed here
        
        let path_str = path.to_string_lossy().to_string();
        println!("[DEBUG] Playing M3U playlist: {}", path_str);
        
        // Verify file exists and has content
        if let Ok(content) = std::fs::read_to_string(&path) {
            println!("[DEBUG] M3U Content:\n{}", content);
        }
        
        // Try to play the first episode directly if M3U fails
        if let Some((first_title, first_url)) = entries.first() {
            println!("[DEBUG] VLC M3U fallback: Starting with first episode: {}", first_title);
            
            // First try the M3U playlist
            println!("[DEBUG] Attempting to start M3U playlist with VLC...");
            match start_player(self.effective_config(), &path_str) {
                Ok(_) => {
                    println!("[DEBUG] ✅ M3U playlist started successfully");
                    Ok(())
                }
                Err(e) => {
                    println!("[DEBUG] ❌ M3U playlist failed: {}", e);
                    println!("[DEBUG] Trying direct URL fallback...");
                    // Fallback to direct URL
                    match start_player(self.effective_config(), first_url) {
                        Ok(_) => {
                            println!("[DEBUG] ✅ Direct URL playback started successfully");
                            Ok(())
                        }
                        Err(e2) => {
                            let error = format!("Both M3U playlist and direct URL failed: M3U={}, Direct={}", e, e2);
                            println!("[DEBUG] ❌ {}", error);
                            Err(error)
                        }
                    }
                }
            }
        } else {
            Err("No episodes to play".to_string())
        }
    }

    fn generate_m3u_content(&self, _category_id: &str) -> String {
        let mut content = String::from("#EXTM3U\n");
        
        // Get all channels for this category
        for item in &self.all_channels {
            // Build stream URL for this channel
            let url = crate::player::build_stream_url(&self.config, &item.id);
            content.push_str(&format!("#EXTINF:-1,{}\n", item.name));
            content.push_str(&format!("{}\n", url));
        }
        
        content
    }
    
    fn copy_to_clipboard(&mut self, text: String) {
        use arboard::Clipboard;
        match Clipboard::new() {
            Ok(mut clipboard) => {
                if clipboard.set_text(&text).is_ok() {
                    self.last_error = Some("✅ Copied to clipboard!".into());
                } else {
                    self.last_error = Some("❌ Failed to copy to clipboard".into());
                }
            }
            Err(_) => {
                self.last_error = Some("❌ Clipboard not available".into());
            }
        }
    }
    
    fn save_m3u_file(&mut self, content: String, category_name: &str) {
        use std::io::Write;
        
        // Use native file dialog
        let default_name = format!("{}.m3u", category_name.replace(" ", "_"));
        
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter("M3U Playlist", &["m3u"])
            .save_file()
        {
            match std::fs::File::create(&path) {
                Ok(mut file) => {
                    if file.write_all(content.as_bytes()).is_ok() {
                        self.last_error = Some(format!("✅ Saved to {}", path.display()));
                    } else {
                        self.last_error = Some("❌ Failed to write file".into());
                    }
                }
                Err(e) => {
                    self.last_error = Some(format!("❌ Failed to create file: {}", e));
                }
            }
        }
    }
    
    fn play_all_channels(&mut self, _category_id: &str, _category_name: &str) {
        let mut entries: Vec<(String, String)> = Vec::new();
        
        for item in &self.all_channels {
            let url = crate::player::build_stream_url(&self.config, &item.id);
            entries.push((item.name.clone(), url));
        }
        
        if entries.is_empty() {
            self.last_error = Some("No channels found in this category".into());
            return;
        }
        
        if let Err(e) = self.create_and_play_m3u(&entries) {
            self.last_error = Some(format!("Failed to play playlist: {}", e));
        }
    }

    fn resume_incomplete_downloads(&mut self) {
        // Temporär minimal: alte fehlerhafte Logik entfernt nach Patch-Kollision.
        // TODO: Reimplementierung des fortsetzbaren Download-Scans.
        if !self.config.enable_downloads { return; }
        // Placeholder – macht aktuell nichts außer schneller Rückkehr.
    }

    // Lade Items einer Kategorie (Live/VOD/Series) asynchron und sende Ergebnis zurück.
    fn spawn_load_items(&mut self, kind: &str, category_id: String) {
        if !self.config_is_complete() { return; }
        let tx = self.tx.clone();
        let cfg = self.config.clone();
        let kind_s = kind.to_string();
        self.is_loading = true;
        self.loading_total = 1;
        self.loading_done = 0;
        tokio::spawn(async move {
            let action = match kind_s.as_str() { "subplaylist" => "get_live_streams", "vod" => "get_vod_streams", "series" => "get_series", other => other };
            let url = format!("{}/player_api.php?username={}&password=***&action={}&category_id={}", cfg.address, cfg.username, action, category_id);
            let res = fetch_items(&cfg, &kind_s, &category_id).await.map_err(|e| format!("{} (URL: {})", e, url));
            let _ = tx.send(Msg::ItemsLoaded { kind: kind_s, items: res });
        });
    }

    // Lade Episoden einer Serie
    fn spawn_load_episodes(&mut self, series_id: String) {
        if !self.config_is_complete() { return; }
        let tx = self.tx.clone();
        let cfg = self.config.clone();
        self.is_loading = true;
        self.loading_total = 1;
        self.loading_done = 0;
        tokio::spawn(async move {
            let url = format!("{}/player_api.php?username={}&password=***&action=get_series_info&series_id={}", cfg.address, cfg.username, series_id);
            let res = fetch_series_episodes(&cfg, &series_id).await.map_err(|e| format!("{} (URL: {})", e, url));
            let _ = tx.send(Msg::EpisodesLoaded { series_id, episodes: res });
        });
    }

    // Episoden für Bulk-Download laden (nur Enqueue – keine UI-Anzeige)
    fn spawn_fetch_episodes_for_download(&mut self, series_id: String) {
        if !self.config_is_complete() { return; }
        let tx = self.tx.clone();
        let cfg = self.config.clone();
        tokio::spawn(async move {
            let res = fetch_series_episodes(&cfg, &series_id).await.map_err(|e| e.to_string());
            let _ = tx.send(Msg::SeriesEpisodesForDownload { series_id, episodes: res });
        });
    }
    
    // Load EPG data for a live channel
    fn spawn_load_epg(&mut self, stream_id: String) {
        if !self.config_is_complete() { return; }
        if self.epg_loading.contains(&stream_id) { return; } // already loading
        
        self.epg_loading.insert(stream_id.clone());
        let tx = self.tx.clone();
        let cfg = self.config.clone();
        let stream_id_clone = stream_id.clone();
        
        tokio::spawn(async move {
            use crate::api::fetch_short_epg;
            match fetch_short_epg(&cfg, &stream_id_clone).await {
                Ok(Some(program)) => {
                    // Format: "Title (until HH:MM)"
                    let program_text = if !program.end.is_empty() {
                        // Try to extract just the time (HH:MM) from the end timestamp
                        let time_part = program.end.split_whitespace().last().unwrap_or(&program.end);
                        format!("{} (until {})", program.title, time_part)
                    } else {
                        program.title
                    };
                    let _ = tx.send(Msg::EpgLoaded { 
                        stream_id: stream_id_clone, 
                        program: Some(program_text) 
                    });
                }
                Ok(None) => {
                    // No EPG data available - normal for many channels
                    let _ = tx.send(Msg::EpgLoaded { 
                        stream_id: stream_id_clone, 
                        program: None 
                    });
                }
                Err(e) => {
                    if e.is_timeout() || e.is_connect() {
                        eprintln!("EPG network error for {}: {}", stream_id_clone, e);
                    }
                    let _ = tx.send(Msg::EpgLoaded { 
                        stream_id: stream_id_clone, 
                        program: None 
                    });
                }
            }
        });
    }

    // Download all episodes starting from a specific episode
    fn download_episodes_from_here(&mut self, rows: &[Row], starting_row: &Row) {
        if !self.config.enable_downloads { return; }
        
        // Find the index of the starting episode
        let start_idx = rows.iter().position(|row| row.id == starting_row.id);
        if let Some(idx) = start_idx {
            // Get all episodes from this point onwards
            let episodes_to_download: Vec<&Row> = rows[idx..]
                .iter()
                .filter(|row| row.info == "SeriesEpisode")
                .collect();
            
            println!("📥 Queuing {} episodes for download starting from: {}", 
                episodes_to_download.len(), starting_row.name);
            
            let count = episodes_to_download.len();
            
            // Queue each episode for download
            for episode in &episodes_to_download {
                self.spawn_download(episode);
            }
            
            // Show confirmation message using localized strings
            let message = format!("{} {} {} '{}'", 
                count,
                t("episodes_queued", self.config.language), 
                "", // connector word  
                starting_row.name);
            self.last_error = Some(message);
        }
    }

    // Einzelnes Cover abrufen (falls nicht bereits vorhanden oder in Arbeit)
    fn spawn_fetch_cover(&mut self, url: &str) {
        if url.is_empty() { return; }
        if self.textures.contains_key(url) { return; }
        if self.pending_covers.contains(url) { return; }
        if self.pending_decode_urls.contains(url) { return; }
        // Mark as pending
        self.pending_covers.insert(url.to_string());
        let tx = self.tx.clone();
        let client = self.http_client.clone();
        let url_s = url.to_string();
        tokio::spawn(async move {
            match client.get(&url_s).send().await {
                Ok(resp) => {
                    if let Ok(bytes) = resp.bytes().await { let _ = tx.send(Msg::CoverLoaded { url: url_s, bytes: bytes.to_vec() }); }
                }
                Err(_e) => {
                    // Fehler ignorieren – Eintrag wird später bereinigt
                }
            }
        });
    }

    fn spawn_build_index(&mut self) {
        println!("🔧 [spawn_build_index] Called - indexing={}, config_complete={}", self.indexing, self.config_is_complete());
        if self.indexing {
            println!("⚠️ [spawn_build_index] Already indexing, skipping");
            return;
        }
        if !self.config_is_complete() {
            println!("⚠️ [spawn_build_index] Config incomplete, skipping");
            return;
        }
        
        // Set indexing flag early to prevent concurrent builds
        self.indexing = true;
        println!("🚀 [spawn_build_index] Set indexing=true");
        
        // Versuche zuerst, den Index von Disk zu laden
        if let Some((movies, series, channels, paths)) = crate::storage::load_search_index(&self.config.address, &self.config.username) {
            println!("✨ Index loaded from cache: {} movies, {} series, {} channels", movies.len(), series.len(), channels.len());
            
            // Only use cache if it actually has data
            let total_items = movies.len() + series.len() + channels.len();
            if total_items > 0 {
                println!("✅ Cache has data, using it");
                self.all_movies = movies.clone();
                self.all_series = series.clone();
                self.all_channels = channels.clone();
                self.index_paths = paths.clone();
                
                // Sende auch als IndexData für UI-Update
                let movies_with_paths: Vec<(Item, String)> = movies.into_iter()
                    .map(|item| {
                        let path = paths.get(&item.id).cloned().unwrap_or_default();
                        (item, path)
                    })
                    .collect();
                let series_with_paths: Vec<(Item, String)> = series.into_iter()
                    .map(|item| {
                        let path = paths.get(&item.id).cloned().unwrap_or_default();
                        (item, path)
                    })
                    .collect();
                let channels_with_paths: Vec<(Item, String)> = channels.into_iter()
                    .map(|item| {
                        let path = paths.get(&item.id).cloned().unwrap_or_default();
                        (item, path)
                    })
                    .collect();
                
                let _ = self.tx.send(Msg::IndexData { 
                    movies: movies_with_paths, 
                    series: series_with_paths, 
                    channels: channels_with_paths 
                });
                let _ = self.tx.send(Msg::IndexBuilt { 
                    movies: self.all_movies.len(), 
                    series: self.all_series.len(), 
                    channels: self.all_channels.len() 
                });
                // Reset indexing flag since we used cache
                self.indexing = false;
                println!("✅ [spawn_build_index] Cache used, set indexing=false");
                return;
            } else {
                println!("⚠️ Cache is empty, building fresh index instead");
            }
        }
        
        // Kein Cache vorhanden oder ungültig - baue Index neu auf
        // indexing flag already set above
        println!("📂 [spawn_build_index] Starting async index build task");
        self.search_status = SearchStatus::Indexing { progress: "Starte Index-Aufbau...".to_string() };
        let tx = self.tx.clone();
        let cfg = self.config.clone();
        tokio::spawn(async move {
            println!("📂 Starte Index-Aufbau...");
            let _ = tx.send(Msg::IndexProgress { message: "Lade Kategorien...".to_string() });
            println!("⚡ Lade alle Kategorien parallel...");
            let _ = tx.send(Msg::IndexProgress { message: "Lade Kategorien parallel...".to_string() });
            let (vod_result, ser_result, live_result) = tokio::join!(
                fetch_categories(&cfg, "get_vod_categories"),
                fetch_categories(&cfg, "get_series_categories"),
                fetch_categories(&cfg, "get_live_categories")
            );
            
            let vod = match vod_result {
                Ok(cats) => { println!("✅ VOD: {} Kategorien", cats.len()); cats }
                Err(e) => { println!("❌ VOD Fehler: {}", e); Vec::new() }
            };
            let ser = match ser_result {
                Ok(cats) => { println!("✅ Series: {} categories", cats.len()); cats }
                Err(e) => { println!("❌ Series error: {}", e); Vec::new() }
            };
            let live = match live_result {
                Ok(cats) => { println!("✅ Live: {} Kategorien", cats.len()); cats }
                Err(e) => { println!("❌ Live Fehler: {}", e); Vec::new() }
            };
            let mut all_movies: Vec<(Item,String)> = Vec::new();
            let mut all_series: Vec<(Item,String)> = Vec::new();
            let mut all_channels: Vec<(Item,String)> = Vec::new();
            let total_categories = vod.len() + ser.len() + live.len();
            let _ = tx.send(Msg::IndexProgress { message: format!("Lade Inhalte aus {} Kategorien...", total_categories) });
            println!("⚡ Lade Items parallel...");
            
            // Parallel loading mit begrenzter Concurrency (max 10 gleichzeitig)
            use futures::stream::{self, StreamExt};
            
            // VOD Items parallel laden
            if !vod.is_empty() {
                let vod_futures = vod.into_iter().map(|c| {
                    let cfg = cfg.clone();
                    let path = format!("VOD / {}", c.name);
                    let name = c.name.clone();
                    async move {
                        match fetch_items(&cfg, "vod", &c.id).await {
                            Ok(items) => {
                                let count = items.len();
                                (Ok(items.into_iter().map(|it| (it, path.clone())).collect::<Vec<_>>()), name, count)
                            }
                            Err(e) => (Err(e), name, 0)
                        }
                    }
                });
                let vod_results: Vec<_> = stream::iter(vod_futures)
                    .buffer_unordered(10)
                    .collect().await;
                
                for (result, name, count) in vod_results {
                    match result {
                        Ok(items) => {
                            println!("🎬 {} Movies ({})", count, name);
                            all_movies.extend(items);
                        }
                        Err(e) => println!("❌ VOD '{}': {}", name, e)
                    }
                }
            }
            
            // Series Items parallel laden  
            if !ser.is_empty() {
                let ser_futures = ser.into_iter().map(|c| {
                    let cfg = cfg.clone();
                    let path = format!("Series / {}", c.name);
                    let name = c.name.clone();
                    async move {
                        match fetch_items(&cfg, "series", &c.id).await {
                            Ok(items) => {
                                let count = items.len();
                                (Ok(items.into_iter().map(|it| (it, path.clone())).collect::<Vec<_>>()), name, count)
                            }
                            Err(e) => (Err(e), name, 0)
                        }
                    }
                });
                let ser_results: Vec<_> = stream::iter(ser_futures)
                    .buffer_unordered(10)
                    .collect().await;
                
                for (result, name, count) in ser_results {
                    match result {
                        Ok(items) => {
                            println!("📚 {} Series ({})", count, name);
                            all_series.extend(items);
                        }
                        Err(e) => println!("❌ Series '{}': {}", name, e)
                    }
                }
            }
            
            // Live Channels parallel laden
            if !live.is_empty() {
                let live_futures = live.into_iter().map(|c| {
                    let cfg = cfg.clone();
                    let path = format!("Live / {}", c.name);
                    let name = c.name.clone();
                    async move {
                        match fetch_items(&cfg, "subplaylist", &c.id).await {
                            Ok(items) => {
                                let count = items.len();
                                (Ok(items.into_iter().map(|it| (it, path.clone())).collect::<Vec<_>>()), name, count)
                            }
                            Err(e) => (Err(e), name, 0)
                        }
                    }
                });
                let live_results: Vec<_> = stream::iter(live_futures)
                    .buffer_unordered(10)
                    .collect().await;
                
                for (result, name, count) in live_results {
                    match result {
                        Ok(items) => {
                            println!("📡 {} Channels ({})", count, name);
                            all_channels.extend(items);
                        }
                        Err(e) => println!("❌ Live '{}': {}", name, e)
                    }
                }
            }
            // Deduplicate items using a HashMap to keep the first occurrence
            let mut movies_map: std::collections::HashMap<String, (Item, String)> = std::collections::HashMap::new();
            let mut series_map: std::collections::HashMap<String, (Item, String)> = std::collections::HashMap::new();
            let mut channels_map: std::collections::HashMap<String, (Item, String)> = std::collections::HashMap::new();
            
            for (item, path) in all_movies {
                movies_map.entry(item.id.clone()).or_insert((item, path));
            }
            for (item, path) in all_series {
                series_map.entry(item.id.clone()).or_insert((item, path));
            }
            for (item, path) in all_channels {
                channels_map.entry(item.id.clone()).or_insert((item, path));
            }
            
            let deduplicated_movies: Vec<(Item, String)> = movies_map.into_iter().map(|(_, v)| v).collect();
            let deduplicated_series: Vec<(Item, String)> = series_map.into_iter().map(|(_, v)| v).collect();
            let deduplicated_channels: Vec<(Item, String)> = channels_map.into_iter().map(|(_, v)| v).collect();
            
            let movies_len = deduplicated_movies.len();
            let series_len = deduplicated_series.len();
            let channels_len = deduplicated_channels.len();
            
            println!("✅ Index build complete: {} movies, {} series, {} channels", movies_len, series_len, channels_len);
            
            // Speichere Index auf Disk
            let movies_only: Vec<Item> = deduplicated_movies.iter().map(|(item, _)| item.clone()).collect();
            let series_only: Vec<Item> = deduplicated_series.iter().map(|(item, _)| item.clone()).collect();
            let channels_only: Vec<Item> = deduplicated_channels.iter().map(|(item, _)| item.clone()).collect();
            let mut paths_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            for (item, path) in &deduplicated_movies {
                paths_map.insert(item.id.clone(), path.clone());
            }
            for (item, path) in &deduplicated_series {
                paths_map.insert(item.id.clone(), path.clone());
            }
            for (item, path) in &deduplicated_channels {
                paths_map.insert(item.id.clone(), path.clone());
            }
            crate::storage::save_search_index(&movies_only, &series_only, &channels_only, &paths_map, &cfg.address, &cfg.username);
            
            let _ = tx.send(Msg::IndexData { movies: deduplicated_movies, series: deduplicated_series, channels: deduplicated_channels });
            let _ = tx.send(Msg::IndexBuilt { movies: movies_len, series: series_len, channels: channels_len });
        });
    }

    fn start_search(&mut self) {
        let tx = self.tx.clone();
        let movies = self.all_movies.clone();
        let series = self.all_series.clone();
        let channels = self.all_channels.clone();
        let query = self.search_text.clone();
        let language_filter = self.search_language_filter.clone();
        
        // Debug-Informationen für die Suche
        println!("🔍 Starte Suche mit Query: '{}'", query);
        println!("🌐 Sprach-Filter: {:?}", language_filter);
        println!("📊 Index sizes: movies={}, series={}, channels={}", movies.len(), series.len(), channels.len());
        println!("⚙️ Indexing: {}, Config komplett: {}", self.indexing, self.config_is_complete());
        
        if movies.is_empty() && series.is_empty() && channels.is_empty() && !self.indexing {
            println!("📂 Index leer, starte Index-Aufbau...");
            self.spawn_build_index(); return; }
        if self.indexing { 
            println!("⏳ Index build already in progress...");
            return; 
        }
        
        // CRITICAL: Set search view BEFORE starting search to prevent ItemsLoaded from interfering
        if !matches!(self.current_view, Some(ViewState::Search { .. })) {
            println!("🔄 Setze View auf Search");
            if let Some(cv) = &self.current_view {
                self.view_stack.push(cv.clone());
            }
            self.current_view = Some(ViewState::Search { 
                query: query.clone() 
            });
        }
        
        // Send search started message and update status
        let _ = tx.send(Msg::SearchStarted);
        self.search_status = SearchStatus::Searching;
        self.is_loading = true; self.loading_total = 1; self.loading_done = 0;
        
        tokio::spawn(async move {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                println!("🔎 Running search...");
                search_items_with_language_filter(&movies, &series, &channels, &query, &language_filter)
            })) {
                Ok(results) => {
                    println!("✅ Suche abgeschlossen, {} Treffer gefunden", results.len());
                    
                    // Deduplicate results by ID before converting to Rows
                    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
                    let unique_results: Vec<_> = results.into_iter()
                        .filter(|s| seen_ids.insert(s.id.clone()))
                        .collect();
                    
                    let rows: Vec<Row> = unique_results.into_iter().map(|s| Row {
                        name: s.name.clone(), 
                        id: s.id, 
                        info: s.info, 
                        container_extension: if s.container_extension.is_empty(){None}else{Some(s.container_extension)}, 
                        stream_url: None, 
                        cover_url: s.cover, 
                        year: s.year.clone(), 
                        release_date: s.release_date.clone().or_else(|| extract_year_from_title(&s.name)), 
                        rating_5based: s.rating_5based, 
                        genre: s.genre, 
                        path: None,
                        audio_languages: None,
                    }).collect();
                    
                    let result_count = rows.len();
                    println!("📊 {} unique results after deduplication", result_count);
                    let _ = tx.send(Msg::SearchReady(rows));
                    
                    if result_count == 0 {
                        println!("❌ No search results");
                        let _ = tx.send(Msg::SearchCompleted { results: 0 });
                    } else {
                        println!("📤 Sending {} results to UI", result_count);
                        let _ = tx.send(Msg::SearchCompleted { results: result_count });
                    }
                },
                Err(e) => {
                    println!("💥 Suchfehler: {:?}", e);
                    let _ = tx.send(Msg::SearchFailed { error: "Suche fehlgeschlagen".to_string() });
                }
            }
        });
    }

    fn spawn_preload_all(&mut self) {
        if !self.config_is_complete() {
            return;
        }
        // Parallel preloading with concurrent requests
        let cfg = self.config.clone();
        let tx = self.tx.clone();
        self.is_loading = true;
        self.loading_done = 0;
        self.loading_total = 0; // wird gleich gesetzt
        tokio::spawn(async move {
            // Fetch all categories in parallel
            let (vod_result, ser_result, live_result) = tokio::join!(
                fetch_categories(&cfg, "get_vod_categories"),
                fetch_categories(&cfg, "get_series_categories"),
                fetch_categories(&cfg, "get_live_categories")
            );

            let vod = vod_result.unwrap_or_default();
            let ser = ser_result.unwrap_or_default();
            let live = live_result.unwrap_or_default();

            // Calculate total steps for progress tracking
            let total_steps = vod.len() + ser.len() + live.len();
            let _ = tx.send(Msg::PreloadSet {
                total: total_steps.max(1),
            });

            // Create semaphore to limit concurrent requests (avoid overwhelming server)
            let category_parallel = if cfg.category_parallel == 0 {
                6
            } else {
                cfg.category_parallel
            } as usize;
            let sem = Arc::new(tokio::sync::Semaphore::new(category_parallel));
            let mut live_tasks = Vec::new();
            let mut content_tasks = Vec::new();

            // Spawn live stream loading tasks (no cover URLs needed)
            for c in live {
                let cfg_clone = cfg.clone();
                let tx_clone = tx.clone();
                let sem_clone = sem.clone();
                let task = tokio::spawn(async move {
                    let _permit = sem_clone.acquire().await.ok();
                    let _ = fetch_items(&cfg_clone, "subplaylist", &c.id).await;
                    let _ = tx_clone.send(Msg::PreloadTick);
                });
                live_tasks.push(task);
            }

            // Spawn VOD loading tasks (collect cover URLs)
            for c in vod {
                let cfg_clone = cfg.clone();
                let tx_clone = tx.clone();
                let sem_clone = sem.clone();
                let c_id = c.id.clone();
                let task = tokio::spawn(async move {
                    let _permit = sem_clone.acquire().await.ok();
                    let mut urls = Vec::new();
                    if let Ok(items) = fetch_items(&cfg_clone, "vod", &c_id).await {
                        for it in &items {
                            if let Some(cu) = &it.cover {
                                urls.push(cu.clone());
                            }
                        }
                    }
                    let _ = tx_clone.send(Msg::PreloadTick);
                    urls
                });
                content_tasks.push(task);
            }

            // Spawn series loading tasks (collect cover URLs)
            for c in ser {
                let cfg_clone = cfg.clone();
                let tx_clone = tx.clone();
                let sem_clone = sem.clone();
                let c_id = c.id.clone();
                let task = tokio::spawn(async move {
                    let _permit = sem_clone.acquire().await.ok();
                    let mut urls = Vec::new();
                    if let Ok(items) = fetch_items(&cfg_clone, "series", &c_id).await {
                        for it in &items {
                            if let Some(cu) = &it.cover {
                                urls.push(cu.clone());
                            }
                        }
                    }
                    let _ = tx_clone.send(Msg::PreloadTick);
                    urls
                });
                content_tasks.push(task);
            }

            // Wait for live tasks to complete
            for task in live_tasks {
                let _ = task.await;
            }

            // Wait for content tasks and collect cover URLs
            let mut cover_urls = Vec::new();
            for task in content_tasks {
                if let Ok(urls) = task.await {
                    cover_urls.extend(urls);
                }
            }

            // Remove duplicates and send for prefetching
            cover_urls.sort();
            cover_urls.dedup();
            let _ = tx.send(Msg::PrefetchCovers(cover_urls));
        });
    }

    fn expand_download_dir(&self) -> PathBuf {
        expand_download_dir(&self.config.download_dir)
    }


    fn local_file_path(&self, id: &str, name: &str, container_ext: Option<&str>) -> PathBuf {
        // Filename now based on (sanitized) title instead of id.
        let mut dir = self.expand_download_dir();
        let ext = container_ext.unwrap_or("mp4").trim_start_matches('.');
        let mut base = sanitize_filename(name);
        if base.len() < 2 {
            base = id.to_string();
        }
        let filename = format!("{base}.{ext}");
        dir.push(filename);
        dir
    }

    fn local_file_exists(
        &self,
        id: &str,
        name: &str,
        container_ext: Option<&str>,
    ) -> Option<PathBuf> {
        let p = self.local_file_path(id, name, container_ext);
        if p.exists() { Some(p) } else { None }
    }

    // (Old local_file_exists(id, ext) removed)



    fn spawn_download(&mut self, row: &Row) {
        if !self.config_is_complete() {
            return;
        }
        let id = row.id.clone();
        if self
            .downloads
            .get(&id)
            .map(|d| d.finished && d.error.is_none())
            .unwrap_or(false)
        {
            return;
        }
        // If file already on disk (maybe previous session) play immediately
        if let Some(path) =
            self.local_file_exists(&id, &row.name, row.container_extension.as_deref())
        {
            let uri = file_path_to_uri(&path);
            let _ = start_player(self.effective_config(), &uri);
            return;
        }
        // If currently downloading just ignore
        if self.downloads.get(&id).is_some() {
            return;
        }

        if row.info == "Channel" {
            return;
        }
        let meta = DownloadMeta {
            id: row.id.clone(),
            name: row.name.clone(),
            info: row.info.clone(),
            container_extension: row.container_extension.clone(),
            size: None,
            modified: None,
        };
        self.download_meta.insert(id.clone(), meta);
        self.download_order.push(id.clone());
        self.downloads.insert(
            id.clone(),
            DownloadState {
                waiting: true,
                path: Some(
                    self.local_file_path(&row.id, &row.name, row.container_extension.as_deref())
                        .to_string_lossy()
                        .into(),
                ),
                ..Default::default()
            },
        );
        self.maybe_start_next_download();
    }

    // Enqueue a download job without auto-playing if the file already exists (used for bulk)
    fn spawn_download_bulk(
        &mut self,
        id: String,
        name: String,
        info: String,
        container_extension: Option<String>,
    ) {
        if !self.config_is_complete() {
            return;
        }
        if self
            .downloads
            .get(&id)
            .map(|d| d.finished && d.error.is_none())
            .unwrap_or(false)
        {
            return;
        }
        // Skip if file already exists (no auto-play in bulk)
        if self
            .local_file_exists(&id, &name, container_extension.as_deref())
            .is_some()
        {
            return;
        }
        // If currently downloading just ignore
        if self.downloads.get(&id).is_some() {
            return;
        }
        if info == "Channel" {
            return;
        }
        let meta = DownloadMeta {
            id: id.clone(),
            name: name.clone(),
            info: info.clone(),
            container_extension: container_extension.clone(),
            size: None,
            modified: None,
        };
        self.download_meta.insert(id.clone(), meta);
        self.download_order.push(id.clone());
        self.downloads.insert(
            id.clone(),
            DownloadState {
                waiting: true,
                path: Some(
                    self.local_file_path(&id, &name, container_extension.as_deref())
                        .to_string_lossy()
                        .into(),
                ),
                ..Default::default()
            },
        );
        self.maybe_start_next_download();
    }

    fn active_downloads(&self) -> usize {
        self.downloads
            .values()
            .filter(|s| !s.waiting && !s.finished && s.error.is_none())
            .count()
    }

    fn maybe_start_next_download(&mut self) {
        let max_parallel = if self.config.max_parallel_downloads == 0 { 1 } else { self.config.max_parallel_downloads as usize };
        if self.active_downloads() >= max_parallel {
            return;
        }
        let next_id = match self.download_order.iter().find(|id| {
            self.downloads
                .get(*id)
                .map(|s| s.waiting && s.error.is_none())
                .unwrap_or(false)
        }) {
            Some(id) => id.clone(),
            None => return,
        };
        if let Some(st) = self.downloads.get_mut(&next_id) {
            st.waiting = false;
        }
        let meta = match self.download_meta.get(&next_id) {
            Some(m) => m.clone(),
            None => return,
        };
        let url = build_url_by_type(
            &self.config,
            &meta.id,
            &meta.info,
            meta.container_extension.as_deref(),
        );
        let target_path =
            self.local_file_path(&meta.id, &meta.name, meta.container_extension.as_deref());
        let tmp_path = target_path.with_extension(format!(
            "{}.part",
            target_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("tmp")
        ));
        let cancel_flag = self
            .downloads
            .get(&next_id)
            .and_then(|d| d.cancel_flag.clone())
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
        let tx = self.tx.clone();
        let id = next_id.clone();
        let cfg_clone = self.config.clone();
        tokio::spawn(async move {
            let attempts_max = cfg_clone.download_retry_max.max(1) as usize;
            let delay_ms = if cfg_clone.download_retry_delay_ms == 0 { 1000 } else { cfg_clone.download_retry_delay_ms } as u64;
            let client = reqwest::Client::builder()
                .tcp_nodelay(true)
                .tcp_keepalive(Some(Duration::from_secs(60)))
                .timeout(Duration::from_secs(7200))
                .connect_timeout(Duration::from_secs(30))
                .user_agent("VLC/3.0.18 LibVLC/3.0.18")
                .danger_accept_invalid_certs(true)
                .build()
                .unwrap();
            if let Some(parent) = target_path.parent() { let _ = tokio::fs::create_dir_all(parent).await; }
            // Sidecar schreiben (für Resume) falls noch nicht vorhanden
            if let Some(ext) = target_path.extension().and_then(|e| e.to_str()) {
                let sidecar = target_path.with_extension(format!("{}.json", ext));
                if !sidecar.exists() {
                    let js = serde_json::json!({"id": meta.id, "name": meta.name, "info": meta.info, "ext": meta.container_extension.as_deref().unwrap_or("mp4")});
                    if let Ok(data) = serde_json::to_vec(&js) { let _ = tokio::fs::write(&sidecar, &data).await; }
                }
            }
            let mut attempt = 0usize;
            let mut final_total: Option<u64> = None;
            loop {
                if cancel_flag.load(Ordering::Relaxed) { let _ = tx.send(Msg::DownloadCancelled { id: id.clone() }); return; }
                // Aktuelle Teilgröße bestimmen (Resume)
                let existing_len = match tokio::fs::metadata(&tmp_path).await { Ok(m)=>m.len(), Err(_)=>0 };
                // Datei öffnen (append oder create)
                let mut file = if existing_len > 0 { tokio::fs::OpenOptions::new().append(true).open(&tmp_path).await.unwrap() } else { tokio::fs::File::create(&tmp_path).await.unwrap() };
                println!("Download attempt {}/{} id={} resume_from={}", attempt+1, attempts_max, id, existing_len);
                log_line(&format!("Download attempt {}/{} id={} resume_from={} bytes", attempt+1, attempts_max, id, existing_len));
                if attempt == 0 && existing_len > 0 {
                    // Sofort Fortschritt melden vor neuem Request
                    let _ = tx.send(Msg::DownloadProgress { id: id.clone(), received: existing_len, total: None });
                }
                let mut req = client.get(&url);
                if existing_len > 0 { req = req.header(reqwest::header::RANGE, format!("bytes={}-", existing_len)); }
                let resp = match req.send().await { 
                    Ok(r) => r, 
                    Err(e) => { 
                        let err_msg = e.to_string();
                        let err = if err_msg.contains("timeout") || err_msg.contains("connection") {
                            format!("Network error (VPN-related?): {}. Try disconnecting VPN if using one.", e)
                        } else {
                            format!("Network error: {}", e)
                        };
                        println!("{}", err); 
                        log_line(&err); 
                        attempt+=1; 
                        if attempt>=attempts_max { 
                            let _=tx.send(Msg::DownloadError { id: id.clone(), error: err }); 
                            return; 
                        } else { 
                            // Longer delay for potential VPN issues
                            let retry_delay = if err_msg.contains("timeout") { delay_ms * 2 } else { delay_ms };
                            tokio::time::sleep(Duration::from_millis(retry_delay)).await; 
                            continue; 
                        } 
                    } 
                };
                if resp.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                    // Möglicherweise schon komplett -> rename falls final nicht existiert
                    if !target_path.exists() { let _ = tokio::fs::rename(&tmp_path, &target_path).await; }
                    let _ = tx.send(Msg::DownloadFinished { id: id.clone(), path: target_path.to_string_lossy().into() });
                    return;
                }
                if !resp.status().is_success() {
                    let status_code = resp.status();
                    let err = match status_code.as_u16() {
                        458 => "HTTP 458: Server anti-bot protection triggered. Retrying with delay...".to_string(),
                        403 => "HTTP 403: Access forbidden. Server may be blocking requests.".to_string(),
                        429 => "HTTP 429: Too many requests. Server rate limiting active.".to_string(),
                        451 => "HTTP 451: Content unavailable for legal reasons.".to_string(),
                        _ => format!("HTTP {}: Server error", status_code)
                    };
                    println!("{}", err); 
                    log_line(&err);
                    
                    // For HTTP 458 and 403, use exponential backoff
                    let is_anti_bot = matches!(status_code.as_u16(), 458 | 403);
                    let retry_delay = if is_anti_bot {
                        // Exponential backoff: 2s, 4s, 8s, etc.
                        delay_ms * 2u64.pow(attempt as u32)
                    } else if status_code.as_u16() == 429 {
                        delay_ms * 3
                    } else {
                        delay_ms
                    };
                    
                    attempt+=1; 
                    if attempt>=attempts_max { 
                        let final_err = if is_anti_bot {
                            format!("{} (Hint: Server may be detecting automated downloads. Try downloading manually or waiting a few minutes)", err)
                        } else {
                            err
                        };
                        let _=tx.send(Msg::DownloadError { id: id.clone(), error: final_err }); 
                        return; 
                    } else { 
                        println!("Retrying in {}ms (attempt {}/{})...", retry_delay, attempt, attempts_max);
                        tokio::time::sleep(Duration::from_millis(retry_delay)).await; 
                        continue; 
                    }
                }
                // Total Größe bestimmen
                let total_opt = if resp.status()==reqwest::StatusCode::PARTIAL_CONTENT {
                    if let Some(cr) = resp.headers().get(reqwest::header::CONTENT_RANGE).and_then(|v| v.to_str().ok()) {
                        // Format bytes start-end/total
                        if let Some((_,rest)) = cr.split_once(' ') { if let Some((_range,tot)) = rest.split_once('/') { tot.parse::<u64>().ok() } else { None } } else { None }
                    } else { None }
                } else { resp.content_length() };
                if final_total.is_none() { final_total = total_opt; }
                if attempt == 0 { let _ = tx.send(Msg::DownloadStarted { id: id.clone(), path: target_path.to_string_lossy().into() }); }
                if existing_len > 0 && final_total.is_some() {
                    let _ = tx.send(Msg::DownloadProgress { id: id.clone(), received: existing_len, total: final_total });
                }
                let mut received = existing_len;
                let mut last_sent = std::time::Instant::now();
                let mut stream = resp.bytes_stream();
                use futures_util::StreamExt;
                while let Some(chunk_res) = stream.next().await {
                    match chunk_res {
                        Ok(c) => {
                            if cancel_flag.load(Ordering::Relaxed) { let _=tx.send(Msg::DownloadCancelled { id: id.clone() }); return; }
                            if let Err(e)=tokio::io::AsyncWriteExt::write_all(&mut file, &c).await { let err = format!("Write error: {}", e); let _=tx.send(Msg::DownloadError { id: id.clone(), error: err }); return; }
                            received += c.len() as u64;
                            if last_sent.elapsed() > std::time::Duration::from_millis(250) { last_sent=std::time::Instant::now(); let _=tx.send(Msg::DownloadProgress { id: id.clone(), received, total: final_total }); }
                        }
                        Err(e) => { let err = format!("Stream error: {}", e); println!("{}", err); log_line(&err); break; }
                    }
                }
                // Flush
                let _ = tokio::io::AsyncWriteExt::flush(&mut file).await;
                drop(file);
                if let Some(total)=final_total { if received < total { let msg = format!("Early EOF detected id={} received={} total={}", id, received, total); println!("{}", msg); log_line(&msg); attempt+=1; if attempt<attempts_max { continue; } else { let _=tx.send(Msg::DownloadError { id: id.clone(), error: format!("Incomplete after {} attempts", attempts_max) }); return; } } }
                // Erfolgreich - rename mit Retry falls Datei noch von Player gelesen wird
                let mut rename_attempts = 0;
                loop {
                    match tokio::fs::rename(&tmp_path, &target_path).await {
                        Ok(_) => break,
                        Err(e) => {
                            rename_attempts += 1;
                            if rename_attempts > 5 {
                                let _=tx.send(Msg::DownloadError { id: id.clone(), error: format!("Rename failed after {} attempts: {}", rename_attempts, e) });
                                return;
                            }
                            // Warte kurz und versuche erneut (Player könnte Datei noch lesen)
                            log_line(&format!("Rename attempt {}/5 failed (file may be in use): {}", rename_attempts, e));
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
                let _=tx.send(Msg::DownloadFinished { id: id.clone(), path: target_path.to_string_lossy().into() });
                return;
            }
        });
        if self.active_downloads() < max_parallel {
            self.maybe_start_next_download();
        }
    }

    fn resolve_play_url(&self, row: &Row) -> String {
        if row.info == "Movie" || row.info == "SeriesEpisode" {
            if let Some(p) =
                self.local_file_exists(&row.id, &row.name, row.container_extension.as_deref())
            {
                return file_path_to_uri(&p);
            }
        }
        if row.info == "SeriesEpisode" {
            build_url_by_type(
                &self.config,
                &row.id,
                &row.info,
                row.container_extension.as_deref(),
            )
        } else {
            row.stream_url.clone().unwrap_or_else(|| {
                build_url_by_type(
                    &self.config,
                    &row.id,
                    &row.info,
                    row.container_extension.as_deref(),
                )
            })
        }
    }

    fn scan_download_directory(&mut self) {
        let now = std::time::Instant::now();
        if let Some(last) = self.last_download_scan {
            if now.duration_since(last) < Duration::from_secs(5) {
                return;
            }
        }
        self.last_download_scan = Some(now);
        let dir = self.expand_download_dir();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut out: Vec<ScannedDownload> = Vec::new();
            if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = rd.next_entry().await {
                    let path = entry.path();
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if filename == ".DS_Store" {
                        continue;
                    }
                    let ext = path.extension().and_then(|e| e.to_str());
                    if ext == Some("part") || ext == Some("json") {
                        continue;
                    }
                    if let Ok(md) = entry.metadata().await {
                        if md.is_file() {
                            let mut id = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default()
                                .to_string();
                            let mut name = id.clone();
                            let mut info = "Movie".to_string();
                            let mut container_extension = path
                                .extension()
                                .and_then(|s| s.to_str())
                                .map(|s| s.to_string());
                            let sidecar = path.with_extension(format!(
                                "{}.json",
                                path.extension()
                                    .and_then(|e| e.to_str())
                                    .unwrap_or_default()
                            ));
                            if let Ok(data) = tokio::fs::read(&sidecar).await {
                                if let Ok(js) = serde_json::from_slice::<serde_json::Value>(&data) {
                                    if let Some(v) = js.get("id").and_then(|v| v.as_str()) {
                                        id = v.to_string();
                                    }
                                    if let Some(v) = js.get("name").and_then(|v| v.as_str()) {
                                        name = v.to_string();
                                    }
                                    if let Some(v) = js.get("info").and_then(|v| v.as_str()) {
                                        info = v.to_string();
                                    }
                                    if let Some(v) = js.get("ext").and_then(|v| v.as_str()) {
                                        container_extension = Some(v.to_string());
                                    }
                                }
                            }
                            out.push(ScannedDownload {
                                id,
                                name,
                                info,
                                container_extension,
                                path: path.to_string_lossy().into(),
                                size: md.len(),
                                modified: md
                                    .modified()
                                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                            });
                        }
                    }
                }
            }
            out.sort_by_key(|d| std::cmp::Reverse(d.modified));
            let _ = tx.send(Msg::DownloadsScanned(out));
        });
    }
}

impl MacXtreamer {
    fn render_wisdom_gate_panel(&mut self, ui: &mut egui::Ui) {
        let previous_tab = self.ai_panel_tab.clone();
        
        // Load recently added items when switching to that tab
        if self.ai_panel_tab == "recently_added" && self.recently_added_items.is_empty() {
            let cfg = self.config.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                if let Ok(items) = crate::api::fetch_recently_added(&cfg).await {
                    let _ = tx.send(Msg::RecentlyAddedItems(items));
                }
            });
        }
        
        render_ai_panel(ui, &self.config, &self.wisdom_gate_recommendations, &self.recently_added_items, &mut self.ai_panel_tab, &self.tx);
        
        // Save preference if tab changed
        if self.ai_panel_tab != previous_tab {
            self.config.ai_panel_tab = self.ai_panel_tab.clone();
            self.pending_save_config = true;
        }
    }
}

impl eframe::App for MacXtreamer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- Frame timing & hibernate/wake detection ---
        let now = std::time::Instant::now();
        let wall_now = std::time::SystemTime::now();

        // Elapsed wall-clock time since last frame.  On a normal 60 Hz frame this is ~16 ms.
        // After system sleep/hibernate it can jump by minutes.  We use >8 s as the threshold.
        let wall_gap_ms = wall_now
            .duration_since(self.last_frame_wallclock)
            .unwrap_or_default()
            .as_millis() as u64;
        let woke_from_sleep = wall_gap_ms > 8_000;

        self.last_frame_wallclock = wall_now;

        // If we just woke, start a 2.5 s cooldown window.  During that window we throttle
        // repaints to ≤5 FPS so egui's internal event burst doesn't produce visible flicker.
        if woke_from_sleep {
            println!("💤 Wake from sleep detected (wall gap {}ms) — starting repaint cooldown", wall_gap_ms);
            self.post_wake_cooldown_until = Some(now + Duration::from_millis(2500));
            // Reset frame-timing state to avoid stale averages.
            self.last_forced_repaint = now;
            self.avg_frame_ms = 16.0;
        }

        // Are we still inside the post-wake cooldown?
        let in_wake_cooldown = self
            .post_wake_cooldown_until
            .map(|deadline| now < deadline)
            .unwrap_or(false);
        if !in_wake_cooldown {
            self.post_wake_cooldown_until = None;
        }

        // Keep the old `is_after_sleep` binding so that the message-processing code
        // below (which already guards on it) continues to work without changes.
        let is_after_sleep = in_wake_cooldown;

        let dt = now.duration_since(self.last_frame_time).as_millis() as f32;
        self.last_frame_time = now;
        
        // Exponentielles Glätten
        if self.avg_frame_ms == 0.0 { self.avg_frame_ms = dt; } else { self.avg_frame_ms = self.avg_frame_ms * 0.9 + dt * 0.1; }

        let time_since_forced_raw: u64 = now.duration_since(self.last_forced_repaint).as_millis() as u64;
        let time_since_forced = time_since_forced_raw;

        // Watchdog: if update check hangs, auto-clear after deadline
        if self.checking_for_updates {
            if let Some(deadline) = self.update_check_deadline {
                if now > deadline {
                    println!("⏱️ Update-check watchdog expired, clearing checking_for_updates flag");
                    self.checking_for_updates = false;
                    self.update_check_deadline = None;
                    self.add_toast("Update check timed out (watchdog)".to_string(), ToastType::Warning);
                }
            }
        }

        // Theme anwenden (einmalig oder bei Wechsel)
        if !self.theme_applied {
            match self.current_theme.as_str() {
                "light" => ctx.set_visuals(egui::Visuals::light()),
                _ => ctx.set_visuals(egui::Visuals::dark()),
            }
            self.theme_applied = true;
        }
        // Schriftgröße skalieren (bei jeder Änderung neu setzen)
        if !self.font_scale_applied {
            // Get the default style as baseline
            let default_style = egui::Style::default();
            let mut style = default_style;

            // Apply our scale to the default font sizes (not current ones)
            let scale = self.current_font_scale.max(0.6).min(2.0);
            style.text_styles.iter_mut().for_each(|(_, ts)| {
                ts.size *= scale;
            });

            ctx.set_style(style);
            self.font_scale_applied = true;
        }
        // Während Hintergrundaktivität nur dann neu zeichnen, wenn tatsächlich Änderungen stattfinden
        // Reduzierte Repaint-Frequenz um Flimmern zu vermeiden
        let has_critical_bg_work = self.is_loading 
            || self.active_downloads() > 0
            || (!self.pending_texture_uploads.is_empty() && self.pending_texture_uploads.len() > 5); // Nur bei größeren Warteschlangen
        
        let has_minor_bg_work = (!self.pending_covers.is_empty() && self.pending_covers.len() > 10)
            || (!self.pending_decode_urls.is_empty() && self.pending_decode_urls.len() > 10)
            || self.indexing;
        
        // CPU FIX: Dramatically reduce automatic repaint frequency
        // FLICKER FIX: Disable aggressive repaint_after calls - let egui's spinner/progress handle animation
        if self.config.low_cpu_mode {
            // Low CPU Mode: adaptive Thresholds basierend auf durchschnittlicher Frame-Zeit
            let base_critical = 5000u64; // Much longer - let spinner animate
            let base_minor = 10000u64;
            let critical_threshold = if self.avg_frame_ms > 30.0 { base_critical * 2 } else { base_critical };
            let minor_threshold = if self.avg_frame_ms > 30.0 { base_minor * 2 } else { base_minor };
            if time_since_forced >= critical_threshold && has_critical_bg_work {
                ctx.request_repaint_after(Duration::from_millis(5000));
                self.last_forced_repaint = now;
            } else if time_since_forced >= minor_threshold && has_minor_bg_work {
                ctx.request_repaint_after(Duration::from_millis(10000));
                self.last_forced_repaint = now;
            }
        } else if self.config.ultra_low_flicker_mode {
            // Ultra Flicker Guard: Nur heartbeats, keine Auto-Repaints außer bei kritischen Szenarien
            let repaint_interval = 2000u64; // Much longer interval
            if time_since_forced >= repaint_interval && has_critical_bg_work {
                ctx.request_repaint();
                self.last_forced_repaint = now;
            }
        } else {
            // Normal Mode: DISABLED - let egui widgets handle their own animation
            // Don't call request_repaint_after here - it causes flicker!
            // egui's spinner and progress bar will handle repaints automatically
        }
        // If no background work, NO automatic repaints at all!

        // CRITICAL CPU FIX: Limit message processing to prevent endless loops
        let mut got_msg = false;
        let mut covers_to_prefetch: Vec<String> = Vec::new();
        let mut message_count = 0;
        // After sleep: significantly reduce message processing to prevent flicker
        // During wake cooldown process 2 messages/frame: enough to drain the queue
        // without triggering the repaint burst that is_after_sleep / in_wake_cooldown guards against.
        let max_msgs_per_frame = if is_after_sleep { 2 } else { 5 };
        
        while let Ok(msg) = self.rx.try_recv() {
            message_count += 1;
            if message_count > max_msgs_per_frame {
                break; // Prevent infinite message processing
            }
            got_msg = true;
            self.pending_repaint_due_to_msg = true; // Mark that a repaint is needed for visual update
            match msg {
                Msg::LiveCategories(res) => {
                    match res {
                        Ok(list) => {
                            self.playlists = list;
                            // Reload by last ID if present
                            if let Some(ref wanted) = self.last_live_cat_id {
                                if let Some((i, cat)) = self
                                    .playlists
                                    .iter()
                                    .enumerate()
                                    .find(|(_, c)| &c.id == wanted)
                                {
                                    self.selected_playlist = Some(i);
                                    self.selected_vod = None;
                                    self.selected_series = None;
                                    // Don't reset loading state if already loading categories
                                    if !self.is_loading {
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                    }
                                    self.spawn_load_items("subplaylist", cat.id.clone());
                                }
                            } else if let Some(i) = self.selected_playlist {
                                if i < self.playlists.len() {
                                    // Don't reset loading state if already loading categories
                                    if !self.is_loading {
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                    }
                                    let id = self.playlists[i].id.clone();
                                    self.spawn_load_items("subplaylist", id);
                                } else {
                                    self.selected_playlist = None;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ LiveCategories Fehler: {}", e);
                            self.last_error = Some(e.clone());
                            let url = format!("{}/player_api.php?username={}&password=***&action=get_live_categories", self.config.address, self.config.username);
                            self.loading_error = format!("Fehler beim Laden der Live-Kategorien:\n{}\n\nURL: {}", e, url);
                            self.show_error_dialog = true;
                        }
                    }
                    self.loading_done = self.loading_done.saturating_add(1);
                    if self.loading_done >= self.loading_total {
                        self.is_loading = false;
                        println!("🏁 [LiveCategories] All categories loaded ({}/{})", self.loading_done, self.loading_total);
                        
                        // Auto-build search index if empty
                        if self.all_movies.is_empty() && self.all_series.is_empty() && self.all_channels.is_empty() {
                            println!("🔍 [Auto-build] Index is empty, triggering spawn_build_index()");
                            self.spawn_build_index();
                        } else {
                            println!("✅ [Auto-build] Index already has {} movies, {} series, {} channels",
                                self.all_movies.len(), self.all_series.len(), self.all_channels.len());
                        }
                    }
                }
                Msg::VodCategories(res) => {
                    match res {
                        Ok(list) => {
                            self.vod_categories = list;
                            if let Some(ref wanted) = self.last_vod_cat_id {
                                if let Some((i, cat)) = self
                                    .vod_categories
                                    .iter()
                                    .enumerate()
                                    .find(|(_, c)| &c.id == wanted)
                                {
                                    self.selected_vod = Some(i);
                                    self.selected_playlist = None;
                                    self.selected_series = None;
                                    // Don't reset loading state if already loading categories
                                    if !self.is_loading {
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                    }
                                    self.spawn_load_items("vod", cat.id.clone());
                                }
                            } else if let Some(i) = self.selected_vod {
                                if i < self.vod_categories.len() {
                                    // Don't reset loading state if already loading categories
                                    if !self.is_loading {
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                    }
                                    let id = self.vod_categories[i].id.clone();
                                    self.spawn_load_items("vod", id);
                                } else {
                                    self.selected_vod = None;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ VodCategories Fehler: {}", e);
                            self.last_error = Some(e.clone());
                            let url = format!("{}/player_api.php?username={}&password=***&action=get_vod_categories", self.config.address, self.config.username);
                            self.loading_error = format!("Fehler beim Laden der VOD-Kategorien:\n{}\n\nURL: {}", e, url);
                            self.show_error_dialog = true;
                        }
                    }
                    self.loading_done = self.loading_done.saturating_add(1);
                    if self.loading_done >= self.loading_total {
                        self.is_loading = false;
                        println!("🏁 [VodCategories] All categories loaded ({}/{})", self.loading_done, self.loading_total);
                        
                        // Auto-build search index if empty
                        if self.all_movies.is_empty() && self.all_series.is_empty() && self.all_channels.is_empty() {
                            println!("🔍 [Auto-build] Index is empty, triggering spawn_build_index()");
                            self.spawn_build_index();
                        } else {
                            println!("✅ [Auto-build] Index already has {} movies, {} series, {} channels",
                                self.all_movies.len(), self.all_series.len(), self.all_channels.len());
                        }
                    }
                }
                Msg::SeriesCategories(res) => {
                    match res {
                        Ok(list) => {
                            self.series_categories = list;
                            if let Some(ref wanted) = self.last_series_cat_id {
                                if let Some((i, cat)) = self
                                    .series_categories
                                    .iter()
                                    .enumerate()
                                    .find(|(_, c)| &c.id == wanted)
                                {
                                    self.selected_series = Some(i);
                                    self.selected_playlist = None;
                                    self.selected_vod = None;
                                    // Don't reset loading state if already loading categories
                                    if !self.is_loading {
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                    }
                                    self.spawn_load_items("series", cat.id.clone());
                                }
                            } else if let Some(i) = self.selected_series {
                                if i < self.series_categories.len() {
                                    // Don't reset loading state if already loading categories
                                    if !self.is_loading {
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                    }
                                    let id = self.series_categories[i].id.clone();
                                    self.spawn_load_items("series", id);
                                } else {
                                    self.selected_series = None;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ SeriesCategories Fehler: {}", e);
                            self.last_error = Some(e.clone());
                            let url = format!("{}/player_api.php?username={}&password=***&action=get_series_categories", self.config.address, self.config.username);
                            self.loading_error = format!("Fehler beim Laden der Serien-Kategorien:\n{}\n\nURL: {}", e, url);
                            self.show_error_dialog = true;
                        }
                    }
                    self.loading_done = self.loading_done.saturating_add(1);
                    if self.loading_done >= self.loading_total {
                        self.is_loading = false;
                        println!("🏁 [SeriesCategories] All categories loaded ({}/{})", self.loading_done, self.loading_total);
                        
                        // Auto-build search index if empty
                        if self.all_movies.is_empty() && self.all_series.is_empty() && self.all_channels.is_empty() {
                            println!("🔍 [Auto-build] Index is empty, triggering spawn_build_index()");
                            self.spawn_build_index();
                        } else {
                            println!("✅ [Auto-build] Index already has {} movies, {} series, {} channels",
                                self.all_movies.len(), self.all_series.len(), self.all_channels.len());
                        }
                    }
                }
                Msg::VlcDiagnostics(output) => {
                    self.last_error = Some(format!("VLC Diagnose: {}", output));
                }
                Msg::VlcDiagUpdate { lines, suggestion } => {
                    for l in lines {
                        if self.vlc_diag_lines.len() >= 128 { self.vlc_diag_lines.pop_front(); }
                        self.vlc_diag_lines.push_back(l);
                    }
                    self.vlc_diag_suggestion = suggestion;
                }
                Msg::PlayerDetection { has_vlc, has_mpv, vlc_version, mpv_version, vlc_path, mpv_path } => {
                    self.has_vlc = has_vlc; self.has_mpv = has_mpv; self.vlc_version = vlc_version; self.mpv_version = mpv_version;
                    self.detected_vlc_path = vlc_path;
                    self.detected_mpv_path = mpv_path;
                    // Policy: if user wanted mpv but not present -> disable
                    if self.config.use_mpv && !self.has_mpv { self.config.use_mpv = false; self.last_error = Some("mpv not found – falling back to VLC".into()); self.pending_save_config = true; }
                    // If mpv only available -> auto enable
                    if !self.config.use_mpv && self.has_mpv && !self.has_vlc { self.config.use_mpv = true; self.pending_save_config = true; }
                }
                Msg::PlayerSpawnFailed { player, error } => {
                    // Detaillierte Fehlerbehandlung je nach Player-Typ
                    if player.contains("mpv") { 
                        self.mpv_fail_count = self.mpv_fail_count.saturating_add(1); 
                        log_line(&format!("🔥 mpv Fehler #{}: {}", self.mpv_fail_count, error));
                    }
                    if player.to_lowercase().contains("vlc") { 
                        self.vlc_fail_count = self.vlc_fail_count.saturating_add(1); 
                        log_line(&format!("🔥 VLC Fehler #{}: {}", self.vlc_fail_count, error));
                    }
                    
                    // Spezielle Behandlung für System-Fehler (kein Player verfügbar)
                    if player == "System" {
                        self.last_error = Some(format!("❌ KRITISCHER FEHLER: {}", error));
                        log_line(&format!("💥 System-Fehler: {}", error));
                    } else {
                        self.last_error = Some(format!("🔥 {} Startfehler: {}", player, error));
                    }
                    
                    // Auto-Fallback bei wiederholten Fehlern
                    if self.config.use_mpv && self.mpv_fail_count >= 3 && self.has_vlc { 
                        self.config.use_mpv = false; 
                        self.pending_save_config = true; 
                        self.last_error = Some("⚠️ mpv wiederholt fehlgeschlagen – Automatischer Wechsel auf VLC".into()); 
                        log_line("🔄 Auto-Wechsel: mpv → VLC");
                    }
                    if !self.config.use_mpv && self.vlc_fail_count >= 3 && self.has_mpv { 
                        self.config.use_mpv = true; 
                        self.pending_save_config = true; 
                        self.last_error = Some("⚠️ VLC wiederholt fehlgeschlagen – Automatischer Wechsel auf mpv".into()); 
                        log_line("🔄 Auto-switch: VLC → mpv");
                    }
                }
                Msg::DiagnosticsStopped => {
                    self.last_error = Some("VLC Diagnostics stopped".into());
                    if let Some(flag) = &self.active_diag_stop { flag.store(true, std::sync::atomic::Ordering::Relaxed); }
                }
                Msg::StopDiagnostics => {
                    if let Some(flag) = &self.active_diag_stop { flag.store(true, std::sync::atomic::Ordering::Relaxed); }
                }
                Msg::ItemsLoaded { kind, items } => {
                    // Ignore ItemsLoaded if we're not in an Items view (e.g., switched to Search)
                    let should_process = match &self.current_view {
                        Some(ViewState::Items { .. }) => true,
                        _ => false,
                    };
                    
                    if !should_process {
                        println!("⚠️ Ignoriere ItemsLoaded - nicht in Items-View (current_view={:?})", self.current_view);
                        // Still update loading state
                        self.loading_done = self.loading_done.saturating_add(1);
                        if self.loading_done >= self.loading_total {
                            self.is_loading = false;
                        }
                        return;
                    }
                    
                    // Reset search status when loading new items (but only if not in search view)
                    if !matches!(self.current_view, Some(ViewState::Search { .. })) {
                        self.search_status = SearchStatus::Idle;
                    }
                    
                    match items {
                        Ok(items) => {
                            // Don't process items if we're in a search view - search results should not be overwritten
                            if !matches!(self.current_view, Some(ViewState::Search { .. })) {
                                // map to rows - DO NOT modify all_movies/all_series/all_channels here
                                // those are only for the search index and should only be populated by IndexData
                                self.content_rows.clear();
                                for it in items {
                                    let info = match kind.as_str() {
                                        "subplaylist" => "Channel",
                                        "vod" => "Movie",
                                        "series" => "Series",
                                        _ => "Item",
                                    };
                                    
                                    // Apply language filter if enabled
                                    let should_include = if self.config.filter_by_language && !self.config.default_search_languages.is_empty() {
                                        if let Some(langs) = &it.audio_languages {
                                            let langs_upper = langs.to_uppercase();
                                            // Check if any configured language appears in item's languages
                                            self.config.default_search_languages.iter().any(|filter_lang| {
                                                langs_upper.contains(&filter_lang.to_uppercase())
                                            })
                                        } else {
                                            // Keep items without language info (channels, series, etc.)
                                            info == "Channel" || info == "Series"
                                        }
                                    } else {
                                        // Filter disabled, include all
                                        true
                                    };
                                    
                                    if !should_include {
                                        continue;
                                    }
                                    
                                    self.content_rows.push(Row {
                                        name: it.name.clone(),
                                        id: it.id,
                                        info: info.to_string(),
                                        container_extension: if info == "Movie"
                                            && !it.container_extension.is_empty()
                                        {
                                            Some(it.container_extension)
                                        } else {
                                            None
                                        },
                                        stream_url: it.stream_url.clone(),
                                        cover_url: it.cover.clone(),
                                        year: it.year.clone(),
                                        release_date: it.release_date.clone().or_else(|| extract_year_from_title(&it.name)).or_else(|| it.year.clone()),
                                        rating_5based: it.rating_5based,
                                        genre: it.genre.clone(),
                                        audio_languages: it.audio_languages.clone(),
                                        path: Some(match info {
                                            "Movie" => format!(
                                                "VOD / {}",
                                                self.vod_categories
                                                    .get(self.selected_vod.unwrap_or(0))
                                                    .map(|c| c.name.clone())
                                                    .unwrap_or_else(|| "?".into())
                                            ),
                                            "Series" => format!(
                                                "Series / {}",
                                                self.series_categories
                                                    .get(self.selected_series.unwrap_or(0))
                                                    .map(|c| c.name.clone())
                                                    .unwrap_or_else(|| "?".into())
                                            ),
                                            "Channel" => format!(
                                                "Live / {}",
                                                self.playlists
                                                    .get(self.selected_playlist.unwrap_or(0))
                                                    .map(|c| c.name.clone())
                                                    .unwrap_or_else(|| "?".into())
                                            ),
                                            _ => "".into(),
                                        }),
                                    });
                                }
                                
                                // Auto-load EPG data for live channels
                                if kind == "subplaylist" {
                                    let stream_ids: Vec<String> = self.content_rows.iter()
                                        .filter(|r| r.info == "Channel")
                                        .take(20) // Limit to first 20 channels to avoid overwhelming the server
                                        .map(|r| r.id.clone())
                                        .collect();
                                    
                                    for stream_id in stream_ids {
                                        self.spawn_load_epg(stream_id);
                                    }
                                }
                            } else {
                                println!("⚠️ Ignoring ItemsLoaded during search view (current_view={:?})", self.current_view);
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ ItemsLoaded Fehler: {}", e);
                            self.last_error = Some(e.clone());
                            self.loading_error = format!("Fehler beim Laden von Inhalten:\n{}", e);
                            self.show_error_dialog = true;
                        }
                    }
                    // Always increment loading_done and check if we're finished
                    self.loading_done = self.loading_done.saturating_add(1);
                    if self.loading_done >= self.loading_total {
                        self.is_loading = false;
                    }
                }
                Msg::EpisodesLoaded {
                    series_id: _sid,
                    episodes,
                } => {
                    // Reset search status when loading episodes (but only if not in search view)
                    if !matches!(self.current_view, Some(ViewState::Search { .. })) {
                        self.search_status = SearchStatus::Idle;
                    }
                    
                    match episodes {
                    Ok(eps) => {
                        // Sort episodes by parsed season and episode number (e.g., S27E05)
                        fn parse_season_episode(s: &str) -> Option<(u32, u32)> {
                            // Robust parsing for patterns: S27E05, s01e01, 27x05, Season 27 Episode 5
                            let t = s.to_ascii_uppercase();
                            let bytes = t.as_bytes();
                            // Helper to read number starting at idx
                            fn read_num(bytes: &[u8], start: usize) -> (u32, usize) {
                                let mut i = start;
                                let mut val: u32 = 0;
                                let mut any = false;
                                while i < bytes.len() {
                                    let b = bytes[i];
                                    if b.is_ascii_digit() {
                                        any = true;
                                        val = val * 10 + (b - b'0') as u32;
                                        i += 1;
                                    } else { break; }
                                }
                                (if any { val } else { 0 }, i)
                            }
                            // SxxEyy
                            if let Some(si) = t.find('S') {
                                let (season, after_s) = read_num(bytes, si + 1);
                                if season > 0 {
                                    if let Some(ei_rel) = t[after_s..].find('E') {
                                        let ei = after_s + ei_rel + 1; // start of digits after E
                                        let (episode, _) = read_num(bytes, ei);
                                        if episode > 0 { return Some((season, episode)); }
                                    }
                                }
                            }
                            // 27x05
                            if let Some(xpos) = t.find('X') {
                                // read number to the left (scan backwards to first digit run)
                                let mut l = xpos;
                                while l > 0 && !bytes[l-1].is_ascii_digit() { l -= 1; }
                                let mut start = l;
                                while start > 0 && bytes[start-1].is_ascii_digit() { start -= 1; }
                                let (season, _) = read_num(bytes, start);
                                let (episode, _) = read_num(bytes, xpos + 1);
                                if season > 0 && episode > 0 { return Some((season, episode)); }
                            }
                            // Season 27 Episode 5
                            if let Some(pos) = t.find("SEASON ") {
                                let (season, after_season) = read_num(bytes, pos + "SEASON ".len());
                                if season > 0 {
                                    if let Some(ep_pos) = t[after_season..].find("EPISODE ") {
                                        let ep_idx = after_season + ep_pos + "EPISODE ".len();
                                        let (episode, _) = read_num(bytes, ep_idx);
                                        if episode > 0 { return Some((season, episode)); }
                                    }
                                }
                            }
                            None
                        }

                        let mut eps_sorted = eps.clone();
                        use std::cmp::Ordering;
                        eps_sorted.sort_by(|a, b| {
                            match (parse_season_episode(&a.name), parse_season_episode(&b.name)) {
                                (Some((sa, ea)), Some((sb, eb))) => sa.cmp(&sb).then(ea.cmp(&eb)),
                                (Some(_), None) => Ordering::Less,   // parsed episodes come before unknowns
                                (None, Some(_)) => Ordering::Greater,
                                (None, None) => a.name.cmp(&b.name),
                            }
                        });

                        self.content_rows.clear();
                        for ep in eps_sorted {
                            self.content_rows.push(Row {
                                name: ep.name,
                                id: ep.episode_id,
                                info: "SeriesEpisode".to_string(),
                                container_extension: Some(ep.container_extension),
                                stream_url: ep.stream_url.clone(),
                                cover_url: ep.cover.clone(),
                                year: None,
                                release_date: None,
                                rating_5based: None,
                                genre: None,
                                path: Some("Series / Episodes".into()),
                                audio_languages: None,
                            });
                        }
                        self.is_loading = false;
                        self.loading_done = self.loading_total;
                    }
                    Err(e) => {
                        eprintln!("❌ EpisodesLoaded Fehler: {}", e);
                        self.is_loading = false;
                        self.last_error = Some(e.clone());
                        self.loading_error = format!("Fehler beim Laden der Episoden:\n{}", e);
                        self.show_error_dialog = true;
                        self.loading_done = self.loading_total;
                    }
                    }
                }
                Msg::CoverLoaded { url, bytes } => {
                    // Offload decoding/resizing to a background worker to avoid UI hitches
                    if self.textures.contains_key(&url) || self.pending_decode_urls.contains(&url) {
                        // Nothing to do
                    } else {
                        self.pending_decode_urls.insert(url.clone());
                        let tx2 = self.tx.clone();
                        let dec_sem = self.decode_sem.clone();
                        let target_h: u32 = (self.cover_height * 2.0).clamp(32.0, 512.0) as u32;
                        tokio::spawn(async move {
                            let _permit = dec_sem.acquire_owned().await.ok();
                            let url2 = url.clone();
                            let res = tokio::task::spawn_blocking(move || {
                                // Decode and lightly downscale to reduce upload size
                                match image::load_from_memory(&bytes) {
                                    Ok(mut img) => {
                                        // Target height derived from UI settings
                                        let (w, h) = img.dimensions();
                                        if h > target_h {
                                            let new_w = ((w as f32) * (target_h as f32)
                                                / (h as f32))
                                                .round()
                                                .max(1.0)
                                                as u32;
                                            img = img.resize_exact(
                                                new_w,
                                                target_h,
                                                image::imageops::FilterType::Triangle,
                                            );
                                        }
                                        let rgba = img.to_rgba8();
                                        let (w2, h2) = rgba.dimensions();
                                        let data = rgba.into_raw();
                                        Ok((data, w2, h2))
                                    }
                                    Err(e) => Err(e.to_string()),
                                }
                            })
                            .await;
                            if let Ok(Ok((rgba, w, h))) = res {
                                let _ = tx2.send(Msg::CoverDecoded {
                                    url: url2,
                                    rgba,
                                    w,
                                    h,
                                });
                            } else {
                                // On failure, ignore; pending will be cleared later to allow retries if needed
                            }
                        });
                    }
                }
                Msg::CoverDecoded { url, rgba, w, h } => {
                    if !self.textures.contains_key(&url)
                        && !self.pending_texture_urls.contains(&url)
                    {
                        self.pending_texture_urls.insert(url.clone());
                        self.pending_texture_uploads
                            .push_back((url.clone(), rgba, w, h));
                    }
                }
                Msg::IndexProgress { message } => {
                    self.search_status = SearchStatus::Indexing { progress: message };
                }
                Msg::IndexBuilt { movies: _m, series: _s, channels: _c } => {
                    // Bei Bedarf könnten wir hier all_movies/all_series aktualisieren,
                    // aktuell dienen die Caches von fetch_*; setze Flag zurück
                    println!("🏗️ Index aufgebaut: {} Movies, {} Series, {} Channels", _m, _s, _c);
                    self.indexing = false;
                    self.search_status = SearchStatus::Idle;
                    
                    // If we have a search query, flag to perform/repeat the search
                    // This ensures search includes all content types (movies, series, channels)
                    if !self.search_text.trim().is_empty() {
                        println!("🔄 Repeating search with complete index (incl. {} channels)...", _c);
                        self.should_start_search = true;
                    }
                }
                Msg::IndexData { movies, series, channels } => {
                    // Clear existing data to avoid accumulation
                    self.all_movies.clear();
                    self.all_series.clear();
                    self.all_channels.clear();
                    self.index_paths.clear();
                    
                    // Build new index from deduplicated data
                    for (item, path) in movies {
                        self.all_movies.push(item.clone());
                        self.index_paths.insert(item.id, path);
                    }
                    for (item, path) in series {
                        self.all_series.push(item.clone());
                        self.index_paths.insert(item.id, path);
                    }
                    for (item, path) in channels {
                        self.all_channels.push(item.clone());
                        self.index_paths.insert(item.id, path);
                    }
                    
                    println!("📊 Index aktualisiert: {} Movies, {} Series, {} Channels in Speicher", 
                             self.all_movies.len(), self.all_series.len(), self.all_channels.len());
                }
                Msg::PreloadSet { total } => {
                    self.is_loading = true;
                    self.loading_total = total;
                    self.loading_done = 0;
                }
                Msg::PreloadTick => {
                    self.loading_done = (self.loading_done + 1).min(self.loading_total);
                    if self.loading_done >= self.loading_total {
                        self.is_loading = false;
                    }
                }
                Msg::PrefetchCovers(urls) => {
                    // Sammle URLs; tatsächliches Laden nach dem Drain, um Borrow-Konflikte zu vermeiden
                    // Hinweis: covers_to_prefetch wird vor dem Loop deklariert
                    covers_to_prefetch.extend(urls);
                }
                Msg::SeriesEpisodesForDownload {
                    series_id: sid,
                    episodes,
                } => {
                    match episodes {
                        Ok(list) => {
                            // Read options for this series
                            let opts = self
                                .bulk_options_by_series
                                .get(&sid)
                                .cloned()
                                .unwrap_or(self.bulk_opts_draft.clone());
                            let mut added = 0u32;
                            for ep in list.into_iter() {
                                // Season filter: try to parse season from name like "S01E02" or "1x02" or "Season 1"
                                if let Some(season_want) = opts.season {
                                    let name_lower = ep.name.to_lowercase();
                                    let mut season_hit = false;
                                    // Patterns: sNN, season NN, NNx
                                    for pat in ["s", "season "] {
                                        if let Some(idx) = name_lower.find(pat) {
                                            let tail = &name_lower[idx + pat.len()..];
                                            let num: String = tail
                                                .chars()
                                                .take_while(|c| c.is_ascii_digit())
                                                .collect();
                                            if let Ok(n) = num.parse::<u32>() {
                                                if n == season_want {
                                                    season_hit = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    if !season_hit {
                                        // Try pattern like '1x02'
                                        let mut last_digit_seq = String::new();
                                        for ch in name_lower.chars() {
                                            if ch.is_ascii_digit() {
                                                last_digit_seq.push(ch);
                                            } else if ch == 'x' && !last_digit_seq.is_empty() {
                                                if let Ok(n) = last_digit_seq.parse::<u32>() {
                                                    if n == season_want {
                                                        season_hit = true;
                                                    }
                                                }
                                                last_digit_seq.clear();
                                            } else {
                                                last_digit_seq.clear();
                                            }
                                        }
                                        if !season_hit {
                                            continue;
                                        }
                                    }
                                }
                                // Skip already downloaded if desired
                                if opts.only_not_downloaded {
                                    if let Some(p) = self.local_file_exists(
                                        &ep.episode_id,
                                        &ep.name,
                                        Some(&ep.container_extension),
                                    ) {
                                        let _ = p;
                                        continue;
                                    }
                                }
                                // Enqueue
                                self.pending_bulk_downloads.push((
                                    ep.episode_id.clone(),
                                    ep.name.clone(),
                                    "SeriesEpisode".into(),
                                    Some(ep.container_extension.clone()),
                                ));
                                added += 1;
                                if opts.max_count > 0 && added >= opts.max_count {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            self.last_error = Some(format!("Failed to fetch episodes: {}", e));
                        }
                    }
                }
                Msg::DownloadStarted { id, path } => {
                    if let Some(st) = self.downloads.get_mut(&id) {
                        st.path = Some(path);
                    }
                }
                Msg::DownloadProgress {
                    id,
                    received,
                    total,
                } => {
                    let st = self.downloads.entry(id).or_default();
                    let now = Instant::now();
                    if st.started_at.is_none() { st.started_at = Some(now); st.prev_received = st.received; }
                    // Berechnung aktuelle Geschwindigkeit
                    if let Some(last) = st.last_update_at {
                        let dt = now.duration_since(last).as_secs_f64();
                        if dt > 0.15 {
                            let delta_bytes = received.saturating_sub(st.prev_received) as f64;
                            st.current_speed_bps = if delta_bytes > 0.0 { delta_bytes / dt } else { 0.0 };
                            st.prev_received = received;
                            st.last_update_at = Some(now);
                        }
                    } else {
                        st.last_update_at = Some(now);
                    }
                    // Durchschnittliche Geschwindigkeit
                    if let Some(start) = st.started_at {
                        let elapsed = now.duration_since(start).as_secs_f64();
                        if elapsed >= 1.0 { st.avg_speed_bps = received as f64 / elapsed; }
                    }
                    st.received = received;
                    st.total = total;
                }
                Msg::DownloadFinished { id, path } => {
                    let st = self.downloads.entry(id.clone()).or_default();
                    st.finished = true;
                    st.path = Some(path.clone());
                    // Auto-play when finished
                    // if let Some(p) = st.path.clone() {
                    //     let uri = Self::file_path_to_uri(Path::new(&p));
                    //     let _ = start_player(&self.config, &uri);
                    // }
                    
                    // Flag to check for next downloads after message processing
                    self.should_check_downloads = true;
                }
                Msg::DownloadError { id, error } => {
                    let st = self.downloads.entry(id).or_default();
                    st.error = Some(error);
                    st.finished = true;
                    
                    // Flag to check for next downloads after message processing
                    self.should_check_downloads = true;
                }
                Msg::DownloadCancelled { id } => {
                    if let Some(st) = self.downloads.get_mut(&id) {
                        st.error = Some("Cancelled".into());
                        st.finished = true;
                    }
                }
                Msg::SearchStarted => {
                    self.search_status = SearchStatus::Searching;
                }
                Msg::SearchReady(mut rows) => {
                    println!("📥 SearchReady empfangen mit {} Zeilen", rows.len());
                    println!("🔍 Aktuelle View: {:?}", self.current_view);
                    println!("📊 Aktuelle content_rows vor Update: {}", self.content_rows.len());
                    
                    // Deduplicate rows by ID (keep first occurrence)
                    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
                    rows.retain(|r| seen_ids.insert(r.id.clone()));
                    
                    // Add paths from index
                    for r in &mut rows {
                        if r.path.is_none() {
                            if let Some(p) = self.index_paths.get(&r.id) { 
                                r.path = Some(p.clone()); 
                            }
                        }
                    }
                    
                    // Ensure we're in Search view when results arrive
                    if !matches!(self.current_view, Some(ViewState::Search { .. })) {
                        println!("⚠️ Nicht in Search View - setze auf Search View");
                        self.current_view = Some(ViewState::Search { 
                            query: self.search_text.clone() 
                        });
                    }
                    
                    self.content_rows = rows;
                    self.is_loading = false;
                    
                    println!("📊 Suchergebnisse gesetzt: {} eindeutige Treffer", self.content_rows.len());
                }
                Msg::SearchCompleted { results } => {
                    if results == 0 {
                        self.search_status = SearchStatus::NoResults;
                    } else {
                        self.search_status = SearchStatus::Completed { results };
                    }
                }
                Msg::SearchFailed { error } => {
                    self.search_status = SearchStatus::Error(error);
                    self.is_loading = false;
                }
                Msg::DownloadsScanned(list) => {
                    for d in &list {
                        if let Some(st) = self.downloads.get_mut(&d.id) {
                            if st.path.is_none() { st.path = Some(d.path.clone()); }
                            if !st.finished { st.finished = true; }
                            if let Some(meta) = self.download_meta.get_mut(&d.id) {
                                meta.size = Some(d.size);
                                meta.modified = Some(d.modified);
                            }
                        } else {
                            self.downloads.insert(
                                d.id.clone(),
                                DownloadState { finished: true, path: Some(d.path.clone()), ..Default::default() }
                            );
                            self.download_order.push(d.id.clone());
                            self.download_meta.insert(
                                d.id.clone(),
                                DownloadMeta { id: d.id.clone(), name: d.name.clone(), info: d.info.clone(), container_extension: d.container_extension.clone(), size: Some(d.size), modified: Some(d.modified) }
                            );
                        }
                    }
                }
                Msg::SearchResults { .. } => {
                    // Legacy Such-Ergebnisse – aktuell nicht genutzt (SearchReady wird verwendet)
                    self.is_loading = false;
                }
                Msg::WisdomGateRecommendations(content) => {
                    if !content.starts_with("API Fehler") && !content.starts_with("🌐 **Offline-Modus**") {
                        self.config.update_wisdom_gate_cache(content.clone());
                        if let Err(e) = crate::config::write_config(&self.config) {
                            println!("⚠️ Fehler beim Speichern des Caches: {}", e);
                        } else { println!("💾 Cache erfolgreich gespeichert"); }
                    }
                    self.wisdom_gate_recommendations = Some(content);
                }
                Msg::RecentlyAddedItems(items) => {
                    self.recently_added_items = items;
                }
                Msg::LoadingError(err) => {
                    self.loading_error = err;
                    self.show_error_dialog = true;
                    self.is_loading = false;
                }
                Msg::UpdateAvailable(update_info) => {
                    println!("📲 Received UpdateAvailable message: v{}", update_info.latest_version);
                    self.checking_for_updates = false;
                    self.update_check_deadline = None;
                    if self.startup_auto_install {
                        self.startup_auto_install = false;
                        self.add_toast(
                            format!("🔄 Update v{} found — installing automatically...", update_info.latest_version),
                            ToastType::Info
                        );
                        self.start_update_download(update_info);
                    } else {
                        let version = update_info.latest_version.clone();
                        self.available_update = Some(update_info);
                        self.add_toast(
                            format!("🎉 {} {} {}", 
                                t("new_version", self.config.language),
                                version,
                                t("available_short", self.config.language)
                            ),
                            ToastType::Success
                        );
                    }
                }
                Msg::NoUpdateAvailable => {
                    println!("📲 No update available");
                    self.checking_for_updates = false;
                    self.update_check_deadline = None;
                    let was_startup = self.startup_auto_install;
                    self.startup_auto_install = false;
                    if !was_startup {
                        // Only show toast for manual checks
                        self.add_toast(
                            format!("✓ {}", t("up_to_date", self.config.language)),
                            ToastType::Info
                        );
                    }
                }
                Msg::UpdateProgress(msg) => {
                    self.update_progress = msg;
                }
                Msg::UpdateError(error) => {
                    println!("📲 Received UpdateError message: {}", error);
                    self.checking_for_updates = false;
                    self.update_check_deadline = None;
                    self.update_downloading = false;
                    self.update_installing = false;
                    // Only show toast for initial update check errors, not manual ones
                    if !self.show_config {
                        self.add_toast(
                            format!("ℹ️ {}", error),
                            ToastType::Warning
                        );
                    } else {
                        self.last_error = Some(error);
                    }
                }
                Msg::UpdateInstalled => {
                    println!("📲 Update installed — restarting...");
                    self.update_downloading = false;
                    self.update_installing = false;
                    self.update_progress = "Installation complete! Restarting...".to_string();
                    std::thread::spawn(|| {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        println!("🔄 Launching new version from /Applications/macxtreamer.app");
                        let _ = std::process::Command::new("open")
                            .arg("/Applications/macxtreamer.app")
                            .spawn();
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        std::process::exit(0);
                    });
                }
                Msg::EpgLoaded { stream_id, program } => {
                    self.epg_loading.remove(&stream_id);
                    if let Some(prog_text) = program {
                        self.epg_data.insert(stream_id, prog_text);
                    } else {
                        self.epg_data.insert(stream_id, "Not available".to_string());
                    }
                    // Trigger repaint to show updated EPG data
                    self.pending_repaint_due_to_msg = true;
                }
                Msg::SkipSeriesLoading => {
                    println!("⏭️ Series loading skipped");
                    // Mark series loading as complete
                    if self.loading_total > 0 {
                        self.loading_done = self.loading_done.saturating_add(1);
                        if self.loading_done >= self.loading_total {
                            self.is_loading = false;
                        }
                    }
                }
                Msg::ProxyTestResult { success, message } => {
                    if success {
                        self.add_toast(message.clone(), ToastType::Success);
                    } else {
                        self.add_toast(message.clone(), ToastType::Error);
                        self.last_error = Some(message.clone());
                    }
                }
            } // end match msg
        } // end while let Ok(msg)
    
    // After message processing, request repaint if messages arrived
    // FLICKER FIX: Nach Sleep - nur vorsichtig repainten um Flackern zu vermeiden
    if (self.pending_repaint_due_to_msg || got_msg) && !is_after_sleep {
        ctx.request_repaint();
        self.last_forced_repaint = now;
        self.pending_repaint_due_to_msg = false;
    } else if is_after_sleep && (self.pending_repaint_due_to_msg || got_msg) {
        // Nach Sleep: setze Flag aber keine sofortige Repaint - warte bis nächster Frame
        // Dies verhindert aggressives Flackern beim Aufwachen
        self.pending_repaint_due_to_msg = false;
    }
    // Otherwise, don't force repaints - let egui widgets (spinners, progress bars) handle their own animation

        // Check for next downloads if flagged
        if self.should_check_downloads {
            self.should_check_downloads = false;
            self.maybe_start_next_download();
        }

        // Start search if index was just built
        if self.should_start_search {
            self.should_start_search = false;
            self.start_search();
        }

        // CRITICAL CPU FIX: Massively reduce repaint frequency to prevent CPU overload
        // 50ms was causing 400% CPU usage!
        // FLICKER FIX: Nach Sleep - vermeide aggressive Repaints
        if got_msg && !is_after_sleep {
            // Only repaint for critical loading states, and much less frequently
            if self.is_loading {
                ctx.request_repaint_after(Duration::from_millis(1000)); // 1 second instead of 50ms!
            }
            // No automatic repaints for content updates - let user interaction drive them
        }

        // Determine what work is actually happening
        let has_active_downloads = self.active_downloads() > 0;
        let has_pending_work = has_critical_bg_work || has_minor_bg_work;
        
        // Download Heartbeat: ensure download progress UI updates regularly
        // This is critical - users need to see downloads progressing even without mouse movement
        // FLICKER FIX: Nach Sleep - verlängere die Download-Heartbeat-Intervalle
        let heartbeat_interval = if is_after_sleep { 1500 } else { 500 };
        if has_active_downloads && !is_after_sleep {
            ctx.request_repaint_after(Duration::from_millis(heartbeat_interval)); // Show download progress every 500ms
        }

        // Idle Governor: only throttle to 1 FPS if there's absolutely nothing going on
        // AND no downloads are pending (which would block indefinitely)
        let is_completely_idle = !has_pending_work
            && !self.is_loading
            && !has_active_downloads
            && self.downloads.iter().all(|(_, s)| s.finished || s.error.is_some() || s.waiting);
        
        if is_completely_idle && !is_after_sleep {
            // Enforce a low-frequency heartbeat to keep UI responsive without burning CPU
            ctx.request_repaint_after(Duration::from_millis(1000));
        }

        // CRITICAL FIX: Don't prefetch covers aggressively - this keeps the app busy even when idle!
        // Instead, covers will be loaded on-demand when items are rendered (lazy loading)
        // This dramatically improves idle CPU usage
        if !covers_to_prefetch.is_empty() {
            // Log that we received prefetch request but ignore it to keep idle CPU low
            println!("⚠️ Ignoring {} prefetch cover requests to keep idle CPU low (covers loaded on-demand instead)", covers_to_prefetch.len());
        }

        // Verarbeite pro Frame nur ein kleines Budget an Texture-Uploads,
        // um Frame-Drops beim Scrollen zu vermeiden.
        {
            // During wake cooldown: upload at most 1 texture per frame to avoid a burst of state
            // changes that trigger immediate repaints from the texture-atlas resize.
            let max_uploads_per_frame: usize = if in_wake_cooldown {
                1
            } else {
                self.config.cover_uploads_per_frame.max(1).min(16) as usize
            };
            let mut done = 0usize;
            while done < max_uploads_per_frame {
                let Some((url, rgba_bytes, w, h)) = self.pending_texture_uploads.pop_front() else {
                    break;
                };
                if !self.textures.contains_key(&url) {
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [w as usize, h as usize],
                        &rgba_bytes,
                    );
                    let tex =
                        ctx.load_texture(url.clone(), color_image, egui::TextureOptions::LINEAR);
                    self.textures.insert(url.clone(), tex);
                }
                // Upload (oder Versuch) abgeschlossen -> Flags bereinigen
                self.pending_texture_urls.remove(&url);
                self.pending_covers.remove(&url);
                self.pending_decode_urls.remove(&url);
                done += 1;
            }
            if !self.pending_texture_uploads.is_empty() {
                // CPU FIX: Much less frequent texture upload repaints
                // Only repaint if many textures are pending
                if self.pending_texture_uploads.len() > 10 {
                    ctx.request_repaint_after(Duration::from_millis(2000)); // 2s instead of 150ms
                }
            }
            // Grobe LRU-Begrenzung für Texturen
            let limit = self.config.texture_cache_limit.max(64) as usize;
            if self.textures.len() > limit {
                let remove_count = self.textures.len() - limit;
                let keys: Vec<String> = self.textures.keys().take(remove_count).cloned().collect();
                for k in keys {
                    self.textures.remove(&k);
                }
            }
        }

        // --- Post-wake repaint throttle ------------------------------------------
        // During the cooldown window we must still schedule repaints (so the UI
        // isn't frozen) but at a capped rate of ~5 FPS.  All the guards above
        // already skip immediate request_repaint() calls when in_wake_cooldown,
        // so this single call provides the gentle heartbeat that keeps the UI
        // updating without triggering the burst that causes visible flicker.
        if in_wake_cooldown {
            ctx.request_repaint_after(Duration::from_millis(200));
        }

        let win_h = ctx.input(|i| i.screen_rect().height());
        let top_h = win_h / 3.0;
        egui::TopBottomPanel::top("top")
            .resizable(false)
            .show_separator_line(true)
            .exact_height(top_h)
            .show(ctx, |ui| {
                // Kopfzeile mit Aktionen und Suche
                ui.horizontal(|ui| {
                    ui.heading("MacXtreamer");
                    if self.config.low_cpu_mode {
                        ui.label(egui::RichText::new("Low-CPU Mode").small().color(egui::Color32::from_rgb(150,255,150))).on_hover_text(format!("Ø Frame {:.1}ms", self.avg_frame_ms));
                    }
                    if !self.view_stack.is_empty() {
                        if ui
                            .button("Back")
                            .on_hover_text("Go to previous view")
                            .clicked()
                        {
                            if let Some(prev) = self.view_stack.pop() {
                                match &prev {
                                    ViewState::Items { kind, category_id } => {
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                        self.spawn_load_items(kind, category_id.clone());
                                    }
                                    ViewState::Episodes { series_id } => {
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                        self.spawn_load_episodes(series_id.clone());
                                    }
                                    ViewState::Search { query } => {
                                        self.search_text = query.clone();
                                        self.start_search();
                                    }
                                }
                                self.current_view = Some(prev);
                            }
                        }
                    }
                    if ui.button("Reload").clicked() {
                        // Clear disk + memory caches and force a full fresh reload
                        self.clear_caches_and_reload();
                    }
                    if self.initial_config_pending && !self.config_is_complete() {
                        ui.label(colored_text_by_type("Please complete settings to start", "warning"));
                    }
                    if ui.button("Open Log").clicked() {
                        // Open log file directly in editor
                        let path = crate::logger::log_path();
                        #[cfg(target_os = "macos")]
                        {
                            let _ = std::process::Command::new("open")
                                .arg(&path)
                                .spawn();
                        }
                        #[cfg(target_os = "linux")]
                        {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(&path)
                                .spawn();
                        }
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("notepad")
                                .arg(&path)
                                .spawn();
                        }
                    }
                    // Reuse VLC toggle
                    let mut reuse = self.config.reuse_vlc;
                    if ui
                        .checkbox(&mut reuse, "Reuse VLC")
                        .on_hover_text("Open URLs in the already running VLC instance")
                        .changed()
                    {
                        self.config.reuse_vlc = reuse;
                    }
                    let mut use_mpv = self.config.use_mpv;
                    ui.add_enabled_ui(self.has_mpv, |ui| {
                        if ui.checkbox(&mut use_mpv, "Use MPV").on_hover_text(if self.has_mpv { "Use MPV player instead of VLC" } else { "mpv not found (brew install mpv)" }).changed() {
                            self.config.use_mpv = use_mpv;
                            if use_mpv { self.config.reuse_vlc = false; }
                            self.pending_save_config = true;
                        }
                    });
                    // Effektive Parameter-Vorschau (gekürzt) für beide Player
                    let vlc_preview = crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Default, &self.config)
                        .replace("{URL}", "<URL>");
                    // mpv Preview (nur falls installiert) – parallele Logik zu player::start_player
                    let mpv_preview = if self.has_mpv {
                        let (net_ms, live_ms, file_ms) = crate::player::apply_bias(&self.config);
                        let to_secs = |ms: u32| (ms / 1000).max(1);
                        let cache_secs = if self.config.mpv_cache_secs_override != 0 { self.config.mpv_cache_secs_override } else { to_secs(net_ms) };
                        let read_secs = if self.config.mpv_readahead_secs_override != 0 { self.config.mpv_readahead_secs_override } else { to_secs(file_ms.max(live_ms)) };
                        let mut parts = vec!["mpv".to_string(), "--force-window=no".into(), "--fullscreen".into(), format!("--cache-secs={}", cache_secs), format!("--demuxer-readahead-secs={}", read_secs)];
                        if !self.config.mpv_extra_args.trim().is_empty() { parts.extend(self.config.mpv_extra_args.split_whitespace().map(|s| s.to_string())); }
                        parts.push("<URL>".into());
                        parts.join(" ")
                    } else { "mpv: not found".into() };
                    let shorten = |s: &str| { if s.len() > 120 { format!("{}…", &s[..120]) } else { s.to_string() } };
                    ui.vertical(|ui| {
                        let (n,l,f) = crate::player::apply_bias(&self.config);
                        ui.label(egui::RichText::new(format!("VLC: {}", shorten(&vlc_preview))).small()).on_hover_text(format!("Bias -> network={}ms live={}ms file={}ms", n,l,f));
                        let mpv_hover = if self.has_mpv {
                            format!("MPV Cache Mapping: cache-secs={} readahead basiert auf Bias/Overrides\nPfad: {}\nVersion: {}",
                                if self.config.mpv_cache_secs_override!=0 { self.config.mpv_cache_secs_override.to_string() } else { ((n/1000).max(1)).to_string() },
                                self.detected_mpv_path.clone().unwrap_or("?".into()),
                                self.mpv_version.clone().unwrap_or("?".into()))
                        } else {
                            "mpv nicht verfügbar – optional manuellen Pfad in Settings setzen".into()
                        };
                        ui.label(egui::RichText::new(format!("MPV: {}", shorten(&mpv_preview))).small()).on_hover_text(mpv_hover);
                        if self.config.low_cpu_mode {
                            ui.label(egui::RichText::new(format!("Pending tex:{} covers:{} decodes:{} dl:{}", self.pending_texture_uploads.len(), self.pending_covers.len(), self.pending_decode_urls.len(), self.active_downloads())).small()).on_hover_text("Debug Statistiken im Low-CPU Mode");
                        }
                        if !self.has_mpv {
                            ui.colored_label(egui::Color32::YELLOW, "mpv not found – open Settings and set the path if installed");
                        }
                    });
                    if ui.button("Settings").clicked() {
                        self.config_draft = Some(self.config.clone());
                        self.show_config = true;
                    }
                    
                    // Update check button
                    if self.checking_for_updates {
                        ui.add_enabled(false, egui::Button::new(&t("checking_updates", self.config.language)));
                    } else {
                        if ui.button(&t("check_updates", self.config.language)).clicked() {
                            self.check_for_updates();
                        }
                    }
                    // Proxy status / quick access
                    {
                        let proxy_active = self.config.proxy_enabled && !self.config.proxy_host.is_empty();
                        let status_text = if proxy_active { t("proxy_status_connected", self.config.language) } else { t("proxy_status_disconnected", self.config.language) };
                        if ui.add(egui::Button::new(status_text)).on_hover_text(&t("proxy_help", self.config.language)).clicked() {
                            // Open full settings when clicking the proxy status
                            self.config_draft = Some(self.config.clone());
                            self.show_config = true;
                        }
                        // (Top-bar test button removed; use Test in Settings dialog)
                    }
                    // Short hint about player URL placeholder
                    ui.add_space(6.0);
                    // Theme Toggle
                    egui::ComboBox::from_label("")
                        .selected_text(if self.current_theme == "light" {
                            "Light"
                        } else {
                            "Dark"
                        })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(self.current_theme == "dark", "Dark")
                                .clicked()
                            {
                                self.current_theme = "dark".into();
                                self.theme_applied = false;
                            }
                            if ui
                                .selectable_label(self.current_theme == "light", "Light")
                                .clicked()
                            {
                                self.current_theme = "light".into();
                                self.theme_applied = false;
                            }
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Text size");

                        // Use tracking variable for consistent state
                        let current_percent = (self.current_font_scale * 100.0).round() as i32;
                        let min_percent = 60;
                        let max_percent = 200;

                        // Decrease button
                        if ui
                            .add_enabled(current_percent > min_percent, egui::Button::new("−"))
                            .clicked()
                        {
                            let new_percent = (current_percent - 5).max(min_percent);
                            self.current_font_scale = new_percent as f32 / 100.0;
                            self.config.font_scale = self.current_font_scale;
                            self.font_scale_applied = false;
                            self.pending_save_config = true;
                        }

                        // Display percentage
                        ui.label(format!("{}%", current_percent));

                        // Increase button
                        if ui
                            .add_enabled(current_percent < max_percent, egui::Button::new("+"))
                            .clicked()
                        {
                            let new_percent = (current_percent + 5).min(max_percent);
                            self.current_font_scale = new_percent as f32 / 100.0;
                            self.config.font_scale = self.current_font_scale;
                            self.font_scale_applied = false;
                            self.pending_save_config = true;
                        }
                        
                        ui.separator();
                        
                        // Note: Per-category "Filter by Language" toggles are available in the Live / VOD / Series headers.
                        ui.label("Per-list 'Filter by Language' toggles control filtering behavior.");
                    });
                    if self.is_loading {
                        let pct = if self.loading_total > 0 {
                            (self.loading_done * 100 / self.loading_total).min(100)
                        } else {
                            0
                        };
                        ui.horizontal(|ui| {
                            ui.spinner(); // Add spinning indicator
                            ui.label(format!(
                                "Loading… {}% ({}/{})",
                                pct, self.loading_done, self.loading_total
                            ));
                        });
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let search_enabled = !self.indexing && matches!(self.search_status, SearchStatus::Idle | SearchStatus::Completed { .. } | SearchStatus::NoResults | SearchStatus::Error(_));
                        
                        ui.add_enabled_ui(search_enabled, |ui| {
                            if ui.button("Search").clicked() {
                                // Don't push Search view onto stack if already in Search view
                                if let Some(cv) = &self.current_view {
                                    if !matches!(cv, ViewState::Search { .. }) {
                                        self.view_stack.push(cv.clone());
                                    }
                                }
                                self.current_view = Some(ViewState::Search {
                                    query: self.search_text.clone(),
                                });
                                self.start_search();
                            }
                        });
                        
                        if !search_enabled {
                            if self.indexing {
                                ui.label(RichText::new("🏗️ Index wird erstellt...").size(12.0).color(Color32::from_rgb(255, 165, 0)));
                            }
                        }
                        
                        // Enhanced search box with history
                        ui.horizontal(|ui| {
                            let text_edit_resp = egui::TextEdit::singleline(&mut self.search_text)
                                .hint_text(if self.indexing { "Index wird erstellt..." } else { "🔍 Suchen..." })
                                .desired_width(180.0)
                                .lock_focus(true)
                                .show(ui);
                            
                            let enter_pressed = search_enabled && text_edit_resp.response.ctx.input(|i| i.key_pressed(egui::Key::Enter));
                            
                            // Clear button
                            if !self.search_text.is_empty() {
                                if ui.button("✖").on_hover_text("Suche löschen").clicked() {
                                    self.search_text.clear();
                                }
                            }
                            
                            // Language filter button
                            let lang_button = ui.button("🌐").on_hover_text("Sprach-Filter");
                            if lang_button.clicked() {
                                self.show_language_filter = !self.show_language_filter;
                            }
                            
                            // Show language filter popup
                            if self.show_language_filter {
                                // Define common_langs outside the window to avoid borrowing issues
                                let common_langs = vec!["EN", "DE", "FR", "ES", "IT", "MULTI", "4K", "HD"];
                                let all_langs: Vec<String> = common_langs.iter().map(|s| s.to_string()).collect();
                                
                                egui::Window::new("Sprach-Filter")
                                    .anchor(egui::Align2::RIGHT_TOP, [-10.0, 40.0])
                                    .collapsible(false)
                                    .resizable(false)
                                    .show(ui.ctx(), |ui| {
                                        ui.set_min_width(200.0);
                                        ui.label(RichText::new("Sprachen filtern:").strong());
                                        ui.separator();
                                        
                                        // Common languages
                                        for lang in &common_langs {
                                            let mut selected = self.search_language_filter.contains(&lang.to_string());
                                            if ui.checkbox(&mut selected, *lang).clicked() {
                                                if selected {
                                                    if !self.search_language_filter.contains(&lang.to_string()) {
                                                        self.search_language_filter.push(lang.to_string());
                                                    }
                                                } else {
                                                    self.search_language_filter.retain(|l| l != lang);
                                                }
                                            }
                                        }
                                        
                                        ui.separator();
                                        ui.horizontal(|ui| {
                                            if ui.button("Alle").clicked() {
                                                self.search_language_filter = all_langs.clone();
                                            }
                                            if ui.button("Keine").clicked() {
                                                self.search_language_filter.clear();
                                            }
                                            if ui.button("Standard").clicked() {
                                                self.search_language_filter = self.config.default_search_languages.clone();
                                            }
                                        });
                                    });
                            }
                            
                            // History dropdown button
                            if !self.search_history.is_empty() {
                                let history_button = ui.button("🕐").on_hover_text("Search history");
                                if history_button.clicked() {
                                    self.show_search_history = !self.show_search_history;
                                }
                                
                                // Show history popup
                                if self.show_search_history {
                                    egui::Window::new("Search History")
                                        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 40.0])
                                        .collapsible(false)
                                        .resizable(false)
                                        .show(ui.ctx(), |ui| {
                                            ui.set_min_width(250.0);
                                            ui.label(RichText::new("Letzte Suchen:").strong());
                                            ui.separator();
                                            
                                            let mut clicked_query: Option<String> = None;
                                            for (i, query) in self.search_history.iter().enumerate() {
                                                ui.horizontal(|ui| {
                                                    if ui.button("🔍").clicked() {
                                                        clicked_query = Some(query.clone());
                                                    }
                                                    if ui.selectable_label(false, query).clicked() {
                                                        clicked_query = Some(query.clone());
                                                    }
                                                });
                                                if i < self.search_history.len() - 1 {
                                                    ui.add_space(2.0);
                                                }
                                            }
                                            
                                            ui.separator();
                                            if ui.button("🗑 Clear history").clicked() {
                                                self.search_history.clear();
                                                save_search_history(&self.search_history);
                                                self.show_search_history = false;
                                            }
                                            
                                            if let Some(query) = clicked_query {
                                                self.search_text = query.clone();
                                                self.show_search_history = false;
                                                // Don't push Search view onto stack if already in Search view
                                                if let Some(cv) = &self.current_view {
                                                    if !matches!(cv, ViewState::Search { .. }) {
                                                        self.view_stack.push(cv.clone());
                                                    }
                                                }
                                                self.current_view = Some(ViewState::Search { query });
                                                self.start_search();
                                            }
                                        });
                                }
                            }
                            
                            if enter_pressed && !self.search_text.trim().is_empty() {
                                let query = self.search_text.clone();
                                
                                // Add to history (avoid duplicates, keep max 10)
                                self.search_history.retain(|q| q != &query);
                                self.search_history.insert(0, query.clone());
                                if self.search_history.len() > 10 {
                                    self.search_history.truncate(10);
                                }
                                save_search_history(&self.search_history);
                                self.show_search_history = false;
                                
                                // Don't push Search view onto stack if already in Search view
                                if let Some(cv) = &self.current_view {
                                    if !matches!(cv, ViewState::Search { .. }) {
                                        self.view_stack.push(cv.clone());
                                    }
                                }
                                self.current_view = Some(ViewState::Search { query });
                                self.start_search();
                                text_edit_resp.response.request_focus();
                            }
                        });
                    });
                    
                    // Such-Status-Anzeige
                    match &self.search_status {
                        SearchStatus::Searching => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(colored_text_by_type("Searching...", "info"));
                            });
                        }
                        SearchStatus::NoResults => {
                            ui.label(colored_text_by_type("⚠ Keine Ergebnisse gefunden", "warning"));
                        }
                        SearchStatus::Error(err) => {
                            ui.label(colored_text_by_type(&format!("❌ Suchfehler: {}", err), "error"));
                        }
                        SearchStatus::Completed { results } => {
                            ui.label(colored_text_by_type(&format!("✅ {} Ergebnisse gefunden", results), "success"));
                        }
                        SearchStatus::Idle => {
                            // Keine Anzeige bei Idle
                        }
                        SearchStatus::Indexing { progress } => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(colored_text_by_type(&format!("🏗️ {}", progress), "info"));
                            });
                        }
                    }
                    
                    if self.indexing {
                        render_loading_spinner(ui, "Indexing");
                    }
                    
                    // Kritische Fehlermeldungen prominent anzeigen
                    let should_clear_error = if let Some(ref error) = self.last_error {
                        if error.contains("KRITISCHER FEHLER") || error.contains("kein Player") {
                            ui.separator();
                            let mut clear = false;
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("🚨").size(16.0));
                                ui.label(RichText::new(error).color(Color32::from_rgb(255, 100, 100)).strong());
                                if ui.small_button("❌").on_hover_text("Fehlermeldung schließen").clicked() {
                                    clear = true;
                                }
                            });
                            clear
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if should_clear_error {
                        self.last_error = None;
                    }
                });

                ui.separator();

                // Drei Listen im oberen Bereich (Live, VOD, Serien)
                ui.columns(3, |cols| {
                    // Strikte Filterung: Nur Kategorien mit Sprach-Prefix ODER |XX| Tag aus den Einstellungen
                    let filter_langs: Vec<String> = self.config.default_search_languages.iter().map(|l| l.to_uppercase()).collect();
                    fn has_lang_tag(name: &str, langs: &[String]) -> bool {
                        let name_up = name.to_uppercase();
                        for lang in langs {
                            // common separators/prefixes encountered in feeds: "XX ", "XX|", "XX-", "XX:", "XX/", and pipe-tags like "|XX|" or "|XX ".
                            let s_space = format!("{} ", lang);
                            let s_pipe = format!("{}|", lang);
                            let s_dash = format!("{}-", lang);
                            let s_colon = format!("{}:", lang);
                            let s_slash = format!("{}/", lang);
                            let tag_pipe = format!("|{}|", lang);
                            let tag_pipe_space = format!("|{} ", lang);

                            if name_up.starts_with(&s_space)
                                || name_up.starts_with(&s_pipe)
                                || name_up.starts_with(&s_dash)
                                || name_up.starts_with(&s_colon)
                                || name_up.starts_with(&s_slash)
                                || name_up.contains(&tag_pipe)
                                || name_up.contains(&tag_pipe_space)
                            {
                                return true;
                            }
                        }
                        false
                    }
                    // Mapping für LiveTV: EN -> UK, US, CA; DE -> DE, AT, CH
                    fn live_lang_match(name: &str, langs: &[String]) -> bool {
                        let name_up = name.to_uppercase();
                        for lang in langs {
                            match lang.as_str() {
                                "EN" => {
                                    let countries = ["UK", "US", "CA"];
                                    for c in countries.iter() {
                                        // reuse same separator logic as has_lang_tag
                                        let s_space = format!("{} ", c);
                                        let s_pipe = format!("{}|", c);
                                        let s_dash = format!("{}-", c);
                                        let s_colon = format!("{}:", c);
                                        let s_slash = format!("{}/", c);
                                        let tag_pipe = format!("|{}|", c);
                                        let tag_pipe_space = format!("|{} ", c);
                                        if name_up.starts_with(&s_space)
                                            || name_up.starts_with(&s_pipe)
                                            || name_up.starts_with(&s_dash)
                                            || name_up.starts_with(&s_colon)
                                            || name_up.starts_with(&s_slash)
                                            || name_up.contains(&tag_pipe)
                                            || name_up.contains(&tag_pipe_space)
                                        {
                                            return true;
                                        }
                                    }
                                }
                                "DE" => {
                                    let countries = ["DE", "AT", "CH"];
                                    for c in countries.iter() {
                                        let s_space = format!("{} ", c);
                                        let s_pipe = format!("{}|", c);
                                        let s_dash = format!("{}-", c);
                                        let s_colon = format!("{}:", c);
                                        let s_slash = format!("{}/", c);
                                        let tag_pipe = format!("|{}|", c);
                                        let tag_pipe_space = format!("|{} ", c);
                                        if name_up.starts_with(&s_space)
                                            || name_up.starts_with(&s_pipe)
                                            || name_up.starts_with(&s_dash)
                                            || name_up.starts_with(&s_colon)
                                            || name_up.starts_with(&s_slash)
                                            || name_up.contains(&tag_pipe)
                                            || name_up.contains(&tag_pipe_space)
                                        {
                                            return true;
                                        }
                                    }
                                }
                                _ => {
                                    let s_space = format!("{} ", lang);
                                    let s_pipe = format!("{}|", lang);
                                    let s_dash = format!("{}-", lang);
                                    let s_colon = format!("{}:", lang);
                                    let s_slash = format!("{}/", lang);
                                    let tag_pipe = format!("|{}|", lang);
                                    let tag_pipe_space = format!("|{} ", lang);
                                    if name_up.starts_with(&s_space)
                                        || name_up.starts_with(&s_pipe)
                                        || name_up.starts_with(&s_dash)
                                        || name_up.starts_with(&s_colon)
                                        || name_up.starts_with(&s_slash)
                                        || name_up.contains(&tag_pipe)
                                        || name_up.contains(&tag_pipe_space)
                                    {
                                        return true;
                                    }
                                }
                            }
                        }
                        false
                    }
                    let filtered_playlists: Vec<(usize, Category)> = self.playlists.iter().enumerate()
                        .filter(|(_, c)| {
                            if self.filter_live_language && !filter_langs.is_empty() {
                                live_lang_match(&c.name, &filter_langs)
                            } else {
                                true
                            }
                        })
                        .map(|(i, c)| (i, c.clone()))
                        .collect();
                    
                    cols[0].horizontal(|ui| {
                        ui.label(RichText::new("Live").strong());
                        if ui.checkbox(&mut self.filter_live_language, "Filter by Language").changed() {
                            // Persist change
                            self.config.filter_live_language = self.filter_live_language;
                            let _ = crate::config::save_config(&self.config);
                        }
                    });
                    egui::ScrollArea::vertical()
                        .id_source("live_list")
                        .show(&mut cols[0], |ui| {
                            for (i, c) in filtered_playlists.iter() {
                                let response = ui.selectable_label(self.selected_playlist == Some(*i), &c.name);
                                
                                // Left click - load category
                                if response.clicked() {
                                    let cat_id = c.id.clone();
                                    if let Some(cv) = &self.current_view {
                                        self.view_stack.push(cv.clone());
                                    }
                                    self.current_view = Some(ViewState::Items {
                                        kind: "subplaylist".into(),
                                        category_id: cat_id.clone(),
                                    });
                                    self.selected_playlist = Some(*i);
                                    self.selected_vod = None;
                                    self.selected_series = None;
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
                                    self.last_live_cat_id = Some(cat_id.clone());
                                    self.spawn_load_items("subplaylist", cat_id);
                                }
                                
                                // Right click - show context menu
                                response.context_menu(|ui| {
                                    let cat_id = c.id.clone();
                                    let cat_name = c.name.clone();
                                    
                                    if ui.button("▶ Play All").clicked() {
                                        // First load the items if not loaded
                                        if self.selected_playlist != Some(*i) {
                                            self.selected_playlist = Some(*i);
                                            self.spawn_load_items("subplaylist", cat_id.clone());
                                            // Give it a moment to load
                                            std::thread::sleep(std::time::Duration::from_millis(500));
                                        }
                                        self.play_all_channels(&cat_id, &cat_name);
                                        ui.close_menu();
                                    }
                                    
                                    if ui.button("📋 Copy M3U to Clipboard").clicked() {
                                        let content = self.generate_m3u_content(&cat_id);
                                        self.copy_to_clipboard(content);
                                        ui.close_menu();
                                    }
                                    
                                    if ui.button("💾 Save as M3U Playlist").clicked() {
                                        let content = self.generate_m3u_content(&cat_id);
                                        self.save_m3u_file(content, &cat_name);
                                        ui.close_menu();
                                    }
                                });
                            }
                        });

                    // Filter VOD categories
                    let filtered_vod: Vec<(usize, Category)> = self.vod_categories.iter().enumerate()
                        .filter(|(_, c)| {
                            if self.filter_vod_language && !filter_langs.is_empty() {
                                has_lang_tag(&c.name, &filter_langs)
                            } else {
                                true
                            }
                        })
                        .map(|(i, c)| (i, c.clone()))
                        .collect();

                    cols[1].horizontal(|ui| {
                        ui.label(RichText::new("VOD").strong());
                        if ui.checkbox(&mut self.filter_vod_language, "Filter by Language").changed() {
                            self.config.filter_vod_language = self.filter_vod_language;
                            let _ = crate::config::save_config(&self.config);
                        }
                    });
                    egui::ScrollArea::vertical()
                        .id_source("vod_list")
                        .show(&mut cols[1], |ui| {
                            if self.vod_categories.is_empty() {
                                if self.is_loading {
                                    ui.weak("⏳ Loading...");
                                } else if !self.config_is_complete() {
                                    ui.weak("⚠️ Settings required");
                                } else {
                                    ui.colored_label(egui::Color32::from_rgb(255, 150, 0), "⚠️ No VOD categories found");
                                    if let Some(ref err) = self.last_error {
                                        ui.small(err);
                                        if ui.small_button("🔄 Retry").clicked() {
                                            self.clear_caches_and_reload();
                                        }
                                    } else {
                                        ui.weak("Server provides no VOD categories.");
                                    }
                                }
                            }
                            for (i, c) in filtered_vod.iter() {
                                let cat_id = c.id.clone();
                                let _cat_name = c.name.clone();
                                if ui
                                    .selectable_label(self.selected_vod == Some(*i), &c.name)
                                    .clicked()
                                {
                                    if let Some(cv) = &self.current_view {
                                        self.view_stack.push(cv.clone());
                                    }
                                    self.current_view = Some(ViewState::Items {
                                        kind: "vod".into(),
                                        category_id: cat_id.clone(),
                                    });
                                    self.selected_vod = Some(*i);
                                    self.selected_playlist = None;
                                    self.selected_series = None;
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
                                    self.last_vod_cat_id = Some(cat_id.clone());
                                    self.spawn_load_items("vod", cat_id);
                                }
                            }
                        });

                    // Filter Series categories
                    let filtered_series: Vec<(usize, Category)> = self.series_categories.iter().enumerate()
                        .filter(|(_, c)| {
                            if self.filter_series_language && !filter_langs.is_empty() {
                                has_lang_tag(&c.name, &filter_langs)
                            } else {
                                true
                            }
                        })
                        .map(|(i, c)| (i, c.clone()))
                        .collect();

                    cols[2].horizontal(|ui| {
                        ui.label(RichText::new("Series").strong());
                        if ui.checkbox(&mut self.filter_series_language, "Filter by Language").changed() {
                            self.config.filter_series_language = self.filter_series_language;
                            let _ = crate::config::save_config(&self.config);
                        }
                    });
                    egui::ScrollArea::vertical().id_source("series_list").show(
                        &mut cols[2],
                        |ui| {
                            if self.series_categories.is_empty() {
                                if self.is_loading {
                                    ui.weak("⏳ Loading...");
                                } else if !self.config_is_complete() {
                                    ui.weak("⚠️ Settings required");
                                } else {
                                    ui.colored_label(egui::Color32::from_rgb(255, 150, 0), "⚠️ No series categories found");
                                    if let Some(ref err) = self.last_error {
                                        ui.small(err);
                                        if ui.small_button("🔄 Retry").clicked() {
                                            self.clear_caches_and_reload();
                                        }
                                    } else {
                                        ui.weak("Server provides no series categories.");
                                    }
                                }
                            }
                            for (i, c) in filtered_series.iter() {
                                let cat_id = c.id.clone();
                                if ui
                                    .selectable_label(self.selected_series == Some(*i), &c.name)
                                    .clicked()
                                {
                                    if let Some(cv) = &self.current_view {
                                        self.view_stack.push(cv.clone());
                                    }
                                    self.current_view = Some(ViewState::Items {
                                        kind: "series".into(),
                                        category_id: cat_id.clone(),
                                    });
                                    self.selected_series = Some(*i);
                                    self.selected_playlist = None;
                                    self.selected_vod = None;
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
                                    self.last_series_cat_id = Some(cat_id.clone());
                                    self.spawn_load_items("series", cat_id);
                                }
                            }
                        },
                    );
                });
                // Visible grab bar at the bottom edge of the Top panel
                let grip_h = 6.0;
                let full = ui.max_rect();
                let grip_rect = egui::Rect::from_min_max(
                    egui::pos2(full.min.x, full.max.y - grip_h),
                    egui::pos2(full.max.x, full.max.y),
                );
                let grip_color = ui.visuals().selection.bg_fill;
                ui.painter().rect_filled(grip_rect, 0.0, grip_color);
            });

        // Bottom panel logic: Wenn Höhe==0 keinerlei Panel einfügen; stattdessen ein kleines Floating-Expand-Icon am unteren Rand.
        let win_h = ctx.available_rect().height();
        let max_bottom = (win_h * 0.45).max(120.0);
        if self.config.bottom_panel_height <= 0.0 {
            // Floating expand button – beeinflusst Layout nicht
            egui::Area::new("bottom_expand_btn")
                .anchor(egui::Align2::LEFT_BOTTOM, [8.0, -6.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    if ui.small_button("▲ Panel").on_hover_text("Zeige Recently / Favorites / Downloads").clicked() {
                        self.config.bottom_panel_height = (win_h * 0.30).clamp(80.0, max_bottom);
                        let _ = crate::config::write_config(&self.config);
                        ctx.request_repaint();
                    }
                });
        } else {
            let desired_h = self.config.bottom_panel_height.min(max_bottom);
            egui::TopBottomPanel::bottom("bottom")
                .resizable(true)
                .show_separator_line(true)
                .min_height(0.0)
                .max_height(max_bottom)
                .default_height(desired_h)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    // Recently / Favorites / Downloads
                    ui.columns(3, |cols| {
                    // Left column: Recently
                    cols[0].vertical(|ui| {
                        ui.label(RichText::new("Recently played").strong());
                        let h = ui.available_height();
                        egui::ScrollArea::vertical()
                            .id_source("recent_list")
                            .auto_shrink([false, false])
                            .max_height(h)
                            .show(ui, |ui| {
                                if self.recently.is_empty() {
                                    ui.weak("Nothing played yet.");
                                } else {
                                    for it in &self.recently {
                                        if ui.button(format!("{} ({})", it.name, it.info)).clicked()
                                        {
                                            let url = build_url_by_type(
                                                &self.config,
                                                &it.id,
                                                &it.info,
                                                it.container_extension.as_deref(),
                                            );
                                            // Prefer lokale Datei falls vorhanden (MPV soll nicht erneut Stream öffnen)
                                            let play_target = if let Some(p)=self.local_file_exists(&it.id, &it.name, it.container_extension.as_deref()) { file_path_to_uri(&p) } else { url.clone() };
                                            let _ = start_player(self.effective_config(), &play_target);
                                        }
                                    }
                                }
                            });
                    });
                    // Right column: Favorites
                    cols[1].vertical(|ui| {
                        ui.label(RichText::new("Favorites").strong());
                        let h = ui.available_height();
                        egui::ScrollArea::vertical()
                            .id_source("favorites_list")
                            .auto_shrink([false, false])
                            .max_height(h)
                            .show(ui, |ui| {
                                if self.favorites.is_empty() {
                                    ui.weak("No favorites yet.");
                                } else {
                                    let favorites = self.favorites.clone();
                                    for it in &favorites {
                                        ui.horizontal(|ui| {
                                            // Display name with type indicator
                                            let type_icon = match it.item_type.as_str() {
                                                "Series" => "📺",
                                                "Movie" => "🎬",
                                                "Channel" => "📡",
                                                "Episode" => "📺",
                                                _ => "📄"
                                            };
                                            ui.label(format!("{} {} ({})", type_icon, it.name, it.item_type));
                                            
                                            if it.item_type == "Series" {
                                                // For series, navigate to episodes view
                                                if ui.small_button("Episodes").clicked() {
                                                    if let Some(cv) = &self.current_view {
                                                        self.view_stack.push(cv.clone());
                                                    }
                                                    self.current_view = Some(ViewState::Episodes {
                                                        series_id: it.id.clone(),
                                                    });
                                                    self.is_loading = true;
                                                    self.loading_total = 1;
                                                    self.loading_done = 0;
                                                    self.spawn_load_episodes(it.id.clone());
                                                }
                                            } else {
                                                // For movies/channels/episodes, show play button
                                                let url =
                                                    it.stream_url.clone().unwrap_or_else(|| {
                                                        build_url_by_type(
                                                            &self.config,
                                                            &it.id,
                                                            &it.info,
                                                            it.container_extension.as_deref(),
                                                        )
                                                    });
                                                if ui.small_button("Play").clicked() {
                                                    let play_target = if let Some(p)=self.local_file_exists(&it.id, &it.name, it.container_extension.as_deref()) { file_path_to_uri(&p) } else { url.clone() };
                                                    let _ = start_player(self.effective_config(), &play_target);
                                                }
                                                if ui.small_button("Copy").clicked() {
                                                    ui.output_mut(|o| o.copied_text = url.clone());
                                                }
                                            }
                                            // Remove from favorites button
                                            if ui.small_button("✕").on_hover_text("Aus Favoriten entfernen").clicked() {
                                                toggle_favorite(&it);
                                                self.favorites = load_favorites();
                                            }
                                        });
                                    }
                                }
                            });
                    });
                    // Right column: Downloads (inline statt separates Fenster)
                    cols[2].vertical(|ui| {
                        ui.label(RichText::new("Downloads").strong());
                        // Trigger Scan (intern auf 5s gedrosselt)
                        if self.config.enable_downloads { self.scan_download_directory(); } else { ui.weak("Downloads disabled in settings"); }
                        let h = ui.available_height();
                        egui::ScrollArea::vertical()
                            .id_source("downloads_list")
                            .auto_shrink([false,false])
                            .max_height(h)
                            .show(ui, |ui| {
                                if self.download_order.is_empty() {
                                    ui.weak("No downloads yet.");
                                } else {
                                    let order_snapshot: Vec<String> = self.download_order.clone();
                                    for id in order_snapshot {
                                        if let (Some(meta), Some(st)) = (self.download_meta.get(&id), self.downloads.get(&id)) {
                                            // Kopiere benötigte Felder in lokale Variablen um Borrow-Konflikte zu vermeiden
                                            let name = meta.name.clone();
                                            let size_opt = meta.size;
                                            let waiting = st.waiting;
                                            let finished = st.finished;
                                            let error_opt = st.error.clone();
                                            let total_opt = st.total;
                                            let received = st.received;
                                            let path_opt = st.path.clone();
                                            let cancel_flag = st.cancel_flag.clone();
                                            let is_done_ok = finished && error_opt.is_none();
                                            let modified_opt = meta.modified;
                                            let cur_speed_bps = st.current_speed_bps;
                                            let avg_speed_bps = st.avg_speed_bps;
                                            ui.horizontal(|ui| {
                                                ui.label(name);
                                                if let Some(sz)=size_opt { ui.weak(format_file_size(sz)); }
                                                if waiting { ui.weak("waiting"); }
                                                else if finished {
                                                    if let Some(err)=error_opt.as_ref(){ ui.label(colored_text_by_type(&format!("error: {}",err),"error")); }
                                                    else { ui.label(colored_text_by_type("done","success")); }
                                                } else {
                                                    // Play button während des Downloads (wenn Pfad vorhanden)
                                                    if let Some(p)=&path_opt {
                                                        // Für .part Dateien absoluten Pfad verwenden statt file:// URI
                                                        if ui.small_button("Play").on_hover_text("Play partially downloaded file").clicked(){
                                                            let path_str = if p.ends_with(".part") {
                                                                p.clone() // Absoluter Pfad für .part Dateien
                                                            } else {
                                                                file_path_to_uri(Path::new(p))
                                                            };
                                                            let _= start_player(self.effective_config(), &path_str);
                                                        }
                                                    }
                                                    let frac = total_opt.map(|t| (received as f32 / t as f32).min(1.0)).unwrap_or(0.0);
                                                    let pct_text = if total_opt.is_some(){ format!("{:.0}%", frac*100.0) } else { format!("{} KB", received/1024) };
                                                    // Geschwindigkeiten (aktuell & Durchschnitt)
                                                    let cur_speed = if cur_speed_bps > 0.0 { crate::downloads::format_speed(cur_speed_bps) } else { "-".into() };
                                                    let avg_speed = if avg_speed_bps > 0.0 { crate::downloads::format_speed(avg_speed_bps) } else { "-".into() };
                                                    // ETA berechnen
                                                    let eta_text = if let Some(total) = total_opt {
                                                        if avg_speed_bps > 0.0 && received < total {
                                                            let remaining_bytes = total - received;
                                                            let eta_seconds = (remaining_bytes as f64 / avg_speed_bps) as u64;
                                                            if eta_seconds < 60 {
                                                                format!("{}s", eta_seconds)
                                                            } else if eta_seconds < 3600 {
                                                                format!("{}m", eta_seconds / 60)
                                                            } else {
                                                                format!("{}h {}m", eta_seconds / 3600, (eta_seconds % 3600) / 60)
                                                            }
                                                        } else { String::new() }
                                                    } else { String::new() };
                                                    // ProgressBar ohne Text
                                                    ui.add(egui::ProgressBar::new(frac).desired_width(100.0));
                                                    // Text daneben anzeigen
                                                    ui.label(format!("{} | {}/s (avg {}/s)", pct_text, cur_speed, avg_speed));
                                                    if !eta_text.is_empty() {
                                                        ui.weak(format!("ETA {}", eta_text));
                                                    }
                                                    if let Some(flag)=&cancel_flag { if ui.small_button("Cancel").clicked(){ flag.store(true, std::sync::atomic::Ordering::Relaxed); } }
                                                }
                                                if is_done_ok {
                                                    if let Some(p)=path_opt {
                                                        if ui.small_button("Play").clicked(){
                                                            let uri = file_path_to_uri(Path::new(&p));
                                                            let _= start_player(self.effective_config(), &uri);
                                                        }
                                                        if ui.small_button("Del").on_hover_text("Delete file").clicked(){
                                                            // Versuche Datei zu löschen
                                                            match std::fs::remove_file(&p) {
                                                                Err(e) => {
                                                                    self.last_error = Some(format!("Delete failed: {}", e));
                                                                }
                                                                Ok(_) => {
                                                                    // Lösche auch die .json Sidecar-Datei
                                                                    let p_path = Path::new(&p);
                                                                    if let Some(ext) = p_path.extension().and_then(|e| e.to_str()) {
                                                                        let sidecar = p_path.with_extension(format!("{}.json", ext));
                                                                        let _ = std::fs::remove_file(&sidecar);
                                                                    }
                                                                    // Sofortiges Entfernen aus internen Strukturen
                                                                    self.downloads.remove(&id);
                                                                    self.download_meta.remove(&id);
                                                                    self.download_order.retain(|x| x != &id);
                                                                    // Kein sofortiger Re-Scan nötig; falls dennoch gewünscht: self.scan_download_directory();
                                                                    ctx.request_repaint();
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        // Falls kein Pfad vorhanden aber Eintrag fertig -> Button zum Entfernen anbieten
                                                        if ui.small_button("Remove").on_hover_text("Remove entry").clicked() {
                                                            self.downloads.remove(&id);
                                                            self.download_meta.remove(&id);
                                                            self.download_order.retain(|x| x != &id);
                                                            ctx.request_repaint();
                                                        }
                                                    }
                                                    if let Some(mt)=modified_opt {
                                                        if let Ok(delta)=mt.elapsed(){
                                                            let mins = delta.as_secs()/60; ui.weak(format!("{}m ago", mins));
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                            });
                        if ui.button("Clear finished errors").on_hover_text("Remove finished error entries").clicked(){
                            self.downloads.retain(|_,s| !s.finished || s.error.is_none());
                            self.download_order.retain(|id| self.downloads.contains_key(id));
                        }
                    }); // Ende Downloads Spalte
                }); // Ende columns(3,...)
                    // Panelhöhe nach Rendering messen über ui.max_rect()
                    let current_h = ui.max_rect().height();
                    if current_h < 24.0 { // sehr klein => einklappen
                        if self.config.bottom_panel_height != 0.0 {
                            self.config.bottom_panel_height = 0.0;
                            let _ = crate::config::write_config(&self.config);
                        }
                    } else if (current_h - self.config.bottom_panel_height).abs() > 4.0 {
                        self.config.bottom_panel_height = current_h.min(max_bottom);
                        let _ = crate::config::write_config(&self.config);
                    }
                });
        }

        // Wisdom-Gate AI Empfehlungen in linker Seitenleiste
        egui::SidePanel::left("wisdom_gate_recommendations")
            .default_width(if self.config.left_panel_width>250.0 { self.config.left_panel_width } else { 300.0 })
            .width_range(250.0..=500.0)
            .resizable(true)
            .show(ctx, |ui| {
                self.render_wisdom_gate_panel(ui);
                let w = ui.max_rect().width();
                if (w - self.config.left_panel_width).abs() > 4.0 { self.config.left_panel_width = w; let _ = crate::config::write_config(&self.config); }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            // Capture available size (after reserving top & bottom panels)
            let avail_w = ui.available_width();
            let avail_h = ui.available_height();
            ui.set_width(avail_w);
            
            // Spezielle Behandlung für Suchansicht
            if let Some(ViewState::Search { query }) = &self.current_view {
                match &self.search_status {
                    SearchStatus::Indexing { progress } => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(avail_h / 3.0);
                            ui.horizontal(|ui| {
                                ui.add_space(avail_w / 2.0 - 120.0);
                                ui.spinner();
                                ui.label(RichText::new("🏗️ Index wird erstellt").size(18.0).strong());
                            });
                            ui.add_space(10.0);
                            ui.label(RichText::new(progress).size(14.0).color(Color32::GRAY));
                            ui.add_space(10.0);
                            ui.add(
                                egui::ProgressBar::new(0.5)
                                    .show_percentage()
                                    .text("Katalog wird aufgebaut...")
                                    .desired_width(300.0),
                            );
                            ui.add_space(10.0);
                            ui.label(RichText::new("Search is available once the index is ready.").size(12.0).color(Color32::from_rgb(150, 150, 150)));
                        });
                        return; // Früher Return, um Tabelle nicht zu rendern
                    }
                    SearchStatus::Searching => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(avail_h / 3.0);
                            ui.horizontal(|ui| {
                                ui.add_space(avail_w / 2.0 - 100.0);
                                ui.spinner();
                                ui.label(RichText::new(&format!("Suche nach '{}'...", query)).size(16.0));
                            });
                            ui.add_space(10.0);
                            ui.add(
                                egui::ProgressBar::new(0.5)
                                    .show_percentage()
                                    .text("Durchsuche Katalog...")
                                    .desired_width(300.0),
                            );
                        });
                        return; // Früher Return, um Tabelle nicht zu rendern
                    }
                    SearchStatus::NoResults => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(avail_h / 3.0);
                            ui.label(RichText::new("🔍 Keine Ergebnisse gefunden").size(18.0).color(Color32::from_rgb(255, 165, 0)));
                            ui.add_space(10.0);
                            ui.label(RichText::new(&format!("No results found for '{}'.", query)).size(14.0));
                            ui.add_space(10.0);
                            ui.label("Versuche es mit einem anderen Suchbegriff.");
                        });
                        return;
                    }
                    SearchStatus::Error(err) => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(avail_h / 3.0);
                            ui.label(RichText::new("❌ Suchfehler").size(18.0).color(Color32::RED));
                            ui.add_space(10.0);
                            ui.label(RichText::new(err).size(14.0).color(Color32::from_rgb(200, 100, 100)));
                        });
                        return;
                    }
                    SearchStatus::Completed { results } => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&format!("🔍 Search results for '{}': {} found", query, results)).strong());
                        });
                        ui.separator();
                    }
                    SearchStatus::Idle => {
                        // Zeige normale Tabelle
                    }
                }
            }
            
            let rows = self.content_rows.clone();
            // Workaround für Borrow-Checker: Spalten- und Zeilen-Konfiguration kopieren
            let columns = self.column_config.clone();
            let mut rows = rows.clone();

                        // Spalten-Konfigurations-UI
                        let mut show_column_popup = false;
                        ui.horizontal(|ui| {
                            let anchor = ui.button("⚙️ Spalten konfigurieren");
                            if anchor.clicked() {
                                show_column_popup = true;
                            }
                        });
                        if show_column_popup {
                            ui.memory_mut(|mem| mem.open_popup(egui::Id::new("column_config_popup")));
                        }
                        let anchor = ui.button("");
                        egui::popup::popup_below_widget(ui, egui::Id::new("column_config_popup"), &anchor, |ui| {
                            ui.label("Spalten ein-/ausblenden und sortieren:");
                            let all_columns = [
                                ColumnKey::Cover, ColumnKey::Name, ColumnKey::ID, ColumnKey::Info, ColumnKey::Year,
                                ColumnKey::ReleaseDate, ColumnKey::Rating, ColumnKey::Genre, ColumnKey::Languages, ColumnKey::Path, ColumnKey::Actions
                            ];
                            // Sichtbarkeit togglen
                            for col in &all_columns {
                                let mut visible = self.column_config.contains(col);
                                let label = format!("{:?}", col);
                                if ui.checkbox(&mut visible, label).changed() {
                                    if visible && !self.column_config.contains(col) {
                                        self.column_config.push(*col);
                                        self.save_column_config();
                                    } else if !visible {
                                        self.column_config.retain(|c| c != col);
                                        self.save_column_config();
                                    }
                                }
                            }
                            ui.separator();
                            ui.label("Reorder columns (drag & drop):");
                            for col in self.column_config.iter() {
                                // TODO: Drag-and-drop für Spaltenreihenfolge mit egui unterstützen
                                // Derzeit deaktiviert, da die API nicht existiert (start_drag, dragged_item, stop_drag)
                                let (rect, _response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::click());
                                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, format!("{:?}", col), egui::TextStyle::Button.resolve(ui.style()), Color32::BLACK);
                            }
                            // Drag-and-drop für Spaltenreihenfolge ist aktuell deaktiviert
                            if ui.button("Schließen").clicked() {
                                ui.memory_mut(|mem| mem.close_popup());
                            }
                        });

                        // Hilfsfunktion: Spalten-Konfiguration speichern
                        // (column_config_to_csv als freie Funktion weiter unten)
            
            // Apply language filter if enabled
            if self.config.filter_by_language && !self.config.default_search_languages.is_empty() {
                rows.retain(|row| {
                    // If no audio_languages info, keep the item (could be a channel or missing metadata)
                    if let Some(langs) = &row.audio_languages {
                        let langs_upper = langs.to_uppercase();
                        // Check if any of the configured languages appear in the item's languages
                        self.config.default_search_languages.iter().any(|filter_lang| {
                            langs_upper.contains(&filter_lang.to_uppercase())
                        })
                    } else {
                        // Keep items without language info (channels, etc.)
                        true
                    }
                });
            }
            
            // Apply sorting if active
            if let Some(key) = self.sort_key {
                match key {
                    SortKey::Name => {
                        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    }
                    SortKey::Year => {
                        fn parse_year(y: &Option<String>) -> i32 {
                            y.as_deref()
                                .and_then(|s| s.parse::<i32>().ok())
                                .unwrap_or(0)
                        }
                        rows.sort_by(|a, b| parse_year(&a.year).cmp(&parse_year(&b.year)));
                    }
                    SortKey::ReleaseDate => {
                        use chrono::NaiveDate;
                        fn parse_date(s: &Option<String>) -> NaiveDate {
                            if let Some(s) = s {
                                // Versuche yyyy-mm-dd
                                if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                                    return d;
                                }
                                // Versuche nur yyyy
                                if let Ok(y) = s.parse::<i32>() {
                                    return NaiveDate::from_ymd_opt(y, 1, 1).unwrap_or(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
                                }
                            }
                            // Fallback: sehr altes Datum
                            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
                        }
                        rows.sort_by(|a, b| parse_date(&a.release_date).cmp(&parse_date(&b.release_date)));
                    }
                    SortKey::Rating => {
                        rows.sort_by(|a, b| {
                            let av = a.rating_5based.unwrap_or(-1.0);
                            let bv = b.rating_5based.unwrap_or(-1.0);
                            av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                    SortKey::Genre => {
                        rows.sort_by(|a, b| {
                            a.genre
                                .clone()
                                .unwrap_or_default()
                                .to_lowercase()
                                .cmp(&b.genre.clone().unwrap_or_default().to_lowercase())
                        });
                    }
                    SortKey::Languages => {
                        rows.sort_by(|a, b| {
                            a.audio_languages
                                .clone()
                                .unwrap_or_default()
                                .to_lowercase()
                                .cmp(&b.audio_languages.clone().unwrap_or_default().to_lowercase())
                        });
                    }
                }
                if !self.sort_asc {
                    rows.reverse();
                }
            }
            let cover_w = self.cover_height * (2.0 / 3.0);
            let row_h = (self.cover_height + 8.0).max(28.0);
            let header_h = 22.0;
            let mut table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .vscroll(true)
                .max_scroll_height(avail_h);
            for col in &columns {
                match col {
                    ColumnKey::Cover => table = table.column(egui_extras::Column::initial(cover_w + 16.0)),
                    ColumnKey::Name => table = table.column(egui_extras::Column::initial(400.0).at_least(400.0)),
                    ColumnKey::ID => table = table.column(egui_extras::Column::initial(140.0)),
                    ColumnKey::Info => table = table.column(egui_extras::Column::initial(120.0)),
                    ColumnKey::Year => table = table.column(egui_extras::Column::initial(80.0)),
                    ColumnKey::ReleaseDate => table = table.column(egui_extras::Column::initial(100.0)),
                    ColumnKey::Rating => table = table.column(egui_extras::Column::initial(80.0)),
                    ColumnKey::Genre => table = table.column(egui_extras::Column::initial(200.0)),
                    ColumnKey::Languages => table = table.column(egui_extras::Column::initial(150.0)),
                    ColumnKey::Path => table = table.column(egui_extras::Column::initial(220.0)),
                    ColumnKey::CurrentProgram => table = table.column(egui_extras::Column::initial(250.0)),
                    ColumnKey::Actions => table = table.column(egui_extras::Column::remainder().at_least(320.0)),
                }
            }
            table.header(header_h, |mut header| {
                for col in &columns {
                    match col {
                        ColumnKey::Cover => { header.col(|ui| { ui.strong("Cover"); }); },
                        ColumnKey::Name => { header.col(|ui| {
                            let selected = self.sort_key == Some(SortKey::Name);
                            let label = if selected {
                                format!("Name {}", if self.sort_asc { "▲" } else { "▼" })
                            } else { "Name".to_string() };
                            if ui.small_button(label).clicked() {
                                if selected { self.sort_asc = !self.sort_asc; } else { self.sort_key = Some(SortKey::Name); self.sort_asc = true; }
                            }
                        }); },
                        ColumnKey::ID => { header.col(|ui| { ui.strong("ID"); }); },
                        ColumnKey::Info => { header.col(|ui| { ui.strong("Info"); }); },
                        ColumnKey::Year => { header.col(|ui| {
                            let selected = self.sort_key == Some(SortKey::Year);
                            let label = if selected {
                                format!("Year {}", if self.sort_asc { "▲" } else { "▼" })
                            } else { "Year".to_string() };
                            if ui.small_button(label).clicked() {
                                if selected { self.sort_asc = !self.sort_asc; } else { self.sort_key = Some(SortKey::Year); self.sort_asc = true; }
                            }
                        }); },
                        ColumnKey::ReleaseDate => { header.col(|ui| {
                            let selected = self.sort_key == Some(SortKey::ReleaseDate);
                            let label = if selected {
                                format!("Release Date {}", if self.sort_asc { "▲" } else { "▼" })
                            } else { "Release Date".to_string() };
                            if ui.small_button(label).clicked() {
                                if selected { self.sort_asc = !self.sort_asc; } else { self.sort_key = Some(SortKey::ReleaseDate); self.sort_asc = true; }
                            }
                        }); },
                        ColumnKey::Rating => { header.col(|ui| {
                            let selected = self.sort_key == Some(SortKey::Rating);
                            let label = if selected {
                                format!("Rating {}", if self.sort_asc { "▲" } else { "▼" })
                            } else { "Rating".to_string() };
                            if ui.small_button(label).clicked() {
                                if selected { self.sort_asc = !self.sort_asc; } else { self.sort_key = Some(SortKey::Rating); self.sort_asc = false; }
                            }
                        }); },
                        ColumnKey::Genre => { header.col(|ui| {
                            let selected = self.sort_key == Some(SortKey::Genre);
                            let label = if selected {
                                format!("Genre {}", if self.sort_asc { "▲" } else { "▼" })
                            } else { "Genre".to_string() };
                            if ui.small_button(label).clicked() {
                                if selected { self.sort_asc = !self.sort_asc; } else { self.sort_key = Some(SortKey::Genre); self.sort_asc = true; }
                            }
                        }); },
                        ColumnKey::Languages => { header.col(|ui| {
                            let selected = self.sort_key == Some(SortKey::Languages);
                            let label = if selected {
                                format!("Languages {}", if self.sort_asc { "▲" } else { "▼" })
                            } else { "Languages".to_string() };
                            if ui.small_button(label).clicked() {
                                if selected { self.sort_asc = !self.sort_asc; } else { self.sort_key = Some(SortKey::Languages); self.sort_asc = true; }
                            }
                        }); },
                        ColumnKey::Path => { header.col(|ui| { ui.strong("Path"); }); },
                        ColumnKey::CurrentProgram => { header.col(|ui| { ui.strong("Current Program"); }); },
                        ColumnKey::Actions => { header.col(|ui| { ui.strong("Action"); }); },
                    }
                }
            })
            .body(|body| {
                let row_count = rows.len();
                body.rows(row_h, row_count, |i, mut row| {
                    let r = &rows[i];
                    let url = if r.info == "SeriesEpisode" {
                        build_url_by_type(&self.config, &r.id, &r.info, r.container_extension.as_deref())
                    } else {
                        r.stream_url.clone().unwrap_or_else(|| build_url_by_type(&self.config, &r.id, &r.info, r.container_extension.as_deref()))
                    };
                    for col in &columns {
                        match col {
                                    ColumnKey::Cover => { row.col(|ui| {
                                        if let Some(cu) = &r.cover_url {
                                            if let Some(tex) = self.textures.get(cu) {
                                                ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(cover_w, self.cover_height)));
                                            } else {
                                                let rect = ui.allocate_exact_size(egui::vec2(cover_w, self.cover_height), egui::Sense::hover()).0;
                                                ui.painter().rect_filled(rect, 4.0, Color32::from_gray(60));
                                                self.spawn_fetch_cover(cu);
                                            }
                                        }
                                    }); },
                            ColumnKey::Name => { row.col(|ui| {
                                if r.info == "Series" {
                                    if ui.link(&r.name).clicked() {
                                        if let Some(cv) = &self.current_view {
                                            self.view_stack.push(cv.clone());
                                        }
                                        self.current_view = Some(ViewState::Episodes { series_id: r.id.clone() });
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                        self.spawn_load_episodes(r.id.clone());
                                    }
                                } else {
                                    ui.label(&r.name);
                                }
                            }); },
                            ColumnKey::ID => { row.col(|ui| { ui.label(&r.id); }); },
                            ColumnKey::Info => { row.col(|ui| { ui.label(&r.info); }); },
                            ColumnKey::Year => { row.col(|ui| { ui.label(r.year.clone().unwrap_or_default()); }); },
                            ColumnKey::ReleaseDate => { row.col(|ui| { ui.label(r.release_date.clone().unwrap_or_default()); }); },
                            ColumnKey::Rating => { row.col(|ui| { ui.label(r.rating_5based.map(|v| format!("{:.1}", v)).unwrap_or_default()); }); },
                            ColumnKey::Genre => { row.col(|ui| { ui.label(r.genre.clone().unwrap_or_default()); }); },
                            ColumnKey::Languages => { row.col(|ui| { ui.label(r.audio_languages.clone().unwrap_or_default()); }); },
                            ColumnKey::Path => { row.col(|ui| { ui.label(r.path.clone().unwrap_or_default()); }); },
                            ColumnKey::CurrentProgram => { row.col(|ui| {
                                // Only show for live channels
                                if r.info == "Channel" {
                                    if let Some(program) = self.epg_data.get(&r.id) {
                                        ui.label(program);
                                    } else if self.epg_loading.contains(&r.id) {
                                        ui.spinner();
                                    } else {
                                        ui.label(egui::RichText::new("N/A").weak());
                                    }
                                } else {
                                    ui.label("");
                                }
                            }); },
                            ColumnKey::Actions => { row.col(|ui| {
                                ui.horizontal_wrapped(|ui| {
                                    if r.info == "Series" {
                                        if ui.small_button("Episodes").clicked() {
                                            if let Some(cv) = &self.current_view {
                                                self.view_stack.push(cv.clone());
                                            }
                                            self.current_view = Some(ViewState::Episodes { series_id: r.id.clone() });
                                            self.is_loading = true;
                                            self.loading_total = 1;
                                            self.loading_done = 0;
                                            self.spawn_load_episodes(r.id.clone());
                                        }
                                        let is_fav = is_favorite(&r.id, &r.info, "Series");
                                        let fav_text = if is_fav { "★" } else { "☆" };
                                        if ui.small_button(fav_text).on_hover_text("Zu Favoriten hinzufügen/entfernen").clicked() {
                                            toggle_favorite(&FavItem {
                                                id: r.id.clone(),
                                                info: r.info.clone(),
                                                name: r.name.clone(),
                                                item_type: "Series".to_string(),
                                                stream_url: None,
                                                container_extension: None,
                                                cover: r.cover_url.clone(),
                                                series_id: None,
                                            });
                                            self.favorites = load_favorites();
                                        }
                                        if self.config.enable_downloads && ui.small_button("Download all").on_hover_text("Queue all episodes for download").clicked() {
                                            self.confirm_bulk = Some((r.id.clone(), r.name.clone()));
                                        }
                                    } else {
                                        if ui.small_button("Play").clicked() {
                                            if self.config.address.is_empty() || self.config.username.is_empty() || self.config.password.is_empty() {
                                                self.last_error = Some("Please set address/username/password in Settings".into());
                                            } else {
                                                // Binge Watch: For Series Episodes, create playlist from current episode onwards
                                                if r.info == "SeriesEpisode" {
                                                    let current_idx = rows.iter().position(|row| row.id == r.id);
                                                    if let Some(idx) = current_idx {
                                                        // Create playlist from current episode onwards
                                                        let playlist_entries: Vec<(String, String)> = rows[idx..]
                                                            .iter()
                                                            .filter(|row| row.info == "SeriesEpisode")
                                                            .map(|row| {
                                                                let url = build_url_by_type(&self.config, &row.id, &row.info, row.container_extension.as_deref());
                                                                (row.name.clone(), url)
                                                            })
                                                            .collect();
                                                        
                                                        if playlist_entries.len() > 1 {
                                                            // Play as playlist (binge watch)
                                                            if let Err(e) = self.create_and_play_m3u(&playlist_entries) {
                                                                self.last_error = Some(format!("Failed to create binge watch playlist: {}", e));
                                                            }
                                                        } else {
                                                            // Single episode
                                                            let play_url = self.resolve_play_url(r);
                                                            let _ = start_player(self.effective_config(), &play_url);
                                                        }
                                                    }
                                                } else {
                                                    // Regular single play for movies/channels
                                                    let play_url = self.resolve_play_url(r);
                                                    let _ = start_player(self.effective_config(), &play_url);
                                                }
                                            }
                                            let rec = RecentItem {
                                                id: r.id.clone(),
                                                name: r.name.clone(),
                                                info: r.info.clone(),
                                                stream_url: build_url_by_type(&self.config, &r.id, &r.info, r.container_extension.as_deref()),
                                                container_extension: r.container_extension.clone(),
                                            };
                                            add_to_recently(&rec);
                                            self.recently = load_recently_played();
                                        }
                                        if ui.small_button("Copy").clicked() {
                                            ui.output_mut(|o| o.copied_text = url.clone());
                                        }
                                        if r.info == "Movie" || r.info == "SeriesEpisode" || r.info == "Channel" {
                                                if self.config.enable_downloads && ui.small_button(&t("download", self.config.language)).on_hover_text("Download this item").clicked() {
                                                    self.spawn_download(r);
                                                }
                                                
                                                // Add "Download from here" for series episodes
                                                if r.info == "SeriesEpisode" && self.config.enable_downloads {
                                                    if ui.small_button(&t("download_from_here", self.config.language))
                                                        .on_hover_text(&t("download_from_here_tooltip", self.config.language))
                                                        .clicked() {
                                                        self.download_episodes_from_here(&rows, r);
                                                    }
                                                }
                                        }
                                    }
                                });
                            }); },
                        }
                    }
                });
            });
                });

        if self.show_config {
            let mut open = self.show_config;
            let mut cancel_clicked = false;
            egui::Window::new("🔧 Configuration")
                .collapsible(false)
                .resizable(true)
                .default_width(600.0)
                .default_height(650.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .open(&mut open)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            let draft = self.config_draft.get_or_insert_with(|| self.config.clone());
                            
                            ui.collapsing("📡 Server", |ui| {
                                // Server Profile Selector and Manager
                                ui.horizontal(|ui| {
                                    ui.label("Profile:");
                                    let profile_names: Vec<String> = draft.server_profiles.iter().enumerate()
                                        .map(|(i, p)| format!("{}: {}", i + 1, p.name))
                                        .collect();
                                    let mut selected = draft.active_profile_index;
                                    let current_text = profile_names.get(selected)
                                        .cloned()
                                        .unwrap_or_else(|| "No profiles".to_string());
                                    egui::ComboBox::from_id_source("server_profile_selector")
                                        .selected_text(current_text)
                                        .show_ui(ui, |ui| {
                                            for (i, name) in profile_names.iter().enumerate() {
                                                if ui.selectable_value(&mut selected, i, name).clicked() {
                                                    draft.active_profile_index = i;
                                                }
                                            }
                                        });
                                    if ui.button("⚙ Manage").on_hover_text("Manage server profiles").clicked() {
                                        self.show_server_manager = true;
                                    }
                                });
                                
                                ui.separator();
                                
                                // Edit active profile
                                let active = draft.active_profile_mut();
                                ui.label("Profile Name");
                                ui.add(egui::TextEdit::singleline(&mut active.name).desired_width(f32::INFINITY));
                                ui.label("URL");
                                ui.add(egui::TextEdit::singleline(&mut active.address).desired_width(f32::INFINITY));
                                ui.label("Username");
                                ui.add(egui::TextEdit::singleline(&mut active.username).desired_width(f32::INFINITY));
                                ui.label("Password");
                                ui.add(egui::TextEdit::singleline(&mut active.password).password(true).desired_width(f32::INFINITY));
                            });

                            ui.collapsing("🎬 Player", |ui| {
                                ui.label("Custom Player Command (optional)");
                                ui.add(egui::TextEdit::multiline(&mut draft.player_command).desired_rows(2).desired_width(f32::INFINITY));
                                ui.small("Use {URL} placeholder where the stream URL goes");
                                ui.horizontal(|ui| {
                                    let mut reuse = draft.reuse_vlc; if ui.checkbox(&mut reuse, "Reuse VLC").on_hover_text("Open links in running VLC instance (macOS)").changed() { draft.reuse_vlc = reuse; }
                                    let mut use_mpv = draft.use_mpv; if ui.checkbox(&mut use_mpv, "Use MPV").on_hover_text(if self.has_mpv {"enable mpv"} else {"mpv not found"}).changed() { draft.use_mpv = use_mpv; if use_mpv { draft.reuse_vlc = false; } }
                                    if self.has_vlc { if let Some(v)=&self.vlc_version { ui.label(egui::RichText::new(format!("vlc {}", v)).small()); }}
                                    if self.has_mpv { if let Some(v)=&self.mpv_version { ui.label(egui::RichText::new(format!("mpv {}", v)).small()); }}
                                });
                                ui.horizontal(|ui| {
                                    let mut low = draft.low_cpu_mode; if ui.checkbox(&mut low, "Low CPU").on_hover_text("Reduces repaints and diagnostic frequency").changed() { draft.low_cpu_mode = low; }
                                    let mut ultra = draft.ultra_low_flicker_mode; if ui.checkbox(&mut ultra, "Ultra Flicker").on_hover_text("Event-basierte Repaints – evtl. träge").changed() { draft.ultra_low_flicker_mode = ultra; }
                                });
                                // Bias Slider
                                ui.horizontal(|ui| {
                                    ui.label("Latency/Stability Bias");
                                    let mut bias = draft.vlc_profile_bias.min(100) as i32;
                                    if ui.add(egui::Slider::new(&mut bias, 0..=100)).changed() { draft.vlc_profile_bias = bias as u32; }
                                    ui.weak("0=low latency 100=stable");
                                });
                                if ui.button("Apply Bias").on_hover_text("Rebuild VLC command using current bias").clicked() { draft.player_command = crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Default, &draft); }

                                // Command Preview (from earlier stabilized box)
                                let preview = if draft.use_mpv { let mut args = vec!["mpv".to_string(), "--force-window=no".into(), "--fullscreen".into()]; let cache = if draft.mpv_cache_secs_override!=0 { draft.mpv_cache_secs_override } else { (draft.vlc_network_caching_ms/1000).max(1) }; args.push(format!("--cache-secs={}", cache)); let readahead = if draft.mpv_readahead_secs_override!=0 { draft.mpv_readahead_secs_override } else { (draft.vlc_file_caching_ms/1000).max(1) }; args.push(format!("--demuxer-readahead-secs={}", readahead)); if !draft.mpv_extra_args.trim().is_empty() { args.extend(draft.mpv_extra_args.split_whitespace().map(|s| s.to_string())); } args.push("<URL>".into()); args.join(" ") } else { crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Default, &draft) }; if self.command_preview != preview { self.command_preview = preview; }
                                ui.collapsing("Preview", |ui| {
                                    let (n,l,f)=crate::player::apply_bias(&draft);
                                    let (rect,response)=ui.allocate_exact_size(egui::vec2(ui.available_width(),52.0),egui::Sense::hover());
                                    let painter=ui.painter(); painter.rect_filled(rect,4.0,ui.visuals().extreme_bg_color); painter.text(rect.min+egui::vec2(8.0,8.0),egui::Align2::LEFT_TOP,&self.command_preview,egui::TextStyle::Monospace.resolve(ui.style()),ui.visuals().text_color());
                                    if response.hovered(){ egui::show_tooltip(ui.ctx(),egui::Id::new("cmd_prev_tip2"),|ui|{ui.label(format!("Bias -> net={} live={} file={}",n,l,f));}); }
                                });
                                ui.collapsing("MPV Optionen", |ui| {
                                    if !self.has_mpv { ui.colored_label(egui::Color32::RED, "mpv nicht installiert"); }
                                    let mut custom = draft.mpv_custom_path.clone();
                                    if ui.add(egui::TextEdit::singleline(&mut custom).hint_text("mpv Custom Path (optional)")).changed(){ draft.mpv_custom_path = custom; }
                                    if ui.small_button("Test mpv Pfad").on_hover_text("Versucht mpv --version auszuführen").clicked() {
                                        if !draft.mpv_custom_path.trim().is_empty() {
                                            if std::path::Path::new(draft.mpv_custom_path.trim()).exists() {
                                                if let Ok(out) = std::process::Command::new(draft.mpv_custom_path.trim()).arg("--version").output() {
                                                    if let Ok(txt) = String::from_utf8(out.stdout) { println!("mpv custom test: {}", txt.lines().next().unwrap_or("(leer)")); }
                                                }
                                            } else { println!("mpv custom path existiert nicht"); }
                                        }
                                    }
                                    if ui.small_button("Detect jetzt").on_hover_text("Erneute Player-Erkennung mit Custom Path").clicked() {
                                        // Nur Flag setzen – tatsächliche Erkennung außerhalb des UI-Borrows
                                        self.pending_player_redetect = true;
                                    }
                                    let mut extra = draft.mpv_extra_args.clone(); if ui.add(egui::TextEdit::singleline(&mut extra).hint_text("extra mpv args")).changed(){ draft.mpv_extra_args=extra; }
                                    ui.horizontal(|ui| {
                                        let mut cache_override = draft.mpv_cache_secs_override.to_string(); if ui.add(egui::TextEdit::singleline(&mut cache_override).hint_text("cache-secs (0 auto)")).changed(){ draft.mpv_cache_secs_override=cache_override.parse().unwrap_or(0);} 
                                        let mut ra_override = draft.mpv_readahead_secs_override.to_string(); if ui.add(egui::TextEdit::singleline(&mut ra_override).hint_text("readahead (0 auto)")).changed(){ draft.mpv_readahead_secs_override=ra_override.parse().unwrap_or(0);} 
                                    });
                                });
                                ui.add_enabled_ui(!draft.use_mpv, |ui| {
                                    ui.collapsing("VLC Diagnose", |ui| {
                                        ui.horizontal(|ui| {
                                            let mut verbose=draft.vlc_verbose; if ui.checkbox(&mut verbose,"Verbose").changed(){draft.vlc_verbose=verbose;}
                                            let mut diag_once=draft.vlc_diagnose_on_start; if ui.checkbox(&mut diag_once,"Once").changed(){draft.vlc_diagnose_on_start=diag_once;}
                                            let mut cont=draft.vlc_continuous_diagnostics; if ui.checkbox(&mut cont,"Continuous").changed(){draft.vlc_continuous_diagnostics=cont;}
                                            if draft.vlc_continuous_diagnostics { if ui.button("Stop").clicked(){ let _=self.tx.send(Msg::StopDiagnostics); } }
                                        });
                                        if let Some(suggestion)=self.vlc_diag_suggestion { ui.horizontal(|ui| { ui.label(format!("Suggestion net={} live={} file={}",suggestion.0,suggestion.1,suggestion.2)); if ui.button("Apply").clicked(){ draft.vlc_network_caching_ms=suggestion.0; draft.vlc_live_caching_ms=suggestion.1; draft.vlc_file_caching_ms=suggestion.2; let ts=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(); let entry=format!("{}:{}:{}:{}",ts,suggestion.0,suggestion.1,suggestion.2); let mut parts:Vec<String>=draft.vlc_diag_history.split(';').filter(|s|!s.is_empty()).map(|s|s.to_string()).collect(); parts.push(entry); if parts.len()>10 { let overflow=parts.len()-10; parts.drain(0..overflow);} draft.vlc_diag_history=parts.join(";"); } }); }
                                        if !draft.vlc_diag_history.trim().is_empty(){ ui.collapsing("History",|ui|{ for seg in draft.vlc_diag_history.split(';').filter(|s|!s.is_empty()).rev(){ let cols:Vec<&str>=seg.split(':').collect(); if cols.len()==4 { ui.label(format!("ts={} net={} live={} file={}",cols[0],cols[1],cols[2],cols[3])); } } }); }
                                    });
                                });
                            });
                            
                            ui.collapsing("💾 Downloads", |ui| {
                                ui.label("Download directory");
                                ui.add(egui::TextEdit::singleline(&mut draft.download_dir).desired_width(f32::INFINITY));
                                if draft.download_dir.trim().is_empty(){ ui.weak("Default: ~/Downloads/macxtreamer"); }
                                let mut enable = draft.enable_downloads; if ui.checkbox(&mut enable, "Enable Downloads").changed(){ draft.enable_downloads=enable; }
                                ui.horizontal(|ui| { ui.label("Max parallel:"); let mut mp = if draft.max_parallel_downloads==0 {1}else{draft.max_parallel_downloads} as f32; if ui.add(egui::Slider::new(&mut mp,1.0..=5.0).integer()).changed(){ draft.max_parallel_downloads=mp as u32; }});
                            });
                            
                            ui.collapsing("🔍 Search Settings", |ui| {
                                ui.label("Default Language Filter");
                                ui.weak("Select languages to filter by default when searching");
                                let common_langs = vec!["EN", "DE", "FR", "ES", "IT", "MULTI", "4K", "HD"];
                                ui.horizontal_wrapped(|ui| {
                                    for lang in common_langs {
                                        let mut selected = draft.default_search_languages.contains(&lang.to_string());
                                        if ui.checkbox(&mut selected, lang).clicked() {
                                            if selected {
                                                if !draft.default_search_languages.contains(&lang.to_string()) {
                                                    draft.default_search_languages.push(lang.to_string());
                                                }
                                            } else {
                                                draft.default_search_languages.retain(|l| l != lang);
                                            }
                                        }
                                    }
                                });
                                if ui.button("Reset to Default (EN, DE, MULTI)").clicked() {
                                    draft.default_search_languages = vec!["EN".to_string(), "DE".to_string(), "MULTI".to_string()];
                                }
                            });
                            
                            ui.add_space(8.0);

                            // SOCKS5 / Proxy Einstellungen
                            ui.collapsing(&t("proxy_settings", self.config.language), |ui| {
                                ui.horizontal(|ui| {
                                    let mut enabled = draft.proxy_enabled;
                                    if ui.checkbox(&mut enabled, &t("proxy_enable", self.config.language)).changed() {
                                        draft.proxy_enabled = enabled;
                                    }
                                    ui.weak(&t("proxy_help", self.config.language));
                                });

                                ui.horizontal(|ui| {
                                    ui.label(&t("proxy_host", self.config.language));
                                    ui.add(egui::TextEdit::singleline(&mut draft.proxy_host).desired_width(240.0));
                                    ui.label(&t("proxy_port", self.config.language));
                                    let mut port = draft.proxy_port as i32;
                                    if ui.add(egui::DragValue::new(&mut port).clamp_range(0..=65535)).changed() {
                                        draft.proxy_port = port as u16;
                                    }
                                });

                                ui.horizontal(|ui| {
                                    ui.label(&t("proxy_username", self.config.language));
                                    ui.add(egui::TextEdit::singleline(&mut draft.proxy_username).desired_width(160.0));
                                    ui.label(&t("proxy_password", self.config.language));
                                    ui.add(egui::TextEdit::singleline(&mut draft.proxy_password).password(true).desired_width(160.0));
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Type");
                                    let mut pt = draft.proxy_type.clone();
                                    egui::ComboBox::from_id_source("proxy_type")
                                        .selected_text(&pt)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut pt, "socks5".to_string(), "SOCKS5");
                                            ui.selectable_value(&mut pt, "http".to_string(), "HTTP (Privoxy)");
                                        });
                                    if pt != draft.proxy_type { draft.proxy_type = pt; }

                                    if ui.button(&t("proxy_test", draft.language)).clicked() {
                                        let tx = self.tx.clone();
                                        let cfg = draft.clone();
                                        tokio::spawn(async move {
                                            let res = crate::network::test_socks5_connection(&cfg).await;
                                            match res {
                                                Ok(msg) => { let _ = tx.send(Msg::ProxyTestResult { success: true, message: msg }); }
                                                Err(e) => { let _ = tx.send(Msg::ProxyTestResult { success: false, message: e }); }
                                            }
                                        });
                                    }
                                });
                            });

                            ui.heading("🎬 Player Settings");
                            ui.separator();
                            // Update command preview (Draft Config)
                            let preview = if draft.use_mpv {
                                let mut args = vec!["mpv".to_string(), "--force-window=no".into(), "--fullscreen".into()];
                                let cache = if draft.mpv_cache_secs_override!=0 { draft.mpv_cache_secs_override } else { (draft.vlc_network_caching_ms/1000).max(1) }; 
                                args.push(format!("--cache-secs={}", cache));
                                let readahead = if draft.mpv_readahead_secs_override!=0 { draft.mpv_readahead_secs_override } else { (draft.vlc_file_caching_ms/1000).max(1) }; 
                                args.push(format!("--demuxer-readahead-secs={}", readahead));
                                if !draft.mpv_extra_args.trim().is_empty() { args.extend(draft.mpv_extra_args.split_whitespace().map(|s| s.to_string())); }
                                args.push("<URL>".into());
                                args.join(" ")
                            } else {
                                crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Default, &draft)
                            };
                        // Only update when value actually changes to avoid unnecessary repaints
                            if self.command_preview != preview {
                                self.command_preview = preview;
                            }
                            ui.horizontal(|ui| {
                                let mut use_mpv = draft.use_mpv;
                                if ui.checkbox(&mut use_mpv, "Use MPV instead of VLC").on_hover_text(if self.has_mpv { "enable mpv" } else { "mpv not found" }).changed() { draft.use_mpv = use_mpv; if use_mpv { draft.reuse_vlc = false; } }
                                if self.has_mpv { if let Some(v)=&self.mpv_version { ui.label(egui::RichText::new(format!("mpv: {}", v)).small()); }} else { ui.label(egui::RichText::new("mpv: not found").small()); }
                                if self.has_vlc { if let Some(v)=&self.vlc_version { ui.label(egui::RichText::new(format!("vlc: {}", v)).small()); }} else { ui.label(egui::RichText::new("vlc: not found").small()); }
                                let mut low = draft.low_cpu_mode;
                                if ui.checkbox(&mut low, "Low CPU Mode").on_hover_text("Reduces repaints and throttles diagnostics thread").changed() { draft.low_cpu_mode = low; }
                                let mut ultra = draft.ultra_low_flicker_mode;
                                if ui.checkbox(&mut ultra, "Ultra Flicker Guard").on_hover_text("Even fewer repaints (event-based only) – may increase UI latency").changed() { draft.ultra_low_flicker_mode = ultra; }
                            });

                            // MPV Abschnitt
                            // (MPV Optionen moved inside Player collapsing)
                            // (Preview moved inside Player collapsing)

                            // VLC Abschnitt ausgegraut wenn MPV aktiv
                            ui.add_enabled_ui(!draft.use_mpv, |ui| {
                                ui.collapsing("VLC Optimization & Diagnostics", |ui| {
                                    ui.horizontal(|ui| {
                                        let mut verbose = draft.vlc_verbose;
                                        if ui.checkbox(&mut verbose, "Verbose (-vvv)").changed() { draft.vlc_verbose = verbose; }
                                        let mut diag_once = draft.vlc_diagnose_on_start;
                                        if ui.checkbox(&mut diag_once, "Diagnose once").changed() { draft.vlc_diagnose_on_start = diag_once; }
                                        let mut cont_diag = draft.vlc_continuous_diagnostics;
                                        if ui.checkbox(&mut cont_diag, "Continuous").changed() { draft.vlc_continuous_diagnostics = cont_diag; }
                                        if draft.vlc_continuous_diagnostics {
                                            if ui.button("Stop").on_hover_text("Stop the running continuous VLC diagnostics").clicked() {
                                                let _ = self.tx.send(Msg::StopDiagnostics);
                                            }
                                        }
                                    });
                                    if let Some(suggestion) = self.vlc_diag_suggestion {
                                        ui.horizontal(|ui| {
                                            ui.label(format!("Suggestion: net={} live={} file={}", suggestion.0, suggestion.1, suggestion.2));
                                            if ui.button("Apply").on_hover_text("Apply values and save to history (max 10)").clicked() {
                                                draft.vlc_network_caching_ms = suggestion.0;
                                                draft.vlc_live_caching_ms = suggestion.1;
                                                draft.vlc_file_caching_ms = suggestion.2;
                                                draft.player_command = crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Default, &draft);
                                                let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                                                let entry = format!("{}:{}:{}:{}", ts, suggestion.0, suggestion.1, suggestion.2);
                                                let mut parts: Vec<String> = draft.vlc_diag_history.split(';').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
                                                parts.push(entry);
                                                if parts.len() > 10 { let overflow = parts.len() - 10; parts.drain(0..overflow); }
                                                draft.vlc_diag_history = parts.join(";");
                                            }
                                        });
                                    }
                                    if !draft.vlc_diag_history.trim().is_empty() {
                                        ui.collapsing("Suggestions History", |ui| {
                                            for seg in draft.vlc_diag_history.split(';').filter(|s| !s.is_empty()).rev() {
                                                let cols: Vec<&str> = seg.split(':').collect();
                                                if cols.len()==4 {
                                                    ui.label(format!("ts={} net={} live={} file={}", cols[0], cols[1], cols[2], cols[3]));
                                                }
                                            }
                                        });
                                    }
                                    ui.collapsing("VLC Diagnostic Logs", |ui| {
                                        let text = self.vlc_diag_lines.iter().rev().take(40).cloned().collect::<Vec<_>>().join("\n");
                                        ui.add(egui::TextEdit::multiline(&mut text.clone()).desired_rows(8));
                                    });
                                });
                            });

                            ui.collapsing("🖥 Appearance & UI", |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("UI Language");
                                    egui::ComboBox::from_id_source("language_selector")
                                        .selected_text(draft.language.name())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut draft.language, Language::English, Language::English.name());
                                            ui.selectable_value(&mut draft.language, Language::German, Language::German.name());
                                        });
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Text size scale");
                                    let mut fs = if draft.font_scale == 0.0 { 1.15 } else { draft.font_scale };
                                    if ui.add(egui::Slider::new(&mut fs, 0.6..=2.0).step_by(0.05)).on_hover_text("Scale factor for all text in the interface").changed() {
                                        draft.font_scale = fs;
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Cover height");
                                    let mut ch = if draft.cover_height == 0.0 { 60.0 } else { draft.cover_height };
                                    if ui.add(egui::Slider::new(&mut ch, 40.0..=120.0).step_by(2.0)).on_hover_text("Height of cover images in the content view").changed() {
                                        draft.cover_height = ch;
                                    }
                                });
                            });

                            ui.collapsing("⚡ Performance & Cache", |ui| {
                                ui.label("VLC preset commands:");
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("IPTV Optimized").on_hover_text("Apply VLC parameters optimized for IPTV/Xtream Codes streaming").clicked() {
                                        draft.player_command = crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Default, &draft);
                                    }
                                    if ui.button("Live TV").on_hover_text("Minimal buffering for live TV channels").clicked() {
                                        draft.player_command = crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Live, &draft);
                                    }
                                    if ui.button("VOD/Movies").on_hover_text("Larger buffer for better quality VOD playback").clicked() {
                                        draft.player_command = crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Vod, &draft);
                                    }
                                    if ui.button("Minimal").on_hover_text("Minimal VLC parameters for maximum compatibility").clicked() {
                                        draft.player_command = "vlc --fullscreen {URL}".to_string();
                                    }
                                });
                                let preview_cmd = if draft.player_command.trim().is_empty() {
                                    crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Default, &draft)
                                } else {
                                    draft.player_command.clone()
                                };
                                ui.label(egui::RichText::new(format!("Current: {}", preview_cmd)).weak());
                                ui.separator();
                                ui.label("VLC buffer settings:");
                                ui.horizontal(|ui| {
                                    ui.label("Network caching (ms)");
                                    let mut network = if draft.vlc_network_caching_ms == 0 { 10000 } else { draft.vlc_network_caching_ms } as i32;
                                    if ui.add(egui::DragValue::new(&mut network).clamp_range(1000..=60000)).on_hover_text("10s default for live TV stability").changed() {
                                        draft.vlc_network_caching_ms = network as u32;
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Live caching (ms)");
                                    let mut live = if draft.vlc_live_caching_ms == 0 { 5000 } else { draft.vlc_live_caching_ms } as i32;
                                    if ui.add(egui::DragValue::new(&mut live).clamp_range(0..=30000)).on_hover_text("5s default").changed() {
                                        draft.vlc_live_caching_ms = live as u32;
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Prefetch buffer (bytes)");
                                    let mut prefetch = if draft.vlc_prefetch_buffer_bytes == 0 { 16 * 1024 * 1024 } else { draft.vlc_prefetch_buffer_bytes } as i64;
                                    if ui.add(egui::DragValue::new(&mut prefetch).clamp_range(1024..=128 * 1024 * 1024)).on_hover_text("16 MiB default").changed() {
                                        draft.vlc_prefetch_buffer_bytes = prefetch as u64;
                                    }
                                });
                                ui.separator();
                                ui.label("Cover & texture:");
                                ui.horizontal(|ui| {
                                    ui.label("Cover TTL (days)");
                                    let mut ttl = if draft.cover_ttl_days == 0 { 7 } else { draft.cover_ttl_days } as i32;
                                    if ui.add(egui::DragValue::new(&mut ttl).clamp_range(1..=30)).changed() { draft.cover_ttl_days = ttl as u32; }
                                    ui.label("Cover parallelism");
                                    let mut par = if draft.cover_parallel == 0 { 6 } else { draft.cover_parallel } as i32;
                                    if ui.add(egui::DragValue::new(&mut par).clamp_range(1..=16)).changed() { draft.cover_parallel = par as u32; }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Uploads/frame");
                                    let mut upf = if draft.cover_uploads_per_frame == 0 { 3 } else { draft.cover_uploads_per_frame } as i32;
                                    if ui.add(egui::DragValue::new(&mut upf).clamp_range(1..=16)).changed() { draft.cover_uploads_per_frame = upf as u32; }
                                    ui.label("Decode parallelism");
                                    let mut dp = if draft.cover_decode_parallel == 0 { 2 } else { draft.cover_decode_parallel } as i32;
                                    if ui.add(egui::DragValue::new(&mut dp).clamp_range(1..=8)).changed() { draft.cover_decode_parallel = dp as u32; }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Texture cache limit");
                                    let mut tl = if draft.texture_cache_limit == 0 { 512 } else { draft.texture_cache_limit } as i32;
                                    if ui.add(egui::DragValue::new(&mut tl).clamp_range(64..=4096)).changed() { draft.texture_cache_limit = tl as u32; }
                                    ui.label("Category parallelism");
                                    let mut cp = if draft.category_parallel == 0 { 6 } else { draft.category_parallel } as i32;
                                    if ui.add(egui::DragValue::new(&mut cp).clamp_range(1..=20)).on_hover_text("Parallel category requests during loading").changed() { draft.category_parallel = cp as u32; }
                                });
                            });
                    
                    ui.collapsing("🧠 AI Recommendations", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("AI Provider:");
                        ui.radio_value(&mut draft.ai_provider, "wisdom-gate".to_string(), "🤖 Wisdom-Gate");
                        ui.radio_value(&mut draft.ai_provider, "perplexity".to_string(), "🔮 Perplexity");
                        ui.radio_value(&mut draft.ai_provider, "cognora".to_string(), "🧠 Cognora");
                        ui.radio_value(&mut draft.ai_provider, "gemini".to_string(), "💎 Gemini");
                        ui.radio_value(&mut draft.ai_provider, "openai".to_string(), "🤖 OpenAI");
                    });
                    
                    ui.separator();
                    
                    if draft.ai_provider == "wisdom-gate" {
                        ui.label("🤖 Wisdom-Gate AI");
                        ui.horizontal(|ui| {
                            ui.label("API Key:");
                            ui.add(
                                egui::TextEdit::singleline(&mut draft.wisdom_gate_api_key)
                                    .password(true)
                                    .hint_text("sk-xxx...")
                            );
                        });
                        if draft.wisdom_gate_api_key.trim().is_empty() {
                            ui.weak("API key required for AI recommendations");
                        }
                        
                        ui.horizontal(|ui| {
                            ui.label("Model:");
                            egui::ComboBox::from_label("")
                                .selected_text(&draft.wisdom_gate_model)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut draft.wisdom_gate_model, "gpt-3.5-turbo".to_string(), "🥇 GPT-3.5 Turbo (Recommended)");
                                    ui.selectable_value(&mut draft.wisdom_gate_model, "gpt-4".to_string(), "🧠 GPT-4 (Premium)");
                                    ui.selectable_value(&mut draft.wisdom_gate_model, "claude-3-sonnet".to_string(), "🎭 Claude 3 Sonnet");
                                    ui.selectable_value(&mut draft.wisdom_gate_model, "gemini-pro".to_string(), "💎 Gemini Pro");
                                    ui.selectable_value(&mut draft.wisdom_gate_model, "llama-2-70b-chat".to_string(), "🦙 Llama 2 70B Chat");
                                    ui.selectable_value(&mut draft.wisdom_gate_model, "mistral-7b-instruct".to_string(), "⚡ Mistral 7B (Schnell)");

                                    ui.separator();
                                    ui.label("Endpoint:");
                                    ui.text_edit_singleline(&mut draft.wisdom_gate_endpoint);
                                    ui.separator();
                                    ui.label("⚠️ Legacy models (may not be available):");
                                    ui.selectable_value(&mut draft.wisdom_gate_model, "wisdom-ai-dsv3".to_string(), "❌ Wisdom-AI DSV3 (deprecated)");
                                    ui.selectable_value(&mut draft.wisdom_gate_model, "deepseek-v3".to_string(), "❌ DeepSeek V3 (deprecated)");
                                });
                        });
                        
                        ui.label("Prompt for recommendations:");
                        ui.add(
                            egui::TextEdit::multiline(&mut draft.wisdom_gate_prompt)
                                .desired_rows(3)
                                .hint_text("What are the best streaming recommendations for today?")
                        );
                        
                        ui.horizontal(|ui| {
                            if ui.button("Default Prompt").clicked() {
                                draft.wisdom_gate_prompt = crate::models::default_wisdom_gate_prompt();
                            }
                            ui.weak("Tip: Ask for current movies and series");
                        });
                    } else if draft.ai_provider == "perplexity" {
                        ui.label("🔮 Perplexity AI");
                        ui.horizontal(|ui| {
                            ui.label("API Key:");
                            ui.add(
                                egui::TextEdit::singleline(&mut draft.perplexity_api_key)
                                    .password(true)
                                    .hint_text("pplx-xxx...")
                            );
                        });
                        if draft.perplexity_api_key.trim().is_empty() {
                            ui.weak("API key required for Perplexity recommendations");
                        }
                        
                        ui.horizontal(|ui| {
                            ui.label("Model:");
                            egui::ComboBox::from_label("")
                                .selected_text(&draft.perplexity_model)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut draft.perplexity_model, "sonar".to_string(), "🦙 Sonar (Recommended)");
                                    ui.selectable_value(&mut draft.perplexity_model, "sonar-pro".to_string(), "💎 Sonar Pro (Premium)");
                                });
                        });
                        
                        ui.label("Prompt for recommendations:");
                        ui.add(
                            egui::TextEdit::multiline(&mut draft.wisdom_gate_prompt)
                                .desired_rows(3)
                                .hint_text("What are the best streaming recommendations for today?")  // perplexity
                        );
                        
                        ui.horizontal(|ui| {
                            if ui.button("Default Prompt").clicked() {
                                draft.wisdom_gate_prompt = crate::models::default_wisdom_gate_prompt();
                            }
                            ui.weak("Tip: Perplexity has access to current information");
                        });
                    } else if draft.ai_provider == "cognora" {
                        ui.label("🧠 Cognora Toolkit");
                        ui.horizontal(|ui| {
                            ui.label("API Key:");
                            ui.add(
                                egui::TextEdit::singleline(&mut draft.cognora_api_key)
                                    .password(true)
                                    .hint_text("cog-xxx...")
                            );
                        });
                        if draft.cognora_api_key.trim().is_empty() {
                            ui.weak("API key required for Cognora recommendations");
                        }
                        
                        ui.horizontal(|ui| {
                            ui.label("Model:");
                            egui::ComboBox::from_label("")
                                .selected_text(&draft.cognora_model)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut draft.cognora_model, "cognora-3".to_string(), "🧠 Cognora-3 (Recommended)");
                                    ui.selectable_value(&mut draft.cognora_model, "cognora-4".to_string(), "💎 Cognora-4 (Premium)");
                                });
                        });
                        
                        ui.label("Prompt for recommendations:");
                        ui.add(
                            egui::TextEdit::multiline(&mut draft.wisdom_gate_prompt)
                                .desired_rows(3)
                                .hint_text("What are the best streaming recommendations for today?")
                        );
                        
                        ui.horizontal(|ui| {
                            if ui.button("Default Prompt").clicked() {
                                draft.wisdom_gate_prompt = crate::models::default_wisdom_gate_prompt();
                            }
                            ui.weak("Tip: Cognora offers specialized recommendations");
                        });
                    } else if draft.ai_provider == "gemini" {
                        ui.label("💎 Google Gemini");
                        ui.horizontal(|ui| {
                            ui.label("API Key:");
                            ui.add(
                                egui::TextEdit::singleline(&mut draft.gemini_api_key)
                                    .password(true)
                                    .hint_text("AIza...")
                            );
                        });
                        if draft.gemini_api_key.trim().is_empty() {
                            ui.weak("API key required for Gemini recommendations");
                        }
                        
                        ui.horizontal(|ui| {
                            ui.label("Model:");
                            egui::ComboBox::from_label("")
                                .selected_text(&draft.gemini_model)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut draft.gemini_model, "gemini-2.0-flash-exp".to_string(), "⚡ Gemini 2.0 Flash (Recommended)");
                                    ui.selectable_value(&mut draft.gemini_model, "gemini-1.5-pro".to_string(), "🧠 Gemini 1.5 Pro");
                                    ui.selectable_value(&mut draft.gemini_model, "gemini-1.5-flash".to_string(), "💨 Gemini 1.5 Flash");
                                });
                        });
                        
                        ui.label("Prompt for recommendations:");
                        ui.add(
                            egui::TextEdit::multiline(&mut draft.wisdom_gate_prompt)
                                .desired_rows(3)
                                .hint_text("What are the best streaming recommendations for today?")
                        );
                        
                        ui.horizontal(|ui| {
                            if ui.button("Default Prompt").clicked() {
                                draft.wisdom_gate_prompt = crate::models::default_wisdom_gate_prompt();
                            }
                            ui.weak("Tip: Gemini offers multimodal capabilities");
                        });
                    } else if draft.ai_provider == "openai" {
                        ui.label("🤖 OpenAI");
                        ui.horizontal(|ui| {
                            ui.label("API Key:");
                            ui.add(
                                egui::TextEdit::singleline(&mut draft.openai_api_key)
                                    .password(true)
                                    .hint_text("sk-...")
                            );
                        });
                        if draft.openai_api_key.trim().is_empty() {
                            ui.weak("API key required for OpenAI recommendations");
                        }
                        
                        ui.horizontal(|ui| {
                            ui.label("Model:");
                            egui::ComboBox::from_label("")
                                .selected_text(&draft.openai_model)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut draft.openai_model, "gpt-4o".to_string(), "🚀 GPT-4o (Recommended)");
                                    ui.selectable_value(&mut draft.openai_model, "gpt-4o-mini".to_string(), "⚡ GPT-4o Mini");
                                    ui.selectable_value(&mut draft.openai_model, "gpt-4-turbo".to_string(), "🧠 GPT-4 Turbo");
                                    ui.selectable_value(&mut draft.openai_model, "gpt-3.5-turbo".to_string(), "💨 GPT-3.5 Turbo");
                                });
                        });
                        
                        ui.label("Prompt for recommendations:");
                        ui.add(
                            egui::TextEdit::multiline(&mut draft.wisdom_gate_prompt)
                                .desired_rows(3)
                                .hint_text("What are the best streaming recommendations for today?")
                        );
                        
                        ui.horizontal(|ui| {
                            if ui.button("Default Prompt").clicked() {
                                draft.wisdom_gate_prompt = crate::models::default_wisdom_gate_prompt();
                            }
                            ui.weak("Tip: OpenAI offers powerful models");
                        });
                    }
                    });
                    
                    ui.add_space(8.0);
                    ui.heading(&t("update_settings", self.config.language));
                    ui.separator();
                    ui.checkbox(&mut draft.check_for_updates, &t("auto_check_updates", self.config.language))
                        .on_hover_text(&t("auto_check_tooltip", self.config.language));
                    
                    ui.horizontal(|ui| {
                        if ui.button(&t("check_now", self.config.language)).clicked() && !self.checking_for_updates {
                            // Close config dialog and check for updates
                            if let Some(d) = &self.config_draft {
                                self.config = d.clone();
                                self.pending_save_config = true;
                            }
                            self.config_draft = None;
                            self.show_config = false;
                            self.check_for_updates();
                        }
                        
                        if self.checking_for_updates {
                            ui.spinner();
                            ui.label("Checking...");
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            if let Some(d) = &self.config_draft {
                                self.config = d.clone();
                            }
                            // Persist theme setting
                            self.config.theme = if self.current_theme.is_empty() {
                                "dark".into()
                            } else {
                                self.current_theme.clone()
                            };
                            self.pending_save_config = true;
                        }
                        if ui.button("❌ Cancel").clicked() {
                            cancel_clicked = true;
                        }
                    });
                        });
                });

            // Handle window close  
            if cancel_clicked || !open {
                self.config_draft = None;
                self.show_config = false;
            } else {
                self.show_config = open;
            }
        }

        // Update Available Dialog
        if self.available_update.is_some() && !self.update_downloading && !self.update_installing {
            let update_info = self.available_update.clone().unwrap();
            let mut close_update = false;
            let mut start_download = false;
            egui::Window::new("🎉 Update Available")
                .collapsible(false)
                .resizable(false)
                .default_width(480.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(format!("Version {} is available!", update_info.latest_version)).strong().size(16.0));
                        ui.add_space(4.0);
                        ui.label(format!("You are running v{}", env!("CARGO_PKG_VERSION")));
                        ui.add_space(8.0);
                    });
                    if !update_info.release_notes.is_empty() {
                        ui.collapsing("Release Notes", |ui| {
                            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                let notes = if update_info.release_notes.len() > 2000 {
                                    format!("{}\n...(truncated)", &update_info.release_notes[..2000])
                                } else {
                                    update_info.release_notes.clone()
                                };
                                ui.label(&notes);
                            });
                        });
                        ui.add_space(8.0);
                    }
                    if update_info.download_url.is_some() {
                        ui.horizontal(|ui| {
                            if ui.button(egui::RichText::new("⬇ Install Update").strong()).on_hover_text("Downloads the DMG, mounts it and installs the app to /Applications").clicked() {
                                start_download = true;
                            }
                            if ui.button("Later").clicked() {
                                close_update = true;
                            }
                        });
                        ui.add_space(4.0);
                        ui.weak("The app will restart automatically after installation.");
                    } else {
                        ui.colored_label(egui::Color32::YELLOW, "⚠️ No DMG asset found in this release.");
                        ui.horizontal(|ui| {
                            if ui.hyperlink_to("Open Releases page", "https://github.com/modze996/macxtreamer/releases").clicked() {}
                            if ui.button("Close").clicked() { close_update = true; }
                        });
                    }
                });
            if start_download {
                self.start_update_download(update_info);
                self.available_update = None;
            } else if close_update {
                self.available_update = None;
            }
        }

        // Update Progress Dialog
        if self.update_downloading || self.update_installing {
            egui::Window::new("⬇ Installing Update")
                .collapsible(false)
                .resizable(false)
                .default_width(360.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.spinner();
                        ui.add_space(6.0);
                        ui.label(&self.update_progress);
                        ui.add_space(4.0);
                        ui.weak("Please wait, do not close the app...");
                    });
                });
        }

        // Server Profile Manager Window
        if self.show_server_manager {
            let mut open = self.show_server_manager;
            egui::Window::new("🌐 Server Profile Manager")
                .collapsible(false)
                .resizable(true)
                .default_width(500.0)
                .default_height(400.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .open(&mut open)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            let draft = self.config_draft.get_or_insert_with(|| self.config.clone());
                            
                            ui.heading("Server Profiles");
                            ui.separator();
                            
                            // List all profiles
                            let mut profile_to_delete: Option<usize> = None;
                            let mut profile_to_switch: Option<usize> = None;
                            
                            for (i, profile) in draft.server_profiles.iter().enumerate() {
                                let is_active = i == draft.active_profile_index;
                                ui.horizontal(|ui| {
                                    let badge = if is_active { "✓" } else { "" };
                                    let color = if is_active { 
                                        egui::Color32::from_rgb(100, 200, 100)
                                    } else {
                                        ui.visuals().text_color()
                                    };
                                    
                                    ui.colored_label(color, format!("{} {}", badge, profile.name));
                                    ui.label(egui::RichText::new(&profile.address).small().weak());
                                    
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if !is_active && ui.small_button("❌").on_hover_text("Delete profile").clicked() {
                                            profile_to_delete = Some(i);
                                        }
                                        if !is_active && ui.small_button("✓").on_hover_text("Switch to this profile").clicked() {
                                            profile_to_switch = Some(i);
                                        }
                                    });
                                });
                                ui.separator();
                            }
                            
                            // Handle profile actions
                            if let Some(idx) = profile_to_delete {
                                if draft.server_profiles.len() > 1 {
                                    draft.server_profiles.remove(idx);
                                    // Adjust active index if needed
                                    if draft.active_profile_index >= draft.server_profiles.len() {
                                        draft.active_profile_index = draft.server_profiles.len().saturating_sub(1);
                                    }
                                }
                            }
                            
                            if let Some(idx) = profile_to_switch {
                                draft.active_profile_index = idx;
                                self.show_server_manager = false;
                                // Auto-save to make switch immediate
                                self.config = draft.clone();
                                self.pending_save_config = true;
                            }
                            
                            ui.add_space(8.0);
                            
                            // Add new profile section
                            ui.heading("Add New Profile");
                            ui.separator();
                            
                            // Clone new_profile_name to avoid borrow issues
                            let mut profile_name = self.new_profile_name.clone();
                            
                            ui.horizontal(|ui| {
                                ui.label("Name:");
                                ui.add(egui::TextEdit::singleline(&mut profile_name)
                                    .hint_text("e.g., Main Server, Test Server")
                                    .desired_width(200.0));
                            });
                            
                            // Update the original if changed
                            if profile_name != self.new_profile_name {
                                self.new_profile_name = profile_name.clone();
                            }
                            
                            let mut should_cancel = false;
                            
                            ui.horizontal(|ui| {
                                if ui.button("➕ Add Profile").clicked() {
                                    let name = if profile_name.trim().is_empty() {
                                        format!("Server {}", draft.server_profiles.len() + 1)
                                    } else {
                                        profile_name.trim().to_string()
                                    };
                                    
                                    draft.server_profiles.push(crate::models::ServerProfile {
                                        name,
                                        address: String::new(),
                                        username: String::new(),
                                        password: String::new(),
                                    });
                                    
                                    self.new_profile_name.clear();
                                }
                                
                                if ui.button("Revert changes and Cancel").clicked() {
                                    should_cancel = true;
                                }
                            });
                            
                            if should_cancel {
                                self.config_draft = None;
                                self.show_server_manager = false;
                            }
                        });
                });
            
            self.show_server_manager = open;
        }

        // Error dialog window
        if self.show_error_dialog {
            let mut open = self.show_error_dialog;
            egui::Window::new("❌ Fehler beim Laden")
                .collapsible(false)
                .resizable(true)
                .default_width(500.0)
                .default_height(250.0)
                .max_width(800.0)
                .max_height(600.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .open(&mut open)
                .show(ctx, |ui| {
                    // Show message if not empty, otherwise show placeholder
                    if !self.loading_error.is_empty() {
                        egui::ScrollArea::vertical()
                            .max_height(400.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.style_mut().wrap = Some(true);
                                ui.label(&self.loading_error);
                            });
                    } else {
                        ui.label("Ein unbekannter Fehler ist aufgetreten.");
                    }
                    
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    
                    ui.horizontal(|ui| {
                        if !self.loading_error.is_empty() && ui.button("📋 Kopieren").clicked() {
                            ui.output_mut(|o| o.copied_text = self.loading_error.clone());
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("OK").clicked() {
                                self.show_error_dialog = false;
                            }
                        });
                    });
                });
            self.show_error_dialog = open;
        }

        // (Bottom panel already rendered above CentralPanel)

        // Handle deferred save to avoid mutable borrow inside Window closure
        if self.pending_save_config {
            // Check if server changed before saving
            let old_address = self.config.address.clone();
            let old_username = self.config.username.clone();
            
            // Sync active profile to legacy fields before saving
            self.config.sync_active_profile();
            
            let server_changed = old_address != self.config.address || old_username != self.config.username;
            
            let _ = save_config(&self.config);
            // Übernehme neue Parallelität sofort
            let permits = if self.config.cover_parallel == 0 {
                6
            } else {
                self.config.cover_parallel
            } as usize;
            self.cover_sem = Arc::new(Semaphore::new(permits));
            // Apply decode parallelism immediately
            let dpermits = if self.config.cover_decode_parallel == 0 {
                2
            } else {
                self.config.cover_decode_parallel
            } as usize;
            self.decode_sem = Arc::new(Semaphore::new(dpermits));
            // Apply cover height and font scale immediately
            self.cover_height = if self.config.cover_height == 0.0 {
                60.0
            } else {
                self.config.cover_height
            };
            self.font_scale_applied = false; // Force font scale reapplication
            if self.config_is_complete() {
                // Only start loading now if config became complete
                self.reload_categories();
                if self.initial_config_pending {
                    self.spawn_preload_all();
                    self.initial_config_pending = false;
                }
                // Rebuild index if server changed
                if server_changed {
                    println!("🔄 Server profile changed, rebuilding search index...");
                    self.all_movies.clear();
                    self.all_series.clear();
                    self.all_channels.clear();
                    self.index_paths.clear();
                    self.spawn_build_index();
                }
            }
            self.show_config = false;
            self.pending_save_config = false;
            self.config_draft = None;
        }

        // Confirmation window for bulk series download
        if let Some((series_id, series_name)) = self.confirm_bulk.clone() {
            let mut open = true;
            egui::Window::new("Download all episodes")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(format!("Queue all episodes of ‘{}’?", series_name));
                    let mut opts = self
                        .bulk_options_by_series
                        .get(&series_id)
                        .cloned()
                        .unwrap_or(self.bulk_opts_draft.clone());
                    ui.checkbox(&mut opts.only_not_downloaded, "Only not yet downloaded");
                    ui.horizontal(|ui| {
                        ui.label("Season (optional)");
                        let mut s = opts.season.unwrap_or(0) as i32;
                        if ui
                            .add(egui::DragValue::new(&mut s).clamp_range(0..=99))
                            .changed()
                        {
                            opts.season = if s <= 0 { None } else { Some(s as u32) };
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Max episodes (0=all)");
                        let mut m = opts.max_count as i32;
                        if ui
                            .add(egui::DragValue::new(&mut m).clamp_range(0..=2000))
                            .changed()
                        {
                            opts.max_count = m.max(0) as u32;
                        }
                    });
                    self.bulk_options_by_series
                        .insert(series_id.clone(), opts.clone());
                    ui.horizontal(|ui| {
                        if ui.button("Yes, download").clicked() {
                            // Fetch episodes and enqueue with current options
                            self.spawn_fetch_episodes_for_download(series_id.clone());
                            self.confirm_bulk = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm_bulk = None;
                        }
                    });
                });
            if !open {
                self.confirm_bulk = None;
            }
        }

        // Ausstehende Player Neu-Erkennung nach UI Aktionen durchführen
        if self.pending_player_redetect {
            self.perform_player_detection();
            self.pending_player_redetect = false;
        }
        
        // Render toasts
        self.render_toasts(ctx);

        // Process any pending bulk downloads enqueued by messages to avoid borrow conflicts
        if !self.pending_bulk_downloads.is_empty() {
            let jobs: Vec<(String, String, String, Option<String>)> =
                std::mem::take(&mut self.pending_bulk_downloads);
            for (id, name, info, ext) in jobs {
                self.spawn_download_bulk(id, name, info, ext);
            }
        }
    }


}

fn extract_year_from_title(title: &str) -> Option<String> {
    // Simple pattern matching to extract 4-digit year from title like "(2023)" or "[2023]"
    if let Some(start) = title.find('(') {
        if let Some(end) = title[start..].find(')') {
            let year_part = &title[start + 1..start + end];
            if year_part.len() == 4 && year_part.chars().all(|c| c.is_ascii_digit()) {
                return Some(year_part.to_string());
            }
        }
    }
    if let Some(start) = title.find('[') {
        if let Some(end) = title[start..].find(']') {
            let year_part = &title[start + 1..start + end];
            if year_part.len() == 4 && year_part.chars().all(|c| c.is_ascii_digit()) {
                return Some(year_part.to_string());
            }
        }
    }
    None
}

// Die Hilfsfunktion column_config_from_csv bleibt als freie Funktion.

fn column_config_to_csv(cols: &[ColumnKey]) -> String {
    cols.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(",")
}

impl MacXtreamer {
    pub fn save_column_config(&mut self) {
        self.config.column_config_serialized = Some(column_config_to_csv(&self.column_config));
        let _ = save_config(&self.config);
    }
}
