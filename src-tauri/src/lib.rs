use std::sync::{Arc, Mutex};

use app::state::AppState;
use app::tray::create_tray;
use features::nosleep::commands::install_cli;
#[cfg(target_os = "macos")]
use features::nosleep::commands::uninstall_cli;
use features::nosleep::ipc::spawn_cli_ipc_server;
use features::nosleep::state::NoSleepState;
use features::work_timer::state::WorkTimerState;
use features::work_timer::storage::{init_app_data_dir, init_db, persisted_work_seconds_in_range};
use features::work_timer::ticker::{spawn_ticker, today_range_unix};
use settings::autostart::{refresh_launch_at_login, sync_launch_at_login};
use settings::state::SettingsState;
use settings::storage::{default_settings, load_settings};
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_specta::Builder as SpectaBuilder;

mod app;
pub mod cli;
mod features;
mod json_insert;
mod settings;

fn specta_builder() -> SpectaBuilder<tauri::Wry> {
    SpectaBuilder::<tauri::Wry>::new()
        .dangerously_cast_bigints_to_number()
        .commands(tauri_specta::collect_commands![
            features::work_timer::commands::get_stats,
            features::work_timer::commands::get_work_records,
            settings::commands::get_settings,
            settings::commands::update_settings,
        ])
        .events(tauri_specta::collect_events![
            features::work_timer::models::StatsUpdated,
            app::events::ShowTab,
            app::events::LogMessage
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
                work_timer: WorkTimerState::new(today_start, today_end, today_work_seconds),
                nosleep: NoSleepState::default(),
                settings: SettingsState::new(settings.clone(), settings_path),
                db,
            }));
            app.manage(state.clone());

            // macOS: 不在 Dock 显示图标
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // The CLI lifetime follows the application, not the nosleep setting.
            if let Err(err) = install_cli(app.handle()) {
                eprintln!("failed to install nosleep CLI: {err}");
            }
            create_tray(app, today_work_seconds, &settings)?;
            spawn_cli_ipc_server(app.handle().clone(), app_data_dir, state)?;
            spawn_ticker(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                features::nosleep::inhibitor::stop_sleep_inhibitor();
                #[cfg(target_os = "macos")]
                if let Err(err) = uninstall_cli() {
                    eprintln!("failed to uninstall nosleep CLI: {err}");
                }
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
