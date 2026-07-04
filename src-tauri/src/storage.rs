use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::{params, Connection};
use tauri::{App, Manager};

use crate::app_state::AppState;
use crate::models::{AppSettings, HourlyWorkRecord, TrayTimeFormat};

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

pub(crate) fn init_app_data_dir(app: &App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    Ok(app_data_dir)
}

pub(crate) fn init_db(app_data_dir: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    let db = Connection::open(app_data_dir.join("work-stats.sqlite3"))?;
    init_db_schema(&db)?;

    Ok(db)
}

pub(crate) fn init_db_schema(db: &Connection) -> rusqlite::Result<()> {
    log_sql!(CREATE_HOURLY_WORK_STATS_SQL);
    db.execute(CREATE_HOURLY_WORK_STATS_SQL, [])?;

    Ok(())
}

pub(crate) fn default_settings() -> AppSettings {
    AppSettings {
        show_tray_time: true,
        tray_time_format: TrayTimeFormat::HhMm,
        launch_at_login: true,
    }
}

#[cfg(debug_assertions)]
fn log_settings_file(action: &str, path: &Path) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    eprintln!("[settings {timestamp}] {action} path={}", path.display());
}

#[cfg(not(debug_assertions))]
fn log_settings_file(_action: &str, _path: &Path) {}

pub(crate) fn load_settings(path: &Path) -> Result<AppSettings, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(default_settings());
    }

    log_settings_file("read", path);
    let contents = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

pub(crate) fn persist_settings(
    path: &Path,
    settings: &AppSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    log_settings_file("write", path);
    std::fs::write(path, serde_json::to_vec_pretty(settings)?)?;

    Ok(())
}

pub(crate) fn flush_pending_work(state: &mut AppState) -> rusqlite::Result<()> {
    if state.pending_work_seconds_by_hour.is_empty() {
        state.last_flush_at = std::time::Instant::now();
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
    state.last_flush_at = std::time::Instant::now();

    Ok(())
}

pub(crate) fn persisted_work_seconds_in_range(
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

pub(crate) fn work_records_in_range(
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
    use std::collections::HashMap;
    use std::time::Instant;

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
    fn load_settings_uses_json_file_values() {
        let path = std::env::temp_dir().join(format!(
            "i-am-working-test-settings-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let settings = AppSettings {
            show_tray_time: false,
            tray_time_format: TrayTimeFormat::HhMmSs,
            launch_at_login: true,
        };

        persist_settings(&path, &settings).unwrap();

        assert_eq!(load_settings(&path).unwrap(), settings);
        let _ = std::fs::remove_file(path);
    }
}
