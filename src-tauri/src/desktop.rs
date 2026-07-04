use std::sync::{Arc, Mutex};

use tauri::{
    include_image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_specta::Event as SpectaEvent;

use crate::app_state::AppState;
use crate::models::{AppSettings, ShowTab, TrayTimeFormat};
use crate::storage::flush_pending_work;

pub(crate) const TRAY_ID: &str = "work-time";
pub(crate) const AGENT_TRAY_ID: &str = "agent-status";

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
        if let Err(err) = tray.set_title(Some(tray_title(today_work_seconds, settings))) {
            eprintln!("failed to update tray title: {err}");
        }
    }
}

pub(crate) fn update_agent_tray_icon(app: &AppHandle, has_active_agents: bool) {
    if let Some(tray) = app.tray_by_id(AGENT_TRAY_ID) {
        let icon = if has_active_agents {
            include_image!("./icons/agent-on.png")
        } else {
            include_image!("./icons/agent-off.png")
        };

        if let Err(err) = tray.set_icon_with_as_template(Some(icon), true) {
            eprintln!("failed to update agent tray icon: {err}");
        }
    }
}

pub(crate) fn sync_launch_at_login(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    }
    .map_err(|e| e.to_string())
}

pub(crate) fn refresh_launch_at_login(app: &AppHandle, settings: &mut AppSettings) {
    if let Ok(enabled) = app.autolaunch().is_enabled() {
        settings.launch_at_login = enabled;
    }
}

pub(crate) fn toggle_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

pub(crate) fn show_tab(app: &AppHandle, tab: &str) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }

    let _ = ShowTab(tab.to_string()).emit(app);
}

pub(crate) fn show_log_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("log") {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }

    if let Err(err) = WebviewWindowBuilder::new(app, "log", WebviewUrl::App("/log".into()))
        .title("日志")
        .inner_size(820.0, 600.0)
        .build()
    {
        eprintln!("failed to create log window: {err}");
    }
}

pub(crate) fn create_tray(
    app: &App,
    today_work_seconds: u64,
    settings: &AppSettings,
) -> tauri::Result<()> {
    let stats_item = MenuItem::with_id(app, "stats", "统计", true, None::<&str>)?;
    let log_item = MenuItem::with_id(app, "log", "查看日志", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&stats_item, &settings_item, &log_item, &quit_item])?;

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(include_image!("./icons/icon.png"))
        .menu(&menu)
        .tooltip("I am working")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "stats" => show_tab(app, "stats"),
            "log" => show_log_window(app),
            "settings" => show_tab(app, "settings"),
            "quit" => {
                let state = app.state::<Arc<Mutex<AppState>>>();
                if let Ok(mut state) = state.lock() {
                    if let Err(err) = flush_pending_work(&mut state) {
                        eprintln!("failed to flush work stats before quit: {err}");
                    }
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;
    let _agent_tray = TrayIconBuilder::with_id(AGENT_TRAY_ID)
        .icon(include_image!("./icons/agent-off.png"))
        .icon_as_template(true)
        .build(app)?;
    update_tray_title(app.handle(), today_work_seconds, settings);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::default_settings;

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
