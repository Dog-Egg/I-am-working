//! Persisted application settings models.

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
