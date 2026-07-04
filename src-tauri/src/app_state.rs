use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, TimeZone};
use rusqlite::Connection;
use tauri::{AppHandle, Manager};
use tauri_specta::Event as SpectaEvent;

use crate::desktop::update_tray_title;
use crate::models::{AppSettings, LogMessage, Stats, StatsUpdated};
use crate::storage::{flush_pending_work, persisted_work_seconds_in_range};

pub(crate) struct AppState {
    pub(crate) is_active: bool,
    pub(crate) active_agents: HashSet<String>,
    // 进入空闲状态的瞬间；处于工作状态时为 None
    pub(crate) idle_started_at: Option<Instant>,
    pub(crate) pending_work_seconds_by_hour: HashMap<i64, u64>,
    pub(crate) last_flush_at: Instant,
    pub(crate) today_start_unix: i64,
    pub(crate) today_end_unix: i64,
    pub(crate) today_work_seconds: u64,
    pub(crate) settings: AppSettings,
    pub(crate) settings_path: PathBuf,
    pub(crate) db: Connection,
}

const IDLE_THRESHOLD_SECS: u64 = 60;
const FLUSH_INTERVAL_SECS: u64 = 60;
const SECONDS_PER_HOUR: i64 = 60 * 60;

pub(crate) fn push_log_message(app: &AppHandle, message: String) {
    let _ = LogMessage {
        timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        message,
    }
    .emit(app);
}

pub(crate) fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub(crate) fn hour_start_unix(timestamp: i64) -> i64 {
    timestamp - timestamp.rem_euclid(SECONDS_PER_HOUR)
}

pub(crate) fn today_range_unix() -> (i64, i64) {
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

pub(crate) fn build_stats(state: &AppState) -> Stats {
    Stats {
        today_work_seconds: state.today_work_seconds,
        is_active: state.is_active,
        idle_seconds: state
            .idle_started_at
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0),
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
pub(crate) fn apply_tick(state: &mut AppState, idle_secs: f64, now: i64) -> (bool, bool) {
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
pub(crate) fn spawn_ticker(app: AppHandle) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{default_settings, flush_pending_work, init_db_schema};

    fn test_state() -> AppState {
        let db = Connection::open_in_memory().unwrap();
        init_db_schema(&db).unwrap();

        AppState {
            is_active: false,
            active_agents: HashSet::new(),
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
