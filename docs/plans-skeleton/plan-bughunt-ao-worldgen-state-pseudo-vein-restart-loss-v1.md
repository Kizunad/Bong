# plan-bughunt-ao-worldgen-state-pseudo-vein-restart-loss-v1（骨架）

> **骨架（草案）**。一句话主题：`worldgen/state` 主路径确认 1 个高置信真 bug：**心跳生成的伪灵脉 runtime zone 在重启后整体丢失**。`WorldHeartbeat` 只在内存里持有 `active_pseudo_veins`，持久化层只落 `zone_id/spirit_qi/danger_level`，hydrate 也只会回填到已存在静态 zone，导致 `pseudo_vein_*` 这种动态 zone 在关服/崩溃后直接蒸发。对实际游玩体验的影响明确：玩家眼前的伪灵脉、高灵气修炼窗口、预警/消散/余波链和后续兽潮诱发都会被重启硬切断。

> 立项动机：按用户限定只扫 `worldgen/state` 主路径，并避开已禁重主题（zone atmosphere mismatch / world environment resync / ambient audio stale anchor）。本条落点集中在 `server/src/world/heartbeat.rs`、`server/src/world/zone.rs`、`server/src/persistence/mod.rs`，是可达、可复现、能直连实际游玩链路的 runtime-state 缺口。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 伪灵脉 runtime zone 重启丢失 | fix_pr | ⬜ |

## P0 - 伪灵脉 runtime zone 重启丢失

- **#1 major（fix_pr）**：`server/src/world/heartbeat.rs:1697-1748` 在 omen 落地时会：
  - 生成 `pseudo_vein_heartbeat_<n>` 名字；
  - 构造带 `active_events=["pseudo_vein"]`、`danger_level=4`、自定义 AABB 的 `Zone`；
  - 调 `ZoneRegistry::register_runtime_zone(zone)` 追加到运行时 zone 列表；
  - 同时把生命周期塞进 `WorldHeartbeat.active_pseudo_veins`。
- 但 `server/src/world/heartbeat.rs:303-339` 可见 `active_pseudo_veins` / `next_pseudo_vein_index` 全是纯内存字段，`Default` 启动即清空；全仓无任何 hydrate / restore 路径。
- `server/src/world/heartbeat.rs:1002-1048` 又把后续真实玩法都绑在这份内存态上：衰减 `zone.spirit_qi`、发 warning omen、在耗尽时 `send EventChainTrigger::PseudoVeinDissipated`。
- 持久化层并没有保存“动态 zone 本体”：
  - `server/src/persistence/mod.rs:471-475` `ZoneRuntimeRecord` 只有 `zone_id/spirit_qi/danger_level` 三列；
  - `server/src/persistence/mod.rs:2983-3000` `persist_zone_runtime_snapshot()` 也是按这三列写表；
  - `server/src/world/zone.rs:376-381` `apply_runtime_records()` 只会对**已存在** zone 做 `find_zone_mut()` 回填 `spirit_qi/danger_level`；
  - `server/src/persistence/mod.rs:3086-3092` hydrate 只是 `load_zone_runtime_snapshot()` 后调用上面的回填函数，不会新建 runtime zone；
  - `server/src/persistence/mod.rs:725-732` startup bootstrap 也只跑这条回填。
- 结论：只要伪灵脉存在期间发生关服/崩溃/热重启，`zones_runtime` 里的 `pseudo_vein_*` 行即便还在，启动后也找不到对应静态 zone 载体，回填被静默跳过；`active_pseudo_veins` 也从空表开始，整条生命周期永久断线。

## 这个 bug 对实际游玩体验的影响

- 玩家正在赶往或站在伪灵脉里时，一次重启就会让**眼前的高灵气热点直接消失**，重连后只剩外层荒野/原 zone，不是“继续衰减中的同一事件”。
- 伪灵脉的核心收益是诱饵修炼窗口。设计文档明确把它定义为 transient zone + 30/15 分钟生命周期 + 真正可用的高灵气突破点（`docs/finished_plans/plan-terrain-pseudo-vein-v1.md:32-36,55-57`；`docs/finished_plans/plan-world-heartbeat-v1.md:328-332`）。重启后丢失意味着玩家冲刺固元/凝脉窗口会被服务器重启硬吃掉。
- 由于 `PseudoVeinDissipated` 根本不会再发，后续“消散 -> 周边灵气回涨 -> 可能触发兽潮链”也一起断掉（`server/src/world/heartbeat.rs:724-735`；`docs/finished_plans/plan-world-ecology-events-v1.md:178-184`）。体感上就是世界事件被关服硬中断，世界状态前后不连续。

## 反方裁决摘要（两轮）

### 第一轮反方：这会不会只是“临时事件不保活”的设计选择？

- **反方意见**：伪灵脉本来就是 transient zone，重启消失也许是允许的。
- **裁决**：不成立。代码和 plan 都把它定义成**多分钟持续、会衰减、会发 warning、会在耗尽时产生后续链式事件**的运行时世界状态，而不是一次性特效。`advance_active_pseudo_veins()` / `PseudoVeinDissipated` 已经说明它应当“继续存在直到自然消散”，不是“服务器一重启就判事件结束”。

### 第二轮反方：是否已由 `zones_runtime` 快照足够恢复？

- **反方意见**：`zones_runtime` 至少保存了 `zone_id/spirit_qi/danger_level`，也许足够把伪灵脉接回来。
- **裁决**：不成立。`apply_runtime_records()` 只 mutate 既有 zone，不创建缺失 zone；`ZoneRuntimeRecord` 也不含 bounds/dimension/active_events/patrol anchor/生命周期 tick。即使数据库里残留 `pseudo_vein_heartbeat_0`，启动后没有对应 `Zone` 实例承载，hydrate 只能跳过；`WorldHeartbeat.active_pseudo_veins` 同时清空，衰减计时与消散触发都回不来。

## 修复方向（供后续 fix_pr 选型）

1. 最稳修法：为 heartbeat 伪灵脉补一份专门的 runtime persistence/hydrate，至少保存 `zone_id/dimension/bounds/active_events/center_xz/spawned_at/last_tick/qi_current/warning_sent/dissipated/next_index`，启动时先重建 runtime zone，再恢复 `active_pseudo_veins`。
2. 低配但不完整的修法：把动态 zone 本体并入 zone persistence（不止三列），并在 hydrate 时允许“记录驱动创建 zone”；但仍需额外恢复 heartbeat 生命周期，否则只会得到一个不会继续衰减/不会消散的僵尸伪灵脉。

## 审计来源

bughunt AO（2026-07-05，`bughunt-loop-20260705-ao-worldgen-state`，scope 限定 `worldgen/state` 主路径）。本轮选择 report-only：只新增 skeleton，不修代码。结论来自对 `server/src/world/heartbeat.rs`、`server/src/world/zone.rs`、`server/src/persistence/mod.rs` 的实地交叉核对，并补两轮反方证伪后保留。
