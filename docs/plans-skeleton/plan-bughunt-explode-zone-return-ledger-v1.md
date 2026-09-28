# plan-bughunt-explode-zone-return-ledger-v1

> 来源 Issue：#1396。ExplodeZone 归还时把整个 zone 清零，只退固定借款，期间发生的 zone 灵气被静默销毁。

## §0 摘要

`borrow_explode_zone_qi` 从 `WorldQiBudget` 扣下 `EXPLODE_ZONE_QI_COST + zone.spirit_qi.max(0.0)` 并把 zone 写到 1.0；到期的 `apply_due_qi_return` 却无条件把该 zone 写成 0.0，再把固定 `entry.amount` 加回 budget。借款期间玩家吸收、heartbeat 回流或其他合法流动造成的 zone 变化都被覆盖，且 budget 的增减没有对应的 `QiTransfer`。这条借还链需要统一为可审计的 ledger 事务，不能靠两个裸字段赋值维持守恒。

## §1 实际游玩体验影响

- ExplodeZone 持续期间发生的区域灵气变化会在六个月后被抹掉，所有玩家都承担不可见的总量损失。
- zone 原有的正/负灵气状态也被固定的 0.0 覆盖，负灵域债务可能被伪造为无债。
- `WorldQiBudget.current_total` 与可观察 zone/玩家池脱节，守恒审计无法定位损失。

## §2 复现路径

1. 让 `spawn` zone 有非零 `spirit_qi`，调用 `borrow_explode_zone_qi`，记录 `ScheduledQiReturn.amount` 和借款前 zone 值。
2. 在 `due_tick` 前执行玩家吸收、heartbeat 回流或其他改变该 zone 的系统。
3. 运行 `apply_due_qi_returns`，观察 `apply_due_qi_return` 把 zone 直接写成 0.0，同时只把固定 amount 加回 budget。
4. 对拍 zone、ledger、budget 和 `SPIRIT_QI_TOTAL`；期间新增或减少的量没有对应转账。

## §3 根因证据

- `server/src/cultivation/void/ledger_hooks.rs:89-114` 把借款和 zone 峰值写入 `WorldQiBudget`/`Zone`，没有 `QiTransfer` 计划或可恢复的 zone 状态。
- `server/src/cultivation/void/ledger_hooks.rs:145-161` 的到期分支直接 `zone.spirit_qi = 0.0`，随后 `budget.current_total += entry.amount`。
- `WorldQiBudget` 的 `current_total`/`era_decay_accum` 是预算字段，不等价于 zone ledger 余额；不能以 `budget.current_total += ...` 代替 `WorldQiAccount::transfer(QiTransfer { from, to, amount, reason })`。
- `qi_physics::ledger::transfer_ledger_qi_to_zone(&mut WorldQiAccount, from, zone_name, &mut zone.spirit_qi, requested, zone_ceiling, reason)` 已提供 ledger→signed zone 字段的失败原子路径。

## §4 非重复比对

- `docs/plans-skeleton/plan-bughunt-void-barrier-dispel-qi-ledger-v1.md` 处理道伥被障折半的外部源；本 issue 是区域借款生命周期，源和归还时机不同。
- `docs/finished_plans/plan-bughunt-zone-qi-economy-v1.md` 的普通 zone heartbeat 结算不覆盖 `VoidQiReturnSchedule` 的借款快照。
- 这里的核心不是“六个月”配置值，而是借款期间外部流动被固定归零覆盖；归还不能只补一个 budget 数字。

### 立项检查记录（2026-09-28）

- `docs/worldview.md`：检索“化虚、坍缩渊、负灵域、灵气回流”，核对 §二 L18-L22 与 §十六 L1566-L1569。
- `docs/finished_plans/`：检索 `borrow_explode_zone_qi`、`apply_due_qi_return`、`WorldQiBudget`；命中化虚动作和 zone economy 背景文档，但没有借还期间流量保护。
- active plan：检索 `VoidQiReturnSchedule`、`ExplodeZone`、`VoidBarrierReturn`；`plan-container-filter-and-completion-v1.md` 只列 gateway manifest，不覆盖当前归还算法。
- `docs/plans-skeleton/`：检索 `borrow_explode_zone_qi`、`zone.spirit_qi = 0.0`、`ExplodeZone`；除本文件外未发现同主题 skeleton。
- `docs/plans-skeleton/reminder.md`：已查阅并检索 `borrow_explode_zone_qi`、`apply_due_qi_return`、`ExplodeZone`，未发现同主题延后事项。

## 接入面与跨仓契约

- **Inputs**：`WorldQiBudget`、`Zone`/`ZoneRegistry`、`WorldQiAccount`、`VoidQiReturnSchedule`、`CultivationClock`、期间的 zone 流动事件。
- **Outputs**：借款与归还各有可追溯 `QiTransfer`，zone 当前 signed 值和稳定 ledger/budget 余额一致；归还失败时整笔计划重试且无部分写入。
- **共享类型 / 事件**：`ScheduledQiReturn`、`QiTransfer`、`QiTransferReason::VoidAction`、`QiTransferReason::ReleaseToZone`、`WorldQiAccount`、`transfer_ledger_qi_to_zone`；不新增 agent/client payload。
- **server 契约符号**：`borrow_explode_zone_qi`、`apply_due_qi_return`、`apply_due_qi_returns_collect_failures`、`WorldQiAccount::transfer`、`qi_physics::ledger::transfer_ledger_qi_to_zone`、`qi_physics::ledger::assert_conservation`。
- **agent**：无变更。ExplodeZone 借还只存在于 server 的化虚动作和 zone 账本。
- **client**：无变更。客户端继续读取现有 zone/cultivation 快照，不增加借还协议字段。
- **worldview 锚点**：`docs/worldview.md §二 L18-L22`（真元总量与流动）；`§十六 L1566-L1569`（坍缩渊环境及其负压边界）。
- **qi_physics**：zone 字段是 `Zone.spirit_qi` 的 signed 外部权威，稳定余额才在 `WorldQiAccount`。从在线 `Cultivation.qi_current` 出发必须用 `transfer_external_qi_to_ledger` 的真实签名，不能假设 player 账户可扣；ledger → zone 字段用 `transfer_ledger_qi_to_zone`。每笔转账显式构造 `QiTransfer { from, to, amount, reason }` 并调用 `WorldQiAccount::transfer`（或对应原子 helper），释放使用 `ReleaseToZone`；断言引用 `SPIRIT_QI_TOTAL` 与 `assert_conservation`。

## §5 修复骨架

### P0 借还状态建模

- 在 schedule 中保存可核验的借款来源、目标账户、借款前 zone 状态/期间合法流量，归还只结算实际借入的那部分；绝不覆盖期间产生的 zone 变化。
- 借款和归还分别走真实 `QiTransfer`；ledger、zone、budget 更新必须在同一失败原子边界内，缺资源时保留计划等待重试。
- 对负 zone 保留 signed 值，禁止 `.max(0.0)` 和到期 `= 0.0` 这种吞债写法。

### P1 回归与守恒

- 覆盖借款期间无流动、玩家吸收、heartbeat 回流、负 zone、缺 zone 和归还失败。
- 用 `assert_conservation(before, after, era_decay)` 验证 `SPIRIT_QI_TOTAL`；只有明确记录的 era decay 才能改变总量。
