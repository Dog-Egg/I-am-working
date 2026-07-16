use std::path::Path;

use chrono::Local;

use super::models::{AppSettings, TrayTimeFormat};

pub(crate) fn default_settings() -> AppSettings {
    AppSettings {
        show_tray_time: true,
        tray_time_format: TrayTimeFormat::HhMm,
        launch_at_login: true,
        nosleep_enabled: false,
    }
}

#[cfg(debug_assertions)]
fn log_settings_file(action: &str, path: &Path) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    eprintln!("[settings {timestamp}] {action} path={}", path.display());
}

#[cfg(not(debug_assertions))]
fn log_settings_file(_action: &str, _path: &Path) {}

pub(crate) fn load_settings(path: &Path) -> Result<AppSettings, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(default_settings());
    }

    log_settings_file("read", path);
    let contents = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

pub(crate) fn persist_settings(
    path: &Path,
    settings: &AppSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    log_settings_file("write", path);
    std::fs::write(path, serde_json::to_vec_pretty(settings)?)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_settings_uses_json_file_values() {
        let path = std::env::temp_dir().join(format!(
            "i-am-working-test-settings-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let settings = AppSettings {
            show_tray_time: false,
            tray_time_format: TrayTimeFormat::HhMmSs,
            launch_at_login: true,
            nosleep_enabled: true,
        };

        persist_settings(&path, &settings).unwrap();

        assert_eq!(load_settings(&path).unwrap(), settings);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_settings_defaults_nosleep_to_off() {
        let path = std::env::temp_dir().join(format!(
            "i-am-working-test-legacy-settings-{}.json",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"{"show_tray_time":false,"tray_time_format":"HH:MM","launch_at_login":true}"#,
        )
        .unwrap();

        assert!(!load_settings(&path).unwrap().nosleep_enabled);
        let _ = std::fs::remove_file(path);
    }
}
