# plan-bughunt-daozhan-ambush-distance-dimension-gate-v1

> 骨架：来源 #1824。道伥 pending drain 只在选目标时查最近玩家，执行时不复核距离或维度。

## §0 摘要

`server/src/fauna/daozhan.rs:724-734` 玩家 query 没有维度字段，`:793-802` 选最近目标；`:849-850` 记录 pending drain，`:875-934` 执行时直接扣除，没有距离/`CurrentDimension` 再验证。

## §1 游玩影响

玩家跨维度、传送或跑出范围后仍会被道伥吸取真元；这是越权伤害与守恒错误叠加的远程攻击。

## §2 复现路径

1. 道伥锁定玩家后，在 pending 与 execute 间传送到另一维或超出距离。
2. 观察 execute 分支仍按旧 entity 扣 qi/记录 drain。
3. 无距离/维度拒绝事件，玩家资源减少。

## §3 今天 `origin/main` 根因证据

- `daozhan.rs:724-734` 目标查询只有 `Position`/player marker，没有 `CurrentDimension` 过滤。
- `:793-802` 只按距离选最近一次。
- `:875-934` 执行 pending drain 前没有重新读取目标 dimension/距离。

## §4 非重复比对

`plan-bughunt-spiritual-sense-cross-dimension-v1` 处理感知快照泄漏；`plan-bughunt-possession-target-qi-release-v1` 处理夺舍死亡释放。本骨架只锁定道伥攻击的选定→执行时序。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“道伥、距离、维度、吸灵”；`worldview.md §七/§十二` 要求异界隔离和近距捕食。
- **finished_plans**：查 `pending drain`、`Daozhan`、`CurrentDimension`、距离 gate；未见执行时复核。
- **active plan**：查 fauna daozhan/tsy lifecycle；无同一攻击门禁修复。
- **skeleton**：查 `daozhan ambush`、`dimension gate`、`pending drain`；无重复骨架。
- **reminder.md**：查 `道伥`、`drain`、`dimension`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：道伥位置/维度、目标 `Position`/`CurrentDimension`、pending drain、tick。
- **Outputs**：成功时 qi release/drain 与 combat/narration event；失败时稳定拒绝且清理 pending。
- **共享类型或事件**：复用 `CurrentDimension`、`Position`、道伥 drain event、qi ledger API；不复制玩家位置快照。
- **server 符号**：`fauna::daozhan::{select_target,execute_pending_drain}`、`QiTransfer`、`qi_physics::ledger`。
- **agent**：无变更；道伥攻击不通过 agent。
- **client**：无变更；既有伤害/VFX payload 保持不变。
- **worldview 锚点**：`docs/worldview.md §七` 异兽捕食与 §十二`TSY 维度隔离`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 执行前复核 dimension、距离、目标身份并 fail closed |
| P1 | ⬜ | 传送、跨维、目标 despawn、重复 pending 与守恒回归测试 |

## 来源 issue

- #1824 `[flash-review][major] 道伥暴起吸取无距离/维度复核`
