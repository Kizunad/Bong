# BugHunt A7: TSY 入场过滤灵质归零未落账

> 状态：Skeleton / report-only。仅记录 BugHunt 发现，不消费、不归档、不改实现。

## Bug 摘要

玩家踩 Entry 裂缝进入坍缩渊时，`tsy_entry_portal_tick` 调用 `apply_entry_filter`，把背包、装备、hotbar 中 `spirit_quality >= 0.3` 的物品直接清成 `0.0`。但 `spirit_quality * stack_count` 已被 `qi_physics::ledger::summarize_world_qi` 计入 `container_qi`，当前路径没有对应 `QiTransfer`、`WorldQiAccount`、`qi_release_to_zone`、`Rift` 或 `Overflow` 入账，导致入口剥离的物品真元从守恒口径里凭空消失。

这不是 #989「灵物磨损 overflow 未落账」的重复：#989 发生在 `AttritionTax` 已计算并尝试 release 后的 overflow 落账分支；本 bug 发生在 TSY entry filter 更早的位置，完全绕过 `attrition` / `ledger`。

## 对实际游玩体验的影响

玩家把满灵骨币、鲜采灵草、附灵武器或高灵质丹材带进坍缩渊时，物品变成「枯」「失灵」「无灵」是设计预期；但被剥离的真元不进入活坍缩渊账本，也不会在塌缩时回收分发。实际体感会变成：坍缩渊不是“暂时吸走并回收灵气”的高危区域，而是能无限销毁玩家携带灵物价值的资源黑洞。

重复进出入口或携带堆叠高灵质物品时，`container_qi` 会持续下降却没有 `zone_qi` / `ledger_qi` 对冲，长期会让守恒 telemetry 漂移，并削弱坍缩渊塌缩回收、区域灵气经济和物品保值策略的可预期性。

## 证据定位

- `server/src/world/tsy_portal.rs:59`：`tsy_entry_portal_tick` 的玩家 query 只有 `PlayerInventory` / `Position` / `CurrentDimension`，没有 `WorldQiAccount`、`ZoneRegistry` 或 `QiTransfer` writer。
- `server/src/world/tsy_portal.rs:95`：Entry 触发后直接 `let filtered = apply_entry_filter(&mut inv);`。
- `server/src/world/tsy_filter.rs:24`：`apply_entry_filter` 扫描 containers / equipped / hotbar。
- `server/src/world/tsy_filter.rs:49`：`try_strip` 只判断 `item.spirit_quality < ENTRY_FILTER_THRESHOLD`，没有账本上下文。
- `server/src/world/tsy_filter.rs:62`：`apply_spirit_strip` 直接 `item.spirit_quality = 0.0`。
- `server/src/qi_physics/ledger.rs:657`：`summarize_world_qi` 汇总所有 `PlayerInventory` 为 `container_qi`。
- `server/src/qi_physics/ledger.rs:678`：`inventory_qi` 覆盖 containers / equipped / hotbar。
- `server/src/qi_physics/ledger.rs:701`：`item_qi = item.spirit_quality.clamp(0.0, 1.0) * stack_count`。
- `server/src/qi_physics/ledger.rs:268`：`AttritionTax` 注释明确 `item.spirit_quality` 减少量必须守恒归还 zone。
- `docs/finished_plans/plan-qi-physics-v1.md:53`：活坍缩渊吸进的真元，包括“入口剥离”，应暂存在坍缩渊内部账本，不离开 `WorldQiBudget` 守恒域。
- `docs/finished_plans/plan-qi-physics-v1.md:303`：坍缩渊吸入的真元不消失，塌缩瞬间通过 `collapse_redistribute_qi` 分发回周围 zone。

## 触发路径

1. 玩家在主世界 Entry 裂缝旁，背包或装备中有 `spirit_quality >= 0.3` 的物品，例如 `spirit_quality=0.8, stack_count=3` 的骨币/灵物。
2. `tsy_entry_portal_tick` 命中 Entry portal，调用 `apply_entry_filter(&mut inv)`。
3. `try_strip` 记录 `FilteredItem.before_spirit_quality`，随后 `apply_spirit_strip` 把该物品 `spirit_quality` 置 0。
4. 同 tick 没有任何 `QiTransfer(from=container:item, to=rift/zone/overflow, amount=0.8*3, reason=...)`，也没有 ledger balance 增量。
5. 下一次 `summarize_world_qi` 中 `container_qi` 下降，但 `ledger_qi` / `zone_qi` 不上升，守恒断言可观测漂移。

## 反方审查记录

- 第一轮反方：通过。结论是 `spirit_quality` 不是纯品质标签，已由 `summarize_world_qi` 计入 `container_qi`；`tsy_entry_portal_tick` 和 `apply_entry_filter` 没有补账路径；`TsyEnterEmit` 下游只做叙事/schema 映射，不携带金额或账本目标。
- 第二轮反方：通过。结论是本缺口不重复 #989；#989 是 `AttritionTax` overflow 落账缺口，本题是 TSY entry filter 完全绕过 `attrition` / `ledger` 直接清空 item `spirit_quality`。目标账户选 `rift`、`zone` 还是 `overflow` 属于修复设计，不影响“当前无去向归零”这个 bug 成立。

## Skeleton Fix Plan

### P0: 入口剥离量显式化

- [ ] 扩展 entry filter 内部返回结构，记录每个被剥离物品的 `stripped_qi = before_spirit_quality * stack_count.max(1)`。
- [ ] 保持现有 `TsyEnterEmit.filtered` 对外字段兼容；是否把 `stripped_qi` 暴露给 schema 另行决策，不作为守恒修复前置。
- [ ] 阈值以下物品不产生转账；`stripped_qi <= QI_EPSILON` no-op。

### P1: 写入坍缩渊内部账本

- [ ] 在 `tsy_entry_portal_tick` 接入 `WorldQiAccount`，对每个剥离项创建临时 source 账户 `QiAccountId::container("item:<instance_id>")`，目标优先使用 `QiAccountId::rift(<family_id>)`。
- [ ] 新增专用 `QiTransferReason::TsyEntryFilter`（命名可在实施时收口），不要复用 `AttritionTax`，因为这是入口负压强制剥离，不是普通 inventory 操作磨损。
- [ ] 因 item 真元真实存于 `PlayerInventory` 而非 ledger balance，参考 `credit_pending_inflow` 的临时 source shadow balance 模式：先把 source 置为本次 stripped amount，再 `transfer(source -> rift)`，转账后 source 归零、rift 累加。
- [ ] 若 `WorldQiAccount` 不存在，不得先清物品再丢账；实施时选择“拒绝入场并 warn”或“确保 app 初始化必有 ledger resource”，测试锁死。

### P2: 塌缩回收衔接

- [ ] 核对活坍缩渊 collapse 路径是否消费 `QiAccountId::rift(<family_id>)` 余额；若没有，补接 `collapse_redistribute_qi(rift_balance, surrounding_zones)`，分发后清空 rift 账户。
- [ ] 无邻接或邻接满容时，剩余量必须进入 overflow 账户，不能只 emit `QiTransfer` 事件。

## 验收测试计划

- [ ] `apply_entry_filter` 单测：containers / equipped / hotbar 中高灵质物品返回正确 `stripped_qi`，低于阈值不返回。
- [ ] 系统级 entry portal 测试：入场前后 `summarize_world_qi.total_observed()` 不变；`container_qi` 下降量等于 `ledger_qi` 中 `rift:<family_id>` 增量。
- [ ] 堆叠边界：`spirit_quality=0.8, stack_count=3` 必须入账 `2.4`，不是 `0.8`。
- [ ] 负例：低灵质物品、空背包、无 `TsyPresence` / 非 Entry portal 不产生转账。
- [ ] 缺 ledger resource 边界：不得出现“物品已清零但无账本增量”的状态。
- [ ] collapse 衔接测试：rift 账户有入口剥离余额时，塌缩后 rift 归零，周边 zone / overflow 合计增加等于原 rift 余额。

## 风险

- `spirit_quality` 当前是 0..1 分数，守恒口径已按 `stack_count` 折算；修复时不要混用 `QI_ZONE_UNIT_CAPACITY`，否则会把物品灵质和 zone 浓度单位重复换算。
- 如果只把 entry filter 改成 `AttritionTax`，会混淆“普通搬运磨损”和“坍缩渊入口强制剥离”，也会让 #989 的历史语义变得不清。
- 如果只发 `QiTransfer` event 而不更新 `WorldQiAccount` balance，仍会落入 `docs/CLAUDE.md §四` 的 emit-only 红线。
- 如果 rift 账户累加后不接 collapse redistribution，守恒 telemetry 能闭合，但坍缩渊“吸入后回收”的玩法闭环仍不完整。
