use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use crate::config::load_profile;
use crate::process::{start_client_inner, start_native_profile_inner, stop_client_inner};
use crate::runtime::AppRuntime;
use crate::server_config::load_server_profile;
use crate::server_process::{start_server_profile_inner, stop_server_inner};
use crate::types::ClientMode;

pub fn init_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let connect_item = MenuItem::with_id(app, "connect", "连接", true, None::<&str>)?;
    let disconnect_item = MenuItem::with_id(app, "disconnect", "断开", true, None::<&str>)?;
    let server_start_item =
        MenuItem::with_id(app, "server-start", "启动本机 frps", true, None::<&str>)?;
    let server_stop_item =
        MenuItem::with_id(app, "server-stop", "停止本机 frps", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &connect_item,
            &disconnect_item,
            &server_start_item,
            &server_stop_item,
            &quit_item,
        ],
    )?;

    #[cfg(target_os = "macos")]
    const TRAY_ICON_BYTES: &[u8] = include_bytes!("assets/tray_macos_44.png");
    #[cfg(not(target_os = "macos"))]
    const TRAY_ICON_BYTES: &[u8] = include_bytes!("assets/tray_macos_44.png");

    let icon = tauri::image::Image::from_bytes(TRAY_ICON_BYTES)?;

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("frp-panel Client")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "connect" => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let Some(runtime) = app_handle.try_state::<AppRuntime>() else {
                        return;
                    };
                    let Ok(Some(profile)) = load_profile(app_handle.clone(), None) else {
                        return;
                    };
                    match profile.mode {
                        ClientMode::PanelManaged => {
                            if let Some(config) = profile.panel {
                                let _ =
                                    start_client_inner(app_handle.clone(), runtime.inner(), config)
                                        .await;
                            }
                        }
                        ClientMode::NativeFrpc => {
                            let _ = start_native_profile_inner(
                                app_handle.clone(),
                                runtime.inner(),
                                profile,
                            )
                            .await;
                        }
                    }
                });
            }
            "disconnect" => {
                if let Some(runtime) = app.try_state::<AppRuntime>() {
                    let _ = stop_client_inner(app, runtime.inner());
                }
            }
            "server-start" => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let Some(runtime) = app_handle.try_state::<AppRuntime>() else {
                        return;
                    };
                    let Ok(Some(profile)) = load_server_profile(app_handle.clone(), None) else {
                        return;
                    };
                    let _ =
                        start_server_profile_inner(app_handle.clone(), &runtime.server, profile)
                            .await;
                });
            }
            "server-stop" => {
                if let Some(runtime) = app.try_state::<AppRuntime>() {
                    let _ = stop_server_inner(app, &runtime.server);
                }
            }
            "quit" => {
                if let Some(runtime) = app.try_state::<AppRuntime>() {
                    let _ = stop_client_inner(app, runtime.inner());
                    let _ = stop_server_inner(app, &runtime.server);
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
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
