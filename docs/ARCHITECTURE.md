# 架构说明

## 总览

本项目是一个桌面运行壳：它不重新实现 frp-panel Master 协议或 frp 协议，而是安全地管理两个上游引擎。

```text
Vue UI
  │ Tauri invoke / events
  ▼
Rust 后端
  ├─ config / server_config：Client Profile、Server Profile、Keychain 与托管 TOML
  ├─ command_parser：解析 frp-panel 连接命令
  ├─ discovery：外部进程、二进制、LaunchAgent 的只读发现
  ├─ process / server_process：客户端与本机服务端的 verify、启动/停止、日志与运行状态
  ├─ server_dashboard：本机托管 frps 的 Dashboard 只读查询与响应最小化映射
  ├─ tray：系统托盘
  └─ sidecar：目标架构与受限二进制执行
      ├─ frp-panel-client → API/RPC → frp-panel Master
      ├─ frpc → frpc.toml → frps
      └─ frps → frps.toml → 接受其他 frpc 客户端
```

## Profile 与运行模式

Profile 元数据保存在 `profiles.json`。旧版单连接 `connections.json` 会在首次读取时迁移为 `panel-default` Profile；旧 Client Secret 仍在系统凭据库，绝不复制到 Profile JSON。

| Mode | 引擎 | 配置 | 运行边界 |
| --- | --- | --- | --- |
| `panel_managed` | `frp-panel-client` | Client ID、Secret、API/RPC URL | 接收 frp-panel Master 下发的配置 |
| `native_frpc` / `managed` | 官方 `frpc` | App 私有目录中的 `frpc.toml` | 保存时可编辑；启动前强制 `frpc verify -c` |
| `native_frpc` / `external_readonly` | 外部或 App 内置 `frpc` | 用户指定的外部路径 | App 只记录路径，不读取、修改、reload 或停止外部实例 |

原生模式直接连接 `frpc.toml` 中配置的 frps，**不会**连接 frp-panel Master RPC。App 托管原生 Profile 的固定流程是：

```text
保存 TOML → frpc verify -c <config> → frpc -c <config> → stdout/stderr → 托盘和桌面状态
```

`reload` 不在首版自动执行：它依赖用户配置的 `webServer`，且全局连接参数变更不能可靠地仅靠 reload 生效。保存后由用户重新启动，避免产生“看似已应用、实际未生效”的状态。

本机服务端 Profile 单独保存在 `servers.json`，不会占用或切换客户端 Profile。它的固定流程是：

```text
保存 frps TOML → frps verify -c <config> → frps -c <config> → stdout/stderr → 托盘和桌面状态
```

App 可以与本机 `frps` 同时托管一个客户端 Profile。服务端向导生成的是客户端接入基础配置；它不远程编辑或下发其他设备的代理定义。

## 外部实例的只读发现

`discovery` 每 10 秒扫描一次：

- `PATH` 和常见目录中的 `frp-panel`、`frp-panel-client` 与 `frpc`；
- 正在运行的进程；
- macOS `~/Library/LaunchAgents`、`/Library/LaunchAgents`。

面板 Client 只提取不敏感的 Client ID、API/RPC URL，并且只记录是否存在 Secret 参数。原生 `frpc` 只提取命令行中的 `-c` 或 `--config` 路径，**不打开配置文件**，因此不会读取 `auth.token`、TLS 私钥、OIDC 或插件 Secret。

外部实例不进入 `AppRuntime.child`，无法接入其 stdout/stderr；App 不会停止、接管、重载或改写它。当前面板 Client ID 或原生配置路径已经由外部实例使用时，后端会拒绝重复启动 App 托管进程。

## Sidecar 供应链和命名

`frp-panel-client` 由 `scripts/sync-frp-panel-client.sh` 从固定的 VaalaCat/frp-panel commit 构建：

- macOS Apple Silicon：`frp-panel-client-aarch64-apple-darwin`
- macOS Intel：`frp-panel-client-x86_64-apple-darwin`
- Linux x86_64：`frp-panel-client-x86_64-unknown-linux-gnu`
- Windows x86_64：`frp-panel-client-x86_64-pc-windows-msvc.exe`

macOS 原生 `frpc` / `frps` 由 `scripts/sync-frpc.sh` / `scripts/sync-frps.sh` 从固定的官方 `fatedier/frp v0.71.0` archive 下载，先验证 archive SHA-256，再精确提取单个目标二进制：

- Apple Silicon：`frpc-aarch64-apple-darwin`
- Intel：`frpc-x86_64-apple-darwin`
- Apple Silicon：`frps-aarch64-apple-darwin`
- Intel：`frps-x86_64-apple-darwin`

`src-tauri/tauri.macos.conf.json` 才将 `binaries/frpc` / `binaries/frps` 加进 bundle。这样 Linux/Windows 构建不会被要求交付尚未支持的 native sidecar。`THIRD_PARTY_NOTICES.md` 记录了版本、来源、校验和和许可证。

## 秘密、TOML 与日志

- 面板 Client Secret 保存在系统凭据库，Rust 通过 `CLIENT_SECRET` 环境变量传给 sidecar，不出现在命令行或 `connections.json`/`profiles.json`。
- App 托管的 `frpc.toml` 与 `frps.toml` 使用应用私有目录并在 Unix 上设为 `0600`。TOML 可能含 `auth.token`、Dashboard 密码或 OIDC Secret，用户不应将其提交、上传或分享。
- 外部 native TOML 不读取、不复制、不写入。
- Dashboard 查询仅读取 App 托管的 `frps.toml`，并在 Rust 后端使用 `webServer` Basic Auth；密码不会返回给 Vue、不会持久化到普通 JSON，也不会进入日志。HTTP 重定向被拒绝，HTTPS 保持证书验证。
- Dashboard 响应只映射客户端、代理、端口、连接数和流量等运行状态；不向 UI 返回完整 proxy spec、annotations 或 metadata，避免意外显示用户放入其中的敏感内容。
- native 校验和运行日志对包含 `auth.token`、password、secret、plugin token 等典型配置行脱敏；分享日志前仍应人工检查主机、域名、端口与业务信息。

## 状态与托盘

客户端与服务端各自有独立的进程句柄和运行状态。面板 Client 还会基于其日志把状态从 `starting` 提升为 `running`；原生 frpc 与 frps 的 child 成功 spawn 后分别显示为“frpc 运行中”和“frps 运行中”。App 托管的 frps 启用 `webServer` 后，Server 页面通过官方 v2 Dashboard 的只读接口展示实际客户端与代理状态；未启用 `webServer` 时，App 不会把进程运行误称为每个 proxy 均已连接。

窗口关闭默认隐藏到托盘。退出 App 时只停止 App 自己创建的 child，外部 Client/LaunchAgent 不受影响。

## 前端权限边界

- 生产 CSP 只允许本地资源和 Tauri IPC。
- Capability 只允许窗口管理、autostart 和受限的 `frp-panel-client` / `frpc` / `frps` sidecar 执行。
- 前端无法传递任意 shell 命令、二进制路径或 sidecar 参数；Rust 后端构造固定的 `verify`、`-c` 和面板参数。

## 后续路线

1. 扩展可视化配置到 STCP、XTCP、SUDP、visitor 和 plugin。
2. 对未支持 TOML 字段实现完整 round-trip 保留。
3. 支持多面板 Profile 的独立 Keychain 账户。
4. 支持菜单栏 popover 小窗。
5. 如果引入显式的 Agent / OIDC 授权模型，再设计逐客户端授权、吊销或远程配置；不把共享 Token 当作逐客户端控制机制。
