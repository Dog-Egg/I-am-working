use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, TimeZone};
use device_query::DeviceQuery;
use rusqlite::{params, Connection};
use tauri::{
    include_image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Emitter, Manager, State, WindowEvent,
};

#[derive(Clone, serde::Serialize)]
struct Stats {
    today_work_seconds: u64,
    is_active: bool,
    idle_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
struct HourlyWorkRecord {
    hour_start_unix: i64,
    work_seconds: u64,
}

struct AppState {
    last_activity: Instant,
    is_active: bool,
    // 进入空闲状态的瞬间；处于工作状态时为 None
    idle_started_at: Option<Instant>,
    pending_work_seconds_by_hour: HashMap<i64, u64>,
    last_flush_at: Instant,
    today_start_unix: i64,
    today_end_unix: i64,
    today_work_seconds: u64,
    db: Connection,
}

const IDLE_THRESHOLD_SECS: u64 = 60;
const FLUSH_INTERVAL_SECS: u64 = 60;
const SECONDS_PER_HOUR: i64 = 60 * 60;
const CREATE_HOURLY_WORK_STATS_SQL: &str = "CREATE TABLE IF NOT EXISTS hourly_work_stats (
    hour_start_unix INTEGER PRIMARY KEY,
    work_seconds INTEGER NOT NULL DEFAULT 0
)";
const UPSERT_HOURLY_WORK_SQL: &str = "INSERT INTO hourly_work_stats (hour_start_unix, work_seconds)
    VALUES (?1, ?2)
    ON CONFLICT(hour_start_unix)
    DO UPDATE SET work_seconds = work_seconds + excluded.work_seconds";
const SELECT_WORK_SECONDS_IN_RANGE_SQL: &str = "SELECT COALESCE(SUM(work_seconds), 0)
    FROM hourly_work_stats
    WHERE hour_start_unix >= ?1 AND hour_start_unix < ?2";
const SELECT_WORK_RECORDS_SQL: &str = "SELECT hour_start_unix, work_seconds
    FROM hourly_work_stats
    WHERE hour_start_unix >= ?1 AND hour_start_unix < ?2
    ORDER BY hour_start_unix";

macro_rules! log_sql {
    ($sql:expr) => {
        #[cfg(debug_assertions)]
        {
            emit_sql_log($sql, &[]);
        }
    };
    ($sql:expr, $( $name:expr => $value:expr ),+ $(,)?) => {
        #[cfg(debug_assertions)]
        {
            let params = [$(($name, $value.to_string())),+];
            emit_sql_log($sql, &params);
        }
    };
}

#[cfg(debug_assertions)]
fn emit_sql_log(sql: &str, params: &[(&str, String)]) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    let normalized_sql = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let params_text = params
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(", ");

    if params_text.is_empty() {
        eprintln!("[sql {timestamp}] {normalized_sql}");
    } else {
        eprintln!("[sql {timestamp}] {normalized_sql} | params: {params_text}");
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn hour_start_unix(timestamp: i64) -> i64 {
    timestamp - timestamp.rem_euclid(SECONDS_PER_HOUR)
}

fn today_range_unix() -> (i64, i64) {
    let now = Local::now();
    let today = now.date_naive();
    let tomorrow = today.succ_opt().unwrap_or(today);
    let start = Local
        .with_ymd_and_hms(today.year(), today.month(), today.day(), 0, 0, 0)
        .earliest()
        .unwrap_or(now);
    let end = Local
        .with_ymd_and_hms(tomorrow.year(), tomorrow.month(), tomorrow.day(), 0, 0, 0)
        .earliest()
        .unwrap_or(start + chrono::Duration::days(1));
    (start.timestamp(), end.timestamp())
}

fn init_db(app: &App) -> Result<Connection, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    let db = Connection::open(app_data_dir.join("work-stats.sqlite3"))?;
    init_db_schema(&db)?;

    Ok(db)
}

fn init_db_schema(db: &Connection) -> rusqlite::Result<()> {
    log_sql!(CREATE_HOURLY_WORK_STATS_SQL);
    db.execute(CREATE_HOURLY_WORK_STATS_SQL, [])?;

    Ok(())
}

fn flush_pending_work(state: &mut AppState) -> rusqlite::Result<()> {
    if state.pending_work_seconds_by_hour.is_empty() {
        state.last_flush_at = Instant::now();
        return Ok(());
    }

    let pending = state.pending_work_seconds_by_hour.clone();
    let tx = state.db.transaction()?;
    {
        let mut stmt = tx.prepare(UPSERT_HOURLY_WORK_SQL)?;

        for (hour_start, work_seconds) in pending {
            log_sql!(
                UPSERT_HOURLY_WORK_SQL,
                "?1" => hour_start,
                "?2" => work_seconds,
            );
            stmt.execute(params![hour_start, work_seconds])?;
        }
    }
    tx.commit()?;

    state.pending_work_seconds_by_hour.clear();
    state.last_flush_at = Instant::now();

    Ok(())
}

fn persisted_work_seconds_in_range(
    db: &Connection,
    start_unix: i64,
    end_unix: i64,
) -> rusqlite::Result<u64> {
    log_sql!(
        SELECT_WORK_SECONDS_IN_RANGE_SQL,
        "?1" => start_unix,
        "?2" => end_unix,
    );
    let persisted = db.query_row(
        SELECT_WORK_SECONDS_IN_RANGE_SQL,
        params![start_unix, end_unix],
        |row| row.get::<_, i64>(0),
    )?;

    Ok(persisted.max(0) as u64)
}

fn build_stats(state: &AppState) -> Stats {
    Stats {
        today_work_seconds: state.today_work_seconds,
        is_active: state.is_active,
        idle_seconds: state
            .idle_started_at
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0),
    }
}

#[tauri::command]
fn get_stats(state: State<'_, Arc<Mutex<AppState>>>) -> Stats {
    let s = state.lock().unwrap();
    build_stats(&s)
}

#[tauri::command]
fn get_work_records(
    state: State<'_, Arc<Mutex<AppState>>>,
    start_unix: i64,
    end_unix: i64,
) -> Result<Vec<HourlyWorkRecord>, String> {
    let s = state.lock().unwrap();
    work_records_in_range(&s, start_unix, end_unix).map_err(|e| e.to_string())
}

fn work_records_in_range(
    state: &AppState,
    start_unix: i64,
    end_unix: i64,
) -> rusqlite::Result<Vec<HourlyWorkRecord>> {
    let mut records = {
        log_sql!(
            SELECT_WORK_RECORDS_SQL,
            "?1" => start_unix,
            "?2" => end_unix,
        );
        let mut stmt = state.db.prepare(SELECT_WORK_RECORDS_SQL)?;

        let rows = stmt.query_map(params![start_unix, end_unix], |row| {
            Ok(HourlyWorkRecord {
                hour_start_unix: row.get(0)?,
                work_seconds: row.get::<_, i64>(1)?.max(0) as u64,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()?
    };

    for (hour_start, pending_seconds) in &state.pending_work_seconds_by_hour {
        if *hour_start < start_unix || *hour_start >= end_unix {
            continue;
        }

        if let Some(record) = records
            .iter_mut()
            .find(|record| record.hour_start_unix == *hour_start)
        {
            record.work_seconds += pending_seconds;
        } else {
            records.push(HourlyWorkRecord {
                hour_start_unix: *hour_start,
                work_seconds: *pending_seconds,
            });
        }
    }

    records.sort_by_key(|record| record.hour_start_unix);

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> AppState {
        let db = Connection::open_in_memory().unwrap();
        init_db_schema(&db).unwrap();

        AppState {
            last_activity: Instant::now(),
            is_active: false,
            idle_started_at: None,
            pending_work_seconds_by_hour: HashMap::new(),
            last_flush_at: Instant::now(),
            today_start_unix: 0,
            today_end_unix: 86_400,
            today_work_seconds: 0,
            db,
        }
    }

    #[test]
    fn hour_start_unix_rounds_down_to_utc_hour() {
        assert_eq!(hour_start_unix(0), 0);
        assert_eq!(hour_start_unix(3_599), 0);
        assert_eq!(hour_start_unix(3_600), 3_600);
        assert_eq!(hour_start_unix(7_201), 7_200);
        assert_eq!(hour_start_unix(-1), -3_600);
    }

    #[test]
    fn flush_pending_work_inserts_and_accumulates_hourly_rows() {
        let mut state = test_state();

        state.pending_work_seconds_by_hour.insert(3_600, 10);
        flush_pending_work(&mut state).unwrap();

        assert!(state.pending_work_seconds_by_hour.is_empty());
        assert_eq!(
            persisted_work_seconds_in_range(&state.db, 0, 7_200).unwrap(),
            10
        );

        state.pending_work_seconds_by_hour.insert(3_600, 5);
        state.pending_work_seconds_by_hour.insert(7_200, 7);
        flush_pending_work(&mut state).unwrap();

        assert_eq!(
            persisted_work_seconds_in_range(&state.db, 0, 10_800).unwrap(),
            22
        );
        assert_eq!(
            persisted_work_seconds_in_range(&state.db, 3_600, 7_200).unwrap(),
            15
        );
    }

    #[test]
    fn persisted_work_seconds_in_range_uses_start_inclusive_end_exclusive() {
        let mut state = test_state();
        state.pending_work_seconds_by_hour.insert(0, 3);
        state.pending_work_seconds_by_hour.insert(3_600, 5);
        state.pending_work_seconds_by_hour.insert(7_200, 7);
        flush_pending_work(&mut state).unwrap();

        assert_eq!(
            persisted_work_seconds_in_range(&state.db, 3_600, 7_200).unwrap(),
            5
        );
    }

    #[test]
    fn work_records_in_range_merges_persisted_and_pending_records() {
        let mut state = test_state();
        state.pending_work_seconds_by_hour.insert(3_600, 100);
        state.pending_work_seconds_by_hour.insert(7_200, 50);
        flush_pending_work(&mut state).unwrap();

        state.pending_work_seconds_by_hour.insert(3_600, 7);
        state.pending_work_seconds_by_hour.insert(10_800, 11);
        state.pending_work_seconds_by_hour.insert(14_400, 13);

        assert_eq!(
            work_records_in_range(&state, 0, 14_400).unwrap(),
            vec![
                HourlyWorkRecord {
                    hour_start_unix: 3_600,
                    work_seconds: 107,
                },
                HourlyWorkRecord {
                    hour_start_unix: 7_200,
                    work_seconds: 50,
                },
                HourlyWorkRecord {
                    hour_start_unix: 10_800,
                    work_seconds: 11,
                },
            ]
        );
    }

    #[test]
    fn build_stats_uses_cached_today_work_seconds() {
        let mut state = test_state();
        state.today_work_seconds = 42;
        state.pending_work_seconds_by_hour.insert(3_600, 999);
        flush_pending_work(&mut state).unwrap();

        let stats = build_stats(&state);

        assert_eq!(stats.today_work_seconds, 42);
        assert!(!stats.is_active);
        assert_eq!(stats.idle_seconds, 0);
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
            let now = now_unix();
            if now >= s.today_end_unix {
                if let Err(err) = flush_pending_work(&mut s) {
                    eprintln!("failed to flush work stats before day rollover: {err}");
                }

                let (today_start, today_end) = today_range_unix();
                s.today_start_unix = today_start;
                s.today_end_unix = today_end;
                s.today_work_seconds =
                    persisted_work_seconds_in_range(&s.db, today_start, today_end).unwrap_or(0);
            }

            let idle = s.last_activity.elapsed();
            if idle < Duration::from_secs(IDLE_THRESHOLD_SECS) {
                s.is_active = true;
                s.idle_started_at = None;
                let hour_start = hour_start_unix(now);
                *s.pending_work_seconds_by_hour
                    .entry(hour_start)
                    .or_insert(0) += 1;
                if now >= s.today_start_unix && now < s.today_end_unix {
                    s.today_work_seconds += 1;
                }
            } else if s.idle_started_at.is_none() {
                // 刚刚越过阈值进入空闲：记录起点
                s.is_active = false;
                s.idle_started_at = Some(Instant::now());
            }

            if s.last_flush_at.elapsed() >= Duration::from_secs(FLUSH_INTERVAL_SECS) {
                if let Err(err) = flush_pending_work(&mut s) {
                    eprintln!("failed to flush work stats: {err}");
                }
            }

            build_stats(&s)
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
        .on_window_event(|window, event| {
            // 点击窗口关闭按钮时隐藏而非销毁，保留 webview 上下文
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let db = init_db(app)?;
            let (today_start, today_end) = today_range_unix();
            let today_work_seconds =
                persisted_work_seconds_in_range(&db, today_start, today_end).unwrap_or(0);
            let state = Arc::new(Mutex::new(AppState {
                last_activity: Instant::now(),
                is_active: false,
                idle_started_at: None,
                pending_work_seconds_by_hour: HashMap::new(),
                last_flush_at: Instant::now(),
                today_start_unix: today_start,
                today_end_unix: today_end,
                today_work_seconds,
                db,
            }));
            app.manage(state.clone());

            // macOS: 不在 Dock 显示图标
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            spawn_input_monitor(state);
            spawn_ticker(app.handle().clone());

            // 托盘菜单：显示统计 / 退出
            let show_item = MenuItem::with_id(app, "show", "显示统计", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(include_image!("./icons/icon.png"))
                .menu(&menu)
                .tooltip("I am working")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => toggle_window(app),
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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_stats, get_work_records])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
