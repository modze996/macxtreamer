use crate::cache::{load_cache, load_stale_cache, save_cache};
use crate::models::{Category, Config, Episode, Item};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EpgProgram {
    pub title: String,
    pub start: String,
    pub end: String,
    pub description: Option<String>,
}

/// Try to decode base64 string, return original if decoding fails
fn try_decode_base64(s: &str) -> String {
    use base64::{Engine as _, engine::general_purpose};
    
    // Check if string looks like base64 (contains only base64 chars)
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=') {
        if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(s) {
            if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                return decoded_str;
            }
        }
    }
    s.to_string()
}

pub const CACHE_TTL_CATEGORIES_SECS: u64 = 6 * 60 * 60; // 6h
pub const CACHE_TTL_ITEMS_SECS: u64 = 3 * 60 * 60; // 3h
pub const CACHE_TTL_EPISODES_SECS: u64 = 12 * 60 * 60; // 12h

/// Clean problematic Unicode characters that may not render properly
fn clean_unicode_text(text: &str) -> String {
    text.chars()
        .map(|c| {
            // Replace modifier letters and other problematic Unicode with regular equivalents
            match c {
                // Modifier letter small capitals (often used in streaming quality labels)
                'ᴬ' => 'A', 'ᴮ' => 'B', 'ᴰ' => 'D', 'ᴱ' => 'E', 'ᴳ' => 'G', 'ᴴ' => 'H',
                'ᴵ' => 'I', 'ᴶ' => 'J', 'ᴷ' => 'K', 'ᴸ' => 'L', 'ᴹ' => 'M', 'ᴺ' => 'N',
                'ᴼ' => 'O', 'ᴾ' => 'P', 'ᴿ' => 'R', 'ᵀ' => 'T', 'ᵁ' => 'U', 'ⱽ' => 'V',
                'ᵂ' => 'W', 'ᶜ' => 'c', 'ᶠ' => 'f', 'ᵍ' => 'g', 'ʰ' => 'h', 'ⁱ' => 'i',
                'ʲ' => 'j', 'ᵏ' => 'k', 'ˡ' => 'l', 'ᵐ' => 'm', 'ⁿ' => 'n', 'ᵒ' => 'o',
                'ᵖ' => 'p', 'ʳ' => 'r', 'ˢ' => 's', 'ᵗ' => 't', 'ᵘ' => 'u', 'ᵛ' => 'v',
                'ʷ' => 'w', 'ˣ' => 'x', 'ʸ' => 'y', 'ᶻ' => 'z', 'ᵃ' => 'a', 'ᵇ' => 'b',
                'ᵈ' => 'd', 'ᵉ' => 'e', 'ᶦ' => 'i',
                // Superscript numbers
                '⁰' => '0', '¹' => '1', '²' => '2', '³' => '3', '⁴' => '4',
                '⁵' => '5', '⁶' => '6', '⁷' => '7', '⁸' => '8', '⁹' => '9',
                // Keep everything else as-is
                _ => c
            }
        })
        .collect()
}

pub async fn fetch_categories(cfg: &Config, action: &str) -> Result<Vec<Category>, reqwest::Error> {
    let key = match action {
        "get_live_categories" => "live_categories",
        "get_vod_categories" => "vod_categories",
        "get_series_categories" => "series_categories",
        _ => action,
    };
    if let Some(cached) = load_cache::<Vec<Category>>(key, CACHE_TTL_CATEGORIES_SECS) { return Ok(cached); }
    let url = format!("{}/player_api.php?username={}&password={}&action={}", cfg.address, cfg.username, cfg.password, action);
    // println!("🌐 API-Aufruf: {}", url.replace(&cfg.password, "***"));
    let net = async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let res = client.get(&url).send().await?;
        let json = res.json::<Value>().await?;
        let mut out = Vec::new();
        if let Some(arr) = json.as_array() {
            for v in arr {
                let id = v.get("category_id").or_else(|| v.get("id")).and_then(|x| x.as_str()).unwrap_or_default().to_string();
                let name = v.get("category_name").or_else(|| v.get("name")).and_then(|x| x.as_str()).unwrap_or_default().to_string();
                let cleaned_name = clean_unicode_text(&name);
                if !id.is_empty() || !cleaned_name.is_empty() { 
                    out.push(Category { id, name: cleaned_name }); 
                }
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
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let res = client.get(&url).send().await?;
        let json = res.json::<Value>().await?;
        let mut out = Vec::new();
    if let Some(arr) = json.as_array() {
            for v in arr {
                let id = v.get("stream_id").or_else(|| v.get("series_id")).or_else(|| v.get("id")).and_then(|x| x.as_i64()).map(|n| n.to_string()).unwrap_or_default();
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or_default().to_string();
                let cleaned_name = clean_unicode_text(&name);
                let mut item = Item { id, name: cleaned_name, ..Default::default() };
                if let Some(ext) = v.get("container_extension").and_then(|x| x.as_str()) { item.container_extension = ext.to_string(); }
                if let Some(plot) = v.get("plot").and_then(|x| x.as_str()) { 
                    item.plot = clean_unicode_text(plot); 
                }
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
                // Extract language from name if not provided by API
                if let Some(audio_langs) = v.get("audio_languages").and_then(|x| x.as_str()) { 
                    item.audio_languages = Some(audio_langs.to_string()); 
                } else {
                    // Fallback: extract from name (e.g., "EN - Movie Name" -> "EN")
                    item.audio_languages = crate::helpers::extract_language_from_name(&item.name);
                }
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
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let res = client.get(&url).send().await?;
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

/// Fetch current EPG program for a live channel
pub async fn fetch_short_epg(cfg: &Config, stream_id: &str) -> Result<Option<EpgProgram>, reqwest::Error> {
    let key = format!("epg_{}", stream_id);
    // Cache EPG for 5 minutes (programs change)
    if let Some(cached) = load_cache::<Option<EpgProgram>>(&key, 300) { 
        return Ok(cached); 
    }
    
    let url = format!(
        "{}/player_api.php?username={}&password={}&action=get_short_epg&stream_id={}",
        cfg.address, cfg.username, cfg.password, stream_id
    );
    
    let net = async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;
        let res = client.get(&url).send().await?;
        
        // Read response as text first to handle empty/invalid JSON
        let text = res.text().await?;
        
        // Handle empty responses gracefully
        if text.trim().is_empty() {
            return Ok(None);
        }
        
        // Try to parse as JSON
        let json: Value = match serde_json::from_str(&text) {
            Ok(j) => j,
            Err(_) => {
                // Invalid JSON - many channels don't have EPG data
                return Ok(None);
            }
        };
        
        // Extract current/next program
        let program = if let Some(epg_listings) = json.get("epg_listings").and_then(|x| x.as_array()) {
            if epg_listings.is_empty() {
                return Ok(None);
            }
            
            // Find currently running program
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            
            epg_listings.iter()
                .find(|item| {
                    let start = item.get("start_timestamp")
                        .and_then(|x| x.as_str())
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0);
                    let stop = item.get("stop_timestamp")
                        .and_then(|x| x.as_str())
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0);
                    now >= start && now < stop
                })
                .or_else(|| epg_listings.first()) // Fallback to first program
                .map(|item| {
                    let raw_title = item.get("title")
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    let raw_desc = item.get("description")
                        .and_then(|x| x.as_str());
                    
                    EpgProgram {
                        title: try_decode_base64(raw_title),
                        start: item.get("start")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        end: item.get("stop")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        description: raw_desc.map(|s| try_decode_base64(s)),
                    }
                })
        } else {
            None
        };
        
        Ok::<Option<EpgProgram>, reqwest::Error>(program)
    }.await;
    
    match net {
        Ok(program) => {
            save_cache(&key, &program);
            Ok(program)
        }
        Err(e) => {
            if let Some(stale) = load_stale_cache::<Option<EpgProgram>>(&key) {
                Ok(stale)
            } else {
                Err(e)
            }
        }
    }
}

// Wisdom-Gate AI API integration for streaming recommendations
// Demo fallback function for testing
fn get_demo_recommendations() -> String {
    "🎬 **Top Streaming-Empfehlungen Deutschland (November 2025)**\n\n\
    **Netflix Highlights:**\n\
    • Wednesday S2 - Addams Family Horror-Comedy (IMDB: 8.2)\n\
    • Squid Game 2 - Koreanisches Survival-Drama (IMDB: 8.0)\n\
    • Stranger Things 5 - Das große Hawkins-Finale (IMDB: 8.7)\n\
    • Avatar Live-Action - Airbender Neuverfilmung (IMDB: 7.8)\n\n\
    **Prime Video:**\n\
    • The Boys S5 - Düstere Superhelden-Satire (IMDB: 8.7)\n\
    • Fallout - Postapokalyptische Game-Adaption (IMDB: 8.5)\n\
    • Rings of Power S3 - Mittelerde-Epos (IMDB: 6.9)\n\
    • Mr. & Mrs. Smith - Action-Thriller Remake (IMDB: 7.1)\n\n\
    **Disney+ Neuheiten:**\n\
    • Mandalorian S4 - Star Wars Western (IMDB: 8.7)\n\
    • Loki S3 - Marvel Multiversum (IMDB: 8.2)\n\
    • Percy Jackson S2 - Fantasy-Abenteuer (IMDB: 7.0)\n\n\
    **Apple TV+ Premium:**\n\
    • Severance S2 - Workplace-Thriller (IMDB: 8.7)\n\
    • Foundation S3 - Asimov Sci-Fi (IMDB: 7.3)\n\
    • Masters of the Air - WWII Serie (IMDB: 8.3)\n\n\
    🌟 **Offline-Modus aktiv** - Keine Internetverbindung erforderlich! 🍿".to_string()
}

pub async fn fetch_wisdom_gate_recommendations(api_keys: &[String], prompt: &str, model: &str, endpoint: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Check cache first
    let cache_key = format!("{}_{}_{}", prompt, model, endpoint);
    let mut hasher = DefaultHasher::new();
    cache_key.hash(&mut hasher);
    let cache_hash = hasher.finish();
    let cache_file = format!("/tmp/wisdom_cache_{}_{}.json", model.replace(['/', ':', '-'], "_"), cache_hash);
    
    // Load cache if exists
    if let Ok(cache_content) = std::fs::read_to_string(&cache_file) {
        if let Ok(cache_data) = serde_json::from_str::<serde_json::Value>(&cache_content) {
            if let Some(cached_result) = cache_data.get("result").and_then(|v| v.as_str()) {
                println!("📦 Prompt aus Cache: {}", model);
                return Ok(cached_result.to_string());
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    // Try each API key until one works
    for (i, api_key) in api_keys.iter().enumerate() {
        println!("Verwende API-Key {}/{} für Wisdom Gate", i + 1, api_keys.len());
        
        let headers = match api_key.starts_with("Bearer ") {
            true => api_key.clone(),
            false => format!("Bearer {}", api_key),
        };
        
        let request_body = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "model": model,
            "temperature": 0
        });
        
        // Rate limit handling with exponential backoff
        let mut attempt = 0;
        let max_attempts = 3;
        
        while attempt < max_attempts {
            let response = client
                .post(endpoint)
                .header("Content-Type", "application/json")
                .header("Authorization", &headers)
                .json(&request_body)
                .send()
                .await?;
            
            let status = response.status();
            
            if status == 429 {
                // Rate limit - check retry-after header or use exponential backoff
                let retry_after = response.headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(2_u64.pow(attempt));
                
                println!("Fehler 429: Too Many Requests bei API-Key {}. Retry-After: {} Sekunden", i + 1, retry_after);
                
                if attempt < max_attempts - 1 {
                    tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    attempt += 1;
                    continue;
                } else if i < api_keys.len() - 1 {
                    println!("Versuche nächsten API-Key...");
                    break;
                }
            }
            
            let response_text = response.text().await?;
            
            if !status.is_success() {
                println!("❌ API Error: Status {} mit Key {}", status, i + 1);
                
                // Bei 500er Fehlern (Server-Problem) ist es sinnlos, andere Keys zu probieren
                if status.as_u16() >= 500 {
                    println!("🛑 Server Error ({}). Breche ab - alle Keys würden scheitern.", status);
                    let mut hint = String::new();
                    if endpoint.contains("juheapi.com") {
                        hint.push_str("💡 Tipp: Probiere alternativ https://api.wisdom-gate.ai/v1/chat/completions\n");
                    } else if endpoint.contains("wisdom-gate.ai") {
                        hint.push_str("💡 Tipp: Probiere alternativ https://wisdom-gate.juheapi.com/v1/chat/completions\n");
                    }
                    hint.push_str("💡 Der Service scheint temporär nicht verfügbar zu sein.");
                    return Ok(format!("🌐 Server-Fehler ({}): Wisdom Gate Service nicht verfügbar.\nEndpoint: {}\n{}\n\n{}", status, endpoint, hint, crate::api::get_demo_recommendations()));
                }
                
                if i < api_keys.len() - 1 {
                    break; // Try next key
                } else {
                    return Ok(format!("API Fehler ({}): {}", status, response_text));
                }
            }
            
            // Parse successful response
            let response_json: serde_json::Value = serde_json::from_str(&response_text)?;
            
            // Log usage info if available
            if let Some(usage) = response_json.get("usage") {
                if let Some(total_tokens) = usage.get("total_tokens") {
                    println!("Cost: {} tokens", total_tokens);
                }
            }
            
            if let Some(choices) = response_json["choices"].as_array() {
                if let Some(first_choice) = choices.first() {
                    if let Some(message) = first_choice["message"].as_object() {
                        if let Some(content) = message["content"].as_str() {
                            let result = content.trim().to_string();
                            println!("KI-Tipp: {}", result);
                            
                            // Cache successful result
                            let cache_data = serde_json::json!({
                                "result": result,
                                "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
                            });
                            let _ = std::fs::write(&cache_file, serde_json::to_string_pretty(&cache_data).unwrap());
                            
                            return Ok(result);
                        }
                    }
                }
            }
            
            return Ok(format!("Modell {} lieferte keine verwertbare Antwort: {}", model, response_text));
        }
    }
    
    Ok("Alle API-Keys erschöpft".to_string())
}

// Parse API keys from string - support both JSON array and single key formats
fn parse_api_keys(api_key_input: &str) -> Vec<String> {
    if api_key_input.trim().starts_with('[') {
        // Try to parse as JSON array
        if let Ok(keys) = serde_json::from_str::<Vec<String>>(api_key_input) {
            return keys;
        }
    }
    // Fallback: treat as single key
    vec![api_key_input.to_string()]
}

// Wrapper function that handles network errors gracefully and tries fallback models
pub async fn fetch_wisdom_gate_recommendations_safe(api_key: &str, prompt: &str, model: &str, endpoint: &str) -> String {
    let api_keys = parse_api_keys(api_key);
    // Dynamische Modellliste: Stelle sicher, dass für Wisdom-Gate das richtige Default zuerst versucht wird
    let mut models_to_try: Vec<&str> = Vec::new();
    let model_l = model.to_ascii_lowercase();
    let wisdom_default = "wisdom-ai-dsr1";

    // Immer zuerst das Wisdom Gate Default-Modell versuchen, sofern nicht bereits gewählt
    if model_l != wisdom_default {
        models_to_try.push(wisdom_default);
    }
    // Dann das vom Nutzer gewählte Modell
    models_to_try.push(model);
    // Danach allgemeine Fallbacks
    models_to_try.extend_from_slice(&[
        "gpt-3.5-turbo",
        "gpt-4",
        "claude-3-sonnet",
        "gemini-pro",
        "llama-2-70b-chat",
        "mistral-7b-instruct",
        "openchat-3.5",
        "codellama-34b-instruct",
    ]);

    for (attempt, try_model) in models_to_try.iter().enumerate() {
        println!("🔄 Versuche Modell: {}", try_model);
        match fetch_wisdom_gate_recommendations(&api_keys, prompt, try_model, endpoint).await {
            Ok(content) => {
                if !content.starts_with("Modell") && !content.starts_with("API Fehler") {
                    if try_model != &model {
                        println!("✅ Fallback erfolgreich: {} funktioniert!", try_model);
                    }
                    return content;
                }
                println!("⚠️ Modell {} nicht verfügbar, versuche nächstes...", try_model);
            }
            Err(e) => {
                let err_txt = e.to_string();
                println!("❌ Fehler mit Modell {}: {}", try_model, err_txt);

                // Alternative endpoints für Wisdom Gate versuchen
                if attempt < 2 {
                    let alt = if endpoint.contains("juheapi.com") {
                        "https://api.wisdom-gate.ai/v1/chat/completions"
                    } else {
                        "https://wisdom-gate.juheapi.com/v1/chat/completions"
                    };
                    println!("🔄 Versuche alternativen Endpoint: {}", alt);
                    if let Ok(content) = fetch_wisdom_gate_recommendations(&api_keys, prompt, try_model, &alt).await {
                        if !content.starts_with("Modell") && !content.starts_with("API Fehler") {
                            println!("✅ Alternativer Endpoint erfolgreich!");
                            return content;
                        }
                    }
                }
            }
        }
    }
    format!("⚠️ Alle {} Modellversuche fehlgeschlagen.\n\n{}", models_to_try.len(), get_demo_recommendations())
}

// Fetch recommendations from Perplexity AI
pub async fn fetch_perplexity_recommendations(api_key: &str, prompt: &str, model: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Check cache first
    let cache_key = format!("perplexity_{}_{}_{}", prompt, model, api_key.len());
    let mut hasher = DefaultHasher::new();
    cache_key.hash(&mut hasher);
    let cache_hash = hasher.finish();
    let cache_file = format!("/tmp/perplexity_cache_{}_{}.json", model.replace(['/', ':', '-'], "_"), cache_hash);
    
    // Load cache if exists
    if let Ok(cache_content) = std::fs::read_to_string(&cache_file) {
        if let Ok(cache_data) = serde_json::from_str::<serde_json::Value>(&cache_content) {
            if let Some(cached_result) = cache_data.get("result").and_then(|v| v.as_str()) {
                println!("📦 Perplexity aus Cache: {}", model);
                return Ok(cached_result.to_string());
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    println!("🔮 Verwende Perplexity API mit Modell: {}", model);
    
    let headers = if api_key.starts_with("Bearer ") {
        api_key.to_string()
    } else {
        format!("Bearer {}", api_key)
    };
    
    let request_body = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": prompt
        }],
        "temperature": 0.2,
        "max_tokens": 2048
    });
    
    let response = client
        .post("https://api.perplexity.ai/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", &headers)
        .json(&request_body)
        .send()
        .await?;
    
    let status = response.status();
    let response_text = response.text().await?;
    
    if !status.is_success() {
        println!("❌ Perplexity API Error: Status {}", status);
        return Err(format!("Perplexity API error: {} - {}", status, response_text).into());
    }
    
    // Parse response
    let json: serde_json::Value = serde_json::from_str(&response_text)?;
    let content = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    
    if content.is_empty() {
        return Err("Leere Antwort von Perplexity".into());
    }
    
    // Cache the result
    let cache_data = serde_json::json!({
        "result": content,
        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    });
    let _ = std::fs::write(&cache_file, serde_json::to_string_pretty(&cache_data).unwrap_or_default());
    
    Ok(content.to_string())
}

pub async fn fetch_perplexity_recommendations_safe(api_key: &str, prompt: &str, model: &str) -> String {
    // Try Perplexity with the selected model
    println!("🔮 Starte Perplexity API-Anfrage...");
    
    match fetch_perplexity_recommendations(api_key, prompt, model).await.map_err(|e| e.to_string()) {
        Ok(content) => {
            println!("✅ Perplexity Empfehlungen erfolgreich abgerufen");
            content
        }
        Err(error_msg) => {
            println!("❌ Perplexity Fehler: {}", error_msg);
            
            // Try with fallback model
            let fallback_model = "sonar";
            if model != fallback_model {
                println!("🔄 Versuche Fallback-Modell: {}", fallback_model);
                if let Ok(content) = fetch_perplexity_recommendations(api_key, prompt, fallback_model).await {
                    println!("✅ Fallback erfolgreich!");
                    return content;
                }
            }
            
            format!("⚠️ Perplexity API nicht verfügbar: {}\n\n{}", error_msg, get_demo_recommendations())
        }
    }
}

// Cognora Toolkit API
pub async fn fetch_cognora_recommendations(api_key: &str, prompt: &str, model: &str) -> Result<String, Box<dyn std::error::Error>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    // Create cache key
    let mut hasher = DefaultHasher::new();
    model.hash(&mut hasher);
    prompt.hash(&mut hasher);
    let hash = hasher.finish();
    let cache_file = format!("/tmp/cognora_cache_{}_{:x}.json", model, hash);
    
    // Load cache if exists
    if let Ok(cache_content) = std::fs::read_to_string(&cache_file) {
        if let Ok(cache_data) = serde_json::from_str::<serde_json::Value>(&cache_content) {
            if let Some(cached_result) = cache_data.get("result").and_then(|v| v.as_str()) {
                println!("📦 Cognora aus Cache: {}", model);
                return Ok(cached_result.to_string());
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    println!("🧠 Verwende Cognora API mit Modell: {}", model);
    
    let request_body = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": prompt
        }],
        "temperature": 0.2,
        "max_tokens": 2048
    });
    
    let response = client
        .post("https://api.cognora-toolkit.com/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;
    
    let status = response.status();
    let response_text = response.text().await?;
    
    if !status.is_success() {
        println!("❌ Cognora API Error: Status {}", status);
        return Err(format!("Cognora API error: {} - {}", status, response_text).into());
    }
    
    let json: serde_json::Value = serde_json::from_str(&response_text)?;
    let content = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    
    if content.is_empty() {
        return Err("Leere Antwort von Cognora".into());
    }
    
    // Cache the result
    let cache_data = serde_json::json!({
        "result": content,
        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    });
    let _ = std::fs::write(&cache_file, serde_json::to_string_pretty(&cache_data).unwrap_or_default());
    
    Ok(content.to_string())
}

pub async fn fetch_cognora_recommendations_safe(api_key: &str, prompt: &str, model: &str) -> String {
    println!("🧠 Starte Cognora API-Anfrage...");
    
    match fetch_cognora_recommendations(api_key, prompt, model).await.map_err(|e| e.to_string()) {
        Ok(content) => {
            println!("✅ Cognora Empfehlungen erfolgreich abgerufen");
            content
        }
        Err(error_msg) => {
            println!("❌ Cognora Fehler: {}", error_msg);
            
            let fallback_model = "cognora-3";
            if model != fallback_model {
                println!("🔄 Versuche Fallback-Modell: {}", fallback_model);
                if let Ok(content) = fetch_cognora_recommendations(api_key, prompt, fallback_model).await {
                    println!("✅ Fallback erfolgreich!");
                    return content;
                }
            }
            
            format!("⚠️ Cognora API nicht verfügbar: {}\n\n{}", error_msg, get_demo_recommendations())
        }
    }
}

// Google Gemini API
pub async fn fetch_gemini_recommendations(api_key: &str, prompt: &str, model: &str) -> Result<String, Box<dyn std::error::Error>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    model.hash(&mut hasher);
    prompt.hash(&mut hasher);
    let hash = hasher.finish();
    let cache_file = format!("/tmp/gemini_cache_{}_{:x}.json", model, hash);
    
    if let Ok(cache_content) = std::fs::read_to_string(&cache_file) {
        if let Ok(cache_data) = serde_json::from_str::<serde_json::Value>(&cache_content) {
            if let Some(cached_result) = cache_data.get("result").and_then(|v| v.as_str()) {
                println!("📦 Gemini aus Cache: {}", model);
                return Ok(cached_result.to_string());
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    println!("💎 Verwende Gemini API mit Modell: {}", model);
    
    let request_body = serde_json::json!({
        "contents": [{
            "parts": [{
                "text": prompt
            }]
        }],
        "generationConfig": {
            "temperature": 0.2,
            "maxOutputTokens": 2048
        }
    });
    
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key);
    
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;
    
    let status = response.status();
    let response_text = response.text().await?;
    
    if !status.is_success() {
        println!("❌ Gemini API Error: Status {}", status);
        return Err(format!("Gemini API error: {} - {}", status, response_text).into());
    }
    
    let json: serde_json::Value = serde_json::from_str(&response_text)?;
    let content = json
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    
    if content.is_empty() {
        return Err("Leere Antwort von Gemini".into());
    }
    
    let cache_data = serde_json::json!({
        "result": content,
        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    });
    let _ = std::fs::write(&cache_file, serde_json::to_string_pretty(&cache_data).unwrap_or_default());
    
    Ok(content.to_string())
}

pub async fn fetch_gemini_recommendations_safe(api_key: &str, prompt: &str, model: &str) -> String {
    println!("💎 Starte Gemini API-Anfrage...");
    
    match fetch_gemini_recommendations(api_key, prompt, model).await.map_err(|e| e.to_string()) {
        Ok(content) => {
            println!("✅ Gemini Empfehlungen erfolgreich abgerufen");
            content
        }
        Err(error_msg) => {
            println!("❌ Gemini Fehler: {}", error_msg);
            
            let fallback_model = "gemini-1.5-flash";
            if model != fallback_model {
                println!("🔄 Versuche Fallback-Modell: {}", fallback_model);
                if let Ok(content) = fetch_gemini_recommendations(api_key, prompt, fallback_model).await {
                    println!("✅ Fallback erfolgreich!");
                    return content;
                }
            }
            
            format!("⚠️ Gemini API nicht verfügbar: {}\n\n{}", error_msg, get_demo_recommendations())
        }
    }
}

// OpenAI API
pub async fn fetch_openai_recommendations(api_key: &str, prompt: &str, model: &str) -> Result<String, Box<dyn std::error::Error>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    model.hash(&mut hasher);
    prompt.hash(&mut hasher);
    let hash = hasher.finish();
    let cache_file = format!("/tmp/openai_cache_{}_{:x}.json", model, hash);
    
    if let Ok(cache_content) = std::fs::read_to_string(&cache_file) {
        if let Ok(cache_data) = serde_json::from_str::<serde_json::Value>(&cache_content) {
            if let Some(cached_result) = cache_data.get("result").and_then(|v| v.as_str()) {
                println!("📦 OpenAI aus Cache: {}", model);
                return Ok(cached_result.to_string());
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    println!("🤖 Verwende OpenAI API mit Modell: {}", model);
    
    let request_body = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": prompt
        }],
        "temperature": 0.2,
        "max_tokens": 2048
    });
    
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;
    
    let status = response.status();
    let response_text = response.text().await?;
    
    if !status.is_success() {
        println!("❌ OpenAI API Error: Status {}", status);
        return Err(format!("OpenAI API error: {} - {}", status, response_text).into());
    }
    
    let json: serde_json::Value = serde_json::from_str(&response_text)?;
    let content = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    
    if content.is_empty() {
        return Err("Leere Antwort von OpenAI".into());
    }
    
    let cache_data = serde_json::json!({
        "result": content,
        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    });
    let _ = std::fs::write(&cache_file, serde_json::to_string_pretty(&cache_data).unwrap_or_default());
    
    Ok(content.to_string())
}

pub async fn fetch_openai_recommendations_safe(api_key: &str, prompt: &str, model: &str) -> String {
    println!("🤖 Starte OpenAI API-Anfrage...");
    
    match fetch_openai_recommendations(api_key, prompt, model).await.map_err(|e| e.to_string()) {
        Ok(content) => {
            println!("✅ OpenAI Empfehlungen erfolgreich abgerufen");
            content
        }
        Err(error_msg) => {
            println!("❌ OpenAI Fehler: {}", error_msg);
            
            let fallback_model = "gpt-3.5-turbo";
            if model != fallback_model {
                println!("🔄 Versuche Fallback-Modell: {}", fallback_model);
                if let Ok(content) = fetch_openai_recommendations(api_key, prompt, fallback_model).await {
                    println!("✅ Fallback erfolgreich!");
                    return content;
                }
            }
            
            format!("⚠️ OpenAI API nicht verfügbar: {}\n\n{}", error_msg, get_demo_recommendations())
        }
    }
}

/// Fetch recent VOD/Series items (first 10 from each category, sorted by release date)
pub async fn fetch_recently_added(cfg: &Config) -> Result<Vec<Item>, String> {
    let mut all_items = Vec::new();
    
    // Fetch VOD categories
    let vod_cats = fetch_categories(cfg, "get_vod_categories")
        .await
        .map_err(|e| e.to_string())?;
    
    // Take first 2 VOD categories and get first 10 items from each
    for cat in vod_cats.iter().take(2) {
        if let Ok(items) = fetch_items(cfg, "vod", &cat.id).await {
            all_items.extend(items.into_iter().take(10));
        }
    }
    
    // Fetch Series categories  
    let series_cats = fetch_categories(cfg, "get_series_categories")
        .await
        .map_err(|e| e.to_string())?;
    
    // Take first 2 Series categories and get first 10 items from each
    for cat in series_cats.iter().take(2) {
        if let Ok(items) = fetch_items(cfg, "series", &cat.id).await {
            all_items.extend(items.into_iter().take(10));
        }
    }
    
    // Sort by release_date (newest first) - handle missing dates
    all_items.sort_by(|a, b| {
        match (&b.release_date, &a.release_date) {
            (Some(date_b), Some(date_a)) => date_b.cmp(date_a),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
    
    // Return first 20 items
    Ok(all_items.into_iter().take(20).collect())
}

