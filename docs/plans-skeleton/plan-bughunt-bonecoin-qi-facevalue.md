# plan-bughunt-bonecoin-qi-facevalue（骨架）

> BugHunt A4 / server-qi r04。只记录骨架，不消费、不归档。

## Bug 摘要

骨币 `ItemInstance.spirit_quality` 是 0..1 的封灵真元残量比例，但 `qi_physics` 的通用容器真元、磨损回灌、磨损反馈都把它当绝对真元量使用，未乘骨币面值。结果是 `bone_coin_40` 满灵时经济价值是 40 点真元，守恒快照和磨损回灌却只按 1 点记账；3% 搬运磨损实际让骨币价值掉 1.2 点，只回灌/反馈 0.03 点。

这不重复 #975 / #989 / #1000：#989 是磨损 overflow 已算出后未落 `WorldQiAccount`，本缺口发生在 overflow 之前，普通未满 zone 也会把骨币绝对量先算小。

## 实际游玩体验影响

玩家制作或获得高面值封灵骨币后，只要经历拾取、背包槽位移动、TSY 容器搜刮、炼丹加料等磨损路径，骨币面值按 5/15/40 的真实价值缩水，但服务器只把 0..1 的比例当作真元退回环境。

直观结果：玩家的 `bone_coin_40` 从满灵 40 点价值降到 38.8 点，周围 zone 只恢复 0.03 点，HUD/事件也只报 0.03 点磨损。高面值骨币越多，经济实际贬值和 qi 守恒账本偏差越大。

## 证据定位

- `server/src/fauna/bone_coin.rs:117`-`123` 定义 `bone_coin_5/15/40` 面值档；`server/src/fauna/bone_coin.rs:197` 写入 `spirit_quality = sealed_qi / bone_grade.qi_cap()`，说明字段是比例。
- `docs/finished_plans/plan-economy-v1.md:20`-`21` 明确 `spirit_quality (0..=1)` 复用为骨币真元残量比例；`docs/finished_plans/plan-economy-v1.md:158` 要求价格指数按骨币面额与 `spirit_quality` 聚合。
- `server/src/economy/mod.rs:59`-`64` 定义骨币面值；`server/src/economy/mod.rs:221`-`234` 用 `face_value * spirit_quality * stack` 统计骨币真实真元供给。
- `server/src/qi_physics/ledger.rs:701`-`702` 的 `item_qi` 只算 `spirit_quality * stack`，导致守恒快照少记高面值骨币。
- `server/src/qi_physics/attrition.rs:263` 用 `spirit_quality * stack` 算磨损绝对真元，`server/src/qi_physics/attrition.rs:285`-`295` 再按这个低估值扣比例并回灌 zone。
- `server/src/network/qi_attrition_emit.rs:63`-`75` 的磨损反馈同样用 `spirit_quality * stack`，导致客户端/事件低报损失。
- 生产入口不是死代码：`server/src/network/client_request_handler.rs:11176`-`11184` 槽位移动、`server/src/network/client_request_handler.rs:11718`-`11725` 拾取、`server/src/network/client_request_handler.rs:12280`-`12289` 炼丹加料、`server/src/world/tsy_container_search.rs:273`-`279` TSY 容器搜刮都会调用 `apply_attrition_checked`。

## 触发路径

1. 玩家用异变兽骨制作 `bone_coin_40`，`apply_bone_coin_craft_session` 产出 `ItemInstance { template_id="bone_coin_40", spirit_quality=1.0 }`。
2. 玩家拾取或移动该骨币，入口调用 `apply_attrition_checked(..., AttritionOpKind::Pickup/SlotMove, ...)`。
3. 当前代码计算 `abs_qi = 1.0 * 1 = 1.0`，3% 磨损为 `0.03`，把 `spirit_quality` 降到 `0.97` 并向 zone 回灌 `0.03`。
4. 经济侧按面值计算，骨币真实供给从 `40 * 1.0 = 40` 变成 `40 * 0.97 = 38.8`，实际损失 1.2。损失和回灌差出 1.17。

## 反方审查记录

- 第 1 轮反方结论：REAL。未找到“骨币在 qi_physics 按归一化单位记账”的约定，也未找到骨币 attrition 豁免；经济侧和 qi_physics 口径确实冲突。
- 第 2 轮反方结论：REAL。确认不是单纯 telemetry：attrition 会真实写 `zone.spirit_qi`；也不是 #989 duplicate，因为 #989 修 overflow 落账不能修 `0.03` vs `1.2` 的前置低估。

## Skeleton Fix Plan

- [ ] 新增统一 helper：按 `ItemInstance.template_id` 识别骨币面值，返回 `item_effective_qi(item)`；普通物品保持 `spirit_quality * stack`，骨币返回 `face_value * spirit_quality * stack`。
- [ ] `qi_physics::ledger::item_qi`、`qi_physics::attrition::apply_attrition_checked`、`network::qi_attrition_emit::item_abs_qi_for_attrition` 改用同一 helper，避免三套口径继续漂移。
- [ ] 修磨损比例写回：骨币仍按比例降低 `spirit_quality`，但回灌/overflow/反馈使用面值后的绝对损失。
- [ ] 核验骨币制作的无催化剂 `seal_cost` 去向：若它不是封入骨币的真元，必须明确释放到 zone / pending pool / overflow，不能随 `qi_current` 扣减蒸发。
- [ ] 加一条防回归文档/测试约束：任何以 `spirit_quality` 表示“比例”的物品，进入 qi_physics 前必须有面值/容量换算。

## 验收测试计划

- `cargo test qi_physics::ledger::tests::inventory_qi_counts_bone_coin_face_value`：`bone_coin_40`、`spirit_quality=1.0`、`stack=1` 的 `container_qi` 应为 40，不是 1。
- `cargo test qi_physics::attrition::tests::bone_coin_40_pickup_attrition_returns_face_value_loss`：普通 zone 下 3% pickup 后，`spirit_quality=0.97`，zone 增量为 `1.2 / QI_ZONE_UNIT_CAPACITY`，事件 amount 为 1.2。
- `cargo test network::qi_attrition_emit::tests::bone_coin_attrition_payload_uses_face_value`：磨损反馈 amount 对齐面值损失。
- `cargo test fauna::bone_coin::tests::craft_session_converts_bone_and_qi_to_coin`：现有制作测试继续通过，并补断言制作后 qi 快照不因面值少计漂移。
- `cargo test qi_physics::attrition::tests::ordinary_spirit_quality_item_keeps_existing_absolute_semantics`：普通灵物仍按 `spirit_quality * stack`，避免把所有物品误乘骨币面值。

## 风险

- `economy::bone_coin_face_value` 目前在 economy 模块内；修复时若直接从 qi_physics 反向引用 economy 会形成不合适的依赖方向。建议把骨币面值解析移动到 inventory/item 语义层或 qi_physics 可依赖的轻量 helper。
- `PlayerInventory.bone_coins` legacy 标量仍被 NPC 交易、灵田补灵等旧路径使用；本 bug 针对 item 骨币，修复不能误把 legacy 标量重复计入容器真元。
- `spirit_quality` 被多类物品复用为纯度/残量/比例；修复必须白名单骨币模板，不能全局把所有 `spirit_quality` 都按模板面值放大。
