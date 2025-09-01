use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::SystemTime;

pub fn cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(format!("{}/Library/Caches/MacXtreamer/cache", home));
    let _ = fs::create_dir_all(&dir);
    dir
}
pub fn cache_path(key: &str) -> PathBuf { cache_dir().join(format!("{}.json", key)) }

pub fn image_cache_dir() -> PathBuf {
    let mut d = cache_dir();
    d.push("images");
    let _ = fs::create_dir_all(&d);
    d
}
pub fn image_cache_path(url: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish();
    let ext = if url.ends_with(".png") { "png" } else if url.ends_with(".jpg") || url.ends_with(".jpeg") { "jpg" } else { "img" };
    Some(image_cache_dir().join(format!("{:x}.{}", hash, ext)))
}

pub fn ensure_cache_dir() { let _ = fs::create_dir_all(cache_dir()); }

pub fn file_age_secs(path: &PathBuf) -> Option<u64> {
    if let Ok(meta) = fs::metadata(path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                return Some(elapsed.as_secs());
            }
        }
    }
    None
}

pub fn load_cache<T: DeserializeOwned>(key: &str, max_age_secs: u64) -> Option<T> {
    ensure_cache_dir();
    let path = cache_path(key);
    if let Some(age) = file_age_secs(&path) {
        if age <= max_age_secs {
            if let Ok(mut f) = fs::File::open(&path) {
                let mut s = String::new();
                if f.read_to_string(&mut s).is_ok() {
                    if let Ok(v) = serde_json::from_str::<T>(&s) { return Some(v); }
                }
            }
        }
    }
    None
}

pub fn load_stale_cache<T: DeserializeOwned>(key: &str) -> Option<T> {
    ensure_cache_dir();
    let path = cache_path(key);
    if let Ok(mut f) = fs::File::open(&path) {
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            if let Ok(v) = serde_json::from_str::<T>(&s) { return Some(v); }
        }
    }
    None
}

pub fn save_cache<T: Serialize>(key: &str, data: &T) {
    ensure_cache_dir();
    let path = cache_path(key);
    if let Ok(s) = serde_json::to_string(data) { let _ = fs::write(path, s); }
}
