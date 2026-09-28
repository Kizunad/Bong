# plan-bughunt-alchemy-minor-cleanups-v1

> **来源 issue**：#1370。
> 一句话主题：多张丹方残卷合并后没有按合并后的阶段覆盖度重算品质上限。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | `learn_fragment` 合并路径重算 `learned_quality_cap` | ⬜ |
| P1 | 残卷学习生产链路与边界回归 | ⬜ |

## §0 摘要

`RecipeFragment::into_knowledge` 会按单张残卷的 `known_stages` 调 `learned_quality_cap`，严重残卷因此得到上限 1。`LearnedRecipes::learn_fragment` 在 `server/src/alchemy/learned.rs:42-55` 合并第二张残卷时只 extend/dedup 阶段，再取旧值与新值的 `max`，没有用合并后的阶段重新调用 `completeness_for_recipe`/`learned_quality_cap`。多张各自不足半程的残卷合计达到半程后，`PartialRecipeKnowledge.max_quality_tier` 仍锁在 1。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` 的丹方残卷、炼丹品阶和知识残缺关键词；只恢复已有阶段覆盖语义，不调整丹药品阶定义。
- **finished_plans**：查 alchemy-v2 recipe fragment、炼丹 UI 学习与 furnace handoff 归档计划，确认它们没有修合并后的品质 cap。
- **active plan**：查 `docs/plan-*.md` 的 `LearnedRecipes`、`PartialRecipeKnowledge`、`AlchemyLearnRecipeFragment`；未见同一合并重算入口。
- **skeleton**：查 `plan-bughunt-agent-schema-server-contract-drift-v1.md`、`plan-bughunt-alchemy-recipe-fragment-handoff-v1.md` 与 alchemy UI stale 骨架；它们处理 payload/会话，不重复本地知识合并算法。
- **reminder.md**：查 `docs/plans-skeleton/reminder.md` 的 `learn_fragment`、`known_stages`、`max_quality_tier`、`learned_quality_cap`；仓内有该文件，但无此合并缺口登记。

## §2 接入面与跨仓契约

- **Inputs**：client 的 `AlchemyLearnRecipeFragment { item_instance_id }` 请求、`RecipeFragment` 的 `recipe_id/known_stages/max_quality_tier`、server `Recipe` 与玩家 `LearnedRecipes`。
- **Outputs**：合并后的 `PartialRecipeKnowledge` 阶段去重且品质上限按合并后覆盖度计算；成功学习后原残卷只消费一次，后续 alchemy snapshot 读取新的知识。
- **共享类型/事件**：复用 `ClientRequestV1::AlchemyLearnRecipeFragment`、`ProductionRequest::LearnRecipeFragment`、`LearnRecipeFragmentIntent`、`RecipeFragment`、`PartialRecipeKnowledge`、`LearnedRecipes`；不新增 schema 字段。
- **三端契约符号**：server `network::client_request::production::emit_learn_recipe_fragment`、`alchemy::learn_recipe_fragment_system`、`alchemy::learned::LearnedRecipes::learn_fragment`；client **无变更**，请求已只表达残卷实例 ID，结果通过既有 alchemy snapshot；agent **无变更**，炼丹学习不经过 Redis IPC。
- **worldview/qi**：本问题不改变真元数值或 ledger 流转；若实现者补充炼丹回归，守恒断言仍使用现有 `qi_physics::ledger::assert_conservation` 入口和生产常量，不把知识 cap 当成 qi 账。

## §3 游玩影响与复现

准备一个三阶段 recipe，分别提交两张各自只覆盖一个阶段的残卷。第一次学习把 `max_quality_tier` 规范为 1；第二次合并后 `known_stages.len()*2 >= recipe.stages.len()`，但 `partial_for(...).max_quality_tier` 仍为 1，玩家无法制作应达到的高品丹药。

## §4 `origin/main` 根因证据

- `server/src/alchemy/recipe_fragment.rs:37-50` 明确定义合并后覆盖半数阶段即 `UsablePartial`，并由 `learned_quality_cap` 返回 1..=3。
- `server/src/alchemy/learned.rs:42-55` 只对新 `knowledge.max_quality_tier` 做 `.max()`，没有用已 extend/dedup 的 `existing.known_stages` 重新计算 cap。
- `server/src/alchemy/mod.rs:180-227` 是生产消费、合并并消费残卷的路径；`network/client_request/production.rs:112-114,322-327` 证明 client 请求可达该系统。

## §5 非重复比对

recipe-fragment handoff 骨架处理请求字段和消费边界，agent schema drift 骨架处理跨仓枚举；本骨架只处理 server 内部的知识合并重算，不改 client payload 或 agent schema。

## §6 修复计划骨架

- **P0**：合并 `known_stages` 后，以合并后的 `PartialRecipeKnowledge` 调用 `learned_quality_cap(recipe)`（或等价的 `completeness_for_recipe` 入口），再写回 `max_quality_tier`；保留 1..=3 clamp 和去重。
- **P1**：补单张严重残卷、第二张使覆盖达到半程、重复阶段和完整覆盖的最小回归，确认成功合并仍只消费当前 item，snapshot 能读到新 cap。

## §7 验证计划

实现后只跑 server 栈 fmt、clippy、cargo test，重点 `alchemy::learned`、`recipe_fragment` 和 client-request production 链路；agent/client 无代码改动，不跑其门禁。
