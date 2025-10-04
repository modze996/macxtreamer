use std::io;
use std::process::Command;
use crate::models::Config;
use crate::logger::{log_line, log_command, log_error};

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

/// Get optimized VLC command for different streaming types
/// 
/// Key IPTV/Xtream Codes optimizations & error fixes:
/// - network-caching: Buffer for network streams (ms)
/// - live-caching: Additional buffer for live streams 
/// - audio-resampler=soxr: High-quality audio resampling (fixes audio errors)
/// - aout=pulse,alsa,oss: Multiple audio output fallbacks (fixes "no audio output")
/// - clock-master=audio: Use audio clock as master (fixes sync issues)
/// - avcodec-*: Error resilience and bug workarounds for problematic streams
/// - pts-offset=0: Reset timestamp offset (fixes PCR timing errors)
/// - ts-es-id-pid: Better MPEG-TS stream handling
/// - audio-desync=0: Disable audio desync compensation
/// - network-synchronisation: Better sync for network streams
/// - drop-late-frames/skip-frames: Handle network delays gracefully
/// - rtsp-tcp: Force TCP for better reliability
/// - http-reconnect: Auto-reconnect on connection drops
/// - adaptive-logic=rate: Adaptive bitrate based on connection
/// - hls-segment-threads: Parallel HLS segment loading
/// - prefetch-buffer-size: Pre-buffer data amount
pub fn get_optimized_vlc_command(stream_type: &str) -> &'static str {
    match stream_type {
        "live" | "channel" => {
            // Optimized for live TV/IPTV streams - increase buffering to improve stability on flaky networks
            // Significantly raise network-caching and live-caching (values in ms) and increase prefetch buffer (bytes)
            "vlc --fullscreen --no-video-title-show --network-caching=20000 --live-caching=10000 --audio-resampler=soxr --audio-time-stretch --force-dolby-surround=0 --aout=pulse,alsa,oss --audio-desync=0 --clock-master=audio --clock-jitter=0 --network-synchronisation --avcodec-skiploopfilter=4 --avcodec-skip-frame=0 --avcodec-skip-idct=0 --sout-x264-preset=ultrafast --drop-late-frames --skip-frames --intf=dummy --no-video-title --no-snapshot-preview --no-stats --no-osd --rtsp-tcp --http-reconnect --adaptive-logic=rate --hls-segment-threads=6 --prefetch-buffer-size=8388608 --demux-filter=record --ts-es-id-pid --ts-seek-percent --pts-offset=0 {URL}"
        },
        "vod" | "movie" => {
            // Optimized for VOD with audio/video sync fixes
            "vlc --fullscreen --no-video-title-show --network-caching=8000 --file-caching=5000 --audio-resampler=soxr --audio-time-stretch --force-dolby-surround=0 --aout=pulse,alsa,oss --audio-desync=0 --clock-master=audio --sout-mux-caching=3000 --sout-udp-caching=3000 --cr-average=2000 --avcodec-skiploopfilter=0 --avcodec-skip-frame=0 --avcodec-skip-idct=0 --intf=dummy --no-video-title --no-snapshot-preview --no-stats --rtsp-tcp --http-reconnect --adaptive-logic=rate --hls-segment-threads=4 --prefetch-buffer-size=8388608 --pts-offset=0 {URL}"
        },
        "series" => {
            // Balanced settings for series episodes with error handling
            "vlc --fullscreen --no-video-title-show --network-caching=6000 --file-caching=4000 --audio-resampler=soxr --audio-time-stretch --force-dolby-surround=0 --aout=pulse,alsa,oss --audio-desync=0 --clock-master=audio --sout-mux-caching=2500 --cr-average=1500 --avcodec-skiploopfilter=2 --avcodec-skip-frame=0 --avcodec-skip-idct=0 --intf=dummy --no-video-title --no-snapshot-preview --no-stats --rtsp-tcp --http-reconnect --adaptive-logic=rate --hls-segment-threads=4 --prefetch-buffer-size=4194304 --pts-offset=0 {URL}"
        },
        "errorfix" => {
            // Maximum compatibility for problematic streams with all error mitigation
            "vlc --fullscreen --no-video-title-show --network-caching=10000 --live-caching=5000 --file-caching=8000 --audio-resampler=soxr --audio-time-stretch --force-dolby-surround=0 --aout=pulse,alsa,oss,dummy --audio-desync=0 --clock-master=input --input-slave= --audio-track-id=-1 --sub-track-id=-1 --video-track-id=-1 --program=-1 --audio-language= --sub-language= --avcodec-skiploopfilter=4 --avcodec-skip-frame=0 --avcodec-skip-idct=0 --avcodec-hurry-up=0 --avcodec-error-resilience=1 --avcodec-workaround-bugs=1 --sout-x264-preset=ultrafast --drop-late-frames --skip-frames --intf=dummy --no-video-title --no-snapshot-preview --no-stats --no-osd --rtsp-tcp --http-reconnect --adaptive-logic=rate --hls-segment-threads=2 --prefetch-buffer-size=16777216 --demux-filter=record --ts-es-id-pid --ts-seek-percent --pts-offset=0 --clock-jitter=5000 --input-repeat=999 --start-time=0 {URL}"
        },
        _ => {
            // Default optimized for IPTV/Xtream Codes streaming with comprehensive error fixes
            "vlc --fullscreen --no-video-title-show --network-caching=5000 --live-caching=3000 --audio-resampler=soxr --audio-time-stretch --force-dolby-surround=0 --aout=pulse,alsa,oss --audio-desync=0 --clock-master=audio --clock-jitter=0 --network-synchronisation --sout-mux-caching=2000 --file-caching=2000 --sout-udp-caching=2000 --cr-average=1000 --avcodec-skiploopfilter=4 --avcodec-skip-frame=0 --avcodec-skip-idct=0 --sout-x264-preset=ultrafast --drop-late-frames --skip-frames --intf=dummy --no-video-title --no-snapshot-preview --no-stats --no-osd --rtsp-tcp --http-reconnect --adaptive-logic=rate --hls-segment-threads=4 --prefetch-buffer-size=4194304 --demux-filter=record --ts-es-id-pid --ts-seek-percent --pts-offset=0 {URL}"
        }
    }
}

/// Detect stream type from URL patterns
pub fn detect_stream_type(stream_url: &str) -> &'static str {
    if stream_url.contains("/live/") {
        "live"
    } else if stream_url.contains("/movie/") {
        "vod"
    } else if stream_url.contains("/series/") {
        "series"
    } else if stream_url.ends_with(".m3u8") || stream_url.contains("playlist.m3u8") {
        "live" // HLS streams are usually live
    } else if stream_url.ends_with(".mp4") || stream_url.ends_with(".mkv") || stream_url.ends_with(".avi") {
        "vod" // Video files are usually VOD
    } else {
        "default"
    }
}

pub fn start_player(cfg: &Config, stream_url: &str) -> io::Result<()> {
    // Auto-detect stream type and use appropriate VLC parameters, or user's custom command
    let stream_type = detect_stream_type(stream_url);
    // If user provided a custom player command, prefer it. Otherwise build a VLC command using
    // the buffer values from config so the user can tune caching without editing code.
    let cmd = if !cfg.player_command.trim().is_empty() {
        cfg.player_command.trim().to_string()
    } else {
        // Build a command based on stream type, plugging in config values for buffering
        let network_caching = cfg.vlc_network_caching_ms;
        let live_caching = cfg.vlc_live_caching_ms;
        let prefetch_bytes = cfg.vlc_prefetch_buffer_bytes;
        match stream_type {
            "live" | "channel" => format!(
                "vlc --fullscreen --no-video-title-show --network-caching={} --live-caching={} --file-caching=8000 --audio-resampler=soxr --audio-time-stretch --force-dolby-surround=0 --aout=pulse,alsa,oss --audio-desync=0 --clock-master=audio --clock-jitter=2000 --network-synchronisation --avcodec-skiploopfilter=4 --avcodec-skip-frame=0 --avcodec-skip-idct=0 --avcodec-error-resilience=1 --avcodec-workaround-bugs=1 --sout-x264-preset=ultrafast --drop-late-frames --skip-frames --intf=dummy --no-video-title --no-snapshot-preview --no-stats --no-osd --rtsp-tcp --http-reconnect --adaptive-logic=rate --hls-segment-threads=8 --prefetch-buffer-size={} --demux-filter=record --ts-es-id-pid --ts-seek-percent --pts-offset=0 --input-repeat=999 --start-time=0 --sout-mux-caching=5000 --sout-udp-caching=5000 {{URL}}",
                network_caching, live_caching, prefetch_bytes
            ),
            "vod" | "movie" => format!(
                "vlc --fullscreen --no-video-title-show --network-caching={} --file-caching=5000 --audio-resampler=soxr --audio-time-stretch --force-dolby-surround=0 --aout=pulse,alsa,oss --audio-desync=0 --clock-master=audio --sout-mux-caching=3000 --sout-udp-caching=3000 --cr-average=2000 --avcodec-skiploopfilter=0 --avcodec-skip-frame=0 --avcodec-skip-idct=0 --intf=dummy --no-video-title --no-snapshot-preview --no-stats --rtsp-tcp --http-reconnect --adaptive-logic=rate --hls-segment-threads=4 --prefetch-buffer-size={} --pts-offset=0 {{URL}}",
                network_caching, prefetch_bytes
            ),
            "series" => format!(
                "vlc --fullscreen --no-video-title-show --network-caching={} --file-caching=4000 --audio-resampler=soxr --audio-time-stretch --force-dolby-surround=0 --aout=pulse,alsa,oss --audio-desync=0 --clock-master=audio --sout-mux-caching=2500 --cr-average=1500 --avcodec-skiploopfilter=2 --avcodec-skip-frame=0 --avcodec-skip-idct=0 --intf=dummy --no-video-title --no-snapshot-preview --no-stats --rtsp-tcp --http-reconnect --adaptive-logic=rate --hls-segment-threads=4 --prefetch-buffer-size={} --pts-offset=0 {{URL}}",
                network_caching, prefetch_bytes
            ),
            _ => format!(
                "vlc --fullscreen --no-video-title-show --network-caching={} --live-caching=3000 --audio-resampler=soxr --audio-time-stretch --force-dolby-surround=0 --aout=pulse,alsa,oss --audio-desync=0 --clock-master=audio --clock-jitter=0 --network-synchronisation --sout-mux-caching=2000 --file-caching=2000 --sout-udp-caching=2000 --cr-average=1000 --avcodec-skiploopfilter=4 --avcodec-skip-frame=0 --avcodec-skip-idct=0 --sout-x264-preset=ultrafast --drop-late-frames --skip-frames --intf=dummy --no-video-title --no-snapshot-preview --no-stats --no-osd --rtsp-tcp --http-reconnect --adaptive-logic=rate --hls-segment-threads=4 --prefetch-buffer-size={} --demux-filter=record --ts-es-id-pid --ts-seek-percent --pts-offset=0 {{URL}}",
                network_caching, prefetch_bytes
            ),
        }
    };
    let mut parts: Vec<String> = cmd.split_whitespace().map(|s| s.to_string()).collect();
    let mut replaced = false;
    for p in &mut parts {
        if p == "URL" || p == "{URL}" || p == "{url}" { *p = stream_url.to_string(); replaced = true; }
    }
    if !replaced { parts.push(stream_url.to_string()); }
    if parts.is_empty() { return Ok(()); }
    let program = parts.remove(0);

    // If using VLC on macOS, reuse existing instance when possible
    let using_vlc = program.to_lowercase().contains("vlc") || cmd.to_lowercase().contains("vlc");
    #[cfg(target_os = "macos")]
    {
        if using_vlc && cfg.reuse_vlc && is_vlc_running() {
            log_line("Reusing running VLC via 'open -a VLC' with VLC parameters");
            log_line(&format!("URL={}", stream_url));
            log_line(&format!("VLC args: {:?}", parts));
            
            // Use 'open' with --args to pass VLC parameters to the running instance
            let mut open_cmd = Command::new("open");
            open_cmd.arg("-a").arg("VLC");
            if !parts.is_empty() {
                open_cmd.arg("--args").args(&parts);
            } else {
                open_cmd.arg(stream_url);
            }
            
            if let Ok(child) = open_cmd.spawn() {
                log_line(&format!("Spawned 'open' with args pid={}", child.id()));
            }
            return Ok(());
        }
        
        // If reuse is disabled or no VLC running, we may still want to use 'open' for better macOS integration
        if using_vlc {
            log_line("Starting new VLC instance via 'open -a VLC' with full parameters");
            let mut open_cmd = Command::new("open");
            if !cfg.reuse_vlc {
                open_cmd.arg("-n"); // force new instance if reuse is disabled
            }
            open_cmd.arg("-a").arg("VLC");
            if !parts.is_empty() {
                open_cmd.arg("--args").args(&parts);
            } else {
                open_cmd.arg(stream_url);
            }
            
            match open_cmd.spawn() {
                Ok(child) => {
                    log_line(&format!("Spawned new VLC via 'open' pid={}", child.id()));
                    return Ok(());
                }
                Err(e) => {
                    log_line(&format!("Failed to spawn VLC via 'open': {}, falling back to direct spawn", e));
                    // Fall through to direct spawn method below
                }
            }
        }
    }

    // Log and spawn the command; try to capture basic status
    log_command(&program, &parts);
    match Command::new(&program).args(&parts).spawn() {
        Ok(child) => {
            log_line(&format!("Spawned player pid={} program={} args={:?}", child.id(), program, parts));
        }
        Err(e) => {
            log_error("Failed to spawn player", &e);
            return Err(e);
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_type_detection() {
        assert_eq!(detect_stream_type("http://server:8080/live/user/pass/12345.m3u8"), "live");
        assert_eq!(detect_stream_type("http://server:8080/movie/user/pass/12345.mp4"), "vod");
        assert_eq!(detect_stream_type("http://server:8080/series/user/pass/12345.mkv"), "series");
        assert_eq!(detect_stream_type("http://example.com/stream.m3u8"), "live");
        assert_eq!(detect_stream_type("http://example.com/video.mp4"), "vod");
        assert_eq!(detect_stream_type("http://example.com/unknown"), "default");
    }

    #[test]
    fn test_optimized_commands_contain_key_params() {
        let live_cmd = get_optimized_vlc_command("live");
        let vod_cmd = get_optimized_vlc_command("vod");
        
        // All commands should have these IPTV-optimized parameters
        for cmd in &[live_cmd, vod_cmd] {
            assert!(cmd.contains("--network-caching"));
            assert!(cmd.contains("--rtsp-tcp"));
            assert!(cmd.contains("--http-reconnect"));
            assert!(cmd.contains("--adaptive-logic=rate"));
        }
        
    // Live streams should have increased caching for stability on flaky networks
    assert!(live_cmd.contains("--network-caching=20000"));
    assert!(live_cmd.contains("--live-caching=10000"));
        
        // VOD should have larger buffer
        assert!(vod_cmd.contains("--network-caching=8000"));
        
        // All commands should have audio error fixes
        assert!(live_cmd.contains("--audio-resampler=soxr"));
        assert!(vod_cmd.contains("--aout=pulse,alsa,oss"));
        assert!(live_cmd.contains("--clock-master=audio"));
        assert!(vod_cmd.contains("--pts-offset=0"));
    }
}
