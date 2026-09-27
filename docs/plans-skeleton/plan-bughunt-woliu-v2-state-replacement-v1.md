# plan-bughunt-woliu-v2-state-replacement-v1

## §0 摘要

**来源 Issue：#1414、#1404。** Woliu v2 每次施法都在 caster 上无条件 `insert` 两个有生命周期的组件：新的 `TurbulenceField` 覆盖旧场，丢掉旧场尚未衰减的 `remaining_swirl_qi`；新的 `VortexV2State` 覆盖 Heart 状态并重置 `started_at_tick`/`active_skill_kind`，使化虚心诀 30 秒劫难检测被后续普通招式绕过。两者都是“单槽组件替换没有先结算旧状态”的同一根因，本骨架不改生产代码。

接入面：进料是 `resolve_woliu_v2_skill` 的技能规格、zone source、`TurbulenceField` 与 `VortexV2State`；出料必须保留旧场真元、正确的 Heart backfire 状态和生命周期事件。复用 `turbulence_decay_tick` 的 `release_decayed_turbulence_qi`、`heart_active_backfire_tick`、`PassiveVortex` 与 `QiTransfer`，不新增 agent/client schema。worldview §二/§十的灵气守恒和劫难状态持续性是本问题的外部契约。

## §1 游玩影响

- 连续施放带湍流半径的技能会覆盖前一个场，旧场中高额 swirl qi 不再衰减/回流，世界总量下降。
- 化虚玩家开始 Heart 后，在劫难窗口内施放任意其他 Woliu 技能可重置状态；`heart_active_backfire_tick` 因 active kind 不再是 Heart 而跳过断经与 JueBi 惩罚。
- 两条路径都是普通连续施法即可达，且当前事件/视觉仍显示新技能成功，玩家无法察觉旧状态被替换。

## §2 复现路径

### 湍流覆盖

1. 施放带 `spec.turbulence_radius > 0` 且有 `stir.rotational_swirl` 的技能，得到 caster 上的 `TurbulenceField`。
2. 在旧场 `remaining_swirl_qi > 0` 时再次施放同类技能。
3. `world.entity_mut(caster).insert(TurbulenceField::new(...))` 直接替换旧组件；没有 OnRemove/显式 release。

### Heart 劫难覆盖

1. 化虚施放心诀 Heart，记录 `VortexV2State.active_skill_kind=Heart`、`started_at_tick`。
2. 在 30 秒内施放 Burst/Hold 等其它技能。
3. 通用 resolve 再 `insert(VortexV2State{active_skill_kind: skill, started_at_tick: now_tick})`；tick 中 Heart 条件不再满足，惩罚永远不触发。

## §3 根因证据

- `server/src/combat/woliu_v2/skills.rs:418-449` 无条件插入 `VortexV2State` 与 `TurbulenceField`；未读取既有组件，也未在替换前调用释放/状态合并。
- `server/src/combat/woliu_v2/state.rs:24-36` 的 `TurbulenceField` 持有 `remaining_swirl_qi`；`server/src/combat/woliu_v2/tick.rs:28-69` 只在字段自然衰减时释放，组件被替换时没有消费旧余额。
- `server/src/combat/woliu_v2/tick.rs:273-304` 的 `heart_active_backfire_tick` 明确要求 `state.active_skill_kind == WoliuSkillId::Heart`，并以 `clock.tick - state.started_at_tick` 判定 30 秒；覆盖 state 直接改变两个 gate。
- `server/src/combat/woliu_v2/tick.rs:315-334` 的 state lifecycle 只在仍标记 Heart 时移除 `PassiveVortex`，所以 state 被替换后还可能留下被动组件与错误生命周期。

## §4 非重复比对

- `plan-bughunt-carrier-projectile-miss-qi-ledger-v1` 处理投射物 despawn，不涉及 ECS 单组件替换。
- `plan-bughunt-combat-qi-max-shrink-ledger-v1` 处理 qi_max clamp，旧湍流的 `remaining_swirl_qi` 是容器余额，不是 current 上限。
- `plan-bughunt-woliu-v2-placeholder-erosion-gate-v1` 处理虚蚀占位技的门禁/侵蚀计费；本骨架处理所有带 field/state 的 Woliu 施法顺序。

## §5 修复计划骨架

### P0：替换前结算与 Heart 锁

- 新建场前读取旧 `TurbulenceField`，将其剩余 swirl qi 经 `qi_release_to_zone` 按原 source zone/overflow 释放；真实账户提交 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })` 并产生唯一 audit，或明确拒绝覆盖直到旧场自然结束，不能静默丢余额。
- Heart 劫难期间保留不可被普通技能覆盖的 Heart anchor（或把劫难 deadline 独立于 active skill）；普通技能只能更新自身场，不得重置 `started_at_tick`/backfire gate。
- 明确 `PassiveVortex` 与 state 的成对生命周期，避免替换/过期后 orphan component。

### P1：回归契约

- 连续两次湍流施法断言旧场余额全部释放/合并，zone+overflow 守恒，且只存在一个有效 field。
- Heart→普通技能→30 秒 tick 仍触发一次断经/JueBi；普通技能 cooldown、Heart 自然过期和 Heart→Heart 重施分别锁定边界。
- 替换失败/资源缺失时状态与真元不发生半提交。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，覆盖 `woliu_v2::skills`、`woliu_v2::tick` 和 qi ledger。守恒断言引用 `QI_ZONE_UNIT_CAPACITY`/`assert_conservation`；本 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Server：** 施法入口是 `woliu_v2::skills::resolve_woliu_v2_skill`，状态写入点是 `VortexV2State`/`TurbulenceField` 的 `World::entity_mut(...).insert`；生命周期消费者为 `woliu_v2::tick::turbulence_decay_tick`、`release_decayed_turbulence_qi`、`heart_active_backfire_tick` 与 `vortex_v2_state_lifecycle_tick`。回归测试必须锁定替换前后的 `remaining_swirl_qi`、`started_at_tick`、`active_skill_kind` 和 `PassiveVortex`。
- **Qi：** 旧场结算使用 `qi_release_to_zone`、`QiTransferReason::ReleaseToZone`、`QiAccountId`、`WorldQiAccount`、`QI_ZONE_UNIT_CAPACITY` 与 `QI_EPSILON`；真实账户变更明确提交 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })`，不要只发 decay event。守恒测试使用 `assert_conservation` 和 `DEFAULT_SPIRIT_QI_TOTAL`（fixture 的 `SPIRIT_QI_TOTAL` 仍来自 `schema::common`）。
- **Agent：无变更。** 证据是 Woliu 状态组件、tick 与 ledger 都在 server ECS，未改变 agent IPC 或 schema。
- **Client：无变更。** 证据是现有 Woliu cast/VFX/音频 wire id 不变；修复只保留旧场余额和 Heart 生命周期。
