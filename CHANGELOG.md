# Changelog

本项目遵循 Keep a Changelog 的思路，并使用语义化版本号。

## [Unreleased]

### Security

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
