use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_null_string")]
    pub body: String,
    #[serde(default)]
    pub assets: Vec<GitHubAsset>,
}

fn deserialize_null_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNull {
        String(String),
        Null,
    }
    
    match StringOrNull::deserialize(deserializer)? {
        StringOrNull::String(s) => Ok(s),
        StringOrNull::Null => Ok(String::new()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAsset {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub browser_download_url: String,
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub update_available: bool,
    pub release_notes: String,
    pub download_url: Option<String>,
}

/// Compare two semantic version strings (e.g., "v0.1.6" vs "v0.1.7")
pub fn compare_versions(current: &str, latest: &str) -> Ordering {
    let clean_current = current.trim_start_matches('v');
    let clean_latest = latest.trim_start_matches('v');
    
    let current_parts: Vec<u32> = clean_current.split('.').filter_map(|s| s.parse().ok()).collect();
    let latest_parts: Vec<u32> = clean_latest.split('.').filter_map(|s| s.parse().ok()).collect();
    
    for (c, l) in current_parts.iter().zip(latest_parts.iter()) {
        match c.cmp(l) {
            Ordering::Less => return Ordering::Less,
            Ordering::Greater => return Ordering::Greater,
            Ordering::Equal => continue,
        }
    }
    
    current_parts.len().cmp(&latest_parts.len())
}

/// Check for updates from GitHub releases
pub async fn check_for_updates(current_version: &str) -> Result<UpdateInfo, String> {
    let url = "https://api.github.com/repos/modze996/macxtreamer/releases/latest";
    
    let client = reqwest::Client::builder()
        .user_agent("macXtreamer")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;
        
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
        
    if !response.status().is_success() {
        return Err(format!("GitHub API error: {}", response.status()));
    }
    
    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to get response text: {}", e))?;
    
    println!("📄 GitHub API response length: {} bytes", text.len());
    
    let release: GitHubRelease = serde_json::from_str(&text)
        .map_err(|e| {
            println!("❌ JSON parse error details: {}", e);
            println!("📄 Response preview: {}", &text[..std::cmp::min(500, text.len())]);
            format!("JSON parse error: {}", e)
        })?;
    
    let update_available = compare_versions(current_version, &release.tag_name) == Ordering::Less;
    
    // Find macOS app bundle asset
    let download_url = release.assets
        .iter()
        .find(|asset| asset.name.ends_with(".dmg") || asset.name.contains("macOS") || asset.name.contains("darwin"))
        .map(|asset| asset.browser_download_url.clone());
    
    Ok(UpdateInfo {
        latest_version: release.tag_name,
        update_available,
        release_notes: release.body,
        download_url,
    })
}

/// Download DMG and install update automatically (macOS).
/// `progress_tx` receives human-readable status strings (optional).
pub async fn download_and_install_update(
    download_url: &str,
    version: &str,
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
) -> Result<String, String> {
    use futures_util::StreamExt;
    use std::io::Write;

    macro_rules! progress {
        ($msg:expr) => {{
            println!("{}", $msg);
            if let Some(ref tx) = progress_tx {
                let _ = tx.send($msg.to_string());
            }
        }};
        ($fmt:expr, $($arg:tt)*) => {{
            let s = format!($fmt, $($arg)*);
            println!("{}", s);
            if let Some(ref tx) = progress_tx {
                let _ = tx.send(s);
            }
        }};
    }
    
    progress!("📥 Downloading update from: {}", download_url);

    // Create temp directory for download
    let temp_dir = std::env::temp_dir();
    let dmg_filename = format!("macxtreamer_{}.dmg", version);
    let dmg_path = temp_dir.join(&dmg_filename);
    
    // Download DMG file
    let client = reqwest::Client::builder()
        .user_agent("macXtreamer-Updater")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;
    
    let response = client
        .get(download_url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }
    
    let total_size = response.content_length().unwrap_or(0);
    progress!("📦 Download size: {} MB", total_size / 1_048_576.max(1));
    
    // Create file and download with progress
    let mut file = std::fs::File::create(&dmg_path)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Write error: {}", e))?;
        downloaded += chunk.len() as u64;
        
        if total_size > 0 {
            let progress_pct = (downloaded as f64 / total_size as f64 * 100.0) as u32;
            if progress_pct % 10 == 0 && downloaded % (2 * 1_048_576) < chunk.len() as u64 {
                progress!("📥 Downloading... {}%", progress_pct);
            }
        }
    }
    
    drop(file);
    progress!("✅ Download complete");

    // Mount DMG
    progress!("💿 Mounting DMG...");
    let mount_output = std::process::Command::new("hdiutil")
        .args(&["attach", "-nobrowse", "-quiet"])
        .arg(&dmg_path)
        .output()
        .map_err(|e| format!("Failed to mount DMG: {}", e))?;
    
    if !mount_output.status.success() {
        return Err(format!("DMG mount failed: {}", String::from_utf8_lossy(&mount_output.stderr)));
    }
    
    // Parse mount point from output
    let mount_info = String::from_utf8_lossy(&mount_output.stdout);
    let mount_point = mount_info
        .lines()
        .last()
        .and_then(|line| line.split('\t').last())
        .ok_or("Failed to parse mount point")?
        .trim();
    
    progress!("💿 Mounted at: {}", mount_point);
    
    // Find .app bundle in mounted volume
    let mount_path = std::path::Path::new(mount_point);
    let app_entries = std::fs::read_dir(mount_path)
        .map_err(|e| format!("Failed to read mount directory: {}", e))?;
    
    let app_bundle = app_entries
        .filter_map(|e| e.ok())
        .find(|entry| {
            entry.path().extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "app")
                .unwrap_or(false)
        })
        .ok_or("No .app bundle found in DMG")?;
    
    let source_app = app_bundle.path();
    progress!("📦 Found app: {}", source_app.display());
    
    // Install to /Applications
    let dest_app = std::path::Path::new("/Applications/macxtreamer.app");
    
    // Remove old version if exists
    if dest_app.exists() {
        progress!("🗑️  Removing old version...");
        std::fs::remove_dir_all(dest_app)
            .map_err(|e| format!("Failed to remove old version: {}", e))?;
    }
    
    // Copy new version
    progress!("📋 Installing new version...");
    let copy_status = std::process::Command::new("cp")
        .args(&["-R"])
        .arg(&source_app)
        .arg(dest_app)
        .status()
        .map_err(|e| format!("Failed to copy app: {}", e))?;
    
    if !copy_status.success() {
        return Err("Failed to install app".to_string());
    }
    
    // Unmount DMG
    progress!("💿 Unmounting DMG...");
    let _ = std::process::Command::new("hdiutil")
        .args(&["detach", "-quiet"])
        .arg(mount_point)
        .status();

    // Clean up DMG file
    let _ = std::fs::remove_file(&dmg_path);

    progress!("✅ Installation complete!");
    Ok("Update installed successfully. Restarting...".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_version_comparison() {
        assert_eq!(compare_versions("v0.1.5", "v0.1.6"), Ordering::Less);
        assert_eq!(compare_versions("v0.1.6", "v0.1.6"), Ordering::Equal);
        assert_eq!(compare_versions("v0.1.7", "v0.1.6"), Ordering::Greater);
        assert_eq!(compare_versions("v0.2.0", "v0.1.9"), Ordering::Greater);
    }
}