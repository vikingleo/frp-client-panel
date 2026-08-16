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

pub fn native_expected_name() -> String {
    let extension = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    format!("frpc-{}{}", target_triple(), extension)
}

pub fn server_expected_name() -> String {
    let extension = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    format!("frps-{}{}", target_triple(), extension)
}

pub fn sidecar_available(app: &AppHandle) -> bool {
    app.shell().sidecar("frp-panel-client").is_ok()
}

pub fn native_sidecar_available(app: &AppHandle) -> bool {
    app.shell().sidecar("frpc").is_ok()
}

pub fn server_sidecar_available(app: &AppHandle) -> bool {
    app.shell().sidecar("frps").is_ok()
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
    let native_available = native_sidecar_available(&app);
    let server_available = server_sidecar_available(&app);
    SidecarInfo {
        available,
        target_triple: target_triple().to_string(),
        expected_name: expected_name(),
        hint: if available {
            "内置 Client 已就绪，无需另外安装 frp-panel。".to_string()
        } else {
            missing_sidecar_hint()
        },
        native_available,
        native_target_triple: target_triple().to_string(),
        native_expected_name: native_expected_name(),
        server_available,
        server_target_triple: target_triple().to_string(),
        server_expected_name: server_expected_name(),
    }
}

pub fn sidecar_command(
    app: &AppHandle,
    args: Vec<String>,
    client_secret: &str,
    allow_insecure_tls: bool,
) -> Result<tauri_plugin_shell::process::Command, String> {
    app.shell()
        .sidecar("frp-panel-client")
        .map_err(|e| format!("{} 原始错误：{e}", missing_sidecar_hint()))
        .map(|cmd| {
            cmd.args(args)
                // Keep the Client Secret out of the process argument list. The upstream client
                // reads this same value from CLIENT_SECRET through its runtime environment.
                .env("CLIENT_SECRET", client_secret)
                // Certificate verification is mandatory by default. Self-signed deployments must
                // be explicitly acknowledged in the desktop configuration.
                .env(
                    "CLIENT_TLS_INSECURE_SKIP_VERIFY",
                    tls_skip_verify_value(allow_insecure_tls),
                )
                .env("CLIENT_FEATURES_ENABLE_FUNCTIONS", "false")
                .env("CLIENT_FEATURES_ENABLE_REMOTE_SHELL", "false")
                .env("LOGGER_FRP_LOGGER_LEVEL", "info")
                .env("LOGGER_DEFAULT_LOGGER_LEVEL", "info")
        })
}

pub fn native_sidecar_command(
    app: &AppHandle,
    args: Vec<String>,
) -> Result<tauri_plugin_shell::process::Command, String> {
    app.shell()
        .sidecar("frpc")
        .map_err(|error| format!("内置 frpc 不可用：{error}"))
        .map(|command| command.args(args))
}

pub fn server_sidecar_command(
    app: &AppHandle,
    args: Vec<String>,
) -> Result<tauri_plugin_shell::process::Command, String> {
    app.shell()
        .sidecar("frps")
        .map_err(|error| format!("内置 frps 不可用：{error}"))
        .map(|command| command.args(args))
}

fn tls_skip_verify_value(allow_insecure_tls: bool) -> &'static str {
    if allow_insecure_tls {
        "true"
    } else {
        "false"
    }
}

#[cfg(test)]
mod tests {
    use super::{native_expected_name, server_expected_name, tls_skip_verify_value};

    #[test]
    fn tls_verification_is_enabled_by_default() {
        assert_eq!(tls_skip_verify_value(false), "false");
    }

    #[test]
    fn tls_exception_requires_explicit_opt_in() {
        assert_eq!(tls_skip_verify_value(true), "true");
    }

    #[test]
    fn native_sidecar_uses_tauri_target_specific_name() {
        assert!(native_expected_name().starts_with("frpc-"));
    }

    #[test]
    fn server_sidecar_uses_tauri_target_specific_name() {
        assert!(server_expected_name().starts_with("frps-"));
    }
}
