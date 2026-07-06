# plan-bughunt-alchemy-takeback-full-inventory-loss-v1

> Skeleton plan。BugHunt B9（server-gameplay 第九轮）只读审计产物，不消费、不归档。

## Bug 摘要

`alchemy_take_back` 在成丹或残渣成功发放前就把炼丹炉 session 清掉。若玩家取丹时背包已满，且没有可合并的同 identity 堆叠，`add_item_to_player_inventory_with_alchemy` 返回 `inventory full`；handler 只提示“炼丹产物入袋失败”，但投料材料已经在 `alchemy_feed_slot` 阶段消耗，炉内 session 也已经 `take()` 丢失，产物既不入包也不落地，玩家无法重试领取。

收窄边界：

- 正常初始化下 `InventoryInstanceIdAllocator` 存在，本 plan 不把“编号器未就绪”当主要游玩路径。
- 炼丹产物模板可以堆叠，本 plan 不主张“所有产物必占新格”；问题限定为满包且没有可合并同 identity 堆叠。
- 这是取丹结算顺序导致的不可逆产物丢失，不是刷丹漏洞。

## 对实际游玩体验的影响

玩家完整完成一次炼丹，材料已投入并被扣除，取丹时如果背包在过程中被其他物品填满，就会看到服务端报“炼丹产物入袋失败”。此时丹炉已经不再有可取回的 session，再打开炉子也不是“成品待领取”状态，玩家实际损失了药材、火候操作时间和可能的成丹/丹渣。

这对炼丹体验很硬：玩家会认为“炼成了但取不出来”，而不是明确的“背包满，请腾格后再取”。残渣与丹药还携带动态 `alchemy` 数据，不同时间或不同质量的产物通常不能和旧堆自然合并，因此满包触发不是纯理论边界。

## 证据定位

- 投料已消耗：`handle_alchemy_feed_slot` 在投料成功后调用 `consume_item_instance_once` 扣材料，失败才回滚 `session.staged` 与 inventory。见 `server/src/network/client_request_handler.rs:12243`、`server/src/network/client_request_handler.rs:12307`。
- 取丹先清 session：`handle_alchemy_take_back` 先补齐剩余 tick，然后 `session.finished = true`，随后 `furnace.end_session()`。见 `server/src/network/client_request_handler.rs:12377`、`server/src/network/client_request_handler.rs:12384`。
- `end_session()` 是 `self.session.take()`，会把炉内 session 置空。见 `server/src/alchemy/furnace.rs:85`。
- 发放在清 session 之后：非炸炉分支先调用 `grant_alchemy_outcome_item`，若返回 false 直接 return，不恢复 `ended`。见 `server/src/network/client_request_handler.rs:12464`、`server/src/network/client_request_handler.rs:12477`。
- 入包失败无掉地兜底：`grant_alchemy_outcome_item` 走 `add_item_to_player_inventory_with_alchemy`，失败只发“炼丹产物入袋失败”并返回 false。见 `server/src/network/client_request_handler.rs:12569`、`server/src/network/client_request_handler.rs:12578`。
- 库存模块已有满包掉地工具，但炼丹未使用：`add_item_to_player_inventory_or_ground` 明确在 `"inventory full:"` 时写入 `DroppedLootRegistry`。见 `server/src/inventory/mod.rs:1730`。
- 满包可达：`add_item_to_player_inventory_inner` 在没有可用容器或找不到空位时返回错误。见 `server/src/inventory/mod.rs:1835`、`server/src/inventory/mod.rs:1897`。
- 动态产物不一定能合并：堆叠 identity 比较完整 `alchemy` 字段。见 `server/src/inventory/mod.rs:2120`；残渣 `alchemy` 数据包含 `produced_at_tick` / `expires_at_tick`。见 `server/src/alchemy/residue.rs:82`。
- 项目相邻口径支持“不可逆完成前先保证产物”：采药收获先入包或掉地，成功后才 `plant.harvested = true`。见 `server/src/botany/harvest.rs:155`、`server/src/botany/harvest.rs:208`；工作台拆除入包失败会生成地面掉落。见 `server/src/craft/workbench.rs:228`、`server/src/craft/workbench.rs:247`。

## 触发路径

1. 玩家拥有一座归属自己的炼丹炉，并学会任意可产出丹药或残渣的丹方。
2. 玩家起炉并按配方投料，`alchemy_feed_slot` 成功扣除材料。
3. 在 `alchemy_take_back` 前，玩家背包所有可携容器已满；已有同类产物没有完全相同的 `alchemy` identity，或没有同类未满堆。
4. 玩家发送 `alchemy_take_back`。
5. 服务端先 `end_session()` 清掉炉内会话，再尝试发放成品。
6. `add_item_to_player_inventory_with_alchemy` 返回 `inventory full: <template_id>`。
7. 玩家收到错误提示，但丹炉 session 已空，材料已消耗，产物也没有落地。

## 去重

- 不重复 #1029：#1029 是地面掉落拾取不合并已有堆叠；本问题是炼丹产物根本没有落地，且 session 已清。
- 不重复 #981：#981 是炼丹炉交互缺少服务端距离/维度门禁；本问题是同炉合法取丹后的奖励结算顺序。
- 不重复 #943：#943 是战斗丹消费绕过丹毒门禁；本问题发生在炼丹产物发放前。

## 反方审查记录

### Round 1

反方重点挑战是否有掉地兜底、满包是否可达、`end_session` 后是否可恢复、是否已有同题 PR。结论 PASS：

- 没找到炼丹产物掉地或原子回滚；`ended` 是局部变量，失败 return 后无法恢复。
- `inventory full` 是库存 API 的明确错误路径，正常玩法可达。
- open PR 与本地 docs 检索未发现“alchemy_take_back 满包吞产物/session”同题。

### Round 2

反方进一步挑战“玩家应自留格子”、产物可堆叠、fail-closed 防刷丹、与 #1029/#981/#943 重复。结论 PASS，但要求收窄：

- 可以承认取物前留格子是玩家预期之一，但本仓库对“已不可逆完成再发产物”的相邻系统已采用入包或掉地/延后不可逆副作用口径。
- 可以承认产物模板可堆叠，但动态 `alchemy` identity 让很多产物不能与旧堆合并。
- 修复必须防重复领取，不能简单保留可反复 take 的成品 session。

## Skeleton Fix Plan

### P0：取丹发放改为原子语义

- 在 `handle_alchemy_take_back` 中，先基于当前 session clone 或临时 ended session 计算 outcome，但不要在产物发放成功前永久清掉 `furnace.session`。
- 满包时优先采用与 botany 一致的“入包或掉地”策略：为炼丹产物接入 `DroppedLootRegistry`，用丹炉位置和当前维度生成 dropped loot，并保留完整 `AlchemyItemData`。
- 若决定不掉地，则失败时必须保持炉内可领取状态，并给玩家明确提示“背包满，腾格后再取”，且重复请求只能领取一次。

### P1：防重复领取

- 引入明确的取丹终态，例如 `PendingAlchemyOutcome` / `ReadyToCollect` / `Collected`，避免同一 resolved outcome 被多次发放。
- 成功入包或落地后再清 session 或标记 collected。
- 失败分支必须不触发 `AlchemyOutcomeEvent`、VFX 完成事件、skill XP 或其他完成副作用。

### P2：统一满包体验

- 复用 `add_item_to_player_inventory_or_ground` 或抽出支持 `AlchemyItemData` customization 的公共 grant helper。
- 满包落地后推送 dropped loot snapshot / event stream，确保玩家能在炉边看到并拾取产物。

## 验收测试计划

- `alchemy_take_back_full_inventory_keeps_collectable_or_drops`: 构造已完成 session、玩家背包满且无可合并堆，取丹后断言产物没有消失：要么进入 `DroppedLootRegistry`，要么 furnace 保持可领取状态。
- `alchemy_take_back_full_inventory_does_not_emit_completion_side_effects_on_retryable_failure`: 若采用“失败保留 session”，断言失败时不发 `AlchemyOutcomeEvent`、不发完成 VFX、不加 XP。
- `alchemy_take_back_success_clears_session_once`: 正常有空间取丹后断言 session 清空、背包新增 1 个产物、第二次 `take_back` 不再重复发放。
- `alchemy_residue_dynamic_identity_full_pack_regression`: 放一个同 template 但不同 `produced_at_tick` 的未满残渣堆，再填满背包，断言新残渣不会错误合并，也不会丢失。
- `alchemy_take_back_unknown_template_remains_structural_error`: unknown template / registry 缺项仍作为配置错误暴露，不静默掉地掩盖。

## 风险

- 掉地方案需要把 `AlchemyItemData` customization 带入 dropped loot，否则残渣 TTL、丹药 quality/effect 会丢失。
- 保留 session 方案需要严格防重复领取，尤其是同 tick 重发 `alchemy_take_back` 或客户端重试。
- 若复用 dropped loot，需对齐维度来源，不能硬编码 Overworld；丹炉未来可能出现在 TSY 或其他维度。
- 修复不应改动投料消耗顺序；`feed_slot` 当前已有扣除失败回滚，风险主要集中在 take_back 结算阶段。
