use std::io;
use std::process::Command;
use crate::models::Config;

pub fn build_stream_url(cfg: &Config, stream_id: &str) -> String {
    format!("{}/live/{}/{}/{}.ts", cfg.address, cfg.username, cfg.password, stream_id)
}
pub fn build_vod_stream_url(cfg: &Config, stream_id: &str, ext: &str) -> String {
    let ext = ext.trim_start_matches('.');
    format!("{}/movie/{}/{}/{}.{}", cfg.address, cfg.username, cfg.password, stream_id, ext)
}
pub fn build_series_episode_stream_url(cfg: &Config, episode_id: &str, ext: &str) -> String {
    let ext = ext.trim_start_matches('.');
    format!("{}/series/{}/{}/{}.{}", cfg.address, cfg.username, cfg.password, episode_id, ext)
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
    let default_cmd = "vlc --fullscreen --no-video-title-show --network-caching=2000 URL";
    let cmd = if cfg.player_command.trim().is_empty() { default_cmd } else { &cfg.player_command };
    let mut parts: Vec<String> = cmd.split_whitespace().map(|s| s.to_string()).collect();
    let mut replaced = false;
    for p in &mut parts {
        if p == "URL" || p == "{URL}" || p == "{url}" { *p = stream_url.to_string(); replaced = true; }
    }
    if !replaced { parts.push(stream_url.to_string()); }
    if parts.is_empty() { return Ok(()); }
    let program = parts.remove(0);
    let _ = Command::new(program).args(parts).spawn()?;
    Ok(())
}
