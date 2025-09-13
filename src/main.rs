use eframe::egui::{self, Color32, RichText};
use image::GenericImageView;
use egui_extras::TableBuilder;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::collections::{HashMap, HashSet as HashSet2};
// std::fs is used via fully-qualified paths where needed; avoid importing the module.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::io::AsyncReadExt;

mod api;
mod cache;
mod config;
mod icon;
mod logger;
mod models;
mod player;
mod search;
mod storage;

use api::{fetch_categories, fetch_items, fetch_series_episodes};
use cache::{clear_all_caches, file_age_secs, image_cache_path};
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use config::{read_config, save_config};
use models::{Category, Config, Episode, FavItem, Item, RecentItem, Row};
// use crate::logger::log_line; // re-enabled later when spawning actual download
use player::{build_url_by_type, start_player};
use search::search_items;
use storage::{add_to_recently, load_favorites, load_recently_played, toggle_favorite};

// Helper: path to sidecar metadata for an image cache file (stores ETag/Last-Modified)
fn image_meta_path(url: &str) -> Option<std::path::PathBuf> {
    image_cache_path(url).and_then(|p| {
        let fname = p.file_name()?.to_string_lossy().to_string();
        let mut meta = p.clone();
        meta.set_file_name(format!("{}.meta", fname));
        Some(meta)
    })
}

#[derive(Debug, Clone, Default)]
struct BulkOptions {
    only_not_downloaded: bool,
    season: Option<u32>,
    max_count: u32, // 0 = all
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Name,
    Year,
    Rating,
    Genre,
}

enum Msg {
    LiveCategories(Result<Vec<Category>, String>),
    VodCategories(Result<Vec<Category>, String>),
    SeriesCategories(Result<Vec<Category>, String>),
    ItemsLoaded {
        kind: String,
        items: Result<Vec<Item>, String>,
    },
    EpisodesLoaded {
        series_id: String,
        episodes: Result<Vec<Episode>, String>,
    },
    CoverLoaded {
        url: String,
        bytes: Vec<u8>,
    },
    CoverDecoded {
        url: String,
        rgba: Vec<u8>,
        w: u32,
        h: u32,
    },
    IndexBuilt {
        movies: usize,
        series: usize,
    },
    SearchReady(Vec<Row>),
    IndexData {
        movies: Vec<(Item, String)>,
        series: Vec<(Item, String)>,
    },
    PreloadSet {
        total: usize,
    },
    PreloadTick,
    PrefetchCovers(Vec<String>),
    SeriesEpisodesForDownload {
        series_id: String,
        episodes: Result<Vec<Episode>, String>,
    },
    DownloadStarted {
        id: String,
        path: String,
    },
    DownloadProgress {
        id: String,
        received: u64,
        total: Option<u64>,
    },
    DownloadFinished {
        id: String,
        path: String,
    },
    DownloadError {
        id: String,
        error: String,
    },
    DownloadCancelled {
        id: String,
    },
    DownloadsScanned(Vec<ScannedDownload>),
}

#[derive(Debug, Clone)]
enum ViewState {
    Items { kind: String, category_id: String },
    Episodes { series_id: String },
    Search { query: String },
}

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    let icon = icon::generate_icon(256);
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size(egui::vec2(1500.0, 1600.0))
        .with_icon(icon);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "MacXtreamer",
        options,
        Box::new(|_cc| Box::new(MacXtreamer::new())),
    )
}

struct MacXtreamer {
    // Config/State
    config: Config,
    config_draft: Option<Config>,
    playlists: Vec<Category>,
    vod_categories: Vec<Category>,
    series_categories: Vec<Category>,
    content_rows: Vec<Row>,
    all_movies: Vec<Item>,
    all_series: Vec<Item>,
    recently: Vec<RecentItem>,
    favorites: Vec<FavItem>,
    // UI assets
    textures: HashMap<String, egui::TextureHandle>,
    pending_covers: HashSet2<String>,
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
    is_loading: bool,
    loading_done: usize,
    loading_total: usize,
    last_error: Option<String>,
    show_config: bool,
    pending_save_config: bool,
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

    // Async messaging
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    show_log: bool,
    log_text: String,
    initial_config_pending: bool,
    downloads: HashMap<String, DownloadState>,
    download_order: Vec<String>,
    download_meta: HashMap<String, DownloadMeta>,
    show_downloads: bool,
    // Map item-id -> category path for displaying in search results
    index_paths: HashMap<String, String>,
    // UI: confirm bulk series download (series_id, series_name)
    confirm_bulk: Option<(String, String)>,
    bulk_opts_draft: BulkOptions,
    bulk_options_by_series: HashMap<String, BulkOptions>,
    // Defer actual enqueuing of downloads to avoid borrow conflicts inside message loop
    pending_bulk_downloads: Vec<(String, String, String, Option<String>)>,
    // Shared HTTP client for connection reuse
    http_client: reqwest::Client,
    // Zeitpunkt letzter Verzeichnis-Scan (optional Throttling)
    last_download_scan: Option<std::time::Instant>,
    // Navigation
    current_view: Option<ViewState>,
    view_stack: Vec<ViewState>,
}

#[derive(Debug, Clone)]
struct DownloadState {
    received: u64,
    total: Option<u64>,
    finished: bool,
    error: Option<String>,
    path: Option<String>,
    cancel_flag: Option<Arc<AtomicBool>>,
    waiting: bool,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            received: 0,
            total: None,
            finished: false,
            error: None,
            path: None,
            cancel_flag: None,
            waiting: false,
        }
    }
}

#[derive(Debug, Clone)]
struct DownloadMeta {
    id: String,
    name: String,
    info: String,
    container_extension: Option<String>,
    size: Option<u64>,
    modified: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone)]
struct ScannedDownload {
    id: String,
    name: String,
    info: String,
    container_extension: Option<String>,
    path: String,
    size: u64,
    modified: std::time::SystemTime,
}

impl MacXtreamer {
    fn config_is_complete(&self) -> bool {
        !(self.config.address.trim().is_empty()
            || self.config.username.trim().is_empty()
            || self.config.password.trim().is_empty())
    }
    fn create_and_play_m3u(&self, entries: &[(String, String)]) -> Result<(), String> {
        if entries.is_empty() {
            return Err("No episodes to play".into());
        }
        let mut path = std::env::temp_dir();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        path.push(format!("macxtreamer_binge_{}.m3u", ts));
        let mut buf = String::from("#EXTM3U\n");
        for (title, url) in entries {
            buf.push_str(&format!("#EXTINF:-1,{}\n{}\n", title, url));
        }
        std::fs::write(&path, buf).map_err(|e| format!("Failed to write playlist: {}", e))?;
        if let Some(p) = path.to_str() {
            let _ = start_player(&self.config, p);
            Ok(())
        } else {
            Err("Invalid playlist path".into())
        }
    }
    fn clear_caches_and_reload(&mut self) {
        // Clear on-disk caches (JSON + images)
        clear_all_caches();
        // Clear in-memory caches
        self.textures.clear();
        self.pending_covers.clear();
    self.pending_texture_uploads.clear();
    self.pending_texture_urls.clear();
    self.pending_decode_urls.clear();
        self.all_movies.clear();
    self.all_series.clear();
        self.content_rows.clear();
    self.index_paths.clear();
        // Reset loading state and kick off a fresh load
        self.is_loading = true;
        self.loading_done = 0;
        self.loading_total = 3;
        self.last_error = None;
        // Reload categories; when they arrive, items will be fetched from network
        self.reload_categories();
        // Optionally prime caches again in background
        self.spawn_preload_all();
    }
    fn new() -> Self {
        let read_result = read_config();
        let (config, had_file) = match read_result {
            Ok(c) => (c, true),
            Err(_) => (Config::default(), false),
        };
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            config,
            config_draft: None,
            playlists: vec![],
            vod_categories: vec![],
            series_categories: vec![],
            content_rows: vec![],
            all_movies: vec![],
            all_series: vec![],
            recently: load_recently_played(),
            favorites: load_favorites(),
            textures: HashMap::new(),
            pending_covers: HashSet2::new(),
            pending_texture_uploads: VecDeque::new(),
            pending_texture_urls: HashSet::new(),
            pending_decode_urls: HashSet::new(),
            decode_sem: Arc::new(Semaphore::new(2)),
            cover_sem: Arc::new(Semaphore::new(6)),
            cover_height: 60.0,
            search_text: String::new(),
            is_loading: false,
            loading_done: 0,
            loading_total: 0,
            last_error: None,
            show_config: false,
            pending_save_config: false,
            selected_playlist: None,
            selected_vod: None,
            selected_series: None,
            last_live_cat_id: None,
            last_vod_cat_id: None,
            last_series_cat_id: None,
            current_theme: "".into(),
            theme_applied: false,
            font_scale_applied: false,
            current_font_scale: 1.15, // Will be set from config below
            indexing: false,
            sort_key: None,
            sort_asc: true,
            tx,
            rx,
            show_log: false,
            log_text: String::new(),
            initial_config_pending: false,
            downloads: HashMap::new(),
            download_order: Vec::new(),
            download_meta: HashMap::new(),
            show_downloads: false,
            index_paths: HashMap::new(),
            confirm_bulk: None,
            bulk_opts_draft: BulkOptions { only_not_downloaded: true, season: None, max_count: 0 },
            bulk_options_by_series: HashMap::new(),
            pending_bulk_downloads: Vec::new(),
            http_client: reqwest::Client::builder()
                .pool_idle_timeout(Duration::from_secs(30))
                .pool_max_idle_per_host(8) // Allow more connections per host
                .tcp_nodelay(true)
                .tcp_keepalive(Some(Duration::from_secs(30)))
                .timeout(Duration::from_secs(15)) // Add reasonable timeout
                .connect_timeout(Duration::from_secs(5)) // Fast connection timeout
                .http2_prior_knowledge() // Use HTTP/2 if available
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            last_download_scan: None,
            current_view: None,
            view_stack: Vec::new(),
        };
        // Determine initial config readiness
        if !had_file || !app.config_is_complete() {
            app.show_config = true;
            app.initial_config_pending = true;
        }
        app.current_theme = if app.config.theme.is_empty() {
            "dark".into()
        } else {
            app.config.theme.clone()
        };
        if app.config.cover_ttl_days == 0 {
            app.config.cover_ttl_days = 7;
        }
        if app.config.cover_parallel == 0 {
            app.config.cover_parallel = 6;
        }
        if app.config.cover_uploads_per_frame == 0 {
            app.config.cover_uploads_per_frame = 3;
        }
        if app.config.cover_decode_parallel == 0 {
            app.config.cover_decode_parallel = 2;
        }
        if app.config.texture_cache_limit == 0 {
            app.config.texture_cache_limit = 512;
        }
        if app.config.font_scale == 0.0 {
            app.config.font_scale = 1.15;
        }
        app.current_font_scale = app.config.font_scale; // Initialize tracking variable
        if app.config.cover_height == 0.0 {
            app.config.cover_height = 60.0;
        }
        app.cover_height = app.config.cover_height;
        app.cover_sem = Arc::new(Semaphore::new(app.config.cover_parallel as usize));
        app.decode_sem = Arc::new(Semaphore::new(app.config.cover_decode_parallel as usize));
        // Only preload/load categories if config is complete
        if app.config_is_complete() {
            app.reload_categories();
            app.spawn_preload_all();
        }
        app
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

    fn spawn_load_items(&self, kind: &str, category_id: String) {
        if !self.config_is_complete() {
            return;
        }
        let cfg = self.config.clone();
        let tx = self.tx.clone();
        let kind_s = kind.to_string();
        tokio::spawn(async move {
            let res = fetch_items(&cfg, &kind_s, &category_id).await;
            let _ = tx.send(Msg::ItemsLoaded {
                kind: kind_s,
                items: res.map_err(|e| e.to_string()),
            });
        });
    }

    fn spawn_load_episodes(&self, series_id: String) {
        if !self.config_is_complete() {
            return;
        }
        let cfg = self.config.clone();
        let tx = self.tx.clone();
        let sid = series_id;
        tokio::spawn(async move {
            let res = fetch_series_episodes(&cfg, &sid).await;
            let _ = tx.send(Msg::EpisodesLoaded {
                series_id: sid,
                episodes: res.map_err(|e| e.to_string()),
            });
        });
    }

    fn spawn_fetch_episodes_for_download(&self, series_id: String) {
        if !self.config_is_complete() {
            return;
        }
        let cfg = self.config.clone();
        let tx = self.tx.clone();
        let sid = series_id;
        tokio::spawn(async move {
            let res = fetch_series_episodes(&cfg, &sid).await;
            let _ = tx.send(Msg::SeriesEpisodesForDownload {
                series_id: sid,
                episodes: res.map_err(|e| e.to_string()),
            });
        });
    }

    fn spawn_fetch_cover(&mut self, url: &str) {
        if self.pending_covers.contains(url) {
            return;
        }
        self.pending_covers.insert(url.to_string());
        let tx = self.tx.clone();
        let url_s = url.to_string();
        let sem = self.cover_sem.clone();
        let ttl_secs: u64 = (self.config.cover_ttl_days.max(1) as u64) * 24 * 60 * 60;
        let client = self.http_client.clone();
        tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.ok();
            // Versuche Disk-Cache mit TTL zuerst
            let mut served_any = false;
            let mut need_refresh = false;
            // Load cached meta (etag/last-modified) if any
            let (mut cached_etag, mut cached_lm) = (None::<String>, None::<String>);
            if let Some(mpath) = image_meta_path(&url_s) {
                if let Ok(mut f) = tokio::fs::File::open(&mpath).await {
                    let mut s = String::new();
                    let _ = tokio::io::AsyncReadExt::read_to_string(&mut f, &mut s).await;
                    for line in s.lines() {
                        if let Some(val) = line.strip_prefix("etag: ") {
                            cached_etag = Some(val.trim().to_string());
                        } else if let Some(val) = line.strip_prefix("last_modified: ") {
                            cached_lm = Some(val.trim().to_string());
                        }
                    }
                }
            }
            if let Some(path) = image_cache_path(&url_s) {
                if let Some(age) = file_age_secs(&path) {
                    if let Ok(mut f) = tokio::fs::File::open(&path).await {
                        let mut buf = Vec::new();
                        if f.read_to_end(&mut buf).await.is_ok() {
                            let _ = tx.send(Msg::CoverLoaded {
                                url: url_s.clone(),
                                bytes: buf,
                            });
                            served_any = true;
                            if age > ttl_secs {
                                need_refresh = true;
                            }
                        }
                    }
                }
            }
            if !served_any || need_refresh {
                let mut req = client.get(&url_s);
                if let Some(et) = cached_etag.as_deref() {
                    req = req.header(IF_NONE_MATCH, et);
                }
                if let Some(lm) = cached_lm.as_deref() {
                    req = req.header(IF_MODIFIED_SINCE, lm);
                }
                if let Ok(resp) = req.send().await {
                    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
                        // Cache ist aktuell: ggf. mtime auffrischen, keine Doppel-Lieferung nötig
                        if let Some(path) = image_cache_path(&url_s) {
                            if let Ok(mut f) = tokio::fs::File::open(&path).await {
                                let mut buf = Vec::new();
                                if tokio::io::AsyncReadExt::read_to_end(&mut f, &mut buf)
                                    .await
                                    .is_ok()
                                {
                                    let _ = tokio::fs::write(&path, &buf).await;
                                }
                            }
                        }
                        return;
                    }
                    // Capture ETag/Last-Modified before consuming body
                    let et_hdr = resp
                        .headers()
                        .get(ETAG)
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());
                    let lm_hdr = resp
                        .headers()
                        .get(LAST_MODIFIED)
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());
                    if let Ok(bytes) = resp.bytes().await {
                        let data = bytes.to_vec();
                        // Schreibe in Disk-Cache
                        if let Some(path) = image_cache_path(&url_s) {
                            if let Some(parent) = path.parent() {
                                let _ = tokio::fs::create_dir_all(parent).await;
                            }
                            let _ = tokio::fs::write(&path, &data).await;
                            // Write sidecar meta with ETag/Last-Modified
                            if let Some(mpath) = image_meta_path(&url_s) {
                                if let Some(parent) = mpath.parent() {
                                    let _ = tokio::fs::create_dir_all(parent).await;
                                }
                                let et = et_hdr.as_deref().unwrap_or("");
                                let lm = lm_hdr.as_deref().unwrap_or("");
                                let meta = format!("etag: {}\nlast_modified: {}\n", et, lm);
                                let _ = tokio::fs::write(&mpath, meta).await;
                            }
                        }
                        let _ = tx.send(Msg::CoverLoaded {
                            url: url_s.clone(),
                            bytes: data,
                        });
                        return;
                    }
                }
                // Netzwerk fehlgeschlagen: Fallback zu evtl. vorhandenem, aber stale Cache
                if let Some(path) = image_cache_path(&url_s) {
                    if let Ok(mut f) = tokio::fs::File::open(&path).await {
                        let mut buf = Vec::new();
                        if f.read_to_end(&mut buf).await.is_ok() {
                            let _ = tx.send(Msg::CoverLoaded {
                                url: url_s.clone(),
                                bytes: buf,
                            });
                        }
                    }
                }
            }
        });
    }

    fn spawn_build_index(&mut self) {
        if self.indexing {
            return;
        }
        if !self.config_is_complete() {
            return;
        }
        self.indexing = true;
        let tx = self.tx.clone();
        let cfg = self.config.clone();
        tokio::spawn(async move {
            // Fetch categories
            let vod = fetch_categories(&cfg, "get_vod_categories")
                .await
                .unwrap_or_default();
            let ser = fetch_categories(&cfg, "get_series_categories")
                .await
                .unwrap_or_default();
            let mut all_movies: Vec<(Item, String)> = Vec::new();
            let mut all_series: Vec<(Item, String)> = Vec::new();
            for c in vod {
                let path = format!("VOD / {}", c.name);
                if let Ok(items) = fetch_items(&cfg, "vod", &c.id).await {
                    all_movies.extend(items.into_iter().map(|it| (it, path.clone())));
                }
            }
            for c in ser {
                let path = format!("Series / {}", c.name);
                if let Ok(items) = fetch_items(&cfg, "series", &c.id).await {
                    all_series.extend(items.into_iter().map(|it| (it, path.clone())));
                }
            }
            // Dedup by id
            let mut seen = std::collections::HashSet::new();
            all_movies.retain(|(i, _)| seen.insert(i.id.clone()));
            seen.clear();
            all_series.retain(|(i, _)| seen.insert(i.id.clone()));
            // Persist into cache files already handled by fetch_items; send data back
            let _ = tx.send(Msg::IndexBuilt {
                movies: all_movies.len(),
                series: all_series.len(),
            });
            let _ = tx.send(Msg::IndexData { movies: all_movies, series: all_series });
        });
    }

    fn start_search(&mut self) {
        let tx = self.tx.clone();
        let movies = self.all_movies.clone();
        let series = self.all_series.clone();
        let query = self.search_text.clone();
        if movies.is_empty() && series.is_empty() && !self.indexing {
            self.spawn_build_index();
        }
        self.is_loading = true;
        self.loading_total = 1;
        self.loading_done = 0;
        tokio::spawn(async move {
            let results = search_items(&movies, &series, &query);
            let rows: Vec<Row> = results
                .into_iter()
                .map(|s| Row {
                    name: s.name,
                    id: s.id,
                    info: s.info,
                    container_extension: if s.container_extension.is_empty() {
                        None
                    } else {
                        Some(s.container_extension)
                    },
                    stream_url: None,
                    cover_url: s.cover,
                    year: s.year,
                    rating_5based: s.rating_5based,
                    genre: s.genre,
                    path: None,
                })
                .collect();
            let _ = tx.send(Msg::SearchReady(rows));
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
            let category_parallel = if cfg.category_parallel == 0 { 6 } else { cfg.category_parallel } as usize;
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
        let raw = self.config.download_dir.trim();
        let default_dir = || {
            if let Some(ud) = directories::UserDirs::new() {
                if let Some(dl) = ud.download_dir() {
                    return dl.join("macxtreamer");
                }
            }
            if let Some(home) = std::env::var_os("HOME") {
                let mut p = PathBuf::from(home);
                p.push("Downloads");
                p.push("macxtreamer");
                return p;
            }
            let mut p = std::env::temp_dir();
            p.push("macxtreamer_downloads");
            p
        };
        if raw.is_empty() {
            return default_dir();
        }
        if raw.starts_with("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                let mut p = PathBuf::from(home);
                p.push(&raw[2..]);
                return p;
            }
        }
        PathBuf::from(raw)
    }

    fn sanitize_title_for_fs(title: &str) -> String {
        let mut out = String::with_capacity(title.len());
        let mut last_underscore = false;
        for ch in title.chars() {
            let keep = ch.is_ascii_alphanumeric();
            if keep {
                out.push(ch);
                last_underscore = false;
            } else if matches!(ch, ' ' | '-' | '_' | '.' ) {
                if !last_underscore {
                    out.push('_');
                    last_underscore = true;
                }
            }
            // other chars dropped
            if out.len() >= 120 { break; }
        }
        while out.ends_with('_') { out.pop(); }
        if out.is_empty() { "download".into() } else { out }
    }

    fn local_file_path(&self, id: &str, name: &str, container_ext: Option<&str>) -> PathBuf {
        // Filename now based on (sanitized) title instead of id.
        let mut dir = self.expand_download_dir();
        let ext = container_ext.unwrap_or("mp4").trim_start_matches('.');
        let mut base = Self::sanitize_title_for_fs(name);
        if base.len() < 2 { base = id.to_string(); }
        let filename = format!("{base}.{ext}");
        dir.push(filename);
        dir
    }

    fn local_file_exists(&self, id: &str, name: &str, container_ext: Option<&str>) -> Option<PathBuf> {
        let p = self.local_file_path(id, name, container_ext);
        if p.exists() { Some(p) } else { None }
    }

    // (Old local_file_exists(id, ext) removed)

    fn file_path_to_uri(p: &Path) -> String {
        // Simple percent-encode spaces only (sufficient for our filenames)
        let s = p.to_string_lossy().replace(' ', "%20");
        if s.starts_with('/') {
            format!("file://{}", s)
        } else if s.starts_with("file://") {
            s
        } else {
            format!("file://{}", s)
        }
    }

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
    if let Some(path) = self.local_file_exists(&id, &row.name, row.container_extension.as_deref()) {
            let uri = Self::file_path_to_uri(&path);
            let _ = start_player(&self.config, &uri);
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
    fn spawn_download_bulk(&mut self, id: String, name: String, info: String, container_extension: Option<String>) {
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
    if self.local_file_exists(&id, &name, container_extension.as_deref()).is_some() {
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
        let max_parallel = 2usize;
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
    let target_path = self.local_file_path(&meta.id, &meta.name, meta.container_extension.as_deref());
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
        let client = self.http_client.clone();
        tokio::spawn(async move {
            if let Some(parent) = target_path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            let mut resp = match client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Msg::DownloadError {
                        id: id.clone(),
                        error: e.to_string(),
                    });
                    return;
                }
            };
            if !resp.status().is_success() {
                let _ = tx.send(Msg::DownloadError {
                    id: id.clone(),
                    error: format!("HTTP {}", resp.status()),
                });
                return;
            }
            let total = resp.content_length();
            let _ = tx.send(Msg::DownloadStarted {
                id: id.clone(),
                path: target_path.to_string_lossy().into(),
            });
            let mut received: u64 = 0;
            let mut file = match tokio::fs::File::create(&tmp_path).await {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(Msg::DownloadError {
                        id: id.clone(),
                        error: e.to_string(),
                    });
                    return;
                }
            };
            // log_line(&format!("Download started id={} url={} target={} size={}B", id, url, target_path.display(), total.map(|v| v.to_string()).unwrap_or_else(|| "unknown".into())));
            let mut last_sent = std::time::Instant::now();
            while let Ok(chunk) = resp.chunk().await {
                let Some(c) = chunk else {
                    break;
                };
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    let _ = tx.send(Msg::DownloadCancelled { id: id.clone() });
                    // log_line(&format!("Download cancelled id={}", id));
                    return;
                }
                if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut file, &c).await {
                    let _ = tx.send(Msg::DownloadError {
                        id: id.clone(),
                        error: e.to_string(),
                    });
                    return;
                }
                received += c.len() as u64;
                if last_sent.elapsed() > std::time::Duration::from_millis(250) {
                    last_sent = std::time::Instant::now();
                    let _ = tx.send(Msg::DownloadProgress {
                        id: id.clone(),
                        received,
                        total,
                    });
                }
            }
            let _ = tx.send(Msg::DownloadProgress {
                id: id.clone(),
                received,
                total,
            });
            if let Err(e) = tokio::fs::rename(&tmp_path, &target_path).await {
                let _ = tx.send(Msg::DownloadError {
                    id: id.clone(),
                    error: e.to_string(),
                });
                return;
            }
            let _ = tx.send(Msg::DownloadFinished {
                id: id.clone(),
                path: target_path.to_string_lossy().into(),
            });
            // log_line(&format!("Download finished id={} bytes={} path={}", id, received, target_path.display()));
        });
        if self.active_downloads() < max_parallel {
            self.maybe_start_next_download();
        }
    }

    fn resolve_play_url(&self, row: &Row) -> String {
        if row.info == "Movie" || row.info == "SeriesEpisode" {
        if let Some(p) = self.local_file_exists(&row.id, &row.name, row.container_extension.as_deref()) {
                return Self::file_path_to_uri(&p);
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
            if now.duration_since(last) < Duration::from_secs(5) { return; }
        }
        self.last_download_scan = Some(now);
        let dir = self.expand_download_dir();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut out: Vec<ScannedDownload> = Vec::new();
            if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = rd.next_entry().await {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("part") { continue; }
                    if let Ok(md) = entry.metadata().await {
                        if md.is_file() {
                            let mut id = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                            let mut name = id.clone();
                            let mut info = "Movie".to_string();
                            let mut container_extension = path.extension().and_then(|s| s.to_str()).map(|s| s.to_string());
                            let sidecar = path.with_extension(format!("{}.json", path.extension().and_then(|e| e.to_str()).unwrap_or_default()));
                            if let Ok(data) = tokio::fs::read(&sidecar).await {
                                if let Ok(js) = serde_json::from_slice::<serde_json::Value>(&data) {
                                    if let Some(v) = js.get("id").and_then(|v| v.as_str()) { id = v.to_string(); }
                                    if let Some(v) = js.get("name").and_then(|v| v.as_str()) { name = v.to_string(); }
                                    if let Some(v) = js.get("info").and_then(|v| v.as_str()) { info = v.to_string(); }
                                    if let Some(v) = js.get("ext").and_then(|v| v.as_str()) { container_extension = Some(v.to_string()); }
                                }
                            }
                            out.push(ScannedDownload { id, name, info, container_extension, path: path.to_string_lossy().into(), size: md.len(), modified: md.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH) });
                        }
                    }
                }
            }
            out.sort_by_key(|d| std::cmp::Reverse(d.modified));
            let _ = tx.send(Msg::DownloadsScanned(out));
        });
    }
}

impl eframe::App for MacXtreamer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
        // Während Hintergrundaktivität regelmäßig neu zeichnen, damit Channel-Polling stattfindet
        let has_bg_work = self.is_loading
            || self.active_downloads() > 0
            || !self.pending_texture_uploads.is_empty()
            || !self.pending_covers.is_empty()
            || !self.pending_decode_urls.is_empty()
            || self.indexing;
        if has_bg_work {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        // Drain messages
        let mut got_msg = false;
        let mut covers_to_prefetch: Vec<String> = Vec::new();
        for msg in self.rx.try_iter() {
            got_msg = true;
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
                        Err(e) => self.last_error = Some(e),
                    }
                    self.loading_done = self.loading_done.saturating_add(1);
                    if self.loading_done >= self.loading_total {
                        self.is_loading = false;
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
                        Err(e) => self.last_error = Some(e),
                    }
                    self.loading_done = self.loading_done.saturating_add(1);
                    if self.loading_done >= self.loading_total {
                        self.is_loading = false;
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
                        Err(e) => self.last_error = Some(e),
                    }
                    self.loading_done = self.loading_done.saturating_add(1);
                    if self.loading_done >= self.loading_total {
                        self.is_loading = false;
                    }
                }
                Msg::ItemsLoaded { kind, items } => {
                    match items {
                        Ok(items) => {
                            // map to rows and update all_movies/all_series caches
                            self.content_rows.clear();
                            let mut seen_ids: HashSet<String> = HashSet::new();
                            for it in items {
                                let info = match kind.as_str() {
                                    "subplaylist" => "Channel",
                                    "vod" => "Movie",
                                    "series" => "Series",
                                    _ => "Item",
                                };
                                if info == "Movie" {
                                    if seen_ids.insert(it.id.clone()) {
                                        self.all_movies.push(it.clone());
                                    }
                                } else if info == "Series" {
                                    if seen_ids.insert(it.id.clone()) {
                                        self.all_series.push(it.clone());
                                    }
                                }
                                self.content_rows.push(Row {
                                    name: it.name,
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
                                    rating_5based: it.rating_5based,
                                    genre: it.genre.clone(),
                                    path: Some(match info {
                                        "Movie" => format!("VOD / {}", self
                                            .vod_categories
                                            .get(self.selected_vod.unwrap_or(0))
                                            .map(|c| c.name.clone())
                                            .unwrap_or_else(|| "?".into())),
                                        "Series" => format!("Series / {}", self
                                            .series_categories
                                            .get(self.selected_series.unwrap_or(0))
                                            .map(|c| c.name.clone())
                                            .unwrap_or_else(|| "?".into())),
                                        "Channel" => format!("Live / {}", self
                                            .playlists
                                            .get(self.selected_playlist.unwrap_or(0))
                                            .map(|c| c.name.clone())
                                            .unwrap_or_else(|| "?".into())),
                                        _ => "".into(),
                                    }),
                                });
                            }
                        }
                        Err(e) => {
                            self.last_error = Some(e);
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
                } => match episodes {
                    Ok(eps) => {
                        self.content_rows.clear();
                        for ep in eps {
                            self.content_rows.push(Row {
                                name: ep.name,
                                id: ep.episode_id,
                                info: "SeriesEpisode".to_string(),
                                container_extension: Some(ep.container_extension),
                                stream_url: ep.stream_url.clone(),
                                cover_url: ep.cover.clone(),
                                year: None,
                                rating_5based: None,
                                genre: None,
                                path: Some("Series / Episodes".into()),
                            });
                        }
                        self.is_loading = false;
                        self.loading_done = self.loading_total;
                    }
                    Err(e) => {
                        self.is_loading = false;
                        self.last_error = Some(e);
                        self.loading_done = self.loading_total;
                    }
                },
                Msg::CoverLoaded { url, bytes } => {
                    // Offload decoding/resizing to a background worker to avoid UI hitches
                    if self.textures.contains_key(&url) || self.pending_decode_urls.contains(&url)
                    {
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
                                                .max(1.0) as u32;
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
                Msg::IndexBuilt {
                    movies: _m,
                    series: _s,
                } => {
                    // Bei Bedarf könnten wir hier all_movies/all_series aktualisieren,
                    // aktuell dienen die Caches von fetch_*; setze Flag zurück
                    self.indexing = false;
                }
                Msg::IndexData { movies, series } => {
                    self.all_movies = movies.iter().map(|(i, _)| i.clone()).collect();
                    self.all_series = series.iter().map(|(i, _)| i.clone()).collect();
                    self.index_paths.clear();
                    for (it, p) in movies.into_iter() { self.index_paths.insert(it.id, p); }
                    for (it, p) in series.into_iter() { self.index_paths.insert(it.id, p); }
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
                Msg::SeriesEpisodesForDownload { series_id: sid, episodes } => {
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
                                            let num: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
                                            if let Ok(n) = num.parse::<u32>() { if n == season_want { season_hit = true; break; } }
                                        }
                                    }
                                    if !season_hit {
                                        // Try pattern like '1x02'
                                        let mut last_digit_seq = String::new();
                                        for ch in name_lower.chars() {
                                            if ch.is_ascii_digit() { last_digit_seq.push(ch); }
                                            else if ch == 'x' && !last_digit_seq.is_empty() {
                                                if let Ok(n) = last_digit_seq.parse::<u32>() { if n == season_want { season_hit = true; } }
                                                last_digit_seq.clear();
                                            } else { last_digit_seq.clear(); }
                                        }
                                        if !season_hit { continue; }
                                    }
                                }
                                // Skip already downloaded if desired
                                if opts.only_not_downloaded {
                                    if let Some(p) = self.local_file_exists(&ep.episode_id, &ep.name, Some(&ep.container_extension)) { let _ = p; continue; }
                                }
                                // Enqueue
                                self.pending_bulk_downloads.push((
                                    ep.episode_id.clone(),
                                    ep.name.clone(),
                                    "SeriesEpisode".into(),
                                    Some(ep.container_extension.clone()),
                                ));
                                added += 1;
                                if opts.max_count > 0 && added >= opts.max_count { break; }
                            }
                            self.show_downloads = true;
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
                }
                Msg::DownloadError { id, error } => {
                    let st = self.downloads.entry(id).or_default();
                    st.error = Some(error);
                    st.finished = true;
                }
                Msg::DownloadCancelled { id } => {
                    if let Some(st) = self.downloads.get_mut(&id) {
                        st.error = Some("Cancelled".into());
                        st.finished = true;
                    }
                }
                Msg::SearchReady(mut rows) => {
                    // Fill paths for search results using index_paths if available
                    for r in &mut rows {
                        if r.path.is_none() {
                            if let Some(p) = self.index_paths.get(&r.id) {
                                r.path = Some(p.clone());
                            }
                        }
                    }
                    self.content_rows = rows;
                    self.is_loading = false;
                }
                Msg::DownloadsScanned(list) => {
                    // Verschmolzen mit existierenden Download-States falls IDs erkannt werden
                    for d in &list {
                        // Falls bereits bekannt (Session-Download), Pfad/Progress nicht überschreiben
                        if let Some(st) = self.downloads.get_mut(&d.id) {
                            if st.path.is_none() { st.path = Some(d.path.clone()); }
                            if !st.finished { st.finished = true; }
                            if let Some(meta) = self.download_meta.get_mut(&d.id) {
                                meta.size = Some(d.size);
                                meta.modified = Some(d.modified);
                            }
                        } else {
                            // Neue Session-unabhängige Einträge hinzufügen (nur Meta minimal)
                            self.downloads.insert(d.id.clone(), DownloadState { finished: true, path: Some(d.path.clone()), ..Default::default() });
                            self.download_order.push(d.id.clone());
                            self.download_meta.insert(d.id.clone(), DownloadMeta { id: d.id.clone(), name: d.name.clone(), info: d.info.clone(), container_extension: d.container_extension.clone(), size: Some(d.size), modified: Some(d.modified) });
                        }
                    }
                }
            }
        }

        // Wenn Nachrichten eingetroffen sind oder wir laden, sicherstellen, dass ein weiterer Frame kommt
        if got_msg || self.is_loading {
            ctx.request_repaint();
        }

        // Verarbeite pro Frame nur ein kleines Budget an Texture-Uploads,
        // um Frame-Drops beim Scrollen zu vermeiden.
    {
            let max_uploads_per_frame: usize = self
                .config
                .cover_uploads_per_frame
                .max(1)
                .min(16) as usize;
            let mut done = 0usize;
            while done < max_uploads_per_frame {
                let Some((url, rgba_bytes, w, h)) = self.pending_texture_uploads.pop_front() else { break };
                if !self.textures.contains_key(&url) {
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [w as usize, h as usize],
                        &rgba_bytes,
                    );
                    let tex = ctx.load_texture(
                        url.clone(),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.textures.insert(url.clone(), tex);
                }
                // Upload (oder Versuch) abgeschlossen -> Flags bereinigen
                self.pending_texture_urls.remove(&url);
                self.pending_covers.remove(&url);
                self.pending_decode_urls.remove(&url);
                done += 1;
            }
            if !self.pending_texture_uploads.is_empty() {
                // Noch Arbeit übrig – nächster Frame
                ctx.request_repaint();
            }
            // Grobe LRU-Begrenzung für Texturen
            let limit = self.config.texture_cache_limit.max(64) as usize;
            if self.textures.len() > limit {
                let remove_count = self.textures.len() - limit;
                let keys: Vec<String> = self.textures.keys().take(remove_count).cloned().collect();
                for k in keys { self.textures.remove(&k); }
            }
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
                    if !self.view_stack.is_empty() {
                        if ui.button("Back").on_hover_text("Go to previous view").clicked() {
                            if let Some(prev) = self.view_stack.pop() {
                                match &prev {
                                    ViewState::Items { kind, category_id } => {
                                        self.is_loading = true; self.loading_total = 1; self.loading_done = 0;
                                        self.spawn_load_items(kind, category_id.clone());
                                    }
                                    ViewState::Episodes { series_id } => {
                                        self.is_loading = true; self.loading_total = 1; self.loading_done = 0;
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
                        ui.colored_label(Color32::YELLOW, "Please complete settings to start");
                    }
                    if ui.button("Open Log").clicked() {
                        // Read log file and open viewer
                        let path = crate::logger::log_path();
                        self.log_text =
                            std::fs::read_to_string(path).unwrap_or_else(|_| "(no log)".into());
                        self.show_log = true;
                    }
                    if self.config.enable_downloads && ui.button("Downloads").clicked() {
                        self.show_downloads = true;
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
                    if ui.button("Settings").clicked() {
                        self.config_draft = Some(self.config.clone());
                        self.show_config = true;
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
                        if ui.add_enabled(current_percent > min_percent, egui::Button::new("−")).clicked() {
                            let new_percent = (current_percent - 5).max(min_percent);
                            self.current_font_scale = new_percent as f32 / 100.0;
                            self.config.font_scale = self.current_font_scale;
                            self.font_scale_applied = false;
                            self.pending_save_config = true;
                        }
                        
                        // Display percentage
                        ui.label(format!("{}%", current_percent));
                        
                        // Increase button  
                        if ui.add_enabled(current_percent < max_percent, egui::Button::new("+")).clicked() {
                            let new_percent = (current_percent + 5).min(max_percent);
                            self.current_font_scale = new_percent as f32 / 100.0;
                            self.config.font_scale = self.current_font_scale;
                            self.font_scale_applied = false;
                            self.pending_save_config = true;
                        }
                    });
                    if self.is_loading {
                        let pct = if self.loading_total > 0 {
                            (self.loading_done * 100 / self.loading_total).min(100)
                        } else {
                            0
                        };
                        ui.horizontal(|ui| {
                            ui.spinner(); // Add spinning indicator
                            ui.label(format!("Loading… {}% ({}/{})", pct, self.loading_done, self.loading_total));
                        });
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Search").clicked() {
                            if let Some(cv) = &self.current_view { self.view_stack.push(cv.clone()); }
                            self.current_view = Some(ViewState::Search { query: self.search_text.clone() });
                            self.start_search();
                        }
                        let resp = egui::TextEdit::singleline(&mut self.search_text)
                            .hint_text("Search…")
                            .desired_width(220.0)
                            .lock_focus(true)
                            .show(ui);
                        if resp.response.lost_focus()
                            && resp.response.ctx.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            self.start_search();
                            resp.response.request_focus();
                        }
                    });
                    if self.indexing {
                        ui.label("Indexing…");
                    }
                });

                ui.separator();

                // Drei Listen im oberen Bereich (Live, VOD, Serien)
                ui.columns(3, |cols| {
                    cols[0].label(RichText::new("Live").strong());
                    egui::ScrollArea::vertical()
                        .id_source("live_list")
                        .show(&mut cols[0], |ui| {
                            for (i, c) in self.playlists.clone().into_iter().enumerate() {
                                if ui
                                    .selectable_label(self.selected_playlist == Some(i), &c.name)
                                    .clicked()
                                {
                                    if let Some(cv) = &self.current_view { self.view_stack.push(cv.clone()); }
                                    self.current_view = Some(ViewState::Items { kind: "subplaylist".into(), category_id: c.id.clone() });
                                    self.selected_playlist = Some(i);
                                    self.selected_vod = None;
                                    self.selected_series = None;
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
                                    self.last_live_cat_id = Some(c.id.clone());
                                    self.spawn_load_items("subplaylist", c.id);
                                }
                            }
                        });

                    cols[1].label(RichText::new("VOD").strong());
                    egui::ScrollArea::vertical()
                        .id_source("vod_list")
                        .show(&mut cols[1], |ui| {
                            for (i, c) in self.vod_categories.clone().into_iter().enumerate() {
                                if ui
                                    .selectable_label(self.selected_vod == Some(i), &c.name)
                                    .clicked()
                                {
                                    if let Some(cv) = &self.current_view { self.view_stack.push(cv.clone()); }
                                    self.current_view = Some(ViewState::Items { kind: "vod".into(), category_id: c.id.clone() });
                                    self.selected_vod = Some(i);
                                    self.selected_playlist = None;
                                    self.selected_series = None;
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
                                    self.last_vod_cat_id = Some(c.id.clone());
                                    self.spawn_load_items("vod", c.id);
                                }
                            }
                        });

                    cols[2].label(RichText::new("Series").strong());
                    egui::ScrollArea::vertical().id_source("series_list").show(
                        &mut cols[2],
                        |ui| {
                            for (i, c) in self.series_categories.clone().into_iter().enumerate() {
                                if ui
                                    .selectable_label(self.selected_series == Some(i), &c.name)
                                    .clicked()
                                {
                                    if let Some(cv) = &self.current_view { self.view_stack.push(cv.clone()); }
                                    self.current_view = Some(ViewState::Items { kind: "series".into(), category_id: c.id.clone() });
                                    self.selected_series = Some(i);
                                    self.selected_playlist = None;
                                    self.selected_vod = None;
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
                                    self.last_series_cat_id = Some(c.id.clone());
                                    self.spawn_load_items("series", c.id);
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

        // Bottom panel (recently & favorites) MUST be declared before CentralPanel so CentralPanel height excludes it.
        egui::TopBottomPanel::bottom("bottom")
            .resizable(true)
            .show_separator_line(true)
            .default_height(320.0)
            .min_height(120.0)
            .show(ctx, |ui| {
                // Keep a slim spacer; rely on egui's built-in resize handle instead of custom grip overlay
                ui.add_space(4.0);
                ui.columns(2, |cols| {
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
                                        if ui.button(format!("{} ({})", it.name, it.info)).clicked() {
                                            let url = build_url_by_type(
                                                &self.config,
                                                &it.id,
                                                &it.info,
                                                it.container_extension.as_deref(),
                                            );
                                            let _ = start_player(&self.config, &url);
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
                                            ui.label(format!("{} ({})", it.name, it.info));
                                            if it.info == "Series" {
                                                if ui.small_button("Episodes").clicked() {
                                                    self.is_loading = true;
                                                    self.loading_total = 1;
                                                    self.loading_done = 0;
                                                    self.spawn_load_episodes(it.id.clone());
                                                }
                                            } else {
                                                let url = it.stream_url.clone().unwrap_or_else(|| {
                                                    build_url_by_type(
                                                        &self.config,
                                                        &it.id,
                                                        &it.info,
                                                        it.container_extension.as_deref(),
                                                    )
                                                });
                                                if ui.small_button("Play").clicked() {
                                                    let _ = start_player(&self.config, &url);
                                                }
                                                if ui.small_button("Copy").clicked() {
                                                    ui.output_mut(|o| o.copied_text = url.clone());
                                                }
                                            }
                                        });
                                    }
                                }
                            });
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            // Capture available size (after reserving top & bottom panels)
            let avail_w = ui.available_width();
            let avail_h = ui.available_height();
            ui.set_width(avail_w);
            let mut rows = self.content_rows.clone();
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
                }
                if !self.sort_asc {
                    rows.reverse();
                }
            }
            let cover_w = self.cover_height * (2.0 / 3.0);
            let row_h = (self.cover_height + 8.0).max(28.0);
            let header_h = 22.0;
            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .vscroll(true)
                // Leave some breathing room to avoid clipping against bottom panel border
                .min_scrolled_height((avail_h - 8.0).max(50.0))
                .column(egui_extras::Column::initial(cover_w + 16.0)) // Cover
                .column(egui_extras::Column::initial(400.0).at_least(400.0)) // Name (min 400px, resizable)
                .column(egui_extras::Column::initial(140.0)) // ID
                .column(egui_extras::Column::initial(120.0)) // Info
                .column(egui_extras::Column::initial(80.0)) // Year
                .column(egui_extras::Column::initial(80.0)) // Rating
                .column(egui_extras::Column::initial(200.0)) // Genre (resizable)
                .column(egui_extras::Column::initial(220.0)) // Path
                .column(egui_extras::Column::remainder().at_least(320.0)) // Aktion füllt Restbreite
                .header(header_h, |mut header| {
                    header.col(|ui| {
                        ui.strong("Cover");
                    });
                    header.col(|ui| {
                        let selected = self.sort_key == Some(SortKey::Name);
                        let label = if selected {
                            format!("Name {}", if self.sort_asc { "▲" } else { "▼" })
                        } else {
                            "Name".to_string()
                        };
                        if ui.small_button(label).clicked() {
                            if selected {
                                self.sort_asc = !self.sort_asc;
                            } else {
                                self.sort_key = Some(SortKey::Name);
                                self.sort_asc = true;
                            }
                        }
                    });
                    header.col(|ui| {
                        ui.strong("ID");
                    });
                    header.col(|ui| {
                        ui.strong("Info");
                    });
                    header.col(|ui| {
                        let selected = self.sort_key == Some(SortKey::Year);
                        let label = if selected {
                            format!("Year {}", if self.sort_asc { "▲" } else { "▼" })
                        } else {
                            "Year".to_string()
                        };
                        if ui.small_button(label).clicked() {
                            if selected {
                                self.sort_asc = !self.sort_asc;
                            } else {
                                self.sort_key = Some(SortKey::Year);
                                self.sort_asc = true;
                            }
                        }
                    });
                    header.col(|ui| {
                        let selected = self.sort_key == Some(SortKey::Rating);
                        // Default for first click on Rating: descending (highest first)
                        let label = if selected {
                            format!("Rating {}", if self.sort_asc { "▲" } else { "▼" })
                        } else {
                            "Rating".to_string()
                        };
                        if ui.small_button(label).clicked() {
                            if selected {
                                self.sort_asc = !self.sort_asc;
                            } else {
                                self.sort_key = Some(SortKey::Rating);
                                self.sort_asc = false;
                            }
                        }
                    });
                    header.col(|ui| {
                        let selected = self.sort_key == Some(SortKey::Genre);
                        let label = if selected {
                            format!("Genre {}", if self.sort_asc { "▲" } else { "▼" })
                        } else {
                            "Genre".to_string()
                        };
                        if ui.small_button(label).clicked() {
                            if selected {
                                self.sort_asc = !self.sort_asc;
                            } else {
                                self.sort_key = Some(SortKey::Genre);
                                self.sort_asc = true;
                            }
                        }
                    });
                    header.col(|ui| {
                        ui.strong("Path");
                    });
                    header.col(|ui| {
                        ui.strong("Action");
                    });
                })
                .body(|body| {
                    let row_count = rows.len();
                    body.rows(row_h, row_count, |i, mut row| {
                        let r = &rows[i];
                        let url = if r.info == "SeriesEpisode" {
                            // For episodes, always construct URL: base + /series/user/pass/id.ext
                            build_url_by_type(
                                &self.config,
                                &r.id,
                                &r.info,
                                r.container_extension.as_deref(),
                            )
                        } else {
                            // For movies/live, prefer API provided URL, fallback to builder
                            r.stream_url.clone().unwrap_or_else(|| {
                                build_url_by_type(
                                    &self.config,
                                    &r.id,
                                    &r.info,
                                    r.container_extension.as_deref(),
                                )
                            })
                        };
                        // Cover column (lazy: nur für sichtbare Zeilen wird diese Closure aufgerufen)
                        row.col(|ui| {
                            if let Some(cu) = &r.cover_url {
                                if let Some(tex) = self.textures.get(cu) {
                                    ui.add(
                                        egui::Image::new(tex).fit_to_exact_size(egui::vec2(
                                            cover_w,
                                            self.cover_height,
                                        )),
                                    );
                                } else {
                                    // Platzhalter zeichnen und lazy load anstoßen
                                    let (rect, _resp) = ui.allocate_exact_size(
                                        egui::vec2(cover_w, self.cover_height),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(rect, 4.0, Color32::from_gray(60));
                                    self.spawn_fetch_cover(cu);
                                }
                            }
                        });
                        // Name column
                        row.col(|ui| {
                            if r.info == "Series" {
                                if ui.link(&r.name).clicked() {
                                    if let Some(cv) = &self.current_view { self.view_stack.push(cv.clone()); }
                                    self.current_view = Some(ViewState::Episodes { series_id: r.id.clone() });
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
                                    self.spawn_load_episodes(r.id.clone());
                                }
                            } else {
                                ui.label(&r.name);
                            }
                        });
                        row.col(|ui| {
                            ui.label(&r.id);
                        });
                        row.col(|ui| {
                            ui.label(&r.info);
                        });
                        row.col(|ui| {
                            ui.label(r.year.clone().unwrap_or_default());
                        });
                        row.col(|ui| {
                            ui.label(
                                r.rating_5based
                                    .map(|v| format!("{:.1}", v))
                                    .unwrap_or_default(),
                            );
                        });
                        row.col(|ui| {
                            ui.label(r.genre.clone().unwrap_or_default());
                        });
                        row.col(|ui| {
                            ui.label(r.path.clone().unwrap_or_default());
                        });
                        row.col(|ui| {
                            ui.horizontal_wrapped(|ui| {
                                if r.info == "Series" {
                                    if ui.small_button("Episodes").clicked() {
                                        if let Some(cv) = &self.current_view { self.view_stack.push(cv.clone()); }
                                        self.current_view = Some(ViewState::Episodes { series_id: r.id.clone() });
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                        self.spawn_load_episodes(r.id.clone());
                                    }
                                    // Für Series kein direktes File, aber wir bieten Download (öffnet Episoden zum Downloaden)
                                    if self.config.enable_downloads && ui.small_button("Download all").on_hover_text("Queue all episodes for download").clicked() {
                                        self.confirm_bulk = Some((r.id.clone(), r.name.clone()));
                                    }
                                } else {
                                    if ui.small_button("Play").clicked() {
                                        if self.config.address.is_empty()
                                            || self.config.username.is_empty()
                                            || self.config.password.is_empty()
                                        {
                                            self.last_error = Some(
                                                "Please set address/username/password in Settings"
                                                    .into(),
                                            );
                                        } else {
                                            let play_url = self.resolve_play_url(r);
                                            let _ = start_player(&self.config, &play_url);
                                        }
                                        let rec = RecentItem {
                                            id: r.id.clone(),
                                            name: r.name.clone(),
                                            info: r.info.clone(),
                                            stream_url: build_url_by_type(
                                                &self.config,
                                                &r.id,
                                                &r.info,
                                                r.container_extension.as_deref(),
                                            ),
                                            container_extension: r.container_extension.clone(),
                                        };
                                        add_to_recently(&rec);
                                        self.recently = load_recently_played();
                                    }
                                    if ui.small_button("Copy").clicked() {
                                        ui.output_mut(|o| o.copied_text = url.clone());
                                    }
                                    if r.info == "Movie" || r.info == "SeriesEpisode" || r.info == "Series" || r.info == "VOD" {
                                        let st_opt = self.downloads.get(&r.id).cloned();
                                        let existing = self.local_file_exists(
                                            &r.id,
                                            &r.name,
                                            r.container_extension.as_deref(),
                                        );
                                        if let Some(st) = st_opt {
                                            if st.finished
                                                && st.error.is_none()
                                                && existing.is_some()
                                            {
                                                ui.weak("✔ downloaded");
                                                if let Some(p) = existing.clone() {
                                                    if ui
                                                        .small_button("Delete")
                                                        .on_hover_text("Remove local file")
                                                        .clicked()
                                                    {
                                                        if let Err(e) = std::fs::remove_file(&p) {
                                                            self.last_error = Some(format!(
                                                                "Failed to delete: {}",
                                                                e
                                                            ));
                                                        } else {
                                                            // Keep download state but file is gone; allow re-download
                                                            if let Some(ds) =
                                                                self.downloads.get_mut(&r.id)
                                                            {
                                                                ds.finished = false;
                                                                ds.total = None;
                                                                ds.received = 0;
                                                                ds.path = None;
                                                                ds.error = None;
                                                                ds.waiting = false;
                                                            }
                                                        }
                                                    }
                                                }
                                            } else if st.error.is_some() {
                                                ui.colored_label(Color32::RED, "Download failed");
                                                if ui.small_button("Retry").clicked() {
                                                    self.downloads.remove(&r.id);
                                                    self.spawn_download(r);
                                                }
                                            } else {
                                                // In progress
                                                let frac = st
                                                    .total
                                                    .map(|t| {
                                                        (st.received as f32 / t as f32).min(1.0)
                                                    })
                                                    .unwrap_or(0.0);
                                                let pct = if st.total.is_some() {
                                                    format!("{:.0}%", frac * 100.0)
                                                } else {
                                                    format!("{} KB", st.received / 1024)
                                                };
                                                ui.add(
                                                    egui::ProgressBar::new(frac)
                                                        .show_percentage()
                                                        .text(pct),
                                                );
                                                if let Some(flag) = &st.cancel_flag {
                                                    if ui.small_button("Cancel").clicked() {
                                                        flag.store(true, Ordering::Relaxed);
                                                    }
                                                }
                                            }
                                        } else if existing.is_some() {
                                            ui.weak("✔ downloaded");
                                            if let Some(p) = existing.clone() {
                                                if ui
                                                    .small_button("Delete")
                                                    .on_hover_text("Remove local file")
                                                    .clicked()
                                                {
                                                    if let Err(e) = std::fs::remove_file(&p) {
                                                        self.last_error = Some(format!(
                                                            "Failed to delete: {}",
                                                            e
                                                        ));
                                                    } else {
                                                        // Remove any stale state
                                                        self.downloads.remove(&r.id);
                                                    }
                                                }
                                            }
                                        } else {
                                            if self.config.enable_downloads && ui.small_button("Download").clicked() {
                                                self.spawn_download(r);
                                            }
                                        }
                                    }
                                    if r.info == "SeriesEpisode" {
                                        if ui.small_button("binge watch since here").clicked() {
                                            // Build playlist from the currently visible/sorted rows starting at i
                                            let mut entries: Vec<(String, String)> = Vec::new();
                                            for rr in rows.iter().skip(i) {
                                                if rr.info == "SeriesEpisode" {
                                                let url = build_url_by_type(
                                                        &self.config,
                                                        &rr.id,
                                                        &rr.info,
                                                        rr.container_extension.as_deref(),
                                                    );
                                                    entries.push((rr.name.clone(), url));
                                                }
                                            }
                                            if let Err(e) = self.create_and_play_m3u(&entries) {
                                                self.last_error = Some(e);
                                            }
                                        }
                                    }
                                    if ui.small_button("Fav").clicked() {
                                        toggle_favorite(&FavItem {
                                            id: r.id.clone(),
                                            info: r.info.clone(),
                                            name: r.name.clone(),
                                            stream_url: Some(url.clone()),
                                            container_extension: r.container_extension.clone(),
                                        });
                                        self.favorites = load_favorites();
                                    }
                                }
                            });
                        });
                    });
                });
            // Small spacer so last row isn't flush with panel edge
            ui.add_space(4.0);
        });

        if self.show_config {
            let mut open = self.show_config;
            let mut cancel_clicked = false;
            egui::Window::new("Configuration")
                .collapsible(false)
                .default_width(420.0)
                .default_height(260.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .open(&mut open)
                .show(ctx, |ui| {
                    let draft = self.config_draft.get_or_insert_with(|| self.config.clone());
                    ui.label("URL");
                    ui.text_edit_singleline(&mut draft.address);
                    ui.label("Username");
                    ui.text_edit_singleline(&mut draft.username);
                    ui.label("Password");
                    ui.add(egui::TextEdit::singleline(&mut draft.password).password(true));
                    ui.label("Player command");
                    ui.text_edit_singleline(&mut draft.player_command);
                    ui.small("Tip: Use the placeholder URL at the position where the stream URL should be inserted.\nExample: vlc --fullscreen --no-video-title-show --network-caching=2000 URL");
                    ui.horizontal(|ui| {
                        let mut reuse = draft.reuse_vlc;
                        if ui
                            .checkbox(&mut reuse, "Reuse VLC")
                            .on_hover_text("Open links in a running VLC instance (macOS)")
                            .changed()
                        {
                            draft.reuse_vlc = reuse;
                        }
                        ui.separator();
                        ui.label("Download directory");
                        ui.text_edit_singleline(&mut draft.download_dir);
                    });
                    if draft.download_dir.trim().is_empty() {
                        ui.weak("Will default to ~/Downloads/macxtreamer");
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .button("Apply VLC defaults")
                            .on_hover_text("Apply sensible VLC parameters for streaming")
                            .clicked()
                        {
                            draft.player_command = "vlc --fullscreen --no-video-title-show --network-caching=2000 URL".to_string();
                        }
                        // Show the currently effective command (with placeholder visible)
                        let preview = if draft.player_command.trim().is_empty() {
                            "vlc --fullscreen --no-video-title-show --network-caching=2000 URL"
                                .to_string()
                        } else {
                            draft.player_command.clone()
                        };
                        ui.label(egui::RichText::new(format!("Current: {}", preview)).weak());
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Cover TTL (days)");
                        let mut ttl = if draft.cover_ttl_days == 0 {
                            7
                        } else {
                            draft.cover_ttl_days
                        } as i32;
                        if ui
                            .add(egui::DragValue::new(&mut ttl).clamp_range(1..=30))
                            .changed()
                        {
                            draft.cover_ttl_days = ttl as u32;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Cover parallelism");
                        let mut par = if draft.cover_parallel == 0 {
                            6
                        } else {
                            draft.cover_parallel
                        } as i32;
                        if ui
                            .add(egui::DragValue::new(&mut par).clamp_range(1..=16))
                            .changed()
                        {
                            draft.cover_parallel = par as u32;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Uploads/frame");
                        let mut upf = if draft.cover_uploads_per_frame == 0 {
                            3
                        } else {
                            draft.cover_uploads_per_frame
                        } as i32;
                        if ui
                            .add(egui::DragValue::new(&mut upf).clamp_range(1..=16))
                            .changed()
                        {
                            draft.cover_uploads_per_frame = upf as u32;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Decode parallelism");
                        let mut dp = if draft.cover_decode_parallel == 0 {
                            2
                        } else {
                            draft.cover_decode_parallel
                        } as i32;
                        if ui
                            .add(egui::DragValue::new(&mut dp).clamp_range(1..=8))
                            .changed()
                        {
                            draft.cover_decode_parallel = dp as u32;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Texture cache limit");
                        let mut tl = if draft.texture_cache_limit == 0 {
                            512
                        } else {
                            draft.texture_cache_limit
                        } as i32;
                        if ui
                            .add(egui::DragValue::new(&mut tl).clamp_range(64..=4096))
                            .changed()
                        {
                            draft.texture_cache_limit = tl as u32;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Category parallelism");
                        let mut cp = if draft.category_parallel == 0 {
                            6
                        } else {
                            draft.category_parallel
                        } as i32;
                        if ui
                            .add(egui::DragValue::new(&mut cp).clamp_range(1..=20))
                            .on_hover_text("Number of parallel category requests during loading")
                            .changed()
                        {
                            draft.category_parallel = cp as u32;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Cover height");
                        let mut ch = if draft.cover_height == 0.0 {
                            60.0
                        } else {
                            draft.cover_height
                        };
                        if ui
                            .add(egui::Slider::new(&mut ch, 40.0..=120.0).step_by(2.0))
                            .on_hover_text("Height of cover images in the content view")
                            .changed()
                        {
                            draft.cover_height = ch;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Text size scale");
                        let mut fs = if draft.font_scale == 0.0 {
                            1.15
                        } else {
                            draft.font_scale
                        };
                        if ui
                            .add(egui::Slider::new(&mut fs, 0.6..=2.0).step_by(0.05))
                            .on_hover_text("Scale factor for all text in the interface")
                            .changed()
                        {
                            draft.font_scale = fs;
                        }
                    });
                    ui.horizontal(|ui| {
                        let mut enable_downloads = draft.enable_downloads;
                        if ui
                            .checkbox(&mut enable_downloads, "Enable Downloads")
                            .on_hover_text("Show download buttons and enable downloading functionality")
                            .changed()
                        {
                            draft.enable_downloads = enable_downloads;
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
                        if ui.button("Cancel").clicked() {
                            cancel_clicked = true;
                        }
                    });
                });

            // (Bottom-Panel wird immer außerhalb des Config-Fensters gerendert)
            if cancel_clicked || !open {
                self.config_draft = None;
                self.show_config = false;
            } else {
                self.show_config = open;
            }
        }

        // Log viewer window
        if self.show_log {
            let mut open = self.show_log;
            egui::Window::new("Application Log")
                .default_width(840.0)
                .default_height(420.0)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.small_button("Refresh").clicked() {
                            let path = crate::logger::log_path();
                            self.log_text =
                                std::fs::read_to_string(path).unwrap_or_else(|_| "(no log)".into());
                        }
                        if ui.small_button("Clear").clicked() {
                            let path = crate::logger::log_path();
                            let _ = std::fs::write(path, "");
                            self.log_text.clear();
                        }
                    });
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.monospace(&self.log_text);
                        });
                });
            self.show_log = open;
        }

    // (Bottom panel already rendered above CentralPanel)

        // Handle deferred save to avoid mutable borrow inside Window closure
        if self.pending_save_config {
            let _ = save_config(&self.config);
            // Übernehme neue Parallelität sofort
            let permits = if self.config.cover_parallel == 0 {
                6
            } else {
                self.config.cover_parallel
            } as usize;
            self.cover_sem = Arc::new(Semaphore::new(permits));
            // Apply decode parallelism immediately
            let dpermits = if self.config.cover_decode_parallel == 0 { 2 } else { self.config.cover_decode_parallel } as usize;
            self.decode_sem = Arc::new(Semaphore::new(dpermits));
            // Apply cover height and font scale immediately
            self.cover_height = if self.config.cover_height == 0.0 { 60.0 } else { self.config.cover_height };
            self.font_scale_applied = false; // Force font scale reapplication
            if self.config_is_complete() {
                // Only start loading now if config became complete
                self.reload_categories();
                if self.initial_config_pending {
                    self.spawn_preload_all();
                    self.initial_config_pending = false;
                }
            }
            self.show_config = false;
            self.pending_save_config = false;
            self.config_draft = None;
        }

        // Downloads window (queue)
        if self.show_downloads {
            let mut open = self.show_downloads;
            // Beim Öffnen einen Scan initiieren (throttled)
            if open { self.scan_download_directory(); }
            egui::Window::new("Downloads")
                .default_width(620.0)
                .default_height(400.0)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("Active: {}", self.active_downloads()));
                        ui.label(format!(
                            "Waiting: {}",
                            self.downloads.values().filter(|s| s.waiting).count()
                        ));
                        ui.label(format!(
                            "Finished: {}",
                            self.downloads
                                .values()
                                .filter(|s| s.finished && s.error.is_none())
                                .count()
                        ));
                        if ui.small_button("Clear finished").clicked() {
                            self.downloads
                                .retain(|_, s| !s.finished || s.error.is_some());
                            self.download_order
                                .retain(|id| self.downloads.contains_key(id));
                        }
                    });
                    ui.separator();
                    let order_snapshot: Vec<String> = self.download_order.clone();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for id in order_snapshot {
                            let (meta_name, waiting, finished, error_opt, frac_opt, received_kb, total_known, path_opt, cancel_flag, size_opt, modified_opt) = if let (Some(meta), Some(st)) = (self.download_meta.get(&id), self.downloads.get(&id)) {
                                let frac = st.total.map(|t| (st.received as f32 / t as f32).min(1.0));
                                (meta.name.clone(), st.waiting, st.finished, st.error.clone(), frac, st.received / 1024, st.total.is_some(), st.path.clone(), st.cancel_flag.clone(), meta.size, meta.modified)
                            } else { continue };
                            let is_done_ok = finished && error_opt.is_none();
                            ui.horizontal(|ui| {
                                ui.label(format!("{} ({})", meta_name, id));
                                if let Some(sz) = size_opt { ui.weak(format!("{:.1} MB", (sz as f64)/ (1024.0*1024.0))); }
                                if waiting { ui.weak("waiting"); }
                                else if finished {
                                    if let Some(err) = error_opt.as_ref() { ui.colored_label(Color32::RED, format!("error: {}", err)); }
                                    else { ui.colored_label(Color32::GREEN, "done"); }
                                } else {
                                    let frac = frac_opt.unwrap_or(0.0);
                                    let pct = if total_known { format!("{:.0}%", frac * 100.0) } else { format!("{} KB", received_kb) };
                                    ui.add(egui::ProgressBar::new(frac).desired_width(160.0).text(pct));
                                    if let Some(flag) = &cancel_flag { if ui.small_button("Cancel").clicked() { flag.store(true, Ordering::Relaxed); } }
                                }
                                if is_done_ok {
                                    if let Some(p) = &path_opt {
                                        if ui.small_button("Play").clicked() {
                                            let uri = Self::file_path_to_uri(Path::new(p));
                                            let _ = start_player(&self.config, &uri);
                                        }
                                        if ui.small_button("Delete").on_hover_text("Remove local file").clicked() {
                                            if let Err(e) = std::fs::remove_file(p) { self.last_error = Some(format!("Failed to delete: {}", e)); }
                                            else { self.scan_download_directory(); }
                                        }
                                    }
                                    if let Some(mt) = modified_opt { if let Ok(delta) = mt.elapsed() { ui.weak(format!("{}m ago", delta.as_secs()/60)); } }
                                }
                            });
                        }
                    });
                });
            self.show_downloads = open;
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
                        if ui.add(egui::DragValue::new(&mut s).clamp_range(0..=99)).changed() {
                            opts.season = if s <= 0 { None } else { Some(s as u32) };
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Max episodes (0=all)");
                        let mut m = opts.max_count as i32;
                        if ui.add(egui::DragValue::new(&mut m).clamp_range(0..=2000)).changed() {
                            opts.max_count = m.max(0) as u32;
                        }
                    });
                    self.bulk_options_by_series.insert(series_id.clone(), opts.clone());
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
            if !open { self.confirm_bulk = None; }
        }

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

// (Hilfs-Module für Config/Cache/API/Player/Storage/Suche sind ausgelagert)
