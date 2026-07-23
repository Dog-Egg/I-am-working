use tauri::AppHandle;

use crate::app::tray::TRAY_ID;
use crate::settings::models::{AppSettings, TrayTimeFormat};

pub(crate) fn format_hours_minutes(total_seconds: u64) -> String {
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;

    if hours > 0 {
        if minutes > 0 {
            format!("{hours}时{minutes}分")
        } else {
            format!("{hours}时")
        }
    } else {
        format!("{minutes}分")
    }
}

fn format_hours_minutes_seconds(total_seconds: u64) -> String {
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}时{minutes}分{seconds}秒")
    } else if minutes > 0 {
        format!("{minutes}分{seconds}秒")
    } else {
        format!("{seconds}秒")
    }
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
    fn format_hours_minutes_uses_chinese_units_and_omits_seconds() {
        assert_eq!(format_hours_minutes(0), "0分");
        assert_eq!(format_hours_minutes(59), "0分");
        assert_eq!(format_hours_minutes(60), "1分");
        assert_eq!(format_hours_minutes(3_600 + 59 * 60 + 59), "1时59分");
        assert_eq!(format_hours_minutes(100 * 3_600), "100时");
    }

    #[test]
    fn tray_title_respects_visibility_and_format() {
        let mut settings = default_settings();
        assert_eq!(tray_title(3_661, &settings), "1时1分");

        settings.tray_time_format = TrayTimeFormat::HhMmSs;
        assert_eq!(tray_title(3_661, &settings), "1时1分1秒");

        settings.show_tray_time = false;
        assert_eq!(tray_title(3_661, &settings), "");
    }
}
