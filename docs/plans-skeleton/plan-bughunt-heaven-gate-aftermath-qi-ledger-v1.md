# plan-bughunt-heaven-gate-aftermath-qi-ledger-v1

> 骨架：来源 #1389、#1834。化虚天门 aftermath 清零灵剑/玩家真元后只写字段或审计事件，staging 溢出未形成真实账本迁移。

## §0 摘要

`server/src/sword_path/skill_register.rs:691-721` 把 `Bond.stored_qi` 清零并将玩家 `qi_current` 直写 0，再调用 `credit_qi_current_to_zone`；`:733-749` 的 staging buffer 结算仅构造转移事件。近满 zone 的 credit 逻辑在 `:1000-1032` 只写 zone 字段/审计，缺少可核验的 ledger overflow 落点。

## §1 游玩影响

化虚天门结束时灵剑储能与施法者真元会凭空减少；区域接近容量上限时溢出也不可追踪，属于全服真元守恒破坏。

## §2 复现路径

1. 蓄力并触发 `HEAVEN_GATE_AOE_END` aftermath，记录 `Cultivation.qi_current`、`Bond.stored_qi` 与 zone 余额。
2. 观察字段被清零，事件队列之外没有对应的 `WorldQiAccount` 转账。
3. 将 zone 调到容量上限附近，观察 accepted/overflow 之和小于扣除量且只剩 audit event。

## §3 今天 `origin/main` 根因证据

- `skill_register.rs:693-709` 清零 `stored_qi` 与 `qi_current`。
- `:715-721` 调用 `credit_qi_current_to_zone`，其 `:1000-1032` 直接修改 zone/emit audit，未证明 ledger 余额已入账。
- `:733-749` staging aftermath 只发送 `QiTransfer` 事件，未在本路径应用到账本账户。

## §4 非重复比对

`plan-bughunt-heaven-gate-aoe-double-amplification-v1` 处理伤害重复放大，不拥有 aftermath 账本；既有 qi-ledger asymmetry 骨架处理其他技能 overflow。本骨架只统一天门储能、玩家余额与 zone/overflow 的原子释放。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“化虚、天门、灵剑、真元总量”；`worldview.md §二/§三` 的全服守恒与化虚代价命中本题。
- **finished_plans**：查 `HeavenGate`、`stored_qi`、`credit_qi_current_to_zone`、`SPIRIT_QI_TOTAL`；未见 aftermath 的完整账本迁移。
- **active plan**：查 `QiTransfer`、`qi_release_to_zone`、`WorldQiAccount`；没有占用该函数的天门修复。
- **skeleton**：查 `heaven gate aftermath`、`staging buffer`、`overflow`；无同根因骨架。
- **reminder.md**：查 `heaven gate`、`stored_qi`、`overflow`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`HeavenGateChanneling`、`Cultivation.qi_current`、`Bond.stored_qi`、`ZoneRegistry`、`HEAVEN_GATE_AOE_END`。
- **Outputs**：`QiTransfer`、zone 字段与 ledger 账户、overflow 账户、`HeavenGateCastEvent`/审计。
- **共享类型或事件**：所有流动必须构造 `QiTransfer { from, to, amount, reason }`；优先调用 `qi_physics::ledger::transfer_external_qi_to_ledger`、`transfer_ledger_qi_to_zone` 或合适的 `qi_release_to_zone`，原因使用 `QiTransferReason::ReleaseToZone`，并用 `qi_physics::ledger::assert_conservation` 验证。测试常量引用 `schema::common::SPIRIT_QI_TOTAL`；`qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL` 是另一套默认预算，不能混写。
- **server 符号**：`sword_path::skill_register::{heaven_gate_cast_system,credit_qi_current_to_zone}`、`QiAccountId`、`WorldQiAccount`、`QiTransfer`。
- **agent**：无变更；天门 aftermath 是 server 内部事件，Redis agent 不持有余额权威。
- **client**：无变更；既有 `HeavenGateCastEvent`/VFX 继续消费，不修改 payload。
- **worldview 锚点**：`docs/worldview.md §二` 真元易挥发与 §三`化虚`；全服总量以实际 `SPIRIT_QI_TOTAL` 断言。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 统一玩家/灵剑/staging 到 zone 或 overflow 的真实 ledger 转移 |
| P1 | ⬜ | 满区、无区、重复 aftermath 与 `assert_conservation` 回归测试 |

## 来源 issue

- #1389 `[flash-review][major] 化虚结算销毁灵剑 stored_qi`
- #1834 `[flash-review][major] 化虚 aftermath 回灌 zone 用 clamp 销毁溢出`
