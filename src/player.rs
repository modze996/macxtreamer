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

// --- Neu hinzugefügt: fehlende Hilfsfunktionen ---
pub fn detect_stream_type(url: &str) -> StreamType {
    let u = url.to_lowercase();
    if u.contains("/live/") || u.ends_with(".m3u8") { return StreamType::Live; }
    // Serien vor generischen VOD-Erweiterungen erkennen, damit /series/...mkv nicht fälschlich als Vod klassifiziert wird
    if u.contains("/series/") { return StreamType::Series; }
    if u.contains("/movie/") || u.ends_with(".mp4") || u.ends_with(".mkv") || u.ends_with(".avi") { return StreamType::Vod; }
    StreamType::Default
}

pub fn apply_bias(cfg: &Config) -> (u32,u32,u32) {
    let bias = (cfg.vlc_profile_bias.min(100) as f32)/100.0;
    const NET_LOWER: u32 = 2000; const LIVE_LOWER: u32 = 1500; const FILE_LOWER: u32 = 1000;
    const LIVE_FALLBACK_UPPER: u32 = 6000; const FILE_FALLBACK_UPPER: u32 = 5000;
    let net_upper = cfg.vlc_network_caching_ms; // wird später ggf. gecappt
    let live_upper = cfg.vlc_live_caching_ms.max(LIVE_FALLBACK_UPPER);
    let file_upper = cfg.vlc_file_caching_ms.max(FILE_FALLBACK_UPPER);
    let lerp = |lower: u32, upper: u32| -> u32 { (lower as f32 + (upper.saturating_sub(lower)) as f32 * bias).round() as u32 };
    (lerp(NET_LOWER, net_upper), lerp(LIVE_LOWER, live_upper), lerp(FILE_LOWER, file_upper))
}

fn filter_supported(args: &[String], supported: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    'A: for a in args { for f in supported { if a == f || a.starts_with(&format!("{}=", f)) { out.push(a.clone()); continue 'A; } } }
    out
}

pub fn get_vlc_command_for_stream_type(st: StreamType, cfg:&Config) -> String {
    let mut args = build_vlc_args(cfg, st);
    if !args.iter().any(|a| a.contains("{URL}")) { args.push("{URL}".into()); }
    format!("vlc {}", args.join(" "))
}

fn probe_vlc_supported_flags() -> Vec<String> {
    let mut base = vec!["--fullscreen".into(), "--network-caching".into(), "--live-caching".into(), "--file-caching".into(), "--http-reconnect".into()];
    if let Ok(out) = Command::new("vlc").arg("-H").stdout(Stdio::piped()).stderr(Stdio::null()).output() {
        if let Ok(s) = String::from_utf8(out.stdout) {
            for line in s.lines() { let l=line.trim(); if l.starts_with("--") { let flag=l.split_whitespace().next().unwrap_or("").to_string(); if !base.iter().any(|x| x==&flag) { base.push(flag); } } }
        }
    }
    base
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
    // Entfernt: instabile Flags (--mux-caching / --http-timeout)
    if !cfg.vlc_extra_args.trim().is_empty() { for part in cfg.vlc_extra_args.split_whitespace() { args.push(part.to_string()); } }
    args
}

/// Startet einen Player für die gegebene URL. Bevorzugt mpv (falls konfiguriert), sonst VLC.
/// Bei Live-Streams mit aktiviertem Auto-Retry wird mpv bei frühem EOF erneut gestartet.
pub fn start_player(cfg: &Config, url: &str) -> Result<(), String> {
    let st = detect_stream_type(url);
    if cfg.use_mpv {
        // mpv Argumente vorbereiten und dann in Hintergrund-Thread starten, um UI nicht zu blockieren.
        let (net_ms, live_ms, _file_ms) = apply_bias(cfg);
        let cache_secs = if cfg.mpv_cache_secs_override != 0 { cfg.mpv_cache_secs_override } else { (net_ms / 1000).max(1) };
        let readahead_secs = if cfg.mpv_readahead_secs_override != 0 { cfg.mpv_readahead_secs_override } else { (live_ms / 1000).max(1) };
        let mut base_args: Vec<String> = vec!["--fullscreen".into(), "--no-terminal".into(), "--force-window=yes".into(), "--video-paused=no".into()];
        // Moderne mpv Cache Optionen – fallback falls nicht unterstützt:
        base_args.push(format!("--cache-secs={}", cache_secs));
        base_args.push(format!("--demuxer-readahead-secs={}", readahead_secs));
        base_args.push("--cache=yes".into());
        if cfg.mpv_keep_open { base_args.push("--keep-open=yes".into()); }
        if matches!(st, StreamType::Live) { base_args.push("--idle=yes".into()); }
        // Reconnect Optionen nur hinzufügen, wenn mpv sie kennt (prüfen später via list-options)
        base_args.push("--reconnect-on-eof=yes".into());
        base_args.push("--demuxer-lavf-o=reconnect_streamed=1".into());
        if cfg.mpv_verbose { base_args.push("-v".into()); }
        if !cfg.mpv_extra_args.trim().is_empty() { for part in cfg.mpv_extra_args.split_whitespace() { base_args.push(part.to_string()); } }
        base_args.push(url.to_string());
        log_command("mpv", &base_args);
        let cfg_clone = cfg.clone();
        let url_string = url.to_string();
        std::thread::spawn(move || {
            // Optional: Filter nicht unterstützte Optionen anhand mpv --list-options
            let supported = probe_mpv_supported_options();
            let final_args = filter_mpv_supported(&base_args, &supported);
            let mut base_args = if final_args.is_empty() { base_args.clone() } else { final_args };
            // Sicherstellen dass die URL ganz am Ende bleibt (falls Filter Reihenfolge geändert hat)
            // Sicherstellen dass URL letztes Argument ist (ohne Borrow-Konflikte)
            let orig_url_opt = base_args.iter()
                .find(|s| !s.starts_with("--") && (s.starts_with("http://") || s.starts_with("https://")))
                .cloned();
            if let Some(orig_url) = orig_url_opt {
                if let Some(last) = base_args.last() {
                    let needs_move = last.starts_with("--") || last != &orig_url;
                    if needs_move {
                        // Entferne alle Vorkommen der URL und füge sie hinten an
                        base_args.retain(|s| s != &orig_url);
                        base_args.push(orig_url);
                    }
                }
            }
            if !base_args.iter().any(|s| s.starts_with("http://") || s.starts_with("https://")) {
                log_line("Warnung: mpv Argumentliste enthält keine URL – Abbruch und VLC Fallback");
                let st_fb = detect_stream_type(&url_string);
                let args_fb = build_vlc_args(&cfg_clone, st_fb);
                let supported_fb = probe_vlc_supported_flags();
                let filtered_fb = filter_supported(&args_fb, &supported_fb);
                let mut final_fb = filtered_fb;
                final_fb.push(url_string.clone());
                log_command("vlc", &final_fb);
                let _ = Command::new("vlc").args(&final_fb).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
                return;
            }
            let attempt_live_retry = cfg_clone.mpv_live_auto_retry && matches!(st, StreamType::Live);
            let max_attempts = cfg_clone.mpv_live_retry_max.max(1);
            let delay_ms = cfg_clone.mpv_live_retry_delay_ms.max(500);
            let global_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5*60);
            for attempt in 0..max_attempts {
                if attempt > 0 { log_line(&format!("mpv Live-Retry Versuch {}/{}", attempt+1, max_attempts)); }
                let start = std::time::Instant::now();
                match Command::new("mpv").args(&base_args).stdout(Stdio::null()).stderr(Stdio::piped()).spawn() {
                    Ok(mut child) => {
                        if cfg_clone.mpv_verbose {
                            if let Some(mut stderr) = child.stderr.take() {
                                std::thread::spawn(move || {
                                    use std::io::Read; let mut buf = String::new(); let _ = stderr.read_to_string(&mut buf);
                                    let truncated = if buf.len() > 16000 { format!("{}...<truncated>", &buf[..16000]) } else { buf };
                                    log_line(&format!("mpv stderr: {}", truncated.replace('\n', " | ")));
                                });
                            }
                        }
                        match child.wait() {
                            Ok(status) => {
                                let elapsed = start.elapsed();
                                if attempt_live_retry && !status.success() && elapsed < std::time::Duration::from_secs(25) && std::time::Instant::now() < global_deadline && attempt+1 < max_attempts {
                                    log_line("mpv beendete sich sehr früh (<25s) – erneuter Versuch in Kürze...");
                                    std::thread::sleep(std::time::Duration::from_millis(delay_ms as u64));
                                    continue; // neuer Versuch
                                }
                                if !status.success() {
                                    if let Some(tx) = crate::GLOBAL_TX.get().cloned() {
                                        let _ = tx.send(crate::app_state::Msg::PlayerSpawnFailed { player: "mpv".into(), error: format!("Exit status: {:?}", status.code()) });
                                    }
                                }
                                // Fertig (erfolgreich oder nicht) -> Ende
                                return;
                            }
                            Err(e) => {
                                log_error("Fehler beim Warten auf mpv", &e);
                                if let Some(tx) = crate::GLOBAL_TX.get().cloned() { let _ = tx.send(crate::app_state::Msg::PlayerSpawnFailed { player: "mpv".into(), error: e.to_string() }); }
                                break; // Fallback zu VLC
                            }
                        }
                    }
                    Err(e) => {
                        log_error("mpv Start fehlgeschlagen, versuche VLC Fallback", &e);
                        if let Some(tx) = crate::GLOBAL_TX.get().cloned() { let _ = tx.send(crate::app_state::Msg::PlayerSpawnFailed { player: "mpv".into(), error: e.to_string() }); }
                        break; // Fallback zu VLC
                    }
                }
            }
            // Fallback VLC
            log_line("Fallback zu VLC (mpv fehlgeschlagen oder früh beendet)...");
            let st_fb = detect_stream_type(&url_string);
            let args_fb = build_vlc_args(&cfg_clone, st_fb);
            let supported_fb = probe_vlc_supported_flags();
            let filtered_fb = filter_supported(&args_fb, &supported_fb);
            let mut final_fb = filtered_fb;
            final_fb.push(url_string.clone());
            log_command("vlc", &final_fb);
            let _ = Command::new("vlc").args(&final_fb).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        });
        return Ok(()); // Sofort zurück – UI bleibt responsiv
    }

    // VLC Pfad
    let args = build_vlc_args(cfg, st);
    let supported = probe_vlc_supported_flags();
    let filtered = filter_supported(&args, &supported);
    let mut final_args = filtered;
    final_args.push(url.to_string());
    log_command("vlc", &final_args);

    #[cfg(target_os = "macos")]
    if cfg.reuse_vlc && is_vlc_running() {
        if send_url_to_vlc(url) {
            log_line("URL an laufende VLC Instanz gesendet (Reuse-Modus)");
            return Ok(());
        } else {
            log_line("Senden an laufende VLC Instanz fehlgeschlagen, starte neue Instanz...");
        }
    }

    match Command::new("vlc").args(&final_args).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(_) => {
            if cfg.vlc_diagnose_on_start { spawn_vlc_diagnostics(url, cfg); }
            // Continuous Diagnostics nur starten wenn explizit aktiviert und Live
            if cfg.vlc_continuous_diagnostics && matches!(st, StreamType::Live) {
                if let Some(tx) = crate::GLOBAL_TX.get().cloned() {
                    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                    spawn_vlc_continuous_diagnostics(tx, url.to_string(), cfg.clone(), stop);
                }
            }
            Ok(())
        }
        Err(e) => Err(format!("VLC konnte nicht gestartet werden: {}", e))
    }
}

fn probe_mpv_supported_options() -> Vec<String> {
    if let Ok(out) = Command::new("mpv").arg("--list-options").stdout(Stdio::piped()).stderr(Stdio::null()).output() {
        if let Ok(s) = String::from_utf8(out.stdout) {
            return s.lines().filter_map(|l| l.split_whitespace().next()).map(|w| w.trim().to_string()).filter(|w| w.starts_with("--")).collect();
        }
    }
    Vec::new()
}

fn filter_mpv_supported(args: &[String], supported: &[String]) -> Vec<String> {
    if supported.is_empty() { return args.to_vec(); }
    let mut out = Vec::new();
    'A: for a in args {
        // Behalte reine URL (kein führendes -- ) immer bei
        if !a.starts_with("--") { out.push(a.clone()); continue 'A; }
        for f in supported {
            if a == f || a.starts_with(&format!("{}=", f)) {
                out.push(a.clone());
                continue 'A;
            }
        }
    }
    out
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
