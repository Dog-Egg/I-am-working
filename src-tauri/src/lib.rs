use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use device_query::DeviceQuery;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};

#[derive(Clone, serde::Serialize)]
struct Stats {
    work_seconds: u64,
    is_active: bool,
    idle_seconds: u64,
}

struct AppState {
    last_activity: Instant,
    work_seconds: u64,
    is_active: bool,
    // 进入空闲状态的瞬间；处于工作状态时为 None
    idle_started_at: Option<Instant>,
}

const IDLE_THRESHOLD_SECS: u64 = 60;

#[tauri::command]
fn get_stats(state: State<'_, Arc<Mutex<AppState>>>) -> Stats {
    let s = state.lock().unwrap();
    Stats {
        work_seconds: s.work_seconds,
        is_active: s.is_active,
        idle_seconds: s
            .idle_started_at
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0),
    }
}

// 全局键鼠监听线程：轮询设备状态，检测到变化即视为活动，更新 last_activity
// 选用轮询而非事件回调，避免 macOS 上 CGEventTap 在窗口获焦时崩溃
fn spawn_input_monitor(state: Arc<Mutex<AppState>>) {
    std::thread::spawn(move || {
        let device_state = device_query::DeviceState::new();
        let mut last_mouse = device_state.get_mouse();
        let mut last_keys = device_state.query_keymap();
        loop {
            std::thread::sleep(Duration::from_millis(200));
            let mouse = device_state.get_mouse();
            let keys = device_state.query_keymap();
            if mouse.coords != last_mouse.coords
                || mouse.button_pressed != last_mouse.button_pressed
                || keys != last_keys
            {
                if let Ok(mut s) = state.lock() {
                    s.last_activity = Instant::now();
                }
            }
            last_mouse = mouse;
            last_keys = keys;
        }
    });
}

// 每秒 tick：若距上次活动 < 60s 则累计工作时长；否则标记为空闲
fn spawn_ticker(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let state = app.state::<Arc<Mutex<AppState>>>();
        let stats = {
            let mut s = state.lock().unwrap();
            let idle = s.last_activity.elapsed();
            if idle < Duration::from_secs(IDLE_THRESHOLD_SECS) {
                s.is_active = true;
                s.idle_started_at = None;
                s.work_seconds += 1;
            } else if s.idle_started_at.is_none() {
                // 刚刚越过阈值进入空闲：记录起点
                s.is_active = false;
                s.idle_started_at = Some(Instant::now());
            }
            Stats {
                work_seconds: s.work_seconds,
                is_active: s.is_active,
                idle_seconds: s
                    .idle_started_at
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0),
            }
        };
        let _ = app.emit("stats-updated", stats);
    });
}

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(Mutex::new(AppState {
            last_activity: Instant::now(),
            work_seconds: 0,
            is_active: false,
            idle_started_at: None,
        })))
        .on_window_event(|window, event| {
            // 点击窗口关闭按钮时隐藏而非销毁，保留 webview 上下文
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            // macOS: 不在 Dock 显示图标
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let state: Arc<Mutex<AppState>> = app.state::<Arc<Mutex<AppState>>>().inner().clone();
            spawn_input_monitor(state);
            spawn_ticker(app.handle().clone());

            // 托盘菜单：显示统计 / 退出
            let show_item = MenuItem::with_id(app, "show", "显示统计", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("I am working")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => toggle_window(app),
                    "quit" => app.exit(0),
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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_stats])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
