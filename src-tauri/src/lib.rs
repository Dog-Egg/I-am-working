use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use app_state::{spawn_ticker, today_range_unix, AppState};
use desktop::{create_tray, refresh_launch_at_login, sync_launch_at_login};
use ipc::spawn_cli_ipc_server;
use storage::{
    default_settings, init_app_data_dir, init_db, load_settings, persisted_work_seconds_in_range,
};
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_specta::Builder as SpectaBuilder;

mod app_state;
pub mod cli;
mod commands;
mod desktop;
mod ipc;
mod json_insert;
mod models;
mod power;
mod storage;

fn specta_builder() -> SpectaBuilder<tauri::Wry> {
    SpectaBuilder::<tauri::Wry>::new()
        .dangerously_cast_bigints_to_number()
        .commands(tauri_specta::collect_commands![
            commands::get_stats,
            commands::get_work_records,
            commands::get_settings,
            commands::update_settings,
            commands::install_cli,
            commands::is_cli_installed,
            commands::uninstall_cli,
        ])
        .events(tauri_specta::collect_events![
            models::StatsUpdated,
            models::ShowTab,
            models::LogMessage
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
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
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
            let mut settings = load_settings(&settings_path).unwrap_or_else(|_err| {
                #[cfg(debug_assertions)]
                eprintln!("failed to load app settings: {_err}");
                default_settings()
            });
            if let Err(err) = sync_launch_at_login(app.handle(), settings.launch_at_login) {
                #[cfg(debug_assertions)]
                eprintln!("failed to sync launch at login: {err}");
                refresh_launch_at_login(app.handle(), &mut settings);
            }
            let state = Arc::new(Mutex::new(AppState {
                is_active: false,
                active_agents: HashSet::new(),
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

            create_tray(app, today_work_seconds, &settings)?;
            spawn_cli_ipc_server(app.handle().clone(), app_data_dir, state)?;
            spawn_ticker(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                power::stop_sleep_guard();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_bindings() {
        use specta_typescript::Typescript;

        specta_builder()
            .export(Typescript::default(), "../src/lib/bindings.ts")
            .expect("Failed to export typescript bindings");
    }
}
