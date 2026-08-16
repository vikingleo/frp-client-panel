# 使用指南

## 适用对象与前提

本应用提供三个彼此独立的运行面：`frp-panel` 受管 Client、macOS 上的原生 `frpc` Client，以及 macOS 上的本机 `frps` Server。前两者用于连接服务端；本机 `frps` 用于让这台 Mac 接受其他设备的 frpc 连接。

安装前请准备：

1. 与电脑架构匹配的安装包。

如果使用 **frp-panel 受管 Client** 模式，还需要：

2. frp-panel Web UI 生成的 Client 命令。
3. 可访问的 API URL 与 RPC URL。
4. 服务端的有效 TLS 证书；如使用自签名证书，需要理解并主动接受 TLS 例外风险。

如果使用 **原生 frpc** 模式，还需要一份已确认可用的 TOML，或在应用中使用常用代理生成器创建 TOML。App 托管模式会自带 frpc，无需预先安装。

如果使用 **本机 frps** 模式，还需要确认其他客户端能访问这台 Mac 的地址和 `bindPort`。局域网使用时检查 macOS 防火墙；跨公网使用时还需要公网地址、端口映射、IPv6 或 VPN 等可达性方案。

## 安装

| 系统 | 交付物 | 安装方式 |
| --- | --- | --- |
| macOS Apple Silicon | `aarch64` DMG | 打开 DMG，将 App 拖到“应用程序”目录 |
| macOS Intel | `x64` DMG | 打开 DMG，将 App 拖到“应用程序”目录 |
| Linux x86_64 | AppImage | 赋予执行权限后运行；桌面集成因发行版而异 |
| Windows x86_64 | NSIS EXE | 运行安装程序；应用和 sidecar 安装在当前用户目录 |

macOS 未经 Developer ID 签名或 notarization 的测试包可能被 Gatekeeper 拦截；Windows 未签名的测试包可能触发 SmartScreen。只应从项目 Release 页面获取文件，并在发布者提供时校验 SHA256。

## 配置连接

### 方式一：粘贴 Client 启动命令

在 frp-panel Web UI 中复制类似下方的命令：

```bash
frp-panel client -s <secret> -i <client-id> \
  --api-url https://panel.example.com \
  --rpc-url wss://panel.example.com
```

在应用“配置”页粘贴，点击“解析并填充”。应用只提取 Client ID、Secret、API URL 和 RPC URL，不会执行这条命令。

### 方式二：粘贴 Linux 安装命令

如果面板提供：

```bash
curl ... | bash -s -- client -s <secret> -i <client-id> ...
```

同样可以粘贴。应用会识别其中 `client` 后的参数，但**不会**下载脚本、执行 `curl`、运行 `bash` 或改动系统服务。

### 连接与状态

保存后点击“保存并连接”。启动成功后：

1. 总览页显示“正在连接”或“已连接”。
2. 日志页出现 sidecar 启动、注册、拉取配置等信息。
3. 菜单栏/系统托盘可以显示窗口、连接、断开和退出。

首次连接可能短暂出现“config is empty, wait for server init”。这表示 Master 尚未向此 Client 下发配置；它不是桌面应用安装失败。

## 已有命令行 Client、服务与开机启动

如果你已经在本机通过命令行安装并运行了：

```bash
frp-panel client -s <secret> -i <client-id> --api-url <api-url> --rpc-url <rpc-url>
```

不需要再填写或执行安装命令。打开桌面应用后，在“总览”页的“系统已有 frp-panel Client”区域即可看到只读发现结果：

- 在 `PATH` 和常用目录中发现的 `frp-panel` / `frp-panel-client` 二进制；
- 正在运行的 Client 的 PID、二进制路径、Client ID、API URL、RPC URL 和运行时长；
- macOS `~/Library/LaunchAgents` 与 `/Library/LaunchAgents` 中定义的 `frp-panel` Client 启动项。

应用每 10 秒重新检测一次，也可以点击“重新检测”。启动项存在不等于 Client 当前运行；当前运行状态以进程发现结果为准。

你可以点击“填入安全字段”将 Client ID、API URL、RPC URL 复制到桌面应用的配置页。外部 Client 的 Secret **不会被读取、显示或导入**，因此仍需手工填写 Secret 后才能改用内置托管模式。

外部 Client 始终由原有命令、脚本或 LaunchAgent 管理：桌面应用不能读取它的 stdout/stderr，不能停止、重启、接管或修改它。若外部 Client 与当前 Profile 使用同一 Client ID，应用会禁用“连接客户端”并在后端再次阻止启动，以避免重复注册到 frp-panel Master。

## 原生 frpc 模式（macOS）

1. 打开“配置”，点击“+ frpc”。
2. 选择“导入到 App 私有副本”，导入一份 `frpc.toml`，或使用常用代理生成器生成 TCP、UDP、HTTP、HTTPS 的起始 TOML。
3. 保存 Profile 后点击“校验并启动 frpc”。应用先运行 `frpc verify -c`；校验通过才会运行 `frpc -c`。
4. 在总览或日志页观察 App 托管的 frpc 进程。

App 托管模式会把 TOML 保存到应用配置目录并设置用户私有文件权限。TOML 内可能包含 `auth.token`，请勿提交到 Git、截图或共享给不可信人员。

如果你已经通过命令或 LaunchAgent 运行 `frpc -c /path/frpc.toml`，总览会只读显示它的 PID、二进制、配置路径和运行时长。应用不读取配置内容、Token、TLS 私钥或插件 Secret，也不会执行 reload、stop 或改写外部配置。相同配置路径的外部 frpc 正在运行时，应用会阻止启动第二个托管实例。

## 本机 frps 服务端模式（macOS）

1. 打开“本机 frps”，新建或选择一个 Server Profile。
2. 使用服务端向导生成 `frps.toml`，或导入自己的 TOML 到 App 私有副本。向导默认使用 Token、强制 TLS，并把 Dashboard 绑定在 `127.0.0.1`。
3. 点击“保存 Server Profile”，再点击“校验并启动”。应用固定先执行 `frps verify -c`，通过后才执行 `frps -c`。
4. 在同一页面复制“客户端接入配置”到其他设备；为每台客户端继续添加自己的 `[[proxies]]` 配置块。

服务端 Token、Dashboard 密码和其他密钥会写入托管 `frps.toml`，因此该文件使用 App 私有目录和用户私有权限。不要把它提交到 Git、截图或发送给不可信人员。

本机 frps 的运行状态只表示服务端进程已经启动和监听配置；它不表示每个客户端代理都可用。标准 token 模式使用的是服务端与客户端共享的认证信息：本版本可以生成接入模板，但不能远程编辑、单独批准、吊销或停止其他设备上的 frpc。

## 自动启动

配置页提供两个独立开关：

- **打开应用时自动连接**：启动桌面应用后立即拉起 Client。
- **登录后启动应用**：使用当前系统的自动启动机制，使应用登录后常驻托盘。

关闭第二个开关会移除应用设置的自动启动项，但不会改动你手工创建的其他系统服务或发现到的 `frp-panel` LaunchAgent。已经使用外部 LaunchAgent 自启的 Client 无需开启本应用的“打开应用时自动连接”；否则相同 Client ID 会被应用安全地识别并避免重复拉起。

## TLS 证书与自签名部署

默认情况下，应用要求 HTTPS / WSS 服务端证书可验证。推荐为 frp-panel 配置受信任证书和正确的域名。

只有在确实使用自签名证书、且你已核对服务器指纹或可信网络边界时，才在“配置”页启用“允许不验证 TLS 证书”。启用后连接可能遭受中间人攻击，应视为临时兼容措施，而不是常规配置。

## 凭据、日志和隐私

- Client Secret 保存在系统凭据库，不保存在 `connections.json`。
- Secret 不作为 sidecar 的命令行参数传递。
- 当前会话日志保存在应用内存中，退出应用后不会由本应用持久化。
- 应用不会上传遥测、分析或自动崩溃报告。

在提交 issue 前，请删除日志中的 Client ID、域名、IP、端口和业务信息。详情见 [PRIVACY.md](../PRIVACY.md)。

## 更新与卸载

### 更新

下载相同系统和架构的新版本后覆盖安装。首次启动时，应用会从旧 macOS Keychain service 迁移 Secret 到通用系统凭据 service；无需重新粘贴 Secret，除非凭据库条目已被删除。

### 卸载

- macOS：退出应用后删除“应用程序”中的 App；如不再需要连接信息，请在系统钥匙串中删除 `app.frppanel.client` / `client-secret`。
- Linux：删除 AppImage；如不再需要连接信息，请在 Secret Service 和应用配置目录中移除相应条目。
- Windows：在“已安装的应用”中卸载；如不再需要连接信息，请在 Windows Credential Manager 中删除对应凭据。

## 排障

| 现象 | 检查方式 |
| --- | --- |
| “内置 Client 不可用” | 确认安装包与系统架构一致；开发环境先运行 `pnpm sync:client` |
| 证书错误 | 检查 API/RPC 域名、证书链、服务器时间；不要直接长期关闭校验 |
| Client 已注册但没有隧道 | 在 frp-panel Web UI 检查该 Client 是否已分配配置 |
| 保存后提示找不到 Secret | 重新粘贴命令并保存，确认系统凭据库可用 |
| App 启动但不自动连接 | 分别检查“打开应用时自动连接”和“登录后启动应用”开关 |
| Windows/macOS 拦截安装包 | 确认从官方 Release 下载并核验哈希；测试版未签名时可能出现系统警告 |

仍无法解决时，按 [SUPPORT.md](../SUPPORT.md) 提供系统版本、CPU 架构、安装包类型和脱敏日志。
