# plan-bughunt-wanted-player-outbox-retry-v1

> **来源 issue**：#1562。`wanted_player` 事件入 Redis 失败后只告警并丢弃；Wanted 边界事件只发一次，瞬时队列/连接故障会永久漏掉 agent 通知。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | wanted_player 的 durable pending/retry/dead-letter 生命周期 | ⬜ |
| P1 | 发送失败、重连、重复事件和成功后去重测试 | ⬜ |

## §0 摘要

`emit_wanted_player_to_redis` 读取 `IdentityReactionChangedEvent`，构造 `WantedPlayerEventV1` 后直接 `redis.tx_outbound.send`。send 失败只写 warn，事件不会再次产生；这与高声望 tracker 的“成功后才记 emitted”语义不一致。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` §十一的通缉/江湖传播语义；本骨架只保证既有事件可靠送达。
- **finished_plans**：查 `plan-identity-v1`、`plan-social-renown-identity-bridge-v1`、Redis bridge resilience/agent pipeline；它们定义 payload 或通用 agent retry，不覆盖该 server producer 的丢事件窗口。
- **active plan**：查 identity/social renown 与 Redis outbox；未见 wanted_player 自身 pending state。
- **skeleton**：查 `plan-bughunt-k2-identity-social-renown-bridge-v1`、`plan-bughunt-agent-narration-pipeline-v1`、`RedisOutbound::WantedPlayer`；本骨架不重复名声双账本。
- **reminder.md**：查 wanted、Redis、outbox、retry；仓内有该文件，无本 issue 登记。

## §2 接入面与跨仓契约

- **Inputs**：`IdentityReactionChangedEvent`、`PlayerIdentities`、`Lifecycle.character_id`、`WantedPlayerEventV1`、`RedisBridgeResource::tx_outbound`。
- **Outputs**：成功 publish 或 durable pending/retry/dead-letter；同一事件保持稳定 dedupe key，不重复广播。
- **共享类型/事件**：复用 `IdentityReactionChangedEvent`、`WantedPlayerEventV1`、`RedisOutbound::WantedPlayer`、现有 Redis bridge pending/outbox 语义；不新增另一份 wanted schema。
- **三端契约符号**：server `identity::wanted_player_emit::emit_wanted_player_to_redis`/`build_wanted_player_event`；agent 继续消费 `bong:wanted_player` 与 `WantedPlayerEventV1`，**无 schema 变更**；client **无变更**，identity panel 不参与该 pub。
- **worldview/qi**：通缉是 identity/social 语义，不新增 qi 流动；若修复触及 outbox 的账本 telemetry，仍不得改 `SPIRIT_QI_TOTAL`。

## §3 游玩影响与复现

在 Wanted tier 事件发生时让 `tx_outbound` 满或断开 Redis；当前日志出现失败后，agent 永远收不到该玩家的通缉事件，即使随后恢复连接。

## §4 `origin/main` 根因证据

- `server/src/identity/wanted_player_emit.rs:80-111` 只对 `send` 返回错误打 warn；没有 pending、retry 或成功标记。
- `IdentityReactionChangedEvent` 只在 tier 跨入 Wanted 时产生，`wanted_player` 不会由后续 tick 自动重发；对照 `social/high_renown_tracker.rs:80-93,240-266`，其 emitted 状态在成功后才推进。

## §5 非重复比对

`plan-bughunt-k2-identity-social-renown-bridge-v1` 修 identity/social 名声双写；`plan-agent-narration-pipeline-v1` 修 agent runtime 的 publish cursor；本骨架只修 server→Redis wanted producer 的交付可靠性。

## §6 修复计划骨架

- **P0**：在既有 Redis bridge/outbox 中登记稳定 `(char_id, identity_id, at_tick/event_id)`，send 失败保留 pending 并按既有重连重试；达到年龄/次数上限进入可观测 dead-letter，不静默删除。
- **P1**：成功后才提交 dedupe 状态；同一事件重试不换新 id；测试覆盖 channel 满、断线重连、重复 EventReader、dead-letter 和 agent 收到一次。

## §7 验证计划

实现后跑 server fmt/clippy/cargo test，并执行 Redis IPC 契约测试；schema/agent/client 无变更则不跑对应编译门禁。
