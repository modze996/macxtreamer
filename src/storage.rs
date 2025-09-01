use std::fs;
use std::io::Read;
use std::path::PathBuf;
use crate::models::{FavItem, RecentItem};

fn data_dir() -> PathBuf {
    // macOS: ~/Library/Application Support/MacXtreamer
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(format!("{}/Library/Application Support/MacXtreamer", home))
}
fn recently_file() -> PathBuf { let d = data_dir(); let _ = fs::create_dir_all(&d); d.join("recently_played.json") }
fn favorites_file() -> PathBuf { let d = data_dir(); let _ = fs::create_dir_all(&d); d.join("favorites.json") }

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
    if let Some(pos) = all.iter().position(|x| x.id == item.id && x.info == item.info) {
        all.remove(pos);
    } else {
        all.push(item.clone());
    }
    let _ = fs::write(favorites_file(), serde_json::to_string_pretty(&all).unwrap_or("[]".into()));
}
