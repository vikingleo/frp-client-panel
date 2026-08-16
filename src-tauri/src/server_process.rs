use tauri::{AppHandle, Manager, State};
use tauri_plugin_shell::process::CommandEvent;

use crate::runtime::{emit_server_log, emit_server_status, AppRuntime, ServerRuntime};
use crate::server_config::load_server_profile;
use crate::sidecar::{server_sidecar_available, server_sidecar_command};
use crate::types::{LogEntry, RuntimeState, ServerProfile, ServerRuntimeStatus};

#[tauri::command]
pub async fn start_server_profile(
    app: AppHandle,
    runtime: State<'_, AppRuntime>,
    profile_id: Option<String>,
) -> Result<(), String> {
    let profile =
        load_server_profile(app.clone(), profile_id)?.ok_or("请先创建本机 frps Server Profile")?;
    start_server_profile_inner(app, &runtime.server, profile).await
}

pub async fn start_server_profile_inner(
    app: AppHandle,
    runtime: &ServerRuntime,
    profile: ServerProfile,
) -> Result<(), String> {
    profile.validate()?;
    if !server_sidecar_available(&app) {
        return Err("内置 frps sidecar 不可用，请重新安装包含服务端引擎的应用包".into());
    }

    let config_path = profile.native.config_path.clone();
    if !std::path::Path::new(&config_path).is_file() {
        return Err(format!("找不到 frps 配置文件：{config_path}"));
    }
    {
        let child = runtime.child.lock().map_err(|error| error.to_string())?;
        if child.is_some() {
            return Err("frps 服务端已经在运行".into());
        }
    }

    let verify_output = server_sidecar_command(
        &app,
        vec!["verify".into(), "-c".into(), config_path.clone()],
    )?
    .output()
    .await
    .map_err(|error| format!("执行 frps 配置校验失败：{error}"))?;
    log_command_output(&app, runtime, &verify_output.stdout, "stdout");
    log_command_output(&app, runtime, &verify_output.stderr, "stderr");
    if !verify_output.status.success() {
        let detail = command_error_detail(&verify_output.stderr, &verify_output.stdout);
        let message = format!("frps 配置校验失败：{detail}");
        runtime.set_runtime_context(profile.id, "frps", Some(config_path));
        runtime.set_state(RuntimeState::Error, Some(message.clone()));
        emit_server_log(&app, runtime, "system", message.clone());
        emit_server_status(&app, runtime, server_sidecar_available(&app));
        return Err(message);
    }

    let command = server_sidecar_command(&app, vec!["-c".into(), config_path.clone()])?;
    let (mut rx, child) = command
        .spawn()
        .map_err(|error| format!("启动 frps 失败：{error}"))?;
    let child_pid = child.pid();
    {
        let mut guard = runtime.child.lock().map_err(|error| error.to_string())?;
        *guard = Some(child);
    }
    runtime.set_managed_child_pid(child_pid);
    runtime.set_runtime_context(profile.id.clone(), "frps", Some(config_path.clone()));
    let generation = runtime.mark_starting();
    runtime.set_state(RuntimeState::Running, None);
    emit_server_log(
        &app,
        runtime,
        "system",
        format!(
            "已启动本机 frps，profile={}，config={config_path}",
            profile.name
        ),
    );
    emit_server_status(&app, runtime, server_sidecar_available(&app));

    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            let Some(app_runtime) = app_for_task.try_state::<AppRuntime>() else {
                break;
            };
            let server_runtime = &app_runtime.server;
            if !server_runtime.is_current_generation(generation) {
                break;
            }
            match event {
                CommandEvent::Stdout(bytes) => {
                    handle_output(&app_for_task, server_runtime, "stdout", &bytes)
                }
                CommandEvent::Stderr(bytes) => {
                    handle_output(&app_for_task, server_runtime, "stderr", &bytes)
                }
                CommandEvent::Terminated(payload) => {
                    let was_active_child = server_runtime
                        .child
                        .lock()
                        .map(|mut guard| guard.take().is_some())
                        .unwrap_or(false);
                    server_runtime.clear_managed_child_pid();
                    let message = format!(
                        "frps 已退出，code={:?}, signal={:?}",
                        payload.code, payload.signal
                    );
                    let was_error =
                        payload.code.unwrap_or_default() != 0 || payload.signal.is_some();
                    if was_error && was_active_child {
                        server_runtime.set_state(RuntimeState::Error, Some(message.clone()));
                    } else {
                        server_runtime.mark_stopped();
                    }
                    emit_server_log(&app_for_task, server_runtime, "system", message);
                    emit_server_status(
                        &app_for_task,
                        server_runtime,
                        server_sidecar_available(&app_for_task),
                    );
                    break;
                }
                CommandEvent::Error(error) => {
                    let message = format!("frps 进程错误：{error}");
                    server_runtime.set_state(RuntimeState::Error, Some(message.clone()));
                    emit_server_log(&app_for_task, server_runtime, "stderr", message);
                    emit_server_status(
                        &app_for_task,
                        server_runtime,
                        server_sidecar_available(&app_for_task),
                    );
                }
                _ => {}
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub fn stop_server(runtime: State<'_, AppRuntime>, app: AppHandle) -> Result<(), String> {
    stop_server_inner(&app, &runtime.server)
}

pub fn stop_server_inner(app: &AppHandle, runtime: &ServerRuntime) -> Result<(), String> {
    let child = {
        let mut guard = runtime.child.lock().map_err(|error| error.to_string())?;
        guard.take()
    };
    if let Some(child) = child {
        child
            .kill()
            .map_err(|error| format!("停止 frps 失败：{error}"))?;
        runtime.clear_managed_child_pid();
        runtime.mark_stopped();
        emit_server_log(app, runtime, "system", "已停止本机 frps".into());
        emit_server_status(app, runtime, server_sidecar_available(app));
    }
    Ok(())
}

#[tauri::command]
pub fn get_server_status(app: AppHandle, runtime: State<'_, AppRuntime>) -> ServerRuntimeStatus {
    runtime.server.status(server_sidecar_available(&app))
}

#[tauri::command]
pub fn get_server_logs(runtime: State<'_, AppRuntime>) -> Vec<LogEntry> {
    runtime.server.all_logs()
}

#[tauri::command]
pub fn clear_server_logs(runtime: State<'_, AppRuntime>) {
    runtime.server.clear_logs();
}

fn handle_output(app: &AppHandle, runtime: &ServerRuntime, stream: &str, bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    for raw in text.lines() {
        let line = redact_server_line(raw.trim());
        if !line.is_empty() {
            emit_server_log(app, runtime, stream, line);
        }
    }
}

fn log_command_output(app: &AppHandle, runtime: &ServerRuntime, bytes: &[u8], stream: &str) {
    handle_output(app, runtime, stream, bytes);
}

fn command_error_detail(stderr: &[u8], stdout: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);
    stderr
        .lines()
        .chain(stdout.lines())
        .map(redact_server_line)
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| "frps 返回非零退出码".into())
}

fn redact_server_line(line: &str) -> String {
    let lower = line.to_lowercase();
    let sensitive = [
        "auth.token",
        "token =",
        "password =",
        "secret =",
        "privatekey",
        "clientsecret",
        "oidc.client.secret",
    ];
    if sensitive.iter().any(|needle| lower.contains(needle)) {
        if let Some((key, _)) = line.split_once('=') {
            return format!("{}= ******", key.trim());
        }
        return "[sensitive frps configuration line redacted]".into();
    }
    line.to_string()
}

#[cfg(test)]
mod tests {
    use super::redact_server_line;

    #[test]
    fn server_log_redaction_hides_auth_and_dashboard_secrets() {
        assert_eq!(
            redact_server_line("auth.token = \"not-for-logs\""),
            "auth.token= ******"
        );
        assert_eq!(
            redact_server_line("webServer.password = super-secret"),
            "webServer.password= ******"
        );
    }
}
