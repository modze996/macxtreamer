mod models;
mod api;
mod player;
pub mod jni;

pub use models::*;

use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().expect("tokio runtime"));

pub fn spawn<F>(fut: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    RUNTIME.spawn(fut);
}

pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    RUNTIME.block_on(fut)
}

// Simple global config for JNI bridge
use std::sync::Mutex;
#[derive(Default, Clone)]
pub struct CoreConfig {
    pub address: String,
    pub username: String,
    pub password: String,
}

static CORE_CONFIG: Lazy<Mutex<CoreConfig>> = Lazy::new(|| Mutex::new(CoreConfig::default()));

pub fn set_config(address: &str, username: &str, password: &str) {
    let mut c = CORE_CONFIG.lock().unwrap();
    c.address = address.to_string();
    c.username = username.to_string();
    c.password = password.to_string();
}

pub fn get_config() -> CoreConfig {
    CORE_CONFIG.lock().unwrap().clone()
}

pub fn build_stream_url(info: &str, id: &str, ext: Option<&str>) -> String {
    player::build_url_by_type(&get_config(), id, info, ext)
}

pub fn fetch_categories(kind: &str) -> Result<Vec<models::Category>, String> {
    block_on(api::fetch_categories(&get_config(), kind)).map_err(|e| e)
}

pub fn fetch_items(kind: &str, id: &str) -> Result<Vec<models::Item>, String> {
    block_on(api::fetch_items(&get_config(), kind, id)).map_err(|e| e)
}

pub fn fetch_series_episodes(series_id: &str) -> Result<Vec<models::Episode>, String> {
    block_on(api::fetch_series_episodes(&get_config(), series_id)).map_err(|e| e)
}
