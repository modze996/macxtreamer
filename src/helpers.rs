use std::path::Path;

/// Sanitize filename to remove invalid characters
#[allow(dead_code)]
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Convert a file path to a file:// URI for player
pub fn file_path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    if s.starts_with('/') {
        format!("file://{}", s)
    } else {
        format!("file:///{}", s)
    }
}

/// Format file size in human-readable format
pub fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format speed in human-readable format
#[allow(dead_code)]
pub fn format_speed(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes_per_sec >= GB {
        format!("{:.2} GB/s", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.2} MB/s", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.2} KB/s", bytes_per_sec / KB)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

/// Format duration in human-readable format
#[allow(dead_code)]
pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Extract language code from item name (e.g., "EN - Movie" -> "EN", "DE - Film" -> "DE")
pub fn extract_language_from_name(name: &str) -> Option<String> {
    // Common patterns: "EN - ", "DE - ", "MULTI - ", "4K - ", etc.
    // Look for 2-5 uppercase letters followed by " - "
    let parts: Vec<&str> = name.splitn(2, " - ").collect();
    if parts.len() == 2 {
        let prefix = parts[0].trim();
        // Check if it looks like a language code (2-5 uppercase letters/numbers)
        if prefix.len() >= 2 && prefix.len() <= 5 && prefix.chars().all(|c| c.is_uppercase() || c.is_numeric()) {
            return Some(prefix.to_string());
        }
    }
    None
}
