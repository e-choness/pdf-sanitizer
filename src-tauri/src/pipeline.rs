use pdfsan_core::{sanitize, verify_pdf, SanitizationSettings, SanitizeError, Stage};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

pub struct FileTask {
    pub id: String,
    pub path: String,
}

/// Pre-flight check: verify backup folder is writable.
pub fn preflight_backup_folder(folder: &str) -> Result<(), SanitizeError> {
    if folder.is_empty() {
        return Err(SanitizeError::BackupFolderUnwritable(
            "no folder configured".to_string(),
        ));
    }
    let dir = Path::new(folder);
    fs::create_dir_all(dir)
        .map_err(|e| SanitizeError::BackupFolderUnwritable(format!("cannot create: {e}")))?;
    // Probe write
    let probe = dir.join(".pdfsan-probe");
    fs::write(&probe, b"")
        .map_err(|e| SanitizeError::BackupFolderUnwritable(format!("not writable: {e}")))?;
    let _ = fs::remove_file(&probe);
    Ok(())
}

/// Move original to backup folder, handle name collisions.
fn move_to_backup(original: &Path, backup_folder: &str) -> Result<PathBuf, String> {
    let folder = Path::new(backup_folder);
    let stem = original
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = original
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("pdf");

    let mut dest = folder.join(format!("{stem}.{ext}"));
    let mut n = 2u32;
    while dest.exists() {
        dest = folder.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }

    // Try rename first; fall back to copy+delete for cross-device
    if fs::rename(original, &dest).is_err() {
        fs::copy(original, &dest).map_err(|e| format!("copy failed: {e}"))?;
        // fsync the copy
        {
            let f = fs::File::open(&dest).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
        }
        fs::remove_file(original).map_err(|e| format!("delete original failed: {e}"))?;
    }

    Ok(dest)
}

pub async fn run_batch(
    files: Vec<FileTask>,
    settings: SanitizationSettings,
    app: AppHandle,
    tokens: std::sync::Arc<tokio::sync::Mutex<HashMap<String, CancellationToken>>>,
    batch_token: CancellationToken,
) -> serde_json::Value {
    let settings = std::sync::Arc::new(settings);
    let max = settings.max_concurrent.clamp(1, 8) as usize;
    let semaphore = std::sync::Arc::new(Semaphore::new(max));

    let mut handles = vec![];
    let mut ok = 0u32;
    let mut failed = 0u32;
    let mut cancelled = 0u32;
    let mut bytes_saved = 0i64;

    for task in files {
        let id = task.id.clone();
        let file_token = batch_token.child_token();

        {
            let mut map = tokens.lock().await;
            map.insert(id.clone(), file_token.clone());
        }

        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let settings = settings.clone();
        let app = app.clone();
        let tokens = tokens.clone();
        let id2 = id.clone();

        let handle = tokio::task::spawn(async move {
            let _permit = permit;

            // Check cancelled before starting
            if file_token.is_cancelled() {
                let _ = app.emit("file_cancelled", serde_json::json!({ "id": id2 }));
                let mut map = tokens.lock().await;
                map.remove(&id2);
                return (false, true, 0i64);
            }

            emit_progress(&app, &id2, "loading", 0, None);

            let path = PathBuf::from(&task.path);
            let token_clone = file_token.clone();
            let settings_clone = settings.clone();
            let app_clone = app.clone();
            let id_clone = id2.clone();

            let result = tokio::task::spawn_blocking(move || {
                sanitize(
                    &path,
                    &settings_clone,
                    &token_clone,
                    &mut |stage| match &stage {
                        Stage::Rewriting { pct } => {
                            emit_progress(&app_clone, &id_clone, "rewriting", *pct, None);
                        }
                        Stage::Optimizing { pct } => {
                            emit_progress(&app_clone, &id_clone, "optimizing", *pct, None);
                        }
                        Stage::Saving => {
                            emit_progress(&app_clone, &id_clone, "saving", 99, None);
                        }
                        _ => {}
                    },
                )
            })
            .await;

            let (temp_path, report) = match result {
                Ok(Ok(r)) => r,
                Ok(Err(SanitizeError::Cancelled)) => {
                    let _ = app.emit("file_cancelled", serde_json::json!({ "id": id2 }));
                    let mut map = tokens.lock().await;
                    map.remove(&id2);
                    return (false, true, 0i64);
                }
                Ok(Err(e)) => {
                    let _ = app.emit(
                        "file_error",
                        serde_json::json!({ "id": id2, "error": e.to_string() }),
                    );
                    let mut map = tokens.lock().await;
                    map.remove(&id2);
                    return (false, false, 0i64);
                }
                Err(e) => {
                    let _ = app.emit(
                        "file_error",
                        serde_json::json!({ "id": id2, "error": format!("internal: {e}") }),
                    );
                    let mut map = tokens.lock().await;
                    map.remove(&id2);
                    return (false, false, 0i64);
                }
            };

            // Verification
            emit_progress(&app, &id2, "verifying", 100, Some(report.pages));
            let ver_result = tokio::task::spawn_blocking({
                let tp = temp_path.clone();
                let pages = report.pages;
                let s = settings.clone();
                move || verify_pdf(&tp, pages, &s)
            })
            .await;

            // Never replace the original unless verification positively passed.
            let ver_result = match ver_result {
                Ok(r) => r,
                Err(e) => Err(format!("verifier crashed: {e}")),
            };
            if let Err(msg) = ver_result {
                let _ = fs::remove_file(&temp_path);
                let _ = app.emit(
                    "file_error",
                    serde_json::json!({
                        "id": id2,
                        "error": SanitizeError::VerificationFailed(msg).to_string()
                    }),
                );
                let mut map = tokens.lock().await;
                map.remove(&id2);
                return (false, false, 0i64);
            }

            // Atomic replace
            emit_progress(&app, &id2, "replacing", 100, None);
            let orig = PathBuf::from(&task.path);
            let backup_folder = settings.output_folder.clone();

            let move_result = tokio::task::spawn_blocking(move || {
                let backup_path = match move_to_backup(&orig, &backup_folder) {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = fs::remove_file(&temp_path);
                        return Err(format!("backup failed: {e}"));
                    }
                };
                // Move temp → original path
                if let Err(e) = fs::rename(&temp_path, &orig) {
                    let _ = fs::remove_file(&temp_path);
                    // Restore the original; the backup folder may be on another
                    // volume, so fall back to copy when rename is not possible.
                    let restored = fs::rename(&backup_path, &orig).is_ok()
                        || fs::copy(&backup_path, &orig).is_ok();
                    return Err(if restored {
                        format!("swap failed: {e}. Original restored.")
                    } else {
                        format!("swap failed: {e}. Original is in {}", backup_path.display())
                    });
                }
                Ok(backup_path)
            })
            .await;

            match move_result {
                Ok(Ok(backup_path)) => {
                    let saved = report.input_size as i64 - report.output_size as i64;
                    let _ = app.emit(
                        "file_complete",
                        serde_json::json!({
                            "id": id2,
                            "input_size": report.input_size,
                            "output_size": report.output_size,
                            "backup_path": backup_path.to_string_lossy(),
                            "report": report,
                        }),
                    );
                    let mut map = tokens.lock().await;
                    map.remove(&id2);
                    (true, false, saved)
                }
                Ok(Err(e)) => {
                    let msg = e;
                    let _ = app.emit("file_error", serde_json::json!({ "id": id2, "error": msg }));
                    let mut map = tokens.lock().await;
                    map.remove(&id2);
                    (false, false, 0i64)
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = app.emit("file_error", serde_json::json!({ "id": id2, "error": msg }));
                    let mut map = tokens.lock().await;
                    map.remove(&id2);
                    (false, false, 0i64)
                }
            }
        });

        handles.push(handle);
    }

    for h in handles {
        match h.await {
            Ok((did_ok, did_cancel, saved)) => {
                if did_ok {
                    ok += 1;
                    bytes_saved += saved;
                } else if did_cancel {
                    cancelled += 1;
                } else {
                    failed += 1;
                }
            }
            Err(_) => {
                failed += 1;
            }
        }
    }

    serde_json::json!({
        "ok": ok,
        "failed": failed,
        "cancelled": cancelled,
        "bytes_saved": bytes_saved,
    })
}

fn emit_progress(app: &AppHandle, id: &str, stage: &str, pct: u8, pages: Option<u32>) {
    let mut payload = serde_json::json!({ "id": id, "stage": stage, "pct": pct });
    if let Some(p) = pages {
        payload["pages"] = serde_json::json!(p);
    }
    let _ = app.emit("file_progress", payload);
}
