# plan-bughunt-network-woliu-state-lifecycle-v1（骨架）

> **来源 issue**：#1755、#1409、#1406。Woliu v1/v2 状态合并时 active 覆盖、实体 cache 生命周期和场次计数都没有明确边界。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | v1/v2 active 语义统一，实体离场清理 cache，每场 reset intercepted_count | ⬜ |

## §0 摘要

`apply_woliu_v2_state_overlay` 在 cooldown 期无条件覆盖 v1 `VortexField` 的 active；`VortexStateEmitCache` 的 active/intercepted 两个 map 不 prune；`intercepted_count` 只累加不按场次归零。客户端 HUD 会被错误关闭、长期驻留或跨场次累计。

## §1 游玩影响

冷却期仍有 v1 遮蔽时 HUD 显示不 active；实体离场后内存增长；下一场技能显示上一场拦截数。视觉/审计不可信，但不改变 qi ledger 结果。

## §2 复现路径

1. 同一实体保留 v1 `VortexField`，建立已结束 active window 的 v2 state，观察 v2 overlay 直接把 active 改为 false。
2. spawn/despawn client entity，运行 `emit_vortex_state_payloads`，检查 cache 仍有旧 key。
3. 触发两场 projectile drain，观察第二场 `intercepted_count` 从第一场累计。

## §3 今天 `origin/main` 的证据

- `server/src/network/woliu_state_emit.rs:20-123` 两个 HashMap 无 prune；`:42-47` 累加 intercepted；`:59-71` 下发累计值；`:98-101` v2 active 无条件覆盖 v1。
- 同文件 `:214-237` 测试仍锁住旧 overlay 行为，实施时需改成真实生命周期契约。

## §4 非重复比对

已查 woliu v1/v2 active/finished plans、active plan、skeleton 的 `VortexStateEmitCache`、`apply_woliu_v2_state_overlay`、`intercepted_count`；没有现成骨架覆盖三条生命周期问题。#1755/#1409/#1406 共用同一 emit cache 根因，逐子项验收。

## §5 立项检查记录

- `docs/worldview.md`：查涡流/雾堤、区域视觉和招式生命周期；不新增技能或真元规则。
- `docs/finished_plans/`：查 Woliu v1/v2 及 combat feedback 计划，确认 payload 语义。
- active plan：查 `VortexStateEmitCache`、`VortexField`、`VortexV2State`，未发现同一 cache 修改。
- skeleton：查 #1755、#1409、#1406、`intercepted_count`、`active_until_tick`，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无 Woliu state lifecycle 条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：v1 `VortexField`、v2 `VortexV2State`/`TurbulenceField`、combat clock 和 `ProjectileQiDrainedEvent`。
- **Outputs**：`ServerDataPayloadV1::VortexState`，通过 server data channel 更新客户端 HUD；cache 仅保留 live entity/current cast 数据。
- **共享类型/事件**：复用 `VortexFieldStateV1`、`VortexStateEmitCache`、`VortexField`、`VortexV2State`；不新造 active enum。
- **三端契约符号**：server `emit_vortex_state_payloads`/`apply_woliu_v2_state_overlay`；agent `VortexFieldStateV1` 与 `ServerDataVortexStateV1` schema **无运行时变更**；client `VortexStateHandler`、`VortexStateStore` 和 Woliu HUD planners。
- **worldview/qi_physics 锚点**：涡流状态只呈现已结算的战斗状态；本计划不执行 QiTransfer、不修改 `SPIRIT_QI_TOTAL`。

## P0 验收

- v1 active 与 v2 cooldown/active 的优先级按真实场状态合并，不被过期 v2 无条件清空。
- despawn/disconnect 后 cache 没有旧 Entity；新的 generation 不继承旧 count。
- 每个 Woliu cast/场次重新开始 count，payload 与客户端 HUD 一致。
