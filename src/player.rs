use std::io;
use std::process::Command;
use std::process::Stdio;
use crate::models::Config;
use crate::logger::{log_line, log_command, log_error};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType { Live, Vod, Series, Default }

fn base_url(addr: &str) -> String {
    // Strip trailing / and optional /player_api.php to get the service root
    let mut a = addr.trim().trim_end_matches('/').to_string();
    if a.ends_with("/player_api.php") {
        a.truncate(a.len() - "/player_api.php".len());
        a = a.trim_end_matches('/').to_string();
    }
    if !a.starts_with("http://") && !a.starts_with("https://") {
        format!("http://{}", a)
    } else {
        a
    }
}

pub fn build_stream_url(cfg: &Config, stream_id: &str) -> String {
    // Many Xtream servers prefer HLS playlists for live streams
    format!(
        "{}/live/{}/{}/{}.m3u8",
        base_url(&cfg.address),
        cfg.username,
        cfg.password,
        stream_id
    )
}
pub fn build_vod_stream_url(cfg: &Config, stream_id: &str, ext: &str) -> String {
    let ext = ext.trim_start_matches('.');
    format!(
        "{}/movie/{}/{}/{}.{}",
        base_url(&cfg.address),
        cfg.username,
        cfg.password,
        stream_id,
        ext
    )
}
pub fn build_series_episode_stream_url(cfg: &Config, episode_id: &str, ext: &str) -> String {
    let ext = ext.trim_start_matches('.');
    format!(
        "{}/series/{}/{}/{}.{}",
        base_url(&cfg.address),
        cfg.username,
        cfg.password,
        episode_id,
        ext
    )
}
pub fn build_url_by_type(cfg: &Config, id: &str, info: &str, container_ext: Option<&str>) -> String {
    match info {
        "Channel" => build_stream_url(cfg, id),
        "Movie" => build_vod_stream_url(cfg, id, container_ext.unwrap_or("mp4")),
        "SeriesEpisode" => build_series_episode_stream_url(cfg, id, container_ext.unwrap_or("mp4")),
        _ => build_stream_url(cfg, id),
    }
}

/// Build VLC argument vector based on stream type and config (excluding program and URL)
fn build_vlc_args(cfg: &Config, st: StreamType) -> Vec<String> {
    let (net_ms, live_ms, file_ms) = apply_bias(cfg);
    let mut args = Vec::new();
    args.push("--fullscreen".into());
    let mut net_val = net_ms;
    if net_val > 12000 {
        log_line(&format!("Warnung: network-caching {}ms > 12000ms -> setze auf 12000 für geringere Latenz", net_val));
        net_val = 12000;
    }
    if net_val > 0 { args.push(format!("--network-caching={}", net_val)); }
    match st {
        StreamType::Live => {
            if live_ms > 0 { args.push(format!("--live-caching={}", live_ms)); }
            // `--http-reconnect` ist stabil genug; behalten für Live Streams
            if cfg.vlc_http_reconnect { args.push("--http-reconnect".into()); }
        }
        StreamType::Vod | StreamType::Series | StreamType::Default => {
            if file_ms > 0 { args.push(format!("--file-caching={}", file_ms)); }
        }
    }
    // Entfernt: --mux-caching / --http-timeout da diese bei manchen VLC Builds zu sofortigem Exit führen (nicht überall verfügbar oder anderer Namensraum)
    if !cfg.vlc_extra_args.trim().is_empty() {
        for part in cfg.vlc_extra_args.split_whitespace() { args.push(part.to_string()); }
    }
    if cfg.vlc_verbose { args.insert(0, "-vvv".into()); }
    args
}

// Bias Anwendung: linear interpolation zwischen Minimal und Maximalwerten
pub(crate) fn apply_bias(cfg: &Config) -> (u32, u32, u32) {
    let b = cfg.vlc_profile_bias.min(100) as f32 / 100.0; // 0.0 .. 1.0
    // Minimal (Latenz) vs Maximal (Stabilität) Grenzen definieren
    let net_min = 2000u32; let net_max = cfg.vlc_network_caching_ms.max(8000); // fallback auf config maxima
    let live_min = 1500u32; let live_max = cfg.vlc_live_caching_ms.max(6000);
    let file_min = 1000u32; let file_max = cfg.vlc_file_caching_ms.max(5000);
    let lerp = |mn: u32, mx: u32| -> u32 { mn + (((mx - mn) as f32) * b) as u32 };
    (lerp(net_min, net_max), lerp(live_min, live_max), lerp(file_min, file_max))
}

// Einfache Flag-Erkennung: Parse "vlc --help" einmalig und extrahiere bekannte Optionen (Heuristik: "--" am Zeilenanfang oder nach zwei Spaces)
fn probe_vlc_supported_flags() -> Vec<String> {
    let output = Command::new("vlc").arg("--help").stdout(Stdio::piped()).stderr(Stdio::null()).output();
    if let Ok(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut flags = Vec::new();
            for line in text.lines() {
                if let Some(idx) = line.find("--") {
                    // Nimm bis erstes Space danach
                    let rest = &line[idx..];
                    let flag = rest.split_whitespace().next().unwrap_or("");
                    if flag.starts_with("--") && flag.len() > 3 && flag.chars().all(|c| c.is_ascii() ) {
                        flags.push(flag.to_string());
                    }
                }
            }
            flags.sort(); flags.dedup();
            return flags;
        }
    }
    // Fallback Basisliste
    vec![
        "--fullscreen".into(),
        "--network-caching".into(),
        "--live-caching".into(),
        "--file-caching".into(),
        "--http-reconnect".into(),
    ]
}

// Filter ungekennzeichnete Flags heraus (lassen Werte-Parameter wie =123 bestehen)
fn filter_supported(args: &[String], supported: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        if !a.starts_with("--") { out.push(a.clone()); continue; }
        let base = if let Some((b,_)) = a.split_once('=') { b } else { a.as_str() };
        if supported.iter().any(|s| s == base) { out.push(a.clone()); } else { log_line(&format!("Filter unsupported VLC flag: {}", a)); }
    }
    out
}

/// Backwards-compatible command builder returning a single string with {URL} placeholder
pub fn get_vlc_command_for_stream_type(stream_type: StreamType, cfg: &Config) -> String {
    let args = build_vlc_args(cfg, stream_type);
    let mut cmd = String::from("vlc");
    for a in &args { cmd.push(' '); cmd.push_str(a); }
    cmd.push(' ');
    cmd.push_str("{URL}");
    cmd
}

/// Detect stream type from URL patterns
pub fn detect_stream_type(stream_url: &str) -> StreamType {
    if stream_url.contains("/live/") { StreamType::Live }
    else if stream_url.contains("/movie/") { StreamType::Vod }
    else if stream_url.contains("/series/") { StreamType::Series }
    else if stream_url.ends_with(".m3u8") || stream_url.contains("playlist.m3u8") { StreamType::Live }
    else if stream_url.ends_with(".mp4") || stream_url.ends_with(".mkv") || stream_url.ends_with(".avi") { StreamType::Vod }
    else { StreamType::Default }
}

pub fn start_player(cfg: &Config, stream_url: &str) -> io::Result<()> {
    log_line(&format!("Start player URL={}", stream_url));
    let st = detect_stream_type(stream_url);
    log_line(&format!("Detected stream type: {:?}", st));

    // MPV Pfad: wenn aktiviert, baue mpv Argumente und starte direkt
    if cfg.use_mpv {
        let (net_ms, live_ms, file_ms) = apply_bias(cfg);
        let to_secs = |ms: u32| -> u32 { (ms / 1000).max(1) };
        let mut args: Vec<String> = vec!["--force-window=no".into(), "--fullscreen".into()];
        let net_secs = if cfg.mpv_cache_secs_override != 0 { cfg.mpv_cache_secs_override } else { to_secs(net_ms) };
        args.push(format!("--cache-secs={}", net_secs));
        let readahead = match st {
            StreamType::Live => to_secs(if cfg.mpv_readahead_secs_override != 0 { cfg.mpv_readahead_secs_override * 1000 } else { live_ms.max(net_ms/2) }),
            StreamType::Vod | StreamType::Series | StreamType::Default => to_secs(if cfg.mpv_readahead_secs_override != 0 { cfg.mpv_readahead_secs_override * 1000 } else { file_ms.max(1500) }),
        };
        args.push(format!("--demuxer-readahead-secs={}", readahead));
        if !cfg.mpv_extra_args.trim().is_empty() {
            for part in cfg.mpv_extra_args.split_whitespace() { args.push(part.to_string()); }
        }
        args.push(stream_url.to_string());
        log_line(&format!("Starte mpv args={:?}", args));
        let start_time = std::time::Instant::now();
        match Command::new("mpv").args(&args).spawn() {
            Ok(child) => {
                log_line(&format!("mpv gestartet pid={}", child.id()));
                let dur = start_time.elapsed().as_millis();
                if let Some(tx) = crate::GLOBAL_TX.get() { let _ = tx.send(crate::app_state::Msg::VlcDiagUpdate { lines: vec![format!("mpv spawn time {} ms", dur)], suggestion: None }); }
                return Ok(());
            }
            Err(e) => {
                log_error("mpv Start fehlgeschlagen – Fallback auf VLC", &e);
                if let Some(tx) = crate::GLOBAL_TX.get() { let _ = tx.send(crate::app_state::Msg::PlayerSpawnFailed { player: "mpv".into(), error: e.to_string() }); }
                // Fallback auf VLC Weg unten
            }
        }
    }

    if cfg.vlc_diagnose_on_start && !cfg.use_mpv {
        spawn_vlc_diagnostics(stream_url, cfg);
    }
    if cfg.vlc_continuous_diagnostics && !cfg.use_mpv {
        if let Some(tx) = crate::GLOBAL_TX.get() {
            log_line("Starte kontinuierliche VLC Diagnose");
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            spawn_vlc_continuous_diagnostics(tx.clone(), stream_url.to_string(), cfg.clone(), stop.clone());
            // store stop flag via message? (simpler: ignore here, main state stores separately when starting player)
        } else { log_line("GLOBAL_TX nicht initialisiert – Continuous Diagnostics deaktiviert"); }
    }

    let supported_flags = probe_vlc_supported_flags();
    let (program, mut parts): (String, Vec<String>) = if !cfg.player_command.trim().is_empty() {
        let raw = cfg.player_command.trim().to_string();
        let mut ps: Vec<String> = raw.split_whitespace().map(|s| s.to_string()).collect();
        if ps.is_empty() { return Ok(()); }
        (ps.remove(0), ps)
    } else {
        ("vlc".into(), build_vlc_args(cfg, st))
    };
    // Filter vor Placeholder Ersetzung
    parts = filter_supported(&parts, &supported_flags);
    let mut replaced = false;
    for p in &mut parts {
        if p == "URL" || p == "{URL}" || p.to_lowercase() == "{url}" {
            *p = stream_url.to_string();
            replaced = true;
        }
    }
    if !replaced { parts.push(stream_url.to_string()); }
    log_line(&format!("Program={} args={:?}", program, parts));

    // VLC spezifische Behandlung auf macOS: Reuse laufender Instanz (URL via AppleScript schicken)
    let using_vlc = program.to_lowercase().contains("vlc");
    #[cfg(target_os = "macos")]
    {
        if using_vlc {
            if cfg.reuse_vlc && is_vlc_running() {
                log_line("VLC reuse aktiv: versuche laufender Instanz neue URL zu schicken (AppleScript)");
                if send_url_to_vlc(stream_url) {
                    log_line("AppleScript OpenURL erfolgreich an laufende VLC Instanz gesendet");
                    log_line("Hinweis: Netzwerk-/Live-Caching Parameter können nur beim Start einer neuen Instanz wirken");
                    return Ok(());
                } else {
                    log_line("AppleScript Reuse fehlgeschlagen – Fallback: öffne URL über 'open -a VLC'");
                    // minimal Fallback (keine neuen Startup Parameter)
                    let fallback = Command::new("open")
                        .arg("-a").arg("VLC")
                        .arg(stream_url)
                        .spawn();
                    if let Ok(child) = fallback {
                        log_line(&format!("Fallback open -a VLC reuse pid={}", child.id()));
                        return Ok(());
                    }
                    // Wenn selbst Fallback nicht geht -> weiter unten normaler Start
                }
            }
            // Keine Reuse oder VLC läuft nicht -> neue Instanz starten mit Parametern
            log_line("Starte neue VLC Instanz (macOS) mit Parametern über 'open -a VLC'");
            log_line(&format!("URL={}", stream_url));
            log_line(&format!("VLC args: {:?}", parts));
            let mut open_cmd = Command::new("open");
            if !cfg.reuse_vlc {
                open_cmd.arg("-n"); // explizit neue Instanz erzwingen falls reuse deaktiviert
            }
            open_cmd.arg("-a").arg("VLC");
            if !parts.is_empty() {
                open_cmd.arg("--args").args(&parts);
            } else {
                open_cmd.arg(stream_url);
            }
            match open_cmd.spawn() {
                Ok(child) => {
                    log_line(&format!("Neue VLC Instanz gestartet pid={}", child.id()));
                    return Ok(());
                }
                Err(e) => {
                    log_line(&format!("Fehler beim Start über 'open': {} – versuche direkten Spawn", e));
                    // Fallback auf direkten Spawn unten
                }
            }
        }
    }

    // Log and spawn the command; try to capture basic status
    log_command(&program, &parts);
    let start_time = std::time::Instant::now();
    match Command::new(&program).args(&parts).spawn() {
        Ok(child) => {
            log_line(&format!("Spawned player pid={} program={} args={:?}", child.id(), program, parts));
            let dur = start_time.elapsed().as_millis();
            if let Some(tx) = crate::GLOBAL_TX.get() { let _ = tx.send(crate::app_state::Msg::VlcDiagUpdate { lines: vec![format!("{} spawn time {} ms", program, dur)], suggestion: None }); }
        }
        Err(e) => {
            log_error("Failed to spawn player", &e);
            if let Some(tx) = crate::GLOBAL_TX.get() { let _ = tx.send(crate::app_state::Msg::PlayerSpawnFailed { player: program.clone(), error: e.to_string() }); }
            // Fallback: mpv versuchen falls installiert
            log_line("Versuche Fallback auf mpv");
            let mpv_args = vec!["--fullscreen".to_string(), stream_url.to_string()];
            match Command::new("mpv").args(&mpv_args).spawn() {
                Ok(child2) => {
                    log_line(&format!("Fallback mpv gestartet pid={}", child2.id()));
                    return Ok(());
                }
                Err(e2) => {
                    log_error("Fallback mpv fehlgeschlagen", &e2);
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

fn spawn_vlc_diagnostics(url: &str, cfg: &Config) {
    let diag_args = ["--fullscreen", url];
    let mut cmd = Command::new("vlc");
    if cfg.vlc_verbose { cmd.arg("-vvv"); }
    for a in &diag_args { cmd.arg(a); }
    match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                if let Some(mut out) = child.stderr.take() {
                    use std::io::Read;
                    let mut buf = String::new();
                    let _ = out.read_to_string(&mut buf);
                    let truncated = if buf.len() > 8000 { format!("{}...<truncated>", &buf[..8000]) } else { buf };
                    // Kein globaler AppState verfügbar hier – wir loggen nur die erste Diagnose-Ausgabe.
                    log_line(&format!("VLC Diagnose Output (truncated): {}", truncated.replace('\n', " | ")));
                }
            });
        }
        Err(e) => log_error("Konnte VLC Diagnose nicht starten", &e),
    }
}

fn spawn_vlc_continuous_diagnostics(tx: std::sync::mpsc::Sender<crate::app_state::Msg>, url: String, cfg: Config, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    std::thread::spawn(move || {
        let mut cmd = Command::new("vlc");
        if cfg.vlc_verbose { cmd.arg("-vvv"); }
        cmd.arg("--fullscreen").arg(&url);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let start = std::time::Instant::now();
        match cmd.spawn() {
            Ok(mut child) => {
                if let Some(err) = child.stderr.take() {
                    use std::io::BufRead;
                    let reader = std::io::BufReader::new(err);
                    let mut buffering_events = 0u32;
                    let mut lines_batch: Vec<String> = Vec::new();
                    for line in reader.lines().flatten() {
                        if stop.load(std::sync::atomic::Ordering::Relaxed) { let _ = child.kill(); let _ = tx.send(crate::app_state::Msg::DiagnosticsStopped); break; }
                        let l = line.trim().to_string();
                        if l.contains("buffering") || l.contains("Buffering") { buffering_events += 1; }
                        if l.len() > 2 { lines_batch.push(l); }
                        if lines_batch.len() >= 10 {
                            // Heuristik Vorschlag
                            let suggestion = if buffering_events > 5 {
                                // Viele buffering events -> Erhöhe network/live caching leicht
                                Some((cfg.vlc_network_caching_ms + 1000, cfg.vlc_live_caching_ms + 500, cfg.vlc_file_caching_ms))
                            } else if start.elapsed() > std::time::Duration::from_secs(60) && buffering_events == 0 {
                                // Sehr stabil -> reduzieren etwas
                                Some((cfg.vlc_network_caching_ms.saturating_sub(500), cfg.vlc_live_caching_ms.saturating_sub(250), cfg.vlc_file_caching_ms))
                            } else { None };
                            let _ = tx.send(crate::app_state::Msg::VlcDiagUpdate { lines: lines_batch.clone(), suggestion });
                            lines_batch.clear();
                            // Low CPU Mode: kurze Pause um Nachrichtenfluss zu drosseln
                            if cfg.low_cpu_mode {
                                std::thread::sleep(std::time::Duration::from_millis(120));
                            } else {
                                std::thread::sleep(std::time::Duration::from_millis(30));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log_error("Continuous VLC Diagnose konnte nicht gestartet werden", &e);
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn is_vlc_running() -> bool {
    // Use pgrep to check for a process named exactly "VLC"; best-effort
    Command::new("pgrep")
        .arg("-x")
        .arg("VLC")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn send_url_to_vlc(url: &str) -> bool {
    // AppleScript an VLC schicken um URL zu öffnen ohne neue Instanz zu starten
    // Erst versuchen über OpenURL (schnell), dann fallback auf open location
    let script = format!("tell application \"VLC\"\ntry\nOpenURL \"{}\"\nactivate\nreturn true\non error\nreturn false\nend try\nend tell", url);
    match Command::new("osascript").arg("-e").arg(script).status() {
        Ok(s) if s.success() => return true,
        _ => {}
    }
    // Fallback Variante
    let script2 = format!("tell application \"VLC\"\ntry\nopen location \"{}\"\nactivate\nreturn true\non error\nreturn false\nend try\nend tell", url);
    match Command::new("osascript").arg("-e").arg(script2).status() {
        Ok(s) => s.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_type_detection() {
        assert!(matches!(detect_stream_type("http://server:8080/live/user/pass/12345.m3u8"), StreamType::Live));
        assert!(matches!(detect_stream_type("http://server:8080/movie/user/pass/12345.mp4"), StreamType::Vod));
        assert!(matches!(detect_stream_type("http://server:8080/series/user/pass/12345.mkv"), StreamType::Series));
        assert!(matches!(detect_stream_type("http://example.com/stream.m3u8"), StreamType::Live));
        assert!(matches!(detect_stream_type("http://example.com/video.mp4"), StreamType::Vod));
        assert!(matches!(detect_stream_type("http://example.com/unknown"), StreamType::Default));
    }

    #[test]
    fn test_vlc_command_generation() {
        let mut cfg = Config::default();
        cfg.vlc_network_caching_ms = 25000;
        cfg.vlc_live_caching_ms = 15000;
        cfg.vlc_file_caching_ms = 3000;
        // Set maximal Bias to ensure upper bounds applied for predictable assertions
        cfg.vlc_profile_bias = 100;
        let live_cmd = get_vlc_command_for_stream_type(StreamType::Live, &cfg);
        let vod_cmd = get_vlc_command_for_stream_type(StreamType::Vod, &cfg);
        
        // All commands should have basic VLC parameters
        assert!(live_cmd.contains("vlc --fullscreen"));
        assert!(vod_cmd.contains("vlc --fullscreen"));
        
    // Live streams should have both network and live caching (network capped to 12000 for latency)
    assert!(live_cmd.contains("--network-caching=12000"));
        assert!(live_cmd.contains("--live-caching=15000"));
        
        // VOD should only have network caching
    assert!(vod_cmd.contains("--network-caching=12000"));
        assert!(!vod_cmd.contains("--live-caching"));
    // File caching bias with upper bound fallback reaches 5000 (max of default upper bound)
    assert!(vod_cmd.contains("--file-caching=5000"));
        
        // All should have URL placeholder
        assert!(live_cmd.contains("{URL}"));
        assert!(vod_cmd.contains("{URL}"));
    }

    #[test]
    fn test_removed_unstable_flags() {
        let mut cfg = Config::default();
        cfg.vlc_network_caching_ms = 5000;
        cfg.vlc_live_caching_ms = 3000;
        cfg.vlc_file_caching_ms = 2000;
        cfg.vlc_mux_caching_ms = 9999; // should be ignored now
        cfg.vlc_timeout_ms = 12345;    // should be ignored now
        let cmd_live = get_vlc_command_for_stream_type(StreamType::Live, &cfg);
        assert!(!cmd_live.contains("--mux-caching="));
        assert!(!cmd_live.contains("--http-timeout="));
        // keep http-reconnect when enabled
        cfg.vlc_http_reconnect = true;
        let cmd_live2 = get_vlc_command_for_stream_type(StreamType::Live, &cfg);
        assert!(cmd_live2.contains("--http-reconnect"));
    }

    #[test]
    fn test_bias_application() {
        let mut cfg = Config::default();
        cfg.vlc_network_caching_ms = 8000; // upper
        cfg.vlc_live_caching_ms = 6000;
        cfg.vlc_file_caching_ms = 5000;
        cfg.vlc_profile_bias = 0; // minimal
        let (n0,l0,f0) = super::apply_bias(&cfg);
        assert!(n0 <= 3000 && l0 <= 2000 && f0 <= 1500, "low bias should keep small caches");
        cfg.vlc_profile_bias = 100; // maximal
        let (n1,l1,f1) = super::apply_bias(&cfg);
        assert!(n1 >= 7500 && l1 >= 5500 && f1 >= 4500, "high bias should approach upper bounds");
    }

    #[test]
    fn test_bias_midpoint() {
        let mut cfg = Config::default();
        cfg.vlc_network_caching_ms = 8000; // upper
        cfg.vlc_live_caching_ms = 6000;
        cfg.vlc_file_caching_ms = 5000;
        cfg.vlc_profile_bias = 50; // midpoint
        let (n,l,f) = super::apply_bias(&cfg);
        // Expect exact linear interpolation values: 2000+(6000*0.5)=5000, 1500+(4500*0.5)=3750, 1000+(4000*0.5)=3000
        assert_eq!(n, 5000, "network midpoint should be 5000");
        assert_eq!(l, 3750, "live midpoint should be 3750");
        assert_eq!(f, 3000, "file midpoint should be 3000");
    }

    #[test]
    fn test_flag_filtering() {
        let supported = vec!["--fullscreen".into(), "--network-caching".into()];
        let args = vec!["--fullscreen".into(), "--network-caching=5000".into(), "--doesnotexist=3".into(), "--another".into()];
        let filtered = super::filter_supported(&args, &supported);
        assert!(filtered.contains(&"--fullscreen".to_string()));
        assert!(filtered.iter().any(|a| a.starts_with("--network-caching")));
        assert!(!filtered.iter().any(|a| a.starts_with("--doesnotexist")));
        assert!(!filtered.contains(&"--another".to_string()));
    }
}
