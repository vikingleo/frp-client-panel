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
- [x] 推送到 GitHub 仓库并执行一次云端 CI 验证。

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
- [x] 在 GitHub Actions 实际完成一次 Linux AppImage 与 Windows NSIS EXE 云端构建验证。

## M10：开源治理与安全发布

- [x] 添加 AGPL-3.0-only 许可证、NOTICE 与第三方 sidecar 声明。
- [x] 添加贡献、行为准则、安全、支持、隐私、用户和二次开发文档。
- [x] 默认启用 TLS 证书校验，并让自签名证书例外显式可见。
- [x] 将 Client Secret 从 sidecar 命令行参数迁移到子进程环境变量。
- [x] 收紧 production CSP、关闭生产 devtools feature、禁止前端任意 sidecar 参数。
- [x] 添加 Dependabot、JS/Rust 审计、许可证/来源检查和 CI artifact 上传流程。
- [x] 为 Release 添加 SHA256、SBOM 与 provenance attestation 流程。
- [ ] 在 GitHub Actions 验证新的安全 workflow 和 CI artifact 上传。
- [ ] 以预发布 tag 验证一次 Draft Release 的校验和、SBOM 与 provenance。
- [ ] 在配置 Apple Developer ID、notarization 和 Authenticode 凭据后完成一次签名 Release 演练。

## M11：本机 frps 服务端与 Dashboard

- [x] 内置官方 macOS `frps` sidecar，并按目标架构校验版本、格式和 SHA-256。
- [x] 添加独立 Server Profile、托管/外部只读配置、启动前 `frps verify` 和托盘启停。
- [x] 添加服务端配置向导、客户端接入模板和服务端日志脱敏。
- [x] 接入 FRP 0.71.0 Dashboard v2 只读 API，显示系统流量、客户端、代理和在线状态。
- [x] 使用实际 `frps` + `frpc` 联调验证在线客户端、在线代理和远程端口展示。
- [ ] 设计基于 Agent/OIDC 的逐客户端授权、吊销和远程配置；共享 Token 模式不提供此能力。

## 当前限制

- 只支持一个 `frp-panel` 受管 Profile；原生 `frpc` 与本机 `frps` 可分别保存多个 Profile。
- 不自动执行 `join-token` 注册。
- 不做 Apple notarization。
- 不保证 worker、remote shell、WireGuard 等高级能力在桌面图形壳里可用。
- Dashboard 仅查询 App 托管的 `frps` 配置；外部只读服务端不会被读取、接管或远程管理。
