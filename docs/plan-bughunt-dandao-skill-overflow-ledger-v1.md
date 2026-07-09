# plan-bughunt-dandao-skill-overflow-ledger-v1

> BugHunt active plan / server-qi / 2026-07-08 r01。本文只记录缺陷与修复任务，本 PR 不消费 plan、不改代码、不归档。

## Bug 摘要

`server/src/dandao/skills.rs::drain_dandao_qi` 在丹道三基础招成功后先扣玩家 `Cultivation.qi_current`，正常命中可接收 zone 时会通过 `qi_release_to_zone` 写回 `zone.spirit_qi`。但 full-zone overflow、no-zone、无 `Position` 或无 `ZoneRegistry` 的 fallback 分支，只把 `QiTransfer(to=overflow:dandao_skill_*)` 放进 `Events<QiTransfer>`，没有写入 `WorldQiAccount` 或任何真实余额。

仓内约束明确：`QiTransfer` 是守恒审计事件，真实余额必须由 `WorldQiAccount` 或调用点直接 apply。当前 fallback 中玩家真元已扣，zone 没接收，overflow 账户也没有余额增长，导致这部分真元从 `summarize_world_qi()` 口径消失。

## 实际游玩体验影响

当前正常玩家技能栏入口受 #1045「丹道三基础招技能栏断链」阻断，因此本 bug 不是“今天所有玩家都稳定触发”的高频问题。它是丹道 resolver/registry 路径里的 latent runtime 守恒漏洞：三招 resolver 已注册进生产 `SkillRegistry`，一旦 #1045 把技能栏入口修通，丹道玩家在高灵区或 zone 外施放基础招时会立刻撞到这条漏账路径。

体感上，玩家会看到丹道招式正常进入冷却、真元条减少，但高灵区已满时环境灵气不会继续承接，overflow ledger 也没有增加；在 zone AABB 外施放时更是完整成本只留下事件痕迹。长期看，丹道战斗会把高灵区和边界区域变成无声真元黑洞，破坏“招式释放只扣攻方不写入环境 = 守恒红旗”的世界规则。

## 触发路径

1. `server/src/cultivation/skill_registry.rs:98-114` 初始化全局 `SkillRegistry`，其中 `server/src/cultivation/skill_registry.rs:112` 注册 `crate::dandao::register_skills`。
2. `server/src/dandao/mod.rs:60-63` 将 `dandao.pill_rush` / `dandao.pill_bomb` / `dandao.pill_mist` 三个 resolver 注册进 skill registry。
3. 这些 resolver 成功 gate 后调用 `drain_dandao_qi`：`server/src/dandao/skills.rs:242`、`:286`、`:329`。
4. `drain_dandao_qi` 在 `server/src/dandao/skills.rs:73-86` 先扣玩家 `qi_current`。
5. 正常 zone 主路径在 `server/src/dandao/skills.rs:101-118` 写 `zone.spirit_qi`，这部分不是本 bug。
6. 当 zone 已满或近满时，`server/src/dandao/skills.rs:120-133` 只构造 overflow `QiTransfer` 并加入 `pending_transfers`。
7. 当玩家位置找不到 zone 时，`server/src/dandao/skills.rs:169-180` 同样只构造 `dandao_skill_no_zone` overflow 事件。
8. 当缺 `Position` 或缺 `ZoneRegistry` 时，`server/src/dandao/skills.rs:182-193` 也只构造 overflow 事件。
9. 最后 `server/src/dandao/skills.rs:196-200` 仅 `events.send(t)`；没有 `WorldQiAccount` 写入。

## 根因证据

- `server/src/test_coverage_guards.rs:100-103`：`QiTransfer` 被列为 `DirectResourceConsumer`，说明“QiTransfer 是守恒审计事件；真实余额由 WorldQiAccount/调用点直接 apply，不能要求 EventReader 消费”。
- `server/src/qi_physics/ledger.rs:455-456`：`WorldQiAccount::total()` 只统计 balances。
- `server/src/qi_physics/ledger.rs:641-665`：`summarize_world_qi()` 统计 `ZoneRegistry.spirit_qi`、`Cultivation.qi_current`、inventory、`WorldQiAccount::total()`，不读 `Events<QiTransfer>`。
- 全仓搜索没有生产 `EventReader<QiTransfer>` 落账；`qi_physics::register` 只提供事件资源，不消费事件余额。
- `server/src/dandao/skills.rs:840-873` 的 no-position 回归测试只断言 overflow `QiTransfer` 事件存在，没有插入 `WorldQiAccount`，也没有断言 `summarize_world_qi` 守恒，因此当前测试锁住的是弱行为。
- 同型正确口径可参考当前 `server/src/npc/npc_skill.rs:74-125`：活体 NPC qi 已扣后，overflow 分支会临时点燃 source balance、调用 `WorldQiAccount::transfer`，再发审计事件。
- `server/src/npc/npc_skill.rs:1048-1085` 已有满仓 zone 测试，断言 overflow 落入 `WorldQiAccount`、临时 source 归零、`assert_conservation` 通过；丹道缺少等价 pin。

## 非重复说明

已按硬约束先运行：

```bash
gh pr list --state all --limit 600 --json number,title,headRefName,url
```

不重复近期 server-qi：

- #1043「NPC 技能 overflow 真元未落账」覆盖 `npc_skill`，当前 `npc_skill` 已有 `credit_spent_qi_to_ledger` 和满仓 overflow 守恒测试；本题是玩家丹道三基础招 `dandao::skills` 的独立调用点。
- #1045「丹道三基础招技能栏断链」覆盖客户端/技能栏入口不可达，不覆盖 resolver 被调用后的 qi overflow 真实余额问题。本 plan 必须保留 #1045 reachability caveat。
- #1050 / #1056 / #1076 / #1082 / #1089 / #1096 / #1102 / #1107 / #1122 均不是丹道三基础招 overflow/no-zone ledger。
- #678 修的是丹道旧版硬编码 `current_zone` 导致正常 zone 主路径蒸发。当前代码已能写真实 zone，但 overflow/no-zone 分支仍只发事件不写 ledger，属于 #678 未覆盖的边界遗留。

## 修复计划

- [ ] 为 `drain_dandao_qi` 增加 `WorldQiAccount` 写入能力，专门处理“玩家 ECS qi 已扣，accepted 写 zone field，overflow 写真实 ledger account”的模式。
- [ ] accepted 部分继续以 `zone.spirit_qi` 作为真实落点，不额外写同名 zone ledger，避免 `summarize_world_qi()` 双计。
- [ ] full-zone / near-cap overflow、no-zone、无 `Position`、无 `ZoneRegistry` 分支，把未被 zone 接收的量写入 `QiAccountId::overflow("dandao_skill_*")` 的真实 `WorldQiAccount` balance，并保留 `QiTransfer` 审计轨迹。
- [ ] 若使用 `WorldQiAccount::transfer`，只临时点燃 live player source balance，转账后 source 必须归零；不要长期 mirror 玩家 `Cultivation.qi_current`。
- [ ] 与 `npc_skill` 的 `credit_spent_qi_to_ledger` 语义对齐，必要时抽共享 helper，避免每个技能模块自写一套 overflow 规则。

## 验收测试计划

- [ ] `dandao::skills` 单测：`spawn` zone `spirit_qi = 1.0` 时施放 `pill_mist` 扣 10，zone 不变，`WorldQiAccount` 的 `overflow:dandao_skill_overflow:*` 增加 10，`summarize_world_qi` 守恒。
- [ ] `dandao::skills` 单测：zone `spirit_qi = 0.85` 时施放 `pill_mist`，accepted 为 7.5、overflow 为 2.5，zone field + overflow ledger 总和等于扣减量。
- [ ] `dandao::skills` 单测：无 zone / 无 `Position` / 无 `ZoneRegistry` 三个 fallback 分支都把完整成本写入真实 overflow balance，而不是只发 `Events<QiTransfer>`。
- [ ] `dandao::skills` 回归：正常可接收 zone 主路径仍只更新 `zone.spirit_qi` + 审计事件，不因新增 ledger 写入造成 double count。
- [ ] 集成 pin：在 `WorldQiAccount` 存在的真实 skill registry 调用路径中，成功施放后的 `player_qi + zone_qi + ledger_qi` 不漂移。
- [ ] 回归 #1045 修通后：技能栏实际 cast 丹道三招时仍走同一守恒路径。

## 对抗复核结论

### Round 1

反方结论：`REAL`，但要求收窄。正常命中 zone 且容量足够的 accepted 主路径已写 `zone.spirit_qi`，不是本 bug；`find_zone` 后 `find_zone_mut` 失败属于防御分支，不作为主触发证据。保留 full-zone overflow、no-zone、无 `Position` / 无 `ZoneRegistry` fallback。

### Round 2

反方结论：`REAL`，但必须带 #1045 reachability caveat。#1045 使正常玩家技能栏入口当前受阻，但三招 resolver 已在生产 `SkillRegistry` 注册，不能判死代码；#678 修的是硬编码假 zone，不覆盖当前 overflow/no-zone 真实余额缺失；`Events<QiTransfer>` 不能当作 overflow 余额存储；full-zone/near-cap 并非纯测试构造，`spirit_qi=1.0` 是合法上限，丹雾固定 10 qi 在 `spirit_qi > 0.8` 时就会 overflow。

最终裁决：成立。定级为丹道三基础招 resolver 的 latent major 守恒漏洞；修复 #1045 后会成为实际战斗路径的玩家可感知漏账。

## 风险

- 修复时最容易把 accepted zone 部分同时写 `zone.spirit_qi` 和 zone ledger，造成双计；accepted 与 overflow 必须分开处理。
- 玩家真元已经在 ECS `Cultivation.qi_current` 中统计，不应长期 mirror 到 `WorldQiAccount`。
- fallback 分支缺 `WorldQiAccount` 时应拒绝扣费或显式回滚，而不是继续扣后只发事件；具体行为需实施时统一到现有 qi 守恒 helper 口径。
