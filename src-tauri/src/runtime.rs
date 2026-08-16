use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tauri::Emitter;
use tauri_plugin_shell::process::CommandChild;

use crate::types::{ClientMode, LogEntry, RuntimeState, RuntimeStatus, ServerRuntimeStatus};

pub const LOG_LIMIT: usize = 800;

pub struct AppRuntime {
    pub child: Mutex<Option<CommandChild>>,
    managed_child_pid: Mutex<Option<u32>>,
    pub state: Mutex<RuntimeState>,
    pub error: Mutex<Option<String>>,
    pub started_at: Mutex<Option<Instant>>,
    pub logs: Mutex<VecDeque<LogEntry>>,
    pub generation: AtomicU64,
    pub profile_id: Mutex<Option<String>>,
    pub mode: Mutex<Option<ClientMode>>,
    pub binary_name: Mutex<Option<String>>,
    pub config_path: Mutex<Option<String>>,
    pub server: ServerRuntime,
}

pub struct ServerRuntime {
    pub child: Mutex<Option<CommandChild>>,
    pub managed_child_pid: Mutex<Option<u32>>,
    pub state: Mutex<RuntimeState>,
    pub error: Mutex<Option<String>>,
    pub started_at: Mutex<Option<Instant>>,
    pub logs: Mutex<VecDeque<LogEntry>>,
    pub generation: AtomicU64,
    pub profile_id: Mutex<Option<String>>,
    pub binary_name: Mutex<Option<String>>,
    pub config_path: Mutex<Option<String>>,
}

impl Default for ServerRuntime {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
            managed_child_pid: Mutex::new(None),
            state: Mutex::new(RuntimeState::Stopped),
            error: Mutex::new(None),
            started_at: Mutex::new(None),
            logs: Mutex::new(VecDeque::new()),
            generation: AtomicU64::new(0),
            profile_id: Mutex::new(None),
            binary_name: Mutex::new(None),
            config_path: Mutex::new(None),
        }
    }
}

impl ServerRuntime {
    pub fn set_managed_child_pid(&self, pid: u32) {
        if let Ok(mut guard) = self.managed_child_pid.lock() {
            *guard = Some(pid);
        }
    }

    pub fn clear_managed_child_pid(&self) {
        if let Ok(mut guard) = self.managed_child_pid.lock() {
            *guard = None;
        }
    }

    pub fn status(&self, sidecar_available: bool) -> ServerRuntimeStatus {
        let state = self.state.lock().map(|g| *g).unwrap_or(RuntimeState::Error);
        let running = self.child.lock().map(|g| g.is_some()).unwrap_or(false);
        let error = self.error.lock().ok().and_then(|g| g.clone());
        let started_at_ms = self
            .started_at
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|i| i.elapsed().as_millis()));
        ServerRuntimeStatus {
            state,
            state_label: state.as_str().to_string(),
            running,
            error,
            started_at_ms,
            sidecar_available,
            profile_id: self.profile_id.lock().ok().and_then(|g| g.clone()),
            binary_name: self.binary_name.lock().ok().and_then(|g| g.clone()),
            config_path: self.config_path.lock().ok().and_then(|g| g.clone()),
        }
    }

    pub fn set_runtime_context(
        &self,
        profile_id: impl Into<String>,
        binary_name: impl Into<String>,
        config_path: Option<String>,
    ) {
        if let Ok(mut guard) = self.profile_id.lock() {
            *guard = Some(profile_id.into());
        }
        if let Ok(mut guard) = self.binary_name.lock() {
            *guard = Some(binary_name.into());
        }
        if let Ok(mut guard) = self.config_path.lock() {
            *guard = config_path;
        }
    }

    pub fn set_state(&self, state: RuntimeState, error: Option<String>) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = state;
        }
        if let Ok(mut guard) = self.error.lock() {
            *guard = error;
        }
    }

    pub fn mark_starting(&self) -> u64 {
        self.set_state(RuntimeState::Starting, None);
        if let Ok(mut guard) = self.started_at.lock() {
            *guard = Some(Instant::now());
        }
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn mark_stopped(&self) -> u64 {
        self.set_state(RuntimeState::Stopped, None);
        if let Ok(mut guard) = self.started_at.lock() {
            *guard = None;
        }
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn is_current_generation(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) == generation
    }

    pub fn push_log(&self, stream: &str, line: String) -> LogEntry {
        let entry = LogEntry {
            stream: stream.to_string(),
            line,
            ts_ms: now_ms(),
        };
        if let Ok(mut logs) = self.logs.lock() {
            logs.push_back(entry.clone());
            while logs.len() > LOG_LIMIT {
                logs.pop_front();
            }
        }
        entry
    }

    pub fn all_logs(&self) -> Vec<LogEntry> {
        self.logs
            .lock()
            .map(|logs| logs.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear_logs(&self) {
        if let Ok(mut logs) = self.logs.lock() {
            logs.clear();
        }
    }
}

impl Default for AppRuntime {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
            managed_child_pid: Mutex::new(None),
            state: Mutex::new(RuntimeState::Stopped),
            error: Mutex::new(None),
            started_at: Mutex::new(None),
            logs: Mutex::new(VecDeque::new()),
            generation: AtomicU64::new(0),
            profile_id: Mutex::new(None),
            mode: Mutex::new(None),
            binary_name: Mutex::new(None),
            config_path: Mutex::new(None),
            server: ServerRuntime::default(),
        }
    }
}

impl AppRuntime {
    pub fn managed_child_pid(&self) -> Option<u32> {
        self.managed_child_pid.lock().ok().and_then(|pid| *pid)
    }

    pub fn set_managed_child_pid(&self, pid: u32) {
        if let Ok(mut guard) = self.managed_child_pid.lock() {
            *guard = Some(pid);
        }
    }

    pub fn clear_managed_child_pid(&self) {
        if let Ok(mut guard) = self.managed_child_pid.lock() {
            *guard = None;
        }
    }

    pub fn status(&self, sidecar_available: bool) -> RuntimeStatus {
        let state = self.state.lock().map(|g| *g).unwrap_or(RuntimeState::Error);
        let running = self.child.lock().map(|g| g.is_some()).unwrap_or(false);
        let error = self.error.lock().ok().and_then(|g| g.clone());
        let started_at_ms = self
            .started_at
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|i| i.elapsed().as_millis()));
        RuntimeStatus {
            state,
            state_label: state.as_str().to_string(),
            running,
            error,
            started_at_ms,
            sidecar_available,
            profile_id: self.profile_id.lock().ok().and_then(|g| g.clone()),
            mode: self.mode.lock().ok().and_then(|g| *g),
            binary_name: self.binary_name.lock().ok().and_then(|g| g.clone()),
            config_path: self.config_path.lock().ok().and_then(|g| g.clone()),
        }
    }

    pub fn set_runtime_context(
        &self,
        profile_id: impl Into<String>,
        mode: ClientMode,
        binary_name: impl Into<String>,
        config_path: Option<String>,
    ) {
        if let Ok(mut guard) = self.profile_id.lock() {
            *guard = Some(profile_id.into());
        }
        if let Ok(mut guard) = self.mode.lock() {
            *guard = Some(mode);
        }
        if let Ok(mut guard) = self.binary_name.lock() {
            *guard = Some(binary_name.into());
        }
        if let Ok(mut guard) = self.config_path.lock() {
            *guard = config_path;
        }
    }

    pub fn set_state(&self, state: RuntimeState, error: Option<String>) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = state;
        }
        if let Ok(mut guard) = self.error.lock() {
            *guard = error;
        }
    }

    pub fn mark_starting(&self) -> u64 {
        self.set_state(RuntimeState::Starting, None);
        if let Ok(mut guard) = self.started_at.lock() {
            *guard = Some(Instant::now());
        }
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn mark_stopped(&self) -> u64 {
        self.set_state(RuntimeState::Stopped, None);
        if let Ok(mut guard) = self.started_at.lock() {
            *guard = None;
        }
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn is_current_generation(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) == generation
    }

    pub fn push_log(&self, stream: &str, line: String) -> LogEntry {
        let entry = LogEntry {
            stream: stream.to_string(),
            line,
            ts_ms: now_ms(),
        };
        if let Ok(mut logs) = self.logs.lock() {
            logs.push_back(entry.clone());
            while logs.len() > LOG_LIMIT {
                logs.pop_front();
            }
        }
        entry
    }

    pub fn all_logs(&self) -> Vec<LogEntry> {
        self.logs
            .lock()
            .map(|logs| logs.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear_logs(&self) {
        if let Ok(mut logs) = self.logs.lock() {
            logs.clear();
        }
    }
}

pub fn emit_log(app: &tauri::AppHandle, runtime: &AppRuntime, stream: &str, line: String) {
    let entry = runtime.push_log(stream, line);
    let _ = app.emit("client://log", entry);
}

pub fn emit_status(app: &tauri::AppHandle, runtime: &AppRuntime, sidecar_available: bool) {
    let _ = app.emit("client://status", runtime.status(sidecar_available));
}

pub fn emit_server_log(
    app: &tauri::AppHandle,
    runtime: &ServerRuntime,
    stream: &str,
    line: String,
) {
    let entry = runtime.push_log(stream, line);
    let _ = app.emit("server://log", entry);
}

pub fn emit_server_status(
    app: &tauri::AppHandle,
    runtime: &ServerRuntime,
    sidecar_available: bool,
) {
    let _ = app.emit("server://status", runtime.status(sidecar_available));
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}
