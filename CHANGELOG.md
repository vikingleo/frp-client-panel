# Changelog

本项目遵循 Keep a Changelog 的思路，并使用语义化版本号。

## [Unreleased]

### Added

- 新增只读外部 Client 发现：识别系统已安装的 `frp-panel` / `frp-panel-client`、正在运行的 Client 进程和 macOS LaunchAgent。
- 总览页新增外部 Client 状态、二进制与启动项展示，并可导入 Client ID、API URL、RPC URL 等非敏感字段。
- 当外部 Client 与当前 Profile 使用相同 Client ID 时，阻止重复启动内置 sidecar。

### Security

- 外部进程与 LaunchAgent 发现不导出、不显示、不保存或记录 Client Secret；不会自动接管、停止或修改外部 Client。
- Client Secret 改由 sidecar 环境变量传递，不再放入 sidecar 命令行参数。
- TLS 证书校验默认开启；自签名证书例外需要在 UI 中显式启用。

### Documentation

- 新增许可证、第三方声明、隐私策略、安全策略、贡献指南和支持说明。
- 新增用户指南、二次开发指南和发布指南。

## [0.1.0] - 2026-08-15

### Added

- macOS Apple Silicon / Intel、Linux x86_64、Windows x86_64 的桌面打包支持。
- frp-panel 受管 Client 命令解析、系统凭据库、托盘和实时日志。
- 原生 GitHub Actions bundle 验证。
