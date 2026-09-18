#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod pipeline;
mod settings;

use pdfsan_core::SanitizationSettings;
use pipeline::{preflight_backup_folder, run_batch, FileTask};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub struct AppState {
    settings: Arc<tokio::sync::RwLock<SanitizationSettings>>,
    /// Per-file cancellation tokens, keyed by file id.
    tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// Batch-level token; replaced on each new batch.
    batch_token: Arc<Mutex<Option<CancellationToken>>>,
    batch_running: Arc<std::sync::atomic::AtomicBool>,
}

#[tauri::command]
async fn load_settings(state: State<'_, AppState>) -> Result<SanitizationSettings, String> {
    Ok(state.settings.read().await.clone())
}

#[tauri::command]
async fn save_settings(
    new_settings: SanitizationSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let clamped = SanitizationSettings {
        max_concurrent: new_settings.max_concurrent.clamp(1, 8),
        ..new_settings
    };
    *state.settings.write().await = clamped.clone();
    settings::save_settings(&clamped)
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct StatResult {
    pub path: String,
    pub size: u64,
}

#[tauri::command]
async fn stat_files(paths: Vec<String>) -> Result<Vec<StatResult>, String> {
    let results = paths
        .into_iter()
        .map(|p| {
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            StatResult { path: p, size }
        })
        .collect();
    Ok(results)
}

#[tauri::command]
async fn process_files(
    files: Vec<serde_json::Value>,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    if state
        .batch_running
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("A batch is already running".to_string());
    }

    let settings = state.settings.read().await.clone();

    // Pre-flight: backup folder
    if let Err(e) = preflight_backup_folder(&settings.output_folder) {
        let _ = app_handle.emit("batch_error", serde_json::json!({ "error": e.to_string() }));
        return Err(e.to_string());
    }

    // Parse file tasks
    let tasks: Vec<FileTask> = files
        .into_iter()
        .filter_map(|v| {
            let id = v.get("id")?.as_str()?.to_string();
            let path = v.get("path")?.as_str()?.to_string();
            // Per-file pre-flight: exists, readable, .pdf extension, %PDF header
            let p = std::path::Path::new(&path);
            if !p.exists() || p.extension().and_then(|e| e.to_str()) != Some("pdf") {
                let _ = app_handle.emit(
                    "file_error",
                    serde_json::json!({ "id": id, "error": "File not found or not a PDF." }),
                );
                return None;
            }
            Some(FileTask { id, path })
        })
        .collect();

    if tasks.is_empty() {
        return Ok(());
    }

    let batch_token = CancellationToken::new();
    *state.batch_token.lock().await = Some(batch_token.clone());
    state
        .batch_running
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let tokens = state.tokens.clone();
    let running_flag = state.batch_running.clone();
    let app = app_handle.clone();

    tokio::spawn(async move {
        run_batch(tasks, settings, app, tokens, batch_token).await;
        running_flag.store(false, std::sync::atomic::Ordering::SeqCst);
    });

    Ok(())
}

#[tauri::command]
async fn cancel_file(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let map = state.tokens.lock().await;
    if let Some(token) = map.get(&id) {
        token.cancel();
    }
    Ok(())
}

#[tauri::command]
async fn cancel_all(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(token) = state.batch_token.lock().await.as_ref() {
        token.cancel();
    }
    Ok(())
}

fn main() {
    let initial_settings = settings::load_settings();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .manage(AppState {
            settings: Arc::new(tokio::sync::RwLock::new(initial_settings)),
            tokens: Arc::new(Mutex::new(HashMap::new())),
            batch_token: Arc::new(Mutex::new(None)),
            batch_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            stat_files,
            process_files,
            cancel_file,
            cancel_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
