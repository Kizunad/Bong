# plan-bughunt-dugu-v2-taint-decay-ledger-v1

## §0 摘要

**来源 Issue：#1806。** 临时 `TaintMark` 到期时，`taint_decay_tick` 只恢复目标的 `temporary_qi_max_loss` 并移除组件；mark 中代表尚未散逸的 `intensity` 没有被 Reverse 消费，也没有在到期时释放到目标 zone/overflow。毒蛊入体真元因此随状态删除而消失。该骨架只记录当前缺口，不改生产代码。

接入面：进料是 `TaintMark`、目标 `Cultivation`、`Position`/`CurrentDimension` 与 `ZoneRegistry`；出料是 mark intensity 的 `ReleaseToZone`/overflow 审计和恢复后的 `qi_max`。复用 `eclipse_zone_credit_tick`/`reverse_zone_credit_tick` 使用的 `qi_release_to_zone`、`QiTransfer`、`WorldQiAccount`，不新造事件 consumer。server 内部状态不改变 agent/client schema。worldview 锚点为 §二、§十的脏真元逸散与总量守恒。

## §1 游玩影响

- 目标撑过临时毒蛊持续时间而未触发 Reverse 时，标记消失的同时真元残留被静默删除。
- 玩家只看到 `qi_max` 恢复，不会看到本应返回环境的真元；多次命中后世界总量持续减少。
- 这是正常技能时序（持续时间到期）即可触发，不依赖死亡或异常断线。

## §2 复现路径

1. 施放 Eclipse 使目标获得 `TaintTier::Temporary` 的 `TaintMark`，记录其 `intensity` 和 `expires_at_tick`。
2. 不在到期前施放 Reverse，将 `CombatClock.tick` 推进到过期。
3. `taint_decay_tick` 恢复 `temporary_qi_max_loss`，移除 `TaintMark`。
4. 检查目标所在 zone、overflow account 与 `QiTransfer`：当前没有 intensity 对应的释放。

## §3 根因证据

- `server/src/combat/dugu_v2/tick.rs:41-59` 的 `taint_decay_tick` 查询 `(Entity, &mut Cultivation, &TaintMark)`，过期分支只执行 `cultivation.qi_max += temporary_qi_max_loss` 和 `commands.entity(entity).remove::<TaintMark>()`；没有 `Position`、维度、zone、ledger 或 transfer writer。
- `server/src/combat/dugu_v2/state.rs:39-52` 明确 `TaintMark.intensity` 是与 `returned_zone_qi` 分开的字段；`reverse_zone_credit_tick` 只消费 `ReverseTriggeredEvent.returned_zone_qi`，不会自动读取被移除的 mark。
- `server/src/combat/dugu_v2/skills.rs:255-288` 在 Temporary 分支把 intensity 写入 mark，并把 `temporary_qi_max_loss` 另存；因此到期删除组件会丢掉仍未被 Reverse 处理的 residue。
- `server/src/combat/dugu_v2/tick.rs:265-300` 的 Reverse 释放路径已证明 zone/overflow 需要显式调用 `qi_release_to_zone`；事件 reader 不会替代过期分支的真实账本操作。

## §4 非重复比对

- `plan-bughunt-dugu-v2-eclipse-qi-ledger-v1` 处理 Eclipse 创建 mark 时的实际扣减与 intensity 对齐；本骨架是临时 mark 生命周期终点，两者不应合并成一个实现步骤。
- `plan-bughunt-combat-qi-max-shrink-ledger-v1` 处理永久 decay/qi_max clamp 的 current excess；本骨架只处理临时 mark intensity 的遗失。
- `docs/finished_plans/plan-qi-conservation-leaks-v1.md` 的 zone credit tick 只覆盖已经产生的 Eclipse/Reverse 事件，不覆盖 component 到期删除。

## §5 修复计划骨架

### P0：临时标记终止结算

- 在 `taint_decay_tick` 选择明确的终止语义：若 intensity 尚未被 Reverse 消费，则调用 `qi_release_to_zone` 按目标位置/维度释放到 zone，zone 不可达时进入 overflow；真实账户回流用 `ledger.transfer(QiTransfer { from, to, amount, reason })`，成功记账后再移除 mark。
- 保持 `temporary_qi_max_loss` 恢复与 residue 释放为同一原子状态转换；重复 tick、已被 Reverse 移除的 mark 和零 intensity 必须幂等。

### P1：回归契约

- 到期未 Reverse：目标 `qi_max` 恢复、zone/overflow 增加恰当 intensity、恰一条释放审计。
- 到期前 Reverse：只允许 Reverse 路径结算一次，到期 tick 不重复释放。
- 无 zone/位置时不吞真元；永久 mark 继续走永久 decay 路径。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，重点覆盖 `dugu_v2::tick` 的过期、Reverse 竞态和 overflow 分支。守恒断言引用 `QI_ZONE_UNIT_CAPACITY`/`assert_conservation`；本 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Inputs：** `taint_decay_tick` 输入 `CombatClock.tick`、`Cultivation`、`TaintMark.intensity/temporary_qi_max_loss`、目标 `Position/CurrentDimension` 和 `ZoneRegistry`。
- **Outputs：** 过期 mark 在移除前把未消费 intensity 结算到 zone 或 overflow，恢复 `qi_max` 且只产生一次审计。
- **共享类型/事件：** `TaintMark`、`CombatClock`、`ZoneRegistry`、`WorldQiAccount`、`QiTransfer`；server 符号为 `taint_decay_tick`、`eclipse_zone_credit_tick`、`reverse_zone_credit_tick`、`route_dugu_qi_to_overflow`。
- **三端契约符号：** Server 负责 mark 生命周期与 zone 结算；Agent：无变更，理由是所有输入/输出都是 server ECS；Client：无变更，理由是既有战斗事件和 Fabric payload 不增字段。
- **Qi：** zone 回灌先调用 `qi_release_to_zone(amount, from, zone, zone_current, QI_ZONE_UNIT_CAPACITY)`；`DuguReturnToZone` 是 audit-only，只 `push_transfer_audit`，不可 `ledger.transfer`。真实 overflow/container 账户才调用 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })`；断言用 `qi_physics::ledger::assert_conservation`、`QI_EPSILON` 与 `crate::schema::common::SPIRIT_QI_TOTAL`。
- **worldview 锚点：** `docs/worldview.md` §二、§十的脏真元逸散和总量守恒；删除组件前必须完成结算。
