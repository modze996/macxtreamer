#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use tokio::sync::Semaphore;

use crate::models::Item;

#[derive(Debug, Clone, Default)]
pub struct BulkOptions {
    pub only_not_downloaded: bool,
    pub season: Option<u32>,
    pub max_count: u32, // 0 = all
}

#[derive(Debug, Clone)]
pub struct DownloadState {
    pub id: String,
    pub title: String,
    pub url: String,
    pub filename: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub progress: f32, // 0.0 to 1.0
    pub status: DownloadStatus,
    pub speed_bps: f64,
    pub eta_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed(String),
    Cancelled,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            url: String::new(),
            filename: String::new(),
            total_bytes: None,
            downloaded_bytes: 0,
            progress: 0.0,
            status: DownloadStatus::Queued,
            speed_bps: 0.0,
            eta_seconds: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadMeta {
    pub total_files: usize,
    pub completed_files: usize,
    pub failed_files: usize,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ScannedDownload {
    pub id: String,
    pub title: String,
    pub filename: String,
    pub size_bytes: Option<u64>,
    pub exists: bool,
    pub item: Item,
}

pub enum DownloadMsg {
    Progress {
        id: String,
        downloaded: u64,
        total: Option<u64>,
        speed_bps: f64,
    },
    Completed {
        id: String,
        filepath: PathBuf,
    },
    Failed {
        id: String,
        error: String,
    },
    DownloadCancelled {
        id: String,
    },
    DownloadsScanned(Vec<ScannedDownload>),
}

/// Manages download operations and state
pub struct DownloadManager {
    downloads: HashMap<String, DownloadState>,
    download_tx: Option<Sender<DownloadMsg>>,
    download_rx: Option<Receiver<DownloadMsg>>,
    download_semaphore: Arc<Semaphore>,
    cancel_flag: Arc<AtomicBool>,
}

impl Default for DownloadManager {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            downloads: HashMap::new(),
            download_tx: Some(tx),
            download_rx: Some(rx),
            download_semaphore: Arc::new(Semaphore::new(3)), // Default concurrent downloads
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl DownloadManager {
    pub fn new(concurrent_downloads: usize) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            downloads: HashMap::new(),
            download_tx: Some(tx),
            download_rx: Some(rx),
            download_semaphore: Arc::new(Semaphore::new(concurrent_downloads)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn get_download_state(&self, id: &str) -> Option<&DownloadState> {
        self.downloads.get(id)
    }

    pub fn get_all_downloads(&self) -> &HashMap<String, DownloadState> {
        &self.downloads
    }

    pub fn update_download_state(&mut self, id: String, state: DownloadState) {
        self.downloads.insert(id, state);
    }

    pub fn remove_download(&mut self, id: &str) -> Option<DownloadState> {
        self.downloads.remove(id)
    }

    pub fn cancel_all_downloads(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    pub fn reset_cancel_flag(&self) {
        self.cancel_flag.store(false, Ordering::Relaxed);
    }

    pub fn get_sender(&self) -> Option<Sender<DownloadMsg>> {
        self.download_tx.clone()
    }

    pub fn try_receive_message(&mut self) -> Option<DownloadMsg> {
        if let Some(ref rx) = self.download_rx {
            rx.try_recv().ok()
        } else {
            None
        }
    }

    pub fn get_download_stats(&self) -> DownloadMeta {
        let total_files = self.downloads.len();
        let mut completed_files = 0;
        let mut failed_files = 0;
        let mut total_bytes = 0;
        let mut downloaded_bytes = 0;

        for download in self.downloads.values() {
            match download.status {
                DownloadStatus::Completed => completed_files += 1,
                DownloadStatus::Failed(_) => failed_files += 1,
                _ => {}
            }
            if let Some(total) = download.total_bytes {
                total_bytes += total;
            }
            downloaded_bytes += download.downloaded_bytes;
        }

        DownloadMeta {
            total_files,
            completed_files,
            failed_files,
            total_bytes,
            downloaded_bytes,
        }
    }
}

/// Check if a file has been downloaded based on filename patterns
pub fn is_already_downloaded(item: &Item, download_dir: &str) -> bool {
    let expected_filename = sanitize_filename(&item.name);
    let download_path = PathBuf::from(download_dir);
    
    // Check for various file extensions
    let extensions = ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v"];
    
    for ext in &extensions {
        let file_path = download_path.join(format!("{}.{}", expected_filename, ext));
        if file_path.exists() {
            return true;
        }
    }
    
    false
}

/// Sanitize filename for safe file system usage
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Calculate download progress as a percentage
pub fn calculate_progress(downloaded: u64, total: Option<u64>) -> f32 {
    match total {
        Some(total_bytes) if total_bytes > 0 => {
            (downloaded as f64 / total_bytes as f64).min(1.0) as f32
        }
        _ => 0.0,
    }
}

/// Format bytes into human-readable format
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Format download speed into human-readable format
pub fn format_speed(bytes_per_second: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_second as u64))
}

/// Format ETA (estimated time of arrival) into human-readable format
pub fn format_eta(seconds: Option<u64>) -> String {
    match seconds {
        Some(secs) => {
            let hours = secs / 3600;
            let minutes = (secs % 3600) / 60;
            let seconds = secs % 60;
            
            if hours > 0 {
                format!("{}h {}m {}s", hours, minutes, seconds)
            } else if minutes > 0 {
                format!("{}m {}s", minutes, seconds)
            } else {
                format!("{}s", seconds)
            }
        }
        None => "Unknown".to_string(),
    }
}
