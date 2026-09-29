# plan-bughunt-network-state-semantics-minor-cleanups-v1（骨架）

> **来源 issue**：#1413、#1394、#1376。三条都是状态 payload 的字段语义错误：HUD 使用硬编码 idle、境界名称违背正典、morph 把 race id 填进 body-plan 字段。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | payload 字段从真实状态派生，境界显示使用“化虚”，morph 的 body plan 走解析入口 | ⬜ |

## §0 摘要

`yidao_state_emit` 对非施法 healer 固定传 `(0,0,false)`，HUD 永远 idle；`Realm::Void` 在 `tsy_polish` 显示旧称“洞虚”；`morph_state_emit` 将 `RaceId` 的字符串写入 `form_body_plan_id`，当前 whale 只是碰巧相等。三项需按现有 schema 语义修正，不改变底层状态机。

## §1 游玩影响

玩家看到错误 healer 行为、错误境界称谓和不稳定的易形模型；跨端状态存储可能选择错误模型。没有新增真元或 agent 推演。

## §2 复现路径

1. 让 healer 处于非施法但有动作/目标的状态，观察 HUD payload 仍是 `healer_npc_decision(0, 0.0, false)`。
2. 打开化虚境界 boss HUD，观察显示“洞虚”。
3. 让一个 race 的 body plan 与 race id 不同，比较 `form_body_plan_id` 与 `resolve_body_plan` 结果。

## §3 今天 `origin/main` 的证据

- `server/src/network/yidao_state_emit.rs:195-209` 在 `:202` 对非施法态使用 `healer_npc_decision(0, 0.0, false)`。
- `server/src/network/tsy_polish.rs:243-251` 在 `:250` 将 `Realm::Void` 显示为“洞虚”。
- `server/src/network/morph_state_emit.rs:52-67,169-183` 两处以 `state.form.as_str()` 填 `form_body_plan_id`；`MorphState.form` 是 RaceId（`server/src/body_plan/morph.rs:42-51`），真实解析入口是 `body_plan::resolve::resolve_body_plan`/`resolve_race_to_plan`。

## §4 非重复比对

已查 yidao HUD、正典境界、race/morph finished/active plans 与 skeleton；没有同一字段语义骨架。#1413、#1394、#1376 共享“wire 字段必须表达其声明语义”，但每项保留独立验收。

## §5 立项检查记录

- `docs/worldview.md`：查六境界（化虚）、医道 HUD 与易形/身形关键词。
- `docs/finished_plans/`：查 yidao、race system、morph payload 契约。
- active plan：查 `healer_npc_decision`、`Realm::Void`、`form_body_plan_id`/`resolve_body_plan`，未见同一入口改动。
- skeleton：查三个 issue 号及字段名，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无 state semantics 条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：真实 healer decision、`Realm`、`MorphState.form`/body-plan registry。
- **Outputs**：`ServerDataPayloadV1::YidaoHudState`、realm HUD 文本、`ServerDataPayloadV1::MorphState`。
- **共享类型/事件**：复用 `HealerNpcAiStateV1`/`YidaoHudStateV1`、`Realm`、`MorphStateEntryV1` 与 `resolve_body_plan`；不新增同义字段。
- **三端契约符号**：server `yidao_state_emit`、`tsy_polish`、`morph_state_emit`；agent `HealerNpcAiStateV1`、`ServerDataMorphStateV1` schema 保持字段含义；client `HealerNpcAiStateHandler`/`MorphStateHandler`、`MorphStateStore` 接收真实值；agent/client **无 schema 字段新增**。
- **worldview 锚点**：六境界正典必须写“化虚”；易形模型对应身形而非 race id；不涉及 qi_physics。

## P0 验收

- healer 非施法态仍反映真实 action/target，空状态只在真实为空时出现。
- `Realm::Void` 的所有 player-facing 文本为“化虚”。
- `form_body_plan_id` 与 `resolve_body_plan` 的结果一致，不能以 race id 巧合通过。
