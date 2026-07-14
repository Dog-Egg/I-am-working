use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

const APP_IDENTIFIER: &str = "com.iamworking.desktop";
const IPC_FILE_NAME: &str = "ipc.json";

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct IpcInfo {
    pub(crate) port: u16,
    pub(crate) token: String,
}

#[derive(Debug, serde::Serialize)]
struct NosleepRequest {
    name: String,
    active: bool,
}

pub fn set_guard_status(name: &str, active: bool) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("name cannot be empty".to_string());
    }

    let body = serde_json::to_string(&NosleepRequest {
        name: name.to_string(),
        active,
    })
    .map_err(|err| format!("failed to encode nosleep request: {err}"))?;
    let response = send_ipc_request("POST", "/nosleep", Some(&body))?;
    parse_empty_success_response(&response)
}

fn send_ipc_request(method: &str, path: &str, body: Option<&str>) -> Result<String, String> {
    let ipc_info = read_ipc_info()?;
    let mut stream = TcpStream::connect(("127.0.0.1", ipc_info.port))
        .map_err(|err| format!("failed to connect to I Am Working: {err}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|err| format!("failed to configure IPC read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|err| format!("failed to configure IPC write timeout: {err}"))?;

    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        ipc_info.token,
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("failed to send IPC request: {err}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|err| format!("failed to read IPC response: {err}"))?;

    Ok(response)
}

pub fn run_from_env_args() -> Result<(), String> {
    run_from_args(std::env::args().skip(1))
}

pub fn run_from_args(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let args = args.into_iter().collect::<Vec<_>>();
    let (name, active) = parse_nosleep_args(&args)?;
    set_guard_status(name, active)
}

fn parse_nosleep_args(args: &[String]) -> Result<(&str, bool), String> {
    if args.len() != 4 || args[0] != "nosleep" || args[2] != "--name" {
        return Err(usage());
    }

    let active = match args[1].as_str() {
        "on" => true,
        "off" => false,
        _ => return Err(usage()),
    };
    let name = args[3].trim();
    if name.is_empty() {
        return Err("name cannot be empty".to_string());
    }

    Ok((name, active))
}

pub(crate) fn ipc_info_path(app_data_dir: PathBuf) -> PathBuf {
    app_data_dir.join(IPC_FILE_NAME)
}

fn read_ipc_info() -> Result<IpcInfo, String> {
    let path = macos_app_data_dir()?.join(IPC_FILE_NAME);
    let contents = std::fs::read_to_string(&path).map_err(|err| {
        format!(
            "failed to read IPC file at {}: {err}. Is I Am Working running?",
            path.display()
        )
    })?;

    serde_json::from_str(&contents).map_err(|err| {
        format!(
            "failed to parse IPC file at {}: {err}. Restart I Am Working and try again.",
            path.display()
        )
    })
}

#[cfg(target_os = "macos")]
fn macos_app_data_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;

    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join(APP_IDENTIFIER))
}

#[cfg(not(target_os = "macos"))]
fn macos_app_data_dir() -> Result<PathBuf, String> {
    Err("the I Am Working CLI IPC path is only implemented on macOS for now".to_string())
}

fn parse_empty_success_response(response: &str) -> Result<(), String> {
    let headers = response
        .split_once("\r\n\r\n")
        .map(|(headers, _body)| headers)
        .ok_or_else(|| "invalid IPC response".to_string())?;
    let status_line = headers
        .lines()
        .next()
        .ok_or_else(|| "missing IPC status line".to_string())?;

    if !status_line.contains(" 200 ") {
        return Err(format!("I Am Working returned {status_line}"));
    }

    Ok(())
}

fn usage() -> String {
    "usage: iaw nosleep on --name <name>\n       iaw nosleep off --name <name>".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nosleep_args_accepts_on_and_off() {
        let on = ["nosleep", "on", "--name", "codex"].map(String::from);
        let off = ["nosleep", "off", "--name", "claude"].map(String::from);

        assert_eq!(parse_nosleep_args(&on), Ok(("codex", true)));
        assert_eq!(parse_nosleep_args(&off), Ok(("claude", false)));
    }

    #[test]
    fn parse_nosleep_args_requires_name_option() {
        let missing = ["nosleep", "on"].map(String::from);
        let positional = ["nosleep", "on", "codex"].map(String::from);
        let empty = ["nosleep", "on", "--name", "  "].map(String::from);

        assert!(parse_nosleep_args(&missing).is_err());
        assert!(parse_nosleep_args(&positional).is_err());
        assert_eq!(
            parse_nosleep_args(&empty),
            Err("name cannot be empty".to_string())
        );
    }

    #[test]
    fn parse_nosleep_args_rejects_unrelated_command() {
        let unrelated = ["worker", "start", "codex"].map(String::from);

        assert!(parse_nosleep_args(&unrelated).is_err());
    }

    #[test]
    fn parse_empty_success_response_accepts_200() {
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";

        assert!(parse_empty_success_response(response).is_ok());
    }
}
