# plan-bughunt-sword-intent-reach-origin-v1

> 骨架：来源 #1851。剑意实体命中时以 owner 位置而非 intent 当前坐标判定 reach。

## §0 摘要

`server/src/sword_path/sword_intent_entity.rs:125-152` 让 intent 移动到目标并在当前坐标命中，但构造 `AttackIntent` 时 `attacker=intent.owner`，reach 校验使用 owner 位置。远程剑意命中被当作超距攻击拒绝并记录反作弊。

## §1 游玩影响

合法飞行中的剑意只能在施法者附近命中，远距离技能被误判作弊，玩家看到命中 VFX 却没有伤害。

## §2 复现路径

1. 从位置 A 发射 sword intent，目标在远处 B。
2. 观察 intent 到达 B 并 emit `AttackIntent`。
3. combat reach 仍从 owner A 计算，命中被拒绝。

## §3 今天 `origin/main` 根因证据

- `sword_intent_entity.rs:125-141` 移动和 `dist_to_target` 判定基于 intent `Position`。
- `:143-151` 的 AttackIntent 没有携带 intent 当前 origin，只有 owner attacker 与固定 reach。
- combat resolver 无法区分实体投射物与 owner 近战距离。

## §4 非重复比对

`plan-bughunt-sword-skill-zone-overflow-v1` 处理真元释放，普通近战 reach 规则由 combat plan 负责；本骨架只补投射物命中 origin 契约。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“剑意、飞剑、距离、命中”；`worldview.md §四/§八` 允许远程剑意实体传递攻击。
- **finished_plans**：查 `SwordIntentEntity`、`AttackIntent`、`AttackReach`；未见投射物 origin 字段约定。
- **active plan**：查 sword intent/combat reach；无同一命中修复。
- **skeleton**：查 `sword intent reach`、`origin`、`projectile`；无重复骨架。
- **reminder.md**：查 `sword intent`、`reach`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`SwordIntentEntity.Position`、owner/target、hit radius、`AttackIntent`。
- **Outputs**：合法命中伤害、反作弊审计、VFX；远程实体不被 owner 距离误拒。
- **共享类型或事件**：复用 `AttackIntent`/`AttackReach`/`SwordIntentEntity`；扩展 origin 时同步所有 resolver consumer。
- **server 符号**：`sword_path::sword_intent_entity::sword_intent_tracking_system`、`combat::resolve`。
- **agent**：无变更；剑意攻击不走 agent IPC。
- **client**：无变更；既有 sword intent VFX 使用 entity position。
- **worldview 锚点**：`docs/worldview.md §四` 剑意形态与 §八`战斗技能`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 让 AttackIntent reach/origin 表达投射物当前位置，保留 owner 身份归属 |
| P1 | ⬜ | 远程命中、owner 近战、移动目标和反作弊回归测试 |

## 来源 issue

- #1851 `[flash-review][major] 剑意化形以施法者位置判 reach`
