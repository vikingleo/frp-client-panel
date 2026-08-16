export interface ConnectionConfig {
  client_id: string;
  client_secret: string;
  api_url: string;
  rpc_url: string;
  auto_connect: boolean;
  launch_at_login: boolean;
  allow_insecure_tls: boolean;
}

export type RuntimeState = "stopped" | "starting" | "running" | "error";

export interface RuntimeStatus {
  state: RuntimeState;
  state_label: RuntimeState;
  running: boolean;
  error: string | null;
  started_at_ms: number | null;
  sidecar_available: boolean;
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
}
