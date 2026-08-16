# 依赖风险登记

本文件记录当前 `cargo audit` 的非阻断告警。它们没有被加入忽略列表；安全 workflow 会继续显示这些结果，维护者必须在每次依赖升级时重新评估。

## 2026-08-16 审查结果

`cargo audit --file src-tauri/Cargo.lock` 未报告已知可利用漏洞，但报告以下上游告警：

本次 Dashboard 只读客户端新增 `reqwest`、`rustls` 和 `webpki-roots` 依赖路径。
`cargo deny` 的许可证、来源与 bans 检查已通过；`webpki-roots` 使用的
CDLA-Permissive-2.0 已在 `deny.toml` 中显式允许，并在
`THIRD_PARTY_NOTICES.md` 中记录。

| 范围 | 告警类型 | 处理原则 |
| --- | --- | --- |
| `gtk`、`gdk`、`atk` 及其 `-sys` / 宏包 | GTK3 Rust bindings 已停止维护 | 这些是 Linux Tauri/Wry 图形栈的传递依赖。跟踪 Tauri/Wry 的 GTK4 或替代后端迁移，不通过本项目直接 fork 规避。 |
| `glib` 0.18.5 | 迭代器实现的 unsound 告警 | 由 GTK3 栈传入。保持 Tauri/Wry 更新，评估 Linux 运行时是否触发对应 API。 |
| `proc-macro-error` | 已停止维护 | 传递开发/构建依赖；随上游宏依赖更新处理。 |
| `unic-*` 0.9 | 已停止维护 | 传递 Unicode 标识符依赖；随上游更新处理。 |

## 记录：GHSA-wrw7-89jp-8q8g（`glib` 0.18.5）

- **告警与范围**：GitHub Dependabot 将 `glib` 0.18.5 标记为 runtime
  moderate 告警；修复版本为 0.20.0。受影响代码位于 Linux 图形运行时，
  不影响 macOS 或 Windows 发行包。
- **依赖路径**：`tauri` / `tauri-runtime-wry` → `wry` / `webkit2gtk` →
  GTK3 bindings → `glib` 0.18.5。可用
  `cargo tree --target all -i glib@0.18.5 --manifest-path src-tauri/Cargo.toml`
  复核。
- **当前限制**：本项目没有直接调用 `glib` API；但它仍是 Linux WebKitGTK
  UI 运行时的一部分，因此不能将该告警描述为“无影响”。当前 Tauri 2.11.5
  图形栈要求 GTK3 / `glib` 0.18，直接把 lockfile 强制更新为 0.20 会破坏
  兼容依赖关系。
- **临时措施**：不将该 advisory 加入 `cargo audit` 的 ignore；持续运行
  `cargo audit` 和 Dependabot；每次升级 Tauri/Wry 或发布 Linux 包前都重新
  检查是否已可解析到 `glib` 0.20 或更高版本。
- **发布要求**：在该路径仍无法升级时，发布维护者必须在 Draft Release 审查
  中明确记录这项 Linux-only 例外及复核结果。若项目开始直接使用 GVariant
  或 `glib::VariantStrIter`，或者上游提供兼容更新，则不再允许以本例外发布。
- **下次复核**：不晚于 2026-09-15，并在任何 Tauri、Wry、WebKitGTK 或 GTK
  依赖更新时提前复核。

## 维护规则

1. 新增 `cargo audit` 告警时，不要直接写入 ignore 配置。
2. 若告警有可利用路径、影响凭据/TLS/进程执行，或可以在当前兼容范围内升级到修复版本，应阻断 Release。
3. 对无法在当前上游兼容范围内修复的运行时告警，只有在记录受影响范围、上游约束、临时缓解措施和复审日期后，才可作为显式 Release 例外接受；不得将其静默忽略。
4. 每个正式 Release 都应检查本文件是否仍准确。
