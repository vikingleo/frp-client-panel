<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import {
  clearServerLogs,
  deleteServerProfile,
  getServerLogs,
  getServerStatus,
  getSidecarInfo,
  listServerProfiles,
  loadManagedServerConfig,
  loadServerProfile,
  saveServerProfile,
  selectServerProfile,
  startServerProfile,
  stopServer,
  errorMessage,
} from "../commands";
import type {
  LogEntry,
  NativeFrpsConfig,
  ServerProfile,
  ServerProfileSummary,
  ServerRuntimeStatus,
  SidecarInfo,
} from "../types";

type NoticeKind = "success" | "error" | "info";

const emptyNative = (): NativeFrpsConfig => ({
  config_path: "",
  source: "managed",
  auto_start: false,
});

const emptyStatus = (): ServerRuntimeStatus => ({
  state: "stopped",
  state_label: "stopped",
  running: false,
  error: null,
  started_at_ms: null,
  sidecar_available: false,
  profile_id: null,
  binary_name: null,
  config_path: null,
});

const profiles = ref<ServerProfileSummary[]>([]);
const activeProfile = ref<ServerProfile | null>(null);
const profileName = ref("本机 frps");
const nativeConfig = ref<NativeFrpsConfig>(emptyNative());
const serverToml = ref("");
const importedName = ref("");
const status = ref<ServerRuntimeStatus>(emptyStatus());
const logs = ref<LogEntry[]>([]);
const sidecar = ref<SidecarInfo | null>(null);
const busy = ref<"loading" | "saving" | "starting" | "stopping" | null>(null);
const notice = ref<{ kind: NoticeKind; message: string } | null>(null);
const unlisteners: UnlistenFn[] = [];
const serverForm = ref({
  bindAddr: "0.0.0.0",
  bindPort: "7000",
  serverAddr: "",
  authToken: "",
  dashboardAddr: "127.0.0.1",
  dashboardPort: "7500",
  dashboardUser: "admin",
  dashboardPassword: "",
  tlsForce: true,
});

const configured = computed(() => Boolean(activeProfile.value?.id && nativeConfig.value.config_path.trim()));
const canStart = computed(
  () => Boolean(sidecar.value?.server_available) &&
    (nativeConfig.value.source === "managed"
      ? Boolean(serverToml.value.trim() || nativeConfig.value.config_path.trim())
      : Boolean(nativeConfig.value.config_path.trim())),
);
const clientAccessToml = computed(() => {
  const address = serverForm.value.serverAddr.trim() || serverForm.value.bindAddr.trim();
  const port = Number(serverForm.value.bindPort);
  const token = serverForm.value.authToken.trim();
  if (!address || !Number.isInteger(port) || port < 1 || port > 65535 || !token) return "";
  return [
    `serverAddr = "${escapeToml(address)}"`,
    `serverPort = ${port}`,
    "",
    "[auth]",
    'method = "token"',
    `token = "${escapeToml(token)}"`,
    "",
    "# Add one or more [[proxies]] blocks below for each client service.",
    "",
  ].join("\n");
});

function escapeToml(value: string) {
  return value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function setNotice(kind: NoticeKind, message: string) {
  notice.value = { kind, message };
}

function clearNotice() {
  notice.value = null;
}

function applyProfile(profile: ServerProfile | null) {
  activeProfile.value = profile;
  if (!profile) {
    profileName.value = "本机 frps";
    nativeConfig.value = emptyNative();
    serverToml.value = "";
    return;
  }
  profileName.value = profile.name;
  nativeConfig.value = profile.native;
  if (profile.native.source !== "managed") {
    serverToml.value = "";
  }
}

async function hydrate(profileId?: string) {
  const [nextProfiles, profile, nextStatus, nextLogs, nextSidecar] = await Promise.all([
    listServerProfiles(),
    loadServerProfile(profileId),
    getServerStatus(),
    getServerLogs(),
    getSidecarInfo(),
  ]);
  profiles.value = nextProfiles;
  applyProfile(profile);
  status.value = nextStatus;
  logs.value = nextLogs;
  sidecar.value = nextSidecar;
  if (profile?.id && profile.native.source === "managed") {
    try {
      serverToml.value = await loadManagedServerConfig(profile.id);
    } catch (error) {
      serverToml.value = "";
      setNotice("error", errorMessage(error));
    }
  }
}

async function chooseProfile(profileId: string) {
  if (!profileId) return;
  try {
    await selectServerProfile(profileId);
    await hydrate(profileId);
    clearNotice();
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
}

function createProfile() {
  activeProfile.value = { id: "", name: "本机 frps", native: emptyNative() };
  profileName.value = "本机 frps";
  nativeConfig.value = emptyNative();
  serverToml.value = "";
  importedName.value = "";
  clearNotice();
}

async function handleConfigFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  if (file.size > 1024 * 1024) {
    setNotice("error", "frps 配置文件超过 1 MiB，拒绝导入");
    input.value = "";
    return;
  }
  try {
    serverToml.value = await file.text();
    importedName.value = file.name;
    nativeConfig.value.source = "managed";
    setNotice("success", "frps TOML 已读入编辑器；保存时会写入 App 私有副本");
  } catch (error) {
    setNotice("error", `读取 frps 配置失败：${errorMessage(error)}`);
  } finally {
    input.value = "";
  }
}

function generateServerToml() {
  const form = serverForm.value;
  const bindPort = Number(form.bindPort);
  const dashboardPort = Number(form.dashboardPort);
  if (!form.bindAddr.trim() || !Number.isInteger(bindPort) || bindPort < 1 || bindPort > 65535) {
    return setNotice("error", "请填写有效的监听地址与 bindPort");
  }
  if (!form.dashboardAddr.trim() || !Number.isInteger(dashboardPort) || dashboardPort < 1 || dashboardPort > 65535) {
    return setNotice("error", "请填写有效的 Dashboard 地址与端口");
  }
  if (!form.authToken.trim() || !form.dashboardPassword.trim()) {
    return setNotice("error", "服务端 Token 与 Dashboard 密码不能为空");
  }
  const lines = [
    `bindAddr = "${escapeToml(form.bindAddr.trim())}"`,
    `bindPort = ${bindPort}`,
    "",
    "[auth]",
    'method = "token"',
    `token = "${escapeToml(form.authToken.trim())}"`,
    "",
    "[transport.tls]",
    `force = ${form.tlsForce ? "true" : "false"}`,
    "",
    "[webServer]",
    `addr = "${escapeToml(form.dashboardAddr.trim())}"`,
    `port = ${dashboardPort}`,
    `user = "${escapeToml(form.dashboardUser.trim() || "admin")}"`,
    `password = "${escapeToml(form.dashboardPassword.trim())}"`,
    "",
  ];
  nativeConfig.value.source = "managed";
  serverToml.value = lines.join("\n");
  importedName.value = "由服务端向导生成";
  setNotice("success", "已生成 frps TOML；保存并启动前会自动执行 frps verify");
};

async function save(showNotice = true) {
  const saved = await saveServerProfile({
    profileId: activeProfile.value?.id || undefined,
    name: profileName.value,
    configPath: nativeConfig.value.config_path,
    source: nativeConfig.value.source,
    autoStart: nativeConfig.value.auto_start,
    importedContent: nativeConfig.value.source === "managed" ? serverToml.value : undefined,
  });
  await hydrate(saved.id);
  if (showNotice) setNotice("success", "本机 frps Server Profile 已保存");
}

async function saveProfile() {
  busy.value = "saving";
  try {
    await save();
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busy.value = null;
  }
}

async function start() {
  busy.value = "starting";
  try {
    if (!canStart.value) throw new Error("请先保存有效的 frps Profile，并确认服务端 sidecar 已就绪");
    await save(false);
    await startServerProfile(activeProfile.value?.id);
    status.value = await getServerStatus();
    setNotice("success", "frps 已启动；其他客户端现在可以按接入配置连接");
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busy.value = null;
  }
}

async function stop() {
  busy.value = "stopping";
  try {
    await stopServer();
    status.value = await getServerStatus();
  } catch (error) {
    setNotice("error", errorMessage(error));
  } finally {
    busy.value = null;
  }
}

async function removeProfile() {
  if (!activeProfile.value?.id || profiles.value.length <= 1) return;
  try {
    await deleteServerProfile(activeProfile.value.id);
    await hydrate();
    setNotice("success", "Server Profile 已删除；托管配置文件仍保留在本地");
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
}

async function clearLogs() {
  await clearServerLogs();
  logs.value = [];
}

async function copy(value: string, message: string) {
  if (!value) return setNotice("info", "当前没有可复制的内容");
  try {
    await navigator.clipboard.writeText(value);
    setNotice("success", message);
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
}

onMounted(async () => {
  unlisteners.push(
    await listen<ServerRuntimeStatus>("server://status", (event) => (status.value = event.payload)),
    await listen<LogEntry>("server://log", (event) => (logs.value = [...logs.value, event.payload].slice(-800))),
  );
  try {
    await hydrate();
  } catch (error) {
    setNotice("error", errorMessage(error));
  }
});

onBeforeUnmount(() => unlisteners.splice(0).forEach((unlisten) => unlisten()));
</script>

<template>
  <section class="server-workspace">
    <div v-if="notice" class="notice" :class="`notice-${notice.kind}`" role="status"><i class="bi" :class="notice.kind === 'error' ? 'bi-exclamation-triangle-fill' : notice.kind === 'success' ? 'bi-check-circle-fill' : 'bi-info-circle-fill'"></i><span>{{ notice.message }}</span><button type="button" class="icon-button notice-close" aria-label="关闭提示" @click="clearNotice"><i class="bi bi-x"></i></button></div>
    <div class="page-heading"><div><div class="eyebrow">SERVER / FRPS RUNTIME</div><h1>本机 FRP 服务端</h1><p class="page-subtitle">App 直接托管官方 frps，接受其他 frpc 客户端接入</p></div><div class="page-heading-actions"><button v-if="!status.running" type="button" class="button button-primary" :disabled="busy !== null || !canStart" @click="start"><i class="bi bi-play-fill"></i>{{ busy === 'starting' ? '正在启动' : '启动本机 frps' }}</button><button v-else type="button" class="button button-danger" :disabled="busy !== null" @click="stop"><i class="bi bi-power"></i>停止 frps</button></div></div>

    <div class="server-grid">
      <section class="panel server-profile-panel">
        <div class="panel-heading"><div><div class="panel-kicker">SERVER PROFILE</div><h2>服务端实例</h2></div><button type="button" class="button button-secondary button-small" @click="createProfile">+ 新建</button></div>
        <select aria-label="切换服务端 Profile" :value="activeProfile?.id ?? ''" @change="chooseProfile(($event.target as HTMLSelectElement).value)"><option value="" disabled>{{ profiles.length ? '选择 Server Profile' : '尚无 Server Profile' }}</option><option v-for="profile in profiles" :key="profile.id" :value="profile.id">{{ profile.name }}</option></select>
        <div class="server-state-card" :class="`state-${status.state}`"><span class="status-dot"></span><div><strong>{{ status.running ? 'frps 运行中' : status.state === 'error' ? 'frps 运行错误' : 'frps 未运行' }}</strong><small>{{ status.config_path || '尚未保存服务端配置' }}</small></div></div>
        <div class="form-actions"><button type="button" class="button button-danger button-small" :disabled="!activeProfile?.id || profiles.length <= 1 || status.running" @click="removeProfile">删除 Profile</button></div>
      </section>

      <section class="panel server-profile-panel"><div class="panel-heading"><div><div class="panel-kicker">FRPS SIDECAR</div><h2>官方服务端引擎</h2></div><span class="availability-badge" :class="sidecar?.server_available ? 'available' : 'missing'"><span class="status-dot"></span>{{ sidecar?.server_available ? '已就绪' : '缺失' }}</span></div><div class="sidecar-details"><div><span>版本</span><code>frps v0.71.0</code></div><div><span>目标架构</span><code>{{ sidecar?.server_target_triple ?? '—' }}</code></div><div><span>外部文件</span><code>{{ sidecar?.server_expected_name ?? '—' }}</code></div></div></section>
    </div>

    <section class="panel config-form"><div class="panel-heading"><div><div class="panel-kicker">FRPS CONFIGURATION</div><h2>服务端配置</h2></div><span class="form-status" :class="configured ? 'is-ready' : 'is-empty'"><span class="status-dot"></span>{{ configured ? '已保存配置' : '待导入或生成' }}</span></div><div class="field-grid"><label class="field"><span class="field-label">Server Profile 名称</span><input v-model="profileName" /></label><label class="field"><span class="field-label">配置来源</span><select v-model="nativeConfig.source"><option value="managed">导入到 App 私有副本（可编辑）</option><option value="external_readonly">引用外部 frps 配置（只读）</option></select></label></div><div v-if="nativeConfig.source === 'managed'" class="native-import"><label class="field"><span class="field-label">导入 frps.toml</span><input type="file" accept=".toml,text/plain" @change="handleConfigFile" /></label><span class="field-hint">{{ importedName || '选择现有配置，或使用服务端向导生成一份安全起始配置。' }}</span></div><label v-else class="field"><span class="field-label">外部 frps 配置绝对路径</span><input v-model="nativeConfig.config_path" placeholder="/etc/frp/frps.toml" /><span class="field-hint">App 只保存路径，不读取、改写或接管外部服务。</span></label><label v-if="nativeConfig.source === 'managed'" class="field"><span class="field-label">高级 frps TOML</span><textarea v-model="serverToml" class="code-area" spellcheck="false" aria-label="高级 frps TOML 编辑器"></textarea></label><div class="config-options"><label class="toggle-row"><input v-model="nativeConfig.auto_start" type="checkbox" /><span class="toggle-control"><span></span></span><span><strong>打开 App 后自动启动 frps</strong><small>只启动当前 Server Profile</small></span></label></div><div class="form-actions"><button type="button" class="button button-primary" :disabled="busy !== null" @click="saveProfile"><i class="bi bi-floppy"></i>保存 Server Profile</button><button type="button" class="button button-secondary" :disabled="busy !== null || !configured" @click="start"><i class="bi bi-shield-check"></i>校验并启动</button></div></section>

    <section v-if="nativeConfig.source === 'managed'" class="panel server-builder"><div class="panel-heading"><div><div class="panel-kicker">SERVER SETUP WIZARD</div><h2>常用服务端配置向导</h2></div><button type="button" class="button button-secondary button-small" @click="generateServerToml">生成 frps TOML</button></div><p class="field-hint">生成的配置默认启用 token、强制 TLS，并将 Dashboard 绑定到本机回环地址；保存后仍会由内置 frps verify 再次校验。</p><div class="field-grid"><label class="field"><span class="field-label">监听地址</span><input v-model="serverForm.bindAddr" placeholder="0.0.0.0" /></label><label class="field"><span class="field-label">bindPort</span><input v-model="serverForm.bindPort" inputmode="numeric" /></label><label class="field"><span class="field-label">客户端连接地址</span><input v-model="serverForm.serverAddr" placeholder="公网 IP / 域名 / 局域网地址" /></label><label class="field"><span class="field-label">服务端 Token</span><input v-model="serverForm.authToken" type="password" /></label><label class="field"><span class="field-label">Dashboard 地址</span><input v-model="serverForm.dashboardAddr" /></label><label class="field"><span class="field-label">Dashboard 端口</span><input v-model="serverForm.dashboardPort" inputmode="numeric" /></label><label class="field"><span class="field-label">Dashboard 用户</span><input v-model="serverForm.dashboardUser" /></label><label class="field"><span class="field-label">Dashboard 密码</span><input v-model="serverForm.dashboardPassword" type="password" /></label></div><label class="toggle-row"><input v-model="serverForm.tlsForce" type="checkbox" /><span class="toggle-control"><span></span></span><span><strong>强制客户端 TLS</strong><small>推荐开启，客户端配置需匹配 TLS</small></span></label></section>

    <section class="panel server-access-panel"><div class="panel-heading"><div><div class="panel-kicker">CLIENT ONBOARDING</div><h2>其他客户端接入配置</h2></div><button type="button" class="button button-secondary button-small" :disabled="!clientAccessToml" @click="copy(clientAccessToml, '客户端接入配置已复制')"><i class="bi bi-copy"></i>复制 TOML</button></div><p class="field-hint">这是客户端连接服务端的基础配置；每台设备仍需在 `[[proxies]]` 下添加自己的代理定义。</p><textarea class="code-area" :value="clientAccessToml || '先在服务端向导填写可达地址与 Token，再生成客户端接入配置。'" readonly spellcheck="false" aria-label="客户端接入 TOML"></textarea></section>

    <section class="panel server-log-panel"><div class="panel-heading"><div><div class="panel-kicker">FRPS LOGS</div><h2>服务端日志</h2></div><div class="page-heading-actions"><button type="button" class="button button-secondary button-small" :disabled="!logs.length" @click="copy(logs.map((entry) => entry.line).join('\n'), '服务端日志已复制')">复制</button><button type="button" class="button button-ghost button-small" :disabled="!logs.length" @click="clearLogs">清空</button></div></div><section class="log-terminal server-log-terminal"><div v-if="!logs.length" class="terminal-empty"><i class="bi bi-terminal"></i><span>暂无服务端日志</span></div><div v-for="(entry, index) in logs" :key="`${entry.ts_ms}-${index}`" class="terminal-line" :class="`stream-${entry.stream}`"><span class="terminal-time">{{ new Date(Number(entry.ts_ms)).toLocaleTimeString('zh-CN', { hour12: false }) }}</span><span class="terminal-tag">{{ entry.stream === 'stderr' ? '错误' : entry.stream === 'system' ? '系统' : '输出' }}</span><code>{{ entry.line }}</code></div></section></section>
  </section>
</template>
