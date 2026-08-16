export interface ConnectionConfig {
  client_id: string;
  client_secret: string;
  api_url: string;
  rpc_url: string;
  auto_connect: boolean;
  launch_at_login: boolean;
  allow_insecure_tls: boolean;
}

export type ClientMode = "panel_managed" | "native_frpc";
export type NativeConfigSource = "managed" | "external_readonly";

export interface NativeFrpcConfig {
  config_path: string;
  source: NativeConfigSource;
  auto_connect: boolean;
  launch_at_login: boolean;
}

export interface Profile {
  id: string;
  name: string;
  mode: ClientMode;
  panel: ConnectionConfig | null;
  native: NativeFrpcConfig | null;
}

export interface ProfileSummary {
  id: string;
  name: string;
  mode: ClientMode;
  active: boolean;
  configured: boolean;
  config_path: string | null;
}

export type RuntimeState = "stopped" | "starting" | "running" | "error";

export interface RuntimeStatus {
  state: RuntimeState;
  state_label: RuntimeState;
  running: boolean;
  error: string | null;
  started_at_ms: number | null;
  sidecar_available: boolean;
  profile_id: string | null;
  mode: ClientMode | null;
  binary_name: string | null;
  config_path: string | null;
}

export interface LogEntry {
  stream: "stdout" | "stderr" | "system";
  line: string;
  ts_ms: number;
}

export interface SidecarInfo {
  available: boolean;
  target_triple: string;
  expected_name: string;
  hint: string;
  native_available: boolean;
  native_target_triple: string;
  native_expected_name: string;
}

export interface ExternalBinaryInfo {
  name: string;
  path: string;
  source: string;
}

export interface ObservedClientInfo {
  pid: number;
  binary_name: string;
  binary_path: string | null;
  client_id: string | null;
  api_url: string | null;
  rpc_url: string | null;
  started_at_epoch_seconds: number | null;
  run_time_seconds: number | null;
  secret_argument_present: boolean;
}

export interface StartupItemInfo {
  label: string;
  path: string;
  kind: string;
  client_id: string | null;
  api_url: string | null;
  rpc_url: string | null;
  secret_argument_present: boolean;
}

export interface ExternalClientDiscovery {
  installed_binaries: ExternalBinaryInfo[];
  running_clients: ObservedClientInfo[];
  startup_items: StartupItemInfo[];
  native_installed_binaries: ExternalBinaryInfo[];
  native_running_clients: ObservedNativeFrpcInfo[];
  native_startup_items: NativeStartupItemInfo[];
}

export interface ObservedNativeFrpcInfo {
  pid: number;
  binary_name: string;
  binary_path: string | null;
  config_path: string | null;
  started_at_epoch_seconds: number | null;
  run_time_seconds: number | null;
}

export interface NativeStartupItemInfo {
  label: string;
  path: string;
  kind: string;
  binary_path: string | null;
  config_path: string | null;
}
