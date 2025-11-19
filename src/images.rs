#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::SystemTime;
use tokio::sync::Semaphore;
use eframe::egui;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};

use crate::cache::{image_cache_path, file_age_secs};
use crate::logger::{log_line, log_error};

/// Helper: path to sidecar metadata for an image cache file (stores ETag/Last-Modified)
pub fn image_meta_path(url: &str) -> Option<PathBuf> {
    image_cache_path(url).and_then(|p| {
        let fname = p.file_name()?.to_string_lossy().to_string();
        let mut meta = p.clone();
        meta.set_file_name(format!("{}.meta", fname));
        Some(meta)
    })
}

/// Image loading and caching manager
pub struct ImageManager {
    pub loading_images: HashMap<String, Arc<AtomicBool>>,
    pub texture_cache: HashMap<String, egui::TextureHandle>,
    pub failed_images: std::collections::HashSet<String>,
    load_semaphore: Arc<Semaphore>,
    last_batch_repaint: std::time::Instant,
    batch_loaded_since_last: u32,
}

impl Default for ImageManager {
    fn default() -> Self {
        Self::new(6) // Default concurrent image loads
    }
}

impl ImageManager {
    pub fn new(concurrent_loads: usize) -> Self {
        Self {
            loading_images: HashMap::new(),
            texture_cache: HashMap::new(),
            failed_images: std::collections::HashSet::new(),
            load_semaphore: Arc::new(Semaphore::new(concurrent_loads)),
            last_batch_repaint: std::time::Instant::now(),
            batch_loaded_since_last: 0,
        }
    }

    /// Check if an image is currently being loaded
    pub fn is_loading(&self, url: &str) -> bool {
        self.loading_images
            .get(url)
            .map(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
    }

    /// Check if an image failed to load
    pub fn has_failed(&self, url: &str) -> bool {
        self.failed_images.contains(url)
    }

    /// Get cached texture if available
    pub fn get_texture(&self, url: &str) -> Option<&egui::TextureHandle> {
        self.texture_cache.get(url)
    }

    /// Start loading an image asynchronously
    pub fn start_loading_image(
        &mut self,
        url: String,
        ctx: egui::Context,
        config_uploads_per_frame: usize,
    ) {
        use std::sync::atomic::{AtomicBool, Ordering};

        if self.loading_images.contains_key(&url) || self.failed_images.contains(&url) {
            return;
        }

        let loading_flag = Arc::new(AtomicBool::new(true));
        self.loading_images.insert(url.clone(), loading_flag.clone());

        let semaphore = self.load_semaphore.clone();
        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            
            match load_image_with_cache(&url, config_uploads_per_frame).await {
                Ok(image_data) => {
                    let color_image = match image::load_from_memory(&image_data) {
                        Ok(dynamic_image) => {
                            let rgba = dynamic_image.to_rgba8();
                            let (width, height) = rgba.dimensions();
                            egui::ColorImage::from_rgba_unmultiplied(
                                [width as usize, height as usize],
                                rgba.as_raw(),
                            )
                        }
                        Err(e) => {
                            log_error("Failed to decode image", &e);
                            loading_flag.store(false, Ordering::Relaxed);
                            return;
                        }
                    };

                    let _texture = ctx.load_texture(&url, color_image, Default::default());
                    // Batch Repaint: nur jede ~50ms oder nach 6 Bildern
                    let now = std::time::Instant::now();
                    // Zugriff auf Manager Felder nicht direkt möglich (move). Vereinfachung: sofort repaint wenn >50ms vergangen
                    // Für echte Batch-Steuerung müsste ein Channel zurück in Hauptthread genutzt werden.
                    // Minimale Variante: zeitbasierter throttle.
                    use std::sync::OnceLock;
                    static REPAINT_LAST: OnceLock<std::sync::Mutex<std::time::Instant>> = OnceLock::new();
                    let repaint_last = REPAINT_LAST.get_or_init(|| std::sync::Mutex::new(std::time::Instant::now()));
                    let mut guard = repaint_last.lock().unwrap();
                    if now.duration_since(*guard).as_millis() > 50 { ctx.request_repaint(); *guard = now; }

                    // Note: In a real implementation, you'd need a way to communicate
                    // the loaded texture back to the main thread. This is a simplified version.
                }
                Err(e) => {
                    log_error("Failed to load image", e.as_ref());
                }
            }
            
            loading_flag.store(false, Ordering::Relaxed);
        });
    }

    /// Clear texture cache to free memory
    pub fn clear_texture_cache(&mut self) {
        self.texture_cache.clear();
    }

    /// Remove old failed entries to allow retrying
    pub fn clear_failed_images(&mut self) {
        self.failed_images.clear();
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> ImageCacheStats {
        ImageCacheStats {
            cached_textures: self.texture_cache.len(),
            loading_count: self.loading_images.len(),
            failed_count: self.failed_images.len(),
        }
    }
}

pub struct ImageCacheStats {
    pub cached_textures: usize,
    pub loading_count: usize,
    pub failed_count: usize,
}

/// Load image with caching support
async fn load_image_with_cache(
    url: &str,
    _uploads_per_frame: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // Check if we have a cached version
    if let Some(cache_path) = image_cache_path(url) {
        if cache_path.exists() {
            // Check cache age and metadata for conditional requests
            let cache_age = file_age_secs(&cache_path).unwrap_or(u64::MAX);
            
            // If cache is relatively fresh (less than 1 hour), use it directly
            if cache_age < 3600 {
                if let Ok(data) = tokio::fs::read(&cache_path).await {
                    log_line(&format!("Using cached image: {}", url));
                    return Ok(data);
                }
            }

            // Try conditional request using stored metadata
            if let Some(meta_path) = image_meta_path(url) {
                if meta_path.exists() {
                    if let Ok(meta_content) = tokio::fs::read_to_string(&meta_path).await {
                        return fetch_image_conditional(url, &cache_path, &meta_content).await;
                    }
                }
            }
        }
    }

    // No cache or cache is stale, fetch fresh image
    fetch_image_fresh(url).await
}

/// Fetch image with conditional headers (ETag, Last-Modified)
async fn fetch_image_conditional(
    url: &str,
    cache_path: &Path,
    meta_content: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let mut request = client.get(url);

    // Parse metadata and add conditional headers
    for line in meta_content.lines() {
        if let Some((key, value)) = line.split_once(':') {
            match key.trim() {
                "etag" => {
                    request = request.header(IF_NONE_MATCH, value.trim());
                }
                "last-modified" => {
                    request = request.header(IF_MODIFIED_SINCE, value.trim());
                }
                _ => {}
            }
        }
    }

    let response = request.send().await?;
    
    if response.status() == 304 {
        // Not modified, use cached version
        log_line(&format!("Image not modified, using cache: {}", url));
        let data = tokio::fs::read(cache_path).await?;
        return Ok(data);
    }

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()).into());
    }

    // Get headers before consuming response
    let headers = response.headers().clone();
    let bytes = response.bytes().await?;
    let data = bytes.to_vec();

    // Cache the new image and metadata
    if let Some(cache_dir) = cache_path.parent() {
        tokio::fs::create_dir_all(cache_dir).await.ok();
        tokio::fs::write(cache_path, &data).await.ok();

        // Save metadata
        if let Some(meta_path) = image_meta_path(url) {
            let mut metadata = String::new();
            
            if let Some(etag) = headers.get(ETAG) {
                if let Ok(etag_str) = etag.to_str() {
                    metadata.push_str(&format!("etag: {}\n", etag_str));
                }
            }
            
            if let Some(last_modified) = headers.get(LAST_MODIFIED) {
                if let Ok(lm_str) = last_modified.to_str() {
                    metadata.push_str(&format!("last-modified: {}\n", lm_str));
                }
            }
            
            tokio::fs::write(meta_path, metadata).await.ok();
        }
    }

    log_line(&format!("Downloaded and cached image: {}", url));
    Ok(data)
}

/// Fetch image without caching considerations
async fn fetch_image_fresh(
    url: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()).into());
    }

    let bytes = response.bytes().await?;
    let data = bytes.to_vec();

    // Cache the image
    if let Some(cache_path) = image_cache_path(url) {
        if let Some(cache_dir) = cache_path.parent() {
            tokio::fs::create_dir_all(cache_dir).await.ok();
            tokio::fs::write(cache_path, &data).await.ok();
        }
    }

    log_line(&format!("Downloaded image: {}", url));
    Ok(data)
}

/// Clean up old cached images
pub async fn cleanup_old_images(max_age_seconds: u64) -> std::io::Result<usize> {
    let cache_dir = directories::ProjectDirs::from("com", "yourcompany", "MacXtreamer")
        .map(|dirs| dirs.cache_dir().join("images"));

    let Some(cache_dir) = cache_dir else {
        return Ok(0);
    };

    if !cache_dir.exists() {
        return Ok(0);
    }

    let mut cleaned_count = 0;
    let mut entries = tokio::fs::read_dir(&cache_dir).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        
        if let Ok(metadata) = entry.metadata().await {
            if let Ok(modified) = metadata.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    if age.as_secs() > max_age_seconds {
                        if tokio::fs::remove_file(&path).await.is_ok() {
                            cleaned_count += 1;
                            
                            // Also remove associated .meta file
                            if let Some(meta_path) = image_meta_path(path.to_string_lossy().as_ref()) {
                                tokio::fs::remove_file(meta_path).await.ok();
                            }
                        }
                    }
                }
            }
        }
    }

    log_line(&format!("Cleaned up {} old cached images", cleaned_count));
    Ok(cleaned_count)
}

/// Get total size of image cache
pub async fn get_cache_size() -> std::io::Result<u64> {
    let cache_dir = directories::ProjectDirs::from("com", "yourcompany", "MacXtreamer")
        .map(|dirs| dirs.cache_dir().join("images"));

    let Some(cache_dir) = cache_dir else {
        return Ok(0);
    };

    if !cache_dir.exists() {
        return Ok(0);
    }

    let mut total_size = 0u64;
    let mut entries = tokio::fs::read_dir(&cache_dir).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        if let Ok(metadata) = entry.metadata().await {
            total_size += metadata.len();
        }
    }

    Ok(total_size)
}
