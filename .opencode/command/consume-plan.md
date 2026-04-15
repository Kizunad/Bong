---
description: 全自动消费 Bong 的 docs/plan-*.md，经 Prometheus → Metis → Momus → Atlas 一键落地
agent: Sisyphus
---

ultrawork high accuracy.

**参数**：`$ARGUMENTS` — 要消费的 plan 名（不含 `plan-` 前缀和 `.md` 后缀，例如 `HUD-v1`）。

---

## 执行

你现在要**全自动**消费 `docs/plan-$ARGUMENTS.md`。这是 Bong 的定稿开发 plan。

遵循 `@.opencode/prompts/auto-consume.md` 的完整四阶段流水线。把该文件里所有 `{{PLAN_NAME}}` 占位符替换为 `$ARGUMENTS`。

**前置校验**（你自己先做，不要问我）：

1. `docs/plan-$ARGUMENTS.md` 必须存在；若不存在，立即输出 `<promise>BLOCKED: plan 文件不存在</promise>` 退出。
2. **禁止**消费 `docs/plans-skeleton/`（骨架占位）和 `docs/finished_plans/`（已归档）下的文件。若 `$ARGUMENTS` 指向这些目录，立即输出 `<promise>BLOCKED: 不能消费骨架或已归档 plan</promise>` 退出。
3. 若 `.sisyphus/inputs/$ARGUMENTS.md` 不存在，执行：
   ```bash
   mkdir -p .sisyphus/inputs .sisyphus/plans
   cp docs/plan-$ARGUMENTS.md .sisyphus/inputs/$ARGUMENTS.md
   ```

然后严格按 `@.opencode/prompts/auto-consume.md` 走：

1. Prometheus 规整 → `.sisyphus/plans/$ARGUMENTS.md`
2. Metis 预分析 → 回填
3. Momus 审核（high-accuracy）→ 通过才继续
4. `/start-work $ARGUMENTS` → Atlas 执行到 `<promise>DONE</promise>` 或 `<promise>BLOCKED: ...</promise>`

**提示**：如果你是在宿主 `scripts/bong-plan-auto.sh` 脚本启动的 worktree 内，worktree 已就绪，直接开干。如果你是在人工 opencode 会话里被调起的，注意**不要在主工作区直接改代码** —— 先让用户确认是否需要先 `bash scripts/bong-plan-auto.sh $ARGUMENTS` 在 worktree 里跑。
