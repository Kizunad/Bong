# plan-rust-stable-clippy-baseline-v1（骨架）

> **骨架（草案）**。一句话主题：仓库声明的 server 门禁 `cargo clippy --all-targets -- -D warnings` 在 Rust 1.96.1 下被报告产生约 69 条 lint 错误，导致正常 BugFix PR 无法满足既定质量门。

## 问题摘要

- 待按第一性原理在干净的 `origin/main` 基线上复现并确认具体数量、lint 类别与真实根因，不能把“已有失败”直接当作免责依据。
- 若问题属实，优先以最小代码改动消除 lint；不得擅自 pin 或修改 Rust toolchain、依赖版本及生产配置。
- 验收以 Rust 1.96.1 下 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 与 `cargo test` 全绿为准。
