use eframe::egui::{self, Color32, RichText};
use egui_extras::TableBuilder;
use std::collections::HashSet;
use std::collections::{HashMap, HashSet as HashSet2};
use std::fs::{self, File};
use std::io::Read;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use tokio::sync::Semaphore;

mod api;
mod cache;
mod config;
mod icon;
mod models;
mod player;
mod search;
mod storage;
mod logger;

use api::{fetch_categories, fetch_items, fetch_series_episodes};
use cache::{file_age_secs, image_cache_path, clear_all_caches};
use config::{read_config, save_config};
use models::{Category, Config, Episode, FavItem, Item, RecentItem, Row};
use player::{build_url_by_type, start_player};
use search::search_items;
use storage::{add_to_recently, load_favorites, load_recently_played, toggle_favorite};

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
    IndexBuilt {
        movies: usize,
        series: usize,
    },
    SearchReady(Vec<Row>),
    IndexData {
        movies: Vec<Item>,
        series: Vec<Item>,
    },
    PreloadSet {
        total: usize,
    },
    PreloadTick,
    PrefetchCovers(Vec<String>),
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
    indexing: bool,
    sort_key: Option<SortKey>,
    sort_asc: bool,

    // Async messaging
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    show_log: bool,
    log_text: String,
}

impl MacXtreamer {
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
        self.all_movies.clear();
        self.all_series.clear();
        self.content_rows.clear();
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
        let config = read_config().unwrap_or_default();
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
            indexing: false,
            sort_key: None,
            sort_asc: true,
            tx,
            rx,
            show_log: false,
            log_text: String::new(),
        };
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
        if app.config.font_scale == 0.0 {
            app.config.font_scale = 1.15;
        }
        app.cover_sem = Arc::new(Semaphore::new(app.config.cover_parallel as usize));
        app.reload_categories();
        // Starte Preloading im Hintergrund
        app.spawn_preload_all();
        app
    }

    fn reload_categories(&mut self) {
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

    fn spawn_fetch_cover(&mut self, url: &str) {
        if self.pending_covers.contains(url) {
            return;
        }
        self.pending_covers.insert(url.to_string());
        let tx = self.tx.clone();
        let url_s = url.to_string();
        let sem = self.cover_sem.clone();
        let ttl_secs: u64 = (self.config.cover_ttl_days.max(1) as u64) * 24 * 60 * 60;
        tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.ok();
            // Versuche Disk-Cache mit TTL zuerst
            let mut served_any = false;
            let mut need_refresh = false;
            if let Some(path) = image_cache_path(&url_s) {
                if let Some(age) = file_age_secs(&path) {
                    if let Ok(mut f) = File::open(&path) {
                        let mut buf = Vec::new();
                        if f.read_to_end(&mut buf).is_ok() {
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
                if let Ok(resp) = reqwest::get(&url_s).await {
                    if let Ok(bytes) = resp.bytes().await {
                        let data = bytes.to_vec();
                        // Schreibe in Disk-Cache
                        if let Some(path) = image_cache_path(&url_s) {
                            if let Some(parent) = path.parent() {
                                let _ = fs::create_dir_all(parent);
                            }
                            let _ = fs::write(&path, &data);
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
                    if let Ok(mut f) = File::open(&path) {
                        let mut buf = Vec::new();
                        if f.read_to_end(&mut buf).is_ok() {
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
            let mut all_movies: Vec<Item> = Vec::new();
            let mut all_series: Vec<Item> = Vec::new();
            for c in vod {
                if let Ok(items) = fetch_items(&cfg, "vod", &c.id).await {
                    all_movies.extend(items);
                }
            }
            for c in ser {
                if let Ok(items) = fetch_items(&cfg, "series", &c.id).await {
                    all_series.extend(items);
                }
            }
            // Dedup by id
            let mut seen = std::collections::HashSet::new();
            all_movies.retain(|i| seen.insert(i.id.clone()));
            seen.clear();
            all_series.retain(|i| seen.insert(i.id.clone()));
            // Persist into cache files already handled by fetch_items; send data back
            let _ = tx.send(Msg::IndexBuilt {
                movies: all_movies.len(),
                series: all_series.len(),
            });
            let _ = tx.send(Msg::IndexData {
                movies: all_movies,
                series: all_series,
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
                })
                .collect();
            let _ = tx.send(Msg::SearchReady(rows));
        });
    }

    fn spawn_preload_all(&mut self) {
        // Nur einmal zu Beginn sinnvoll; löst Caching aller Kategorien/Items aus, inkl. Cover
        let cfg = self.config.clone();
        let tx = self.tx.clone();
        self.is_loading = true;
        self.loading_done = 0;
        self.loading_total = 0; // wird gleich gesetzt
        tokio::spawn(async move {
            // Kategorien
            let vod = fetch_categories(&cfg, "get_vod_categories")
                .await
                .unwrap_or_default();
            let ser = fetch_categories(&cfg, "get_series_categories")
                .await
                .unwrap_or_default();
            let live = fetch_categories(&cfg, "get_live_categories")
                .await
                .unwrap_or_default();

            // Gesamtzahl: alle Kategorien zählen + geschätzte Item-Abfragen pro Kategorie
            let total_steps = vod.len() + ser.len() + live.len();
            let _ = tx.send(Msg::PreloadSet {
                total: total_steps.max(1),
            });

            // Sammle Cover-URLs
            let mut cover_urls: Vec<String> = Vec::new();

            // Live-Streams: nur laden, kein Cover
            for c in live {
                let _ = fetch_items(&cfg, "subplaylist", &c.id).await; // Cache füllen
                let _ = tx.send(Msg::PreloadTick);
            }
            // VOD
            for c in vod {
                if let Ok(items) = fetch_items(&cfg, "vod", &c.id).await {
                    for it in &items {
                        if let Some(cu) = &it.cover {
                            cover_urls.push(cu.clone());
                        }
                    }
                }
                let _ = tx.send(Msg::PreloadTick);
            }
            // Serien
            for c in ser {
                if let Ok(items) = fetch_items(&cfg, "series", &c.id).await {
                    for it in &items {
                        if let Some(cu) = &it.cover {
                            cover_urls.push(cu.clone());
                        }
                    }
                }
                let _ = tx.send(Msg::PreloadTick);
            }
            // Cover prefetchen (doppelte entfernen)
            cover_urls.sort();
            cover_urls.dedup();
            let _ = tx.send(Msg::PrefetchCovers(cover_urls));
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
        // Schriftgröße skalieren (einmalig oder bei Wechsel)
        if !self.font_scale_applied {
            let mut style = (*ctx.style()).clone();
            style.text_styles.iter_mut().for_each(|(_, ts)| {
                ts.size *= self.config.font_scale.max(0.6).min(2.0);
            });
            ctx.set_style(style);
            self.font_scale_applied = true;
        }
        // Während Ladevorgängen regelmäßig neu zeichnen, damit Channel-Polling stattfindet
        if self.is_loading {
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
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
                                    self.spawn_load_items("subplaylist", cat.id.clone());
                                }
                            } else if let Some(i) = self.selected_playlist {
                                if i < self.playlists.len() {
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
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
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
                                    self.spawn_load_items("vod", cat.id.clone());
                                }
                            } else if let Some(i) = self.selected_vod {
                                if i < self.vod_categories.len() {
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
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
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
                                    self.spawn_load_items("series", cat.id.clone());
                                }
                            } else if let Some(i) = self.selected_series {
                                if i < self.series_categories.len() {
                                    self.is_loading = true;
                                    self.loading_total = 1;
                                    self.loading_done = 0;
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
                                });
                            }
                            self.is_loading = false;
                        }
                        Err(e) => {
                            self.is_loading = false;
                            self.last_error = Some(e);
                        }
                    }
                    self.loading_done = self.loading_total.min(self.loading_done + 1);
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
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let rgba = img.to_rgba8();
                        let (w, h) = rgba.dimensions();
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            &rgba,
                        );
                        let tex = ctx.load_texture(
                            url.clone(),
                            color_image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.textures.insert(url.clone(), tex);
                    }
                    self.pending_covers.remove(&url);
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
                    self.all_movies = movies;
                    self.all_series = series;
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
                Msg::SearchReady(rows) => {
                    self.content_rows = rows;
                    self.is_loading = false;
                }
            }
        }

        // Wenn Nachrichten eingetroffen sind oder wir laden, sicherstellen, dass ein weiterer Frame kommt
        if got_msg || self.is_loading {
            ctx.request_repaint();
        }

        let win_h = ctx.input(|i| i.screen_rect().height());
        egui::TopBottomPanel::top("top")
            .resizable(true)
            .show_separator_line(true)
            .default_height(win_h / 3.0)
            .show(ctx, |ui| {
                // Kopfzeile mit Aktionen und Suche
                ui.horizontal(|ui| {
                    ui.heading("MacXtreamer");
                    if ui.button("Reload").clicked() {
                        // Clear disk + memory caches and force a full fresh reload
                        self.clear_caches_and_reload();
                    }
                    if ui.button("Open Log").clicked() {
                        // Read log file and open viewer
                        let path = crate::logger::log_path();
                        self.log_text = std::fs::read_to_string(path).unwrap_or_else(|_| "(no log)".into());
                        self.show_log = true;
                    }
                    // Reuse VLC toggle
                    let mut reuse = self.config.reuse_vlc;
                    if ui.checkbox(&mut reuse, "Reuse VLC").on_hover_text("Open URLs in the already running VLC instance").changed() {
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
                    ui.label("Cover height");
                    ui.add(egui::Slider::new(&mut self.cover_height, 40.0..=120.0).step_by(2.0));
                    if self.is_loading {
                        let pct = if self.loading_total > 0 {
                            (self.loading_done * 100 / self.loading_total).min(100)
                        } else {
                            0
                        };
                        ui.label(format!("Loading… {}%", pct));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Search").clicked() {
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
                                    self.selected_playlist = Some(i);
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
                                    self.selected_vod = Some(i);
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
                                    self.selected_series = Some(i);
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

        // (Bottom-Panel wird nach dem CentralPanel gerendert)

        egui::CentralPanel::default().show(ctx, |ui| {
            // Use the full available width; height is controlled by the panel layout
            let avail_w = ui.available_width();
            ui.set_width(avail_w);
            // Table should take the full available height
            let avail_h = ui.available_height();
            ui.set_min_height(avail_h);
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
                .min_scrolled_height(avail_h)
                .column(egui_extras::Column::initial(cover_w + 16.0)) // Cover
                .column(egui_extras::Column::initial(400.0).at_least(400.0)) // Name (min 400px, resizable)
                .column(egui_extras::Column::initial(140.0)) // ID
                .column(egui_extras::Column::initial(120.0)) // Info
                .column(egui_extras::Column::initial(80.0)) // Year
                .column(egui_extras::Column::initial(80.0)) // Rating
                .column(egui_extras::Column::initial(200.0)) // Genre (resizable)
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
                            ui.horizontal_wrapped(|ui| {
                                if r.info == "Series" {
                                    if ui.small_button("Episodes").clicked() {
                                        self.is_loading = true;
                                        self.loading_total = 1;
                                        self.loading_done = 0;
                                        self.spawn_load_episodes(r.id.clone());
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
                                            let _ = start_player(&self.config, &url);
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
                                    if r.info == "SeriesEpisode" {
                                        if ui.small_button("binge watch since here").clicked() {
                                            // Build playlist from the currently visible/sorted rows starting at i
                                            let mut entries: Vec<(String, String)> = Vec::new();
                                            for rr in rows.iter().skip(i) {
                                                if rr.info == "SeriesEpisode" {
                                                    let u = build_url_by_type(
                                                        &self.config,
                                                        &rr.id,
                                                        &rr.info,
                                                        rr.container_extension.as_deref(),
                                                    );
                                                    entries.push((rr.name.clone(), u));
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
        });

        // (Bottom-Panel folgt gleich)

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
                if ui.checkbox(&mut reuse, "Reuse VLC").on_hover_text("Open links in a running VLC instance (macOS)").changed() {
                    draft.reuse_vlc = reuse;
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Apply VLC defaults").on_hover_text("Apply sensible VLC parameters for streaming").clicked() {
                    draft.player_command = "vlc --fullscreen --no-video-title-show --network-caching=2000 URL".to_string();
                }
                // Show the currently effective command (with placeholder visible)
                let preview = if draft.player_command.trim().is_empty() {
                    "vlc --fullscreen --no-video-title-show --network-caching=2000 URL".to_string()
                } else {
                    draft.player_command.clone()
                };
                ui.label(egui::RichText::new(format!("Current: {}", preview)).weak());
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Cover TTL (days)");
                let mut ttl = if draft.cover_ttl_days == 0 { 7 } else { draft.cover_ttl_days } as i32;
                if ui.add(egui::DragValue::new(&mut ttl).clamp_range(1..=30)).changed() {
                    draft.cover_ttl_days = ttl as u32;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Cover parallelism");
                let mut par = if draft.cover_parallel == 0 { 6 } else { draft.cover_parallel } as i32;
                if ui.add(egui::DragValue::new(&mut par).clamp_range(1..=16)).changed() {
                    draft.cover_parallel = par as u32;
                }
            });
        ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    if let Some(d) = &self.config_draft { self.config = d.clone(); }
            // Persist theme setting
            self.config.theme = if self.current_theme.is_empty() { "dark".into() } else { self.current_theme.clone() };
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
                            self.log_text = std::fs::read_to_string(path).unwrap_or_else(|_| "(no log)".into());
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

        // (kein zweites Bottom-Panel)

        // Unterer Bereich: Recently Played und Favorites (am Ende zeichnen, damit Separator obenliegt)
        egui::TopBottomPanel::bottom("bottom")
            .resizable(true)
            .show_separator_line(true)
            .default_height(320.0)
            .min_height(120.0)
            .show(ctx, |ui| {
                // Visible grab bar directly under the separator
                let grip_h = 6.0;
                let full = ui.max_rect();
                let grip_rect = egui::Rect::from_min_max(
                    egui::pos2(full.min.x, full.min.y),
                    egui::pos2(full.max.x, full.min.y + grip_h),
                );
                let grip_color = ui.visuals().selection.bg_fill;
                ui.painter().rect_filled(grip_rect, 0.0, grip_color);
                ui.add_space(grip_h + 4.0);
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
                                        if ui.button(format!("{} ({})", it.name, it.info)).clicked()
                                        {
                                            // Use builder to ensure URL matches current config and extension (esp. SeriesEpisode)
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
                                                // Rebuild URL using current config; prefer stored container_extension if present
                                                let url = it
                                                    .stream_url
                                                    .clone()
                                                    .unwrap_or_else(|| {
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
            self.reload_categories();
            self.show_config = false;
            self.pending_save_config = false;
            self.config_draft = None;
        }
    }
}

// (Hilfs-Module für Config/Cache/API/Player/Storage/Suche sind ausgelagert)
