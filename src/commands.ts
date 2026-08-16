import { invoke } from "@tauri-apps/api/core";

import type {
  ConnectionConfig,
  ExternalClientDiscovery,
  LogEntry,
  NativeConfigSource,
  Profile,
  ProfileSummary,
  RuntimeStatus,
  ServerProfile,
  ServerProfileSummary,
  ServerRuntimeStatus,
  SidecarInfo,
} from "./types";

export const emptyConfig = (): ConnectionConfig => ({
  client_id: "",
  client_secret: "",
  api_url: "",
  rpc_url: "",
  auto_connect: false,
  launch_at_login: false,
  allow_insecure_tls: false,
});

export function loadConnection() {
  return invoke<ConnectionConfig | null>("load_connection");
}

export function listProfiles() {
  return invoke<ProfileSummary[]>("list_profiles");
}

export function loadProfile(profileId?: string) {
  return invoke<Profile | null>("load_profile", { profileId });
}

export function selectProfile(profileId: string) {
  return invoke<void>("select_profile", { profileId });
}

export function saveNativeProfile(options: {
  profileId?: string;
  name: string;
  configPath: string;
  source: NativeConfigSource;
  autoConnect: boolean;
  launchAtLogin: boolean;
  importedContent?: string;
}) {
  return invoke<Profile>("save_native_profile", options);
}

export function loadManagedNativeConfig(profileId: string) {
  return invoke<string>("load_managed_native_config", { profileId });
}

export function deleteProfile(profileId: string) {
  return invoke<void>("delete_profile", { profileId });
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

export function startNativeProfile(profileId?: string) {
  return invoke<void>("start_native_profile", { profileId });
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

export function getExternalClientDiscovery() {
  return invoke<ExternalClientDiscovery>("get_external_client_discovery");
}

export function listServerProfiles() {
  return invoke<ServerProfileSummary[]>("list_server_profiles");
}

export function loadServerProfile(profileId?: string) {
  return invoke<ServerProfile | null>("load_server_profile", { profileId });
}

export function selectServerProfile(profileId: string) {
  return invoke<void>("select_server_profile", { profileId });
}

export function saveServerProfile(options: {
  profileId?: string;
  name: string;
  configPath: string;
  source: NativeConfigSource;
  autoStart: boolean;
  importedContent?: string;
}) {
  return invoke<ServerProfile>("save_server_profile", options);
}

export function loadManagedServerConfig(profileId: string) {
  return invoke<string>("load_managed_server_config", { profileId });
}

export function deleteServerProfile(profileId: string) {
  return invoke<void>("delete_server_profile", { profileId });
}

export function startServerProfile(profileId?: string) {
  return invoke<void>("start_server_profile", { profileId });
}

export function stopServer() {
  return invoke<void>("stop_server");
}

export function getServerStatus() {
  return invoke<ServerRuntimeStatus>("get_server_status");
}

export function getServerLogs() {
  return invoke<LogEntry[]>("get_server_logs");
}

export function clearServerLogs() {
  return invoke<void>("clear_server_logs");
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return "操作失败";
}
