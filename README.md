# frp-panel Client

面向 macOS、Linux 与 Windows 的 `frp-panel` 受管 Client 桌面工具。它提供桌面窗口、系统托盘、运行状态和实时日志，并在本机启动与平台匹配的 `frp-panel-client` sidecar。

> 社区维护的第三方项目，**不是** VaalaCat/frp-panel 官方发布的软件，也不是通用 `frpc.toml` 编辑器。

## 项目基础信息

| 项目项 | 当前值 |
| --- | --- |
| 许可证 | AGPL-3.0-only，详见 [LICENSE](LICENSE) |
| 支持平台 | macOS Apple Silicon / Intel、Linux x86_64、Windows x86_64 |
| 发布物 | macOS `.dmg`、Linux `.AppImage`、Windows NSIS `.exe` 安装程序 |
| 桌面技术栈 | Vue 3、Tauri v2、Rust |
| 受管 Client | 从固定上游 commit 构建的 `frp-panel-client` sidecar |
| Profile | 当前仅支持一个连接 Profile |
| 网络遥测 | 不包含分析、广告 SDK 或自动崩溃报告上传 |

`frp-panel` sidecar 的上游来源、固定 commit、许可证和源码获取方式见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

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

MoonProxy 面向标准 `frpc` 与 `frpc.toml`；它不能直接替代本项目所需的 `frp-panel` 受管 Client 协议。

## 功能范围

- 粘贴并解析 `frp-panel client ...` 或包含 `curl | bash ... client ...` 的安装命令。
- 保存一个连接 Profile，并启动、停止、自动重连受管 Client。
- 在系统托盘常驻，支持登录后启动应用。
- 显示 stdout、stderr 和系统状态日志。
- 校验内嵌 sidecar 是否与当前系统、CPU 架构匹配。
- 将 Client Secret 保存到 macOS Keychain、Windows Credential Manager 或 Linux Secret Service。
- 默认校验 HTTPS / WSS 证书；自签名证书只能由用户明确启用例外。
- 默认禁用上游 sidecar 的 functions 与 remote shell 功能。

不包含：多 Profile、join-token 自动注册、`frpc.toml` 编辑器、worker/remote shell/WireGuard 的专用图形配置。

## 快速开始

1. 从 GitHub Release 下载与系统和架构相符的安装包。
2. 打开应用，进入“配置”页面。
3. 从 frp-panel Web UI 复制 Client 启动命令，粘贴后点击“解析并填充”。
4. 检查 Client ID、API URL、RPC URL，保存后点击“保存并连接”。
5. 在“日志”页面确认 Client 已注册并拉取配置。

最终用户不需要自行安装 `frp-panel`，也不应在本机执行面板提供的 Linux 安装脚本。

完整操作、证书例外、更新、卸载与排障见 [使用指南](docs/USER_GUIDE.md)。

## 安全边界

- Client Secret 不写入应用 JSON 配置，保存在系统凭据库。
- 启动 sidecar 时，Secret 通过子进程环境变量传递，不出现在 sidecar 命令行参数中。
- 应用显示日志时会对当前 Secret 脱敏；分享日志前仍需手动检查 Client ID、域名和其他运行信息。
- TLS 证书校验默认开启。仅当你确认服务端使用自签名证书时，才在配置页启用“不验证 TLS 证书”。
- 本应用不请求 sudo，不写入 `/etc/frpp/.env`，也不执行粘贴命令中的 shell 内容。

详见 [PRIVACY.md](PRIVACY.md) 和 [SECURITY.md](SECURITY.md)。

## 开发与构建

开发前需要 Node.js 22、pnpm 10、Rust stable 与 Go 1.25。常用命令：

```bash
pnpm install --frozen-lockfile
pnpm sync:client
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
