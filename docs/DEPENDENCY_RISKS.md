# 依赖风险登记

本文件记录当前 `cargo audit` 的非阻断告警。它们没有被加入忽略列表；安全 workflow 会继续显示这些结果，维护者必须在每次依赖升级时重新评估。

## 2026-08-15 审查结果

`cargo audit --file src-tauri/Cargo.lock` 未报告已知可利用漏洞，但报告以下上游告警：

| 范围 | 告警类型 | 处理原则 |
| --- | --- | --- |
| `gtk`、`gdk`、`atk` 及其 `-sys` / 宏包 | GTK3 Rust bindings 已停止维护 | 这些是 Linux Tauri/Wry 图形栈的传递依赖。跟踪 Tauri/Wry 的 GTK4 或替代后端迁移，不通过本项目直接 fork 规避。 |
| `glib` 0.18.5 | 迭代器实现的 unsound 告警 | 由 GTK3 栈传入。保持 Tauri/Wry 更新，评估 Linux 运行时是否触发对应 API。 |
| `proc-macro-error` | 已停止维护 | 传递开发/构建依赖；随上游宏依赖更新处理。 |
| `unic-*` 0.9 | 已停止维护 | 传递 Unicode 标识符依赖；随上游更新处理。 |

## 维护规则

1. 新增 `cargo audit` 告警时，不要直接写入 ignore 配置。
2. 若告警有可利用路径、影响凭据/TLS/进程执行或已提供修复版本，应阻断 Release。
3. 只有在记录受影响范围、上游 issue、临时缓解措施和复审日期后，才可接受非阻断告警。
4. 每个正式 Release 都应检查本文件是否仍准确。
