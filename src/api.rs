use crate::cache::{load_cache, load_stale_cache, save_cache};
use crate::models::{Category, Config, Episode, Item};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;

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

                // DNS / Verbindungsfehler früh erkennen und abbrechen (alle Modelle würden scheitern)
                let lower = err_txt.to_lowercase();
                let is_dns = lower.contains("dns") || lower.contains("failed to lookup") || lower.contains("nodename nor servname") || lower.contains("name or service not known") || lower.contains("network unreachable") || lower.contains("connection refused") || lower.contains("no route to host");
                let is_connect = lower.contains("error trying to connect") || lower.contains("connect timeout") || lower.contains("timed out") || lower.contains("could not connect");
                if attempt == 0 && (is_dns || is_connect) {
                    println!("🛑 Verbindungsfehler ({}). Versuche alternativen Endpoint...", if is_dns {"DNS"} else {"Connect"});
                    // Versuche automatisch alternative Endpoint-Varianten einmal
                    let mut alt_endpoints: Vec<String> = Vec::new();
                    if endpoint.contains("wisdom-gate.juheapi.com") {
                        alt_endpoints.push("https://api.wisdom-gate.ai/v1/chat/completions".to_string());
                        alt_endpoints.push("https://wisdom-gate.juheapi.com/v1/chat/completions".to_string()); // original
                    } else if endpoint.contains("api.wisdom-gate.ai") {
                        alt_endpoints.push("https://wisdom-gate.juheapi.com/v1/chat/completions".to_string());
                        alt_endpoints.push("https://api.wisdom-gate.ai/v1/chat/completions".to_string()); // original
                    } else if endpoint.contains("wisdomgate") {
                        alt_endpoints.push("https://api.wisdom-gate.ai/v1/chat/completions".to_string());
                        alt_endpoints.push("https://wisdom-gate.juheapi.com/v1/chat/completions".to_string());
                    }

                    for alt in alt_endpoints {
                        println!("🔁 Teste alternativen Endpoint: {}", alt);
                        match fetch_wisdom_gate_recommendations(&api_keys, prompt, try_model, &alt).await {
                            Ok(content) => {
                                if !content.starts_with("API Fehler") && !content.starts_with("Modell") {
                                    println!("✅ Alternativer Endpoint erfolgreich");
                                    return content;
                                }
                            }
                            Err(e2) => {
                                println!("⚠️ Alternativer Endpoint fehlgeschlagen: {}", e2);
                            }
                        }
                    }

                    println!("🛑 Schwerer Verbindungsfehler ({}). Breche Fallback-Kette ab.", if is_dns {"DNS"} else {"Connect"});
                    let mut hint = String::new();
                    if endpoint.contains("wisdom-gate") {
                        hint.push_str("💡 Tipp: Probiere alternativ https://api.wisdomgate.ai/v1/chat/completions (ohne Bindestrich)\n");
                    } else if endpoint.contains("wisdomgate") {
                        hint.push_str("💡 Tipp: Probiere alternativ https://api.wisdom-gate.ai/v1/chat/completions (mit Bindestrich)\n");
                    }
                    hint.push_str("💡 Prüfe außerdem: Internetzugang, DNS, Proxy/VPN, Firewall.");
                    return format!(
                        "🌐 DNS/Verbindungsfehler: {}\nEndpoint: {} nicht erreichbar.\n{}\n\n{}",
                        err_txt,
                        endpoint,
                        hint,
                        get_demo_recommendations()
                    );
                }
                // Andere Fehler -> weiter versuchen
            }
        }
    }

    println!("🌐 Alle Modelle fehlgeschlagen - Verwende Demo-Empfehlungen");
    format!("🌐 **Offline-Modus** (Alle Modelle fehlgeschlagen)\n\n{}", get_demo_recommendations())
}
