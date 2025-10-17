use crate::cache::{load_cache, load_stale_cache, save_cache};
use crate::models::{Category, Config, Episode, Item};
use serde_json::Value;

pub const CACHE_TTL_CATEGORIES_SECS: u64 = 6 * 60 * 60; // 6h
pub const CACHE_TTL_ITEMS_SECS: u64 = 3 * 60 * 60; // 3h
pub const CACHE_TTL_EPISODES_SECS: u64 = 12 * 60 * 60; // 12h

pub async fn fetch_categories(cfg: &Config, action: &str) -> Result<Vec<Category>, reqwest::Error> {
    let key = match action {
        "get_live_categories" => "live_categories",
        "get_vod_categories" => "vod_categories",
        "get_series_categories" => "series_categories",
        _ => action,
    };
    if let Some(cached) = load_cache::<Vec<Category>>(key, CACHE_TTL_CATEGORIES_SECS) { return Ok(cached); }
    let url = format!("{}/player_api.php?username={}&password={}&action={}", cfg.address, cfg.username, cfg.password, action);
    let net = async {
        let res = reqwest::get(&url).await?;
        let json = res.json::<Value>().await?;
        let mut out = Vec::new();
        if let Some(arr) = json.as_array() {
            for v in arr {
                let id = v.get("category_id").or_else(|| v.get("id")).and_then(|x| x.as_str()).unwrap_or_default().to_string();
                let name = v.get("category_name").or_else(|| v.get("name")).and_then(|x| x.as_str()).unwrap_or_default().to_string();
                if !id.is_empty() || !name.is_empty() { out.push(Category { id, name }); }
            }
        }
        Ok::<Vec<Category>, reqwest::Error>(out)
    }.await;
    match net { Ok(list) => { save_cache(key, &list); Ok(list) } Err(e) => { if let Some(stale) = load_stale_cache::<Vec<Category>>(key) { Ok(stale) } else { Err(e) } } }
}

pub async fn fetch_items(cfg: &Config, kind: &str, category_id: &str) -> Result<Vec<Item>, reqwest::Error> {
    let action = match kind { "subplaylist" => "get_live_streams", "vod" => "get_vod_streams", "series" => "get_series", other => other };
    let key = format!("items_{}_{}", action, category_id);
    if let Some(cached) = load_cache::<Vec<Item>>(&key, CACHE_TTL_ITEMS_SECS) { return Ok(cached); }
    let url = format!("{}/player_api.php?username={}&password={}&action={}&category_id={}", cfg.address, cfg.username, cfg.password, action, category_id);
    let net = async {
        let res = reqwest::get(&url).await?;
        let json = res.json::<Value>().await?;
        let mut out = Vec::new();
    if let Some(arr) = json.as_array() {
            for v in arr {
                let id = v.get("stream_id").or_else(|| v.get("series_id")).or_else(|| v.get("id")).and_then(|x| x.as_i64()).map(|n| n.to_string()).unwrap_or_default();
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or_default().to_string();
                let mut item = Item { id, name, ..Default::default() };
                if let Some(ext) = v.get("container_extension").and_then(|x| x.as_str()) { item.container_extension = ext.to_string(); }
                if let Some(plot) = v.get("plot").and_then(|x| x.as_str()) { item.plot = plot.to_string(); }
                if let Some(url) = v.get("stream_url").and_then(|x| x.as_str()) { item.stream_url = Some(url.to_string()); }
                if let Some(cover) = v.get("cover").or_else(|| v.get("stream_icon")).and_then(|x| x.as_str()) { item.cover = Some(cover.to_string()); }
                if let Some(year) = v.get("year").and_then(|x| x.as_str()) { item.year = Some(year.to_string()); }
                if let Some(release_date) = v.get("releaseDate").or_else(|| v.get("release_date")).or_else(|| v.get("releasedate")).and_then(|x| x.as_str()) { item.release_date = Some(release_date.to_string()); }
        // Ratings: handle both "rating_5based" (number or string) and "rating" (string/number), normalize to 0..5
        let read_f32 = |val: &serde_json::Value| -> Option<f32> {
            val.as_f64().map(|x| x as f32)
            .or_else(|| val.as_str().and_then(|s| s.trim().parse::<f32>().ok()))
        };
        let r5 = v.get("rating_5based").and_then(read_f32);
        let r10 = v.get("rating").and_then(read_f32);
        let rating_norm = r5.or_else(|| r10.map(|x| if x > 5.0 { x / 2.0 } else { x }));
        if let Some(r) = rating_norm { item.rating_5based = Some(r); }
                if let Some(genre) = v.get("genre").and_then(|x| x.as_str()) { item.genre = Some(genre.to_string()); }
                if let Some(dir) = v.get("director").and_then(|x| x.as_str()) { item.director = Some(dir.to_string()); }
                if let Some(cast) = v.get("cast").and_then(|x| x.as_str()) { item.cast = Some(cast.to_string()); }
                out.push(item);
            }
        }
        Ok::<Vec<Item>, reqwest::Error>(out)
    }.await;
    match net { Ok(items) => { save_cache(&key, &items); Ok(items) } Err(e) => { if let Some(stale) = load_stale_cache::<Vec<Item>>(&key) { Ok(stale) } else { Err(e) } } }
}

pub async fn fetch_series_episodes(cfg: &Config, series_id: &str) -> Result<Vec<Episode>, reqwest::Error> {
    let key = format!("episodes_{}", series_id);
    if let Some(cached) = load_cache::<Vec<Episode>>(&key, CACHE_TTL_EPISODES_SECS) { return Ok(cached); }
    let url = format!("{}/player_api.php?username={}&password={}&action=get_series_info&series_id={}", cfg.address, cfg.username, cfg.password, series_id);
    let net = async {
        let res = reqwest::get(&url).await?;
        let json = res.json::<Value>().await?;
        let mut out = Vec::new();
        // Series-level cover lives at info.movie_image (fallback to info.cover)
        let series_cover = json
            .get("info")
            .and_then(|i| i.get("movie_image").or_else(|| i.get("cover")))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        if let Some(episodes_by_season) = json.get("episodes").and_then(|x| x.as_object()) {
            for (_season, eps) in episodes_by_season.iter() {
                if let Some(arr) = eps.as_array() {
                    for ep in arr {
                        // Read ID from several possible shapes (string or number)
                        let read_id = |v: &Value| -> Option<String> {
                            v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string()))
                        };
                        let episode_id = ep.get("episode_id")
                            .and_then(read_id)
                            .or_else(|| ep.get("id").and_then(read_id))
                            .or_else(|| ep.get("stream_id").and_then(read_id))
                            .unwrap_or_default();
                        let name = ep.get("title").or_else(|| ep.get("name")).and_then(|x| x.as_str()).unwrap_or_default().to_string();
                        let container_extension = ep.get("container_extension").and_then(|x| x.as_str()).unwrap_or("mp4").to_string();
                        let stream_url = ep.get("stream_url").and_then(|x| x.as_str()).map(|s| s.to_string());
                        // Prefer episode-specific image if present, else series-level cover
                        let ep_cover = ep
                            .get("cover")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| series_cover.clone());
                        out.push(Episode { episode_id, name, container_extension, stream_url, cover: ep_cover });
                    }
                }
            }
        }
        Ok::<Vec<Episode>, reqwest::Error>(out)
    }.await;
    match net { Ok(eps) => { save_cache(&key, &eps); Ok(eps) } Err(e) => { if let Some(stale) = load_stale_cache::<Vec<Episode>>(&key) { Ok(stale) } else { Err(e) } } } }

// Wisdom-Gate AI API integration for streaming recommendations
// Demo fallback function for testing
fn get_demo_recommendations() -> String {
    "🎬 **Heutige Top Streaming-Empfehlungen (16. Oktober 2025)**\n\n\
    **Netflix:**\n\
    • The Crown (Staffel 6) - Das finale Kapitel der Royal-Saga\n\
    • Wednesday (Staffel 2) - Addams Family Mystery geht weiter\n\
    • Stranger Things: The Final Season - Hawkins' letzter Kampf\n\n\
    **Amazon Prime:**\n\
    • The Boys (Staffel 4) - Superhelden-Satire auf dem Höhepunkt\n\
    • Lord of the Rings: The Rings of Power (Staffel 2)\n\
    • The Marvelous Mrs. Maisel - Finale der Comedy-Serie\n\n\
    **Disney+:**\n\
    • The Mandalorian (Staffel 4) - Neue Abenteuer in der Galaxie\n\
    • Loki (Staffel 3) - Multiversum-Chaos geht weiter\n\
    • What If...? (Staffel 3) - Alternative Marvel-Realitäten\n\n\
    **Apple TV+:**\n\
    • Ted Lasso (Special Episodes) - Rückkehr des beliebten Trainers\n\
    • Severance (Staffel 2) - Psycho-Thriller der Extraklasse\n\n\
    Viel Spaß beim Streamen! 🍿".to_string()
}

pub async fn fetch_wisdom_gate_recommendations(api_key: &str, prompt: &str, model: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    
    // Try different API key formats and models
    let headers = match api_key.starts_with("Bearer ") {
        true => format!("{}", api_key),  // Already has Bearer prefix
        false => format!("Bearer {}", api_key),  // Add Bearer prefix
    };
    
    let url = "https://wisdom-gate.juheapi.com/v1/chat/completions";
    println!("🔗 Connecting to: {}", url);
    println!("🤖 Model: {}", model);
    
    // Simplified request body - exactly matching the JavaScript example
    let request_body = serde_json::json!({
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ],
        "model": model,
        "max_tokens": 2000,  // Adding back in case it helps
        "temperature": 0.7   // Adding back in case it helps
    });
    
    println!("📋 Request body: {}", serde_json::to_string_pretty(&request_body)?);
    println!("🔑 Auth header: {}", headers);
    
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Authorization", &headers)
        .json(&request_body)
        .send()
        .await?;
        
    let status = response.status();
    let response_text = response.text().await?;
    
    println!("📊 Response status: {}", status);
    println!("📄 Response body: {}", response_text);
    
    if !status.is_success() {
        println!("❌ API Error: Status {}", status);
        println!("❌ Error details: {}", response_text);
        return Ok(format!("API Fehler ({}): {}", status, response_text));
    }
    
    // Parse response
    let response_json: serde_json::Value = serde_json::from_str(&response_text)?;
    
    if let Some(choices) = response_json["choices"].as_array() {
        if let Some(first_choice) = choices.first() {
            if let Some(message) = first_choice["message"].as_object() {
                if let Some(content) = message["content"].as_str() {
                    return Ok(content.to_string());
                }
            }
        }
    }
    
    Ok(format!("Modell {} lieferte keine verwertbare Antwort: {}", model, response_text))
}

// Wrapper function that handles network errors gracefully and tries fallback models
pub async fn fetch_wisdom_gate_recommendations_safe(api_key: &str, prompt: &str, model: &str) -> String {
    // List of actually available models to try in order
    let models_to_try = vec![
        model,  // Try user-selected model first
        "wisdom-ai-dsv3",
        "deepseek-v3",
        "gemini-2.5-flash",
        "wisdom-ai-gemini-2.5-flash"
    ];
    
    for try_model in models_to_try {
        println!("🔄 Versuche Modell: {}", try_model);
        match fetch_wisdom_gate_recommendations(api_key, prompt, try_model).await {
            Ok(content) => {
                if !content.starts_with("Modell") && !content.starts_with("API Fehler") {
                    if try_model != model {
                        println!("✅ Fallback erfolgreich: {} funktioniert!", try_model);
                    }
                    return content;
                }
                println!("⚠️ Modell {} nicht verfügbar, versuche nächstes...", try_model);
            }
            Err(e) => {
                println!("❌ Fehler mit Modell {}: {}", try_model, e);
            }
        }
    }
    
    // If all models fail, return demo
    println!("🌐 Alle Modelle fehlgeschlagen - Verwende Demo-Empfehlungen");
    format!("🌐 **Offline-Modus** (Alle verfügbaren Modelle getestet)\n\n{}", get_demo_recommendations())
}
