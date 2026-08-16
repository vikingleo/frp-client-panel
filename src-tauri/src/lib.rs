use tauri::{Manager, RunEvent, WindowEvent};
#[cfg(target_os = "macos")]
use tauri_plugin_autostart::MacosLauncher;

use command_parser::parse_panel_command;
use config::{
    delete_profile, list_profiles, load_connection, load_managed_native_config, load_profile,
    save_connection, save_native_profile, select_profile,
};
use discovery::get_external_client_discovery;
use process::{
    clear_logs, get_logs, get_status, start_client, start_client_inner, start_native_profile,
    start_native_profile_inner, stop_client,
};
use runtime::AppRuntime;
use server_config::{
    delete_server_profile, list_server_profiles, load_managed_server_config, load_server_profile,
    save_server_profile, select_server_profile,
};
use server_process::{
    clear_server_logs, get_server_logs, get_server_status, start_server_profile,
    start_server_profile_inner, stop_server, stop_server_inner,
};
use sidecar::get_sidecar_info;
use tray::init_tray;

mod command_parser;
mod config;
mod discovery;
mod process;
mod runtime;
mod server_config;
mod server_process;
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
                if let Ok(Some(profile)) = load_profile(app_handle.clone(), None) {
                    if let Some(runtime) = app_handle.try_state::<AppRuntime>() {
                        match profile.mode {
                            types::ClientMode::PanelManaged => {
                                if let Some(config) =
                                    profile.panel.filter(|config| config.auto_connect)
                                {
                                    let _ = start_client_inner(
                                        app_handle.clone(),
                                        runtime.inner(),
                                        config,
                                    )
                                    .await;
                                }
                            }
                            types::ClientMode::NativeFrpc => {
                                if profile
                                    .native
                                    .as_ref()
                                    .map(|native| native.auto_connect)
                                    .unwrap_or(false)
                                {
                                    let _ = start_native_profile_inner(
                                        app_handle.clone(),
                                        runtime.inner(),
                                        profile,
                                    )
                                    .await;
                                }
                            }
                        }
                    }
                }
                if let Ok(Some(profile)) = load_server_profile(app_handle.clone(), None) {
                    if profile.native.auto_start {
                        if let Some(runtime) = app_handle.try_state::<AppRuntime>() {
                            let _ = start_server_profile_inner(
                                app_handle.clone(),
                                &runtime.server,
                                profile,
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
            list_profiles,
            load_profile,
            load_managed_native_config,
            select_profile,
            save_native_profile,
            delete_profile,
            list_server_profiles,
            load_server_profile,
            load_managed_server_config,
            select_server_profile,
            save_server_profile,
            delete_server_profile,
            parse_panel_command,
            start_client,
            start_native_profile,
            stop_client,
            get_status,
            get_logs,
            clear_logs,
            start_server_profile,
            stop_server,
            get_server_status,
            get_server_logs,
            clear_server_logs,
            get_sidecar_info,
            get_external_client_discovery
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
                    let _ = stop_server_inner(app_handle, &runtime.server);
                }
            }
            _ => {}
        });
}
