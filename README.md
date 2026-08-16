# frp-panel Client

面向 macOS、Linux 与 Windows 的 FRP 桌面运行壳。它提供桌面窗口、系统托盘、运行状态和实时日志，可在本机启动 `frp-panel-client` 受管 Client；macOS 另提供官方 `frpc` 原生客户端模式和本机 `frps` 服务端模式。

> 社区维护的第三方项目，**不是** VaalaCat/frp-panel 官方发布的软件。原生 frpc 模式是运行器优先的配置工具，不承诺替代官方完整配置参考。

## 项目基础信息

| 项目项 | 当前值 |
| --- | --- |
| 许可证 | AGPL-3.0-only，详见 [LICENSE](LICENSE) |
| 支持平台 | macOS Apple Silicon / Intel、Linux x86_64、Windows x86_64 |
| 发布物 | macOS `.dmg`、Linux `.AppImage`、Windows NSIS `.exe` 安装程序 |
| 桌面技术栈 | Vue 3、Tauri v2、Rust |
| frp-panel 引擎 | 从固定上游 commit 构建的 `frp-panel-client` sidecar |
| 原生 frp 引擎 | macOS 内嵌、SHA-256 校验的官方 `frpc` / `frps v0.71.0` sidecar |
| Profile | 支持一个面板 Profile、多个原生 `frpc` Profile 和独立的本机 `frps` Server Profile |
| 网络遥测 | 不包含分析、广告 SDK 或自动崩溃报告上传 |

两个 sidecar 的上游来源、固定版本/commit、校验和与许可证见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

## 它解决什么问题

`frp-panel` 的受管 Client 由 Master 下发配置并保持 RPC 控制连接，启动参数通常为：

```bash
frp-panel client \
  -s <client-secret> \
  -i <client-id> \
  --api-url <api-url> \
  --rpc-url <rpc-url>
```

本应用解析其中的连接参数，在系统凭据库保存 Client Secret，并使用内嵌 sidecar 运行官方受管 Client。它不会自行实现 `frp-panel` 管理协议，也不会执行你粘贴的 shell 安装脚本。

如果电脑上已经通过命令行安装并运行了 `frp-panel client`，应用也可以作为桌面壳进行只读适配：检测 `PATH` / 常用目录中的二进制、正在运行的 Client 进程，以及 macOS LaunchAgent；显示 PID、运行时长、Client ID 与 API/RPC URL。它不会读取外部进程的 Secret、接管或停止外部进程，也不会获取外部进程的 stdout/stderr。发现相同 `client_id` 的外部 Client 时，应用会阻止再次启动内置 Client，避免重复注册。

MoonProxy 面向标准 `frpc` 与 `frpc.toml`；它不能直接替代本项目所需的 `frp-panel` 受管 Client 协议。

从本版本起，本应用也可作为标准 `frpc` 的 macOS 运行壳：导入或生成 App 托管的 `frpc.toml`，启动前执行 `frpc verify -c`，再使用官方 sidecar 执行 `frpc -c`。此模式直接连接配置中的 `frps`，**不会**连接或使用 frp-panel Master RPC。

macOS 还可以直接作为 `frps` 宿主：在“本机 frps”页面导入或生成 `frps.toml`，先执行 `frps verify -c` 再启动 `frps -c`。页面可生成其他设备的基础 `frpc.toml` 接入配置，并通过本机托管 Dashboard 的只读 API 展示已连接客户端、代理在线状态和流量概览。该配置负责客户端接入地址、Token 与 TLS；每台客户端自己的 `[[proxies]]` 仍由其客户端配置决定。

## 功能范围

- 粘贴并解析 `frp-panel client ...` 或包含 `curl | bash ... client ...` 的安装命令。
- 保存并切换多个 Profile：frp-panel 受管 Profile 与原生 `frpc` Profile 分离。
- macOS 原生 `frpc`：导入 TOML 到应用私有目录、启动前校验、启动/停止、日志与状态栏控制。
- macOS 本机 `frps`：独立 Server Profile、导入/编辑/生成 `frps.toml`、启动前校验、启动/停止、服务端日志与托盘控制。
- Dashboard 概览：只读显示本机托管 `frps` 的在线客户端、代理状态、连接数和流量；默认每 10 秒刷新，也可手动刷新。
- 生成给其他设备使用的基础 `frpc.toml` 接入配置，包含服务端地址、端口和 Token，不执行远程命令或接管其他客户端。
- 原生模式提供 TCP、UDP、HTTP、HTTPS 的单代理 TOML 起始配置生成器；高级字段保留在 TOML 编辑器中。
- 在系统托盘常驻，支持登录后启动应用。
- 显示 stdout、stderr 和系统状态日志。
- 校验内嵌 sidecar 是否与当前系统、CPU 架构匹配。
- 只读发现系统已安装、已运行及已设置启动项的 `frp-panel` / `frp-panel-client` 与 `frpc`；原生 `frpc` 只显示 PID、二进制与 `-c/--config` 路径，不读取配置内容或密钥。
- 将 Client Secret 保存到 macOS Keychain、Windows Credential Manager 或 Linux Secret Service。
- 默认校验 HTTPS / WSS 证书；自签名证书只能由用户明确启用例外。
- 默认禁用上游 sidecar 的 functions 与 remote shell 功能。

不包含：逐客户端远程配置下发或吊销、join-token 自动注册、完整的所有 frp proxy/plugin/visitor 图形表单、外部 `frpc` 接管/重载、worker/remote shell/WireGuard 专用图形配置。Dashboard 数据只读，不会向其他设备执行命令或改写其配置。

## 快速开始

1. 从 GitHub Release 下载与系统和架构相符的安装包。
2. 打开应用，进入“配置”页面。
3. 从 frp-panel Web UI 复制 Client 启动命令，粘贴后点击“解析并填充”。
4. 检查 Client ID、API URL、RPC URL，保存后点击“保存并连接”。
5. 在“日志”页面确认 Client 已注册并拉取配置。

默认情况下，最终用户不需要自行安装 `frp-panel` 或 `frpc`。若你已经有可工作的命令行 Client，可以直接打开本应用，在“总览”的外部进程区域查看它，无需重复安装；外部实例始终由原命令或 LaunchAgent 管理。

若希望其他设备接入这台 Mac，请打开“本机 frps”，生成并保存服务端配置，校验后启动。把“客户端接入配置”复制到其他设备的 `frpc.toml`，再按需为每台客户端添加 `[[proxies]]`。跨公网使用时，客户端必须能访问这台 Mac 的实际地址和 `bindPort`；请同时配置防火墙、路由或 VPN。

完整操作、证书例外、更新、卸载与排障见 [使用指南](docs/USER_GUIDE.md)。

## 安全边界

- Client Secret 不写入应用 JSON 配置，保存在系统凭据库。
- 启动 sidecar 时，Secret 通过子进程环境变量传递，不出现在 sidecar 命令行参数中。
- 应用显示日志时会对当前 Secret 脱敏；分享日志前仍需手动检查 Client ID、域名和其他运行信息。
- TLS 证书校验默认开启。仅当你确认服务端使用自签名证书时，才在配置页启用“不验证 TLS 证书”。
- App 托管的原生 TOML 位于应用私有目录并使用用户私有权限；该 TOML 可能含 `auth.token`，因此不应提交、复制或分享到公共位置。
- 外部原生 frpc 配置不会被应用读取、复制、修改、reload 或停止。
- 本应用不请求 sudo，不写入 `/etc/frpp/.env`，也不执行粘贴命令中的 shell 内容。

详见 [PRIVACY.md](PRIVACY.md) 和 [SECURITY.md](SECURITY.md)。

## 开发与构建

开发前需要 Node.js 22、pnpm 10、Rust stable 与 Go 1.25。常用命令：

```bash
pnpm install --frozen-lockfile
pnpm sync:client
pnpm sync:frpc
pnpm sync:frps
pnpm tauri dev

pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```

不同平台的 bundle 命令：

```bash
pnpm bundle:mac
pnpm bundle:linux
pnpm bundle:windows
```

完整的架构、代码分层、sidecar 构建、二次开发约束和测试流程见 [开发指南](docs/DEVELOPMENT.md)。发布维护者请阅读 [发布指南](docs/RELEASING.md)。

## 参与和支持

- 贡献流程： [CONTRIBUTING.md](CONTRIBUTING.md)
- 社区行为规范： [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- 安全漏洞报告： [SECURITY.md](SECURITY.md)
- 使用支持： [SUPPORT.md](SUPPORT.md)
- 版本变更： [CHANGELOG.md](CHANGELOG.md)

提交 issue、截图或日志时，**不要**粘贴 Client Secret、Cookie、Token、生产服务器完整地址或私有配置文件。
