use crate::{models::*, CoreConfig};

pub async fn fetch_categories(cfg: &CoreConfig, kind: &str) -> Result<Vec<Category>, String> {
    let url = match kind {
        "get_vod_categories" => format!("{}/player_api.php?username={}&password={}&action=get_vod_categories", cfg.address, cfg.username, cfg.password),
        "get_series_categories" => format!("{}/player_api.php?username={}&password={}&action=get_series_categories", cfg.address, cfg.username, cfg.password),
    "get_live_categories" => format!("{}/player_api.php?username={}&password={}&action=get_live_categories", cfg.address, cfg.username, cfg.password),
        _ => return Ok(vec![]),
    };
    let res = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if !res.status().is_success() { return Err(format!("HTTP {}", res.status())); }
    let cats: Vec<Category> = res.json().await.map_err(|e| e.to_string())?;
    Ok(cats)
}

pub async fn fetch_items(cfg: &CoreConfig, kind: &str, id: &str) -> Result<Vec<Item>, String> {
    let action = match kind { "vod" => "get_vod_streams", "series" => "get_series", "live" => "get_live_streams", _ => return Ok(vec![]) };
    let url = format!(
        "{}/player_api.php?username={}&password={}&action={}&category_id={}",
        cfg.address, cfg.username, cfg.password, action, id
    );
    let res = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if !res.status().is_success() { return Err(format!("HTTP {}", res.status())); }
    let body = res.text().await.map_err(|e| e.to_string())?;
    // Mapping vereinfachen; echte Struktur in App anpassen
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let arr = v.as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for o in arr {
        let id = o.get("stream_id").or_else(|| o.get("series_id")).and_then(|x| x.as_i64()).unwrap_or_default().to_string();
        let name = o.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let url = o.get("stream_url").and_then(|x| x.as_str()).map(|s| s.to_string());
        let ext = o.get("container_extension").and_then(|x| x.as_str()).map(|s| s.to_string());
        let cover = o.get("cover").or_else(|| o.get("cover_url")).and_then(|x| x.as_str()).map(|s| s.to_string());
        out.push(Item { id, name, stream_url: url, container_extension: ext, cover_url: cover });
    }
    Ok(out)
}

pub async fn fetch_series_episodes(cfg: &CoreConfig, series_id: &str) -> Result<Vec<Episode>, String> {
    let url = format!(
        "{}/player_api.php?username={}&password={}&action=get_series_info&series_id={}",
        cfg.address, cfg.username, cfg.password, series_id
    );
    let res = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if !res.status().is_success() { return Err(format!("HTTP {}", res.status())); }
    let v: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    if let Some(info) = v.get("episodes") {
        if let Some(seasons) = info.as_object() {
            for (_sn, eps) in seasons {
                if let Some(arr) = eps.as_array() {
                    for ep in arr {
                        let episode_id = ep.get("id").or_else(|| ep.get("episode_id")).and_then(|x| x.as_i64()).unwrap_or_default().to_string();
                        let name = ep.get("title").or_else(|| ep.get("name")).and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let container_extension = ep.get("container_extension").and_then(|x| x.as_str()).unwrap_or("mp4").to_string();
                        out.push(Episode { episode_id, name, container_extension });
                    }
                }
            }
        }
    }
    Ok(out)
}
