use crate::auth::{self, OfflineProfile};
use crate::config::{self, AppConfig};
use crate::launcher::{self, LaunchParams};
use crate::manifest::{self, VersionDto};

#[tauri::command]
pub async fn get_minecraft_versions() -> Result<Vec<VersionDto>, String> {
    let manifest = manifest::fetch_manifest()
        .await
        .map_err(|e| e.to_string())?;

    Ok(manifest::to_dto_list(&manifest))
}

#[tauri::command]
pub async fn login_offline(username: String) -> Result<OfflineProfile, String> {
    let profile = auth::validate_and_create_profile(&username).map_err(|e| e.to_string())?;

    config::upsert_profile(&profile.username, &profile.uuid)
        .await
        .map_err(|e| e.to_string())?;

    Ok(profile)
}

#[tauri::command]
pub async fn prepare_version(version_id: String) -> Result<launcher::VersionDetail, String> {
    let version_dir = launcher::versions_dir().join(&version_id);
    let cache_file = version_dir.join(format!("{version_id}-details.json"));

    if cache_file.exists() {
        if let Ok(content) = tokio::fs::read_to_string(&cache_file).await {
            if let Ok(detail) = serde_json::from_str::<launcher::VersionDetail>(&content) {
                return Ok(detail);
            }
        }
    }

    let manifest = manifest::fetch_manifest()
        .await
        .map_err(|e| e.to_string())?;

    let version_entry = manifest
        .versions
        .iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| format!("Surum bulunamadi: {version_id}"))?;

    let detail = launcher::fetch_version_details(&version_entry.url)
        .await
        .map_err(|e| e.to_string())?;

    // Cache the version detail locally
    if let Ok(_) = tokio::fs::create_dir_all(&version_dir).await {
        if let Ok(content) = serde_json::to_string(&detail) {
            let _ = tokio::fs::write(&cache_file, content).await;
        }
    }

    Ok(detail)
}

#[tauri::command]
pub async fn download_client_jar(detail: launcher::VersionDetail) -> Result<String, String> {
    let path = launcher::download_client(&detail)
        .await
        .map_err(|e| e.to_string())?;

    Ok(path.display().to_string())
}

#[tauri::command]
pub async fn download_libraries(app: tauri::AppHandle, detail: launcher::VersionDetail) -> Result<usize, String> {
    use serde::Serialize;
    use tauri::Emitter;

    #[derive(Clone, Serialize)]
    struct ProgressPayload {
        stage: String,
        completed: usize,
        total: usize,
    }

    let emitter = std::sync::Arc::new(move |completed: usize, total: usize| {
        let _ = app.emit(
            "download-progress",
            ProgressPayload {
                stage: "libraries".to_string(),
                completed,
                total,
            },
        );
    });

    launcher::download_libraries(&detail, emitter)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_assets(app: tauri::AppHandle, detail: launcher::VersionDetail) -> Result<usize, String> {
    use serde::Serialize;
    use tauri::Emitter;

    #[derive(Clone, Serialize)]
    struct ProgressPayload {
        stage: String,
        completed: usize,
        total: usize,
    }

    let emitter = std::sync::Arc::new(move |completed: usize, total: usize| {
        let _ = app.emit(
            "download-progress",
            ProgressPayload {
                stage: "assets".to_string(),
                completed,
                total,
            },
        );
    });

    launcher::download_assets(&detail, emitter)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ensure_java(major_version: u32) -> Result<String, String> {
    let path = launcher::ensure_java_runtime(major_version)
        .await
        .map_err(|e| e.to_string())?;

    Ok(path.display().to_string())
}

#[tauri::command]
pub async fn get_fabric_loader_versions(game_version: String) -> Result<Vec<String>, String> {
    let entries = launcher::fetch_fabric_loader_versions(&game_version)
        .await
        .map_err(|e| e.to_string())?;

    Ok(entries.into_iter().map(|e| e.loader.version).collect())
}

#[tauri::command]
pub async fn prepare_fabric(
    game_version: String,
    loader_version: String,
) -> Result<launcher::FabricProfile, String> {
    launcher::fetch_fabric_profile(&game_version, &loader_version)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_fabric_libraries(profile: launcher::FabricProfile) -> Result<Vec<String>, String> {
    let paths = launcher::download_fabric_libraries(&profile)
        .await
        .map_err(|e| e.to_string())?;

    Ok(paths.iter().map(|p| p.display().to_string()).collect())
}

fn parse_extra_args(raw: &str) -> Vec<String> {
    raw.split_whitespace()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

async fn monitor_game_process(
    mut child: tokio::process::Child,
    app: tauri::AppHandle,
    version_id: String,
    start_time: std::time::Instant,
) {
    use serde::Serialize;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use tauri::Emitter;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::sync::Mutex;

    const MAX_LOG_LINES: usize = 300;

    let log_buffer: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES)));

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let buf_out = log_buffer.clone();
    let stdout_task = tokio::spawn(async move {
        if let Some(out) = stdout {
            let mut lines = BufReader::new(out).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                println!("{line}");
                let mut buf = buf_out.lock().await;
                if buf.len() >= MAX_LOG_LINES {
                    buf.pop_front();
                }
                buf.push_back(line);
            }
        }
    });

    let buf_err = log_buffer.clone();
    let stderr_task = tokio::spawn(async move {
        if let Some(err) = stderr {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("{line}");
                let mut buf = buf_err.lock().await;
                if buf.len() >= MAX_LOG_LINES {
                    buf.pop_front();
                }
                buf.push_back(line);
            }
        }
    });

    let status = child.wait().await;
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    let lines: Vec<String> = log_buffer.lock().await.iter().cloned().collect();

    #[derive(Clone, Serialize)]
    struct CrashPayload {
        reason: String,
        fix_action: Option<String>,
        crash_report_path: Option<String>,
    }

    match status {
        Ok(exit_status) if exit_status.success() => {
            let elapsed_seconds = start_time.elapsed().as_secs();
            let _ = config::add_playtime(&version_id, elapsed_seconds).await;
            let _ = app.emit("game-closed", ());
        }
        _ => {
            let elapsed_seconds = start_time.elapsed().as_secs();
            let _ = config::add_playtime(&version_id, elapsed_seconds).await;
            let diagnosis = launcher::diagnose_crash(&lines);
            let crash_report_path = launcher::latest_crash_report_path().map(|p| p.display().to_string());
            let _ = app.emit(
                "game-crashed",
                CrashPayload {
                    reason: diagnosis.reason,
                    fix_action: diagnosis.fix_action,
                    crash_report_path,
                },
            );
        }
    }
}

#[tauri::command]
pub async fn launch_game(
    app: tauri::AppHandle,
    detail: launcher::VersionDetail,
    instance_id: String,
    username: String,
    uuid: String,
    min_ram_mb: Option<u32>,
    max_ram_mb: Option<u32>,
    extra_jvm_args: Option<String>,
    java_path: Option<String>,
    main_class_override: Option<String>,
    extra_classpath: Option<Vec<String>>,
) -> Result<(), String> {
    let detail_clone = detail.clone();
    let natives_path = tokio::task::spawn_blocking(move || launcher::extract_natives(&detail_clone))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    let parsed_extra_args = extra_jvm_args
        .as_deref()
        .map(parse_extra_args)
        .unwrap_or_default();

    let params = LaunchParams {
        username,
        uuid,
        min_ram_mb,
        max_ram_mb,
        extra_jvm_args: parsed_extra_args,
        java_path: java_path.map(std::path::PathBuf::from),
        main_class_override,
        extra_classpath: extra_classpath
            .unwrap_or_default()
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect(),
        instance_dir: launcher::instance_dir(&instance_id),
    };

    let child = launcher::launch_game(&detail, &natives_path, params)
        .await
        .map_err(|e| e.to_string())?;

    let launch_start_time = std::time::Instant::now();
    tokio::spawn(monitor_game_process(child, app, instance_id, launch_start_time));

    Ok(())
}

#[tauri::command]
pub async fn load_config() -> AppConfig {
    config::load_config().await
}

#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<(), String> {
    config::save_config(&config).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_profile(username: String) -> Result<AppConfig, String> {
    config::remove_profile(&username).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ping_minecraft_server(address: String) -> Result<launcher::ServerStatus, String> {
    launcher::ping_server(&address).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_server(name: String, address: String) -> Result<AppConfig, String> {
    config::add_server(&name, &address).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_server(address: String) -> Result<AppConfig, String> {
    config::remove_server(&address).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_mods(instance_id: String) -> Result<Vec<launcher::ModEntry>, String> {
    launcher::list_mods(&instance_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_mod(instance_id: String, file_name: String) -> Result<(), String> {
    launcher::toggle_mod(&instance_id, &file_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_mod(instance_id: String, file_name: String) -> Result<(), String> {
    launcher::delete_mod(&instance_id, &file_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_mod_from_path(instance_id: String, source_path: String) -> Result<(), String> {
    launcher::add_mod_from_path(&instance_id, &source_path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_system_memory_mb() -> u64 {
    launcher::get_total_memory_mb()
}

#[tauri::command]
pub async fn search_modrinth_mods(
    query: String,
    game_version: String,
    loader: String,
) -> Result<Vec<launcher::ModrinthSearchHit>, String> {
    launcher::search_modrinth_mods(&query, &game_version, &loader)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_modrinth_mod(
    instance_id: String,
    project_id: String,
    game_version: String,
    loader: String,
) -> Result<Vec<String>, String> {
    launcher::install_mod_with_dependencies(&instance_id, &project_id, &game_version, &loader)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn is_version_installed(version_id: String) -> bool {
    launcher::install_marker_path(&version_id).exists()
}

#[tauri::command]
pub async fn mark_version_installed(version_id: String) -> Result<(), String> {
    let path = launcher::install_marker_path(&version_id);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::write(&path, b"ok").await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn apply_crash_fix(instance_id: String, action: String) -> Result<String, String> {
    match action.as_str() {
        "lower_ram" => {
            let new_ram = config::lower_max_ram().await.map_err(|e| e.to_string())?;
            Ok(format!("Maksimum RAM {new_ram} MB olarak ayarlandi"))
        }
        "disable_last_mod" => {
            let disabled = launcher::disable_most_recent_mod(&instance_id)
                .await
                .map_err(|e| e.to_string())?;

            match disabled {
                Some(name) => Ok(format!("'{name}' modu devre disi birakildi")),
                None => Err("Devre disi birakilacak aktif mod bulunamadi".to_string()),
            }
        }
        _ => Err("Bilinmeyen duzeltme eylemi".to_string()),
    }
}

#[tauri::command]
pub async fn create_backup(instance_id: String, label: String) -> Result<Option<launcher::BackupEntry>, String> {
    launcher::create_backup(&instance_id, &label).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_backups(instance_id: String) -> Result<Vec<launcher::BackupEntry>, String> {
    launcher::list_backups(&instance_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_backup(instance_id: String, id: String) -> Result<(), String> {
    launcher::restore_backup(&instance_id, &id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_backup(instance_id: String, id: String) -> Result<(), String> {
    launcher::delete_backup(&instance_id, &id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_mod_updates(instance_id: String, game_version: String, loader: String) -> Result<Vec<launcher::ModUpdateInfo>, String> {
    launcher::check_mod_updates(&instance_id, &game_version, &loader)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn apply_mod_update(instance_id: String, old_file_name: String, download_url: String, new_file_name: String) -> Result<(), String> {
    launcher::apply_mod_update(&instance_id, &old_file_name, &download_url, &new_file_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_resource_packs(instance_id: String) -> Result<Vec<launcher::PackEntry>, String> {
    launcher::list_packs(&launcher::instance_resourcepacks_dir(&instance_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_resource_pack(instance_id: String, file_name: String) -> Result<(), String> {
    launcher::delete_pack(&launcher::instance_resourcepacks_dir(&instance_id), &file_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_resource_pack_from_path(instance_id: String, source_path: String) -> Result<(), String> {
    launcher::add_pack_from_path(&launcher::instance_resourcepacks_dir(&instance_id), &source_path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_shader_packs(instance_id: String) -> Result<Vec<launcher::PackEntry>, String> {
    launcher::list_packs(&launcher::instance_shaderpacks_dir(&instance_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_shader_pack(instance_id: String, file_name: String) -> Result<(), String> {
    launcher::delete_pack(&launcher::instance_shaderpacks_dir(&instance_id), &file_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_shader_pack_from_path(instance_id: String, source_path: String) -> Result<(), String> {
    launcher::add_pack_from_path(&launcher::instance_shaderpacks_dir(&instance_id), &source_path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_resource_packs(query: String, game_version: String) -> Result<Vec<launcher::ModrinthSearchHit>, String> {
    launcher::search_projects(&query, &game_version, "resourcepack").await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_resource_pack(instance_id: String, project_id: String, game_version: String) -> Result<String, String> {
    launcher::install_project_file(&project_id, &game_version, &launcher::instance_resourcepacks_dir(&instance_id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_shader_packs(query: String, game_version: String) -> Result<Vec<launcher::ModrinthSearchHit>, String> {
    launcher::search_projects(&query, &game_version, "shader").await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_shader_pack(instance_id: String, project_id: String, game_version: String) -> Result<String, String> {
    launcher::install_project_file(&project_id, &game_version, &launcher::instance_shaderpacks_dir(&instance_id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_worlds(instance_id: String) -> Result<Vec<launcher::WorldEntry>, String> {
    launcher::list_worlds(&instance_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_world(instance_id: String, folder_name: String) -> Result<(), String> {
    launcher::delete_world(&instance_id, &folder_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_world(instance_id: String, old_name: String, new_name: String) -> Result<(), String> {
    launcher::rename_world(&instance_id, &old_name, &new_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_playtime_stats() -> std::collections::HashMap<String, u64> {
    let config = config::load_config().await;
    config.playtime_seconds_by_version
}

/// Eski (paylasilan) mod/dunya/paket klasorlerini, verilen instance'a
/// tek seferlik olarak tasir. Zaten tasinmissa veya eski veri yoksa
/// hicbir sey yapmaz, false doner.
#[tauri::command]
pub async fn migrate_legacy_data(instance_id: String) -> Result<bool, String> {
    launcher::migrate_legacy_data_to_instance(&instance_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_game_dir() -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(launcher::game_dir())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}