use std::path::PathBuf;
use std::time::SystemTime;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Local download tracking structs (specialized for UI & retry logic)
#[derive(Debug, Clone)]
pub struct DownloadMeta {
    pub id: String,
    pub name: String,
    pub info: String,
    pub container_extension: Option<String>,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub struct DownloadState {
    pub waiting: bool,
    pub finished: bool,
    pub error: Option<String>,
    pub path: Option<String>,
    pub received: u64,
    pub total: Option<u64>,
    pub cancel_flag: Option<Arc<AtomicBool>>,
    pub started_at: Option<std::time::Instant>,
    pub last_update_at: Option<std::time::Instant>,
    pub prev_received: u64,
    pub current_speed_bps: f64,
    pub avg_speed_bps: f64,
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
pub struct ScannedDownload {
    pub id: String,
    pub name: String,
    pub info: String,
    pub container_extension: Option<String>,
    pub path: String,
    pub size: u64,
    pub modified: SystemTime,
}

/// Expand download directory with ~ expansion and default fallback
pub fn expand_download_dir(download_dir: &str) -> PathBuf {
    let raw = download_dir.trim();
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
