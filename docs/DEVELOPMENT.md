# 二次开发指南

## 架构与边界

```text
Vue UI
  │ Tauri invoke / events
  ▼
Rust application layer
  ├─ config.rs          配置元数据与系统凭据库
  ├─ command_parser.rs  仅解析粘贴命令，不执行 shell
  ├─ discovery.rs       只读发现外部二进制、运行进程与启动项
  ├─ process.rs         受控启动/停止 panel/frpc sidecar、日志脱敏与 native verify
  ├─ runtime.rs         进程状态和日志环形缓冲
  ├─ sidecar.rs         平台二进制选择、panel 安全环境变量与 frpc sidecar
  └─ tray.rs            系统托盘
  │
  ▼
frp-panel-client sidecar / official frpc sidecar
  │ API / RPC
  ▼
frp-panel Master
```

桌面应用不重新实现 frp-panel 的受管 Client 协议。协议兼容性由上游 `frp-panel-client` 负责；原生模式不重新实现 frp，而是管理官方 `frpc`。本项目的职责是安全地管理进程和本地用户体验。

## 工具链

| 工具 | 版本/用途 |
| --- | --- |
| Node.js | 22.x |
| pnpm | 10.x |
| Rust | stable，见 `rust-toolchain.toml` |
| Go | 1.25.x，用于构建 sidecar |
| macOS | Xcode Command Line Tools |
| Linux | WebKitGTK 4.1、GTK 3、AppIndicator、librsvg、patchelf |
| Windows | Visual Studio Build Tools / MSVC 环境 |

Go 1.24 构建的 macOS Intel sidecar 已被验证会在启动时崩溃，因此不要将 CI 或正式交付回退到 Go 1.24。

## 初始化

```bash
git clone git@github.com:vikingleo/frp-client-panel.git
cd frp-client-panel
pnpm install --frozen-lockfile
pnpm sync:client
pnpm sync:frpc
pnpm tauri dev
```

`pnpm sync:client` 默认为当前宿主系统生成 sidecar。指定目标时使用：

```bash
FRP_PANEL_TARGET_TRIPLE=aarch64-apple-darwin pnpm sync:client
FRP_PANEL_TARGET_TRIPLE=x86_64-apple-darwin pnpm sync:client
FRP_PANEL_TARGET_TRIPLE=x86_64-unknown-linux-gnu pnpm sync:client
FRP_PANEL_TARGET_TRIPLE=x86_64-pc-windows-msvc pnpm sync:client

# 官方原生 frpc 目前仅随 macOS bundle 提供
FRP_PANEL_TARGET_TRIPLE=aarch64-apple-darwin pnpm sync:frpc
FRP_PANEL_TARGET_TRIPLE=x86_64-apple-darwin pnpm sync:frpc
```

构建脚本固定上游 commit `1a58b856d7de19de8669b7072872986d2fa1604a`。更新上游版本时，必须在原生 macOS Intel、macOS Apple Silicon、Linux x86_64 和 Windows x86_64 runner 上重新验证。

## 代码位置

| 需求 | 推荐修改位置 |
| --- | --- |
| 表单、状态、日志视图 | `src/App.vue` |
| 前端 Tauri 调用 | `src/commands.ts` |
| 前端数据类型 | `src/types.ts` |
| 连接配置和凭据迁移 | `src-tauri/src/config.rs` |
| 解析面板命令 | `src-tauri/src/command_parser.rs` |
| 探测外部命令行 Client / LaunchAgent | `src-tauri/src/discovery.rs` |
| 启动、停止和脱敏 | `src-tauri/src/process.rs` |
| Profile 迁移、Keychain 与托管 TOML | `src-tauri/src/config.rs` |
| sidecar / 环境变量 / 目标架构 | `src-tauri/src/sidecar.rs` |
| 运行时状态和日志 | `src-tauri/src/runtime.rs` |
| 系统托盘 | `src-tauri/src/tray.rs` |
| Tauri 权限 | `src-tauri/capabilities/default.json` |

## 安全不变量

所有二次开发必须保持以下不变量：

1. 不执行用户粘贴的 shell 命令。
2. 不将 Client Secret 写入普通 JSON、日志、README、issue 模板或测试夹具。
3. 不将 Client Secret 作为 sidecar 命令行参数；使用 `CLIENT_SECRET` 环境变量。
4. 默认设置 `CLIENT_TLS_INSECURE_SKIP_VERIFY=false`。
5. 自签名证书例外必须是用户可见、默认关闭的显式选项。
6. 保持 `CLIENT_FEATURES_ENABLE_FUNCTIONS=false` 和 `CLIENT_FEATURES_ENABLE_REMOTE_SHELL=false`，除非产品定义、安全评估和 UI 提示同步更新。
7. 不扩大 Tauri shell capability；任何新增 sidecar 参数都应由 Rust 后端构造和校验。
8. 外部进程发现只能读取非敏感字段：不得复制、显示、日志化、持久化 `-s` / `--secret` 的值。
9. 不自动停止、接管、重启或修改外部 Client 与外部 LaunchAgent；同 Client ID 时只阻止重复启动内置 sidecar。
10. 原生 Profile 启动前必须执行固定参数的 `frpc verify -c <配置>`；前端不可传入任意 shell 命令或二进制路径。
11. App 托管原生配置只写 TOML、设置 0600 权限；外部原生配置仅保存其路径，不读写内容或 Secret。

## 测试与本地验证

```bash
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml

pnpm verify:client
pnpm verify:frpc
pnpm verify:bundle
```

目标 bundle：

```bash
pnpm bundle:mac
pnpm bundle:linux
pnpm bundle:windows
```

新增功能至少应补充：Rust 单元测试、前端类型检查、失败路径测试和一条不含真实 Secret 的手工验证记录。涉及外部发现时，至少覆盖 Client 命令识别、非 Client 命令忽略、同 ID/配置路径冲突、二进制去重，以及不返回 Secret；macOS 还应覆盖 LaunchAgent plist 解析。涉及 sidecar 或打包时，必须运行相应平台的 `verify:client`、`verify:frpc` 与 `verify:bundle`。

## 依赖与供应链

- JavaScript 依赖由 `pnpm-lock.yaml` 锁定。
- Rust 依赖由 `src-tauri/Cargo.lock` 锁定。
- sidecar 上游源码由 `scripts/sync-frp-panel-client.sh` 中的 commit 锁定。
- 不要通过 `latest`、未固定 branch 或手工下载的二进制更新正式 Release。
- 依赖更新应包含许可证、漏洞扫描和跨平台 CI 结果。
- `cargo audit` 的当前非阻断告警记录在 `docs/DEPENDENCY_RISKS.md`；不得未经风险评估将其静默忽略。

## 调试原则

- 使用演示环境或临时 Client Secret。
- 日志和截图进入 issue 前必须脱敏。
- 不要为了排障默认关闭 TLS 校验。
- 不要把开发环境的 `target/`、`node_modules/`、`.env`、凭据文件或私钥提交到仓库。

## 文档同步要求

修改用户可见行为时，至少同时更新：

1. 根目录 `README.md` 的项目概要。
2. `docs/USER_GUIDE.md` 的操作步骤或限制。
3. 本文件中的架构/安全不变量（如适用）。
4. `CHANGELOG.md` 的 Unreleased 条目。
