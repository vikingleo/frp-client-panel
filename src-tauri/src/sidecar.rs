use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use crate::types::SidecarInfo;

pub fn target_triple() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    {
        "unsupported"
    }
}

pub fn expected_name() -> String {
    let extension = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    format!("frp-panel-client-{}{}", target_triple(), extension)
}

pub fn sidecar_available(app: &AppHandle) -> bool {
    app.shell().sidecar("frp-panel-client").is_ok()
}

fn missing_sidecar_hint() -> String {
    if cfg!(debug_assertions) {
        "开发环境的 Client sidecar 不可用。请运行 pnpm sync:client，或将对应二进制放入 src-tauri/binaries/。"
            .to_string()
    } else {
        "内置 Client 不可用。请重新安装与当前系统和架构匹配的 FRP Panel Client 安装包。".to_string()
    }
}

#[tauri::command]
pub fn get_sidecar_info(app: AppHandle) -> SidecarInfo {
    let available = sidecar_available(&app);
    SidecarInfo {
        available,
        target_triple: target_triple().to_string(),
        expected_name: expected_name(),
        hint: if available {
            "内置 Client 已就绪，无需另外安装 frp-panel。".to_string()
        } else {
            missing_sidecar_hint()
        },
    }
}

pub fn sidecar_command(
    app: &AppHandle,
    args: Vec<String>,
) -> Result<tauri_plugin_shell::process::Command, String> {
    app.shell()
        .sidecar("frp-panel-client")
        .map_err(|e| format!("{} 原始错误：{e}", missing_sidecar_hint()))
        .map(|cmd| {
            cmd.args(args)
                .env("CLIENT_FEATURES_ENABLE_FUNCTIONS", "false")
                .env("CLIENT_FEATURES_ENABLE_REMOTE_SHELL", "false")
                .env("LOGGER_FRP_LOGGER_LEVEL", "info")
                .env("LOGGER_DEFAULT_LOGGER_LEVEL", "info")
        })
}
