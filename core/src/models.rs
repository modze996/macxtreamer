use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category { pub id: String, pub name: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub stream_url: Option<String>,
    pub container_extension: Option<String>,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub episode_id: String,
    pub name: String,
    pub container_extension: String,
}
