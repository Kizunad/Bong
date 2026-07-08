# plan-bughunt-dropped-loot-g-pickup-range-desync-v1

> Skeleton Plan。BugHunt C8 client-ui 第八轮只记录问题与修复骨架；不要在本 plan 中消费、归档或顺手修改代码。

## Bug 摘要

统一交互键 `G` 的 dropped loot fallback 没有对齐服务端 2.5 格拾取范围。客户端 `DroppedItemPickupIntentHandler` 只要 `DroppedItemStore.nearestTo(...)` 返回条目就产出 `PickupDroppedItem` 候选，并在 `dispatch()` 里重新取最近条目后直接发送 `pickup_dropped_item`；服务端 `pickup_dropped_loot_instance` 却会在距离超过 `2.5 * 2.5` 时拒绝。

这不是 #1007 的跨维同坐标拾取，也不是 #984 的断线旧 store 残留。本案限定为：同维、当前有效 dropped loot store、掉落物实体本身离玩家超过服务端拾取范围，但客户端仍把它当作 `G` 的可拾取 fallback。

## 对实际游玩体验的影响

玩家在 32m 内能看到 dropped loot 的 world-space billboard；如果周围没有更高优先级交互目标，按 `G` 会向服务端发送一个注定失败的 `pickup_dropped_item`。服务端拒绝后只重发 inventory snapshot，不给“太远了”一类玩家可见反馈，所以体感是“明明看得见地上的东西，按 G 没反应”。

更糟的场景是掉落物在当前 store 中只有远处一个条目：`G` fallback 会稳定命中这个远处条目，直到玩家走进 2.5 格内或 store 更新。玩家会把问题理解成按键被吞、交互路由坏了，尤其在战后捡装备、满包 overflow 掉落、TSY 清场回收这类高频路径里。

## 证据定位

- `client/src/main/java/com/bong/client/inventory/DroppedItemPickupIntentHandler.java:15-26`：`candidate()` 只要求 `nearest != null`，虽然算出 `distanceSq`，但只用于路由排序，没有 range gate。
- `client/src/main/java/com/bong/client/inventory/DroppedItemPickupIntentHandler.java:30-36`：`dispatch()` 忽略传入 candidate 的目标，重新 `nearest(client)` 后直接 `sendPickupDroppedItem(...)`。
- `client/src/main/java/com/bong/client/inventory/state/DroppedItemStore.java:56-73`：`nearestTo(...)` 只选最近 dropped loot，没有最大距离参数或截断逻辑。
- `client/src/main/java/com/bong/client/inventory/render/DroppedItemWorldRenderer.java:42-86`：world-space billboard 在 32m 内渲染 dropped loot，玩家可以在远超 2.5 格范围外看到目标。
- `server/src/inventory/mod.rs:4915-4932`：服务端权威拾取超过 2.5 格直接返回 `out of pickup range`。
- `server/src/network/client_request_handler.rs:11750-11761`：服务端拒绝 pickup 后只 `resync_snapshot(..., "pickup_rejection")`，没有对客户端/玩家发可见的太远提示。
- `docs/finished_plans/plan-input-binding-v1.md:76` 与 `:132-135`：早期设计只写 `DroppedItemStore.nearestTo(...) != null`，解释了问题来源，但没有覆盖服务端 2.5 格门禁。
- `docs/finished_plans/plan-inventory-v1.md:51`：HUD marker 已下线，不能作为“远处只是方向提示”的兜底；当前可见反馈主要是 world-space billboard。

## 触发路径

1. 地面存在 dropped loot，客户端通过 `dropped_loot_sync` 收到当前有效 entry。
2. 玩家与该 entry 距离大于 2.5 格，但在 32m 渲染范围内，或 store 中没有更近 dropped loot。
3. 玩家当前没有更高优先级的准星容器、NPC、玩家交易目标。
4. 玩家按 `G`。
5. `DroppedItemPickupIntentHandler.candidate()` 产出 priority 70 的 `PickupDroppedItem` 候选；路由选择它。
6. `dispatch()` 发送 `pickup_dropped_item(instance_id)`。
7. 服务端因距离超过 2.5 格拒绝，只重发 inventory snapshot；玩家没有明确反馈。

## 反方审查记录

Round 1 反方最强攻击：`plan-input-binding-v1` 早期确实把 dropped pickup 条件写成 `DroppedItemStore.nearestTo(...) != null`，并要求 dispatch 发 `sendPickupDroppedItem(instanceId)`；高优先级交互也会压过 priority 70 的 pickup。因此这不是“实现偏离旧文档”的简单 bug。

Round 1 结论：候选保留。旧文档没有否定服务端 2.5 格范围；客户端与服务端门禁不一致仍会制造无效 C2S。开放 PR #1007/#984/#910 的问题边界分别是跨维、断线残留、实体交互距离漂移，未覆盖同维 dropped loot 的 2.5 格 false-positive。

Round 2 反方继续攻击影响面：并非所有远处 dropped loot 都会劫持 `G`，因为容器/NPC/玩家交易会以更高 priority 胜出，同 priority 的遗骸也可能按更近距离胜出。

Round 2 结论：候选通过，但严重度按“局部但真实的 UI/input false-positive”记录。真实影响集中在没有更高优先级候选、store 里只有远处 dropped loot、或玩家正在按 world billboard 尝试捡远处物品的场景。

## Skeleton Fix Plan

- 在 `DroppedItemPickupIntentHandler` 内引入与服务端一致的客户端准入常量，建议命名为 `MAX_PICKUP_DISTANCE_SQ = 2.5 * 2.5`，只作为 UI/input 预过滤；server 仍保持权威校验。
- `candidate()` 在 `nearest != null` 后先判断距离，超过范围时返回 `Optional.empty()`，让 `G` 可以落到其它同 tick 候选或保持 no-op。
- `dispatch()` 不应重新选择任意最近条目；应解析 `candidate.debugLabel()` 中的 `dropped_loot:<instance_id>`，并在发送前用 `DroppedItemStore.get(instanceId)` 复查目标仍存在且仍在 2.5 格内。
- 对“远处可见但不可拾取”的 world-space billboard 是否需要交互提示另开设计；本修复优先阻止无效 C2S，不改变渲染距离。
- 不改 `DroppedItemStore.nearestTo(...)` 的通用语义，避免影响 world renderer、测试里的 tie-break 规格和其它读者。

## 验收测试计划

- `DroppedItemPickupIntentHandlerTest`：2.5 格内 dropped loot 产出 `PickupDroppedItem` 候选，debug label 为 `dropped_loot:<id>`。
- `DroppedItemPickupIntentHandlerTest`：2.5 格外、32m 内 dropped loot 不产出 pickup 候选。
- `DroppedItemPickupIntentHandlerTest`：`dispatch()` 使用 candidate id；若 nearest 在 candidate 生成后变化，不应改发到另一个 dropped loot。
- `DroppedItemPickupIntentHandlerTest`：candidate id 对应 entry 已消失或距离变为 2.5 格外时，dispatch 返回 false 且不发 C2S。
- `InteractKeyRouterTest`：远处 dropped loot 不阻塞其它合法同 tick 候选；没有其它候选时 `G` no-op。
- server 侧保留现有 `pickup_dropped_loot_instance` 2.5 格拒绝测试；新增 client 测试应引用同样数值或共享注释说明，避免未来再次漂移。

## 风险

- 旧 `plan-input-binding-v1` 的文字“附近只有 dropped loot：按 G 仍发送 `pickup_dropped_item`”需要在修复 PR 中解释为“附近且在服务端拾取范围内”，否则表面上像回归。
- 2.5 格常量如果只复制到 client，未来仍可能双端漂移；短期 skeleton 建议在测试名和注释中 pin 住服务端来源，长期可考虑共享协议常量。
- 只做客户端过滤不能替代 server 权威校验；跨维、旧 store、竞态被他人拾取等问题仍应由 server 拒绝。
- world-space billboard 仍会显示 32m 内掉落物。修复后玩家看到远处物品但按 `G` 不发送 pickup，这是正确门禁，但若需要更强反馈应另立 UI affordance，不混入本修复。
