# 架构说明

## 总览

本项目是一个跨平台桌面壳，核心职责是可靠管理由上游 `frp-panel` 源码构建的目标平台 `frp-panel-client`。

```text
Vue UI
  │ invoke / events
  ▼
Tauri Rust 后端
  ├─ config：连接配置持久化
  ├─ command_parser：解析面板启动命令
  ├─ discovery：只读发现系统二进制、外部进程和启动项
  ├─ process：sidecar 启停、日志采集、状态推导
  ├─ tray：系统托盘菜单
  └─ sidecar：选择/检查 frp-panel-client 二进制
      │
      ▼
frp-panel-client sidecar
      │ api-url / rpc-url
      ▼
frp-panel Master
```

## 两种运行模式

应用支持两种互不混淆的模式：

1. **内置托管模式**：Rust 后端启动随应用打包的 `frp-panel-client` sidecar，保存其 child handle、采集 stdout/stderr、支持停止和托盘控制。这是默认模式。
2. **外部只读观测模式**：发现用户已经通过命令行、脚本或 macOS LaunchAgent 启动的 `frp-panel client` / `frp-panel-client client`。应用只显示可安全提取的状态字段，不能附加到其 stdout/stderr，也不会停止、接管或改写它。

外部进程的命令行可能包含 `-s` / `--secret`。发现逻辑只记录“是否存在 Secret 参数”，绝不复制、显示、持久化或写日志记录其值。若外部 Client 的 `client_id` 与当前 Profile 相同，`process` 模块会拒绝启动第二个内置 Client，防止向 Master 重复注册。

macOS 下，`discovery` 会只读扫描用户和系统 LaunchAgent 目录：

```text
~/Library/LaunchAgents
/Library/LaunchAgents
```

启动项存在并不代表其进程正在运行；运行状态仍以进程发现为准。应用自身的“登录后启动”LaunchAgent 不会被当作外部 `frp-panel` Client，也不会被外部发现功能修改。

## 为什么不直接用 MoonProxy

MoonProxy 是标准 `frpc` 图形客户端。它生成 `frpc.toml` 并启动 `frpc`，适用于自带 `frps` 的普通 frp 场景。

`frp-panel` 的受管 Client 使用的是：

```bash
frp-panel client -s <secret> -i <client-id> --api-url <url> --rpc-url <url>
```

该进程会向 Master 拉配置，并响应面板的启动、停止、更新等控制。MoonProxy 没有这层 `frp-panel` Master 控制协议。

## 为什么封装上游 Client

封装官方 Client 源码构建物可以保持协议兼容，避免重新实现：

- gRPC / WebSocket RPC 连接；
- Master API / RPC 协议逻辑；
- 动态配置拉取；
- 多 server/client 关系；
- 面板远程控制消息；
- frp client service 更新。

## Sidecar 命名

Tauri externalBin 使用基础名 `frp-panel-client`，构建或开发时实际文件按目标平台命名：

- macOS Apple Silicon：`src-tauri/binaries/frp-panel-client-aarch64-apple-darwin`
- macOS Intel：`src-tauri/binaries/frp-panel-client-x86_64-apple-darwin`
- Linux x86_64：`src-tauri/binaries/frp-panel-client-x86_64-unknown-linux-gnu`
- Windows x86_64：`src-tauri/binaries/frp-panel-client-x86_64-pc-windows-msvc.exe`

同步脚本默认固定检出已验证的官方 `frp-panel` commit `1a58b856d7de19de8669b7072872986d2fa1604a`，并使用 `CGO_ENABLED=0` 交叉编译到目标 OS/架构；构建完成后会在原生 runner 上执行 `client --help` 冒烟检查。这样可在打包前发现预编译包与目标运行时之间的兼容性问题，同时保证 release 可复现。更新时可通过 `FRP_PANEL_SOURCE_REF` 显式指定新 ref。

## 配置文件

连接元数据使用 `tauri-plugin-store` 保存到应用配置目录的 `connections.json`；`client_secret` 使用系统凭据库保存，不写入 Store。

旧版本 Store 中的明文 `client_secret` 会在首次成功加载时迁移到系统凭据库并从 JSON 清除。原 macOS Keychain service 会在首次读取时迁移到通用 service。凭据库不可用时，加载或保存会明确报错，不会回退到明文持久化。

字段：

- `client_id`
- `client_secret`
- `api_url`
- `rpc_url`
- `auto_connect`
- `launch_at_login`
- `allow_insecure_tls`

语义：

- `auto_connect` 决定应用启动后是否拉起 sidecar。
- `launch_at_login` 由 Tauri autostart 插件同步为各系统的自动启动入口；它只决定应用是否随登录启动。
- `allow_insecure_tls` 默认 `false`。仅为自签名证书部署提供显式例外；启动 sidecar 时映射为 `CLIENT_TLS_INSECURE_SKIP_VERIFY`。

## Secret 与 TLS 传递

桌面壳从面板命令提取 Secret，但不将它写入 Store，也不把它传为
sidecar 的 `-s` 命令行参数。启动时由 Rust 后端设置：

```text
CLIENT_SECRET=<credential-store value>
CLIENT_TLS_INSECURE_SKIP_VERIFY=false
CLIENT_FEATURES_ENABLE_FUNCTIONS=false
CLIENT_FEATURES_ENABLE_REMOTE_SHELL=false
```

其中 TLS 跳过校验只会在用户明确启用自签名证书例外时变为 `true`。

## 状态推导

进程状态是权威基础：

- child handle 存在：进程运行中。
- child handle 不存在：停止或已退出。

外部 Client 不进入 `AppRuntime.child`。其“正在运行”状态由操作系统进程表推导，默认每 10 秒刷新一次，也可以由用户手动刷新；这与内置 sidecar 的日志状态保持分离。

日志状态作为增强信号：

- 启动成功后进入 `starting`。
- 观测到 `pull client config success`、`start to run client` 等日志进入 `running`。
- 观测到 `failed`、`error`、`fatal`、`cannot` 等错误日志进入 `error`。

## 托盘行为

- 左键点击状态栏图标显示主窗口。
- 菜单项：
  - 显示窗口
  - 连接 / 断开
  - 退出
- 窗口关闭默认隐藏，除非用户从菜单退出。

## 前端权限边界

- CSP 在 production 环境中只允许本地资源与 Tauri IPC；开发环境额外允许 Vite HMR。
- Capability 只允许窗口管理、autostart 和受限的 `frp-panel-client` sidecar 启停；前端不能直接向 sidecar 传递任意参数，参数由 Rust 后端构造。未授予外部 URL/文件打开或前端 Store 读写权限。

## 后续增强路线

1. 支持 `join-token`：通过应用私有 `.env` 或直接调用 API 实现自动注册。
2. 支持状态健康检查：调用 Master API 或读取本地状态。
3. 支持 sidecar 自动更新：固定上游源码 commit 并记录/校验构建产物 SHA256。
4. 支持多连接 Profile。
5. 支持菜单栏 popover 小窗。
