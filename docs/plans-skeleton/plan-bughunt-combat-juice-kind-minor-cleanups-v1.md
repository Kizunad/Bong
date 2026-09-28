# BugHunt：combat_event 的 qi_damage 视听分类遗漏

> 来源 Issue：#1556。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | qi_damage 映射到真元碰撞反馈 | ⬜ |

## §0 摘要

server 鼠咬抽真元事件使用 kind=qi_damage；client CombatEventHandler.parseKind/defaultColorFor 已识别该值，但 juiceKind 没有对应分支，落到 HIT，于是触发错误的相机震动而没有 QI_COLLISION 的屏幕/实体反馈。

## §1 游玩影响

玩家被噬元鼠抽真元时看到普通命中反馈，真元碰撞的视觉语义缺失，战斗反馈与实际资源损耗不一致；不改变 server ledger 结算。

## §2 复现路径

触发 rat_bite，观察 combat_event.kind=qi_damage 经 client toJuiceEvent 后进入 CombatJuiceEvent.Kind.HIT。

## §3 今天 origin/main 的根因证据

- client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:159-180 的 juiceKind switch 只有 qi_collision 等分支，qi_damage 落 default；同文件 parseKind:88-98 与 defaultColorFor:105-114 已有 qi_damage，形成分类分叉。
- server/src/combat/rat_bite.rs 与 server/src/network/combat_event_emit.rs 的生产事件把鼠咬抽真元编码为 qi_damage；plan-combat-feedback-v1 的 combat_event schema 保持该 kind。

## §4 非重复比对与立项检查记录

- worldview：查噬元鼠、真元碰撞和战斗反馈；本修复只改 client 分类。
- finished_plans：查 plan-combat-feedback-v1、plan-combat-gamefeel-v1 与盾格挡 feedback；确认 qi_damage wire 已存在，不重写 rat bite ledger。
- active plan：查 CombatEventHandler/juiceKind；未找到该遗漏的 active 修法。
- skeleton：已有 plan-bughunt-combat-event-juice-runtime-bridge-gap-v1 覆盖 server 富字段 bridge；它不覆盖 qi_damage 到 juice enum 的本地 switch，因此本 issue 独立记录。
- reminder.md：仓内无 docs/plans-skeleton/reminder.md，无相关条目。

## §5 修复骨架

补齐 qi_damage/兼容别名到 CombatJuiceEvent.Kind.QI_COLLISION 的明确映射；保持 parseKind 的 DamageFloaterStore.Kind.QI_DAMAGE 与颜色不变。回归同时断言不再走 HIT 相机震动分支。

## §6 接入面与跨仓契约

- Inputs：server combat_event 的 kind/amount，由 CombatEventHandler 解析。
- Outputs：DamageFloaterStore.QI_DAMAGE 与 CombatJuiceSystem 的 QI_COLLISION 视听反馈。
- 共享类型或事件：复用 CombatEventFloaterEntryV1、combat_event、CombatJuiceEvent.Kind；不新增 wire kind。
- server 符号：rat_bite 事件与 emit_combat_event_to_client；无变更。
- agent：无变更；combat_event 不由 agent 生成。
- client：CombatEventHandler.parseKind/defaultColorFor/juiceKind 与 CombatJuiceSystem 回归。
- worldview / qi_physics：噬元鼠的真元扣除仍由 server 既有 ledger 负责；本骨架不改任何真元数值。
