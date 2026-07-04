use std::sync::{Arc, Mutex};

use tauri::{AppHandle, State};

use crate::app_state::{build_stats, AppState};
use crate::desktop::{refresh_launch_at_login, sync_launch_at_login, update_tray_title};
use crate::models::{AppSettings, HourlyWorkRecord, Stats};
use crate::storage::{persist_settings, work_records_in_range};

const CLI_INSTALL_PATH: &str = "/usr/local/bin/iaw";

#[specta::specta]
#[tauri::command]
pub(crate) fn get_stats(state: State<'_, Arc<Mutex<AppState>>>) -> Stats {
    let s = state.lock().unwrap();
    build_stats(&s)
}

#[specta::specta]
#[tauri::command]
pub(crate) fn get_work_records(
    state: State<'_, Arc<Mutex<AppState>>>,
    start_unix: i64,
    end_unix: i64,
) -> Result<Vec<HourlyWorkRecord>, String> {
    let s = state.lock().unwrap();
    work_records_in_range(&s, start_unix, end_unix).map_err(|e| e.to_string())
}

#[specta::specta]
#[tauri::command]
pub(crate) fn get_settings(app: AppHandle, state: State<'_, Arc<Mutex<AppState>>>) -> AppSettings {
    let s = state.lock().unwrap();
    let mut settings = s.settings.clone();
    refresh_launch_at_login(&app, &mut settings);
    settings
}

#[specta::specta]
#[tauri::command]
pub(crate) fn update_settings(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    let (today_work_seconds, next_settings) = {
        let mut s = state.lock().unwrap();
        sync_launch_at_login(&app, settings.launch_at_login)?;
        persist_settings(&s.settings_path, &settings).map_err(|e| e.to_string())?;
        s.settings = settings;
        (s.today_work_seconds, s.settings.clone())
    };

    update_tray_title(&app, today_work_seconds, &next_settings);

    Ok(next_settings)
}

#[specta::specta]
#[tauri::command]
pub(crate) fn install_cli(app: AppHandle) -> Result<String, String> {
    install_cli_inner(&app).map(|path| path.display().to_string())
}

#[cfg(target_os = "macos")]
fn install_cli_inner(_app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let source =
        std::env::current_exe().map_err(|err| format!("failed to locate app executable: {err}"))?;
    let destination = std::path::PathBuf::from(CLI_INSTALL_PATH);

    match install_cli_symlink(&source, &destination) {
        Ok(()) => Ok(destination),
        Err(err) if is_permission_error(&err) => {
            install_cli_symlink_with_admin(&source, &destination)?;
            Ok(destination)
        }
        Err(err) => Err(err.to_string()),
    }
}

#[cfg(not(target_os = "macos"))]
fn install_cli_inner(_app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Err("CLI installation is only implemented on macOS for now".to_string())
}

#[cfg(target_os = "macos")]
fn install_cli_symlink(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> std::io::Result<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match std::fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            std::fs::remove_file(destination)?;
        }
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "{} already exists and is not a symlink",
                    destination.display()
                ),
            ));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }

    std::os::unix::fs::symlink(source, destination)
}

#[cfg(target_os = "macos")]
fn install_cli_symlink_with_admin(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), String> {
    let script = format!(
        "set -e; mkdir -p {}; if [ -e {} ] && [ ! -L {} ]; then echo 'destination exists and is not a symlink' >&2; exit 17; fi; ln -sfn {} {}",
        shell_quote(destination.parent().unwrap_or_else(|| std::path::Path::new("/")).as_os_str()),
        shell_quote(destination.as_os_str()),
        shell_quote(destination.as_os_str()),
        shell_quote(source.as_os_str()),
        shell_quote(destination.as_os_str())
    );
    let apple_script = format!(
        "do shell script {} with administrator privileges",
        apple_script_quote(&script)
    );
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(apple_script)
        .output()
        .map_err(|err| format!("failed to request administrator privileges: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(target_os = "macos")]
fn is_permission_error(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
    )
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &std::ffi::OsStr) -> String {
    let value = value.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
fn apple_script_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn shell_quote_wraps_and_escapes_single_quotes() {
        assert_eq!(
            shell_quote(std::ffi::OsStr::new("/tmp/it's iaw")),
            "'/tmp/it'\\''s iaw'"
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn apple_script_quote_wraps_and_escapes_double_quotes() {
        assert_eq!(apple_script_quote("say \"hi\""), "\"say \\\"hi\\\"\"");
    }
}
