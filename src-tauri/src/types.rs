use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionConfig {
    pub client_id: String,
    pub client_secret: String,
    pub api_url: String,
    pub rpc_url: String,
    pub auto_connect: bool,
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default)]
    pub allow_insecure_tls: bool,
}

impl ConnectionConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.client_id.trim().is_empty() {
            return Err("Client ID 不能为空".into());
        }
        if self.client_secret.trim().is_empty() {
            return Err("Client Secret 不能为空".into());
        }
        if !is_http_url(&self.api_url) {
            return Err("API URL 必须以 http:// 或 https:// 开头".into());
        }
        if !is_rpc_url(&self.rpc_url) {
            return Err("RPC URL 必须以 grpc://、ws:// 或 wss:// 开头".into());
        }
        Ok(())
    }
}

fn is_http_url(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("http://") || value.starts_with("https://")
}

fn is_rpc_url(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("grpc://") || value.starts_with("ws://") || value.starts_with("wss://")
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeState {
    Stopped,
    Starting,
    Running,
    Error,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self::Stopped
    }
}

impl RuntimeState {
    pub fn as_str(self) -> &'static str {
        match self {
            RuntimeState::Stopped => "stopped",
            RuntimeState::Starting => "starting",
            RuntimeState::Running => "running",
            RuntimeState::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatus {
    pub state: RuntimeState,
    pub state_label: String,
    pub running: bool,
    pub error: Option<String>,
    pub started_at_ms: Option<u128>,
    pub sidecar_available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub stream: String,
    pub line: String,
    pub ts_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct SidecarInfo {
    pub available: bool,
    pub target_triple: String,
    pub expected_name: String,
    pub hint: String,
}
