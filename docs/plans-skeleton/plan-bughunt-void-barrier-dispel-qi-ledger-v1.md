# plan-bughunt-void-barrier-dispel-qi-ledger-v1

> 来源 Issue：#1390。化虚障命中道伥时直接改写 `Cultivation.qi_current`，折半差额没有进入 qi ledger。

## §0 摘要

`apply_barrier_dispel_system` 对几何范围内的 `TsyHostileMarker` 直接调用 `barrier_dispel_qi`，把其 `Cultivation.qi_current` 折半后写回。道伥的在线真元权威在 `Cultivation.qi_current`，不是 `WorldQiAccount` 的 player 账户；当前系统没有拿 zone、overflow 或 ledger 资源，也没有产生 `QiTransfer`。被折掉的真元会随活体组件的数值变化直接消失，违反 `SPIRIT_QI_TOTAL` 守恒。

## §1 实际游玩体验影响

- 化虚玩家立障后，道伥进入几何范围即可损失一半当前真元，玩家看不到对应的区域回流。
- 同一道伥可反复经过不同化虚障，损耗成为可重复的世界总量漏洞。
- 当前测试只验证折半数学函数，不验证真元落点或 ledger 审计。

## §2 复现路径

1. 准备带 `TsyHostileMarker`、`Cultivation.qi_current > 0` 的道伥和一枚 `BarrierField`。
2. 让道伥进入障的几何范围并运行 `apply_barrier_dispel_system`。
3. 对比调用前后的 `Cultivation.qi_current`、目标 zone 的 `spirit_qi`、overflow 账户和 `WorldQiAccount` 转账记录。
4. 当前只有 current 减半，没有 `ReleaseToZone` 转账或 overflow 入账，差额没有物理落点。

## §3 根因证据

- `server/src/cultivation/void/actions.rs:528-546` 的系统直接写 `cultivation.qi_current = barrier_dispel_qi(...).min(...).max(...)`，没有 ledger、zone 或事件参数。
- `server/src/cultivation/void/actions.rs:146-148` 的 `barrier_dispel_qi` 只返回折半数值，无法表达源、目标和失败原子性。
- `server/src/cultivation/components/qi_flow.rs:603-735` 已有外部真元释放事务；对带 `Cultivation` 的活体应优先使用 `Cultivation::release_to_zone`，而不是先把字段改小再补一条审计。
- `qi_physics::ledger::transfer_external_qi_to_ledger(&mut WorldQiAccount, QiAccountId, QiAccountId, f64, QiTransferReason)` 是外部源入账的真实签名；不能假定 player ledger 里已有可扣余额。

## §4 非重复比对

- `docs/finished_plans/plan-bughunt-qimax-shrink-clamp-leak-v1.md` 覆盖的是延寿丹和断续散缩容；没有覆盖 `apply_barrier_dispel_system` 的道伥折半路径。
- `docs/plans-skeleton/plan-bughunt-possession-target-qi-release-v1.md` 处理夺舍目标软删除前的全额释放；本 issue 是障命中后的部分释放，入口和触发条件不同。
- 这不是 client 显示或 agent narration 问题；服务端当前值已经发生不可审计的减少。

### 立项检查记录（2026-09-28）

- `docs/worldview.md`：检索“化虚障、道伥、真元、负压”，核对 §二 L18-L22 与 §十六 L1566-L1569。
- `docs/finished_plans/`：检索 `apply_barrier_dispel_system`、`barrier_dispel_qi`、`VoidAction`；命中化虚动作总 plan，但没有道伥折半的 qi release。
- active plan：检索 `BarrierField`、`TsyHostileMarker`、`VoidBarrierReturn`；`plan-container-filter-and-completion-v1.md` 的 barrier return 网关清单不覆盖本折半入口。
- `docs/plans-skeleton/`：检索 `barrier_dispel`、`TsyHostileMarker`、`道伥`; 除本文件外未发现同主题 skeleton。
- `docs/plans-skeleton/reminder.md`：已查阅并检索 `barrier_dispel`、`TsyHostileMarker`、`VoidBarrierReturn`，未发现同主题延后事项。

## 接入面与跨仓契约

- **Inputs**：`BarrierField`、道伥 `TsyHostileMarker`、`Cultivation.qi_current/qi_max`、`Position`、`CurrentDimension`、`ZoneRegistry`、`WorldQiAccount`、`Events<QiTransfer>`。
- **Outputs**：折半差额进入同维 zone 的 `spirit_qi`，满 zone 或缺 zone 时进入真实 overflow；`Cultivation.qi_current`、审计事件和失败原子性一致。
- **共享类型 / 事件**：复用 `QiTransfer`、`QiTransferReason::ReleaseToZone`、`ActorQiIdentity`、`Cultivation::release_to_zone` 或 `release_external_qi_to_zone`；不新增 agent/client payload。
- **server 契约符号**：`apply_barrier_dispel_system`、`barrier_dispel_qi`、`Cultivation::release_to_zone`、`qi_physics::ledger::transfer_external_qi_to_ledger`、`qi_physics::ledger::assert_conservation`。
- **agent**：无变更。该效果完全在 server ECS 内结算，agent 不拥有道伥真元。
- **client**：无变更。既有 cultivation snapshot 会呈现结算后的 current，不增加字段。
- **worldview 锚点**：`docs/worldview.md §二 L18-L22`（真元总量与流动规则）；`§十六 L1566-L1569`（坍缩渊/化虚环境对感知与真元的影响）。
- **qi_physics**：在线/活体权威是 `Cultivation.qi_current`。释放前构造真实 `QiTransfer`，通过 `Cultivation::release_to_zone(..., QiTransferReason::ReleaseToZone)`；底层对 overflow 使用 `transfer_external_qi_to_ledger`，不能只调用 `WorldQiAccount::transfer` 从未同步的 player 账户扣款。守恒测试引用 `SPIRIT_QI_TOTAL` 和 `assert_conservation`。

## §5 修复骨架

### P0 折半事务化

- 先计算待释放的折半差额，使用道伥的稳定 `ActorQiIdentity`；完成 zone/overflow 预检后才提交 `Cultivation.qi_current` 的新值。
- 正常 zone 走 `Cultivation::release_to_zone`，理由使用 `ReleaseToZone`；缺少身份、账本或目标时 fail closed，不能回退到 raw clamp。
- 同一障只处理一次的历史标记继续保留，但标记必须在释放成功后提交。

### P1 回归与守恒

- 覆盖正 zone、负 zone、满 zone、缺 zone 和 ledger 失败；确认 current 减少量等于 zone accepted 加 overflow。
- 用 `assert_conservation(before, after, era_decay)` 对拍 `SPIRIT_QI_TOTAL`，不写字面量总量。
