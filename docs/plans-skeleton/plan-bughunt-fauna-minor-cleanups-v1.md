# plan-bughunt-fauna-minor-cleanups-v1

> **来源 issue**：#1453、#1398。兽潮 `FlowFields` 和负灵域 `GhostEntity` 的生命周期都只增不减，长期运行会积累计算/内存状态。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | flow field 与 ghost 的终态回收/registry 一致性 | ⬜ |
| P1 | 长跑容量、zone 回正、缺 zone 和重复清理测试 | ⬜ |

## §0 摘要

`flow_field_compute_system` 用 `(source,target,computed_tick)` 生成 id 并插入 `FlowFields.fields_by_id`，没有淘汰。`ghost_spawn_system` 把 entity 登记进 `GhostZoneRegistry`，`ghost_drift_system` 对正灵域/消失 zone 没有生产 cleanup；现有 `remove_entity` 只有测试/调用方契约。两条路径均属于 server-only 生命周期清理。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` §二负灵域、§七异兽和世界长期运行语义；不改变 ghost 密度或兽潮公式。
- **finished_plans**：查 `plan-neg-domain-fauna-v1`（文档声称有 cleanup）和 fauna migration/flow field 计划，确认当前代码未形成完整终态回收。
- **active plan**：查 fauna、world unload、npc lifecycle；无同一 FlowFields/ghost registry 生产 cleanup。
- **skeleton**：查 `plan-bughunt-network-ephemeral-cache-prune-v1`、dormant lifecycle 和 terrain cleanup；这些不拥有 fauna resources。
- **reminder.md**：查 fauna、ghost、flow field、despawn；仓内有该文件，无本 issue 登记。

## §2 接入面与跨仓契约

- **Inputs**：`FlowFieldComputeTask`/`FlowFields`、`BeastHordeEvent`、`GhostEntity`、`GhostZoneRegistry`、`ZoneRegistry`、tick/zone 状态。
- **Outputs**：过期 flow field 被回收或复用，正灵域/消失 zone 的 ghost 实体和 registry 条目一致清理；ghost contact 的 qi release 仍走原 ledger。
- **共享类型/事件**：复用 `FlowField`、`FlowFields`、`GhostEntity`、`GhostZoneRegistry`、`QiTransferReason::ReleaseToZone`；诡影接触现有释放 helper 已统一使用 `ReleaseToZone`，不新增不存在的 `GhostContact` reason，也不新增客户端实体协议。
- **三端契约符号**：server `fauna::{migration::flow_field_compute_system,ghost::{ghost_spawn_system,ghost_drift_system,ghost_contact_system}}`；agent **无变更**，fauna runtime 不走 IPC；client **无变更**，ghost 是 server-only，flow field 只影响 NPC movement。
- **worldview/qi**：ghost contact 的 `Cultivation.qi_current` 外部来源仍通过 `qi_release_to_zone`/`QiTransfer` 记账；生命周期清理不得删除未释放余额。守恒测试引用 `SPIRIT_QI_TOTAL`。

## §3 游玩影响与复现

反复触发不同 tick 的兽潮任务，观察 `FlowFields.len()` 只增不减；负灵域回正或 zone 被移除后，旧 GhostEntity 仍漂移/占 registry 名额，重新变负时新 ghost 无法按真实数量生成。

## §4 `origin/main` 根因证据

- `server/src/fauna/migration.rs:449-479` 每个含 tick 的任务插入新 field，`FlowFields` 只有 `insert/get/contains/len`，无 TTL/eviction。
- `server/src/fauna/ghost.rs:107-176` 按负灵压 spawn 并登记；`:178-208` drift 对正灵域或消失 zone 不做 despawn/registry 清理；`GhostZoneRegistry::remove_entity` 在 `:93` 只是可调用 helper。

## §5 非重复比对

`plan-neg-domain-fauna-v1` 是玩法落地计划，不等于当前 cleanup 已实现；network/client cache prune 不触及 server fauna resources。worldgen 与 BongWorldGen 不在本骨架范围。

## §6 修复计划骨架

- **P0**：为 flow field 定义基于 horde phase/last-use 的有界 retention，复用仍在迁移中的 field，不按 tick 无限造 id；ghost 在 zone 回正、zone 消失或实体终结时原子移除 ECS 与 registry。
- **P1**：长跑容量、同一 field 重用、ghost 回正再生成、缺 zone、重复 cleanup 和 contact 后 qi 守恒测试；`assert_conservation` 的 era 参数按实际为 `0.0`。

## §7 验证计划

实现后跑 server fmt/clippy/cargo test，重点 `fauna::migration`、`fauna::ghost`；不跑其他栈。
