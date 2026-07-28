use crate::error::{AppError, AppResult};
use crate::launcher::{download_file_no_checksum, libraries_dir, FabricLoaderListEntry, FabricProfile};
use futures_util::{stream, StreamExt};
use std::path::PathBuf;

const FABRIC_META_BASE: &str = "https://meta.fabricmc.net/v2/versions/loader";
const MAX_CONCURRENT_DOWNLOADS: usize = 16;

/// Verilen Minecraft surumuyle uyumlu Fabric loader surumlerini ceker.
/// En yeni/stabil surumler listenin basinda gelir (Fabric API'sinin dogal sirasi).
pub async fn fetch_fabric_loader_versions(game_version: &str) -> AppResult<Vec<FabricLoaderListEntry>> {
    let url = format!("{FABRIC_META_BASE}/{game_version}");
    let response = reqwest::get(&url).await.map_err(AppError::Network)?;
    let entries: Vec<FabricLoaderListEntry> = response.json().await.map_err(AppError::Network)?;
    Ok(entries)
}

/// Verilen oyun ve loader surumu icin Fabric profilini (mainClass + kutuphaneler) ceker.
pub async fn fetch_fabric_profile(game_version: &str, loader_version: &str) -> AppResult<FabricProfile> {
    let cache_dir = libraries_dir().join("fabric-profiles");
    let cache_file = cache_dir.join(format!("{game_version}-{loader_version}.json"));

    if cache_file.exists() {
        if let Ok(content) = tokio::fs::read_to_string(&cache_file).await {
            if let Ok(profile) = serde_json::from_str::<FabricProfile>(&content) {
                return Ok(profile);
            }
        }
    }

    let url = format!("{FABRIC_META_BASE}/{game_version}/{loader_version}/profile/json");
    let response = reqwest::get(&url).await.map_err(AppError::Network)?;
    let profile: FabricProfile = response.json().await.map_err(AppError::Network)?;

    // Cache the profile JSON locally
    if let Ok(_) = tokio::fs::create_dir_all(&cache_dir).await {
        if let Ok(content) = serde_json::to_string(&profile) {
            let _ = tokio::fs::write(&cache_file, content).await;
        }
    }

    Ok(profile)
}

/// "group:artifact:version" (opsiyonel ":classifier" ile) seklindeki Maven
/// koordinatini, dosya sistemindeki goreli yola cevirir.
/// Ornek: "net.fabricmc:fabric-loader:0.15.7" ->
///        "net/fabricmc/fabric-loader/0.15.7/fabric-loader-0.15.7.jar"
fn maven_coordinate_to_path(coord: &str) -> Option<String> {
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 {
        return None;
    }

    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts.get(3);

    let filename = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };

    Some(format!("{group}/{artifact}/{version}/{filename}"))
}

/// Fabric profilindeki tum kutuphaneleri indirir (checksum Fabric API'sinde
/// verilmedigi icin dogrulama yapilmaz, sadece indirilir). Basarili indirilen
/// dosyalarin tam yollarini dondurur - bunlar classpath'e eklenecek.
pub async fn download_fabric_libraries(profile: &FabricProfile) -> AppResult<Vec<PathBuf>> {
    let tasks: Vec<(String, PathBuf)> = profile
        .libraries
        .iter()
        .filter_map(|lib| {
            let relative_path = maven_coordinate_to_path(&lib.name)?;
            let base_url = lib
                .url
                .as_deref()
                .unwrap_or("https://maven.fabricmc.net/")
                .trim_end_matches('/');
            let url = format!("{base_url}/{relative_path}");
            let dest = libraries_dir().join(&relative_path);
            Some((url, dest))
        })
        .collect();

    let results: Vec<AppResult<PathBuf>> = stream::iter(tasks)
        .map(|(url, dest)| async move {
            if !dest.exists() {
                download_file_no_checksum(&url, &dest).await?;
            }
            Ok(dest)
        })
        .buffer_unordered(MAX_CONCURRENT_DOWNLOADS)
        .collect()
        .await;

    let mut paths = Vec::new();
    for result in results {
        paths.push(result?);
    }

    Ok(paths)
}