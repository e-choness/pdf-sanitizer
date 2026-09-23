use pdfsan_core::SanitizationSettings;
use std::fs;
use std::path::PathBuf;

const SETTINGS_FILE: &str = "settings.json";

pub fn save_settings(settings: &SanitizationSettings) -> Result<(), String> {
    let path = get_settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_settings() -> SanitizationSettings {
    let path = get_settings_path();
    if !path.exists() {
        return SanitizationSettings::default();
    }
    match fs::read_to_string(&path) {
        Err(_) => SanitizationSettings::default(),
        Ok(json) => match serde_json::from_str::<SanitizationSettings>(&json) {
            Ok(s) => {
                // Clamp max_concurrent
                SanitizationSettings {
                    max_concurrent: s.max_concurrent.clamp(1, 8),
                    ..s
                }
            }
            Err(_) => {
                // Rename corrupt file and use defaults
                let bak = path.with_extension("json.bak");
                let _ = fs::rename(&path, &bak);
                log::warn!("settings.json was corrupt; renamed to .bak and reset to defaults");
                SanitizationSettings::default()
            }
        },
    }
}

fn get_settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pdf-sanitizer")
        .join(SETTINGS_FILE)
}
