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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentItem {
    pub id: String,
    pub name: String,
    pub info: String,
    pub stream_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FavItem {
    pub id: String,
    pub info: String,
    pub name: String,
    pub stream_url: Option<String>,
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
    pub rating_5based: Option<f32>,
    pub genre: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchItem {
    pub id: String,
    pub name: String,
    pub info: String,
    pub container_extension: String,
    pub cover: Option<String>,
    pub year: Option<String>,
    pub rating_5based: Option<f32>,
    pub genre: Option<String>,
}
