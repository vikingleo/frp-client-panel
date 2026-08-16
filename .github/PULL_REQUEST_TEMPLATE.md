## 变更说明

<!-- 说明解决的问题、用户可见影响和设计取舍。 -->

## 影响范围

- [ ] macOS Apple Silicon
- [ ] macOS Intel
- [ ] Linux x86_64
- [ ] Windows x86_64
- [ ] Sidecar / 协议兼容性
- [ ] 本机 frps / Dashboard
- [ ] 凭据、TLS 或 Tauri 权限
- [ ] 文档

## 验证

- [ ] `pnpm build`
- [ ] `cargo fmt --check --manifest-path src-tauri/Cargo.toml`
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml`
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] 适用平台的 `pnpm verify:client` / `pnpm verify:frpc` / `pnpm verify:frps` / `pnpm verify:bundle`
- [ ] 若变更 Rust 依赖，已运行 `(cd src-tauri && cargo deny --config ../deny.toml check licenses bans sources)`

## 安全确认

- [ ] 未提交 Client Secret、Token、生产 URL、私钥或未脱敏日志。
- [ ] 未扩大 sidecar/Tauri 权限，或已在此处说明原因。
- [ ] 未默认关闭 TLS 证书校验，或已在此处说明明确的用户选择与风险提示。
- [ ] 未通过 Dashboard 状态接口、日志或普通 JSON 暴露 Token、Dashboard 密码、完整 proxy metadata 或远程客户端配置；如调整托管 TOML 编辑器，已说明本机 UI 内存与截图风险。
