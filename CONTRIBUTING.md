# 贡献指南

感谢你愿意改进 frp-panel Client。提交前请先阅读 [开发指南](docs/DEVELOPMENT.md) 和 [安全策略](SECURITY.md)。

## 提交前检查

```bash
pnpm install --frozen-lockfile
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

涉及 sidecar 或安装包时，还必须运行适用平台的：

```bash
pnpm verify:client
pnpm verify:frpc
pnpm verify:frps
pnpm verify:bundle
(cd src-tauri && cargo deny --config ../deny.toml check licenses bans sources)
```

## 贡献规则

1. 一个 PR 只解决一个明确问题，说明用户影响和测试证据。
2. 不提交 Client Secret、Token、生产 URL、私钥、日志转储、`.env` 或打包产物。
3. 不执行或引入执行用户粘贴 shell 命令的能力。
4. 不降低 TLS 默认校验，不在没有显式用户选择的情况下跳过证书验证。
5. 变更用户行为时同步更新 `README.md`、`docs/USER_GUIDE.md`、相关架构/隐私文档和 `CHANGELOG.md`。
6. 更新 sidecar 上游 commit 时，说明来源、原因、许可证影响和四平台验证结果。

## Pull Request 内容

请在 PR 中填写：

- 问题背景与解决方案；
- 涉及的平台和架构；
- 已运行的测试；
- 用户可见的变化；
- 是否影响凭据、TLS、sidecar、权限或发布物。

提交贡献即表示你有权在本仓库的 AGPL-3.0-only 许可证下提交该代码。
