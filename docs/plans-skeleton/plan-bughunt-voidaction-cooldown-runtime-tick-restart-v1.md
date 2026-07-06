# Bong · plan-bughunt-voidaction-cooldown-runtime-tick-restart-v1

> **Skeleton Plan**。一句话主题：化虚动作冷却把基于运行时 tick 的绝对 `ready_at_tick` 写进 SQLite，但 `CombatClock` / `CultivationClock` 重启后从默认 0 重新计数，hydrate 又原样回灌旧 tick，导致服务端成功施放并持久化后的 Barrier / SuppressTsy / ExplodeZone 冷却在重启后额外叠加施放前服务器 uptime。

## 实际游玩体验影响

- 化虚玩家成功施放世界级动作后，如果服务器在冷却期间重启，服务端会继续按旧 uptime 里的绝对 tick 判定冷却。玩家看到的是“明明只该等 7/30/90 天的动作，重启后还要多等服务器之前已经运行过的时间”。
- 典型场景：服务器已运行 10 天，玩家成功施放 7 天冷却的 `Barrier` 并写入 DB；若随后重启，新的运行时 tick 从 0 附近开始，服务端要等到旧 `ready_at_tick = 10 天 + 7 天` 才放行，实际变成约 17 天运行时冷却。
- 本 bug 不主张停服期间必须按墙钟流逝；最小要求只是重启不能把“施放前 uptime”叠加到已经生成的剩余冷却里。

## 复现路径

1. 启动带 SQLite persistence 的 server，让运行时 tick 前进一段可观时间，例如 10 天 tick。
2. 使用一个化虚角色成功施放 `Barrier`，确保 `resolve_void_action_intents` 成功路径执行并写入 `void_action_cooldowns`。
3. 记录 DB 中该角色 `barrier` 的 `ready_at_tick`，它会等于施放时运行时 tick 加 `BARRIER_COOLDOWN_TICKS`。
4. 重启 server，不改 DB。
5. 立刻再次尝试 `Barrier`，服务端 `precheck_void_action` 仍用重启后较小的运行时 tick 与旧 `ready_at_tick` 比较，拒绝为 `OnCooldown`。
6. 推进 7 天运行时 tick 后仍未放行；必须再补上重启前 uptime 才会通过。

备注：`SuppressTsy` 同机制受影响，但正式 UI 入口受既有 `plan-voidaction-target-zone-lock-v1` 的 target-zone 问题干扰；本 plan 的复现主例用 `Barrier`，避免和 #880 的 client 伪冷却混淆。

## 根因证据

- `server/src/network/client_request_handler.rs:696-700`：`ClientRequestV1::VoidAction` 入队时写 `requested_at_tick: combat_clock.tick`。
- `server/src/cultivation/void/actions.rs:190-199`：服务端实际判定 tick 是 `intent.requested_at_tick.max(clock.tick)`，再把 `cooldowns.ready_at(&actor_id, kind)` 放进 precheck。
- `server/src/cultivation/void/actions.rs:111-115`：`precheck_void_action` 只做 `now_tick < ready_at_tick` 的绝对 tick 比较，命中即 `OnCooldown`。
- `server/src/cultivation/void/actions.rs:289-294`：施放成功后 `cooldowns.set_used(..., now_tick)`，随后把 `ready_at_tick` 持久化。
- `server/src/cultivation/void/components.rs:12-16`：三类有冷却的化虚动作分别是 7 / 30 / 90 天 tick。
- `server/src/cultivation/void/components.rs:210-218`：`set_used` 写入 `now_tick.saturating_add(cooldown)`，即运行时 tick 的绝对 ready time。
- `server/src/persistence/mod.rs:2619-2633`：SQLite 表写入 `ready_at_tick` 和 `last_updated_wall`。
- `server/src/persistence/mod.rs:2647-2648`：hydrate 读取只取 `character_id, kind, ready_at_tick`，没有读取 `last_updated_wall` 做墙钟或相对剩余量换算。
- `server/src/persistence/mod.rs:2685-2690`：hydrate 原样 `force_ready_at(record.ready_at_tick)`。
- `server/src/combat/mod.rs:206`、`server/src/combat/debug.rs:12-13`：`CombatClock` 默认插入并从 0 运行时自增。
- `server/src/cultivation/mod.rs:236-237`、`server/src/cultivation/tick.rs:38-41,130`：`CultivationClock` 同样默认插入并从 0 运行时自增。
- `server/src/persistence/mod.rs:7898-7913`：现有回归只断言 `ready_at_tick = 12_345` roundtrip，缺少“旧 uptime 施放 + 重启后剩余冷却不增加”的行为 pin。

## 修复计划骨架

- [ ] 明确定义化虚动作冷却的跨重启语义：至少保证重启不增加剩余冷却；是否让停服墙钟流逝作为产品决策单独拍板。
- [ ] 改造 `void_action_cooldowns` 的持久化/恢复语义，避免直接把旧运行时 tick 原样当成新运行时 tick 的绝对 ready time。候选方案：
  - 存相对剩余 tick，并在 shutdown / hydrate 时按当前运行时 tick 重建；
  - 或存 `used_at_wall` / `cooldown_ticks`，hydrate 时按墙钟和运行时策略计算剩余；
  - 或持久化全局单调 tick，但需审计所有依赖 `CombatClock` / `CultivationClock` 的模块，避免把一次局部修复扩大成全局时间迁移。
- [ ] 保留 `last_updated_wall` 的用途或删除误导字段；若采用墙钟补偿，补齐读取和迁移。
- [ ] 把 #880 的 client target-zone / 本地伪冷却与本 plan 的 server persistence 冷却分开修，避免一个 PR 同时承载前端路由和服务端时间基准。

## 验证计划

- [ ] server 单测：在 `CombatClock` / `CultivationClock` 旧 uptime 为 10 天时成功施放 `Barrier`，持久化后模拟重启为 tick 0，hydrate 后断言 7 天 tick 到达即可通过，而不是 17 天。
- [ ] server 单测：`SuppressTsy`、`ExplodeZone` 同样覆盖 30 / 90 天冷却，不因旧 uptime 增加。
- [ ] persistence 单测：`void_action_cooldowns` 读取路径覆盖 `last_updated_wall` 或相对剩余量字段，禁止只 roundtrip 原始 `ready_at_tick`。
- [ ] 回归：无 persistence resource 时仍只 memory-only warning，不引入 panic。
- [ ] e2e/协议级复现：成功 `Barrier` → 重启 → 冷却状态按修正后的语义恢复；并确认此测试不依赖 #880 的 target-zone UI 修复。

## 对抗结论

- Round 1 反方质疑：候选最初把时间源说窄了，实际生产入口来自 `CombatClock.tick`，服务端再与 `CultivationClock.tick` 取 max；还需要限定为“成功施放且 persistence 启用”，并和 #880 的 client 伪冷却做去重。
- Round 2 修正/反驳：候选改为“运行时 tick 基准重启归零”，并把复现主例改为 `Barrier`；不要求停服墙钟流逝，只要求重启不增加已生成冷却。`last_updated_wall` 虽写入但 hydrate 不读取，不能构成隐藏补偿路径。
- 最终裁决：反方确认候选成立，当前服务端把化虚动作冷却持久化为基于运行时 tick 的绝对 `ready_at_tick`，但 `CombatClock` / `CultivationClock` 跨重启均从默认值重新计数，hydrate 又原样 `force_ready_at(record.ready_at_tick)`，因此会额外叠加施放前 uptime；该根因不同于 #880，适合单独立 persistence skeleton。
