---
description: 列出 Bong 当前可消费的活跃 plan（不含 skeleton 和 finished）
agent: explore
subtask: true
---

列出 `docs/plan-*.md`（仅 `docs/` 根下的 plan，不递归到 `plans-skeleton/` 或 `finished_plans/`）。

对每个 plan 输出：

- 文件名（去掉 `plan-` 前缀和 `.md` 后缀，即 `/consume-plan <name>` 要传的参数）
- 首行 H1 标题
- 是否已在 `.sisyphus/plans/<name>.md` 中（表示 Prometheus 已规整过，可直接 resume）
- 是否在 `.worktree/plan-<name>/` 有进行中的 worktree

输出紧凑表格，中文，无需其它解读。
