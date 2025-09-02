use std::io;
use std::process::Command;
use crate::models::Config;

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

pub fn start_player(cfg: &Config, stream_url: &str) -> io::Result<()> {
    // Platzhalter "URL" wird ersetzt, sonst am Ende angehängt. Empty => VLC-Defaults.
    let default_cmd = "vlc --fullscreen --no-video-title-show --network-caching=2000 {URL}";
    let cmd = if cfg.player_command.trim().is_empty() { default_cmd } else { &cfg.player_command };
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
            // Reuse existing VLC instance by asking macOS to open the URL in VLC
            // This avoids spawning a new VLC process each time.
            let _ = Command::new("open").arg("-a").arg("VLC").arg(stream_url).spawn()?;
            return Ok(());
        }
    }

    let _ = Command::new(program).args(parts).spawn()?;
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
