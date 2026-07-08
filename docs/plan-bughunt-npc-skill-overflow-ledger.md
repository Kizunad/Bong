# BugHunt: NPC 技能 overflow 真元未落账

## Bug 摘要

`server/src/npc/npc_skill.rs::release_npc_qi_to_zone` 在 `npc.heal_basic` / `npc.buff_speed` / `npc.buff_defense` 成功施放后，把 NPC 已扣除的真元释放回 home zone。accepted 部分会直接写入 `ZoneRegistry.zone.spirit_qi`，但 zone 满仓、home zone 缺失、或 caster 没有 `NpcPatrol` 时，overflow 只被构造成 `QiTransfer(to=overflow:npc_skill_*)` 并发送到 `Events<QiTransfer>`，没有写入 `WorldQiAccount` 或任何真实余额。

仓内守恒约束明确：`QiTransfer` 是审计事件，真实余额必须由 `WorldQiAccount` 或调用点直接 apply。当前 overflow 分支中 NPC 的 `Cultivation.qi_current` 已扣，zone 没接收，overflow 账户也没有余额增长，导致真元从 `summarize_world_qi()` 口径消失。

## 对实际游玩体验的影响

玩家在高灵气区域遇到引气/凝脉以上散修时，NPC 会在战斗中自疗、疾行或护体。高灵区接近满仓时，技能消耗的 5-8 点真元无法完全回灌到 zone，本应进入 overflow/待处理账户的部分会被无声蒸发。

体感上，玩家看到 NPC 正常施法、回血或加 buff，但区域灵气、守恒 telemetry 和后续回流系统没有承接对应真元。NPC 群体在高灵区反复交战时，技能越频繁，世界真元预算漂移越明显；这会让高灵区战斗比设计更像真元黑洞，而不是“NPC 付出真元，环境或 overflow 账户承接代价”。

## 证据定位

- `server/src/npc/npc_skill.rs:246-250`：`npc_heal_basic` 先扣 `HEAL_QI_COST`，再调用 `release_npc_qi_to_zone`。
- `server/src/npc/npc_skill.rs:291-295`：`npc_buff_speed` 先扣 `BUFF_SPEED_QI_COST`，再调用释放 helper。
- `server/src/npc/npc_skill.rs:347-351`：`npc_buff_defense` 先扣 `BUFF_DEFENSE_QI_COST`，再调用释放 helper。
- `server/src/npc/npc_skill.rs:59-71`：无 `NpcPatrol` 时只发送 `QiTransfer(to=overflow:npc_skill_no_zone:*)`。
- `server/src/npc/npc_skill.rs:92-109`：accepted 分支写 `zone.spirit_qi`，但 `outcome.overflow` 只 push 事件。
- `server/src/npc/npc_skill.rs:130-148`：home zone 找不到时同样只发送 overflow 事件。
- `server/src/test_coverage_guards.rs:100-103`：`QiTransfer` 被列为审计事件，真实余额由 `WorldQiAccount` / 调用点直接 apply。
- `server/src/qi_physics/ledger.rs:455-456`：`WorldQiAccount::total()` 只统计 balances。
- `server/src/qi_physics/ledger.rs:662-665`：`summarize_world_qi()` 的 `ledger_qi` 只读 `WorldQiAccount::total()`，不读 `Events<QiTransfer>`。

## 触发路径

1. `server/src/npc/technique.rs:270-290` 给引气以上 NPC 注入 `npc.heal_basic`，给凝脉以上 NPC 注入 `npc.buff_speed` 或 `npc.buff_defense`。
2. `server/src/cultivation/skill_registry.rs:98-114` 初始化 `SkillRegistry`，注册 `crate::npc::npc_skill::register_npc_skills`。
3. `server/src/npc/technique.rs:843-850` 的 `NpcTechniqueAction` / `NpcHealAction` 驱动 NPC 技能释放。
4. `server/src/npc/technique.rs:928-948` 通过 `SkillRegistry.lookup()` 找到 skill fn 并实际调用。
5. NPC 技能成功后扣 `qi_current`，进入 `release_npc_qi_to_zone`。
6. 当 home zone 容量不足、home zone 不存在、或 caster 缺 `NpcPatrol` 时，扣除量的全部或部分只进入 `Events<QiTransfer>`，不进入 `ZoneRegistry` 或 `WorldQiAccount`。

高灵区不是必须 `spirit_qi == 1.0` 才触发。以 `QI_ZONE_UNIT_CAPACITY = 50` 计，回血成本 8 时只要 `spirit_qi > 0.84` 就会产生 overflow；护体成本 6 时阈值是 `> 0.88`；疾行成本 5 时阈值是 `> 0.90`。

## 反方审查记录

### Round 1

反方结论：REAL。

反方检查没有找到 `EventReader<QiTransfer>` 或其它运行时消费者会把 `npc_skill_overflow` / `npc_skill_no_zone` 事件写入 `WorldQiAccount`。`summarize_world_qi()` 只统计 `WorldQiAccount` balances，不统计事件队列。NPC 技能并非测试专用，已由 skill registry、NPC 技法注入和 action system 接入生产路径。

去重结论：不重复 #975 dormant 负灵域死亡释放、#989 attrition overflow、#1000 heartbeat 伪灵脉、#1013 骨币面值、#1020 垂死大能死亡 overflow、#1026 医道截断、#1037 TSY 入场过滤。

### Round 2

反方结论：REAL，但严重度应表述为高灵区/异常 zone 状态下的明确守恒缺口，而不是全局常态吞真元。

Round 2 指出修复不能简单调用 `WorldQiAccount::transfer`：活体 NPC 真元存储在 ECS `Cultivation.qi_current`，不是长期 ledger balance，直接 transfer 会因 source 余额不足失败。正确修复边界应避免 accepted 部分在 `zone.spirit_qi` 和 zone ledger 中双计；overflow/no-zone 部分才需要真实 ledger 目的账户余额或“临时点燃 source balance -> transfer -> source 归零”的模式。

Round 2 也否定了“把 overflow 退回 NPC `qi_current`”的修法，因为现有语义是技能成功后固定扣完整成本，再由环境或 overflow 承接已花掉的真元；退回 NPC 会让满仓高灵区施法变相打折。

### 独立反方

第二个独立反方同样结论 REAL。它补充指出：`ZoneRegistry` 资源缺失时，外层 `if let Some(mut zones)` 没有 `else`，甚至不会发送 overflow 事件；这也是同一 helper 的更强丢失分支，修复时应一并覆盖。

## Skeleton Fix Plan

- [ ] 为 `release_npc_qi_to_zone` 增加 `WorldQiAccount` 写入能力，或抽一个通用 helper，专门处理“活体 ECS qi 已扣，accepted 写 zone field，overflow 写真实 ledger account”的模式。
- [ ] accepted 部分继续以 `zone.spirit_qi` 作为真实落点，不额外写同名 zone ledger，避免 `summarize_world_qi()` 双计。
- [ ] overflow/no-zone/invalid-zone/缺 `ZoneRegistry` 分支把未接收量写入 `QiAccountId::overflow("npc_skill_*")` 的真实 balance，并保留 `QiTransfer` 审计轨迹。
- [ ] source 侧不要长期镜像活体 NPC qi。若使用 `WorldQiAccount::transfer`，只做临时 source balance 点燃并在转账后归零；或显式 `set_balance` 增加 overflow 目标并 `push_transfer_audit`，但必须有真实余额增长。
- [ ] 将 NPC skill overflow 记账 helper 与 #989/#1020 同类 overflow 修复策略对齐，避免每个模块手写一份不一致的 overflow 语义。

## 验收测试计划

- [ ] `npc_skill` 单测：spawn zone `spirit_qi = 1.0` 时，`npc_heal_basic` 扣 8，`zone.spirit_qi` 不变，`WorldQiAccount` 的 `overflow:npc_skill_overflow:*` 增加 8，`summarize_world_qi()` 总量不漂移。
- [ ] `npc_skill` 单测：spawn zone `spirit_qi = 0.90` 时，`npc_buff_defense` accepted 5、overflow 1，zone field 和 ledger overflow 总和等于扣减量。
- [ ] `npc_skill` 单测：缺 `NpcPatrol`、home zone 找不到、缺 `ZoneRegistry` 三个 fallback 分支都把完整成本写入真实 overflow balance。
- [ ] 回归测试：负灵域 `spirit_qi < 0` 仍按裸 `spirit_qi * QI_ZONE_UNIT_CAPACITY` 接收，不恢复 `.max(0.0)` 抹负缺口问题。
- [ ] 集成测试：`NpcHealAction` / `NpcTechniqueAction` 走 `SkillRegistry.lookup()` 的真实 action 路径时，成功施法后的 `player_qi + zone_qi + ledger_qi` 守恒。

## 风险

- 活体 NPC qi 与 ledger balance 的双账本边界容易误用，不能把 NPC 长期 mirror 到 `WorldQiAccount` 后又被 ECS 统计一次。
- accepted 部分若同时写 `zone.spirit_qi` 和 zone ledger，会在 telemetry 中双计。
- overflow 账户命名需要稳定，避免每 tick 用不稳定 id 产生难以归并的 ledger 噪音。
- 缺 `ZoneRegistry` 分支修复后可能暴露测试环境未插资源的问题；应把生产系统资源和单测最小 world 区分清楚。
