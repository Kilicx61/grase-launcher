mod models;
pub use models::{VersionDto, VersionManifest};

use crate::error::{AppError, AppResult};

const MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

pub async fn fetch_manifest() -> AppResult<VersionManifest> {
    let cache_file = crate::launcher::game_dir().join("version_manifest.json");

    // Try to read cache first if it's less than 24 hours old
    let mut use_cache = false;
    if cache_file.exists() {
        if let Ok(metadata) = std::fs::metadata(&cache_file) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    if elapsed.as_secs() < 86400 {
                        use_cache = true;
                    }
                }
            }
        }
    }

    if use_cache {
        if let Ok(content) = std::fs::read_to_string(&cache_file) {
            if let Ok(manifest) = serde_json::from_str::<VersionManifest>(&content) {
                return Ok(manifest);
            }
        }
    }

    // Fetch from Mojang API
    match reqwest::get(MANIFEST_URL).await {
        Ok(response) => {
            if let Ok(manifest) = response.json::<VersionManifest>().await {
                // Save to cache
                if let Ok(content) = serde_json::to_string(&manifest) {
                    let _ = std::fs::create_dir_all(crate::launcher::game_dir());
                    let _ = std::fs::write(&cache_file, content);
                }
                return Ok(manifest);
            }
        }
        Err(e) => {
            // Offline or network error: fall back to cache even if old
            if cache_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&cache_file) {
                    if let Ok(manifest) = serde_json::from_str::<VersionManifest>(&content) {
                        return Ok(manifest);
                    }
                }
            }
            return Err(AppError::Network(e));
        }
    }

    // Fallback cache read in case of API parsing errors
    if cache_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&cache_file) {
            if let Ok(manifest) = serde_json::from_str::<VersionManifest>(&content) {
                return Ok(manifest);
            }
        }
    }

    Err(AppError::ChecksumMismatch("Surum listesi yuklenemedi".to_string()))
}

pub fn to_dto_list(manifest: &VersionManifest) -> Vec<VersionDto> {
    manifest
        .versions
        .iter()
        .filter(|v| v.version_type == "release")
        .map(|v| VersionDto {
            id: v.id.clone(),
            version_type: v.version_type.clone(),
            is_latest: v.id == manifest.latest.release,
        })
        .collect()
}