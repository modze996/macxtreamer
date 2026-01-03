use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub published_at: String,
    pub prerelease: bool,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
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
    
    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;
    
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

/// Download and install update (opens download URL in browser for now)
pub fn install_update(download_url: &str) -> Result<(), String> {
    // For now, open download URL in browser
    // In future versions, could implement automatic download and installation
    if let Err(e) = webbrowser::open(download_url) {
        Err(format!("Failed to open download URL: {}", e))
    } else {
        Ok(())
    }
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