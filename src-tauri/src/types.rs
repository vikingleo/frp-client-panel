use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientMode {
    PanelManaged,
    NativeFrpc,
}

impl Default for ClientMode {
    fn default() -> Self {
        Self::PanelManaged
    }
}

impl ClientMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::PanelManaged => "frp-panel 受管",
            Self::NativeFrpc => "原生 frpc",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NativeConfigSource {
    Managed,
    ExternalReadonly,
}

impl Default for NativeConfigSource {
    fn default() -> Self {
        Self::Managed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeFrpcConfig {
    pub config_path: String,
    #[serde(default)]
    pub source: NativeConfigSource,
    #[serde(default)]
    pub auto_connect: bool,
    #[serde(default)]
    pub launch_at_login: bool,
}

impl NativeFrpcConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.config_path.trim().is_empty() {
            return Err("原生 frpc 配置文件路径不能为空".into());
        }
        Ok(())
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub mode: ClientMode,
    #[serde(default)]
    pub panel: Option<ConnectionConfig>,
    #[serde(default)]
    pub native: Option<NativeFrpcConfig>,
}

impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("Profile ID 不能为空".into());
        }
        if self.name.trim().is_empty() {
            return Err("Profile 名称不能为空".into());
        }
        match self.mode {
            ClientMode::PanelManaged => self
                .panel
                .as_ref()
                .ok_or_else(|| "frp-panel Profile 缺少连接配置".to_string())?
                .validate(),
            ClientMode::NativeFrpc => self
                .native
                .as_ref()
                .ok_or_else(|| "原生 frpc Profile 缺少配置文件".to_string())?
                .validate(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub mode: ClientMode,
    pub active: bool,
    pub configured: bool,
    pub config_path: Option<String>,
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
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub mode: Option<ClientMode>,
    #[serde(default)]
    pub binary_name: Option<String>,
    #[serde(default)]
    pub config_path: Option<String>,
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
    #[serde(default)]
    pub native_available: bool,
    #[serde(default)]
    pub native_target_triple: String,
    #[serde(default)]
    pub native_expected_name: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ExternalClientDiscovery {
    pub installed_binaries: Vec<ExternalBinaryInfo>,
    pub running_clients: Vec<ObservedClientInfo>,
    pub startup_items: Vec<StartupItemInfo>,
    #[serde(default)]
    pub native_installed_binaries: Vec<ExternalBinaryInfo>,
    #[serde(default)]
    pub native_running_clients: Vec<ObservedNativeFrpcInfo>,
    #[serde(default)]
    pub native_startup_items: Vec<NativeStartupItemInfo>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExternalBinaryInfo {
    pub name: String,
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ObservedClientInfo {
    pub pid: u32,
    pub binary_name: String,
    pub binary_path: Option<String>,
    pub client_id: Option<String>,
    pub api_url: Option<String>,
    pub rpc_url: Option<String>,
    pub started_at_epoch_seconds: Option<u64>,
    pub run_time_seconds: Option<u64>,
    pub secret_argument_present: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StartupItemInfo {
    pub label: String,
    pub path: String,
    pub kind: String,
    pub client_id: Option<String>,
    pub api_url: Option<String>,
    pub rpc_url: Option<String>,
    pub secret_argument_present: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ObservedNativeFrpcInfo {
    pub pid: u32,
    pub binary_name: String,
    pub binary_path: Option<String>,
    pub config_path: Option<String>,
    pub started_at_epoch_seconds: Option<u64>,
    pub run_time_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NativeStartupItemInfo {
    pub label: String,
    pub path: String,
    pub kind: String,
    pub binary_path: Option<String>,
    pub config_path: Option<String>,
}
