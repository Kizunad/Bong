# BugHunt：灵宝对话的 equipped 好感门未约束 LLM 增量

> 来源 Issue：#1767。修复灵宝对话中“未装备不得增加好感”的 server→agent→server 契约，不把 fallback 文案当作唯一门禁。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 在权威应用边界按 `context.equipped` 约束 `affinity_delta` | ⬜ |
| P1 | 装备/未装备、LLM/fallback 与边界值回归 | ⬜ |

## §0 摘要

`SpiritTreasureDialogueRuntime.produceDialogue` 对 LLM 返回的 `affinity_delta` 只做 `[-0.1,0.1]` clamp。请求里的 `context.equipped` 没有参与决策，未装备玩家仍可通过 LLM 的正增量抬高灵宝好感；同文件 `fallbackDialogue` 对未装备返回 `-0.01`，反而暴露出预期门禁未被生产 LLM 路径复用。

## §1 游玩影响

玩家把灵宝放在背包而非身上时，连续对话仍可能获得正好感和后续被动效果，绕过“装备在身才增好感”的玩法边界；只影响好感数值与对话反馈，不直接改变真元总量。

## §2 复现路径

1. server `chat_collector` 以 `active.equipped == false` 构造灵宝对话请求。
2. LLM 返回合法 JSON 且 `affinity_delta: 0.1`。
3. `produceDialogue` 在 `:190` 直接 clamp 后返回；server `process_spirit_treasure_dialogue` 在 `:76` 调 `apply_affinity_delta`，没有再次按装备状态拒绝。

## §3 今天 `origin/main` 的根因证据

- `agent/packages/tiandao/src/spirit-treasure-dialogue-runtime.ts:164-191` 解析 LLM 后只 clamp `affinity_delta`；`fallbackDialogue:249-255` 在 `!request.context.equipped` 时才返回负增量。
- `server/src/network/chat_collector.rs:423-445` 把 `active.equipped` 放进 `SpiritTreasureDialogueContextV1`；`server/src/network/spirit_treasure_emit.rs:46-86` 的 `process_spirit_treasure_dialogue` 无条件把 dialogue 增量传给 `registry.apply_affinity_delta`。
- 共享 schema `agent/packages/schema/src/spirit-treasure.ts:34-80` 已声明 `equipped` 与 `affinity_delta`，所以不是字段缺失，而是权威应用边界漏检查。

## §4 非重复比对与立项检查记录

- **worldview**：查 `docs/worldview.md` 的“灵宝”“装备”“好感”“真元”关键词；确认本骨架只修装备门，不新增经济或修炼规则。
- **finished_plans**：查 `plan-spirit-treasure-v1.md`、`plan-agent-v2.md`、灵宝状态/对话归档计划的 affinity 与 equipped 章节；没有已落地的 LLM 增量门禁。
- **active plan**：查 `docs/plan-*.md` 的 `apply_affinity_delta`、`SpiritTreasureDialogueContextV1`；未发现同一缺口的 active 修法。
- **skeleton**：查 `spirit-treasure-dialogue-runtime.ts`、`equipped`、`affinity_delta`；没有同根因骨架。
- **reminder.md**：已查 `docs/plans-skeleton/reminder.md`；无灵宝对话或 affinity 条目。

## §5 修复骨架

- P0：在 server 权威应用入口使用请求中的装备快照重新核验当前持有状态；未装备不得接受正的 LLM 增量，可复用既有 fallback 惩罚语义，但不能只依赖 agent 自报。
- P1：agent 与 server 都保留 `equipped`/增量的契约测试；覆盖装备、未装备、LLM 正/负/非法增量和 cooldown，防止只修 fallback。
- 验收：未装备请求不产生正好感；装备请求仍可按 clamp 后增量更新；响应和状态 payload 与实际 registry 状态一致。

## §6 接入面与跨仓契约

- **Inputs**：`bong:spirit_treasure_dialogue_request`、`SpiritTreasureDialogueRequestV1.context.equipped`、`affinity` 与 LLM JSON。
- **Outputs**：`bong:spirit_treasure_dialogue`、`SpiritTreasureDialogueV1.affinity_delta`、server `ServerDataPayloadV1::SpiritTreasureDialogue` 与状态刷新。
- **共享类型或事件**：TypeBox `SpiritTreasureDialogueRequestV1/DialogueV1`、Rust 对应 schema、`ActiveSpiritTreasures`；不另造 equipped 字段。
- **server 符号**：`collect_player_chat`、`process_spirit_treasure_dialogue`、`ActiveSpiritTreasures::apply_affinity_delta`、`CH_SPIRIT_TREASURE_DIALOGUE_REQUEST/CH_SPIRIT_TREASURE_DIALOGUE`；server 是最终好感门。
- **agent 符号**：`SpiritTreasureDialogueRuntime.produceDialogue`、`fallbackDialogue`、`validateSpiritTreasureDialogueRequestV1Contract` 与 `validateSpiritTreasureDialogueV1Contract`。
- **client**：无变更；client 只显示 `ServerDataPayloadV1::SpiritTreasureDialogue/State`，不拥有好感结算。
- **worldview / qi_physics**：对应灵宝与装备语义；本骨架不新增真元流、不调用 ledger，不改 `SPIRIT_QI_TOTAL`。
