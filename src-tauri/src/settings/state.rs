use std::path::PathBuf;

use super::models::AppSettings;

/// Runtime state owned by application settings.
pub(crate) struct SettingsState {
    pub(crate) settings: AppSettings,
    pub(crate) settings_path: PathBuf,
}

impl SettingsState {
    pub(crate) fn new(settings: AppSettings, settings_path: PathBuf) -> Self {
        Self {
            settings,
            settings_path,
        }
    }
}
