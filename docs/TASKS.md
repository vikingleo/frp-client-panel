# 任务清单

## 已确认事实

- MoonProxy 不直接支持 `frp-panel` 受管 Client 接入。
- `frp-panel` 有兼容标准 frp client 的模式，但不能替代完整面板受管 Client。
- `frp-panel` 上游源码可用 `CGO_ENABLED=0` 构建 Darwin、Linux 与 Windows Client；各平台运行时以源码构建物作为可靠交付路径。

## M1：项目骨架

- [x] 创建 `package.json`、Vite、TypeScript 配置。
- [x] 创建 `src-tauri/Cargo.toml`、`tauri.conf.json`、capability。
- [x] 创建 Vue 入口文件。
- [x] 创建 Rust 入口文件。

## M2：Rust 后端

- [x] 定义 `ConnectionConfig`、`RuntimeStatus`、`LogEntry`。
- [x] 实现配置保存与加载。
- [x] 实现命令解析。
- [x] 实现 sidecar 可用性检查。
- [x] 实现启动。
- [x] 实现停止。
- [x] 实现日志采集和脱敏。
- [x] 实现状态事件。
- [x] 实现托盘菜单。

## M3：前端 UI

- [x] 实现状态面板。
- [x] 实现配置表单。
- [x] 实现粘贴命令解析。
- [x] 实现日志面板。
- [x] 实现关于/安装提示。
- [x] 实现 loading、错误提示、可访问标签。

## M4：脚本和文档

- [x] 写 `scripts/sync-frp-panel-client.sh`。
- [x] 写 README。
- [x] 写开发和构建说明。

## M5：验证

- [x] `pnpm install`。
- [x] `pnpm build`。
- [x] `cargo fmt --check`。
- [x] `cargo check`。
- [x] 检查 sidecar 缺失时 UI 提示明确。

## M6：发布自动化

- [x] 添加 sidecar 架构、可执行权限与同架构 CLI 冒烟校验脚本。
- [x] 添加 macOS CI：安装依赖、从上游源码构建 host sidecar、前端/Rust 验证。
- [x] 添加 Apple Silicon 与 Intel 原生 Release draft 构建矩阵。
- [x] 在 Intel 主机实际完成 x86_64 与 aarch64 `.app/.dmg` 构建。
- [ ] 推送到 GitHub 仓库并执行一次云端 CI / Release 验证。

## M7：安全与启动加固

- [x] 使用 macOS Keychain 存储 Client Secret（后续已迁移为跨平台系统凭据库）。
- [x] 自动迁移旧 Store 中的明文 Secret 并清理字段。
- [x] 添加“登录后启动应用”配置，并与 macOS LaunchAgent 同步。
- [x] 收紧 production/dev CSP，并验证 Tauri 开发启动。
- [x] 完成 Keychain 迁移、x64/ARM64 回归验证。

## M8：sidecar macOS 兼容性

- [x] 复现并记录上游预编译 Intel Client 的运行时崩溃。
- [x] 默认从上游源码以 `CGO_ENABLED=0` 构建 Darwin sidecar。
- [x] 在同架构主机增加 `frp-panel client --help` 冒烟校验。
- [x] 将默认上游源码 ref 固定到已验证 commit，保持 release 可复现。
- [x] 重新构建并验证采用源码 sidecar 的 x64 / ARM64 DMG。

## M9：跨平台交付

- [x] 扩展 sidecar 同步/验证脚本至 Linux x86_64 与 Windows x86_64。
- [x] 生成并验证 Linux ELF 与 Windows PE sidecar。
- [x] 扩展运行时 target triple 识别、sidecar 命名和系统凭据库文案。
- [x] 使用跨平台 autostart Builder，保留 macOS LaunchAgent 配置。
- [x] 添加 Linux AppImage 与 Windows NSIS EXE Tauri bundle 配置。
- [x] 添加 macOS、Linux、Windows 原生 CI / Release 构建矩阵。
- [ ] 在 GitHub Actions 实际完成一次 Linux AppImage 与 Windows NSIS EXE 云端构建验证。

## 当前限制

- 只支持一个连接 Profile。
- 不自动执行 `join-token` 注册。
- 不做 Apple notarization。
- 不保证 worker、remote shell、WireGuard 等高级能力在桌面图形壳里可用。
