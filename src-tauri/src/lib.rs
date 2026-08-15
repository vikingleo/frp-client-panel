use tauri::{Manager, RunEvent, WindowEvent};
#[cfg(target_os = "macos")]
use tauri_plugin_autostart::MacosLauncher;

use command_parser::parse_panel_command;
use config::{load_connection, save_connection};
use process::{clear_logs, get_logs, get_status, start_client, stop_client};
use runtime::AppRuntime;
use sidecar::get_sidecar_info;
use tray::init_tray;

mod command_parser;
mod config;
mod process;
mod runtime;
mod sidecar;
mod tray;
mod types;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let autostart = tauri_plugin_autostart::Builder::new()
        .args(["--auto-launched"])
        .app_name("frp-panel Client");
    #[cfg(target_os = "macos")]
    let autostart = autostart.macos_launcher(MacosLauncher::LaunchAgent);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(autostart.build())
        .manage(AppRuntime::default())
        .setup(|app| {
            init_tray(app.handle())?;
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(Some(config)) = config::load_connection_inner(&app_handle) {
                    if config.auto_connect {
                        if let Some(runtime) = app_handle.try_state::<AppRuntime>() {
                            let _ = process::start_client_inner(
                                app_handle.clone(),
                                runtime.inner(),
                                config,
                            )
                            .await;
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            save_connection,
            load_connection,
            parse_panel_command,
            start_client,
            stop_client,
            get_status,
            get_logs,
            clear_logs,
            get_sidecar_info
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } if label == "main" => {
                api.prevent_close();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            RunEvent::ExitRequested { .. } => {
                if let Some(runtime) = app_handle.try_state::<AppRuntime>() {
                    let _ = process::stop_client_inner(app_handle, runtime.inner());
                }
            }
            _ => {}
        });
}
