#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct Stats {
    pub today_work_seconds: u64,
    pub is_active: bool,
    pub idle_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, specta::Type)]
pub struct HourlyWorkRecord {
    pub hour_start_unix: i64,
    pub work_seconds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize, specta::Type)]
pub enum TrayTimeFormat {
    #[serde(rename = "HH:MM:SS")]
    HhMmSs,
    #[serde(rename = "HH:MM")]
    HhMm,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct AppSettings {
    pub show_tray_time: bool,
    pub tray_time_format: TrayTimeFormat,
    pub launch_at_login: bool,
    #[serde(default)]
    pub nosleep_enabled: bool,
}

#[derive(Clone, Debug, serde::Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct StatsUpdated(pub Stats);

#[derive(Clone, Debug, serde::Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct ShowTab(pub String);

#[derive(Clone, Debug, serde::Serialize, specta::Type, tauri_specta::Event)]
pub struct LogMessage {
    pub timestamp: String,
    pub message: String,
}
