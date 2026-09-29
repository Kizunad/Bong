# plan-bughunt-dugu-penetrate-wire-qi-v1（骨架）

> **来源 issue**：#1787。`DuguV2` 穿透事件桥接到 Redis 时把 `returned_zone_qi` 写死为 `0.0`，丢失 combat 结算的真实回灌量。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | Redis payload 携带真实 `returned_zone_qi`，并以 ledger 结算结果为唯一来源 | ⬜ |

## §0 摘要

`server/src/network/dugu_v2_event_bridge.rs` 从战斗事件构造 `RedisOutbound::DuguV2Cast` 时将 `returned_zone_qi` 固定写成 `0.0`。这使 agent 的叙事/审计看不到真实的 `ReleaseToZone` 量。实施不得在 bridge 自行计算或生成真元，必须传递 combat 已通过 `qi_physics::ledger` 得出的结果。

## §1 游玩影响

玩家实际释放后的区域灵气可能已正确结算，但 agent 世界模型和叙事看到零回灌，导致状态描述、审计和后续推演失真。该计划只修跨仓事件字段，不改变战斗伤害或真元物理。

## §2 复现路径

1. 触发 Dugu penetrate 产生 `DuguV2Cast`，使 combat 计算非零 `returned_zone_qi`。
2. 查看 `dugu_v2_event_bridge` 构造的 Redis outbound，字段仍是 `0.0`。
3. 在 agent 消费端观察 payload 与 server combat 结算不一致。

## §3 今天 `origin/main` 的证据

- `server/src/network/dugu_v2_event_bridge.rs:70-86` 的 bridge 在 `:82` 写 `returned_zone_qi: 0.0`。
- `server/src/combat/dugu_v2/events.rs:84-98` 已有该字段；生产填写在 Dugu `skills.rs` 的结算事件中。
- 真实真元转账仍由 combat/`qi_physics::ledger` 负责，bridge 只是序列化边界。

## §4 非重复比对

已查 `docs/finished_plans/plan-combat-*`、active plan、skeleton 中的 `DuguV2Cast`、`returned_zone_qi`、`CH_DUGU_V2_CAST`；没有覆盖该字段丢失的骨架。#1787 是唯一来源，和其他 combat 守恒计划不重复：本计划不重写 ledger。

## §5 立项检查记录

- `docs/worldview.md`：查真元守恒、区域灵气与招式释放；确认回灌量必须来自 ledger。
- `docs/finished_plans/`：查 Dugu v2 与 Redis IPC 计划，确认 payload 的现有字段。
- active plan：查 `returned_zone_qi`、`RedisOutbound::DuguV2Cast`、`DuguV2Cast`，未发现同一 bridge 的 active 修改。
- skeleton：查 #1787、`dugu_v2_event_bridge.rs`、`CH_DUGU_V2_CAST`，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无 Dugu bridge 条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：combat `DuguV2CastEvent` 的 caster/target、`returned_zone_qi` 以及已经完成的 ledger 结算。
- **Outputs**：`RedisOutbound::DuguV2Cast`，发布到 `CH_DUGU_V2_CAST`，字段值原样保真。
- **共享类型/事件**：复用 `server/src/combat/dugu_v2/events.rs` 的事件字段、`DuguV2Cast` schema 与 `RedisOutbound`；不得另造 qi 计算字段。
- **三端契约符号**：server `dugu_v2_event_bridge`、`RedisOutbound::DuguV2Cast`、`CH_DUGU_V2_CAST`；agent 消费该 Redis payload 并更新 world model；client **无变更**（该事件不是客户端 S2C）。
- **worldview/qi_physics 锚点**：`docs/worldview.md` 真元总量恒定；实施只传递已由 `qi_physics::ledger::QiTransfer`（原因为实际 combat reason）结算的数值，禁止 bridge 二次入账或凭空生成。

## P0 验收

- 非零与零两种 combat 结果在 Redis payload 中都与事件完全一致。
- agent 端读到的 `returned_zone_qi` 可审计回对应 ledger transfer；bridge 不执行任何 transfer。
- `CH_DUGU_V2_CAST` 的字段名和序列化兼容现有消费者。
