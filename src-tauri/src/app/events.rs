#[derive(Clone, Debug, serde::Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct ShowTab(pub String);

#[derive(Clone, Debug, serde::Serialize, specta::Type, tauri_specta::Event)]
pub struct LogMessage {
    pub timestamp: String,
    pub message: String,
}
