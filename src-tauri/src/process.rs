use tauri::{AppHandle, Manager, State};
use tauri_plugin_shell::process::CommandEvent;

use crate::config::load_connection_inner;
use crate::runtime::{emit_log, emit_status, AppRuntime};
use crate::sidecar::{sidecar_available, sidecar_command};
use crate::types::{ConnectionConfig, LogEntry, RuntimeState, RuntimeStatus};

#[tauri::command]
pub async fn start_client(app: AppHandle, runtime: State<'_, AppRuntime>) -> Result<(), String> {
    let config = load_connection_inner(&app)?.ok_or("请先保存连接配置")?;
    start_client_inner(app, runtime.inner(), config).await
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

    let args = client_args(&config);

    let command = sidecar_command(&app, args, &config.client_secret, config.allow_insecure_tls)?;
    let (mut rx, child) = command
        .spawn()
        .map_err(|e| format!("启动 frp-panel-client 失败：{e}"))?;

    {
        let mut guard = runtime.child.lock().map_err(|e| e.to_string())?;
        *guard = Some(child);
    }

    let generation = runtime.mark_starting();
    emit_log(
        &app,
        runtime,
        "system",
        format!(
            "已启动 frp-panel-client，client_id={}，api_url={}，rpc_url={}",
            config.client_id, config.api_url, config.rpc_url
        ),
    );
    emit_status(&app, runtime, sidecar_available(&app));

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
                        sidecar_available(&app_for_task),
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
                        sidecar_available(&app_for_task),
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

#[cfg(test)]
mod tests {
    use super::client_args;
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
        child
            .kill()
            .map_err(|e| format!("停止 frp-panel-client 失败：{e}"))?;
        runtime.mark_stopped();
        emit_log(app, runtime, "system", "已停止 frp-panel-client".into());
        emit_status(app, runtime, sidecar_available(app));
    }
    Ok(())
}

#[tauri::command]
pub fn get_status(app: AppHandle, runtime: State<'_, AppRuntime>) -> RuntimeStatus {
    runtime.status(sidecar_available(&app))
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
        emit_status(app, runtime, sidecar_available(app));
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
        emit_status(app, runtime, sidecar_available(app));
    }
}

fn mask_secret(line: &str, secret: &str) -> String {
    if secret.trim().is_empty() {
        line.to_string()
    } else {
        line.replace(secret, "******")
    }
}
