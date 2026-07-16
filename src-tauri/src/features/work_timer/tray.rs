use tauri::AppHandle;

use crate::app::tray::TRAY_ID;
use crate::settings::models::{AppSettings, TrayTimeFormat};

pub(crate) fn format_hours_minutes(total_seconds: u64) -> String {
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;

    format!("{hours:02}:{minutes:02}")
}

fn format_hours_minutes_seconds(total_seconds: u64) -> String {
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;

    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

pub(crate) fn tray_title(today_work_seconds: u64, settings: &AppSettings) -> String {
    if !settings.show_tray_time {
        return String::new();
    }

    match settings.tray_time_format {
        TrayTimeFormat::HhMmSs => format_hours_minutes_seconds(today_work_seconds),
        TrayTimeFormat::HhMm => format_hours_minutes(today_work_seconds),
    }
}

pub(crate) fn update_tray_title(app: &AppHandle, today_work_seconds: u64, settings: &AppSettings) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Err(error) = tray.set_title(Some(tray_title(today_work_seconds, settings))) {
            eprintln!("failed to update tray title: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::storage::default_settings;

    #[test]
    fn format_hours_minutes_omits_seconds() {
        assert_eq!(format_hours_minutes(0), "00:00");
        assert_eq!(format_hours_minutes(59), "00:00");
        assert_eq!(format_hours_minutes(60), "00:01");
        assert_eq!(format_hours_minutes(3_600 + 59 * 60 + 59), "01:59");
        assert_eq!(format_hours_minutes(100 * 3_600), "100:00");
    }

    #[test]
    fn tray_title_respects_visibility_and_format() {
        let mut settings = default_settings();
        assert_eq!(tray_title(3_661, &settings), "01:01");

        settings.tray_time_format = TrayTimeFormat::HhMmSs;
        assert_eq!(tray_title(3_661, &settings), "01:01:01");

        settings.show_tray_time = false;
        assert_eq!(tray_title(3_661, &settings), "");
    }
}
