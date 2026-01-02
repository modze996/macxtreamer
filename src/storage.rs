use std::fs;
use std::io::Read;
use std::path::PathBuf;
use crate::models::{FavItem, RecentItem, Item};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn data_dir() -> PathBuf {
    // macOS: ~/Library/Application Support/MacXtreamer
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(format!("{}/Library/Application Support/MacXtreamer", home))
}
fn recently_file() -> PathBuf { let d = data_dir(); let _ = fs::create_dir_all(&d); d.join("recently_played.json") }
fn favorites_file() -> PathBuf { let d = data_dir(); let _ = fs::create_dir_all(&d); d.join("favorites.json") }
fn search_history_file() -> PathBuf { let d = data_dir(); let _ = fs::create_dir_all(&d); d.join("search_history.json") }
fn search_index_file() -> PathBuf { let d = data_dir(); let _ = fs::create_dir_all(&d); d.join("search_index.json") }
fn search_index_meta_file() -> PathBuf { let d = data_dir(); let _ = fs::create_dir_all(&d); d.join("search_index_meta.json") }

pub fn load_recently_played() -> Vec<RecentItem> {
    let p = recently_file();
    if let Ok(mut f) = fs::File::open(&p) {
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            if let Ok(v) = serde_json::from_str::<Vec<RecentItem>>(&s) { return v; }
        }
    }
    Vec::new()
}
pub fn add_to_recently(item: &RecentItem) {
    let mut all = load_recently_played();
    // Entferne evtl. gleiche Einträge
    all.retain(|x| !(x.id == item.id && x.info == item.info));
    all.insert(0, item.clone());
    if all.len() > 50 { all.truncate(50); }
    let _ = fs::write(recently_file(), serde_json::to_string_pretty(&all).unwrap_or("[]".into()));
}
pub fn load_favorites() -> Vec<FavItem> {
    let p = favorites_file();
    if let Ok(mut f) = fs::File::open(&p) {
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            if let Ok(v) = serde_json::from_str::<Vec<FavItem>>(&s) { return v; }
        }
    }
    Vec::new()
}
pub fn toggle_favorite(item: &FavItem) {
    let mut all = load_favorites();
    // For series, match by id and item_type only (not info)
    // For episodes/movies/channels, match by id and info
    let pos = if item.item_type == "Series" {
        all.iter().position(|x| x.id == item.id && x.item_type == "Series")
    } else {
        all.iter().position(|x| x.id == item.id && x.info == item.info)
    };
    
    if let Some(pos) = pos {
        all.remove(pos);
    } else {
        all.push(item.clone());
    }
    let _ = fs::write(favorites_file(), serde_json::to_string_pretty(&all).unwrap_or("[]".into()));
}

pub fn is_favorite(id: &str, info: &str, item_type: &str) -> bool {
    let all = load_favorites();
    if item_type == "Series" {
        all.iter().any(|x| x.id == id && x.item_type == "Series")
    } else {
        all.iter().any(|x| x.id == id && x.info == info)
    }
}

pub fn load_search_history() -> Vec<String> {
    let p = search_history_file();
    if let Ok(mut f) = fs::File::open(&p) {
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            if let Ok(v) = serde_json::from_str::<Vec<String>>(&s) { return v; }
        }
    }
    Vec::new()
}

pub fn save_search_history(history: &Vec<String>) {
    let _ = fs::write(search_history_file(), serde_json::to_string_pretty(history).unwrap_or("[]".into()));
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SearchIndexData {
    movies: Vec<Item>,
    series: Vec<Item>,
    channels: Vec<Item>,
    paths: std::collections::HashMap<String, String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SearchIndexMeta {
    server_hash: u64,
    timestamp: u64,
}

/// Calculate a hash for the current server config to detect changes
fn calculate_server_hash(address: &str, username: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    address.hash(&mut hasher);
    username.hash(&mut hasher);
    hasher.finish()
}

/// Save search index to disk
pub fn save_search_index(
    movies: &Vec<Item>,
    series: &Vec<Item>,
    channels: &Vec<Item>,
    paths: &std::collections::HashMap<String, String>,
    address: &str,
    username: &str,
) {
    let data = SearchIndexData {
        movies: movies.clone(),
        series: series.clone(),
        channels: channels.clone(),
        paths: paths.clone(),
    };
    
    let meta = SearchIndexMeta {
        server_hash: calculate_server_hash(address, username),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    
    if let Ok(data_json) = serde_json::to_string(&data) {
        let _ = fs::write(search_index_file(), data_json);
    }
    
    if let Ok(meta_json) = serde_json::to_string(&meta) {
        let _ = fs::write(search_index_meta_file(), meta_json);
    }
}

/// Load search index from disk if valid
pub fn load_search_index(
    address: &str,
    username: &str,
) -> Option<(Vec<Item>, Vec<Item>, Vec<Item>, std::collections::HashMap<String, String>)> {
    // Check if meta file exists and is valid
    let meta_path = search_index_meta_file();
    if let Ok(mut f) = fs::File::open(&meta_path) {
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            if let Ok(meta) = serde_json::from_str::<SearchIndexMeta>(&s) {
                let current_hash = calculate_server_hash(address, username);
                
                // Check if server changed
                if meta.server_hash != current_hash {
                    return None;
                }
                
                // Check if index is older than 24 hours
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                    
                if now - meta.timestamp > 24 * 60 * 60 {
                    return None;
                }
                
                // Try to load data
                let data_path = search_index_file();
                if let Ok(mut f) = fs::File::open(&data_path) {
                    let mut s = String::new();
                    if f.read_to_string(&mut s).is_ok() {
                        if let Ok(data) = serde_json::from_str::<SearchIndexData>(&s) {
                            return Some((data.movies, data.series, data.channels, data.paths));
                        }
                    }
                }
            }
        }
    }
    
    None
}
