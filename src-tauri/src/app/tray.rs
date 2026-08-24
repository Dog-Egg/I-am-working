//! Application tray and window-shell integration.

use std::sync::{Arc, Mutex};

use tauri::{
    include_image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_specta::Event as SpectaEvent;

use crate::app::events::ShowTab;
use crate::app::state::AppState;
use crate::features::nosleep::inhibitor::stop_sleep_inhibitor;
use crate::features::nosleep::tray::create_nosleep_tray;
use crate::features::work_timer::storage::flush_pending_work;
use crate::features::work_timer::tray::update_tray_title;
use crate::settings::models::AppSettings;

pub(crate) const TRAY_ID: &str = "work-time";

fn toggle_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

fn show_tab(app: &AppHandle, tab: &str) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }

    let _ = ShowTab(tab.to_string()).emit(app);
}

fn show_log_window(app: &AppHandle) {
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
    let log_item = MenuItem::with_id(app, "log", "日志", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&stats_item, &settings_item, &log_item, &separator, &quit_item],
    )?;

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
                    let AppState { work_timer, db, .. } = &mut *state;
                    if let Err(err) = flush_pending_work(work_timer, db) {
                        eprintln!("failed to flush work stats before quit: {err}");
                    }
                }
                stop_sleep_inhibitor();
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
    if settings.nosleep_enabled {
        create_nosleep_tray(app.handle())?;
    }
    update_tray_title(app.handle(), today_work_seconds, settings);

    Ok(())
}
