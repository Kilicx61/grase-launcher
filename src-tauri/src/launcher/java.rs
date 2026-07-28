use crate::error::{AppError, AppResult};
use crate::launcher::{download_file_no_checksum, runtimes_dir};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    link: String,
}

fn find_javaw_recursive(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(found) = find_javaw_recursive(&path) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some("javaw.exe") {
            return Some(path);
        }
    }

    None
}

/// Verilen klasor icinde (alt klasorler dahil) "javaw.exe" dosyasini arar.
/// Bulursa tam yolunu dondurur.
fn find_javaw(dir: &Path) -> Option<PathBuf> {
    // 1. Yol: dir/bin/javaw.exe
    let path1 = dir.join("bin").join("javaw.exe");
    if path1.exists() {
        return Some(path1);
    }

    // 2. Yol: dir/*/bin/javaw.exe (Adoptium JRE arsivinden cikan alt klasor)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let path2 = path.join("bin").join("javaw.exe");
                if path2.exists() {
                    return Some(path2);
                }
            }
        }
    }

    // 3. Yol: Fallback olarak tum dizinleri dolas
    find_javaw_recursive(dir)
}

/// Bundled Java'nin zaten kurulu olup olmadigini kontrol eder.
/// Kuruluysa javaw.exe'nin tam yolunu dondurur.
pub fn find_bundled_java(major_version: u32) -> Option<PathBuf> {
    let dir = runtimes_dir(major_version);
    if !dir.exists() {
        return None;
    }
    find_javaw(&dir)
}

/// Adoptium API'sinden verilen major surum icin Windows x64 JRE indirme
/// linkini ceker.
async fn fetch_download_link(major_version: u32) -> AppResult<String> {
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{major_version}/hotspot?architecture=x64&image_type=jre&os=windows"
    );

    let response = reqwest::get(&url).await.map_err(AppError::Network)?;
    let assets: Vec<AdoptiumAsset> = response.json().await.map_err(AppError::Network)?;

    let first = assets.into_iter().next().ok_or_else(|| {
        AppError::ChecksumMismatch(format!(
            "Adoptium'da Java {major_version} icin Windows x64 JRE bulunamadi"
        ))
    })?;

    Ok(first.binary.package.link)
}

/// Verilen major surum icin bundled Java'yi indirir, cikarir ve
/// javaw.exe'nin tam yolunu dondurur. Zaten kuruluysa tekrar indirmez.
pub async fn ensure_java_runtime(major_version: u32) -> AppResult<PathBuf> {
    if let Some(existing) = find_bundled_java(major_version) {
        return Ok(existing);
    }

    let dir = runtimes_dir(major_version);
    tokio::fs::create_dir_all(&dir).await.map_err(AppError::Io)?;

    let link = fetch_download_link(major_version).await?;

    let zip_path = dir.join("runtime.zip");
    download_file_no_checksum(&link, &zip_path).await?;

    // Zip acma islemi blocking oldugu icin ayri bir thread'de calistiriyoruz.
    let dir_clone = dir.clone();
    let zip_path_clone = zip_path.clone();
    tokio::task::spawn_blocking(move || extract_zip(&zip_path_clone, &dir_clone))
        .await
        .map_err(|e| AppError::ChecksumMismatch(e.to_string()))??;

    tokio::fs::remove_file(&zip_path).await.ok();

    find_bundled_java(major_version).ok_or_else(|| {
        AppError::ChecksumMismatch(
            "Java indirildi ama javaw.exe bulunamadi, arsiv yapisi beklenenden farkli olabilir"
                .to_string(),
        )
    })
}

fn extract_zip(zip_path: &Path, dest_dir: &Path) -> AppResult<()> {
    let file = std::fs::File::open(zip_path).map_err(AppError::Io)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::ChecksumMismatch(format!("Zip acilamadi: {e}")))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::ChecksumMismatch(format!("Zip girdisi okunamadi: {e}")))?;

        let Some(relative_path) = entry.enclosed_name() else {
            continue;
        };

        let out_path = dest_dir.join(relative_path);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(AppError::Io)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(AppError::Io)?;
            }
            let mut out_file = std::fs::File::create(&out_path).map_err(AppError::Io)?;
            std::io::copy(&mut entry, &mut out_file).map_err(AppError::Io)?;
        }
    }

    Ok(())
}