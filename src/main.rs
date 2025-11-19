use eframe::egui::{self, Color32, RichText};
use egui_extras::TableBuilder;
use image::GenericImageView;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::sync::Semaphore;

mod api;
mod app_state;
mod cache;
mod config;
mod downloads;
mod icon;
mod images;
mod logger;
mod models;
mod player;
mod search;
mod storage;
mod ui_helpers;

use api::{fetch_categories, fetch_items, fetch_series_episodes};
use app_state::{Msg, SortKey, ViewState};
use cache::{clear_all_caches, file_age_secs, image_cache_path};
use config::{read_config, save_config};
use downloads::{BulkOptions, sanitize_filename};

// Local download tracking structs (specialized for UI & retry logic)
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
struct DownloadState {
    waiting: bool,
    finished: bool,
    error: Option<String>,
    path: Option<String>,
    received: u64,
    total: Option<u64>,
    cancel_flag: Option<Arc<AtomicBool>>,
    started_at: Option<std::time::Instant>,
    last_update_at: Option<std::time::Instant>,
    prev_received: u64,
    current_speed_bps: f64,
    avg_speed_bps: f64,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            waiting: false,
            finished: false,
            error: None,
            path: None,
            received: 0,
            total: None,
            cancel_flag: None,
            started_at: None,
            last_update_at: None,
            prev_received: 0,
            current_speed_bps: 0.0,
            avg_speed_bps: 0.0,
        }
    }
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
use images::image_meta_path;
use logger::log_line;
use models::{Category, Config, FavItem, Item, RecentItem, Row};
use ui_helpers::{colored_text_by_type, render_loading_spinner, format_file_size, file_path_to_uri};
use player::{build_url_by_type, start_player};
use once_cell::sync::OnceCell;
static GLOBAL_TX: OnceCell<Sender<Msg>> = OnceCell::new();
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use search::search_items;
use storage::{add_to_recently, load_favorites, load_recently_played, toggle_favorite};

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
    wisdom_gate_last_fetch: Option<std::time::Instant>,
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
}

impl MacXtreamer {
    fn new() -> Self {
        let read_result = read_config();
        let (config, had_file) = match read_result {
            Ok(c) => (c, true),
            Err(_) => (Config::default(), false),
        };
        
        // Check for cached recommendations
        let cached_recommendations = if config.is_wisdom_gate_cache_valid() && !config.wisdom_gate_cache_content.is_empty() {
            let cache_age = config.get_wisdom_gate_cache_age_hours();
            println!("📦 Lade gecachte Empfehlungen beim Start (Alter: {}h)", cache_age);
            Some(format!("📦 **Gecachte Empfehlungen** (vor {}h aktualisiert)\n\n{}", 
                cache_age, &config.wisdom_gate_cache_content))
        } else {
            None
        };
        
        let (tx, rx) = mpsc::channel();
    let _ = GLOBAL_TX.set(tx.clone());
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
            pending_covers: HashSet::new(),
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
            current_font_scale: 1.15,
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
                .pool_idle_timeout(Duration::from_secs(300))
                .pool_max_idle_per_host(2)
                .tcp_nodelay(true)
                .tcp_keepalive(Some(Duration::from_secs(60)))
                .timeout(Duration::from_secs(7200))
                .connect_timeout(Duration::from_secs(30))
                .user_agent("MacXtreamer/1.0")
                .danger_accept_invalid_certs(true)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            last_download_scan: None,
            should_check_downloads: false,
            should_start_search: false,
            current_view: None,
            view_stack: Vec::new(),
            wisdom_gate_recommendations: cached_recommendations,
            wisdom_gate_last_fetch: None,
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

        // Player Erkennung in Hintergrund-Thread starten
        {
            let tx_detect = app.tx.clone();
            std::thread::spawn(move || {
                use std::process::Command;
                use std::process::Stdio;
                // VLC Detection
                let (has_vlc, vlc_version, vlc_path) = match Command::new("vlc").arg("--version").stdout(Stdio::piped()).stderr(Stdio::null()).output() {
                    Ok(out) => {
                        let ver = String::from_utf8(out.stdout).ok().and_then(|s| s.lines().next().map(|l| l.to_string()));
                        let path = Command::new("which").arg("vlc").output().ok()
                            .and_then(|o| String::from_utf8(o.stdout).ok())
                            .map(|s| s.trim().to_string());
                        (true, ver, path)
                    }
                    Err(_) => (false, None, None),
                };
                // mpv Detection
                let (has_mpv, mpv_version, mpv_path) = match Command::new("mpv").arg("--version").stdout(Stdio::piped()).stderr(Stdio::null()).output() {
                    Ok(out) => {
                        let ver = String::from_utf8(out.stdout).ok().and_then(|s| s.lines().next().map(|l| l.to_string()));
                        let path = Command::new("which").arg("mpv").output().ok()
                            .and_then(|o| String::from_utf8(o.stdout).ok())
                            .map(|s| s.trim().to_string());
                        (true, ver, path)
                    }
                    Err(_) => (false, None, None),
                };
                let _ = tx_detect.send(Msg::PlayerDetection { has_vlc, has_mpv, vlc_version, mpv_version, vlc_path, mpv_path });
            });
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

    fn config_is_complete(&self) -> bool {
        !self.config.address.trim().is_empty()
            && !self.config.username.trim().is_empty()
            && !self.config.password.trim().is_empty()
    }

    fn effective_config(&self) -> &Config {
        if let Some(d) = self.config_draft.as_ref() { d } else { &self.config }
    }

    fn clear_caches_and_reload(&mut self) {
        // In-Memory Texturen und Cover Warteschlangen leeren
        self.textures.clear();
        self.pending_covers.clear();
        // Dateisystem Cache leeren (Images, ggf. andere)
        clear_all_caches();
        // Kategorien neu laden falls Konfig vollständig
        if self.config_is_complete() { self.reload_categories(); }
    }

    fn create_and_play_m3u(&mut self, entries: &[(String,String)]) -> Result<(), String> {
        use std::io::Write;
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_else(|_| std::time::Duration::from_secs(0)).as_secs();
        let path = std::env::temp_dir().join(format!("macxtreamer_playlist_{}.m3u", ts));
        let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        writeln!(file, "#EXTM3U").ok();
        for (title, url) in entries { 
            writeln!(file, "#EXTINF:-1,{}", title).ok();
            writeln!(file, "{}", url).map_err(|e| e.to_string())?; 
        }
        let path_str = path.to_string_lossy().to_string();
        start_player(self.effective_config(), &path_str).map_err(|e| e)
    }

    fn resume_incomplete_downloads(&mut self) {
        if !self.config.enable_downloads { return; }
        let dir = self.expand_download_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else { return; };
        for ent in entries.flatten() {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("part") { continue; }
            // Ableiten ursprünglicher Erweiterung (dateiname.mp4.part -> file_stem = dateiname.mp4)
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
            let (base_name, orig_ext) = match stem.rsplit_once('.') { Some((b,e)) => (b.to_string(), e.to_string()), None => (stem.clone(), "mp4".to_string()) };
            let sidecar = path.with_file_name(format!("{}.{}.json", base_name, orig_ext));
            if !sidecar.exists() { 
                // Ohne Sidecar keine Resume-Metadaten -> überspringen
                continue; 
            }
            let meta_json = match std::fs::read(&sidecar) { Ok(d)=>d, Err(_)=>continue }; 
            let mut id = String::new();
            let mut name = base_name.clone();
            let mut info = "Movie".to_string();
            let mut container_extension = Some(orig_ext.clone());
            if let Ok(js) = serde_json::from_slice::<serde_json::Value>(&meta_json) {
                if let Some(v)=js.get("id").and_then(|v| v.as_str()) { id = v.to_string(); }
                if let Some(v)=js.get("name").and_then(|v| v.as_str()) { name = v.to_string(); }
                if let Some(v)=js.get("info").and_then(|v| v.as_str()) { info = v.to_string(); }
                if let Some(v)=js.get("ext").and_then(|v| v.as_str()) { container_extension = Some(v.to_string()); }
            }
            if id.is_empty() { continue; }
            if self.downloads.contains_key(&id) { continue; }
            // Prüfen ob finale Datei bereits existiert -> dann part löschen
            let final_path = self.local_file_path(&id, &name, container_extension.as_deref());
            if final_path.exists() { let _ = std::fs::remove_file(&path); continue; }
            // DownloadState / Meta anlegen und direkt starten
            let meta = DownloadMeta { id: id.clone(), name: name.clone(), info: info.clone(), container_extension: container_extension.clone(), size: None, modified: None };
            self.download_meta.insert(id.clone(), meta);
            self.download_order.push(id.clone());
            self.downloads.insert(id.clone(), DownloadState { waiting: true, path: Some(final_path.to_string_lossy().into()), ..Default::default() });
        }
        // Versuche ausstehende (wartende) Downloads zu starten
        self.maybe_start_next_download();
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
            // Dedup by id (first movies then series)
            let mut seen = std::collections::HashSet::new();
            all_movies.retain(|(i, _)| seen.insert(i.id.clone()));
            seen.clear();
            all_series.retain(|(i, _)| seen.insert(i.id.clone()));
            // Wichtig: Erst IndexData senden (füllt Caches), dann IndexBuilt (setzt indexing=false und triggert Suche)
            let movies_len = all_movies.len();
            let series_len = all_series.len();
            let _ = tx.send(Msg::IndexData {
                movies: all_movies,
                series: all_series,
            });
            let _ = tx.send(Msg::IndexBuilt {
                movies: movies_len,
                series: series_len,
            });
        });
    }

    fn start_search(&mut self) {
        let tx = self.tx.clone();
        let movies = self.all_movies.clone();
        let series = self.all_series.clone();
        let query = self.search_text.clone();
        if movies.is_empty() && series.is_empty() && !self.indexing {
            self.spawn_build_index();
            // Return early - search will be performed after index is built
            return;
        }
        // If indexing is in progress, wait for it to complete
        if self.indexing {
            return;
        }
        self.is_loading = true;
        self.loading_total = 1;
        self.loading_done = 0;
        tokio::spawn(async move {
            let results = search_items(&movies, &series, &query);
            let rows: Vec<Row> = results
                .into_iter()
                .map(|s| Row {
                    name: s.name.clone(),
                    id: s.id,
                    info: s.info,
                    container_extension: if s.container_extension.is_empty() {
                        None
                    } else {
                        Some(s.container_extension)
                    },
                    stream_url: None,
                    cover_url: s.cover,
                    year: s.year.clone(),
                    release_date: s.release_date.clone().or_else(|| extract_year_from_title(&s.name)),
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
                .user_agent("MacXtreamer/1.0")
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
                let resp = match req.send().await { Ok(r)=>r, Err(e)=>{ let err = format!("Network error: {}", e); println!("{}", err); log_line(&err); attempt+=1; if attempt>=attempts_max { let _=tx.send(Msg::DownloadError { id: id.clone(), error: err }); return; } else { tokio::time::sleep(Duration::from_millis(delay_ms)).await; continue; } } };
                if resp.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                    // Möglicherweise schon komplett -> rename falls final nicht existiert
                    if !target_path.exists() { let _ = tokio::fs::rename(&tmp_path, &target_path).await; }
                    let _ = tx.send(Msg::DownloadFinished { id: id.clone(), path: target_path.to_string_lossy().into() });
                    return;
                }
                if !resp.status().is_success() {
                    let err = format!("HTTP {}", resp.status()); println!("{}", err); log_line(&err);
                    attempt+=1; if attempt>=attempts_max { let _=tx.send(Msg::DownloadError { id: id.clone(), error: err }); return; } else { tokio::time::sleep(Duration::from_millis(delay_ms)).await; continue; }
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
                // Erfolgreich
                if let Err(e)=tokio::fs::rename(&tmp_path, &target_path).await { let _=tx.send(Msg::DownloadError { id: id.clone(), error: format!("Rename failed: {}", e) }); return; }
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
                    if path.extension().and_then(|e| e.to_str()) == Some("part") {
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
        ui.heading("🧠 AI Empfehlungen");
        ui.add_space(5.0);

        // API Key Status
        if self.config.wisdom_gate_api_key.is_empty() {
            ui.colored_label(egui::Color32::YELLOW, "⚠️ Kein API-Key konfiguriert");
            ui.label("Bitte API-Key in den Einstellungen hinzufügen.");
            ui.add_space(5.0);
            ui.label(format!("Model: {}", self.config.wisdom_gate_model));
            ui.label(format!("Prompt: {}", self.config.wisdom_gate_prompt.chars().take(50).collect::<String>() + "..."));
            return;
        }

        // Fetch recommendations button
        ui.horizontal(|ui| {
            if ui.button("🔄 Empfehlungen aktualisieren").clicked() {
                // Check if cache is valid first
                if self.config.is_wisdom_gate_cache_valid() && !self.config.wisdom_gate_cache_content.is_empty() {
                    // Use cached content
                    let cache_age = self.config.get_wisdom_gate_cache_age_hours();
                    println!("📦 Verwende gecachte Empfehlungen (Alter: {}h)", cache_age);
                    self.wisdom_gate_recommendations = Some(format!("📦 **Gecachte Empfehlungen** (vor {}h aktualisiert)\n\n{}", 
                        cache_age, self.config.wisdom_gate_cache_content));
                } else {
                    // Fetch new content
                    let tx = self.tx.clone();
                    let api_key = self.config.wisdom_gate_api_key.clone();
                    let model = self.config.wisdom_gate_model.clone();
                    let prompt = self.config.wisdom_gate_prompt.clone();
                    
                    tokio::spawn(async move {
                        println!("🌐 Lade neue Empfehlungen von Wisdom-Gate...");
                        let content = crate::api::fetch_wisdom_gate_recommendations_safe(&api_key, &prompt, &model).await;
                        let _ = tx.send(crate::app_state::Msg::WisdomGateRecommendations(content));
                    });
                }
            }

            // Show cache status
            if self.config.is_wisdom_gate_cache_valid() {
                let cache_age = self.config.get_wisdom_gate_cache_age_hours();
                ui.label(format!("📦 Cache: {}h alt", cache_age));
            } else if !self.config.wisdom_gate_cache_content.is_empty() {
                ui.colored_label(egui::Color32::YELLOW, "⚠️ Cache abgelaufen");
            } else {
                ui.colored_label(egui::Color32::GRAY, "📭 Kein Cache");
            }
        });

        ui.add_space(10.0);

        // Display recommendations
        if let Some(ref content) = self.wisdom_gate_recommendations {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(egui::RichText::new("🎬 Heutige Streaming-Empfehlungen:")
                    .strong()
                    .size(16.0));
                ui.add_space(8.0);
                
                if content.starts_with("Fehler") {
                    ui.colored_label(egui::Color32::RED, 
                        egui::RichText::new(content).size(14.0));
                } else {
                    // Parse and display with larger font and selectable text
                    for line in content.lines() {
                        if line.trim().is_empty() {
                            ui.add_space(4.0);
                            continue;
                        }
                        
                        // Headers (### or ##)
                        if line.starts_with("###") || line.starts_with("##") {
                            let _ = ui.selectable_label(false, egui::RichText::new(line.trim_start_matches('#').trim())
                                .strong()
                                .size(18.0)
                                .color(egui::Color32::from_rgb(100, 200, 255)));
                            ui.add_space(3.0);
                        } 
                        // Bold text (**text**)
                        else if line.starts_with("**") && line.ends_with("**") {
                            let _ = ui.selectable_label(false, egui::RichText::new(line.trim_start_matches("**").trim_end_matches("**"))
                                .strong()
                                .size(15.0)
                                .color(egui::Color32::from_rgb(255, 255, 150)));
                            ui.add_space(2.0);
                        } 
                        // List items or content with bullets
                        else if line.starts_with("*") || line.starts_with("-") || line.contains("–") {
                            let _ = ui.selectable_label(false, egui::RichText::new(line.trim_start_matches('*').trim_start_matches('-').trim())
                                .size(14.0)
                                .color(egui::Color32::LIGHT_GRAY));
                            ui.add_space(1.0);
                        } 
                        // Regular text
                        else {
                            let _ = ui.selectable_label(false, egui::RichText::new(line)
                                .size(14.0)
                                .color(egui::Color32::LIGHT_GRAY));
                            ui.add_space(1.0);
                        }
                    }
                }
            });
        } else {
            ui.colored_label(egui::Color32::GRAY, "📭 Noch keine Empfehlungen geladen...");
            ui.label("Klicken Sie auf 'Empfehlungen aktualisieren' um zu starten.");
        }
    }
}

impl eframe::App for MacXtreamer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Frame Timing erfassen für adaptive Repaint-Steuerung
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_millis() as f32;
        self.last_frame_time = now;
        // Exponentielles Glätten
        if self.avg_frame_ms == 0.0 { self.avg_frame_ms = dt; } else { self.avg_frame_ms = self.avg_frame_ms * 0.9 + dt * 0.1; }
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
        if self.config.low_cpu_mode {
            // Low CPU Mode: adaptive Thresholds basierend auf durchschnittlicher Frame-Zeit
            let base_critical = 2500u64; // länger
            let base_minor = 5000u64;
            if has_critical_bg_work {
                let delay = if self.avg_frame_ms < 25.0 { base_critical } else { base_critical / 2 }; // Wenn ohnehin langsam -> etwas schneller erlauben
                ctx.request_repaint_after(Duration::from_millis(delay));
            } else if has_minor_bg_work {
                ctx.request_repaint_after(Duration::from_millis(base_minor));
            }
        } else {
            if has_critical_bg_work {
                ctx.request_repaint_after(Duration::from_millis(2000));
            } else if has_minor_bg_work {
                ctx.request_repaint_after(Duration::from_millis(4000));
            }
        }
        // If no background work, NO automatic repaints at all!

        // CRITICAL CPU FIX: Limit message processing to prevent endless loops
        let mut got_msg = false;
        let mut covers_to_prefetch: Vec<String> = Vec::new();
        let mut message_count = 0;
        const MAX_MESSAGES_PER_FRAME: usize = 3; // Very strict limit!
        
        while let Ok(msg) = self.rx.try_recv() {
            message_count += 1;
            if message_count > MAX_MESSAGES_PER_FRAME {
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
                    if self.config.use_mpv && !self.has_mpv { self.config.use_mpv = false; self.last_error = Some("mpv nicht gefunden – zurück zu VLC".into()); self.pending_save_config = true; }
                    // If mpv only available -> auto enable
                    if !self.config.use_mpv && self.has_mpv && !self.has_vlc { self.config.use_mpv = true; self.pending_save_config = true; }
                }
                Msg::PlayerSpawnFailed { player, error } => {
                    if player.contains("mpv") { self.mpv_fail_count = self.mpv_fail_count.saturating_add(1); }
                    if player.to_lowercase().contains("vlc") { self.vlc_fail_count = self.vlc_fail_count.saturating_add(1); }
                    self.last_error = Some(format!("{} Startfehler: {}", player, error));
                    if self.config.use_mpv && self.mpv_fail_count >= 3 && self.has_vlc { self.config.use_mpv = false; self.pending_save_config = true; self.last_error = Some("mpv wiederholt fehlgeschlagen – Wechsel auf VLC".into()); }
                    if !self.config.use_mpv && self.vlc_fail_count >= 3 && self.has_mpv { self.config.use_mpv = true; self.pending_save_config = true; self.last_error = Some("VLC wiederholt fehlgeschlagen – Wechsel auf mpv".into()); }
                }
                Msg::DiagnosticsStopped => {
                    self.last_error = Some("VLC Diagnose gestoppt".into());
                    if let Some(flag) = &self.active_diag_stop { flag.store(true, std::sync::atomic::Ordering::Relaxed); }
                }
                Msg::StopDiagnostics => {
                    if let Some(flag) = &self.active_diag_stop { flag.store(true, std::sync::atomic::Ordering::Relaxed); }
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
                                release_date: None,
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
                Msg::IndexBuilt {
                    movies: _m,
                    series: _s,
                } => {
                    // Bei Bedarf könnten wir hier all_movies/all_series aktualisieren,
                    // aktuell dienen die Caches von fetch_*; setze Flag zurück
                    self.indexing = false;
                    
                    // If we're in search view and have a search query, flag to perform the search
                    if let Some(ViewState::Search { .. }) = &self.current_view {
                        if !self.search_text.trim().is_empty() {
                            self.should_start_search = true;
                        }
                    }
                }
                Msg::IndexData { movies, series } => {
                    self.all_movies = movies.iter().map(|(i, _)| i.clone()).collect();
                    self.all_series = series.iter().map(|(i, _)| i.clone()).collect();
                    self.index_paths.clear();
                    for (it, p) in movies.into_iter() {
                        self.index_paths.insert(it.id, p);
                    }
                    for (it, p) in series.into_iter() {
                        self.index_paths.insert(it.id, p);
                    }
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
                            if st.path.is_none() {
                                st.path = Some(d.path.clone());
                            }
                            if !st.finished {
                                st.finished = true;
                            }
                            if let Some(meta) = self.download_meta.get_mut(&d.id) {
                                meta.size = Some(d.size);
                                meta.modified = Some(d.modified);
                            }
                        } else {
                            // Neue Session-unabhängige Einträge hinzufügen (nur Meta minimal)
                            self.downloads.insert(
                                d.id.clone(),
                                DownloadState {
                                    finished: true,
                                    path: Some(d.path.clone()),
                                    ..Default::default()
                                },
                            );
                            self.download_order.push(d.id.clone());
                            self.download_meta.insert(
                                d.id.clone(),
                                DownloadMeta {
                                    id: d.id.clone(),
                                    name: d.name.clone(),
                                    info: d.info.clone(),
                                    container_extension: d.container_extension.clone(),
                                    size: Some(d.size),
                                    modified: Some(d.modified),
                                },
                            );
                        }
                    }
                }
                Msg::SearchResults {
                    query: _query,
                    results,
                } => {
                    // Convert search results to content rows
                    let mut rows = Vec::new();
                    for item in results {
                        rows.push(Row {
                            name: item.name.clone(),
                            id: item.id.clone(),
                            info: format!(
                                "Year: {} - Rating: {:.1}/5",
                                item.year.clone().unwrap_or("N/A".to_string()),
                                item.rating_5based.unwrap_or(0.0)
                            ),
                            container_extension: Some(item.container_extension.clone()),
                            stream_url: item.stream_url.clone(),
                            cover_url: item.cover.clone(),
                            year: item.year.clone(),
                            release_date: item.release_date.clone().or_else(|| extract_year_from_title(&item.name)).or_else(|| item.year.clone()),
                            rating_5based: item.rating_5based,
                            genre: item.genre.clone(),
                            path: None,
                        });
                    }
                    self.content_rows = rows;
                    self.is_loading = false;
                }

                Msg::WisdomGateRecommendations(content) => {
                    // Update cache with new content (only if it's not an error or demo content)
                    if !content.starts_with("API Fehler") && !content.starts_with("🌐 **Offline-Modus**") {
                        self.config.update_wisdom_gate_cache(content.clone());
                        // Save config to persist cache
                        if let Err(e) = crate::config::write_config(&self.config) {
                            println!("⚠️ Fehler beim Speichern des Caches: {}", e);
                        } else {
                            println!("💾 Cache erfolgreich gespeichert");
                        }
                    }
                    
                    self.wisdom_gate_recommendations = Some(content);
                    self.wisdom_gate_last_fetch = Some(std::time::Instant::now());
                }
            }
        }
        // Decide if we trigger a repaint now (mouse move shouldn't force full re-render if nothing changed)
    let time_since_forced = now.duration_since(self.last_forced_repaint).as_millis() as u64;
    // Ultra Flicker Guard erhöht Intervall und erzwingt ausschließlich Event-basierte Repaints
    let repaint_interval = if self.config.ultra_low_flicker_mode { 900 } else if self.config.low_cpu_mode { 500 } else { 120 }; // base cadence
        if self.pending_repaint_due_to_msg || got_msg {
            ctx.request_repaint();
            self.last_forced_repaint = now;
            self.pending_repaint_due_to_msg = false;
        } else if time_since_forced >= repaint_interval {
            // periodic heartbeat only if background work exists
            if !self.config.ultra_low_flicker_mode && (has_critical_bg_work || has_minor_bg_work) {
                ctx.request_repaint();
                self.last_forced_repaint = now;
            }
        }

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
        if got_msg {
            // Only repaint for critical loading states, and much less frequently
            if self.is_loading {
                ctx.request_repaint_after(Duration::from_millis(1000)); // 1 second instead of 50ms!
            }
            // No automatic repaints for content updates - let user interaction drive them
        }

        // Verarbeite pro Frame nur ein kleines Budget an Texture-Uploads,
        // um Frame-Drops beim Scrollen zu vermeiden.
        {
            let max_uploads_per_frame: usize =
                self.config.cover_uploads_per_frame.max(1).min(16) as usize;
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
                    let mut use_mpv = self.config.use_mpv;
                    ui.add_enabled_ui(self.has_mpv, |ui| {
                        if ui.checkbox(&mut use_mpv, "Use MPV").on_hover_text(if self.has_mpv { "Statt VLC den mpv Player verwenden" } else { "mpv nicht gefunden (brew install mpv)" }).changed() {
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
                        ui.label(egui::RichText::new(format!("MPV: {}", shorten(&mpv_preview))).small()).on_hover_text(if self.has_mpv { format!("MPV Cache Mapping: cache-secs={} readahead basiert auf Bias/Overrides", if self.config.mpv_cache_secs_override!=0 { self.config.mpv_cache_secs_override.to_string() } else { ((n/1000).max(1)).to_string() }) } else { "mpv nicht verfügbar".into() });
                        if self.config.low_cpu_mode {
                            ui.label(egui::RichText::new(format!("Pending tex:{} covers:{} decodes:{} dl:{}", self.pending_texture_uploads.len(), self.pending_covers.len(), self.pending_decode_urls.len(), self.active_downloads())).small()).on_hover_text("Debug Statistiken im Low-CPU Mode");
                        }
                    });
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
                        if ui.button("Search").clicked() {
                            if let Some(cv) = &self.current_view {
                                self.view_stack.push(cv.clone());
                            }
                            self.current_view = Some(ViewState::Search {
                                query: self.search_text.clone(),
                            });
                            self.start_search();
                        }
                        let resp = egui::TextEdit::singleline(&mut self.search_text)
                            .hint_text("Search…")
                            .desired_width(220.0)
                            .lock_focus(true)
                            .show(ui);
                        if resp.response.ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Some(cv) = &self.current_view {
                                self.view_stack.push(cv.clone());
                            }
                            self.current_view = Some(ViewState::Search {
                                query: self.search_text.clone(),
                            });
                            self.start_search();
                            resp.response.request_focus();
                        }
                    });
                    if self.indexing {
                        render_loading_spinner(ui, "Indexing");
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
                                    if let Some(cv) = &self.current_view {
                                        self.view_stack.push(cv.clone());
                                    }
                                    self.current_view = Some(ViewState::Items {
                                        kind: "subplaylist".into(),
                                        category_id: c.id.clone(),
                                    });
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
                                    if let Some(cv) = &self.current_view {
                                        self.view_stack.push(cv.clone());
                                    }
                                    self.current_view = Some(ViewState::Items {
                                        kind: "vod".into(),
                                        category_id: c.id.clone(),
                                    });
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
                                    if let Some(cv) = &self.current_view {
                                        self.view_stack.push(cv.clone());
                                    }
                                    self.current_view = Some(ViewState::Items {
                                        kind: "series".into(),
                                        category_id: c.id.clone(),
                                    });
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
                                            ui.label(format!("{} ({})", it.name, it.info));
                                            if it.info == "Series" {
                                                if ui.small_button("Episodes").clicked() {
                                                    self.is_loading = true;
                                                    self.loading_total = 1;
                                                    self.loading_done = 0;
                                                    self.spawn_load_episodes(it.id.clone());
                                                }
                                            } else {
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
                                                if let Some(sz)=size_opt { ui.weak(format_file_size(Some(sz))); }
                                                if waiting { ui.weak("waiting"); }
                                                else if finished {
                                                    if let Some(err)=error_opt.as_ref(){ ui.label(colored_text_by_type(&format!("error: {}",err),"error")); }
                                                    else { ui.label(colored_text_by_type("done","success")); }
                                                } else {
                                                    let frac = total_opt.map(|t| (received as f32 / t as f32).min(1.0)).unwrap_or(0.0);
                                                    let pct_text = if total_opt.is_some(){ format!("{:.0}%", frac*100.0) } else { format!("{} KB", received/1024) };
                                                    // Geschwindigkeiten (aktuell & Durchschnitt)
                                                    let cur_speed = if cur_speed_bps > 0.0 { crate::downloads::format_speed(cur_speed_bps) } else { "-".into() };
                                                    let avg_speed = if avg_speed_bps > 0.0 { crate::downloads::format_speed(avg_speed_bps) } else { "-".into() };
                                                    let bar_text = format!("{} | {} / avg {}", pct_text, cur_speed, avg_speed);
                                                    ui.add(egui::ProgressBar::new(frac).desired_width(160.0).text(bar_text));
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
                    SortKey::ReleaseDate => {
                        fn parse_year(y: &Option<String>) -> i32 {
                            y.as_deref()
                                .and_then(|s| s.parse::<i32>().ok())
                                .unwrap_or(0)
                        }
                        rows.sort_by(|a, b| parse_year(&a.release_date).cmp(&parse_year(&b.release_date)));
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
                .column(egui_extras::Column::initial(100.0)) // Release Date
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
                        let selected = self.sort_key == Some(SortKey::ReleaseDate);
                        let label = if selected {
                            format!("Release Date {}", if self.sort_asc { "▲" } else { "▼" })
                        } else {
                            "Release Date".to_string()
                        };
                        if ui.small_button(label).clicked() {
                            if selected {
                                self.sort_asc = !self.sort_asc;
                            } else {
                                self.sort_key = Some(SortKey::ReleaseDate);
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
                                    if let Some(cv) = &self.current_view {
                                        self.view_stack.push(cv.clone());
                                    }
                                    self.current_view = Some(ViewState::Episodes {
                                        series_id: r.id.clone(),
                                    });
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
                            ui.label(r.release_date.clone().unwrap_or_default());
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
                                        if let Some(cv) = &self.current_view {
                                            self.view_stack.push(cv.clone());
                                        }
                                        self.current_view = Some(ViewState::Episodes {
                                            series_id: r.id.clone(),
                                        });
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                        self.spawn_load_episodes(r.id.clone());
                                    }
                                    // Für Series kein direktes File, aber wir bieten Download (öffnet Episoden zum Downloaden)
                                    if self.config.enable_downloads
                                        && ui
                                            .small_button("Download all")
                                            .on_hover_text("Queue all episodes for download")
                                            .clicked()
                                    {
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
                                            let _ = start_player(self.effective_config(), &play_url);
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
                                    if r.info == "Movie"
                                        || r.info == "SeriesEpisode"
                                        || r.info == "Series"
                                        || r.info == "VOD"
                                    {
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
                                            } else if let Some(error) = &st.error {
                                                ui.label(colored_text_by_type(&format!("Download failed: {}", error), "error"));
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
                                            if self.config.enable_downloads
                                                && ui.small_button("Download").clicked()
                                            {
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
                                ui.label("URL");
                                ui.add(egui::TextEdit::singleline(&mut draft.address).desired_width(f32::INFINITY));
                                ui.label("Username");
                                ui.add(egui::TextEdit::singleline(&mut draft.username).desired_width(f32::INFINITY));
                                ui.label("Password");
                                ui.add(egui::TextEdit::singleline(&mut draft.password).password(true).desired_width(f32::INFINITY));
                            });

                            ui.collapsing("🎬 Player", |ui| {
                                ui.label("Custom Player Command (optional)");
                                ui.add(egui::TextEdit::multiline(&mut draft.player_command).desired_rows(2).desired_width(f32::INFINITY));
                                ui.small("Use {URL} placeholder where the stream URL goes");
                                ui.horizontal(|ui| {
                                    let mut reuse = draft.reuse_vlc; if ui.checkbox(&mut reuse, "Reuse VLC").on_hover_text("Open links in running VLC instance (macOS)").changed() { draft.reuse_vlc = reuse; }
                                    let mut use_mpv = draft.use_mpv; if ui.checkbox(&mut use_mpv, "Use MPV").on_hover_text(if self.has_mpv {"mpv aktivieren"} else {"mpv nicht gefunden"}).changed() { draft.use_mpv = use_mpv; if use_mpv { draft.reuse_vlc = false; } }
                                    if self.has_vlc { if let Some(v)=&self.vlc_version { ui.label(egui::RichText::new(format!("vlc {}", v)).small()); }}
                                    if self.has_mpv { if let Some(v)=&self.mpv_version { ui.label(egui::RichText::new(format!("mpv {}", v)).small()); }}
                                });
                                ui.horizontal(|ui| {
                                    let mut low = draft.low_cpu_mode; if ui.checkbox(&mut low, "Low CPU").on_hover_text("Reduziert Repaints & Diagnose-Frequenz").changed() { draft.low_cpu_mode = low; }
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
                            
                            ui.add_space(8.0);
                            ui.heading("🎬 Player Einstellungen");
                            ui.separator();
                            // Kommando-Vorschau aktualisieren (Draft Config)
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
                            // Nur aktualisieren wenn sich der Wert wirklich geändert hat, um unnötige Repaints zu vermeiden
                            if self.command_preview != preview {
                                self.command_preview = preview;
                            }
                            ui.horizontal(|ui| {
                                let mut use_mpv = draft.use_mpv;
                                if ui.checkbox(&mut use_mpv, "MPV statt VLC verwenden").on_hover_text(if self.has_mpv { "mpv aktivieren" } else { "mpv nicht gefunden" }).changed() { draft.use_mpv = use_mpv; if use_mpv { draft.reuse_vlc = false; } }
                                if self.has_mpv { if let Some(v)=&self.mpv_version { ui.label(egui::RichText::new(format!("mpv: {}", v)).small()); }} else { ui.label(egui::RichText::new("mpv: not found").small()); }
                                if self.has_vlc { if let Some(v)=&self.vlc_version { ui.label(egui::RichText::new(format!("vlc: {}", v)).small()); }} else { ui.label(egui::RichText::new("vlc: not found").small()); }
                                let mut low = draft.low_cpu_mode;
                                if ui.checkbox(&mut low, "Low CPU Mode").on_hover_text("Reduziert Repaints & drosselt Diagnose-Thread").changed() { draft.low_cpu_mode = low; }
                                let mut ultra = draft.ultra_low_flicker_mode;
                                if ui.checkbox(&mut ultra, "Ultra Flicker Guard").on_hover_text("Noch weniger Repaints (nur bei Events/Heartbeat) – kann UI-Verzögerung erhöhen").changed() { draft.ultra_low_flicker_mode = ultra; }
                            });

                            // MPV Abschnitt
                            // (MPV Optionen moved inside Player collapsing)
                            // (Preview moved inside Player collapsing)

                            // VLC Abschnitt ausgegraut wenn MPV aktiv
                            ui.add_enabled_ui(!draft.use_mpv, |ui| {
                                ui.collapsing("VLC Optimierung & Diagnose", |ui| {
                                    ui.horizontal(|ui| {
                                        let mut verbose = draft.vlc_verbose;
                                        if ui.checkbox(&mut verbose, "Verbose (-vvv)").changed() { draft.vlc_verbose = verbose; }
                                        let mut diag_once = draft.vlc_diagnose_on_start;
                                        if ui.checkbox(&mut diag_once, "Diagnose einmalig").changed() { draft.vlc_diagnose_on_start = diag_once; }
                                        let mut cont_diag = draft.vlc_continuous_diagnostics;
                                        if ui.checkbox(&mut cont_diag, "Kontinuierliche Diagnose").changed() { draft.vlc_continuous_diagnostics = cont_diag; }
                                        if draft.vlc_continuous_diagnostics {
                                            if ui.button("Stop Diagnose").on_hover_text("Beendet die laufende kontinuierliche VLC Diagnose").clicked() {
                                                let _ = self.tx.send(Msg::StopDiagnostics);
                                            }
                                        }
                                    });
                                    if let Some(suggestion) = self.vlc_diag_suggestion {
                                        ui.horizontal(|ui| {
                                            ui.label(format!("Suggestion: net={} live={} file={}", suggestion.0, suggestion.1, suggestion.2));
                                            if ui.button("Anwenden").on_hover_text("Übernimmt Werte und speichert sie im Verlauf (max 10)").clicked() {
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
                                        ui.collapsing("Verlauf Vorschläge", |ui| {
                                            for seg in draft.vlc_diag_history.split(';').filter(|s| !s.is_empty()).rev() {
                                                let cols: Vec<&str> = seg.split(':').collect();
                                                if cols.len()==4 {
                                                    ui.label(format!("ts={} net={} live={} file={}", cols[0], cols[1], cols[2], cols[3]));
                                                }
                                            }
                                        });
                                    }
                                    ui.collapsing("VLC Diagnose Logs", |ui| {
                                        let text = self.vlc_diag_lines.iter().rev().take(40).cloned().collect::<Vec<_>>().join("\n");
                                        ui.add(egui::TextEdit::multiline(&mut text.clone()).desired_rows(8));
                                    });
                                });
                            });
                            // (Bias controls moved into Player collapsing)
                            
                            ui.horizontal_wrapped(|ui| {
                        if ui
                            .button("IPTV Optimized")
                            .on_hover_text("Apply VLC parameters optimized for IPTV/Xtream Codes streaming")
                            .clicked()
                        {
                            draft.player_command = crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Default, &draft);
                        }
                        if ui
                            .button("Live TV")
                            .on_hover_text("Minimal buffering for live TV channels")
                            .clicked()
                        {
                            draft.player_command = crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Live, &draft);
                        }
                        if ui
                            .button("VOD/Movies")
                            .on_hover_text("Larger buffer for better quality VOD playback")
                            .clicked()
                        {
                            draft.player_command = crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Vod, &draft);
                        }
                        if ui
                            .button("Minimal")
                            .on_hover_text("Minimal VLC parameters for maximum compatibility")
                            .clicked()
                        {
                            draft.player_command = "vlc --fullscreen {URL}".to_string();
                        }
                        // Show the currently effective command (with placeholder visible)
                        let preview = if draft.player_command.trim().is_empty() {
                            crate::player::get_vlc_command_for_stream_type(crate::player::StreamType::Default, &draft)
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
                    ui.collapsing("🧮 Buffering & Caching", |ui| {
                        ui.label("VLC buffer settings");
                    ui.horizontal(|ui| {
                        ui.label("Network caching (ms)");
                        let mut network = if draft.vlc_network_caching_ms == 0 { 10000 } else { draft.vlc_network_caching_ms } as i32;
                        if ui.add(egui::DragValue::new(&mut network).clamp_range(1000..=60000)).on_hover_text("Amount of network buffering in milliseconds for VLC (10s default for live TV stability)").changed() {
                            draft.vlc_network_caching_ms = network as u32;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Live caching (ms)");
                        let mut live = if draft.vlc_live_caching_ms == 0 { 5000 } else { draft.vlc_live_caching_ms } as i32;
                        if ui.add(egui::DragValue::new(&mut live).clamp_range(0..=30000)).on_hover_text("Additional live-specific caching in milliseconds (5s default)").changed() {
                            draft.vlc_live_caching_ms = live as u32;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Prefetch buffer (bytes)");
                        let mut prefetch = if draft.vlc_prefetch_buffer_bytes == 0 { 16 * 1024 * 1024 } else { draft.vlc_prefetch_buffer_bytes } as i64;
                        if ui.add(egui::DragValue::new(&mut prefetch).clamp_range(1024..=128 * 1024 * 1024)).on_hover_text("Prefetch buffer size in bytes used by VLC (16 MiB default for stability)").changed() {
                            draft.vlc_prefetch_buffer_bytes = prefetch as u64;
                        }
                    });
                    });
                    
                    ui.collapsing("🧠 AI Empfehlungen", |ui| {
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
                        ui.weak("API Key erforderlich für AI-Empfehlungen");
                    }
                    
                    ui.horizontal(|ui| {
                        ui.label("Model:");
                        egui::ComboBox::from_label("")
                            .selected_text(&draft.wisdom_gate_model)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut draft.wisdom_gate_model, "wisdom-ai-dsv3".to_string(), "Wisdom-AI DSV3 (Standard)");
                                ui.selectable_value(&mut draft.wisdom_gate_model, "deepseek-v3".to_string(), "DeepSeek V3");
                                ui.selectable_value(&mut draft.wisdom_gate_model, "gemini-2.5-flash".to_string(), "Gemini 2.5 Flash");
                                ui.selectable_value(&mut draft.wisdom_gate_model, "gemini-2.5-flash-image".to_string(), "Gemini 2.5 Flash (Vision)");
                                ui.selectable_value(&mut draft.wisdom_gate_model, "wisdom-ai-gemini-2.5-flash".to_string(), "Wisdom-AI Gemini 2.5 Flash");
                                ui.selectable_value(&mut draft.wisdom_gate_model, "wisdom-vision-gemini-2.5-flash-image".to_string(), "Wisdom Vision Gemini 2.5 Flash");
                                ui.selectable_value(&mut draft.wisdom_gate_model, "tts-1".to_string(), "TTS-1 (Text-to-Speech)");
                            });
                    });
                    
                    ui.label("Prompt für Empfehlungen:");
                    ui.add(
                        egui::TextEdit::multiline(&mut draft.wisdom_gate_prompt)
                            .desired_rows(3)
                            .hint_text("Was sind die besten Streaming-Empfehlungen für heute?")
                    );
                    
                    ui.horizontal(|ui| {
                        if ui.button("Standard Prompt").clicked() {
                            draft.wisdom_gate_prompt = crate::models::default_wisdom_gate_prompt();
                        }
                        ui.weak("Tipp: Frage nach aktuellen Filmen und Serien");
                    });
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
            }
            self.show_config = false;
            self.pending_save_config = false;
            self.config_draft = None;
        }

        // Separate Downloads Fenster entfällt durch Inline-Spalte; Flag wird ignoriert
        self.show_downloads = false;

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
