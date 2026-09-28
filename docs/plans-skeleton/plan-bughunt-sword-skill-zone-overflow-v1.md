# plan-bughunt-sword-skill-zone-overflow-v1

> 骨架：来源 #1852。`credit_skill_qi_to_zone` 近满 zone 只 clamp accepted，overflow 仅构造审计事件。

## §0 摘要

`server/src/sword_path/skill_register.rs:1064-1104` 将技能释放真元写入 zone，并在容量不足时只生成 `QiTransfer`/审计信息；没有把 `QiFlowOutcome.overflow` 写入稳定 overflow 账户。近满区域因此吞掉多余真元。

## §1 游玩影响

剑技释放的真元总量随 zone 容量变化，满区时世界总量下降，长服后不同区域会产生不可解释的守恒缺口。

## §2 复现路径

1. 将目标 zone 调到接近 `QI_ZONE_UNIT_CAPACITY`。
2. 触发 `credit_skill_qi_to_zone` 注入超过剩余容量的技能。
3. 对比调用前后玩家/技能来源、zone、overflow ledger，发现差额仅有审计事件。

## §3 今天 `origin/main` 根因证据

- `skill_register.rs:1064-1104` 直接 clamp `zone_after`。
- `outcome.overflow` 没有调用 `transfer_external_qi_to_ledger` 或等价稳定入账 helper。
- 审计事件不会改变 `WorldQiAccount` 余额，不能作为守恒落点。

## §4 非重复比对

`plan-bughunt-heaven-gate-aftermath-qi-ledger-v1` 处理天门 aftermath 的多个来源；本骨架专注普通技能 credit helper 的 zone overflow。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“剑技、灵气区域、容量、守恒”；`worldview.md §二` 要求真元不因环境容量凭空消失。
- **finished_plans**：查 `credit_skill_qi_to_zone`、`QiFlowOutcome`、overflow account；未见稳定 overflow 落点。
- **active plan**：查 sword path/qi ledger；无同一 helper 修复。
- **skeleton**：查 `skill zone overflow`、`credit_skill_qi_to_zone`；无重复骨架。
- **reminder.md**：查 `sword`、`overflow`、`zone capacity`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：技能来源真元、zone current/capacity、caster/skill identity。
- **Outputs**：accepted zone credit、稳定 overflow ledger、审计事件与技能结果。
- **共享类型或事件**：必须使用 `QiTransfer { from, to, amount, reason }`；核对 `qi_physics::ledger::{transfer_external_qi_to_ledger,transfer_ledger_qi_to_zone,assert_conservation}` 与 `qi_release_to_zone` 签名，overflow 用 `QiTransferReason::ReleaseToZone`，测试常量引用 `SPIRIT_QI_TOTAL`。
- **server 符号**：`sword_path::skill_register::credit_skill_qi_to_zone`、`QiFlowOutcome`、`WorldQiAccount`。
- **agent**：无变更；剑技真元账本在 server。
- **client**：无变更；技能结果 payload 不承担余额权威。
- **worldview 锚点**：`docs/worldview.md §二` 真元守恒与 §八`剑技`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | accepted 与 overflow 分路真实入账，禁止只 emit 审计 |
| P1 | ⬜ | 近满/满区、无区、重复 event 与全服守恒测试 |

## 来源 issue

- #1852 `[flash-review][major] credit_skill_qi_to_zone 近满 zone 静默丢弃溢出真元`
