<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";

import {
  clearLogs,
  deleteProfile,
  emptyConfig,
  errorMessage,
  getExternalClientDiscovery,
  getLogs,
  getSidecarInfo,
  getStatus,
  listProfiles,
  loadManagedNativeConfig,
  loadProfile,
  parsePanelCommand,
  saveConnection,
  saveNativeProfile,
  selectProfile,
  startClient,
  startNativeProfile,
  stopClient,
} from "./commands";
import type {
  ClientMode,
  ConnectionConfig,
  ExternalClientDiscovery,
  LogEntry,
  NativeConfigSource,
  NativeFrpcConfig,
  Profile,
  ProfileSummary,
  RuntimeState,
  RuntimeStatus,
  SidecarInfo,
} from "./types";

type ViewName = "overview" | "config" | "logs" | "about";
type BusyAction = "loading" | "parsing" | "saving" | "starting" | "stopping" | null;
type NoticeKind = "success" | "error" | "info";
type ThemePreference = "system" | "light" | "dark";
type DisplayState = RuntimeState | "external";

const emptyNativeConfig = (): NativeFrpcConfig => ({
  config_path: "",
  source: "managed",
  auto_connect: false,
  launch_at_login: false,
});

const emptyDiscovery = (): ExternalClientDiscovery => ({
  installed_binaries: [],
  running_clients: [],
  startup_items: [],
  native_installed_binaries: [],
  native_running_clients: [],
  native_startup_items: [],
});

const emptyStatus = (): RuntimeStatus => ({
  state: "stopped",
  state_label: "stopped",
  running: false,
  error: null,
  started_at_ms: null,
  sidecar_available: false,
  profile_id: null,
  mode: null,
  binary_name: null,
  config_path: null,
});

const activeView = ref<ViewName>("overview");
const profiles = ref<ProfileSummary[]>([]);
const activeProfile = ref<Profile | null>(null);
const config = ref<ConnectionConfig>(emptyConfig());
const nativeConfig = ref<NativeFrpcConfig>(emptyNativeConfig());
const nativeProfileName = ref("我的原生 frpc");
const nativeToml = ref("");
const nativeImportName = ref("");
const commandInput = ref("");
const logs = ref<LogEntry[]>([]);
const logFilter = ref<"all" | "stdout" | "stderr" | "system">("all");
const busyAction = ref<BusyAction>(null);
const notice = ref<{ kind: NoticeKind; message: string } | null>(null);
const sidecar = ref<SidecarInfo | null>(null);
const externalDiscovery = ref<ExternalClientDiscovery>(emptyDiscovery());
const status = ref<RuntimeStatus>(emptyStatus());
const hydrated = ref(false);
const logViewport = ref<HTMLElement | null>(null);
const unlisteners: UnlistenFn[] = [];
const themePreference = ref<ThemePreference>("system");
const systemPrefersDark = ref(false);
const quickProxy = ref({
  serverAddr: "",
  serverPort: "7000",
  authToken: "",
  type: "tcp",
  name: "local-service",
  localIp: "127.0.0.1",
  localPort: "3000",
  remotePort: "6000",
  domains: "",
});

let systemThemeQuery: MediaQueryList | null = null;
let externalRefreshTimer: number | null = null;
const THEME_STORAGE_KEY = "frp-panel-client.theme";
const LEGACY_THEME_STORAGE_KEY = "frp-panel-mac-client.theme";

const themeOptions: Array<{ value: ThemePreference; label: string; icon: string }> = [
  { value: "light", label: "亮色", icon: "bi-sun" },
  { value: "dark", label: "暗色", icon: "bi-moon-stars" },
  { value: "system", label: "系统", icon: "bi-display" },
];

const navItems: Array<{ id: ViewName; label: string; icon: string }> = [
  { id: "overview", label: "总览", icon: "bi-grid-1x2" },
  { id: "config", label: "配置", icon: "bi-sliders2" },
  { id: "logs", label: "日志", icon: "bi-terminal" },
  { id: "about", label: "关于", icon: "bi-info-circle" },
];

const pageMeta: Record<ViewName, { title: string; eyebrow: string }> = {
  overview: { title: "连接总览", eyebrow: "PROFILE / RUNTIME" },
  config: { title: "Profile 配置", eyebrow: "PANEL + NATIVE FRPC" },
  logs: { title: "实时日志", eyebrow: "RUNTIME / STREAM OUTPUT" },
  about: { title: "应用与引擎", eyebrow: "SYSTEM / SIDECAR" },
};

const resolvedTheme = computed<"light" | "dark">(() =>
  themePreference.value === "system"
    ? systemPrefersDark.value
      ? "dark"
      : "light"
    : themePreference.value,
);
const currentPage = computed(() => pageMeta[activeView.value]);
const profileMode = computed<ClientMode>(() => activeProfile.value?.mode ?? "panel_managed");
const isNative = computed(() => profileMode.value === "native_frpc");
const isRunning = computed(() => status.value.running);
const panelReady = computed(() =>
  [config.value.client_id, config.value.client_secret, config.value.api_url, config.value.rpc_url].every((value) => value.trim()),
);
const nativeReady = computed(() => Boolean(activeProfile.value?.id && nativeConfig.value.config_path.trim()));
const activeReady = computed(() => (isNative.value ? nativeReady.value : panelReady.value));
const currentPanelExternal = computed(() => {
  const clientId = config.value.client_id.trim();
  return clientId
    ? externalDiscovery.value.running_clients.find((client) => client.client_id === clientId) ?? null
    : null;
});
const currentNativeExternal = computed(() => {
  const path = nativeConfig.value.config_path.trim();
  return path
    ? externalDiscovery.value.native_running_clients.find((client) => client.config_path === path) ?? null
    : null;
});
const monitoredExternal = computed(() =>
  isNative.value
    ? currentNativeExternal.value ?? externalDiscovery.value.native_running_clients[0] ?? null
    : currentPanelExternal.value ?? externalDiscovery.value.running_clients[0] ?? null,
);
const hasExternalConflict = computed(() => (isNative.value ? Boolean(currentNativeExternal.value) : Boolean(currentPanelExternal.value)));
const displayState = computed<DisplayState>(() => (!status.value.running && monitoredExternal.value ? "external" : status.value.state));
const currentState = computed(() => {
  const native = isNative.value;
  const entries: Record<DisplayState, { label: string; description: string; tone: string }> = {
    stopped: {
      label: native ? "frpc 未运行" : "未连接",
      description: native ? "尚未启动 App 托管的原生 frpc" : "尚未启动 frp-panel Client",
      tone: "neutral",
    },
    starting: { label: native ? "正在启动 frpc" : "正在连接", description: "正在准备运行引擎", tone: "warning" },
    running: {
      label: native ? "frpc 运行中" : "已连接",
      description: native ? "App 正在管理官方 frpc 进程" : "frp-panel-client 正在运行",
      tone: "success",
    },
    error: { label: "运行错误", description: "引擎停止或返回了错误", tone: "danger" },
    external: { label: "外部运行", description: "命令行实例正在运行，本应用仅只读观测", tone: "info" },
  };
  return entries[displayState.value];
});
const engineAvailable = computed(() => (isNative.value ? Boolean(sidecar.value?.native_available) : Boolean(sidecar.value?.available)));
const activeProfileName = computed(() => activeProfile.value?.name || (isNative.value ? nativeProfileName.value : "未命名 Profile"));
const connectionHint = computed(() => {
  if (isNative.value) return nativeConfig.value.config_path || "尚未导入 frpc 配置";
  return panelReady.value ? `${config.value.client_id} · ${config.value.api_url}` : "尚未保存连接参数";
});
const uptimeLabel = computed(() => {
  if (status.value.running && status.value.started_at_ms != null) return formatDuration(Number(status.value.started_at_ms));
  return displayState.value === "external" ? formatSeconds(monitoredExternal.value?.run_time_seconds ?? null) : "—";
});
const filteredLogs = computed(() => (logFilter.value === "all" ? logs.value : logs.value.filter((entry) => entry.stream === logFilter.value)));
const recentLogs = computed(() => logs.value.slice(-6).reverse());
const lastError = computed(() => status.value.error || [...logs.value].reverse().find((entry) => entry.stream === "stderr")?.line || "");
const logText = computed(() => logs.value.map((entry) => `[${formatTime(entry.ts_ms)}] ${entry.stream.toUpperCase()}  ${entry.line}`).join("\n"));

function setNotice(kind: NoticeKind, message: string) {
  notice.value = { kind, message };
}

function clearNotice() {
  notice.value = null;
}

function navigate(view: ViewName) {
  activeView.value = view;
  clearNotice();
}

function formatTime(timestamp: number) {
  const date = new Date(Number(timestamp));
  return Number.isNaN(date.getTime()) ? "--:--:--" : date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
}

function formatDuration(milliseconds: number) {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours) return `${hours}h ${String(minutes).padStart(2, "0")}m`;
  if (minutes) return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
  return `${seconds}s`;
}

function formatSeconds(seconds: number | null) {
  return seconds == null ? "—" : formatDuration(seconds * 1000);
}

function streamLabel(stream: LogEntry["stream"]) {
  return stream === "stderr" ? "错误" : stream === "system" ? "系统" : "输出";
}

function modeLabel(mode: ClientMode) {
  return mode === "native_frpc" ? "原生 frpc" : "frp-panel 受管";
}

async function applyProfile(profile: Profile | null) {
  activeProfile.value = profile;
  if (!profile) {
    config.value = emptyConfig();
    nativeConfig.value = emptyNativeConfig();
    nativeToml.value = "";
    return;
  }
  if (profile.mode === "panel_managed") {
    config.value = profile.panel ?? emptyConfig();
    nativeToml.value = "";
    return;
  }
  nativeProfileName.value = profile.name;
  nativeConfig.value = profile.native ?? emptyNativeConfig();
  if (profile.id && profile.native?.source === "managed") {
    try {
      nativeToml.value = await loadManagedNativeConfig(profile.id);
    } catch (error) {
      nativeToml.value = "";
      setNotice("error", errorMessage(error));
    }
  } else {
    nativeToml.value = "";
  }
}

async function refreshProfileState(profileId?: string) {
  const [nextProfiles, profile] = await Promise.all([listProfiles(), loadProfile(profileId)]);
  profiles.value = nextProfiles;
  await applyProfile(profile);
}

async function refreshRuntime() {
  const [nextStatus, nextLogs, nextSidecar, nextDiscovery] = await Promise.all([
    getStatus(),
    getLogs(),
    getSidecarInfo(),
    getExternalClientDiscovery(),
  ]);
  status.value = nextStatus;
  logs.value = nextLogs;
  sidecar.value = nextSidecar;
  externalDiscovery.value = nextDiscovery;
}

async function hydrate() {
  busyAction.value = "loading";
  try {
    await Promise.all([refreshProfileState(), refreshRuntime()]);
    const launchEnabled = await readLaunchAtLoginState();
    if (launchEnabled !== null) {
      if (isNative.value) nativeConfig.value.launch_at_login = launchEnabled;
      else config.value.launch_at_login = launchEnabled;
    }
    hydrated.value = true;
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busyAction.value = null;
  }
}

async function readLaunchAtLoginState(): Promise<boolean | null> {
  try {
    return await isAutostartEnabled();
  } catch {
    return null;
  }
}

async function chooseProfile(profileId: string) {
  if (!profileId) return;
  try {
    await selectProfile(profileId);
    await refreshProfileState(profileId);
    clearNotice();
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
}

function createNativeProfile() {
  activeProfile.value = { id: "", name: "我的原生 frpc", mode: "native_frpc", panel: null, native: emptyNativeConfig() };
  nativeProfileName.value = "我的原生 frpc";
  nativeConfig.value = emptyNativeConfig();
  nativeToml.value = "";
  nativeImportName.value = "";
  navigate("config");
}

function createPanelProfile() {
  activeProfile.value = { id: "panel-default", name: "frp-panel Client", mode: "panel_managed", panel: emptyConfig(), native: null };
  config.value = emptyConfig();
  navigate("config");
}

async function parseCommand() {
  if (!commandInput.value.trim()) return setNotice("error", "请先粘贴 frp-panel Client 连接命令");
  busyAction.value = "parsing";
  try {
    const parsed = await parsePanelCommand(commandInput.value);
    config.value = { ...parsed, auto_connect: config.value.auto_connect, launch_at_login: config.value.launch_at_login, allow_insecure_tls: config.value.allow_insecure_tls };
    setNotice("success", "已解析连接参数；应用不会执行粘贴的安装脚本");
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busyAction.value = null;
  }
}

function importPanelExternal(client: { client_id: string | null; api_url: string | null; rpc_url: string | null; secret_argument_present: boolean }) {
  config.value = {
    ...config.value,
    client_id: client.client_id ?? config.value.client_id,
    api_url: client.api_url ?? config.value.api_url,
    rpc_url: client.rpc_url ?? config.value.rpc_url,
  };
  activeProfile.value = { id: "panel-default", name: "frp-panel Client", mode: "panel_managed", panel: config.value, native: null };
  navigate("config");
  setNotice("info", client.secret_argument_present ? "已填入外部进程的非敏感字段；Secret 不会被读取，请手动填写。" : "已填入外部进程可安全读取的字段。");
}

async function savePanel(showNotice = true) {
  await saveConnection(config.value);
  await selectProfile("panel-default");
  await refreshProfileState("panel-default");
  if (showNotice) setNotice("success", "frp-panel Profile 已保存");
}

async function saveNative(showNotice = true) {
  const saved = await saveNativeProfile({
    profileId: activeProfile.value?.mode === "native_frpc" ? activeProfile.value.id || undefined : undefined,
    name: nativeProfileName.value,
    configPath: nativeConfig.value.config_path,
    source: nativeConfig.value.source,
    autoConnect: nativeConfig.value.auto_connect,
    launchAtLogin: nativeConfig.value.launch_at_login,
    importedContent: nativeConfig.value.source === "managed" ? nativeToml.value : undefined,
  });
  await refreshProfileState(saved.id);
  if (showNotice) setNotice("success", nativeConfig.value.source === "managed" ? "托管 frpc TOML 已保存" : "外部只读 frpc Profile 已保存");
}

async function saveActiveProfile() {
  busyAction.value = "saving";
  try {
    if (isNative.value) await saveNative();
    else await savePanel();
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busyAction.value = null;
  }
}

async function connectClient() {
  busyAction.value = "starting";
  clearNotice();
  try {
    await refreshRuntime();
    if (!engineAvailable.value) throw new Error(isNative.value ? "内置 frpc sidecar 不可用" : sidecar.value?.hint || "内置 Client 不可用");
    if (hasExternalConflict.value) throw new Error("检测到相同配置的外部进程正在运行；本应用不会重复接管或启动。");
    if (isNative.value) {
      await saveNative(false);
      await startNativeProfile(activeProfile.value?.id);
    } else {
      await savePanel(false);
      await startClient();
    }
    await refreshRuntime();
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busyAction.value = null;
  }
}

async function disconnectClient() {
  busyAction.value = "stopping";
  try {
    await stopClient();
    await refreshRuntime();
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busyAction.value = null;
  }
}

async function deleteActiveProfile() {
  if (!activeProfile.value?.id || profiles.value.length <= 1) return;
  try {
    await deleteProfile(activeProfile.value.id);
    await refreshProfileState();
    setNotice("success", "Profile 已删除；托管 TOML 文件保留在本地以避免意外数据丢失");
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
}

async function toggleLaunchAtLogin() {
  const enabled = isNative.value ? nativeConfig.value.launch_at_login : config.value.launch_at_login;
  try {
    if (enabled) await enableAutostart();
    else await disableAutostart();
    if (isNative.value) await saveNative(false);
    else await savePanel(false);
  } catch (error) {
    if (isNative.value) nativeConfig.value.launch_at_login = !enabled;
    else config.value.launch_at_login = !enabled;
    setNotice("error", `无法更新登录启动：${errorMessage(error)}`);
  }
}

async function handleNativeFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  if (file.size > 1024 * 1024) return setNotice("error", "配置文件超过 1 MiB，拒绝导入");
  try {
    nativeToml.value = await file.text();
    nativeImportName.value = file.name;
    if (!nativeProfileName.value || nativeProfileName.value === "我的原生 frpc") nativeProfileName.value = file.name.replace(/\.(toml|ini|ya?ml|json)$/i, "");
    setNotice("success", "配置已读入编辑器；保存后会创建 App 私有副本，不会改写原文件");
  } catch (error) {
    setNotice("error", `读取配置失败：${errorMessage(error)}`);
  } finally {
    input.value = "";
  }
}

function generateToml() {
  const q = quickProxy.value;
  const string = (value: string) => value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  const port = Number(q.serverPort);
  const localPort = Number(q.localPort);
  const remotePort = Number(q.remotePort);
  if (!q.serverAddr.trim() || !Number.isInteger(port) || port < 1 || port > 65535) return setNotice("error", "请填写有效的服务端地址与端口");
  if (!q.name.trim() || !Number.isInteger(localPort) || localPort < 1 || localPort > 65535) return setNotice("error", "请填写有效的代理名称与本地端口");
  const lines = [`serverAddr = "${string(q.serverAddr.trim())}"`, `serverPort = ${port}`, ""];
  if (q.authToken.trim()) lines.push(`auth.token = "${string(q.authToken)}"`, "");
  lines.push("[[proxies]]", `name = "${string(q.name.trim())}"`, `type = "${q.type}"`, `localIP = "${string(q.localIp.trim() || "127.0.0.1")}"`, `localPort = ${localPort}`);
  if (q.type === "tcp" || q.type === "udp") {
    if (!Number.isInteger(remotePort) || remotePort < 1 || remotePort > 65535) return setNotice("error", "TCP / UDP 需要有效的远程端口");
    lines.push(`remotePort = ${remotePort}`);
  } else {
    const domains = q.domains.split(",").map((value) => value.trim()).filter(Boolean);
    if (!domains.length) return setNotice("error", "HTTP / HTTPS 需要至少一个自定义域名");
    lines.push(`customDomains = [${domains.map((domain) => `"${string(domain)}"`).join(", ")}]`);
  }
  nativeConfig.value.source = "managed";
  nativeToml.value = `${lines.join("\n")}\n`;
  nativeImportName.value = "由常用代理表单生成";
  setNotice("success", "已生成 TOML；保存并启动前会自动执行 frpc verify");
}

async function clearAllLogs() {
  try {
    await clearLogs();
    logs.value = [];
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
}

async function copyText(value: string, successMessage: string) {
  if (!value) return setNotice("info", "当前没有可复制的内容");
  try {
    await navigator.clipboard.writeText(value);
    setNotice("success", successMessage);
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
}

async function scrollLogsToEnd() {
  await nextTick();
  if (logViewport.value) logViewport.value.scrollTop = logViewport.value.scrollHeight;
}

function initializeTheme() {
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY) ?? window.localStorage.getItem(LEGACY_THEME_STORAGE_KEY);
    if (stored === "light" || stored === "dark" || stored === "system") themePreference.value = stored;
  } catch {
    // Theme persistence is optional.
  }
  systemThemeQuery = window.matchMedia("(prefers-color-scheme: dark)");
  systemPrefersDark.value = systemThemeQuery.matches;
  systemThemeQuery.addEventListener("change", (event) => (systemPrefersDark.value = event.matches));
}

async function initializeListeners() {
  unlisteners.push(
    await listen<RuntimeStatus>("client://status", (event) => (status.value = event.payload)),
    await listen<LogEntry>("client://log", async (event) => {
      logs.value = [...logs.value, event.payload].slice(-800);
      if (activeView.value === "logs") await scrollLogsToEnd();
    }),
  );
}

onMounted(async () => {
  initializeTheme();
  document.documentElement.dataset.theme = resolvedTheme.value;
  document.documentElement.style.colorScheme = resolvedTheme.value;
  try {
    await initializeListeners();
    await hydrate();
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
  externalRefreshTimer = window.setInterval(() => void refreshRuntime(), 10_000);
});

onBeforeUnmount(() => {
  unlisteners.splice(0).forEach((unlisten) => unlisten());
  if (externalRefreshTimer !== null) window.clearInterval(externalRefreshTimer);
});

watch(activeView, async (view) => {
  if (view === "logs") await scrollLogsToEnd();
});

watch(resolvedTheme, (theme) => {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
});

watch(themePreference, (preference) => {
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, preference);
    window.localStorage.removeItem(LEGACY_THEME_STORAGE_KEY);
  } catch {
    // Theme persistence is optional.
  }
});
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand-block">
        <div class="brand-mark" aria-hidden="true"><i class="bi bi-diagram-3-fill"></i></div>
        <div><div class="brand-name">FRP CLIENT</div><div class="brand-caption">PANEL + NATIVE</div></div>
      </div>

      <div class="sidebar-section-label">工作区</div>
      <nav class="navigation" aria-label="主导航">
        <button v-for="item in navItems" :key="item.id" type="button" class="nav-item" :class="{ active: activeView === item.id }" :aria-current="activeView === item.id ? 'page' : undefined" @click="navigate(item.id)">
          <i class="bi" :class="item.icon" aria-hidden="true"></i><span>{{ item.label }}</span><span v-if="item.id === 'logs' && logs.length" class="nav-count">{{ logs.length }}</span>
        </button>
      </nav>

      <div class="profile-sidebar">
        <div class="sidebar-section-label">当前 Profile</div>
        <select aria-label="切换 Profile" :value="activeProfile?.id ?? ''" @change="chooseProfile(($event.target as HTMLSelectElement).value)">
          <option value="" disabled>{{ profiles.length ? '选择 Profile' : '尚无 Profile' }}</option>
          <option v-for="profile in profiles" :key="profile.id" :value="profile.id">{{ profile.name }} · {{ modeLabel(profile.mode) }}</option>
        </select>
        <div class="profile-quick-actions"><button type="button" class="text-button" @click="createPanelProfile">+ 面板</button><button type="button" class="text-button" @click="createNativeProfile">+ frpc</button></div>
      </div>

      <div class="sidebar-spacer"></div>
      <div class="sidebar-runtime" :class="`tone-${currentState.tone}`"><div class="runtime-mini-heading"><span class="status-dot"></span><span>运行状态</span></div><div class="runtime-mini-value">{{ currentState.label }}</div><div class="runtime-mini-detail">{{ connectionHint }}</div></div>
      <div class="sidebar-footer"><span class="version-dot"></span><span>v0.1.0 · {{ isNative ? sidecar?.native_target_triple ?? 'frpc' : sidecar?.target_triple ?? 'desktop' }}</span></div>
    </aside>

    <main class="main-pane">
      <header class="topbar">
        <div class="breadcrumb"><span class="breadcrumb-muted">{{ activeProfileName }}</span><i class="bi bi-chevron-right"></i><span>{{ currentPage.title }}</span></div>
        <div class="topbar-actions">
          <div class="theme-switcher" role="group" aria-label="外观主题"><button v-for="option in themeOptions" :key="option.value" type="button" class="theme-button" :class="{ active: themePreference === option.value }" :aria-pressed="themePreference === option.value" @click="themePreference = option.value"><i class="bi" :class="option.icon"></i><span>{{ option.label }}</span></button></div>
          <div class="top-status" :class="`tone-${currentState.tone}`"><span class="status-dot"></span><span>{{ currentState.label }}</span></div>
        </div>
      </header>

      <div class="content-scroll">
        <div v-if="notice" class="notice" :class="`notice-${notice.kind}`" role="status"><i class="bi" :class="notice.kind === 'error' ? 'bi-exclamation-triangle-fill' : notice.kind === 'success' ? 'bi-check-circle-fill' : 'bi-info-circle-fill'"></i><span>{{ notice.message }}</span><button type="button" class="icon-button notice-close" aria-label="关闭提示" @click="clearNotice"><i class="bi bi-x"></i></button></div>

        <section v-if="activeView === 'overview'" class="view view-overview">
          <div class="page-heading"><div><div class="eyebrow">{{ currentPage.eyebrow }}</div><h1>{{ currentPage.title }}</h1><p class="page-subtitle">桌面壳只负责运行、状态与日志；实际协议由 frp-panel-client 或官方 frpc 承担</p></div><button v-if="!isRunning" type="button" class="button button-primary" :disabled="busyAction !== null || !activeReady || !engineAvailable || hasExternalConflict" @click="connectClient"><i class="bi" :class="busyAction === 'starting' ? 'bi-arrow-repeat spinning' : 'bi-play-fill'"></i>{{ busyAction === 'starting' ? '正在启动' : isNative ? '启动 frpc' : '连接客户端' }}</button><button v-else type="button" class="button button-danger" :disabled="busyAction !== null" @click="disconnectClient"><i class="bi bi-power"></i>停止托管进程</button></div>

          <div class="status-panel" :class="`state-${displayState}`"><div class="status-panel-main"><div class="status-icon"><i v-if="displayState === 'running'" class="bi bi-check-circle-fill"></i><i v-else-if="displayState === 'external'" class="bi bi-terminal-fill"></i><i v-else-if="displayState === 'error'" class="bi bi-exclamation-triangle-fill"></i><i v-else class="bi bi-arrow-repeat" :class="{ spinning: displayState === 'starting' }"></i></div><div><div class="status-label">{{ currentState.label }}</div><div class="status-description">{{ currentState.description }}</div></div></div><div class="status-panel-meta"><div><span class="meta-label">运行时长</span><strong>{{ uptimeLabel }}</strong></div><div><span class="meta-label">进程 / 日志</span><strong>{{ displayState === 'external' ? monitoredExternal?.pid ?? '—' : logs.length }}</strong></div></div></div>

          <div v-if="lastError" class="error-strip"><i class="bi bi-exclamation-triangle-fill"></i><div><span class="error-strip-label">最近错误</span><span class="error-strip-text">{{ lastError }}</span></div><button type="button" class="text-button" @click="navigate('logs')">查看日志</button></div>

          <div class="overview-grid"><section class="panel connection-panel"><div class="panel-heading"><div><div class="panel-kicker">ACTIVE PROFILE</div><h2>{{ activeProfileName }}</h2></div><span class="mode-badge" :class="isNative ? 'native' : 'panel'">{{ modeLabel(profileMode) }}</span></div><div v-if="isNative" class="kv-list"><div class="kv-row"><span>配置来源</span><code>{{ nativeConfig.source === 'managed' ? 'App 托管副本' : '外部只读' }}</code></div><div class="kv-row"><span>frpc.toml</span><code>{{ nativeConfig.config_path || '待导入' }}</code></div><div class="kv-row"><span>自动连接</span><code>{{ nativeConfig.auto_connect ? '已开启' : '关闭' }}</code></div></div><div v-else class="kv-list"><div class="kv-row"><span>Client ID</span><code>{{ config.client_id || '待填写' }}</code></div><div class="kv-row"><span>API URL</span><code>{{ config.api_url || '待填写' }}</code></div><div class="kv-row"><span>自动连接</span><code>{{ config.auto_connect ? '已开启' : '关闭' }}</code></div></div></section><section class="panel activity-panel"><div class="panel-heading"><div><div class="panel-kicker">LATEST OUTPUT</div><h2>最近输出</h2></div><button type="button" class="text-button" @click="navigate('logs')">全部日志</button></div><div v-if="recentLogs.length" class="recent-log-list"><div v-for="entry in recentLogs" :key="`${entry.ts_ms}-${entry.line}`" class="recent-log-line" :class="`stream-${entry.stream}`"><span class="log-time">{{ formatTime(entry.ts_ms) }}</span><span class="log-stream">{{ streamLabel(entry.stream) }}</span><span class="log-line-text">{{ entry.line }}</span></div></div><div v-else class="empty-block compact"><i class="bi bi-terminal"></i><span>暂无运行日志</span></div></section></div>

          <section class="panel external-clients-panel"><div class="panel-heading"><div><div class="panel-kicker">EXTERNAL PROCESS DISCOVERY</div><h2>系统已有实例（只读）</h2></div><button type="button" class="button button-secondary button-small" @click="refreshRuntime"><i class="bi bi-arrow-clockwise"></i>重新检测</button></div><div class="external-safety-note"><i class="bi bi-shield-check"></i><span>不会读取 Token、TLS 私钥或外部配置内容；不会停止、重载、接管任何外部进程或 LaunchAgent。</span></div><div class="external-grid"><div class="external-source-block"><div class="external-source-heading"><i class="bi bi-diagram-3"></i><strong>frp-panel Client</strong><span>{{ externalDiscovery.running_clients.length }}</span></div><p v-if="!externalDiscovery.running_clients.length">未检测到外部受管 Client。</p><div v-for="client in externalDiscovery.running_clients" :key="client.pid" class="external-source-row"><div><code>{{ client.binary_name }} · PID {{ client.pid }}</code><span>{{ client.client_id ?? '未能读取 Client ID' }} · {{ formatSeconds(client.run_time_seconds) }}</span></div><button type="button" class="text-button" @click="importPanelExternal(client)">填入安全字段</button></div></div><div class="external-source-block"><div class="external-source-heading"><i class="bi bi-terminal"></i><strong>原生 frpc</strong><span>{{ externalDiscovery.native_running_clients.length }}</span></div><p v-if="!externalDiscovery.native_running_clients.length">未检测到外部 frpc。</p><div v-for="client in externalDiscovery.native_running_clients" :key="client.pid" class="external-source-row"><div><code>{{ client.binary_name }} · PID {{ client.pid }}</code><span>{{ client.config_path ?? '未提供 -c / --config 路径' }} · {{ formatSeconds(client.run_time_seconds) }}</span></div></div></div></div><div class="external-grid minor"><div class="external-source-block"><div class="external-source-heading"><i class="bi bi-hdd-network"></i><strong>发现的 frpc 二进制</strong><span>{{ externalDiscovery.native_installed_binaries.length }}</span></div><p v-if="!externalDiscovery.native_installed_binaries.length">未在 PATH / 常用目录发现 frpc。</p><div v-for="binary in externalDiscovery.native_installed_binaries" :key="binary.path" class="external-source-row"><code>{{ binary.path }}</code></div></div><div class="external-source-block"><div class="external-source-heading"><i class="bi bi-rocket-takeoff"></i><strong>frpc LaunchAgent</strong><span>{{ externalDiscovery.native_startup_items.length }}</span></div><p v-if="!externalDiscovery.native_startup_items.length">未发现外部 frpc 启动项。</p><div v-for="item in externalDiscovery.native_startup_items" :key="item.path" class="external-source-row"><div><code>{{ item.label }}</code><span>{{ item.config_path ?? '未能读取配置路径' }}</span></div></div></div></div></section>
        </section>

        <section v-else-if="activeView === 'config'" class="view">
          <div class="page-heading compact-heading"><div><div class="eyebrow">{{ currentPage.eyebrow }}</div><h1>{{ currentPage.title }}</h1><p class="page-subtitle">受管 frp-panel 与标准 frpc 使用不同的 Profile 和运行时，互不覆盖。</p></div></div>
          <section class="panel profile-strip"><div><div class="panel-kicker">PROFILE SWITCHER</div><h2>{{ activeProfileName }}</h2></div><div class="profile-strip-actions"><select aria-label="Profile 列表" :value="activeProfile?.id ?? ''" @change="chooseProfile(($event.target as HTMLSelectElement).value)"><option value="" disabled>选择 Profile</option><option v-for="profile in profiles" :key="profile.id" :value="profile.id">{{ profile.name }} · {{ modeLabel(profile.mode) }}</option></select><button type="button" class="button button-secondary button-small" @click="createPanelProfile">+ 面板</button><button type="button" class="button button-secondary button-small" @click="createNativeProfile">+ frpc</button><button v-if="activeProfile?.id && profiles.length > 1" type="button" class="button button-ghost button-small" @click="deleteActiveProfile">删除</button></div></section>

          <section v-if="isNative" class="panel config-form native-editor"><div class="panel-heading"><div><div class="panel-kicker">NATIVE FRPC PROFILE</div><h2>官方 frpc 与配置文件</h2></div><span class="form-status" :class="nativeReady ? 'is-ready' : 'is-empty'"><span class="status-dot"></span>{{ nativeReady ? '已保存配置' : '待导入' }}</span></div><div class="field-grid"><label class="field"><span class="field-label">Profile 名称</span><input v-model="nativeProfileName" autocomplete="off" placeholder="例如：家庭 NAS" /></label><label class="field"><span class="field-label">配置来源</span><select v-model="nativeConfig.source"><option value="managed">导入到 App 私有副本（可编辑）</option><option value="external_readonly">引用外部配置（只读）</option></select></label></div><div v-if="nativeConfig.source === 'managed'" class="native-import"><label class="field"><span class="field-label">导入 TOML</span><input type="file" accept=".toml,text/plain" @change="handleNativeFile" /></label><span class="field-hint">{{ nativeImportName ? `当前编辑：${nativeImportName}` : '选择 TOML，或使用下方模板编辑器生成 TOML。原文件不会被改写。' }}</span></div><div v-else class="field"><label class="field-label">外部 frpc 配置绝对路径</label><input v-model="nativeConfig.config_path" autocomplete="off" placeholder="/Users/you/.config/frp/frpc.toml" /><span class="field-hint">仅保存路径并只读观测；外部配置不复制、不读取、不修改。可引用 TOML、YAML、JSON 或兼容的 INI。</span></div>
            <section v-if="nativeConfig.source === 'managed'" class="native-builder"><div class="panel-heading"><div><div class="panel-kicker">COMMON PROXY BUILDER</div><h2>常用代理可视化生成器</h2></div><button type="button" class="button button-secondary button-small" @click="generateToml">生成 TOML</button></div><p class="field-hint">覆盖 TCP、UDP、HTTP、HTTPS 的单代理起始配置；高级字段、多个代理和插件请在 TOML 编辑器中继续维护。</p><div class="field-grid"><label class="field"><span class="field-label">服务端地址</span><input v-model="quickProxy.serverAddr" placeholder="frps.example.com" /></label><label class="field"><span class="field-label">服务端端口</span><input v-model="quickProxy.serverPort" inputmode="numeric" /></label><label class="field"><span class="field-label">认证 Token（写入 TOML）</span><input v-model="quickProxy.authToken" type="password" placeholder="可选" /></label><label class="field"><span class="field-label">代理类型</span><select v-model="quickProxy.type"><option value="tcp">TCP</option><option value="udp">UDP</option><option value="http">HTTP</option><option value="https">HTTPS</option></select></label><label class="field"><span class="field-label">代理名称</span><input v-model="quickProxy.name" /></label><label class="field"><span class="field-label">本地地址</span><input v-model="quickProxy.localIp" /></label><label class="field"><span class="field-label">本地端口</span><input v-model="quickProxy.localPort" inputmode="numeric" /></label><label v-if="quickProxy.type === 'tcp' || quickProxy.type === 'udp'" class="field"><span class="field-label">远程端口</span><input v-model="quickProxy.remotePort" inputmode="numeric" /></label><label v-else class="field"><span class="field-label">自定义域名（逗号分隔）</span><input v-model="quickProxy.domains" placeholder="example.com, www.example.com" /></label></div></section>
            <label v-if="nativeConfig.source === 'managed'" class="field"><span class="field-label">高级 frpc TOML</span><textarea v-model="nativeToml" class="code-area" spellcheck="false" aria-label="高级 frpc TOML 编辑器"></textarea><span class="field-hint">保存并启动时一定会先执行 <code>frpc verify -c</code>。保存本身不会自动 reload；全局连接参数变更应重新启动。</span></label>
            <div class="config-options"><div class="toggle-list"><label class="toggle-row"><input v-model="nativeConfig.auto_connect" type="checkbox" /><span class="toggle-control"><span></span></span><span><strong>打开应用后自动连接</strong><small>仅启动本应用托管的 frpc</small></span></label><label class="toggle-row"><input v-model="nativeConfig.launch_at_login" type="checkbox" @change="toggleLaunchAtLogin" /><span class="toggle-control"><span></span></span><span><strong>登录时启动桌面壳</strong><small>外部 LaunchAgent 不会被修改</small></span></label></div></div><div class="form-actions"><button type="button" class="button button-primary" :disabled="busyAction !== null" @click="saveActiveProfile"><i class="bi bi-floppy"></i>{{ busyAction === 'saving' ? '保存中' : '保存原生 Profile' }}</button><button type="button" class="button button-secondary" :disabled="busyAction !== null || !nativeReady || !engineAvailable || hasExternalConflict" @click="connectClient"><i class="bi bi-play-fill"></i>校验并启动 frpc</button></div></section>

          <template v-else><section class="panel command-panel"><div class="panel-heading"><div><div class="panel-kicker">IMPORT PANEL COMMAND</div><h2>粘贴 frp-panel Client 连接命令</h2></div><button type="button" class="button button-secondary button-small" @click="parseCommand"><i class="bi bi-check-lg"></i>解析并填充</button></div><textarea v-model="commandInput" class="command-input" spellcheck="false" placeholder="frp-panel client -s <secret> -i <client-id> --api-url <api-url> --rpc-url <rpc-url>"></textarea><div class="command-safety-note"><i class="bi bi-shield-check"></i><span>仅提取连接参数；粘贴的 <code>curl | bash</code> 安装脚本不会执行。</span></div></section><form class="panel config-form" @submit.prevent="saveActiveProfile"><div class="panel-heading"><div><div class="panel-kicker">PANEL MANAGED PROFILE</div><h2>frp-panel 连接参数</h2></div><span class="form-status" :class="panelReady ? 'is-ready' : 'is-empty'"><span class="status-dot"></span>{{ panelReady ? '参数完整' : '待填写' }}</span></div><div class="field-grid"><label class="field"><span class="field-label">Client ID</span><input v-model="config.client_id" autocomplete="off" /></label><label class="field"><span class="field-label">Client Secret</span><input v-model="config.client_secret" type="password" autocomplete="new-password" /></label><label class="field"><span class="field-label">API URL</span><input v-model="config.api_url" placeholder="https://server.example.com" /></label><label class="field"><span class="field-label">RPC URL</span><input v-model="config.rpc_url" placeholder="wss://server.example.com" /></label></div><div class="config-options"><div class="toggle-list"><label class="toggle-row"><input v-model="config.auto_connect" type="checkbox" /><span class="toggle-control"><span></span></span><span><strong>打开应用后自动连接</strong><small>仅启动内置受管 Client</small></span></label><label class="toggle-row"><input v-model="config.launch_at_login" type="checkbox" @change="toggleLaunchAtLogin" /><span class="toggle-control"><span></span></span><span><strong>登录时启动桌面壳</strong><small>使用当前系统的登录启动能力</small></span></label><label class="toggle-row"><input v-model="config.allow_insecure_tls" type="checkbox" /><span class="toggle-control"><span></span></span><span><strong>不验证 TLS 证书</strong><small>仅限明确确认自签名服务端时启用</small></span></label></div></div><div class="form-actions"><button type="submit" class="button button-primary" :disabled="busyAction !== null"><i class="bi bi-floppy"></i>保存 Profile</button><button type="button" class="button button-secondary" :disabled="busyAction !== null || !panelReady || !engineAvailable || hasExternalConflict" @click="connectClient"><i class="bi bi-play-fill"></i>保存并连接</button></div></form></template>
        </section>

        <section v-else-if="activeView === 'logs'" class="view"><div class="page-heading compact-heading"><div><div class="eyebrow">{{ currentPage.eyebrow }}</div><h1>{{ currentPage.title }}</h1><p class="page-subtitle">托管进程 stdout、stderr 和桌面壳系统事件</p></div><div class="page-heading-actions"><button type="button" class="button button-secondary button-small" :disabled="!logs.length" @click="copyText(logText, '日志已复制')"><i class="bi bi-copy"></i>复制</button><button type="button" class="button button-ghost button-small" :disabled="!logs.length" @click="clearAllLogs"><i class="bi bi-trash3"></i>清空</button></div></div><div class="log-toolbar"><div class="filter-group" role="group" aria-label="日志筛选"><button v-for="filter in ['all', 'stdout', 'stderr', 'system'] as const" :key="filter" type="button" class="filter-button" :class="{ active: logFilter === filter }" @click="logFilter = filter">{{ filter === 'all' ? '全部' : streamLabel(filter) }}</button></div><span class="log-count">{{ filteredLogs.length }} / {{ logs.length }}</span></div><section ref="logViewport" class="log-terminal" role="log" aria-live="polite"><div v-if="!filteredLogs.length" class="terminal-empty"><i class="bi bi-terminal"></i><span>暂无匹配日志</span></div><div v-for="(entry, index) in filteredLogs" :key="`${entry.ts_ms}-${index}`" class="terminal-line" :class="`stream-${entry.stream}`"><span class="terminal-time">{{ formatTime(entry.ts_ms) }}</span><span class="terminal-tag">{{ streamLabel(entry.stream) }}</span><code>{{ entry.line }}</code></div></section></section>

        <section v-else class="view"><div class="page-heading compact-heading"><div><div class="eyebrow">{{ currentPage.eyebrow }}</div><h1>{{ currentPage.title }}</h1><p class="page-subtitle">macOS 状态栏工具与本地 sidecar</p></div></div><div class="about-grid"><section class="panel sidecar-panel"><div class="panel-heading"><div><div class="panel-kicker">PANEL SIDECAR</div><h2>frp-panel-client</h2></div><span class="availability-badge" :class="sidecar?.available ? 'available' : 'missing'"><span class="status-dot"></span>{{ sidecar?.available ? '已就绪' : '缺失' }}</span></div><div class="sidecar-details"><div><span>目标架构</span><code>{{ sidecar?.target_triple ?? '—' }}</code></div><div><span>预期文件</span><code>{{ sidecar?.expected_name ?? '—' }}</code></div><div><span>用途</span><code>frp-panel Master 受管模式</code></div></div></section><section class="panel sidecar-panel"><div class="panel-heading"><div><div class="panel-kicker">NATIVE SIDECAR</div><h2>官方 frpc</h2></div><span class="availability-badge" :class="sidecar?.native_available ? 'available' : 'missing'"><span class="status-dot"></span>{{ sidecar?.native_available ? '已就绪' : '缺失' }}</span></div><div class="sidecar-details"><div><span>目标架构</span><code>{{ sidecar?.native_target_triple ?? '—' }}</code></div><div><span>预期文件</span><code>{{ sidecar?.native_expected_name ?? '—' }}</code></div><div><span>用途</span><code>标准 frpc.toml 模式</code></div></div></section></div><section class="panel security-panel"><div class="panel-heading"><div><div class="panel-kicker">SECURITY BOUNDARY</div><h2>运行安全边界</h2></div><i class="bi bi-shield-lock panel-heading-icon"></i></div><div class="security-grid"><div><i class="bi bi-check2-circle"></i><span>frp-panel Secret 保存在系统凭据库，不进入启动参数</span></div><div><i class="bi bi-check2-circle"></i><span>托管原生配置保存至应用私有目录并设置用户私有文件权限</span></div><div><i class="bi bi-check2-circle"></i><span>外部 frpc 与 LaunchAgent 仅可发现和展示，绝不接管</span></div><div><i class="bi bi-check2-circle"></i><span>原生配置启动前强制执行 frpc verify</span></div></div></section></section>
      </div>
    </main>
  </div>
</template>
