//! CLI installation support for the nosleep feature.

use tauri::AppHandle;

const CLI_INSTALL_PATH: &str = "/usr/local/bin/iaw";

#[cfg(target_os = "macos")]
pub(crate) fn install_cli(_app: &AppHandle) -> Result<std::path::PathBuf, String> {
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
pub(crate) fn install_cli(_app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Err("CLI installation is only implemented on macOS for now".to_string())
}

#[cfg(target_os = "macos")]
pub(crate) fn uninstall_cli() -> Result<std::path::PathBuf, String> {
    let source =
        std::env::current_exe().map_err(|err| format!("failed to locate app executable: {err}"))?;
    let destination = std::path::PathBuf::from(CLI_INSTALL_PATH);

    match uninstall_cli_symlink(&source, &destination) {
        Ok(()) => Ok(destination),
        Err(err) if is_permission_error(&err) => {
            uninstall_cli_symlink_with_admin(&source, &destination)?;
            Ok(destination)
        }
        Err(err) => Err(err.to_string()),
    }
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
            if std::fs::read_link(destination)? == source {
                return Ok(());
            }
            return Err(cli_path_conflict(destination));
        }
        Ok(_) => return Err(cli_path_conflict(destination)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }

    std::os::unix::fs::symlink(source, destination)
}

#[cfg(target_os = "macos")]
fn uninstall_cli_symlink(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> std::io::Result<()> {
    match std::fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if std::fs::read_link(destination)? != source {
                return Err(cli_path_conflict(destination));
            }
            std::fs::remove_file(destination)
        }
        Ok(_) => Err(cli_path_conflict(destination)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(target_os = "macos")]
fn cli_path_conflict(destination: &std::path::Path) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!(
            "{} already exists and is not managed by this application",
            destination.display()
        ),
    )
}

#[cfg(target_os = "macos")]
fn install_cli_symlink_with_admin(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), String> {
    let script = format!(
        "set -e; mkdir -p {}; if [ -L {} ]; then [ \"$(readlink {})\" = {} ] || {{ echo 'destination is not managed by this application' >&2; exit 17; }}; exit 0; fi; if [ -e {} ]; then echo 'destination is not managed by this application' >&2; exit 17; fi; ln -s {} {}",
        shell_quote(destination.parent().unwrap_or_else(|| std::path::Path::new("/")).as_os_str()),
        shell_quote(destination.as_os_str()),
        shell_quote(destination.as_os_str()),
        shell_quote(source.as_os_str()),
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
fn uninstall_cli_symlink_with_admin(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), String> {
    let script = format!(
        "set -e; if [ -L {} ]; then [ \"$(readlink {})\" = {} ] || {{ echo 'destination is not managed by this application' >&2; exit 17; }}; rm -f {}; elif [ -e {} ]; then echo 'destination is not managed by this application' >&2; exit 17; fi",
        shell_quote(destination.as_os_str()),
        shell_quote(destination.as_os_str()),
        shell_quote(source.as_os_str()),
        shell_quote(destination.as_os_str()),
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

    #[cfg(target_os = "macos")]
    fn test_cli_paths(test_name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let directory = std::env::temp_dir().join(format!(
            "iaw-{test_name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        (directory.join("app"), directory.join("iaw"))
    }

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

    #[test]
    #[cfg(target_os = "macos")]
    fn install_cli_does_not_replace_foreign_symlink() {
        let (source, destination) = test_cli_paths("install-conflict");
        let foreign_source = source.with_file_name("foreign-app");
        std::os::unix::fs::symlink(&foreign_source, &destination).unwrap();

        let error = install_cli_symlink(&source, &destination).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_link(&destination).unwrap(), foreign_source);
        std::fs::remove_dir_all(destination.parent().unwrap()).unwrap();
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn uninstall_cli_only_removes_owned_symlink() {
        let (source, destination) = test_cli_paths("uninstall-owned");
        std::os::unix::fs::symlink(&source, &destination).unwrap();

        uninstall_cli_symlink(&source, &destination).unwrap();

        assert!(!destination.exists());
        std::fs::remove_dir_all(destination.parent().unwrap()).unwrap();
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn uninstall_cli_preserves_foreign_symlink() {
        let (source, destination) = test_cli_paths("uninstall-conflict");
        let foreign_source = source.with_file_name("foreign-app");
        std::os::unix::fs::symlink(&foreign_source, &destination).unwrap();

        let error = uninstall_cli_symlink(&source, &destination).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_link(&destination).unwrap(), foreign_source);
        std::fs::remove_dir_all(destination.parent().unwrap()).unwrap();
    }
}
