# plan-bughunt-network-delivery-safety-minor-cleanups-v1（骨架）

> **来源 issue**：#1437、#1400、#1350。三条都要求 network/command 出错时 fail closed：Redis 失败不能继续广播、物品入包失败不能销毁、调试音效必须经过现有 operator gate。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 外部发送失败停止 S2C；奖励有可靠落点；`/audio` 接入 operator/dev 授权 | ⬜ |

## §0 摘要

`zone_environment_bridge` 在 Redis send 失败后仍向 client 发状态，可能每 20Hz 重复风暴；tuike ash 入背包失败只 warn，随后 residue 被 despawn，物品丢失；`/audio` 入口没有调用 `DevCommandPermissions::is_operator`。三者都是“副作用失败仍继续”的交付安全问题。

## §1 游玩影响

玩家会收到没有 agent 持久化依据的环境状态、丢失合法蜕壳奖励，普通玩家还能触发生产调试音效。前两项影响状态可信度/物品可靠性，第三项是权限边界问题。

## §2 复现路径

1. 让 Redis publish 返回 error，观察 `zone_environment_bridge` 仍发送 S2C 并在下一个 tick 重试广播。
2. 背包满或玩家离线时触发 tuike ash，观察日志 warn 后 `FalseSkinResidue` 直接 despawn。
3. 非 operator 执行 `/audio`，观察 `audio_event_emit` 构造并广播音效。

## §3 今天 `origin/main` 的证据

- `server/src/network/zone_environment_bridge.rs:18-71` 在 `:44-52` send error 后仍走 `:54-64` S2C。
- `server/src/network/tuike_ash_emit.rs:38-66` 入包失败只 warn；`server/src/combat/tuike_v2/tick.rs:222-240` 随后直接 despawn `FalseSkinResidue`。
- `server/src/network/audio_event_emit.rs:165-228` 未检查 `DevCommandPermissions::is_operator`；现有权限原语在 `server/src/cmd/dev/mod.rs:70-120,193-272`。

## §4 非重复比对

已查 Redis bridge、tuike loot、dev command 的 finished/active plans 与 skeleton；没有共同骨架已覆盖这三个 fail-closed 入口。实施时按三个独立子项保留各自契约测试，来源仅 #1437/#1400/#1350。

## §5 立项检查记录

- `docs/worldview.md`：查区域环境、蜕壳物品化和 dev-only 命令边界；不改生产世界观。
- `docs/finished_plans/`：查 Redis IPC、tuike-v2、dev command 权限计划。
- active plan：查 `zone_environment_bridge`、`FalseSkinResidue`、`DevCommandPermissions`，未见同一入口改动。
- skeleton：查 #1437、#1400、#1350 及 `fail closed`、`is_operator`、`despawn`，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无交付安全条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：Redis publish result、玩家 inventory/online 状态、命令 sender 与 `DevCommandPermissions`。
- **Outputs**：成功发送才产生 zone S2C；奖励必须进入背包/可靠待领落点后才 despawn；未授权 `/audio` 不构造广播。
- **共享类型/事件**：复用 `RedisOutbound`/zone environment payload、`FalseSkinResidue`、inventory add result、`DevCommandPermissions::is_operator`；不新造 bypass。
- **三端契约符号**：server `zone_environment_bridge`、`tuike_ash_emit`、`audio_event_emit`；agent `ZoneEnvironmentStateV1` 只在成功 Redis 路径收到事件；client `EnvironmentEffectController`/音频 handler 只消费成功状态；未授权路径 agent/client **无变更**。
- **worldview/qi_physics 锚点**：蜕壳残余若携带真元必须按既有 ledger 物品化/回灌路径处理，不得因失败凭空吞没；本计划不新增 transfer。

## P0 验收

- Redis 失败不发对应 S2C，也不在无状态确认时重复广播。
- 背包满/离线时奖励留在可重试的权威落点，实体不先 despawn。
- `/audio` 在授权检查前不构造或发送任何调试音效 payload。
