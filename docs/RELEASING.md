# 发布指南

## 发布前条件

1. `main` 分支受保护，所有 PR 通过 CI。
2. `package.json`、`src-tauri/Cargo.toml` 与 `src-tauri/tauri.conf.json` 的版本一致。
3. 四个原生 runner 均已通过 sidecar、测试与 bundle 验证。
4. 已审阅 `CHANGELOG.md`、许可证和第三方声明。
5. 已完成依赖漏洞扫描和 SBOM 生成。
6. macOS Developer ID / notarization 与 Windows Authenticode 签名材料只保存在 GitHub Secrets 或受保护环境中。

## 发布步骤

```bash
# 1. 更新所有版本号和 CHANGELOG
# 2. 本地验证
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml

# 3. 提交并推送
git commit -am "release: vX.Y.Z"
git push origin main

# 4. 创建并推送受保护的 tag
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

推送 `v*` tag 后，Release workflow 会创建 Draft Release。发布维护者必须在 GitHub 上检查：

- macOS ARM64 和 Intel DMG；
- Linux x86_64 AppImage；
- Windows x86_64 NSIS EXE；
- 每个平台应用和 sidecar 的版本、架构、签名与校验和；
- Release notes、第三方声明、SBOM 和 `SHA256SUMS`。

确认无误后，手动发布 Draft Release。

## 签名策略

- macOS：使用 Developer ID Application 签名，并完成 notarization 和 stapling。
- Windows：使用 Authenticode 签名并时间戳。
- Linux：至少发布 SHA256；如提供 GPG 签名，公开并维护发行公钥。

未经正式签名的产物必须标记为测试版，不能宣称已通过平台信任验证。

## 回滚

如果发现发布产物包含错误 sidecar、证书异常、Secret 泄露风险或严重依赖漏洞：

1. 立即将 Release 标记为 pre-release 或撤回公开发布。
2. 不删除 tag 所指向的历史；新增修复提交和更高版本号。
3. 在 `CHANGELOG.md` 和 GitHub Release notes 中说明受影响版本和升级路径。
4. 如涉及安全问题，按 [SECURITY.md](../SECURITY.md) 处理。
