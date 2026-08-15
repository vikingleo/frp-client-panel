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
