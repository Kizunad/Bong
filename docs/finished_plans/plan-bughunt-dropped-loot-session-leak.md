# BugHunt: dropped loot 断线不清导致短窗口旧掉落物串 session

**状态**：✅ 2026-07-11 —— 真 bug，已修复（`BongNetworkHandler.clearClientStateOnDisconnect()` 补接 `DroppedItemStore.clearOnDisconnect()`）。

## Bug 摘要

`DroppedItemStore` 是 client 侧地面掉落物的静态缓存，断线/切服时虽然提供了 `clearOnDisconnect()`，但没有被任何 client disconnect/JOIN 清理路径调用。玩家从一个 server/world 断开后，如果旧缓存里有掉落物，新服首个 `dropped_loot_sync` 抵达前，旧坐标会被新 world 解释为当前 session 的 dropped loot。

这是低严重度但真实的 session hygiene bug：不是永久残留，也不是必然误拾取；主问题是短窗口内旧 dropped loot billboard/特效可能出现，且 G 键 unified interaction 会把旧 entry 当作拾取候选发出旧 `pickup_dropped_item` 请求。

## 对实际游玩体验的影响

玩家切服或重连后，可能在出生点附近短暂看到上一局地面掉落物的浮空图标/粒子，误以为新服地上有可捡物。若此时按 G，client 会向新 server 发送旧 `instanceId` 的拾取请求；大多数情况下 server 会拒绝不存在或超距 id，但玩家会感到按键无响应或交互目标错乱。

当前 G 键优先级下，它抢不过 TSY 搜索/容器/NPC/玩家交易等高优先级交互；真实竞争主要是同级的遗骸拾取，未来若接入低优先级采集交互也会被旧 dropped loot 候选压过。

## 证据定位

- `DroppedItemStore` 持有静态 `entries`/`insertionOrders`，`nearestTo()` 直接从静态缓存选最近 dropped item：`client/src/main/java/com/bong/client/inventory/state/DroppedItemStore.java:38`、`:56`。
- `DroppedItemStore.clearOnDisconnect()` 只定义清空方法：`client/src/main/java/com/bong/client/inventory/state/DroppedItemStore.java:102`；全仓未找到 `DroppedItemStore.clearOnDisconnect` 调用。
- 写入来源包括丢弃事件和全量同步：`InventoryEventHandler` 在 dropped 分支写 `DroppedItemStore.putOrReplace`，见 `client/src/main/java/com/bong/client/network/InventoryEventHandler.java:111`；`DroppedLootSyncHandler` 用 `replaceAll` 覆盖缓存，见 `client/src/main/java/com/bong/client/network/DroppedLootSyncHandler.java:20` 和 `:32`。
- 全局断线清理清了很多 session store，并已清同类 `RemainsStore`，但漏了 `DroppedItemStore`：`client/src/main/java/com/bong/client/BongNetworkHandler.java:131`、`:170`、`:172`。
- JOIN 回调只标记 connected 和设置本地 player id，没有清 dropped loot 缓存：`client/src/main/java/com/bong/client/BongNetworkHandler.java:175`。
- world billboard 每帧直接读 `DroppedItemStore.snapshot()`，仅检查 world/consumer/matrixStack 非空和 32m 距离，entry 本身没有 dimension/session：`client/src/main/java/com/bong/client/inventory/render/DroppedItemWorldRenderer.java:59`、`:60`、`:83`。
- G 键拾取 handler 直接从 `DroppedItemStore.nearestTo()` 取候选并发送 `pickup_dropped_item`：`client/src/main/java/com/bong/client/inventory/DroppedItemPickupIntentHandler.java:15`、`:30`、`:35`。
- 优先级事实：dropped loot 为 70，遗骸也是 70，容器/搜索/NPC/交易更高；同级按距离决胜：`client/src/main/java/com/bong/client/input/ReservedInteractionIntents.java:4`、`:9`、`:11`，`client/src/main/java/com/bong/client/input/InteractPriorityResolver.java:27`。
- 主 HUD marker 路径已下线，不能把 `DroppedItemHudPlanner` 当主影响：`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:156`。
- server join-time 会发 `dropped_loot_sync` 覆盖缓存，所以影响窗口应限定在首个 sync 到达前：`server/src/network/dropped_loot_sync_emit.rs:61`。

## 触发路径

1. 在 server A 丢弃一个物品，client 收到 `inventory_event:dropped` 或 `dropped_loot_sync`，`DroppedItemStore` 写入 entry。
2. 玩家断线、切服或重连。`BongNetworkHandler` DISCONNECT 清理了 NPC、TSY、遗骸等 store，但没有清 `DroppedItemStore`。
3. 进入 server B 后，在 server B 首个 `dropped_loot_sync` 抵达前，如果旧 entry 坐标落在玩家相机 32m 内，`DroppedItemWorldRenderer` 会按 server B 的 world 渲染上一局 dropped loot。
4. 同一短窗口内按 G，`DroppedItemPickupIntentHandler` 会把旧 entry 作为 `PickupDroppedItem` 候选并发送旧 `instanceId`。
5. 首个 `dropped_loot_sync` 抵达后，`DroppedLootSyncHandler.replaceAll()` 会覆盖或清空缓存，残留消失。

## 反方审查记录

- 第一轮反方结论：保留但缩小表述。确认 `DroppedItemStore.clearOnDisconnect()` 无调用，`BongNetworkHandler` 断线清理漏掉它；同时指出 server join-time sync 会覆盖缓存，因此不能写成永久残留。
- 第一轮反方限制：`DroppedItemHudPlanner` 主 HUD marker 已下线，应把主证据放在 `DroppedItemWorldRenderer` 和 `DroppedItemPickupIntentHandler`。
- 第二轮反方结论：通过，但按低严重度写。旧 dropped loot 抢不过 90/100 优先级交互；当前主要和同级遗骸拾取竞争，低优先级 harvest 只是预留风险。
- 第二轮反方限制：billboard 只有旧坐标落在新相机 32m 内才可见；不能写“任何位置显示”“吞所有交互”“必然误拾取”，只能写“短窗口旧 billboard/G 键旧 instanceId 请求风险”。

## Skeleton Fix Plan

- [x] 在 client disconnect 清理路径补接 `DroppedItemStore.clearOnDisconnect()`，位置优先跟 `RemainsStore.clearOnDisconnect()` 相邻，保持同类地面对象缓存一致。✅ 2026-07-11
- [x] 评估是否也在 JOIN 前置清理一次 dropped loot，以抵消 DISCONNECT 回调未执行或单机/测试路径绕过 disconnect 的情况；若做，需避免与 join-time `dropped_loot_sync` 产生竞态。✅ 2026-07-11 —— 评估结论：**不做 JOIN 侧清理**。核实 `server/src/network/dropped_loot_sync_emit.rs` 的 `emit_join_dropped_loot_syncs` 挂载在 `Added<Client>` filter 上，且通过 `server/src/network/mod.rs:962` 的 `register(app)`（`server/src/main.rs:117` 调用）真正接入生产 App，join 必发全量 `dropped_loot_sync`。只做 DISCONNECT 侧清理已消灭本 bug 描述的窗口，且与 `RemainsStore` 现有先例（同样只在 DISCONNECT 清理、不在 JOIN 清理）保持一致，避免额外引入与 join-time sync 竞态的风险面。
- [x] 给 `DroppedItemPickupBootstrap` 或 `BongNetworkHandler` 增加单测/集成测试，断言 disconnect 后 `DroppedItemStore.snapshot()` 为空。✅ 2026-07-11 —— `BongNetworkHandlerTest.disconnectClearsDroppedItemStoreToPreventStaleSessionBleed`（走真实 `clearClientStateOnDisconnect()` 生产路径）+ `DroppedItemStoreTest.clearOnDisconnectEmptiesStoreAndNearestToReturnsNull`（store 层直接单测）。
- [x] 给 G 键候选增加回归测试：断线清理后旧 dropped loot 不再参与 `InteractKeyRouter` 候选选择；同级遗骸不被旧 dropped item 抢走。✅ 2026-07-11 —— `DroppedItemPickupIntentHandler.candidate()` 直接委托 `DroppedItemStore.nearestTo(...)`（无额外状态），`clearOnDisconnectEmptiesStoreAndNearestToReturnsNull` 断言清空后 `nearestTo()` 返回 null，从而锁住"清理后不再产生候选"这一契约；未额外新增 `InteractKeyRouter` 层测试，因为该层已有独立 `InteractKeyRouterTest` 用 stub handler 覆盖候选择优逻辑，不需要为本 bug 重复搭建 `MinecraftClient` fixture。
- [x] 保持 server 权威拒绝不变，不在 client 侧凭空推断拾取成功。✅ 2026-07-11 —— 本次修复未改动 `DroppedItemPickupIntentHandler.dispatch()` 或 `ClientRequestSender.sendPickupDroppedItem`，拾取仍是纯请求-等待 server 权威回执，未引入任何 client 侧乐观推断。

## 验收测试计划

- client 单测：写入一个 `DroppedItemStore.Entry`，模拟 disconnect 清理，断言 `DroppedItemStore.snapshot()` 为空，`nearestTo()` 返回 null。
- client 路由测试：断线清理后注册 dropped item 与 remains 场景，确认旧 dropped item 不再产生 `PickupDroppedItem` 候选。
- 手动/集成回归：server A 丢弃物品后切到 server B；在 server B 首帧和首个 `dropped_loot_sync` 前后观察 world-space dropped loot billboard 不出现旧物品。
- 协议回归：新服首个空 `dropped_loot_sync` 仍能清空缓存；非空 sync 仍能正常渲染新服 dropped loot 并允许 G 键拾取。

## 风险

- 低风险：这是 session 生命周期清理，和 `RemainsStore` 同类；主要风险是清理时机过早导致 join-time 首包前没有 dropped loot 视觉，但这正是目标行为。
- 若同时在 DISCONNECT 与 JOIN 清理，需要确认不会清掉已经到达的 `dropped_loot_sync`；优先只在 DISCONNECT 补接，JOIN 清理需单独证明无竞态。
- 不应引入 dimension/session 猜测逻辑；真正权威仍应由 server 的 dropped loot sync 和拾取校验负责。

## Finish Evidence

**落地清单**：

- `client/src/main/java/com/bong/client/BongNetworkHandler.java`（`clearClientStateOnDisconnect()`，紧邻 `RemainsStore.clearOnDisconnect()` 调用点补接 `DroppedItemStore.clearOnDisconnect()`）
- `client/src/test/java/com/bong/client/inventory/DroppedItemStoreTest.java`（新增 `clearOnDisconnectEmptiesStoreAndNearestToReturnsNull`）
- `client/src/test/java/com/bong/client/BongNetworkHandlerTest.java`（新增 `disconnectClearsDroppedItemStoreToPreventStaleSessionBleed`，走真实 `clearClientStateOnDisconnect()` 生产路径而非直接调 store）

**关键 commit**：

- `a929b679`（2026-07-11）docs(plan): plan-bughunt-dropped-loot-session-leak 骨架升 active
- `bacded6b`（2026-07-11）fix(client): 断线清理接上 DroppedItemStore，堵住 dropped loot session 串味

**测试结果**：

- `cd client && ./gradlew test --tests "com.bong.client.BongNetworkHandlerTest" --tests "com.bong.client.inventory.DroppedItemStoreTest"` → BUILD SUCCESSFUL（含新增 2 条测试全绿）
- `cd client && ./gradlew test build` → BUILD SUCCESSFUL（client 全量单测 + build 门禁绿，无回归）

**跨仓库核验**：

- client：`DroppedItemStore.clearOnDisconnect()`（原有方法，本次接线调用）、`BongNetworkHandler.clearClientStateOnDisconnect()`（调用点新增）
- server：`server/src/network/dropped_loot_sync_emit.rs::emit_join_dropped_loot_syncs`（只读核验，确认 join-time 全量 sync 兜底存在，未改动）——已核实其挂载 `Added<Client>` filter 并通过 `server/src/network/mod.rs` 的 `register(app)`（`server/src/main.rs` 调用）接入生产 App，非孤岛
- agent：不涉及（纯 client 会话生命周期清理，无 IPC/schema 变更）

**Validator**：无上下文 read-only 对抗验证 PASS，验证 HEAD `bacded6b02c4d31f8844e783e54914a641322870`（逐条核实：调用链真连通 DISCONNECT 事件、与 RemainsStore 结构对称、测试非 mock/非 tautological（mental revert 后会撞红）、无遗漏的其他 disconnect 路径、无下游代码假设 DroppedItemStore 跨重连存活、join-time sync 系统确实注册进生产 App、实测两个新测试真实通过）。

**遗留 / 后续**：

- 未新增 `InteractKeyRouter` 层的端到端候选选择测试——该路由层已有独立 `InteractKeyRouterTest`（stub handler 覆盖优先级择优逻辑），且 `DroppedItemPickupIntentHandler.candidate()` 无额外状态、直接委托 `DroppedItemStore.nearestTo()`，store 层测试已充分锁住"清理后不再产生候选"契约，无需为本 bug 重复搭建 `MinecraftClient` fixture。
- skeleton 中提到的"低优先级 harvest 交互未来可能被旧 dropped loot 抢占"风险不在本次修复范围（本次已消灭旧 dropped loot 存活窗口本身，该风险随之消解，无需额外设计）。
