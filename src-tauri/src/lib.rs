use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, TimeZone};
use rusqlite::{params, Connection};
use tauri::{
    include_image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_specta::{Builder as SpectaBuilder, Event as SpectaEvent};

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct Stats {
    pub today_work_seconds: u64,
    pub is_active: bool,
    pub idle_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, specta::Type)]
pub struct HourlyWorkRecord {
    pub hour_start_unix: i64,
    pub work_seconds: u64,
}

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
}

#[derive(Clone, Debug, serde::Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct StatsUpdated(pub Stats);

#[derive(Clone, Debug, serde::Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct ShowTab(pub String);

#[derive(Clone, Debug, serde::Serialize, specta::Type, tauri_specta::Event)]
pub struct LogMessage {
    pub timestamp: String,
    pub message: String,
}

fn push_log_message(app: &AppHandle, message: String) {
    let _ = LogMessage {
        timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        message,
    }
    .emit(app);
}

struct AppState {
    is_active: bool,
    // 进入空闲状态的瞬间；处于工作状态时为 None
    idle_started_at: Option<Instant>,
    pending_work_seconds_by_hour: HashMap<i64, u64>,
    last_flush_at: Instant,
    today_start_unix: i64,
    today_end_unix: i64,
    today_work_seconds: u64,
    settings: AppSettings,
    settings_path: PathBuf,
    db: Connection,
}

const IDLE_THRESHOLD_SECS: u64 = 60;
const FLUSH_INTERVAL_SECS: u64 = 60;
const SECONDS_PER_HOUR: i64 = 60 * 60;
const TRAY_ID: &str = "work-time";
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

fn init_app_data_dir(app: &App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    Ok(app_data_dir)
}

fn init_db(app_data_dir: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    let db = Connection::open(app_data_dir.join("work-stats.sqlite3"))?;
    init_db_schema(&db)?;

    Ok(db)
}

fn init_db_schema(db: &Connection) -> rusqlite::Result<()> {
    log_sql!(CREATE_HOURLY_WORK_STATS_SQL);
    db.execute(CREATE_HOURLY_WORK_STATS_SQL, [])?;

    Ok(())
}

fn default_settings() -> AppSettings {
    AppSettings {
        show_tray_time: true,
        tray_time_format: TrayTimeFormat::HhMm,
    }
}

#[cfg(debug_assertions)]
fn log_settings_file(action: &str, path: &Path) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    eprintln!("[settings {timestamp}] {action} path={}", path.display());
}

#[cfg(not(debug_assertions))]
fn log_settings_file(_action: &str, _path: &Path) {}

fn load_settings(path: &Path) -> Result<AppSettings, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(default_settings());
    }

    log_settings_file("read", path);
    let contents = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

fn persist_settings(path: &Path, settings: &AppSettings) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    log_settings_file("write", path);
    std::fs::write(path, serde_json::to_vec_pretty(settings)?)?;

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

fn format_hours_minutes(total_seconds: u64) -> String {
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

fn tray_title(today_work_seconds: u64, settings: &AppSettings) -> String {
    if !settings.show_tray_time {
        return String::new();
    }

    match settings.tray_time_format {
        TrayTimeFormat::HhMmSs => format_hours_minutes_seconds(today_work_seconds),
        TrayTimeFormat::HhMm => format_hours_minutes(today_work_seconds),
    }
}

fn update_tray_title(app: &AppHandle, today_work_seconds: u64, settings: &AppSettings) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Err(err) = tray.set_title(Some(tray_title(today_work_seconds, settings))) {
            eprintln!("failed to update tray title: {err}");
        }
    }
}

#[specta::specta]
#[tauri::command]
fn get_stats(state: State<'_, Arc<Mutex<AppState>>>) -> Stats {
    let s = state.lock().unwrap();
    build_stats(&s)
}

#[specta::specta]
#[tauri::command]
fn get_work_records(
    state: State<'_, Arc<Mutex<AppState>>>,
    start_unix: i64,
    end_unix: i64,
) -> Result<Vec<HourlyWorkRecord>, String> {
    let s = state.lock().unwrap();
    work_records_in_range(&s, start_unix, end_unix).map_err(|e| e.to_string())
}

#[specta::specta]
#[tauri::command]
fn get_settings(state: State<'_, Arc<Mutex<AppState>>>) -> AppSettings {
    let s = state.lock().unwrap();
    s.settings.clone()
}

#[specta::specta]
#[tauri::command]
fn update_settings(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    let (today_work_seconds, next_settings) = {
        let mut s = state.lock().unwrap();
        persist_settings(&s.settings_path, &settings).map_err(|e| e.to_string())?;
        s.settings = settings;
        (s.today_work_seconds, s.settings.clone())
    };

    update_tray_title(&app, today_work_seconds, &next_settings);

    Ok(next_settings)
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
            is_active: false,
            idle_started_at: None,
            pending_work_seconds_by_hour: HashMap::new(),
            last_flush_at: Instant::now(),
            today_start_unix: 0,
            today_end_unix: 86_400,
            today_work_seconds: 0,
            settings: default_settings(),
            settings_path: std::env::temp_dir().join("i-am-working-test-settings.json"),
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

    #[test]
    fn load_settings_uses_json_file_values() {
        let path = std::env::temp_dir().join(format!(
            "i-am-working-test-settings-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let settings = AppSettings {
            show_tray_time: false,
            tray_time_format: TrayTimeFormat::HhMmSs,
        };

        persist_settings(&path, &settings).unwrap();

        assert_eq!(load_settings(&path).unwrap(), settings);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn export_bindings() {
        use specta_typescript::Typescript;

        specta_builder()
            .export(Typescript::default(), "../src/lib/bindings.ts")
            .expect("Failed to export typescript bindings");
    }

    // ---- apply_tick 状态机测试 ----

    // 辅助：构造一个处于"活动"或"空闲"初始状态的 state
    fn test_state_active() -> AppState {
        let mut s = test_state();
        s.is_active = true;
        s
    }

    fn test_state_idle() -> AppState {
        let mut s = test_state();
        s.is_active = false;
        s.idle_started_at = Some(Instant::now());
        s
    }

    #[test]
    fn apply_tick_active_stays_active_when_idle_below_threshold() {
        // 活动 + idle=10s → 仍活动，无切换，累计 1 秒工作
        let mut state = test_state_active();
        let (was, now) = apply_tick(&mut state, 10.0, 1_000);
        assert_eq!((was, now), (true, true));
        assert!(state.is_active);
        assert!(state.idle_started_at.is_none());
        assert_eq!(state.today_work_seconds, 1);
        assert_eq!(state.pending_work_seconds_by_hour.get(&0), Some(&1));
    }

    #[test]
    fn apply_tick_idle_to_active_transition_at_threshold_boundary() {
        // 空闲 + idle=59.99s → 切换为活动，累计 1 秒
        let mut state = test_state_idle();
        let (was, now) = apply_tick(&mut state, 59.99, 1_000);
        assert_eq!((was, now), (false, true));
        assert!(state.is_active);
        assert!(state.idle_started_at.is_none());
        assert_eq!(state.today_work_seconds, 1);
    }

    #[test]
    fn apply_tick_active_to_idle_transition_at_threshold() {
        // 活动 + idle=60s（等于阈值，不满足 < 60）→ 切换为空闲
        let mut state = test_state_active();
        let (was, now) = apply_tick(&mut state, 60.0, 1_000);
        assert_eq!((was, now), (true, false));
        assert!(!state.is_active);
        assert!(state.idle_started_at.is_some());
        // 不累计工作秒数
        assert_eq!(state.today_work_seconds, 0);
        assert!(state.pending_work_seconds_by_hour.is_empty());
    }

    #[test]
    fn apply_tick_idle_stays_idle_does_not_re_record_start() {
        // 已空闲 + idle=120s → 仍空闲，不切换，idle_started_at 不被覆盖
        let mut state = test_state_idle();
        let original_idle_start = state.idle_started_at;
        let (was, now) = apply_tick(&mut state, 120.0, 1_000);
        assert_eq!((was, now), (false, false));
        assert_eq!(state.idle_started_at, original_idle_start);
    }

    #[test]
    fn apply_tick_accumulates_work_seconds_within_today_range() {
        // now 在 today_range 内 → 累计 today_work_seconds
        let mut state = test_state_active();
        state.today_start_unix = 1_000;
        state.today_end_unix = 2_000;
        apply_tick(&mut state, 10.0, 1_500);
        apply_tick(&mut state, 10.0, 1_500);
        assert_eq!(state.today_work_seconds, 2);
    }

    #[test]
    fn apply_tick_does_not_accumulate_today_work_outside_range() {
        // now < today_start → 不累计 today_work_seconds，但仍累计到 pending_work_seconds_by_hour
        let mut state = test_state_active();
        state.today_start_unix = 2_000;
        state.today_end_unix = 3_000;
        apply_tick(&mut state, 10.0, 1_500);
        assert_eq!(state.today_work_seconds, 0);
        assert_eq!(state.pending_work_seconds_by_hour.get(&0), Some(&1));

        // now >= today_end → 同样不累计 today_work_seconds，但 pending 仍按小时累计
        // 4_000 秒落在 hour_start=3_600 的小时桶
        apply_tick(&mut state, 10.0, 4_000);
        assert_eq!(state.today_work_seconds, 0);
        assert_eq!(state.pending_work_seconds_by_hour.get(&3_600), Some(&1));
    }

    #[test]
    fn apply_tick_accumulates_per_hour_buckets() {
        // 跨小时：两次 tick 落在不同小时桶
        let mut state = test_state_active();
        state.today_start_unix = 0;
        state.today_end_unix = 86_400;

        apply_tick(&mut state, 5.0, 3_599); // hour_start = 0
        apply_tick(&mut state, 5.0, 3_600); // hour_start = 3600
        apply_tick(&mut state, 5.0, 3_601); // hour_start = 3600

        assert_eq!(state.pending_work_seconds_by_hour.get(&0), Some(&1));
        assert_eq!(state.pending_work_seconds_by_hour.get(&3_600), Some(&2));
        assert_eq!(state.today_work_seconds, 3);
    }

    #[test]
    fn apply_tick_exactly_below_threshold_is_active() {
        // 验证阈值边界：idle=59.999...s 仍活动；idle=60s 进入空闲
        let mut state = test_state_active();
        let (_, is_active) = apply_tick(&mut state, 59.999_999_999, 1_000);
        assert!(is_active);

        let mut state = test_state_active();
        let (_, is_active) = apply_tick(&mut state, 60.0, 1_000);
        assert!(!is_active);
    }

    #[test]
    fn apply_tick_zero_idle_is_active() {
        // 刚操作过：idle=0 → 活动
        let mut state = test_state_idle();
        let (was, now) = apply_tick(&mut state, 0.0, 1_000);
        assert_eq!((was, now), (false, true));
        assert!(state.is_active);
    }
}

// 系统空闲时间检测：返回自最后一次用户输入（鼠标/键盘任意事件）至今的秒数。
// macOS: 走 CoreGraphics 的 CGEventSourceSecondsSinceLastEventType，
//   该 API 设计上就是用来查询"用户多久没操作了"，不需要辅助功能权限。
//   因此彻底替代了原来的 device_query 轮询方案——既不会触发系统弹窗，
//   也省掉了每 200ms 一次的轮询线程，CPU/电池占用更低。
#[cfg(target_os = "macos")]
mod cg {
    use std::ffi::c_uint;
    // kCGEventSourceStateHIDSystemState = 1
    pub const HID_SYSTEM_STATE: c_uint = 1;
    // kCGAnyInputEventType = 0xFFFFFFFF（uint32 全 1，Apple 头文件里定义为 ~0）
    pub const ANY_INPUT_EVENT: c_uint = 0xFFFF_FFFF;

    extern "C" {
        pub fn CGEventSourceSecondsSinceLastEventType(state_id: c_uint, event_type: c_uint) -> f64;
    }
}

#[cfg(target_os = "macos")]
fn system_idle_seconds() -> f64 {
    unsafe { cg::CGEventSourceSecondsSinceLastEventType(cg::HID_SYSTEM_STATE, cg::ANY_INPUT_EVENT) }
}

#[cfg(not(target_os = "macos"))]
fn system_idle_seconds() -> f64 {
    0.0
}

// 单次 tick 的状态机核心逻辑：依据系统空闲时长更新活动状态、累计工作秒数，
// 并返回切换信息供调用方做副作用（日志/事件）。纯函数，不接触 AppHandle / 系统 API。
//
// 返回 (was_active, is_active)：调用方据此决定是否发送状态切换日志。
fn apply_tick(state: &mut AppState, idle_secs: f64, now: i64) -> (bool, bool) {
    let was_active = state.is_active;
    if idle_secs < IDLE_THRESHOLD_SECS as f64 {
        state.is_active = true;
        state.idle_started_at = None;
        let hour_start = hour_start_unix(now);
        *state
            .pending_work_seconds_by_hour
            .entry(hour_start)
            .or_insert(0) += 1;
        if now >= state.today_start_unix && now < state.today_end_unix {
            state.today_work_seconds += 1;
        }
    } else if state.idle_started_at.is_none() {
        // 刚刚越过阈值进入空闲：记录起点
        state.is_active = false;
        state.idle_started_at = Some(Instant::now());
    }
    (was_active, state.is_active)
}

// 每秒 tick：若距上次活动 < 60s 则累计工作时长；否则标记为空闲
fn spawn_ticker(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let state = app.state::<Arc<Mutex<AppState>>>();
        let (stats, settings) = {
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

            // 系统级空闲检测：idle_secs 即用户未操作时长，直接与阈值比较判定活动/空闲
            let idle_secs = system_idle_seconds();
            push_log_message(&app, format!("system_idle_seconds() = {idle_secs:.3}s"));

            let (was_active, is_active) = apply_tick(&mut s, idle_secs, now);

            // 状态切换时发送日志到前端
            if was_active && !is_active {
                push_log_message(&app, format!("state: active -> idle"));
            } else if !was_active && is_active {
                push_log_message(&app, format!("state: idle -> active"));
            }

            if s.last_flush_at.elapsed() >= Duration::from_secs(FLUSH_INTERVAL_SECS) {
                if let Err(err) = flush_pending_work(&mut s) {
                    eprintln!("failed to flush work stats: {err}");
                }
            }

            (build_stats(&s), s.settings.clone())
        };
        update_tray_title(&app, stats.today_work_seconds, &settings);
        let _ = StatsUpdated(stats).emit(&app);
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

fn specta_builder() -> SpectaBuilder<tauri::Wry> {
    SpectaBuilder::<tauri::Wry>::new()
        .dangerously_cast_bigints_to_number()
        .commands(tauri_specta::collect_commands![
            get_stats,
            get_work_records,
            get_settings,
            update_settings,
        ])
        .events(tauri_specta::collect_events![
            StatsUpdated,
            ShowTab,
            LogMessage
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta = specta_builder();

    #[cfg(debug_assertions)]
    {
        use specta_typescript::Typescript;
        specta
            .export(Typescript::default(), "../src/lib/bindings.ts")
            .expect("Failed to export typescript bindings");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "log" {
                    // 日志窗口：直接销毁，下次重新创建以释放内存
                    api.prevent_close();
                    let _ = window.destroy();
                } else {
                    // 主窗口：隐藏而非销毁，保留 webview 上下文
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            specta.mount_events(app);

            let app_data_dir = init_app_data_dir(app)?;
            let db = init_db(&app_data_dir)?;
            let (today_start, today_end) = today_range_unix();
            let today_work_seconds =
                persisted_work_seconds_in_range(&db, today_start, today_end).unwrap_or(0);
            let settings_path = app_data_dir.join("settings.json");
            let settings = load_settings(&settings_path).unwrap_or_else(|_err| {
                #[cfg(debug_assertions)]
                eprintln!("failed to load app settings: {_err}");
                default_settings()
            });
            let state = Arc::new(Mutex::new(AppState {
                is_active: false,
                idle_started_at: None,
                pending_work_seconds_by_hour: HashMap::new(),
                last_flush_at: Instant::now(),
                today_start_unix: today_start,
                today_end_unix: today_end,
                today_work_seconds,
                settings: settings.clone(),
                settings_path,
                db,
            }));
            app.manage(state.clone());

            // macOS: 不在 Dock 显示图标
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // 托盘菜单：统计 / 查看日志 / 设置 / 退出
            let stats_item = MenuItem::with_id(app, "stats", "统计", true, None::<&str>)?;
            let log_item = MenuItem::with_id(app, "log", "查看日志", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu =
                Menu::with_items(app, &[&stats_item, &settings_item, &log_item, &quit_item])?;

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
            update_tray_title(app.handle(), today_work_seconds, &settings);

            spawn_ticker(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
