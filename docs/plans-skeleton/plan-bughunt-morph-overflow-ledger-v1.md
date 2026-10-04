# plan-bughunt-morph-overflow-ledger-v1

> 骨架：来源 #1917。易形扣除真元后 overflow/无 zone 分支只发送事件，不应用到真实账本。

## §0 摘要

`server/src/body_plan/morph.rs:397-409` 直接减少 `Cultivation.qi_current`；`:418-517` 的 zone overflow、无 zone、无 registry 分支把 `QiTransfer` 放入 `Events`，没有 system 消费并写入 `WorldQiAccount`。扣掉的真元因此从守恒账本消失。

## §1 游玩影响

玩家易形释放在满区/无区时会永久损失真元，且审计看似有 transfer 事件却无法在余额中追踪，重登后差额扩大。

## §2 复现路径

1. 让玩家有足够 `qi_current`，所在位置 zone 满或缺失。
2. 触发 morph release，观察 `drain_qi_to_zone` 先扣余额。
3. 检查 `Events<QiTransfer>` 没有消费者应用 overflow account，`WorldQiAccount` 无对应增加。

## §3 今天 `origin/main` 根因证据

- `body_plan/morph.rs:397-409` 修改 `Cultivation.qi_current` 后才决定去向。
- `:440-471,475-517` 构造 overflow `QiTransfer` 并发送到 Events；全仓没有该路径的 ledger apply。
- 缺 zone/registry 时同样只 emit，违反“事件不是账本”的守恒要求。

## §4 非重复比对

`plan-bughunt-morph-disconnect-release-save-order-v1` 处理断线保存顺序；通用 `plan-bughunt-qi-ledger-asymmetry-v1` 不拥有 morph 专用 from identity。本骨架只收敛易形外部真元转账。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“易形、真元挥发、释放、守恒”；`worldview.md §二/§四` 要求扣除后必须回到 zone 或稳定中转账户。
- **finished_plans**：查 `drain_qi_to_zone`、`QiTransfer`、`Events`、`WorldQiAccount`；未见该 event 的 apply system。
- **active plan**：查 morph/qi ledger；无同一 overflow 真实入账修复。
- **skeleton**：查 `morph overflow`、`emit QiTransfer`、`ReleaseToZone`；无重复骨架。
- **reminder.md**：查 `morph`、`overflow`、`ledger`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`Cultivation.qi_current`（在线玩家真元权威）、`Position`/`CurrentDimension`、`ZoneRegistry`、morph cost/owner identity。
- **Outputs**：zone accepted 与 overflow 的真实 ledger balances、`QiTransfer` audit、morph state release；缺身份/账本必须 fail closed，不先扣余额。
- **共享类型或事件**：在线外部来源不能假设 player ledger 账户，须核实并调用 `qi_physics::ledger::transfer_external_qi_to_ledger` 或 `cultivation::components::qi_flow::release_external_qi_to_zone`；zone 释放可用 `qi_release_to_zone`/`transfer_ledger_qi_to_zone`，所有转移构造 `QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone }`，并用 `qi_physics::ledger::assert_conservation`，测试引用 `SPIRIT_QI_TOTAL`。
- **server 符号**：`body_plan::morph::drain_qi_to_zone`、`Cultivation`、`ZoneRegistry`、`WorldQiAccount`、`qi_physics::ledger`。
- **agent**：无变更；易形释放没有 agent/Redis 余额契约。
- **client**：无变更；客户端只消费 morph/inventory snapshot，不持有 qi 权威。
- **worldview 锚点**：`docs/worldview.md §二` 全服真元守恒与 §四`易形`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 将外部 Cultivation 扣除与 ledger transfer 原子化，zone/overflow 均有真实落点 |
| P1 | ⬜ | 满区、无区、缺 ledger、重复释放和 `assert_conservation` 回归测试 |

## 来源 issue

- #1917 `[flash-review][major] 易形 cast 的 overflow/无zone 分支只发审计事件不入账`
