# BugHunt: offscreen-war Redis 观测频道缺 TS shared schema parity

## 一句话

`server` 已公开发布 `bong:npc/combat` 与 `bong:npc/relic` 两条 offscreen-war Redis V1 观测频道，但 `agent/packages/schema` 没有对应 channel 常量、TypeBox payload、validator、sample/generated registry 与 pin 测试，TS 侧无法通过 `@bong/schema` 类型化导入或契约校验这两个公共观测面。

## 实际游玩体验影响

离屏战争结算、真元回灌与战场遗物落盘本身不依赖 Tiandao runtime，因此这不是“玩家当场少一条 HUD/叙事”的 bug。实际影响在可观测性与验收回归：当玩家反馈“远方战死后为什么有/没有遗物、真元是否回灌”时，TS 侧天道/QA/观测工具缺少共享 wire 合约，无法可靠追踪战果与遗物来源，只能绕过 `@bong/schema` 读 raw Redis；一旦 server 字段漂移，agent/schema 测试不会拦截，容易把真实世界状态误判为天道未感知、遗物丢失或 e2e 观测假阳性。

## 复现路径

1. 触发 offscreen-war dormant combat，或直接检查发布路径：`publish_dormant_combat_events` 会把 `DormantCombatOutcomeV1` 发送到 `bong:npc/combat`，`publish_pending_dormant_relic_events` 会把 `PendingDormantRelicV1` 发送到 `bong:npc/relic`。
2. 在 TS 侧尝试从 `@bong/schema` 导入 `CHANNELS.NPC_COMBAT`、`CHANNELS.NPC_RELIC`、`DormantCombatOutcomeV1`、`PendingDormantRelicV1` 或对应 validator。
3. 结果：`agent/packages/schema` 与 `agent/packages/tiandao` 中均无这些符号，`REDIS_V1_CHANNELS` 也无法 pin 这两个 server 已发布频道。

## 根因证据

- `server/src/schema/channels.rs:74`-`88` 定义 `CH_NPC_COMBAT = "bong:npc/combat"` 与 `CH_NPC_RELIC = "bong:npc/relic"`，注释明确它们是 plan-offscreen-war-v1 P2/P3 telemetry。
- `server/src/schema/npc.rs:42`-`86` 定义 `DormantCombatOutcomeV1` 与 `PendingDormantRelicV1` wire payload。
- `server/src/network/npc_event_bridge.rs:76`-`135` 从 `DormantCombatOutcome` / `PendingDormantRelicCreated` 构造 wire payload 并送入 `RedisOutbound`。
- `server/src/network/redis_bridge.rs:994`-`1015` 把两种 `RedisOutbound` publish 到 `CH_NPC_COMBAT` / `CH_NPC_RELIC`。
- `server/src/network/mod.rs:457`-`470` 注册上述发布系统。
- `agent/packages/schema/src/channels.ts:104`-`128` 只有 `NPC_SPAWN`、`NPC_DEATH` 与 faction 系列；`agent/packages/schema/src/channels.ts:409` 起的 `REDIS_V1_CHANNELS` 也只列出 `NPC_SPAWN`、`NPC_DEATH` 与 faction 系列。
- `agent/packages/schema/src/npc.ts:52`-`89` 只有 `NpcSpawnedV1` / `NpcDeathV1`，后续直接进入 faction schema；grep `DormantCombatOutcomeV1|PendingDormantRelicV1|NPC_COMBAT|NPC_RELIC|npc/combat|npc/relic` 在 `agent/packages/schema agent/packages/tiandao` 无命中。
- `docs/finished_plans/plan-offscreen-war-v1.md:158`-`166` 写 P4 agent 叙事只复用 `bong:npc/death`，但 `docs/finished_plans/plan-offscreen-war-v1.md:322`-`323` 又声称 agent 有 `DormantCombatOutcomeV1` TypeBox。当前代码落在两者之间：server 发布了公共观测频道，TS shared schema parity 未补齐。
- 去重：近 250 个 PR 标题/分支按 `npc/combat`、`npc/relic`、`DormantCombatOutcome`、`PendingDormantRelic`、`offscreen schema` 等关键词未命中；#803 只处理 HalfStep/woliu/baomai registry 与 BoneCoinTick runtime，不覆盖本问题。

## 修复计划骨架

- [ ] P0: 在 `agent/packages/schema/src/channels.ts` 增加 `CHANNELS.NPC_COMBAT` / `CHANNELS.NPC_RELIC`，并加入 `REDIS_V1_CHANNELS`，补频道 pin 测试。
- [ ] P0: 在 `agent/packages/schema/src/npc.ts` 增加 `DormantCombatOutcomeV1` 与 `PendingDormantRelicV1` TypeBox schema、Static type、validator，字段逐一对齐 Rust serde wire。
- [ ] P0: 增加 positive/negative samples，覆盖 happy path、缺字段、未知字段、数值边界、`kind` / `v` 错误。
- [ ] P0: 更新 `schema-registry` 与 generated JSON，确保 `@bong/schema` 顶层导出可用。
- [ ] P1: 只在产品确认需要 TS runtime 观测后，再决定是否让 Tiandao/observer 订阅或缓存这两个纯 telemetry channel。本 plan 不把 Tiandao runtime 消费作为 P0，也不改变 offscreen-war 玩法结算。

## 验证计划

- `cd agent && npm run build -w @bong/schema`
- `cd agent && npm test -w @bong/schema`
- 若 P1 后续接入 Tiandao observer，再补 `agent/packages/tiandao` 对应单测；P0 schema parity 不要求 Tiandao runtime 订阅。
- 可选联调：运行 offscreen-war 观测脚本或真服 e2e，确认 raw Redis payload 可被新增 TS validator 接受，错误样本会被拒绝。

## 对抗复核结论

- 第一候选“伪灵脉 narration/IPC gap”经反方复核判定重复且边界不纯：既有 `docs/plans-skeleton/plan-module-wiring-gaps-v2.md` 已覆盖生产者/消费链路缺口，且 server 侧已有 zone-scope gameplay narration 兜底，故放弃。
- 本候选第一轮反方质疑成立：`bong:npc/combat` / `bong:npc/relic` 是纯 telemetry，server 注释明确不作为 Tiandao 派系叙事输入，不能写成玩家即时叙事缺失。
- 修正后最终裁决：PASS。收窄为 `server public Redis payload has Rust schema + publisher, but @bong/schema lacks parity`，高置信、非重复，适合 agent-schema 分区。必须强调 caveat：P0 只补共享 schema 契约，不要求 Tiandao runtime 订阅，不声称遗物落盘或玩家 HUD 当场损坏。
