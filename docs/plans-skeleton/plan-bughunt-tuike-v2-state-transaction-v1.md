# plan-bughunt-tuike-v2-state-transaction-v1

## §0 摘要

**来源 Issue：#1922、#1846。** 退壳 v2 有两条状态事务缺口：`cast_transfer_taint` 先把污染写入 `StackedFalseSkins`、写回 `WornFalseSkin`，之后才扣 qi；扣费因浮点边界失败时，拒绝结果仍留下污染状态。另一路在伪皮从胸甲脱下后，inventory 同步只移除 `WornFalseSkin`，非空 `StackedFalseSkins` 被保留，维护 tick 继续扣真元，最终还可能销毁已回收背包的物品。本骨架不改生产代码。

接入面：进料是 `PlayerInventory`、`StackedFalseSkins`/`WornFalseSkin`、`Cultivation` 与 `false_skin_maintenance_tick`；出料是原子成功/拒绝结果、正确的层栈生命周期和既有 zone `QiTransfer`。复用 `transfer_taint_to_outer_skin`、`spend_qi`、`sync_false_skin_stack_from_inventory`、`release_qi_amount_to_zone`，agent/client 不需新 wire。worldview §四/§十要求装备与真元状态一致，拒绝事务不能留下副作用。

## §1 游玩影响

- 玩家在真元刚好处于浮点边界时尝试转移污染，界面显示施法拒绝，但伪皮已吸收污染；重复尝试可绕过正常门控占满容量。
- 玩家脱下伪皮后，离身的非空层栈仍被维护系统视为装备状态，每秒扣真元；真元耗尽时维护 shed 可能按旧 instance 销毁已放回背包的物品。
- 两条路径都能在正常换装/施法中持续发生，属于状态机错位而非仅测试夹具问题。

## §2 复现路径

### 被拒绝的 transfer_taint

1. 穿戴有外层伪皮，准备污染与 qi，使 `transfer_taint_to_outer_skin` 计算出的 `qi_cost` 接近当前余额。
2. 让 `spend_qi` 的 `qi_current + EPSILON < amount` 在浮点误差下返回 false。
3. 当前实现已把 `stack`/`outer` 写回，然后返回 `QiInsufficient`；污染未 drain，皮肤却多了 `contam_load`。

### 脱下后 phantom maintenance

1. `sync_false_skin_stack_from_inventory` 看到胸甲没有伪皮，但 `StackedFalseSkins` 仍有层。
2. `(None, Some(stack), _)` 分支移除 `WornFalseSkin`，仅在 stack 为空且冷却满足时才删除 stack。
3. `false_skin_maintenance_tick` 仍查询非空 stack、按 tier 扣维护真元；不足时 `shed_outer_layer_for_maintenance` 调 `consume_item_instance_once`，即使该 instance 已不在装备槽。

## §3 根因证据

- `server/src/combat/tuike_v2/skills.rs:204-244` 在 `cast_transfer_taint` 的临时 `outcome` 块中先调用 `transfer_taint_to_outer_skin` 并插回 `StackedFalseSkins`/`WornFalseSkin`，随后才执行 `spend_qi`；失败分支直接返回，没有恢复旧 stack。
- `server/src/combat/tuike_v2/skills.rs:430-445` 的 `spend_qi` 用 EPSILON 比较并可能拒绝，证明先写状态再扣费不是原子事务。
- `server/src/combat/tuike_v2/tick.rs:45-95` 的 inventory 同步在 `(None, Some(stack), _)` 只移除外层组件；非空 stack 继续存在。
- `server/src/combat/tuike_v2/tick.rs:107-169` 的 `false_skin_maintenance_tick` 只要求 `StackedFalseSkins` 存在就计费；`shed_outer_layer_for_maintenance`（约 `171-220`）直接对 `layer.instance_id` 调 `consume_item_instance_once`，没有再验证该 instance 仍在 chest 装备槽。

## §4 非重复比对

- `docs/finished_plans/plan-tuike-v2-v1.md` 提供退壳状态底盘与维护机制，但未覆盖 transfer_taint 的扣费原子性及脱装后的 orphan stack。
- `plan-bughunt-carrier-imprint-lifecycle-v1` 处理暗器载具 instance，不能替代伪皮 stack 与 chest slot 的同步。
- 真元释放本身由现有 `release_qi_amount_to_zone` 负责；本骨架重点是避免在错误状态下调用维护或在拒绝时提交污染。

## §5 修复计划骨架

### P0：两个状态转换原子化

- `transfer_taint` 先做 qi 可行性预检/事务试算，再一次性提交 stack、污染、永久 decay marker 与 qi 扣减；真实账户结算调用 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })`，任何拒绝都恢复原状态且不发成功事件。
- inventory 同步发现 chest 已无伪皮时，明确 detach 语义：清理/冻结非空层栈，或把它转为可见的独立残留状态；维护系统不能继续按“已穿戴”扣费。shed 消耗物品前必须核验 instance 仍归该装备路径，避免销毁背包物。

### P1：回归契约

- transfer 成功、qi 不足/浮点边界、永久污秽吸收三类断言 stack、污染、qi、冷却和事件一致。
- 穿戴中脱下、换回同 instance、脱下后维护 tick、qi 不足触发 shed 四类断言：不再维护 phantom stack，不误删背包物品，真实装备仍按原规则维护。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，重点覆盖 `tuike_v2::skills` 与 `tuike_v2::tick`。涉及 qi 的断言必须核对 zone/overflow 与 `QiTransfer`；本 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Inputs：** `cast_transfer_taint`/`transfer_taint_to_outer_skin` 输入 `StackedFalseSkins`、`WornFalseSkin`、`Contamination`、`Cultivation.qi_current`；同步/维护由 `sync_false_skin_stack_from_inventory`、`false_skin_maintenance_tick` 驱动。
- **Outputs：** 成功事务同时提交 stack/污染/qi；拒绝不留副作用，脱装后维护不再扣 phantom stack 或销毁错误 instance。
- **共享类型/事件：** `PlayerInventory`、`StackedFalseSkins`、`WornFalseSkin`、`QiTransfer`、`QiTransferReason::ReleaseToZone`；server 符号为 `spend_qi`、`release_qi_amount_to_zone`、`shed_outer_layer_for_maintenance`。
- **三端契约符号：** Server 负责状态事务和扣费；Agent：无变更，理由是 stack/inventory/maintenance 都在 server ECS；Client：无变更，理由是既有 cast/maintenance 事件和动画 wire 不增字段。
- **Qi：** 玩家 ECS 扣费走 `release_qi_amount_to_zone` → `qi_release_to_zone`，不要从 ledger player 账户扣；真实 ledger 账户之间才用 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })`。断言调用 `qi_physics::ledger::assert_conservation`、`QI_ZONE_UNIT_CAPACITY`、`QI_EPSILON` 与 `crate::schema::common::SPIRIT_QI_TOTAL`。
- **worldview 锚点：** `docs/worldview.md` §四、§十的装备状态一致性和真元零和；拒绝事务不能留下污染。
