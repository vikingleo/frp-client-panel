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
  emptyConfig,
  errorMessage,
  getLogs,
  getSidecarInfo,
  getStatus,
  loadConnection,
  parsePanelCommand,
  saveConnection,
  startClient,
  stopClient,
} from "./commands";
import type { ConnectionConfig, LogEntry, RuntimeState, RuntimeStatus, SidecarInfo } from "./types";

type ViewName = "overview" | "config" | "logs" | "about";
type BusyAction = "loading" | "parsing" | "saving" | "starting" | "stopping" | null;
type NoticeKind = "success" | "error" | "info";
type ThemePreference = "system" | "light" | "dark";

const activeView = ref<ViewName>("overview");
const config = ref<ConnectionConfig>(emptyConfig());
const commandInput = ref("");
const logs = ref<LogEntry[]>([]);
const logFilter = ref<"all" | "stdout" | "stderr" | "system">("all");
const secretVisible = ref(false);
const busyAction = ref<BusyAction>(null);
const notice = ref<{ kind: NoticeKind; message: string } | null>(null);
const sidecar = ref<SidecarInfo | null>(null);
const hydrated = ref(false);
const logViewport = ref<HTMLElement | null>(null);
const unlisteners: UnlistenFn[] = [];
const themePreference = ref<ThemePreference>("system");
const systemPrefersDark = ref(false);
let systemThemeQuery: MediaQueryList | null = null;
const THEME_STORAGE_KEY = "frp-panel-client.theme";
const LEGACY_THEME_STORAGE_KEY = "frp-panel-mac-client.theme";

const themeOptions: Array<{ value: ThemePreference; label: string; icon: string }> = [
  { value: "light", label: "亮色", icon: "bi-sun" },
  { value: "dark", label: "暗色", icon: "bi-moon-stars" },
  { value: "system", label: "系统", icon: "bi-display" },
];

const status = ref<RuntimeStatus>({
  state: "stopped",
  state_label: "stopped",
  running: false,
  error: null,
  started_at_ms: null,
  sidecar_available: false,
});

const pageMeta: Record<ViewName, { title: string; eyebrow: string }> = {
  overview: { title: "连接总览", eyebrow: "FRP PANEL / DESKTOP CLIENT" },
  config: { title: "连接配置", eyebrow: "PROFILE / SINGLE CONNECTION" },
  logs: { title: "实时日志", eyebrow: "RUNTIME / STREAM OUTPUT" },
  about: { title: "应用与客户端", eyebrow: "SYSTEM / SIDECAR" },
};

const stateMeta: Record<RuntimeState, { label: string; description: string; tone: string }> = {
  stopped: { label: "未连接", description: "客户端进程没有运行", tone: "neutral" },
  starting: { label: "正在连接", description: "正在等待 frp-panel Master 响应", tone: "warning" },
  running: { label: "已连接", description: "frp-panel-client 正在运行", tone: "success" },
  error: { label: "连接错误", description: "客户端停止或返回了错误", tone: "danger" },
};

const navItems: Array<{ id: ViewName; label: string; icon: string }> = [
  { id: "overview", label: "总览", icon: "bi-grid-1x2" },
  { id: "config", label: "配置", icon: "bi-sliders2" },
  { id: "logs", label: "日志", icon: "bi-terminal" },
  { id: "about", label: "关于", icon: "bi-info-circle" },
];

const currentPage = computed(() => pageMeta[activeView.value]);
const currentState = computed(() => stateMeta[status.value.state] ?? stateMeta.stopped);
const resolvedTheme = computed<"light" | "dark">(() =>
  themePreference.value === "system"
    ? systemPrefersDark.value
      ? "dark"
      : "light"
    : themePreference.value,
);
const isRunning = computed(() => status.value.running);
const hasConfig = computed(() => {
  const value = config.value;
  return [value.client_id, value.client_secret, value.api_url, value.rpc_url].every((item) => item.trim());
});
const filteredLogs = computed(() => {
  if (logFilter.value === "all") return logs.value;
  return logs.value.filter((entry) => entry.stream === logFilter.value);
});
const recentLogs = computed(() => logs.value.slice(-6).reverse());
const lastError = computed(() => {
  if (status.value.error) return status.value.error;
  return [...logs.value].reverse().find((entry) => entry.stream === "stderr")?.line ?? "";
});
const connectionHint = computed(() => {
  if (!hasConfig.value) return "尚未保存连接参数";
  return `${config.value.client_id} · ${config.value.api_url}`;
});
const uptimeLabel = computed(() => {
  if (!status.value.running || status.value.started_at_ms == null) return "—";
  return formatDuration(Number(status.value.started_at_ms));
});
const logText = computed(() =>
  logs.value
    .map((entry) => `[${formatTime(entry.ts_ms)}] ${entry.stream.toUpperCase()}  ${entry.line}`)
    .join("\n"),
);

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

function setThemePreference(preference: ThemePreference) {
  themePreference.value = preference;
}

function applyTheme() {
  document.documentElement.dataset.theme = resolvedTheme.value;
  document.documentElement.style.colorScheme = resolvedTheme.value;
}

function readThemePreference(): ThemePreference {
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY) ?? window.localStorage.getItem(LEGACY_THEME_STORAGE_KEY);
    if (stored === "light" || stored === "dark" || stored === "system") return stored;
  } catch {
    // Theme persistence is optional when local storage is unavailable.
  }
  return "system";
}

function initializeTheme() {
  themePreference.value = readThemePreference();
  systemThemeQuery = window.matchMedia("(prefers-color-scheme: dark)");
  systemPrefersDark.value = systemThemeQuery.matches;
  systemThemeQuery.addEventListener("change", handleSystemThemeChange);
  applyTheme();
}

function handleSystemThemeChange(event: MediaQueryListEvent) {
  systemPrefersDark.value = event.matches;
}

function formatTime(timestamp: number) {
  const date = new Date(Number(timestamp));
  if (Number.isNaN(date.getTime())) return "--:--:--";
  return date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

function formatDuration(milliseconds: number) {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}h ${String(minutes).padStart(2, "0")}m`;
  if (minutes > 0) return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
  return `${seconds}s`;
}

function streamLabel(stream: LogEntry["stream"]) {
  if (stream === "stderr") return "错误";
  if (stream === "system") return "系统";
  return "输出";
}

async function scrollLogsToEnd() {
  await nextTick();
  if (logViewport.value) logViewport.value.scrollTop = logViewport.value.scrollHeight;
}

async function refreshRuntime() {
  try {
    const [nextStatus, nextLogs, nextSidecar] = await Promise.all([getStatus(), getLogs(), getSidecarInfo()]);
    status.value = nextStatus;
    logs.value = nextLogs;
    sidecar.value = nextSidecar;
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
}

async function hydrate() {
  busyAction.value = "loading";
  try {
    const [storedConfig, nextStatus, nextLogs, nextSidecar, launchAtLoginEnabled] = await Promise.all([
      loadConnection(),
      getStatus(),
      getLogs(),
      getSidecarInfo(),
      readLaunchAtLoginState(),
    ]);
    if (storedConfig) config.value = storedConfig;
    if (launchAtLoginEnabled !== null) config.value.launch_at_login = launchAtLoginEnabled;
    status.value = nextStatus;
    logs.value = nextLogs;
    sidecar.value = nextSidecar;
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

async function pasteCommand() {
  try {
    commandInput.value = await navigator.clipboard.readText();
    setNotice("info", "已从剪贴板读取命令");
  } catch (error) {
    setNotice("error", `无法读取剪贴板：${errorMessage(error)}`);
  }
}

async function parseCommand() {
  if (!commandInput.value.trim()) {
    setNotice("error", "请先粘贴 frp-panel Client 连接命令");
    return;
  }
  busyAction.value = "parsing";
  clearNotice();
  try {
    const parsed = await parsePanelCommand(commandInput.value);
    config.value = {
      ...parsed,
      auto_connect: config.value.auto_connect,
      launch_at_login: config.value.launch_at_login,
    };
    setNotice(
      "success",
      isInstallCommand(commandInput.value)
        ? "安装命令中的连接参数已解析；应用不会执行安装脚本"
        : "连接命令已解析，保存后即可连接",
    );
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busyAction.value = null;
  }
}

async function saveConfig() {
  busyAction.value = "saving";
  clearNotice();
  try {
    await persistConfig();
    setNotice("success", "连接配置已保存");
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busyAction.value = null;
  }
}

async function connectClient() {
  clearNotice();
  try {
    sidecar.value = await getSidecarInfo();
  } catch (error) {
    setNotice("error", errorMessage(error));
    return;
  }
  if (!sidecar.value.available) {
    setNotice("error", sidecar.value.hint);
    return;
  }
  busyAction.value = "starting";
  try {
    await persistConfig();
    await startClient();
    status.value = await getStatus();
    setNotice("success", "已请求启动 frp-panel-client");
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busyAction.value = null;
  }
}

function isInstallCommand(command: string) {
  return /\b(?:curl|wget)\b[\s\S]*\|\s*(?:bash|sh)\b/.test(command);
}

async function persistConfig() {
  const wasLaunchEnabled = await isAutostartEnabled();
  const shouldLaunchAtLogin = config.value.launch_at_login;
  const launchStateChanged = wasLaunchEnabled !== shouldLaunchAtLogin;

  try {
    if (launchStateChanged) {
      if (shouldLaunchAtLogin) {
        await enableAutostart();
      } else {
        await disableAutostart();
      }
    }
    await saveConnection({ ...config.value });
  } catch (error) {
    if (launchStateChanged) {
      try {
        if (wasLaunchEnabled) {
          await enableAutostart();
        } else {
          await disableAutostart();
        }
      } catch {
        // The primary failure is reported to the user. Rollback is best effort.
      }
    }
    throw error;
  }
}

async function disconnectClient() {
  busyAction.value = "stopping";
  clearNotice();
  try {
    await stopClient();
    status.value = await getStatus();
    setNotice("success", "客户端已停止");
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busyAction.value = null;
  }
}

async function copyText(value: string, successMessage: string) {
  if (!value) {
    setNotice("info", "当前没有可复制的内容");
    return;
  }
  try {
    await navigator.clipboard.writeText(value);
    setNotice("success", successMessage);
  } catch (error) {
    setNotice("error", `无法写入剪贴板：${errorMessage(error)}`);
  }
}

async function clearAllLogs() {
  try {
    await clearLogs();
    logs.value = [];
    setNotice("success", "日志已清空");
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
}

async function initializeListeners() {
  const statusUnlisten = await listen<RuntimeStatus>("client://status", (event) => {
    status.value = event.payload;
  });
  const logUnlisten = await listen<LogEntry>("client://log", async (event) => {
    logs.value = [...logs.value, event.payload].slice(-800);
    if (activeView.value === "logs") await scrollLogsToEnd();
  });
  unlisteners.push(statusUnlisten, logUnlisten);
}

onMounted(async () => {
  initializeTheme();
  try {
    await initializeListeners();
  } catch (error) {
    setNotice("error", `事件通道初始化失败：${errorMessage(error)}`);
  }
  await hydrate();
});

onBeforeUnmount(() => {
  unlisteners.splice(0).forEach((unlisten) => unlisten());
  systemThemeQuery?.removeEventListener("change", handleSystemThemeChange);
});

watch(activeView, async (view) => {
  if (view === "logs") await scrollLogsToEnd();
});

watch(resolvedTheme, applyTheme);

watch(themePreference, (preference) => {
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, preference);
    window.localStorage.removeItem(LEGACY_THEME_STORAGE_KEY);
  } catch {
    // Theme persistence is optional when local storage is unavailable.
  }
});
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand-block">
        <div class="brand-mark" aria-hidden="true"><i class="bi bi-diagram-3-fill"></i></div>
        <div>
          <div class="brand-name">FRP PANEL</div>
          <div class="brand-caption">MAC CLIENT</div>
        </div>
      </div>

      <div class="sidebar-section-label">工作区</div>
      <nav class="navigation" aria-label="主导航">
        <button
          v-for="item in navItems"
          :key="item.id"
          type="button"
          class="nav-item"
          :class="{ active: activeView === item.id }"
          :aria-current="activeView === item.id ? 'page' : undefined"
          @click="navigate(item.id)"
        >
          <i class="bi" :class="item.icon" aria-hidden="true"></i>
          <span>{{ item.label }}</span>
          <span v-if="item.id === 'logs' && logs.length" class="nav-count">{{ logs.length }}</span>
        </button>
      </nav>

      <div class="sidebar-spacer"></div>

      <div class="sidebar-runtime" :class="`tone-${currentState.tone}`">
        <div class="runtime-mini-heading">
          <span class="status-dot" aria-hidden="true"></span>
          <span>运行状态</span>
        </div>
        <div class="runtime-mini-value">{{ currentState.label }}</div>
        <div class="runtime-mini-detail">{{ connectionHint }}</div>
      </div>

      <div class="sidebar-footer">
        <span class="version-dot"></span>
        <span>v0.1.0 · {{ sidecar?.target_triple ?? 'desktop' }}</span>
      </div>
    </aside>

    <main class="main-pane">
      <header class="topbar">
        <div class="breadcrumb">
          <span class="breadcrumb-muted">客户端</span>
          <i class="bi bi-chevron-right" aria-hidden="true"></i>
          <span>{{ currentPage.title }}</span>
        </div>
        <div class="topbar-actions">
          <div class="theme-switcher" role="group" aria-label="外观主题">
            <button
              v-for="option in themeOptions"
              :key="option.value"
              type="button"
              class="theme-button"
              :class="{ active: themePreference === option.value }"
              :aria-pressed="themePreference === option.value"
              :title="`切换为${option.label}主题`"
              @click="setThemePreference(option.value)"
            >
              <i class="bi" :class="option.icon" aria-hidden="true"></i>
              <span>{{ option.label }}</span>
            </button>
          </div>
          <div class="top-status" :class="`tone-${currentState.tone}`" aria-live="polite">
            <span class="status-dot" aria-hidden="true"></span>
            <span>{{ currentState.label }}</span>
          </div>
          <button
            type="button"
            class="icon-button"
            title="刷新状态"
            aria-label="刷新状态"
            :disabled="busyAction === 'loading'"
            @click="refreshRuntime"
          >
            <i class="bi bi-arrow-clockwise" :class="{ spinning: busyAction === 'loading' }" aria-hidden="true"></i>
          </button>
        </div>
      </header>

      <div class="content-scroll">
        <div v-if="notice" class="notice" :class="`notice-${notice.kind}`" role="status" aria-live="polite">
          <i v-if="notice.kind === 'success'" class="bi bi-check-circle-fill" aria-hidden="true"></i>
          <i v-else-if="notice.kind === 'info'" class="bi bi-info-circle-fill" aria-hidden="true"></i>
          <i v-else class="bi bi-exclamation-circle-fill" aria-hidden="true"></i>
          <span>{{ notice.message }}</span>
          <button type="button" class="notice-close" aria-label="关闭提示" title="关闭提示" @click="clearNotice">
            <i class="bi bi-x-lg" aria-hidden="true"></i>
          </button>
        </div>

        <div v-if="sidecar && !sidecar.available" class="dependency-banner" role="alert">
          <div class="dependency-icon"><i class="bi bi-box-arrow-down" aria-hidden="true"></i></div>
          <div class="dependency-copy">
            <strong>缺少 Darwin sidecar</strong>
            <span>当前架构：{{ sidecar.target_triple }} · 期望文件：{{ sidecar.expected_name }}</span>
          </div>
          <button type="button" class="button button-secondary button-small" @click="navigate('about')">
            查看安装信息
            <i class="bi bi-chevron-right" aria-hidden="true"></i>
          </button>
        </div>

        <section v-if="activeView === 'overview'" class="view view-overview" aria-labelledby="overview-title">
          <div class="page-heading">
            <div>
              <div class="eyebrow">{{ currentPage.eyebrow }}</div>
              <h1 id="overview-title">{{ currentPage.title }}</h1>
              <p class="page-subtitle">单 Profile 受管客户端控制台</p>
            </div>
            <button
              v-if="!isRunning"
              type="button"
              class="button button-primary"
              :disabled="busyAction !== null || !hasConfig || !sidecar?.available"
              @click="connectClient"
            >
              <i v-if="busyAction === 'starting'" class="bi bi-arrow-repeat spinning" aria-hidden="true"></i>
              <i v-else class="bi bi-play-fill" aria-hidden="true"></i>
              {{ busyAction === 'starting' ? '正在启动' : '连接客户端' }}
            </button>
            <button v-else type="button" class="button button-danger" :disabled="busyAction !== null" @click="disconnectClient">
              <i v-if="busyAction === 'stopping'" class="bi bi-arrow-repeat spinning" aria-hidden="true"></i>
              <i v-else class="bi bi-power" aria-hidden="true"></i>
              {{ busyAction === 'stopping' ? '正在停止' : '断开客户端' }}
            </button>
          </div>

          <div class="status-panel" :class="`state-${status.state}`">
            <div class="status-panel-main">
              <div class="status-icon" aria-hidden="true">
                <i v-if="status.state === 'starting'" class="bi bi-arrow-repeat spinning"></i>
                <i v-else-if="status.state === 'running'" class="bi bi-check-circle-fill"></i>
                <i v-else-if="status.state === 'error'" class="bi bi-exclamation-triangle-fill"></i>
                <i v-else class="bi bi-wifi-off"></i>
              </div>
              <div>
                <div class="status-label">{{ currentState.label }}</div>
                <div class="status-description">{{ currentState.description }}</div>
              </div>
            </div>
            <div class="status-panel-meta">
              <div>
                <span class="meta-label">运行时长</span>
                <strong>{{ uptimeLabel }}</strong>
              </div>
              <div>
                <span class="meta-label">日志条数</span>
                <strong>{{ logs.length }}</strong>
              </div>
            </div>
          </div>

          <div v-if="lastError" class="error-strip" role="alert">
            <i class="bi bi-exclamation-triangle-fill" aria-hidden="true"></i>
            <div>
              <span class="error-strip-label">最近错误</span>
              <span class="error-strip-text">{{ lastError }}</span>
            </div>
            <button type="button" class="text-button" @click="navigate('logs')">查看日志 <i class="bi bi-chevron-right" aria-hidden="true"></i></button>
          </div>

          <div class="overview-grid">
            <section class="panel connection-panel">
              <div class="panel-heading">
                <div>
                  <div class="panel-kicker">CONNECTION</div>
                  <h2>连接参数</h2>
                </div>
                <button type="button" class="icon-button" title="编辑连接配置" aria-label="编辑连接配置" @click="navigate('config')">
                  <i class="bi bi-sliders2" aria-hidden="true"></i>
                </button>
              </div>
              <div v-if="hasConfig" class="kv-list">
                <div class="kv-row"><span>Client ID</span><code>{{ config.client_id }}</code></div>
                <div class="kv-row"><span>API URL</span><code>{{ config.api_url }}</code></div>
                <div class="kv-row"><span>RPC URL</span><code>{{ config.rpc_url }}</code></div>
                <div class="kv-row"><span>Secret</span><code>{{ secretVisible ? config.client_secret : '••••••••••••' }}</code></div>
              </div>
              <div v-else class="empty-block">
                <i class="bi bi-file-earmark-text" aria-hidden="true"></i>
                <span>尚未配置连接参数</span>
                <button type="button" class="text-button" @click="navigate('config')">开始配置 <i class="bi bi-chevron-right" aria-hidden="true"></i></button>
              </div>
            </section>

            <section class="panel activity-panel">
              <div class="panel-heading">
                <div>
                  <div class="panel-kicker">LATEST OUTPUT</div>
                  <h2>最近输出</h2>
                </div>
                <button type="button" class="text-button" @click="navigate('logs')">全部日志 <i class="bi bi-chevron-right" aria-hidden="true"></i></button>
              </div>
              <div v-if="recentLogs.length" class="recent-log-list">
                <div v-for="entry in recentLogs" :key="`${entry.ts_ms}-${entry.line}`" class="recent-log-line" :class="`stream-${entry.stream}`">
                  <span class="log-time">{{ formatTime(entry.ts_ms) }}</span>
                  <span class="log-stream">{{ streamLabel(entry.stream) }}</span>
                  <span class="log-line-text">{{ entry.line }}</span>
                </div>
              </div>
              <div v-else class="empty-block compact">
                <i class="bi bi-terminal" aria-hidden="true"></i>
                <span>暂无运行日志</span>
              </div>
            </section>
          </div>

          <div class="metric-grid">
            <div class="metric-item">
              <span class="metric-icon"><i class="bi bi-hdd-network" aria-hidden="true"></i></span>
              <div><span class="metric-label">sidecar</span><strong>{{ sidecar?.available ? '已就绪' : '未就绪' }}</strong></div>
            </div>
            <div class="metric-item">
              <span class="metric-icon"><i class="bi bi-shield-lock" aria-hidden="true"></i></span>
              <div><span class="metric-label">secret</span><strong>Keychain</strong></div>
            </div>
            <div class="metric-item">
              <span class="metric-icon"><i class="bi bi-wifi" aria-hidden="true"></i></span>
              <div><span class="metric-label">自动连接</span><strong>{{ config.auto_connect ? '已开启' : '已关闭' }}</strong></div>
            </div>
          </div>
        </section>

        <section v-else-if="activeView === 'config'" class="view" aria-labelledby="config-title">
          <div class="page-heading compact-heading">
            <div>
              <div class="eyebrow">{{ currentPage.eyebrow }}</div>
              <h1 id="config-title">{{ currentPage.title }}</h1>
              <p class="page-subtitle">导入 Client 连接参数，或手动填写；桌面版已内置官方 Client</p>
            </div>
          </div>

          <section class="panel command-panel">
            <div class="panel-heading">
              <div>
                <div class="panel-kicker">IMPORT CONNECTION</div>
                <h2>粘贴 Client 连接命令</h2>
              </div>
              <div class="panel-heading-actions">
                <button type="button" class="button button-secondary button-small" @click="pasteCommand">
                  <i class="bi bi-clipboard-plus" aria-hidden="true"></i>
                  粘贴
                </button>
                <button type="button" class="icon-button" title="复制当前命令文本" aria-label="复制当前命令文本" @click="copyText(commandInput, '命令文本已复制')">
                  <i class="bi bi-copy" aria-hidden="true"></i>
                </button>
              </div>
            </div>
            <textarea
              v-model="commandInput"
              class="command-input"
              spellcheck="false"
              aria-label="frp-panel Client 连接命令；仅解析文本，不会执行命令"
              placeholder="frp-panel client -s <secret> -i <client-id> --api-url <api-url> --rpc-url <rpc-url>"
            ></textarea>
            <div class="command-safety-note">
              <i class="bi bi-shield-check" aria-hidden="true"></i>
              <span>应用只从命令文本提取连接参数。即使粘贴 <code>curl | bash</code> 安装命令，也不会下载或执行脚本。</span>
            </div>
            <div class="command-panel-footer">
              <span class="field-hint">支持直接连接命令、Linux 安装命令和带引号参数；安装命令仅用于提取参数。</span>
              <button type="button" class="button button-primary button-small" :disabled="busyAction !== null" @click="parseCommand">
                <i v-if="busyAction === 'parsing'" class="bi bi-arrow-repeat spinning" aria-hidden="true"></i>
                <i v-else class="bi bi-check-lg" aria-hidden="true"></i>
                {{ busyAction === 'parsing' ? '解析中' : '解析并填充' }}
              </button>
            </div>
          </section>

          <form class="panel config-form" @submit.prevent="saveConfig">
            <div class="panel-heading">
              <div>
                <div class="panel-kicker">CLIENT PROFILE</div>
                <h2>连接参数</h2>
              </div>
              <span class="form-status" :class="hasConfig ? 'is-ready' : 'is-empty'">
                <span class="status-dot"></span>{{ hasConfig ? '参数完整' : '待填写' }}
              </span>
            </div>

            <div class="field-grid">
              <label class="field">
                <span class="field-label">Client ID</span>
                <input v-model="config.client_id" type="text" autocomplete="off" placeholder="例如 user.c.mac" />
              </label>
              <label class="field">
                <span class="field-label">Client Secret</span>
                <div class="secret-field">
                  <input v-model="config.client_secret" :type="secretVisible ? 'text' : 'password'" autocomplete="off" placeholder="面板生成的 secret" />
                  <button type="button" class="field-icon-button" :title="secretVisible ? '隐藏 Secret' : '显示 Secret'" :aria-label="secretVisible ? '隐藏 Secret' : '显示 Secret'" @click="secretVisible = !secretVisible">
                    <i v-if="secretVisible" class="bi bi-eye-slash" aria-hidden="true"></i>
                    <i v-else class="bi bi-eye" aria-hidden="true"></i>
                  </button>
                </div>
              </label>
              <label class="field">
                <span class="field-label">API URL</span>
                <input v-model="config.api_url" type="url" autocomplete="off" placeholder="https://panel.example.com" />
                <span class="field-hint">必须使用 http:// 或 https://</span>
              </label>
              <label class="field">
                <span class="field-label">RPC URL</span>
                <input v-model="config.rpc_url" type="text" autocomplete="off" placeholder="wss://panel.example.com/rpc" />
                <span class="field-hint">支持 grpc://、ws://、wss://</span>
              </label>
            </div>

            <div class="config-options">
              <div class="toggle-list">
                <label class="toggle-row">
                  <input v-model="config.auto_connect" type="checkbox" />
                  <span class="toggle-control" aria-hidden="true"><span></span></span>
                  <span>
                    <strong>打开应用时自动连接</strong>
                    <small>应用启动后自动拉起 frp-panel-client</small>
                  </span>
                </label>
                <label class="toggle-row">
                  <input v-model="config.launch_at_login" type="checkbox" />
                  <span class="toggle-control" aria-hidden="true"><span></span></span>
                  <span>
                    <strong>登录后启动应用</strong>
                    <small>使用当前系统的自动启动机制常驻托盘</small>
                  </span>
                </label>
              </div>
              <div class="credential-note">
                <i class="bi bi-shield-lock" aria-hidden="true"></i>
                Secret 保存于系统凭据库，日志由后端脱敏
              </div>
            </div>

            <div class="form-actions">
              <button type="submit" class="button button-primary" :disabled="busyAction !== null">
                <i v-if="busyAction === 'saving'" class="bi bi-arrow-repeat spinning" aria-hidden="true"></i>
                <i v-else class="bi bi-floppy" aria-hidden="true"></i>
                {{ busyAction === 'saving' ? '保存中' : '保存配置' }}
              </button>
              <button type="button" class="button button-secondary" :disabled="busyAction !== null || !hasConfig || !sidecar?.available" @click="connectClient">
                <i class="bi bi-play-fill" aria-hidden="true"></i>
                保存并连接
              </button>
            </div>
          </form>
        </section>

        <section v-else-if="activeView === 'logs'" class="view logs-view" aria-labelledby="logs-title">
          <div class="page-heading compact-heading">
            <div>
              <div class="eyebrow">{{ currentPage.eyebrow }}</div>
              <h1 id="logs-title">{{ currentPage.title }}</h1>
              <p class="page-subtitle">stdout、stderr 和系统事件</p>
            </div>
            <div class="page-heading-actions">
              <button type="button" class="button button-secondary button-small" :disabled="!logs.length" @click="copyText(logText, '日志已复制')">
                <i class="bi bi-copy" aria-hidden="true"></i>
                复制日志
              </button>
              <button type="button" class="button button-ghost button-small" :disabled="!logs.length" @click="clearAllLogs">
                <i class="bi bi-trash3" aria-hidden="true"></i>
                清空
              </button>
            </div>
          </div>

          <div class="log-toolbar">
            <div class="filter-group" role="group" aria-label="日志筛选">
              <button v-for="filter in ['all', 'stdout', 'stderr', 'system'] as const" :key="filter" type="button" class="filter-button" :class="{ active: logFilter === filter }" @click="logFilter = filter">
                {{ filter === 'all' ? '全部' : streamLabel(filter) }}
              </button>
            </div>
            <span class="log-count">{{ filteredLogs.length }} / {{ logs.length }}</span>
          </div>

          <section ref="logViewport" class="log-terminal" role="log" aria-live="polite" aria-label="frp-panel-client 运行日志">
            <div v-if="!filteredLogs.length" class="terminal-empty">
              <i class="bi bi-terminal" aria-hidden="true"></i>
              <span>暂无匹配日志</span>
            </div>
            <div v-for="(entry, index) in filteredLogs" :key="`${entry.ts_ms}-${index}`" class="terminal-line" :class="`stream-${entry.stream}`">
              <span class="terminal-time">{{ formatTime(entry.ts_ms) }}</span>
              <span class="terminal-tag">{{ streamLabel(entry.stream) }}</span>
              <code>{{ entry.line }}</code>
            </div>
          </section>
        </section>

        <section v-else class="view" aria-labelledby="about-title">
          <div class="page-heading compact-heading">
            <div>
              <div class="eyebrow">{{ currentPage.eyebrow }}</div>
              <h1 id="about-title">{{ currentPage.title }}</h1>
              <p class="page-subtitle">Tauri 桌面壳与随应用打包的官方 Client sidecar</p>
            </div>
          </div>

          <div class="about-grid">
            <section class="panel sidecar-panel">
              <div class="panel-heading">
                <div>
                  <div class="panel-kicker">SIDECAR STATUS</div>
                  <h2>frp-panel-client</h2>
                </div>
                <div class="availability-badge" :class="sidecar?.available ? 'available' : 'missing'">
                  <span class="status-dot"></span>{{ sidecar?.available ? '已就绪' : '内置组件缺失' }}
                </div>
              </div>
              <div class="sidecar-visual" :class="{ ready: sidecar?.available }">
                <div class="sidecar-icon"><i class="bi bi-hdd-network" aria-hidden="true"></i></div>
                <div>
                  <strong>{{ sidecar?.expected_name ?? 'frp-panel-client' }}</strong>
                  <span>{{ sidecar?.target_triple ?? '正在检测目标架构' }}</span>
                </div>
              </div>
              <div class="sidecar-details">
                <div><span>当前状态</span><code>{{ sidecar?.available ? 'bundled client ready' : 'bundled client missing' }}</code></div>
                <div><span>支持平台</span><code>macOS · Linux · Windows</code></div>
                <div><span>当前配置</span><code>{{ hydrated ? 'loaded' : 'loading' }}</code></div>
              </div>
              <div v-if="sidecar && !sidecar.available" class="install-hint">
                <i class="bi bi-box-arrow-down" aria-hidden="true"></i>
                <div><strong>内置客户端不可用</strong><span>{{ sidecar.hint }}</span></div>
              </div>
            </section>

            <section class="panel about-panel">
              <div class="panel-heading">
                <div>
                  <div class="panel-kicker">BUILD PROFILE</div>
                  <h2>运行边界</h2>
                </div>
                <i class="bi bi-shield-lock panel-heading-icon" aria-hidden="true"></i>
              </div>
              <div class="about-list">
                <div class="about-row"><span>连接协议</span><strong>frp-panel managed client</strong></div>
                <div class="about-row"><span>桌面框架</span><strong>Tauri v2 + Rust</strong></div>
                <div class="about-row"><span>状态栏</span><strong>macOS tray resident</strong></div>
                <div class="about-row"><span>Profile 数量</span><strong>单连接</strong></div>
              </div>
              <div class="limitation-box">
                <i class="bi bi-info-circle" aria-hidden="true"></i>
                <span>当前版本不执行 join-token，不写入 /etc/frpp/.env。</span>
              </div>
            </section>
          </div>

          <section class="panel security-panel">
            <div class="panel-heading">
              <div>
                <div class="panel-kicker">SECURITY</div>
                <h2>本地凭据策略</h2>
              </div>
              <i class="bi bi-shield-lock panel-heading-icon" aria-hidden="true"></i>
            </div>
            <div class="security-grid">
              <div><i class="bi bi-check2-circle" aria-hidden="true"></i><span>Client Secret 保存于系统凭据库</span></div>
              <div><i class="bi bi-check2-circle" aria-hidden="true"></i><span>运行日志由 Rust 后端脱敏</span></div>
              <div><i class="bi bi-check2-circle" aria-hidden="true"></i><span>附加能力默认关闭</span></div>
            </div>
          </section>
        </section>
      </div>
    </main>
  </div>
</template>
