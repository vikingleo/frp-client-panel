# 安全策略

## 支持版本

当前仅维护最新 `main` 分支和最新正式 Release。旧版本如包含 Secret、TLS、sidecar、权限或依赖漏洞，应升级到最新版本。

## 报告漏洞

请不要在公开 issue 中报告以下问题：

- Client Secret、Token、Cookie 或凭据泄露；
- TLS 证书校验绕过；
- 任意命令执行、Tauri capability 绕过；
- sidecar 替换、发布供应链或签名问题；
- 远程控制、权限提升、数据泄露。

优先使用 GitHub 的 Private Vulnerability Reporting 功能；如果仓库尚未启用该功能，请联系仓库维护者并只提供最小化复现信息。报告中请包含受影响版本、平台、复现步骤、影响范围和脱敏后的证据。

## 处理目标

维护者会确认收到报告、评估影响、制定修复计划，并在发布修复后记录必要的安全公告。请在修复发布前避免公开可利用细节。

## 用户安全建议

- 不要把 Client Secret 放入 issue、截图、命令历史或共享终端。
- 保持 TLS 证书校验开启；自签名证书例外只应在受控环境临时使用。
- 只下载经校验和、签名或可信 Release 页面提供的安装包。
- 发现可疑 sidecar、证书或网络行为时，立即停止连接并保留脱敏日志。

当前已知的非阻断依赖告警及维护原则见 [docs/DEPENDENCY_RISKS.md](docs/DEPENDENCY_RISKS.md)。
