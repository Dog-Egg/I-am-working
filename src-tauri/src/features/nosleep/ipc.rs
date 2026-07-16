use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app::state::AppState;
use crate::features::nosleep::cli::{ipc_info_path, IpcInfo};
use crate::features::nosleep::inhibitor::sync_sleep_inhibitor;
use crate::features::nosleep::tray::update_nosleep_tray;
use tauri::AppHandle;

#[derive(Debug, serde::Deserialize)]
struct NosleepRequest {
    name: String,
    active: bool,
}

fn apply_guard_status(
    enabled: bool,
    active_guards: &mut HashSet<String>,
    request: &NosleepRequest,
) -> Result<Vec<String>, &'static str> {
    if !enabled {
        return Err("nosleep is disabled in settings\n");
    }

    if request.active {
        active_guards.insert(request.name.trim().to_string());
    } else {
        active_guards.remove(request.name.trim());
    }
    let mut active_guards = active_guards.iter().cloned().collect::<Vec<_>>();
    active_guards.sort();
    Ok(active_guards)
}

pub(crate) fn spawn_cli_ipc_server(
    app: AppHandle,
    app_data_dir: PathBuf,
    state: Arc<Mutex<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let token = generate_ipc_token();

    let ipc_info = IpcInfo {
        port,
        token: token.clone(),
    };
    std::fs::write(
        ipc_info_path(app_data_dir),
        serde_json::to_vec_pretty(&ipc_info)?,
    )?;

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_connection(stream, &app, &state, &token),
                Err(err) => eprintln!("failed to accept CLI IPC connection: {err}"),
            }
        }
    });

    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    app: &AppHandle,
    state: &Arc<Mutex<AppState>>,
    token: &str,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let mut buffer = [0_u8; 4096];
    let read = match stream.read(&mut buffer) {
        Ok(read) => read,
        Err(err) => {
            eprintln!("failed to read CLI IPC request: {err}");
            return;
        }
    };
    let request = String::from_utf8_lossy(&buffer[..read]);

    if !is_authorized(&request, token) {
        write_response(
            &mut stream,
            "401 Unauthorized",
            "text/plain",
            "unauthorized\n",
        );
        return;
    }

    let request_line = request.lines().next().unwrap_or_default();
    match request_line {
        "POST /nosleep HTTP/1.1" | "POST /nosleep HTTP/1.0" => {
            let Some((_headers, body)) = request.split_once("\r\n\r\n") else {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain",
                    "missing request body\n",
                );
                return;
            };
            let nosleep = match serde_json::from_str::<NosleepRequest>(body.trim()) {
                Ok(nosleep) if !nosleep.name.trim().is_empty() => nosleep,
                _ => {
                    write_response(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain",
                        "invalid nosleep request\n",
                    );
                    return;
                }
            };
            let active_guards = {
                let mut state = state.lock().unwrap();
                let enabled = state.settings.nosleep_enabled;
                match apply_guard_status(enabled, &mut state.active_guards, &nosleep) {
                    Ok(active_guards) => active_guards,
                    Err(message) => {
                        write_response(&mut stream, "409 Conflict", "text/plain", message);
                        return;
                    }
                }
            };

            update_nosleep_tray(app, &active_guards);
            sync_sleep_inhibitor(!active_guards.is_empty());
            write_response(&mut stream, "200 OK", "application/json", "{}\n");
        }
        _ => {
            write_response(&mut stream, "404 Not Found", "text/plain", "not found\n");
        }
    }
}

fn is_authorized(request: &str, token: &str) -> bool {
    let expected = format!("Bearer {token}");

    request.lines().any(|line| {
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };

        name.eq_ignore_ascii_case("authorization") && value.trim() == expected
    })
}

fn write_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn generate_ipc_token() -> String {
    let mut bytes = [0_u8; 32];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .is_ok()
    {
        return hex_encode(&bytes);
    }

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos:x}{:x}", std::process::id())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_authorized_accepts_bearer_token() {
        let request = "POST /nosleep HTTP/1.1\r\nAuthorization: Bearer abc123\r\n\r\n";

        assert!(is_authorized(request, "abc123"));
    }

    #[test]
    fn is_authorized_rejects_wrong_token() {
        let request = "POST /nosleep HTTP/1.1\r\nAuthorization: Bearer wrong\r\n\r\n";

        assert!(!is_authorized(request, "abc123"));
    }

    #[test]
    fn guard_status_is_rejected_when_nosleep_is_disabled() {
        let mut active_guards = HashSet::new();
        let request = NosleepRequest {
            name: "codex".to_string(),
            active: true,
        };

        assert!(apply_guard_status(false, &mut active_guards, &request).is_err());
        assert!(active_guards.is_empty());
    }

    #[test]
    fn guard_status_updates_and_sorts_active_names() {
        let mut active_guards = HashSet::from(["zed".to_string()]);
        let request = NosleepRequest {
            name: " codex ".to_string(),
            active: true,
        };

        assert_eq!(
            apply_guard_status(true, &mut active_guards, &request).unwrap(),
            vec!["codex".to_string(), "zed".to_string()]
        );
    }
}
