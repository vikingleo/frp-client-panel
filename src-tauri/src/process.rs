use tauri::{AppHandle, Manager, State};
use tauri_plugin_shell::process::CommandEvent;

use crate::config::{load_connection_inner, load_profile};
use crate::discovery::{find_external_client_conflict, find_external_frpc_conflict};
use crate::runtime::{emit_log, emit_status, AppRuntime};
use crate::sidecar::{
    native_sidecar_available, native_sidecar_command, sidecar_available, sidecar_command,
};
use crate::types::{ClientMode, ConnectionConfig, LogEntry, Profile, RuntimeState, RuntimeStatus};

#[tauri::command]
pub async fn start_client(app: AppHandle, runtime: State<'_, AppRuntime>) -> Result<(), String> {
    let config = load_connection_inner(&app)?.ok_or("请先保存连接配置")?;
    start_client_inner(app, runtime.inner(), config).await
}

#[tauri::command]
pub async fn start_native_profile(
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
    profile_id: Option<String>,
) -> Result<(), String> {
    let profile = load_profile(app.clone(), profile_id)?.ok_or("请先创建原生 frpc Profile")?;
    start_native_profile_inner(app, runtime.inner(), profile).await
}

pub async fn start_native_profile_inner(
    app: AppHandle,
    runtime: &AppRuntime,
    profile: Profile,
) -> Result<(), String> {
    profile.validate()?;
    if profile.mode != ClientMode::NativeFrpc {
        return Err("当前 Profile 不是原生 frpc 模式".into());
    }
    let native = profile
        .native
        .as_ref()
        .ok_or_else(|| "原生 frpc Profile 缺少配置文件".to_string())?;
    native.validate()?;
    if !native_sidecar_available(&app) {
        return Err("内置 frpc sidecar 不可用，请重新安装包含原生 frpc 的应用包".into());
    }
    if !std::path::Path::new(&native.config_path).is_file() {
        return Err(format!("找不到 frpc 配置文件：{}", native.config_path));
    }

    {
        let child = runtime.child.lock().map_err(|e| e.to_string())?;
        if child.is_some() {
            return Err("客户端已经在运行".into());
        }
    }
    if let Some(existing) = find_external_frpc_conflict(&native.config_path, None) {
        return Err(format!(
            "检测到相同配置文件的外部 frpc 正在运行（PID {}）。本应用不会重复启动；请在外部进程区域确认其状态。",
            existing.pid
        ));
    }

    let config_path = native.config_path.clone();
    let verify_output = native_sidecar_command(
        &app,
        vec!["verify".into(), "-c".into(), config_path.clone()],
    )?
    .output()
    .await
    .map_err(|error| format!("执行 frpc 配置校验失败：{error}"))?;
    log_command_output(&app, runtime, &verify_output.stdout, "stdout");
    log_command_output(&app, runtime, &verify_output.stderr, "stderr");
    if !verify_output.status.success() {
        let detail = command_error_detail(&verify_output.stderr, &verify_output.stdout);
        let message = format!("frpc 配置校验失败：{detail}");
        runtime.set_runtime_context(
            profile.id.clone(),
            ClientMode::NativeFrpc,
            "frpc",
            Some(config_path.clone()),
        );
        runtime.set_state(RuntimeState::Error, Some(message.clone()));
        emit_log(&app, runtime, "system", message.clone());
        emit_status(&app, runtime, runtime_sidecar_available(&app, runtime));
        return Err(message);
    }

    let command = native_sidecar_command(&app, vec!["-c".into(), config_path.clone()])?;
    let (mut rx, child) = command
        .spawn()
        .map_err(|error| format!("启动 frpc 失败：{error}"))?;
    let child_pid = child.pid();
    {
        let mut guard = runtime.child.lock().map_err(|e| e.to_string())?;
        *guard = Some(child);
    }
    runtime.set_managed_child_pid(child_pid);
    runtime.set_runtime_context(
        profile.id.clone(),
        ClientMode::NativeFrpc,
        "frpc",
        Some(config_path.clone()),
    );
    let generation = runtime.mark_starting();
    runtime.set_state(RuntimeState::Running, None);
    emit_log(
        &app,
        runtime,
        "system",
        format!(
            "已启动{}，profile={}，config={}",
            profile.mode.label(),
            profile.name,
            config_path
        ),
    );
    emit_status(&app, runtime, runtime_sidecar_available(&app, runtime));

    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            let Some(runtime) = app_for_task.try_state::<AppRuntime>() else {
                break;
            };
            if !runtime.is_current_generation(generation) {
                break;
            }
            match event {
                CommandEvent::Stdout(bytes) => {
                    handle_native_output(&app_for_task, runtime.inner(), "stdout", &bytes)
                }
                CommandEvent::Stderr(bytes) => {
                    handle_native_output(&app_for_task, runtime.inner(), "stderr", &bytes)
                }
                CommandEvent::Terminated(payload) => {
                    let was_active_child = runtime
                        .child
                        .lock()
                        .map(|mut guard| guard.take().is_some())
                        .unwrap_or(false);
                    runtime.clear_managed_child_pid();
                    let msg = format!(
                        "frpc 已退出，code={:?}, signal={:?}",
                        payload.code, payload.signal
                    );
                    let was_error =
                        payload.code.unwrap_or_default() != 0 || payload.signal.is_some();
                    if was_error && was_active_child {
                        runtime.set_state(RuntimeState::Error, Some(msg.clone()));
                    } else {
                        runtime.mark_stopped();
                    }
                    emit_log(&app_for_task, runtime.inner(), "system", msg);
                    emit_status(
                        &app_for_task,
                        runtime.inner(),
                        runtime_sidecar_available(&app_for_task, runtime.inner()),
                    );
                    break;
                }
                CommandEvent::Error(error) => {
                    let msg = format!("frpc 进程错误：{error}");
                    runtime.set_state(RuntimeState::Error, Some(msg.clone()));
                    emit_log(&app_for_task, runtime.inner(), "stderr", msg);
                    emit_status(
                        &app_for_task,
                        runtime.inner(),
                        runtime_sidecar_available(&app_for_task, runtime.inner()),
                    );
                }
                _ => {}
            }
        }
    });
    Ok(())
}

pub async fn start_client_inner(
    app: AppHandle,
    runtime: &AppRuntime,
    config: ConnectionConfig,
) -> Result<(), String> {
    config.validate()?;

    {
        let child = runtime.child.lock().map_err(|e| e.to_string())?;
        if child.is_some() {
            return Err("客户端已经在运行".into());
        }
    }

    if let Some(existing) =
        find_external_client_conflict(&config.client_id, runtime.managed_child_pid())
    {
        return Err(format!(
            "检测到系统中已有相同 Client ID 的 frp-panel Client（PID {}）。为避免重复注册，本应用不会再次启动；请在“总览”的“外部 Client”区域确认其状态，或先手动停止外部进程后再由本应用托管。",
            existing.pid
        ));
    }

    let args = client_args(&config);

    let command = sidecar_command(&app, args, &config.client_secret, config.allow_insecure_tls)?;
    let (mut rx, child) = command
        .spawn()
        .map_err(|e| format!("启动 frp-panel-client 失败：{e}"))?;
    let child_pid = child.pid();

    {
        let mut guard = runtime.child.lock().map_err(|e| e.to_string())?;
        *guard = Some(child);
    }
    runtime.set_managed_child_pid(child_pid);

    let generation = runtime.mark_starting();
    runtime.set_runtime_context(
        "panel-default",
        ClientMode::PanelManaged,
        "frp-panel-client",
        None,
    );
    emit_log(
        &app,
        runtime,
        "system",
        format!(
            "已启动 frp-panel-client，client_id={}，api_url={}，rpc_url={}",
            config.client_id, config.api_url, config.rpc_url
        ),
    );
    emit_status(&app, runtime, runtime_sidecar_available(&app, runtime));

    let app_for_task = app.clone();
    let secret_for_mask = config.client_secret.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            let Some(runtime) = app_for_task.try_state::<AppRuntime>() else {
                break;
            };
            if !runtime.is_current_generation(generation) {
                break;
            }
            match event {
                CommandEvent::Stdout(bytes) => {
                    handle_output(
                        &app_for_task,
                        runtime.inner(),
                        "stdout",
                        &bytes,
                        &secret_for_mask,
                    );
                }
                CommandEvent::Stderr(bytes) => {
                    handle_output(
                        &app_for_task,
                        runtime.inner(),
                        "stderr",
                        &bytes,
                        &secret_for_mask,
                    );
                }
                CommandEvent::Terminated(payload) => {
                    let was_active_child = runtime
                        .child
                        .lock()
                        .map(|mut guard| guard.take().is_some())
                        .unwrap_or(false);
                    runtime.clear_managed_child_pid();
                    let msg = format!(
                        "frp-panel-client 已退出，code={:?}, signal={:?}",
                        payload.code, payload.signal
                    );
                    let was_error =
                        payload.code.unwrap_or_default() != 0 || payload.signal.is_some();
                    if was_error && was_active_child {
                        runtime.set_state(RuntimeState::Error, Some(msg.clone()));
                    } else {
                        runtime.mark_stopped();
                    }
                    emit_log(&app_for_task, runtime.inner(), "system", msg);
                    emit_status(
                        &app_for_task,
                        runtime.inner(),
                        runtime_sidecar_available(&app_for_task, runtime.inner()),
                    );
                    break;
                }
                CommandEvent::Error(err) => {
                    let msg = format!("frp-panel-client 进程错误：{err}");
                    runtime.set_state(RuntimeState::Error, Some(msg.clone()));
                    emit_log(&app_for_task, runtime.inner(), "stderr", msg);
                    emit_status(
                        &app_for_task,
                        runtime.inner(),
                        runtime_sidecar_available(&app_for_task, runtime.inner()),
                    );
                }
                _ => {}
            }
        }
    });

    Ok(())
}

fn client_args(config: &ConnectionConfig) -> Vec<String> {
    vec![
        "client".to_string(),
        "-i".to_string(),
        config.client_id.clone(),
        "--api-url".to_string(),
        config.api_url.clone(),
        "--rpc-url".to_string(),
        config.rpc_url.clone(),
    ]
}

fn runtime_sidecar_available(app: &AppHandle, runtime: &AppRuntime) -> bool {
    match runtime.mode.lock().ok().and_then(|guard| *guard) {
        Some(ClientMode::NativeFrpc) => native_sidecar_available(app),
        _ => sidecar_available(app),
    }
}

fn handle_native_output(app: &AppHandle, runtime: &AppRuntime, stream: &str, bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    for raw in text.lines() {
        let line = redact_native_line(raw.trim());
        if !line.is_empty() {
            update_state_from_log(app, runtime, &line);
            emit_log(app, runtime, stream, line);
        }
    }
}

fn log_command_output(app: &AppHandle, runtime: &AppRuntime, bytes: &[u8], stream: &str) {
    let text = String::from_utf8_lossy(bytes);
    for raw in text.lines() {
        let line = redact_native_line(raw.trim());
        if !line.is_empty() {
            emit_log(app, runtime, stream, line);
        }
    }
}

fn command_error_detail(stderr: &[u8], stdout: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);
    let detail = stderr
        .lines()
        .chain(stdout.lines())
        .map(redact_native_line)
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| "frpc 返回非零退出码".into());
    detail
}

fn redact_native_line(line: &str) -> String {
    let lower = line.to_lowercase();
    let sensitive = [
        "auth.token",
        "token =",
        "password =",
        "secret =",
        "adminpwd",
        "plugin.token",
    ];
    if sensitive.iter().any(|needle| lower.contains(needle)) {
        if let Some(index) = line.find('=') {
            return format!("{}= ******", line[..index].trim_end());
        }
        return "[敏感配置已隐藏]".into();
    }
    line.to_string()
}

#[cfg(test)]
mod tests {
    use super::{client_args, redact_native_line};
    use crate::types::ConnectionConfig;

    #[test]
    fn client_secret_is_not_part_of_sidecar_arguments() {
        let config = ConnectionConfig {
            client_id: "user.c.macos".into(),
            client_secret: "must-not-be-an-argument".into(),
            api_url: "https://panel.example.com".into(),
            rpc_url: "wss://panel.example.com".into(),
            auto_connect: false,
            launch_at_login: false,
            allow_insecure_tls: false,
        };

        let args = client_args(&config);

        assert!(!args.iter().any(|arg| arg == "-s" || arg == "--secret"));
        assert!(!args.iter().any(|arg| arg == "must-not-be-an-argument"));
    }

    #[test]
    fn native_log_redaction_never_returns_token_values() {
        assert_eq!(
            redact_native_line("auth.token = \"do-not-log\""),
            "auth.token= ******"
        );
        assert_eq!(
            redact_native_line("password = super-secret"),
            "password= ******"
        );
        assert_eq!(
            redact_native_line("login to server success"),
            "login to server success"
        );
    }
}

#[tauri::command]
pub fn stop_client(app: AppHandle, runtime: State<'_, AppRuntime>) -> Result<(), String> {
    stop_client_inner(&app, runtime.inner())
}

pub fn stop_client_inner(app: &AppHandle, runtime: &AppRuntime) -> Result<(), String> {
    let child = {
        let mut guard = runtime.child.lock().map_err(|e| e.to_string())?;
        guard.take()
    };

    if let Some(child) = child {
        let engine = runtime
            .binary_name
            .lock()
            .ok()
            .and_then(|name| name.clone())
            .unwrap_or_else(|| "客户端".to_string());
        runtime.clear_managed_child_pid();
        child
            .kill()
            .map_err(|e| format!("停止 {engine} 失败：{e}"))?;
        runtime.mark_stopped();
        emit_log(app, runtime, "system", format!("已停止 {engine}"));
        emit_status(app, runtime, runtime_sidecar_available(app, runtime));
    }
    Ok(())
}

#[tauri::command]
pub fn get_status(app: AppHandle, runtime: State<'_, AppRuntime>) -> RuntimeStatus {
    runtime.status(runtime_sidecar_available(&app, runtime.inner()))
}

#[tauri::command]
pub fn get_logs(runtime: State<'_, AppRuntime>) -> Vec<LogEntry> {
    runtime.all_logs()
}

#[tauri::command]
pub fn clear_logs(runtime: State<'_, AppRuntime>) {
    runtime.clear_logs();
}

fn handle_output(app: &AppHandle, runtime: &AppRuntime, stream: &str, bytes: &[u8], secret: &str) {
    let text = String::from_utf8_lossy(bytes);
    for raw in text.lines() {
        let line = mask_secret(raw.trim(), secret);
        if line.is_empty() {
            continue;
        }
        update_state_from_log(app, runtime, &line);
        emit_log(app, runtime, stream, line);
    }
}

fn update_state_from_log(app: &AppHandle, runtime: &AppRuntime, line: &str) {
    let lower = line.to_lowercase();
    if lower.contains("pull client config success")
        || lower.contains("start to run client")
        || lower.contains("run client")
        || lower.contains("client started")
    {
        runtime.set_state(RuntimeState::Running, None);
        emit_status(app, runtime, runtime_sidecar_available(app, runtime));
        return;
    }

    let is_error = lower.contains("fatal")
        || lower.contains("panic")
        || lower.contains("cannot ")
        || lower.contains(" failed")
        || lower.contains(" error")
        || lower.starts_with("error");
    if is_error {
        runtime.set_state(RuntimeState::Error, Some(line.to_string()));
        emit_status(app, runtime, runtime_sidecar_available(app, runtime));
    }
}

fn mask_secret(line: &str, secret: &str) -> String {
    if secret.trim().is_empty() {
        line.to_string()
    } else {
        line.replace(secret, "******")
    }
}
