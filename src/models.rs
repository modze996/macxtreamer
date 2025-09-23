use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub address: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub player_command: String,
    #[serde(default)]
    pub theme: String, // "dark" | "light"
    #[serde(default)]
    pub cover_ttl_days: u32,      // 1 Woche default
    #[serde(default)]
    pub cover_parallel: u32,      // 6 default
    #[serde(default)]
    pub font_scale: f32,          // 1.0..=?, default increased (e.g., 1.15)
    #[serde(default)]
    pub reuse_vlc: bool,          // default true
    #[serde(default)]
    pub download_dir: String,     // default ~/Downloads/macxtreamer
    #[serde(default)]
    pub cover_uploads_per_frame: u32, // default 3
    #[serde(default)]
    pub cover_decode_parallel: u32,   // default 2
    #[serde(default)]
    pub texture_cache_limit: u32,     // default 512
    #[serde(default)]
    pub category_parallel: u32,       // default 6
    #[serde(default)]
    pub cover_height: f32,            // default 60.0
    #[serde(default)]
    pub enable_downloads: bool,       // default false
    #[serde(default)]
    pub max_parallel_downloads: u32,  // default 1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub container_extension: String,
    #[serde(default)]
    pub plot: String,
    #[serde(default)]
    pub stream_url: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub year: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub rating_5based: Option<f32>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub director: Option<String>,
    #[serde(default)]
    pub cast: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Episode {
    pub episode_id: String,
    pub name: String,
    #[serde(default)]
    pub container_extension: String,
    #[serde(default)]
    pub stream_url: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentItem {
    pub id: String,
    pub name: String,
    pub info: String,
    pub stream_url: String,
    #[serde(default)]
    pub container_extension: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FavItem {
    pub id: String,
    pub info: String,
    pub name: String,
    pub stream_url: Option<String>,
    #[serde(default)]
    pub container_extension: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub name: String,
    pub id: String,
    pub info: String,
    pub container_extension: Option<String>,
    pub stream_url: Option<String>,
    pub cover_url: Option<String>,
    pub year: Option<String>,
    pub release_date: Option<String>,
    pub rating_5based: Option<f32>,
    pub genre: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchItem {
    pub id: String,
    pub name: String,
    pub info: String,
    pub container_extension: String,
    pub cover: Option<String>,
    pub year: Option<String>,
    pub release_date: Option<String>,
    pub rating_5based: Option<f32>,
    pub genre: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JustWatchRecommendation {
    pub title: String,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub provider: Option<String>,
    pub content_type: String, // "movie" or "series"
    pub url: Option<String>,
    pub cover_url: Option<String>,
    pub rating: Option<f32>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub director: Option<String>,
    #[serde(default)]
    pub cast: Option<Vec<String>>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub age_rating: Option<String>,
    #[serde(default)]
    pub imdb_rating: Option<f32>,
}

impl Default for JustWatchRecommendation {
    fn default() -> Self {
        Self {
            title: String::new(),
            year: None,
            genre: None,
            provider: None,
            content_type: "movie".to_string(),
            url: None,
            cover_url: None,
            rating: None,
            description: None,
            director: None,
            cast: None,
            runtime: None,
            age_rating: None,
            imdb_rating: None,
        }
    }
}
