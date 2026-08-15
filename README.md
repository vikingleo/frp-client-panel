# frp-panel Client

面向 macOS、Linux 与 Windows 的 `frp-panel` 受管 Client 桌面工具。它通过 Tauri 管理由官方 `frp-panel` 源码构建的目标平台 `frp-panel-client` sidecar，提供主窗口、实时日志和托盘常驻控制。

> 这是 `frp-panel` 的专用受管 Client 壳，不是通用 `frpc.toml` 编辑器。

## 与 MoonProxy 的关系

MoonProxy 管理的是标准 `frpc`，生成并启动 `frpc.toml`。它不能直接接入 `frp-panel` 的受管 Client 控制协议。

本项目执行 `frp-panel` Web UI 生成的 Client 命令：

```bash
frp-panel client \
  -s <client-secret> \
  -i <client-id> \
  --api-url <api-url> \
  --rpc-url <rpc-url>
```

应用不自行实现 frp-panel 的 gRPC / WebSocket 管理协议，而是启动官方 `frp-panel-client` sidecar，以保持与面板下发配置、客户端控制和协议更新的兼容性。

## 平台与交付物

| 平台 | 架构 | 交付物 | 内嵌 sidecar |
| --- | --- | --- | --- |
| macOS | Apple Silicon | `.app` / `.dmg` | `aarch64-apple-darwin` |
| macOS | Intel | `.app` / `.dmg` | `x86_64-apple-darwin` |
| Linux | x86_64 | `.AppImage` | `x86_64-unknown-linux-gnu` |
| Windows | x86_64 | NSIS `.exe` 安装程序 | `x86_64-pc-windows-msvc.exe` |

Linux AppImage 是单文件可执行交付物。Windows 交付物是单个 `.exe` 安装程序；安装后主程序与 `frp-panel-client.exe` 会保留在安装目录中，这是 Tauri 外置 sidecar 启动模型所必需的，不是“单个可携带 EXE”。

## 当前能力

- 跨平台主窗口与托盘常驻；关闭窗口后进程继续在系统托盘运行。
- 粘贴 `frp-panel client ...` 或 `curl | bash -s -- client ...` 文本，解析 Client ID、Secret、API URL、RPC URL。
- 安装命令仅用于提取连接参数；应用不会下载或执行其中的脚本。
- 在本机应用配置目录保存单个连接 Profile。
- 启动、停止与当前上游源码标签对应的目标平台 `frp-panel-client` sidecar。
- 实时显示 stdout、stderr、系统日志；当前 Client Secret 会由 Rust 后端脱敏。
- 启动前检查 sidecar 与当前平台是否匹配；安装包损坏或 sidecar 缺失时给出重装提示。
- Client Secret 使用系统凭据库：macOS Keychain、Windows Credential Manager、Linux Secret Service 兼容实现。

## 终端用户使用

1. 下载与本机系统和架构相符的安装包。
2. 打开应用，在“配置”页粘贴面板生成的 `frp-panel client ...` 连接命令。
3. 点击“解析并填充”，检查四个参数后保存并连接。
4. 在“日志”页确认 Client 已拉取配置并进入运行状态。

最终用户无需在系统中另行安装 `frp-panel`，也不应执行 Linux 的安装脚本。若“关于”页提示内置客户端缺失，请重新安装相符的安装包；`pnpm sync:client` 仅用于项目开发。

## 开发要求

- Node.js 20+ 与 pnpm 10+。
- Rust stable toolchain。
- Go 1.25+，仅用于开发、构建或重新同步 sidecar；Go 1.24 构建的 macOS Intel sidecar 已知会在启动时崩溃。
- Linux 打包额外需要 WebKitGTK 4.1、GTK 3、AppIndicator、librsvg 与 `patchelf`；CI 会自动安装。
- 一个已部署的 `frp-panel` 实例与从其 Web UI 复制的 Client 命令。

## 开发运行

```bash
pnpm install
pnpm sync:client
pnpm tauri dev
```

`pnpm sync:client` 默认为当前宿主平台构建 sidecar。要在任意开发机上准备其他交付目标，请指定 Tauri target triple：

```bash
FRP_PANEL_TARGET_TRIPLE=aarch64-apple-darwin pnpm sync:client
FRP_PANEL_TARGET_TRIPLE=x86_64-apple-darwin pnpm sync:client
FRP_PANEL_TARGET_TRIPLE=x86_64-unknown-linux-gnu pnpm sync:client
FRP_PANEL_TARGET_TRIPLE=x86_64-pc-windows-msvc pnpm sync:client
```

同步脚本默认固定检出上游 commit `1a58b856d7de19de8669b7072872986d2fa1604a`，并以 `CGO_ENABLED=0` 构建，以保证本地和 CI 可复现。可显式指定 branch、tag 或 commit：

```bash
FRP_PANEL_SOURCE_REF=latest pnpm sync:client
FRP_PANEL_SOURCE_REF=<branch-or-tag-or-commit> pnpm sync:client
```

若仅为排查上游预编译文件而需要下载 Release asset，可显式选择非默认模式：

```bash
FRP_PANEL_BUILD_MODE=release pnpm sync:client
```

`pnpm verify:client` 会验证 binary 格式、架构和 Tauri 文件命名；在同平台 runner 上还会执行 `client --help` 冒烟检查。

## 构建交付物

在对应的原生平台上准备 sidecar 后执行：

```bash
# macOS：.app 与 .dmg
pnpm bundle:mac

# Linux x86_64：.AppImage
pnpm bundle:linux

# Windows x86_64：NSIS .exe 安装程序
pnpm bundle:windows
```

构建产物位于 `src-tauri/target/**/release/bundle/`。macOS 本地构建使用 ad-hoc 签名；面向外部用户分发前，仍需配置 Apple Developer ID 与 notarization。Windows 面向外部用户分发前也应配置 Authenticode 代码签名。

## CI 与发布

- `.github/workflows/ci.yml` 在 macOS、Linux 与 Windows 原生 runner 上构建匹配 target 的 sidecar，运行前端/Rust 检查，并生成 DMG、AppImage 与 NSIS EXE。
- `.github/workflows/release.yml` 在推送 `v*` tag 或手动触发时创建 GitHub draft Release，并上传四类交付物。
- 每个 job 都会先验证 sidecar，避免将错误架构的 Client 打进安装包。

准备发布时推送例如 `v0.1.0` 的 tag。GitHub Actions 会创建草稿 Release，审核 `.dmg`、`.AppImage`、`.exe` 后再手动发布。

## 安全说明

- 非敏感连接配置保存在应用配置目录；`client_secret` 保存在系统凭据库，不上传至第三方服务。
- 从旧 macOS 版本迁移时，先尝试读取原 Keychain service，再迁移到通用的 `app.frppanel.client` service；配置文件中的遗留明文 Secret 会被清理。
- 系统凭据库不可用或保存失败时，程序会报错，不会静默回退到明文存储。
- UI 默认隐藏 Secret；实时日志会将当前 Client Secret 替换为 `******`。
- 程序不会请求 sudo，也不会写 `/etc/frpp/.env`。
- sidecar 启动时默认关闭 functions 与 remote shell：
  - `CLIENT_FEATURES_ENABLE_FUNCTIONS=false`
  - `CLIENT_FEATURES_ENABLE_REMOTE_SHELL=false`

## 当前限制

- 仅支持一个连接 Profile。
- 不自动执行 `join-token` 注册。
- 不提供标准 `frpc.toml` 隧道编辑器。
- 不包含 worker、remote shell、WireGuard 等额外能力的专用 UI。
- Linux 当前发布 x86_64 AppImage，Windows 当前发布 x86_64 NSIS EXE。

## 验证命令

```bash
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
pnpm verify:client
pnpm verify:bundle
```

更多实现约束见 `docs/PLAN.md`、`docs/ARCHITECTURE.md` 和 `docs/TASKS.md`。
