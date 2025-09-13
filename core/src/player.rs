use crate::CoreConfig;
use urlencoding::encode;

pub fn build_url_by_type(cfg: &CoreConfig, id: &str, info: &str, ext: Option<&str>) -> String {
    let (path, ext) = match info {
        "SeriesEpisode" => ("series", ext.unwrap_or("mp4")),
        "Movie" | "VOD" => ("movie", ext.unwrap_or("mp4")),
        _ => ("live", ext.unwrap_or("m3u8")),
    };
    format!(
        "{}/{}//{}//{}//{}.{}",
        cfg.address,
        path,
        encode(&cfg.username),
        encode(&cfg.password),
        encode(id),
        ext
    )
}
