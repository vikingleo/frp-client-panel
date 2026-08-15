import { invoke } from "@tauri-apps/api/core";

import type { ConnectionConfig, LogEntry, RuntimeStatus, SidecarInfo } from "./types";

export const emptyConfig = (): ConnectionConfig => ({
  client_id: "",
  client_secret: "",
  api_url: "",
  rpc_url: "",
  auto_connect: false,
  launch_at_login: false,
});

export function loadConnection() {
  return invoke<ConnectionConfig | null>("load_connection");
}

export function saveConnection(config: ConnectionConfig) {
  return invoke<void>("save_connection", { config });
}

export function parsePanelCommand(command: string) {
  return invoke<ConnectionConfig>("parse_panel_command", { command });
}

export function startClient() {
  return invoke<void>("start_client");
}

export function stopClient() {
  return invoke<void>("stop_client");
}

export function getStatus() {
  return invoke<RuntimeStatus>("get_status");
}

export function getLogs() {
  return invoke<LogEntry[]>("get_logs");
}

export function clearLogs() {
  return invoke<void>("clear_logs");
}

export function getSidecarInfo() {
  return invoke<SidecarInfo>("get_sidecar_info");
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return "操作失败";
}
