# Changelog

本项目遵循 Keep a Changelog 的思路，并使用语义化版本号。

## [Unreleased]

### Added

- 新增只读外部 Client 发现：识别系统已安装的 `frp-panel` / `frp-panel-client`、正在运行的 Client 进程和 macOS LaunchAgent。
- 总览页新增外部 Client 状态、二进制与启动项展示，并可导入 Client ID、API URL、RPC URL 等非敏感字段。
- 当外部 Client 与当前 Profile 使用相同 Client ID 时，阻止重复启动内置 sidecar。
- 新增 macOS 原生 `frpc` Profile：托管 `frpc.toml`、启动前校验、启动/停止、日志和状态栏控制。
- 新增本机 `frps` Server Profile：导入/生成 `frps.toml`、启动前校验、启动/停止、自动启动、托盘控制和服务端日志。
- 内置固定版本的官方 macOS `frpc` / `frps` sidecar，并验证 Intel 与 Apple Silicon 目标架构。
- 新增客户端接入模板和只读 Dashboard 概览，展示已连接客户端、代理状态、连接数和流量。

### Security

- 外部进程与 LaunchAgent 发现不导出、不显示、不保存或记录 Client Secret；不会自动接管、停止或修改外部 Client。
- Client Secret 改由 sidecar 环境变量传递，不再放入 sidecar 命令行参数。
- TLS 证书校验默认开启；自签名证书例外需要在 UI 中显式启用。
- App 托管的服务端 TOML 使用用户私有文件权限；Dashboard 状态接口不返回凭据，凭据不进入普通 Profile JSON 或日志。用户主动编辑托管 TOML 时，其内容会短暂出现在本机配置编辑器内存中。
- Dashboard 查询拒绝 HTTP 重定向并保持 HTTPS 证书验证；外部只读 `frps` 配置不会被读取或接管。

### Documentation

- 新增许可证、第三方声明、隐私策略、安全策略、贡献指南和支持说明。
- 新增用户指南、二次开发指南和发布指南。
- 更新计划、隐私、贡献、支持、发布和 Issue/PR 模板，覆盖本机 `frps` 与 Dashboard 工作流。

## [0.1.0] - 2026-08-15

### Added

- macOS Apple Silicon / Intel、Linux x86_64、Windows x86_64 的桌面打包支持。
- frp-panel 受管 Client 命令解析、系统凭据库、托盘和实时日志。
- 原生 GitHub Actions bundle 验证。
