# plan-bughunt-heaven-gate-aoe-double-amplification-v1

> 骨架：来源 #1886。天门 AoE 已计算 damage，却把它当 qi_invest 交给 resolver，再次乘 attack power。

## §0 摘要

`server/src/sword_path/skill_register.rs:645-673` 将 `compute_heaven_gate_damage` 的结果填入 `AttackIntent.qi_invest`；combat resolver 随后按普通攻击再次乘 attack power。天门 AoE 因此发生二次放大，伤害超过技能设计值。

## §1 游玩影响

化虚天门 AoE 对玩家/NPC 造成远超预期的伤害，可能绕过平衡、护盾与境界差异，一次施法清场。

## §2 复现路径

1. 触发 `compute_heaven_gate_damage` 生成一份 AoE damage。
2. 观察 `AttackIntent.qi_invest` 被填为该 damage。
3. 进入 combat resolver 后再乘 attack power，对比单次公式应得值。

## §3 今天 `origin/main` 根因证据

- `skill_register.rs:645-673` 先调用 `compute_heaven_gate_damage`，再构造普通 `AttackIntent`。
- `qi_invest` 在 resolver 中语义是待放大的投资值，不是已完成 damage。
- 天门 intent 没有 `DamageAlreadyComputed`/专用 outcome 标记阻止第二次放大。

## §4 非重复比对

`plan-bughunt-heaven-gate-aftermath-qi-ledger-v1` 处理真元释放；本骨架只处理 damage/qi_invest 语义边界，不修改 aftermath 账本。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“化虚、天门、AoE、境界伤害”；`worldview.md §三/§八` 要求招式威力可解释且不重复计权。
- **finished_plans**：查 `compute_heaven_gate_damage`、`AttackIntent.qi_invest`、resolver；未见已算伤害标记。
- **active plan**：查 sword path/combat resolver；无同一二次放大修复。
- **skeleton**：查 `heaven gate`、`double amplification`、`qi_invest`；无重复骨架。
- **reminder.md**：查 `heaven gate`、`attack power`、`damage`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：天门 cast snapshot、AoE targets、`compute_heaven_gate_damage`、attack power。
- **Outputs**：一次最终 damage、combat event/VFX；不改变 qi ledger 扣费路径。
- **共享类型或事件**：复用 `AttackIntent`/combat resolver；需明确 `qi_invest` 与 precomputed damage 的类型/标志，不新增平行伤害事件。
- **server 符号**：`sword_path::skill_register::compute_heaven_gate_damage`、`AttackIntent`、`combat::resolve`。
- **agent**：无变更；技能伤害不经 agent。
- **client**：无变更；现有 AoE VFX/伤害反馈 payload 足够。
- **worldview 锚点**：`docs/worldview.md §三` 化虚与 §八`招式伤害`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 明确预计算 damage 与 qi_invest 的类型边界，确保 resolver 只放大一次 |
| P1 | ⬜ | 单目标、多目标、attack power、护盾和跨境界回归测试 |

## 来源 issue

- #1886 `[flash-review][major] 化虚天门 AoE 伤害经 qi_invest 被 attack_power 二次放大`
