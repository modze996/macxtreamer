#![allow(dead_code)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::downloads::{DownloadManager, DownloadMsg, BulkOptions};
use crate::images::ImageManager;
use crate::models::{Category, Config, Episode, FavItem, Item, RecentItem, Row};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Year,
    ReleaseDate,
    Rating,
    Genre,
    Languages,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchStatus {
    Idle,
    Indexing { progress: String },
    Searching,
    NoResults,
    Error(String),
    Completed { results: usize },
}

#[derive(Debug, Clone)]
pub enum Msg {
    // Category loading
    LiveCategories(Result<Vec<Category>, String>),
    VodCategories(Result<Vec<Category>, String>),
    SeriesCategories(Result<Vec<Category>, String>),
    
    // Item and episode loading
    ItemsLoaded {
        kind: String,
        items: Result<Vec<Item>, String>,
    },
    EpisodesLoaded {
        series_id: String,
        episodes: Result<Vec<Episode>, String>,
    },
    
    // Cover/image loading
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
    
    // Search and indexing
    IndexBuilt {
        movies: usize,
        series: usize,
        channels: usize,
    },
    IndexProgress { message: String },
    SearchReady(Vec<Row>),
    SearchStarted,
    SearchCompleted { results: usize },
    SearchFailed { error: String },
    IndexData {
        movies: Vec<(Item, String)>,
        series: Vec<(Item, String)>,
        channels: Vec<(Item, String)>,
    },
    
    // Preloading
    PreloadSet {
        total: usize,
    },
    PreloadTick,
    PrefetchCovers(Vec<String>),
    
    // Downloads
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
    DownloadsScanned(Vec<crate::ScannedDownload>),
    
    // Additional variants
    SearchResults {
        query: String,
        results: Vec<Item>,
    },

    WisdomGateRecommendations(String), // AI recommendations content
    RecentlyAddedItems(Vec<Item>), // Recently added VOD/Series items
    VlcDiagnostics(String), // Captured VLC diagnostic output (truncated)
    VlcDiagUpdate { lines: Vec<String>, suggestion: Option<(u32,u32,u32)> },
    PlayerDetection { has_vlc: bool, has_mpv: bool, vlc_version: Option<String>, mpv_version: Option<String>, vlc_path: Option<String>, mpv_path: Option<String> },
    PlayerSpawnFailed { player: String, error: String },
    StopDiagnostics,
    DiagnosticsStopped,
    LoadingError(String), // Error during loading operations
    
    // Update system
    UpdateAvailable(crate::updater::UpdateInfo),
    NoUpdateAvailable,
    UpdateError(String),
    UpdateInstalled,
    
    // EPG data
    EpgLoaded {
        stream_id: String,
        program: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum ViewState {
    Items { kind: String, category_id: String },
    Episodes { series_id: String },
    Search { query: String },
}

/// Central application state manager
pub struct AppState {
    // Core configuration
    pub config: Config,
    
    // Navigation and view state
    pub view_state: Option<ViewState>,
    pub current_kind: String,
    pub current_category_id: String,
    pub current_series_id: String,
    
    // Data state
    pub categories: HashMap<String, Vec<Category>>,
    pub items: HashMap<String, Vec<Item>>,
    pub episodes: HashMap<String, Vec<Episode>>,
    pub series_info: HashMap<String, HashMap<String, serde_json::Value>>,
    pub search_results: Vec<Item>,
    
    // UI state
    pub selected_items: HashSet<String>,
    pub sort_key: Option<SortKey>,
    pub sort_ascending: bool,
    pub search_query: String,
    pub filter_text: String,
    pub show_favorites_only: bool,
    pub show_settings: bool,
    pub settings_draft: Option<Config>,
    
    // Loading and error states
    pub loading_categories: HashMap<String, bool>,
    pub loading_items: HashMap<String, bool>,
    pub loading_episodes: HashMap<String, bool>,
    pub error_message: Option<String>,
    pub last_error: Option<String>,
    
    // Background processing
    pub message_tx: Option<Sender<Msg>>,
    pub message_rx: Option<Receiver<Msg>>,
    
    // Storage
    pub favorites: Vec<FavItem>,
    pub recently_played: VecDeque<RecentItem>,
    
    // Font and UI
    pub current_font_scale: f32,
    
    // Download management
    pub download_manager: DownloadManager,
    pub bulk_options: BulkOptions,
    pub show_downloads: bool,
    
    // Image management  
    pub image_manager: ImageManager,
    
    // Background tasks
    pub stop_loading: Arc<AtomicBool>,
    pub vlc_diag_lines: VecDeque<String>,
    pub vlc_diag_suggestion: Option<(u32,u32,u32)>,
}

impl Default for AppState {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        
        Self {
            config: Config::default(),
            view_state: None,
            current_kind: "live".to_string(),
            current_category_id: String::new(),
            current_series_id: String::new(),
            categories: HashMap::new(),
            items: HashMap::new(),
            episodes: HashMap::new(),
            series_info: HashMap::new(),
            search_results: Vec::new(),
            selected_items: HashSet::new(),
            sort_key: None,
            sort_ascending: true,
            search_query: String::new(),
            filter_text: String::new(),
            show_favorites_only: false,
            show_settings: false,
            settings_draft: None,
            loading_categories: HashMap::new(),
            loading_items: HashMap::new(),
            loading_episodes: HashMap::new(),
            error_message: None,
            last_error: None,
            message_tx: Some(tx),
            message_rx: Some(rx),
            favorites: Vec::new(),
            recently_played: VecDeque::new(),
            current_font_scale: 1.0,
            download_manager: DownloadManager::default(),
            bulk_options: BulkOptions::default(),
            show_downloads: false,
            image_manager: ImageManager::default(),
            stop_loading: Arc::new(AtomicBool::new(false)),
            vlc_diag_lines: VecDeque::with_capacity(128),
            vlc_diag_suggestion: None,
        }
    }
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        
        Self {
            current_font_scale: config.font_scale,
            config,
            message_tx: Some(tx),
            message_rx: Some(rx),
            ..Default::default()
        }
    }

    /// Process incoming messages from background tasks
    pub fn process_messages(&mut self) -> Vec<Msg> {
        let mut messages = Vec::new();
        
        if let Some(ref rx) = self.message_rx {
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }
        }
        
        // Also check download manager messages
        while let Some(download_msg) = self.download_manager.try_receive_message() {
            match download_msg {
                DownloadMsg::Progress { id, downloaded, total, speed_bps } => {
                    if let Some(download) = self.download_manager.get_download_state(&id) {
                        let mut updated = download.clone();
                        updated.downloaded_bytes = downloaded;
                        updated.total_bytes = total;
                        updated.speed_bps = speed_bps;
                        updated.progress = crate::downloads::calculate_progress(downloaded, total);
                        
                        if let Some(total_bytes) = total {
                            if total_bytes > 0 && downloaded > 0 {
                                let remaining_bytes = total_bytes - downloaded;
                                updated.eta_seconds = if speed_bps > 0.0 {
                                    Some((remaining_bytes as f64 / speed_bps) as u64)
                                } else {
                                    None
                                };
                            }
                        }
                        
                        self.download_manager.update_download_state(id, updated);
                    }
                }
                DownloadMsg::Completed { id, filepath: _ } => {
                    if let Some(download) = self.download_manager.get_download_state(&id) {
                        let mut updated = download.clone();
                        updated.status = crate::downloads::DownloadStatus::Completed;
                        updated.progress = 1.0;
                        self.download_manager.update_download_state(id, updated);
                    }
                }
                DownloadMsg::Failed { id, error } => {
                    if let Some(download) = self.download_manager.get_download_state(&id) {
                        let mut updated = download.clone();
                        updated.status = crate::downloads::DownloadStatus::Failed(error);
                        self.download_manager.update_download_state(id, updated);
                    }
                }
                DownloadMsg::DownloadCancelled { id } => {
                    if let Some(download) = self.download_manager.get_download_state(&id) {
                        let mut updated = download.clone();
                        updated.status = crate::downloads::DownloadStatus::Cancelled;
                        self.download_manager.update_download_state(id, updated);
                    }
                }
                DownloadMsg::DownloadsScanned(scanned) => {
                    // Handle scanned downloads - could be processed into a message
                    messages.push(Msg::SearchResults {
                        query: "scanned_downloads".to_string(),
                        results: scanned.into_iter().map(|s| s.item).collect(),
                    });
                }
            }
        }
        
        messages
    }

    /// Get sender for background tasks to communicate back
    pub fn get_message_sender(&self) -> Option<Sender<Msg>> {
        self.message_tx.clone()
    }

    /// Check if currently loading any data
    pub fn is_loading(&self) -> bool {
        self.loading_categories.values().any(|&loading| loading)
            || self.loading_items.values().any(|&loading| loading) 
            || self.loading_episodes.values().any(|&loading| loading)
    }

    /// Set loading state for categories
    pub fn set_loading_categories(&mut self, kind: &str, loading: bool) {
        self.loading_categories.insert(kind.to_string(), loading);
    }

    /// Set loading state for items
    pub fn set_loading_items(&mut self, key: &str, loading: bool) {
        self.loading_items.insert(key.to_string(), loading);
    }

    /// Set loading state for episodes
    pub fn set_loading_episodes(&mut self, series_id: &str, loading: bool) {
        self.loading_episodes.insert(series_id.to_string(), loading);
    }

    /// Check if specific data is loading
    pub fn is_loading_categories(&self, kind: &str) -> bool {
        self.loading_categories.get(kind).copied().unwrap_or(false)
    }

    pub fn is_loading_items(&self, key: &str) -> bool {
        self.loading_items.get(key).copied().unwrap_or(false)
    }

    pub fn is_loading_episodes(&self, series_id: &str) -> bool {
        self.loading_episodes.get(series_id).copied().unwrap_or(false)
    }

    /// Get filtered and sorted items based on current UI state
    pub fn get_filtered_items(&self, key: &str) -> Vec<Item> {
        let items = self.items.get(key).cloned().unwrap_or_default();
        
        let mut filtered: Vec<Item> = items
            .into_iter()
            .filter(|item| {
                // Apply text filter
                if !self.filter_text.is_empty() {
                    if !item.name.to_lowercase().contains(&self.filter_text.to_lowercase()) {
                        return false;
                    }
                }
                
                // Apply favorites filter
                if self.show_favorites_only {
                    if !self.favorites.iter().any(|fav| fav.id == item.id) {
                        return false;
                    }
                }
                
                true
            })
            .collect();

        // Apply sorting
        if let Some(ref sort_key) = self.sort_key {
            filtered.sort_by(|a, b| {
                let ordering = match sort_key {
                    SortKey::Name => a.name.cmp(&b.name),
                    SortKey::Year => a.year.as_deref().unwrap_or("").cmp(b.year.as_deref().unwrap_or("")),
                    SortKey::ReleaseDate => a.year.as_deref().unwrap_or("").cmp(b.year.as_deref().unwrap_or("")), // Fallback to year for now
                    SortKey::Rating => a.rating_5based.unwrap_or(0.0).partial_cmp(&b.rating_5based.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal),
                    SortKey::Genre => a.genre.as_deref().unwrap_or("").cmp(b.genre.as_deref().unwrap_or("")),
                    SortKey::Languages => a.audio_languages.as_deref().unwrap_or("").cmp(b.audio_languages.as_deref().unwrap_or("")),
                };
                
                if self.sort_ascending {
                    ordering
                } else {
                    ordering.reverse()
                }
            });
        }
        
        filtered
    }

    /// Update configuration and save
    pub fn update_config(&mut self, new_config: Config) {
        self.config = new_config;
        if let Err(e) = crate::config::save_config(&self.config) {
            self.error_message = Some(format!("Failed to save config: {}", e));
        }
    }

    /// Clear all cached data
    pub fn clear_all_data(&mut self) {
        self.categories.clear();
        self.items.clear();
        self.episodes.clear();
        self.series_info.clear();
        self.search_results.clear();
        self.selected_items.clear();
        self.image_manager.clear_texture_cache();
        self.image_manager.clear_failed_images();
    }

    /// Get statistics about current state
    pub fn get_stats(&self) -> AppStats {
        AppStats {
            categories_count: self.categories.values().map(|v| v.len()).sum(),
            items_count: self.items.values().map(|v| v.len()).sum(),
            episodes_count: self.episodes.values().map(|v| v.len()).sum(),
            favorites_count: self.favorites.len(),
            recently_played_count: self.recently_played.len(),
            downloads_count: self.download_manager.get_all_downloads().len(),
            image_cache_stats: self.image_manager.get_cache_stats(),
        }
    }

    /// Reset error state
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    /// Set error message
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }
}

pub struct AppStats {
    pub categories_count: usize,
    pub items_count: usize,
    pub episodes_count: usize,
    pub favorites_count: usize,
    pub recently_played_count: usize,
    pub downloads_count: usize,
    pub image_cache_stats: crate::images::ImageCacheStats,
}
