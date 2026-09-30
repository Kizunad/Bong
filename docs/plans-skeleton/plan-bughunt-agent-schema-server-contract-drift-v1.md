# BugHunt：agent schema 与 server IPC 枚举/可空字段漂移

> 来源 Issue：#1734、#1728、#1603、#1361。四条都是 TypeBox validator 与今天 Rust producer/consumer 不一致，合并为一份 schema contract drift 骨架。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 对拍 null、NPC archetype、cultivation death cause 与 heartbeat event type | ⬜ |
| P1 | 重建 generated schema、补正反向 validator/IPC 测试 | ⬜ |

## §0 摘要

agent schema 把 server 合法 payload 判成非法：`InsightRequestV1` 的 optional `secondary` 不接受 server 序列化的 `null`；`NpcArchetypeV1` 漏掉 `dying_elder/mundane`；死亡 cause union 与 Rust 枚举不一致；heartbeat override 白名单漏 `tide_sky_omen`。结果是顿悟/心魔、凡兽/NPC 事件、部分死亡事件或天象命令被静默丢弃。

## §1 游玩影响

不同事件表现为天道没有反馈、世界模型缺记录或 agent 命令无效；这会造成跨仓状态误判，但不直接改变 server 的权威修炼和真元结算。

## §2 复现路径

1. 让玩家没有 secondary qi color，server 发 `"secondary": null`，调用 `validateInsightRequestV1Contract`。
2. 让 server 发布 `NpcDeathV1.archetype = "mundane"` 或 `"dying_elder"`。
3. 发布 Rust `CultivationDeathCause` 中 agent union 没有的生产 cause。
4. 发 `heartbeat_override.params.event_type = "tide_sky_omen"`，经过 `validateAgentCommandV1Contract`。

## §3 今天 `origin/main` 的根因证据

- `agent/packages/schema/src/insight-request.ts:7-15` 用 `Type.Optional(ColorKind)` 表示 secondary；`server/src/network/cultivation_bridge.rs:154-163` 与 `server/src/cultivation/tribulation.rs:4035-4045` 在无次色时序列化 `null`。
- `agent/packages/schema/src/npc.ts:5-16` 没有 `dying_elder/mundane`；server `server/src/npc/lifecycle.rs:80-100,149-163` 产生这两类 archetype，`server/src/network/npc_event_bridge.rs:62-86` 经 `bong:npc/death` 发布。
- `agent/packages/schema/src/cultivation-death.ts:6-13` 只有六个 cause；Rust `server/src/cultivation/death_hooks.rs:28-44` 另有 `DevCommand/SwarmQiDrain/VoidQuotaExceeded/VoidActionBacklash` 等实际变体，`server/src/network/cultivation_bridge.rs:179-190` 用 debug 名称发出。
- `agent/packages/schema/src/agent-command.ts:88-101` 的 heartbeat event type 白名单漏 `tide_sky_omen`；Rust `server/src/world/heartbeat.rs:88-98,822-872` 支持并执行该值。

## §4 非重复比对与立项检查记录

- **worldview**：查 `docs/worldview.md` 的“天道事件”“NPC 生死”“顿悟/心魔”“天象”关键词；只同步既有协议，不改变正典事件含义或真元规则。
- **finished_plans**：查 `plan-ipc-schema-v1.md`、`plan-agent-v1/v2`、NPC lifecycle、tribulation 与 heartbeat 归档；确认没有一份同时覆盖这四个当前 schema 漂移点。
- **active plan**：查 `docs/plan-*.md` 的 `InsightRequestV1`、`NpcArchetypeV1`、`CultivationDeathCause`、`tide_sky_omen`；未发现正在修这组 union/null 边界的 active plan。
- **skeleton**：查 `agent/packages/schema`、generated JSON、Rust `schema/` 与四个 producer；无同名综合骨架。
- **reminder.md**：已查 `docs/plans-skeleton/reminder.md`；没有 schema drift 或 heartbeat 条目。

## §5 修复骨架

- P0：以今天 server producer 的真实 JSON 为准决定 optional/null 语义，补齐 union，并明确 `CultivationDeathCause` 的 wire 命名映射；不得只改 TypeScript 类型而不改 generated artifacts/Rust mirror。
- P1：在 schema tests、server serialization tests 与 agent consumer tests 中加入每个新增值的 positive case 以及未知值的 fail-closed case；重建注册表和 samples。
- 验收：四类合法 payload 都能通过 agent validator 并进入既有 runtime；未知枚举仍被拒绝；Rust/TypeBox/generated JSON 三方一致。

## §6 接入面与跨仓契约

- **Inputs**：`bong:insight_request`/`bong:cultivation_death`、`bong:npc/death`、`AgentCommandV1.heartbeat_override`；对应 TypeBox payload 与 Rust serde mirror。
- **Outputs**：`validateInsightRequestV1Contract`、`validateNpcDeathV1Contract`、`validateCultivationDeathV1Contract`、`validateAgentCommandV1Contract` 的结果，以及进入 `insight-runtime`/`offscreen-war-narration`/command consumer 的合法事件。
- **共享类型或事件**：`InsightRequestV1/QiColorStateV1`、`NpcArchetypeV1/NpcDeathV1`、`CultivationDeathV1`、`Command`/heartbeat union；generated JSON 与 server schema mirror 必须同源更新。
- **server 符号**：`publish_insight_requests`、`CultivationDeathTrigger`、`publish_npc_death_events`、`HeartbeatEventKind::from_wire`、`apply_heartbeat_override_command`、`CH_INSIGHT_REQUEST/CH_CULTIVATION_DEATH/CH_NPC_DEATH`。
- **agent 符号**：`insight-runtime.ts`、`redis-ipc.ts`/`offscreen-war-narration.ts`、`validate*Contract` helpers、`parseAgentCommand`/`npc-producer.ts`。
- **client**：无变更；这些是 server↔agent Redis payload，Fabric client 不解析上述 union，依据现有 `server_data`/chat 链路。
- **worldview / qi_physics**：事件名称与状态按既有 worldview 锚点对拍；不新增真元流，死亡和天象的真元处理仍由 server 现有 `qi_physics` 入口负责。
