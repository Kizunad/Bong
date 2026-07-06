# plan-bughunt-dropped-loot-pickup-stack-merge-v1

> Skeleton BugHunt plan。只记录真实 bug 与修复边界，不消费、不归档、不修改运行代码。

## Bug 摘要

世界掉落物拾取路径不会合并已有同类堆叠：`pickup_dropped_loot_instance` 只找一个空 footprint，把地面 `ItemInstance` 原样 `attach_at_location` 到背包；如果背包网格没有空格，即使玩家已有未满的同模板同属性堆叠，也会返回 `no free container slot` 并拒绝拾取。

这与普通奖励/采集入包路径不一致。`add_item_to_player_inventory_inner` 会读取模板 `max_stack_count`，先在 staged container 上合并同 identity stack，再决定是否需要新格子；世界掉落拾取完全绕过这套 merge 语义。

## 对实际游玩体验的影响

玩家打怪或破坏放置物后，地上明明是可堆叠材料，背包里也已有同类未满堆叠，却因为没有额外空格而捡不起来。典型例子是黑武士保底掉 `sword_embryo_shard x2`：玩家背包被其它 1x1/1x2 物品占满，但已有一组未满的剑胚残片时，服务端仍拒绝拾取，表现为“地上材料就在脚边，背包也能并堆，但交互无效”。这会直接破坏战利品回收、Boss 奖励兑现和背包整理体验。

## 证据定位

- 普通入包会合并：`server/src/inventory/mod.rs:1817-1841` 读取模板与 `max_stack_count`，`server/src/inventory/mod.rs:1859-1871` staged 合并已有同 identity stack，`server/src/inventory/mod.rs:1909-1926` 在真实容器上执行 merge 并记录 `merged_instance_ids`。
- 世界掉落拾取不合并：`server/src/inventory/mod.rs:4915-4942` 只从 `DroppedLootRegistry` clone entry，做 2.5 格距离检查，然后调用 `find_first_fit_container_location` + `attach_at_location`；找不到空格就返回 `no free container slot for dropped instance ...`。
- `find_first_fit_container_location` 不是堆叠查找：`server/src/inventory/mod.rs:5893-5939` 逐格扫描坐标并调用 `validate_attach_fits`；`server/src/inventory/mod.rs:5150-5186` 遇到 footprint overlap 直接 `TargetOccupied`，没有“放到已有 stack”语义。
- 黑武士真实触发源：`server/src/fauna/drop.rs:214-216` 保底 `sword_embryo_shard x2`；`server/src/fauna/drop.rs:428-457` 将 roll 出来的 `ItemInstance` 直接写入 `DroppedLootRegistry`。
- `sword_embryo_shard` 可堆叠：`server/assets/items/sword_materials.toml:75-82` 为 `category = "misc"`、`grid_w = 1`、`grid_h = 2`、`spirit_quality_initial = 0.35`；`server/src/inventory/mod.rs:2443-2451` 中 `Misc` 默认 `max_stack_count = 16`。
- 修复风险：`server/src/network/client_request_handler.rs:11686-11732` 在 pickup 成功后仍按原 dropped `instance_id` 在 inventory 里查物品并执行 `AttritionOpKind::Pickup` 磨损；如果 merge 后原实例 id 消失，当前网络层会找不到目标或错误磨损旧堆。
- 磨损按整堆绝对真元算：`server/src/qi_physics/attrition.rs:248-265` 使用 `item.spirit_quality * item.stack_count` 判断阈值并结算；`server/src/network/qi_attrition_emit.rs:63-75` 也按整堆 `abs_qi` 计算损耗事件。合并修复必须只结算“新拾取数量”，不能把旧堆一起磨损。

## 触发路径

1. 玩家背包所有容器 footprint 都已被占满。
2. 背包内已有一组未满的 `sword_embryo_shard`，且字段与即将拾取的掉落物完全一致。
3. 黑武士死亡触发 `HEIWUSHI_DROPS`，生成 `sword_embryo_shard x2` 的 `DroppedLootEntry`。
4. 玩家同维、2.5 格内发送 `PickupDroppedItem`。
5. `pickup_dropped_loot_instance` 不查 merge capacity，只找空 footprint；由于背包无空格，返回失败，掉落物留在地上。

## 反方审查记录

- 第 1 轮：反方确认候选成立。最强支持点是普通 grant 与 dropped pickup 的路径差异明确；`find_first_fit_container_location` 只代表空 footprint，不代表 stack merge。最强反驳点是世界掉落是已有实例，修复不能简单吞掉 `instance_id`，因为 pickup 后续磨损仍按原 id 查库存。
- 第 2 轮：反方继续确认成立，但要求 skeleton 明确修复边界。核心结论：修复必须引入 richer pickup receipt，并让 attrition 只作用于 incoming 数量对应的真元；不能把整组旧 stack 一起磨损，也不能先磨损地上物导致 `stack_identity_matches` 失配。
- 去重结论：不重复 #973/#981/#990/#1004/#1007/#1014/#1022。#1007 是掉落物跨维可见/可拾取；本 bug 是同维近距下的 stack merge 缺口。#990 是普通世界容器断线软锁；本 bug 不涉及会话锁。#1014 是玩家交易跨维换货；本 bug 不涉及交易状态机。

## Skeleton Fix Plan

1. 为 dropped pickup 引入 receipt：
   - `pickup_dropped_loot_instance` 不再只返回 `InventoryRevision`。
   - receipt 至少包含 `revision`、`consumed_drop_instance_id`、`created_instance_ids`、`merged_instance_ids`、`picked_template_id`、`picked_stack_count`，以及 attrition 应使用的目标信息。
   - 网络层日志、resync、attrition 不再假设 dropped `instance_id` 一定存在于拾取后的 inventory。

2. 增加安全 merge 分支：
   - `pickup_dropped_loot_instance` 需要接入 `ItemRegistry` 或等价模板上下文，只有模板存在且 `max_stack_count > 1` 时才允许 merge。
   - merge 必须使用完整 `stack_identity_matches`，保护 `freshness`、`mineral_id`、`charges`、`forge_*`、`alchemy`、`lingering_owner_qi`、`durability`、`spirit_quality` 等字段；禁止 template-only merge。
   - 候选 stack 必须未满；若 incoming 数量无法全部并入且无明确 split/remainder 设计，本轮应整体失败并保持 inventory 与 `DroppedLootRegistry` 不变。
   - `max_stack_count = 1` 的武器、防具、容器、法宝、scroll、charged 独立实例等仍走原“找空格保留实例 id”路径。

3. 修正 pickup attrition：
   - 合并场景不能直接对合并后的整堆调用现有 `apply_attrition_checked`，否则旧堆也会被磨损。
   - attrition 应只按 incoming 物品的 `spirit_quality * picked_stack_count` 结算，损耗通过守恒 ledger 归还 zone。
   - 合并后的目标 stack 可用加权绝对真元更新 `spirit_quality`，但必须保证旧堆原有绝对真元不被额外扣减。
   - `AttritionAppliedEvent` payload 必须能描述 merged pickup，不再强依赖 consumed dropped instance id 仍在 inventory。

4. 保持原路径：
   - 有空 footprint 且不能安全 merge 时，仍保留原 dropped `instance_id` 入包。
   - pickup 失败时，drop entry 留在 `DroppedLootRegistry`，inventory revision 不变。
   - dropped loot sync 仍由 registry 内容变化广播，不新增孤岛同步路径。

## 验收测试计划

- `inventory::tests::pickup_dropped_loot_merges_when_no_free_footprint_but_matching_stack_has_capacity`：构造满格背包 + 未满 `sword_embryo_shard` stack + 地面同 identity `sword_embryo_shard x2`，pickup 成功、drop 移除、目标 stack 数量增加、revision bump。
- `inventory::tests::pickup_dropped_loot_rejects_without_mutation_when_matching_stack_capacity_insufficient`：容量不足且无空格时，不部分合并，drop 仍在 registry，inventory 原样。
- `inventory::tests::pickup_dropped_loot_does_not_merge_special_field_mismatch`：同 template 但 `freshness/mineral_id/charges/alchemy/forge/spirit_quality` 任一字段不同，无空格时拒绝且不 mutate。
- `network::client_request_handler` 或专门 integration test：merged pickup 后仍触发 pickup attrition，并且只损耗 incoming 数量对应的 abs qi，不磨损旧堆存量。
- receipt pin：纯 merge outcome 不再依赖原 drop `instance_id` 查 inventory；created/merged outcome 可驱动日志、resync、attrition 与事件。
- 旧路径回归：有空 footprint 且不可合并的掉落物仍按原实例 id 入包；2.5 格距离门禁保持不变。

## 风险

- pickup receipt 会触碰 network handler、inventory event、attrition event 的调用契约，需避免把普通 `InventoryRevision` 返回值替换成半成品。
- 真元守恒风险高：stack 合并后若按整堆磨损，会扣旧物真元；若先磨损 incoming 再 merge，又可能因 `spirit_quality` 改变导致无法匹配。实现必须先设计 abs qi 分摊公式并加守恒断言。
- 特殊实例不能被误合并。带 freshness、矿物、炼丹、锻造、charges、灵宝残留真元等字段的 item 必须维持 identity 隔离。
- 该修复与 #1007 的跨维 pickup gate 是相邻模块但不同问题；落地时不要把维度门禁改动混进本 plan。
