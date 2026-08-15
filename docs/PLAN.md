# frp-panel Client 开发计划

## 目标

开发一个面向 macOS、Linux 与 Windows 的 `frp-panel` 图形客户端，提供托盘常驻和桌面主窗口。客户端不重写 `frp-panel` 协议，通过 Tauri sidecar 管理上游、目标平台的 `frp-panel-client` 二进制。应用解析面板生成的 `client -s ... -i ... --api-url ... --rpc-url ...` 命令，但运行时将 Secret 通过环境变量传递，避免出现在进程参数中。

## 非目标

- 不做 `frp-panel` Master / Server 管理端。
- 不在第一版实现标准 `frpc.toml` 手工隧道编辑；这是 MoonProxy 的主要能力，不是本项目核心。
- 不默认安装 LaunchDaemon、systemd 或写 `/etc/frpp/.env`。
- 不在第一版实现 `join-token` 自动注册。原因是 `frp-panel join` 当前会保存到 `/etc/frpp/.env`，在 macOS 桌面应用里需要额外权限与安全设计。
- 不重写 frp 协议、gRPC 协议、远程 shell、worker 或 WireGuard 能力。

## 技术选型

- 桌面框架：Tauri v2。
- 后端：Rust，负责 sidecar 管理、配置存储、状态栏、日志事件。
- 前端：Vue 3 + TypeScript + Vite。
- UI 风格：深色开发者工具风格，主色 `#22C55E` 表示可运行状态，背景 `#0F172A`，文本 `#F8FAFC`。
- 跨平台常驻：Tauri tray icon + 菜单项。

## 运行模型

1. 用户从 `frp-panel` Web UI 复制 Client 启动命令。
2. 应用解析命令中的：
   - `client_id`
   - `client_secret`
   - `api_url`
   - `rpc_url`
3. 应用保存配置到本机应用配置目录。
4. 点击连接后，Rust 后端启动内置 sidecar：

   ```bash
   CLIENT_SECRET=<client_secret> \
   CLIENT_TLS_INSECURE_SKIP_VERIFY=false \
   frp-panel-client client \
     -i <client_id> \
     --api-url <api_url> \
     --rpc-url <rpc_url>
   ```

5. 后端采集 stdout/stderr，推导连接状态并向前端推送事件。
6. 用户关闭窗口时应用隐藏到状态栏，进程继续运行；从菜单可显示窗口或断开连接。

## 状态定义

- `stopped`：未启动 sidecar。
- `starting`：sidecar 已启动，正在连接 Master。
- `running`：进程仍在运行，并观测到连接/拉取配置相关成功日志。
- `error`：启动失败、进程异常退出，或日志出现明确错误。

第一版状态以进程存活和日志关键词推导，不直接调用 `frp-panel` 内部 API。后续可扩展 gRPC/HTTP 健康检查。

## 安全与凭据

- 非敏感连接元数据保存在本机应用配置目录；Client secret 只保存在系统凭据库，不上传到第三方服务。
- 首次加载旧版本明文 `connections.json` 时，将 Secret 迁移到系统凭据库并清理原字段；凭据库写入失败时不回退到明文存储。
- UI 默认隐藏 secret。
- 日志展示会对当前 secret 做脱敏。
- Secret 通过 `CLIENT_SECRET` 环境变量传给 sidecar，不作为命令行参数。
- TLS 证书校验默认开启；自签名证书例外仅由用户显式启用。
- 第一版不请求 sudo、不写 `/etc/frpp/.env`。
- 默认注入环境变量关闭非核心能力：
  - `CLIENT_FEATURES_ENABLE_FUNCTIONS=false`
  - `CLIENT_FEATURES_ENABLE_REMOTE_SHELL=false`

## 启动策略

- `auto_connect`：打开应用后自动启动 sidecar。
- `launch_at_login`：使用系统自动启动入口在用户登录后启动应用；只有同时启用 `auto_connect` 时，登录启动后才会自动连接。

## 开源与发布基线

- 根目录使用 AGPL-3.0-only，并由 `NOTICE` 和 `THIRD_PARTY_NOTICES.md` 记录上游 sidecar 的来源、固定 commit 和许可证。
- CI 固定 Actions 和工具链版本，执行四平台 bundle 验证并保留 workflow artifact。
- 依赖更新由 Dependabot、JS/Rust 审计和 Rust 许可证/来源检查覆盖。
- Release workflow 生成 SHA256、SPDX SBOM 和 provenance attestation；正式分发仍需配置 Apple notarization 与 Windows Authenticode。

## UI 信息架构

- 首页：当前状态、连接/断开按钮、最近错误、核心配置摘要。
- 配置页：粘贴面板启动命令、单独编辑四个连接字段、保存配置。
- 日志页：实时日志、清空日志、复制日志。
- 关于页：sidecar 状态、下载提示、项目说明。
- 状态栏菜单：显示窗口、连接/断开、退出。

## 里程碑

### M1：项目骨架

- 建立 Tauri + Vue + TypeScript 项目。
- 配置 macOS app、状态栏、sidecar capability。
- 写入计划、架构、任务清单文档。

### M2：核心后端

- 实现配置保存/读取。
- 实现粘贴命令解析。
- 实现 sidecar 路径检查。
- 实现启动/停止进程。
- 实现日志环形缓冲和事件推送。
- 实现状态查询。

### M3：桌面 UI

- 实现状态卡片。
- 实现配置表单。
- 实现日志面板。
- 实现错误反馈、loading 状态、可访问标签。

### M4：交付辅助

- 增加下载 `frp-panel-client` Darwin 二进制的脚本。
- 增加 README 使用说明。
- 增加构建说明和验证命令。

### M5：验证

- `pnpm install`。
- `pnpm build`。
- `cargo check` 或 `pnpm tauri build`。
- 代码格式检查。

## 跨平台交付

- macOS：Apple Silicon 与 Intel 的 `.app/.dmg`。
- Linux：x86_64 `.AppImage`。
- Windows：x86_64 NSIS `.exe` 安装程序。
- 每个交付物内置匹配的 `frp-panel-client` sidecar；Windows 安装后的 sidecar 与主程序同目录保留。

## 验收标准

- 应用可以保存并加载 `frp-panel` Client 连接配置。
- 应用可以从面板启动命令中解析必要参数。
- 应用可以启动和停止 `frp-panel-client` sidecar。
- 应用能实时显示日志，并对 secret 脱敏。
- 系统托盘菜单可以显示窗口、连接/断开、退出。
- 每个平台仅打包该目标所需的 sidecar。
- 文档明确说明如何准备各目标 sidecar、如何从面板复制命令、如何运行开发环境。
