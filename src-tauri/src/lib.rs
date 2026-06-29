use std::collections::BTreeMap;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

const MIN_SECONDS: i64 = 1;
const MAX_SECONDS: i64 = 60 * 60;
const DEFAULT_SECONDS: i64 = 25 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DailyWork {
    worked_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    duration_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkState {
    version: i64,
    settings: Settings,
    daily_work: BTreeMap<String, DailyWork>,
}

impl Default for WorkState {
    fn default() -> Self {
        Self {
            version: 1,
            settings: Settings {
                duration_seconds: DEFAULT_SECONDS,
            },
            daily_work: BTreeMap::new(),
        }
    }
}

struct ActiveTimer {
    started_at: i64,
    duration_seconds: i64,
    abort_handle: tauri::async_runtime::JoinHandle<()>,
}

#[derive(Clone)]
struct AppState {
    work_state: Arc<Mutex<WorkState>>,
    active_timer: Arc<Mutex<Option<ActiveTimer>>>,
    tray_tick: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            work_state: Arc::new(Mutex::new(WorkState::default())),
            active_timer: Arc::new(Mutex::new(None)),
            tray_tick: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateResponse {
    button_label: String,
    duration_seconds: i64,
    today_worked_seconds: i64,
    is_active: bool,
    active_started_at: Option<i64>,
    active_duration_seconds: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveDurationResponse {
    duration_seconds: i64,
    today_worked_seconds: i64,
}

fn format_date_key_ms(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .unwrap()
        .format("%Y-%m-%d")
        .to_string()
}

fn get_today_key() -> String {
    format_date_key_ms(Local::now().timestamp_millis())
}

fn get_state_file_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok()
}

fn clamp_duration(duration_seconds: i64) -> i64 {
    duration_seconds.clamp(MIN_SECONDS, MAX_SECONDS)
}

fn read_worked_seconds(state: &AppState, date_key: &str) -> i64 {
    state
        .work_state
        .lock()
        .unwrap()
        .daily_work
        .get(date_key)
        .map(|d| d.worked_seconds)
        .unwrap_or(0)
}

fn format_countdown(total_seconds: i64) -> String {
    let seconds = total_seconds.max(0);
    let minutes_part = seconds / 60;
    let seconds_part = seconds % 60;
    format!("{}:{:02}", minutes_part, seconds_part)
}

fn calc_remaining(timer: &ActiveTimer) -> i64 {
    let ends_at = timer.started_at + timer.duration_seconds * 1000;
    let now = Local::now().timestamp_millis();
    ((ends_at - now) / 1000).max(0) + if (ends_at - now) % 1000 > 0 { 1 } else { 0 }
}

fn read_active_timer_remaining_seconds(state: &AppState) -> i64 {
    let guard = state.active_timer.lock().unwrap();
    guard.as_ref().map_or(0, calc_remaining)
}

fn read_active_timer_elapsed_seconds(state: &AppState) -> i64 {
    let guard = state.active_timer.lock().unwrap();
    let Some(timer) = guard.as_ref() else {
        return 0;
    };
    timer.duration_seconds - calc_remaining(timer)
}

fn read_displayed_worked_seconds(state: &AppState, date_key: &str) -> i64 {
    read_worked_seconds(state, date_key) + read_active_timer_elapsed_seconds(state)
}

fn load_work_state(app: &AppHandle, state: &AppState) {
    let Some(path) = get_state_file_path(app) else {
        return;
    };
    let file = path.join("work-state.json");
    match std::fs::read_to_string(&file) {
        Ok(content) => {
            println!(
                "[IO {}] READ {} ({} bytes)",
                Local::now().to_rfc3339(),
                file.display(),
                content.len()
            );
            if let Ok(stored) = serde_json::from_str::<WorkState>(&content) {
                let mut ws = state.work_state.lock().unwrap();
                ws.version = 1;
                ws.settings.duration_seconds = clamp_duration(stored.settings.duration_seconds);
                ws.daily_work = stored.daily_work;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "[IO {}] READ {} (not found, using defaults)",
                Local::now().to_rfc3339(),
                file.display()
            );
        }
        Err(e) => {
            eprintln!("Failed to load work state: {}", e);
        }
    }
}

fn save_work_state(app: &AppHandle, state: &AppState) -> Result<(), Box<dyn Error>> {
    let Some(dir) = get_state_file_path(app) else {
        return Ok(());
    };
    let file = dir.join("work-state.json");
    let serialized = {
        let ws = state.work_state.lock().unwrap();
        serde_json::to_string_pretty(&*ws)? + "\n"
    };
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&file, serialized.as_bytes())?;
    println!(
        "[IO {}] WRITE {} ({} bytes)",
        Local::now().to_rfc3339(),
        file.display(),
        serialized.len()
    );
    Ok(())
}

fn add_worked_period(started_at_ms: i64, duration_seconds: i64, state: &AppState) {
    let mut remaining = duration_seconds;
    let mut cursor_ms = started_at_ms;
    while remaining > 0 {
        let date_key = format_date_key_ms(cursor_ms);
        let cursor_dt = Local.timestamp_millis_opt(cursor_ms).unwrap();
        let next_day_naive = cursor_dt.date_naive().succ_opt().unwrap();
        let next_day_midnight = next_day_naive.and_hms_opt(0, 0, 0).unwrap();
        let next_day = Local.from_local_datetime(&next_day_midnight).unwrap();
        let secs_until_next_day = ((next_day.timestamp_millis() - cursor_ms) / 1000).max(1);
        let secs_to_add = remaining.min(secs_until_next_day);
        {
            let mut ws = state.work_state.lock().unwrap();
            ws.daily_work
                .entry(date_key)
                .or_insert_with(|| DailyWork { worked_seconds: 0 })
                .worked_seconds += secs_to_add;
        }
        remaining -= secs_to_add;
        cursor_ms += secs_to_add * 1000;
    }
}

fn build_state_response(state: &AppState) -> StateResponse {
    let today_worked_seconds = read_worked_seconds(state, &get_today_key());
    let guard = state.active_timer.lock().unwrap();
    let (is_active, started_at, duration) = match guard.as_ref() {
        Some(t) => (true, Some(t.started_at), Some(t.duration_seconds)),
        None => (false, None, None),
    };
    StateResponse {
        button_label: if today_worked_seconds == 0 {
            "开始工作".to_string()
        } else {
            "继续工作".to_string()
        },
        duration_seconds: state.work_state.lock().unwrap().settings.duration_seconds,
        today_worked_seconds,
        is_active,
        active_started_at: started_at,
        active_duration_seconds: duration,
    }
}

fn hide_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let win_c = win.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = win_c.hide();
        });
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let win_c = win.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = win_c.show();
            let _ = win_c.set_focus();
        });
        return;
    }
    match WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("工作提醒")
        .inner_size(820.0, 620.0)
        .min_inner_size(720.0, 540.0)
        .resizable(true)
        .maximizable(true)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .visible(true)
        .build()
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Failed to create main window: {}", e);
            return;
        }
    };
}

fn update_tray(app: &AppHandle, state: &AppState) {
    if app.tray_by_id("main-tray").is_none() {
        return;
    }
    let today_minutes = read_displayed_worked_seconds(state, &get_today_key()) / 60;
    let remaining = read_active_timer_remaining_seconds(state);
    let countdown = format_countdown(remaining);
    let is_active = state.active_timer.lock().unwrap().is_some();

    let title = if is_active {
        Some(countdown.clone())
    } else {
        None
    };
    let tooltip = if is_active {
        format!(
            "I Am Working - 本轮剩余 {}，今日已工作 {} 分钟",
            countdown, today_minutes
        )
    } else {
        format!("I Am Working - 今日已工作 {} 分钟", today_minutes)
    };
    let app_c = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = app_c.tray_by_id("main-tray") {
            let _ = tray.set_title(title.as_deref());
            let _ = tray.set_tooltip(Some(&tooltip));
        }
    });
}

fn start_tray_countdown(app: &AppHandle, state: &AppState) {
    let mut tick = state.tray_tick.lock().unwrap();
    if tick.is_some() {
        return;
    }
    let app_c = app.clone();
    let state_c = state.clone();
    let handle = tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let still_active = state_c.active_timer.lock().unwrap().is_some();
            if !still_active {
                break;
            }
            update_tray(&app_c, &state_c);
        }
    });
    *tick = Some(handle);
}

fn stop_tray_countdown(state: &AppState) {
    if let Some(handle) = state.tray_tick.lock().unwrap().take() {
        handle.abort();
    }
}

fn begin_work(app: &AppHandle, state: &AppState) {
    {
        let mut active = state.active_timer.lock().unwrap();
        if active.is_some() {
            return;
        }
        let duration = state.work_state.lock().unwrap().settings.duration_seconds;
        let started_at = Local::now().timestamp_millis();
        let app_c = app.clone();
        let state_c = state.clone();
        let handle = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(duration as u64)).await;
            let still = state_c.active_timer.lock().unwrap().is_some();
            if still {
                if let Err(e) = finish_work(&app_c, &state_c) {
                    eprintln!("finish_work failed: {}", e);
                }
            }
        });
        *active = Some(ActiveTimer {
            started_at,
            duration_seconds: duration,
            abort_handle: handle,
        });
    }
    hide_main_window(app);
    start_tray_countdown(app, state);
    update_tray(app, state);
}

fn finish_work(app: &AppHandle, state: &AppState) -> Result<(), Box<dyn Error>> {
    let timer = state.active_timer.lock().unwrap().take();
    let Some(timer) = timer else {
        return Ok(());
    };
    stop_tray_countdown(state);
    add_worked_period(timer.started_at, timer.duration_seconds, state);
    save_work_state(app, state)?;
    update_tray(app, state);
    show_main_window(app);
    let _ = app.emit("timer:finished", ());
    Ok(())
}

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let icon: Image = tauri::include_image!("icons/tray.png");
    let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    let state = app.state::<AppState>().inner().clone();
    update_tray(app, &state);
    Ok(())
}

#[tauri::command]
fn get_state(state: tauri::State<'_, AppState>) -> StateResponse {
    build_state_response(state.inner())
}

#[tauri::command]
async fn save_duration(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    duration_seconds: i64,
) -> Result<SaveDurationResponse, String> {
    let s = state.inner().clone();
    let app_c = app.clone();
    let resp = tokio::task::spawn_blocking(move || {
        {
            let mut ws = s.work_state.lock().unwrap();
            ws.settings.duration_seconds = clamp_duration(duration_seconds);
        }
        let _ = save_work_state(&app_c, &s);
        SaveDurationResponse {
            duration_seconds: s.work_state.lock().unwrap().settings.duration_seconds,
            today_worked_seconds: read_worked_seconds(&s, &get_today_key()),
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    update_tray(&app, state.inner());
    Ok(resp)
}

#[tauri::command]
async fn start_work(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    println!("[DBG] start_work cmd entered");
    let s = state.inner().clone();
    begin_work(&app, &s);
    println!("[DBG] start_work cmd returning");
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let state = app.state::<AppState>().inner().clone();
            load_work_state(&handle, &state);
            if let Err(e) = create_tray(&handle) {
                eprintln!("Failed to create tray: {}", e);
            }
            show_main_window(&handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_duration,
            start_work
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app: &AppHandle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app.state::<AppState>().inner().clone();
                if let Some(t) = state.active_timer.lock().unwrap().take() {
                    t.abort_handle.abort();
                }
                stop_tray_countdown(&state);
            }
        });
}
