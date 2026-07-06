# plan-bughunt-dropped-loot-cross-dimension-pickup-v1

> 状态：Skeleton Plan（BugHunt B5 / server-gameplay 第五轮）。
> 一句话主题：通用 dropped loot 链路记录了 `DroppedLootEntry.dimension`，但 S2C 同步、client store/拾取候选、server pickup 权威结算都没有维度门禁；结果是两个维度同数值坐标 2.5m 内时，玩家可以看到并拾取另一维度的掉落物。

## Bug 摘要

`DroppedLootEntry` 在 server 内部有 `dimension` 字段，但该字段没有进入 `DroppedLootEntryV1` / protobuf / TS schema，也没有参与 `pickup_dropped_loot_instance` 的 server 权威校验。`DroppedLootRegistry` 是全局 `HashMap<u64, DroppedLootEntry>`，`dropped_loot_sync` 对所有 client 广播全量 registry；客户端只按 XYZ 选最近掉落物并发送 `instance_id`；服务端只按 XYZ 距离 2.5m 放行。

边界条件：这不是“任意距离跨维拾取”，而是“跨维同数值坐标 2.5m 内可误拾取”。但 TSY 和 Overworld 是独立维度却共享普通数值坐标系，TSY 掉落物、死亡掉落、丢弃/overflow 都会进入同一个全局 registry，所以该状态可构造、可被客户端拿到 `instance_id`。

## 对实际游玩体验的影响

玩家在主世界靠近某个坐标时，可能看到并按 G 拾取 TSY 同坐标附近的秘境遗物、死亡掉落或满包 overflow 掉落；反过来，TSY 内玩家也可能把主世界同坐标的掉落物吸进背包。实际体验是“地上出现不属于当前维度的物品标记/发光物，按 G 后另一维度物品消失并进入当前玩家背包”。

这会破坏 TSY “本次秘境所得留在秘境、可能随塌缩化灰”的风险闭环，也会让死亡掉落的空间争抢变成跨维同坐标争抢。多人场景中，A 在 TSY 死亡掉的高价值遗物，B 不进入 TSY 也可能在主世界同数值坐标附近捡走。

## 证据定位

- `server/src/inventory/mod.rs:3666-3673`：`DroppedLootEntry` 包含 `dimension: DimensionKind`。
- `server/src/inventory/mod.rs:3677-3683`：`DroppedLootRegistry.entries` 是全局 `HashMap<u64, DroppedLootEntry>`，没有按维度分区。
- `server/src/inventory/mod.rs:4869-4873`：`dropped_loot_snapshot` 直接导出全量 `registry.entries.values()`。
- `server/src/network/dropped_loot_sync_emit.rs:61-72`：join 时对新 client 发送全量 `dropped_loot_sync`。
- `server/src/network/dropped_loot_sync_emit.rs:82-110`：内容变化时对所有 `Client` 广播全量 snapshot。
- `server/src/network/dropped_loot_sync_emit.rs:114-122`：`to_wire_entry` 丢弃 `entry.dimension`。
- `server/src/schema/server_data.rs:1896-1903`、`proto/bong/envelope.proto:2732-2740`、`agent/packages/schema/src/server-data.ts:423-431`：`DroppedLootEntryV1` / proto / TS schema 都没有 `dimension`。
- `client/src/main/java/com/bong/client/network/DroppedLootSyncHandler.java:36-55`、`client/src/main/java/com/bong/client/inventory/state/DroppedItemStore.java:27-36`：client store 只有 instance/source/XYZ/item，无维度。
- `client/src/main/java/com/bong/client/inventory/DroppedItemPickupIntentHandler.java:39-47`：候选只按当前玩家 XYZ 找 `DroppedItemStore.nearestTo`，dispatch 只发 `instance_id`。
- `server/src/network/client_request_handler.rs:11675-11691`：pickup handler 只把 `player_pos` 和 `instance_id` 传给核心函数，没有传玩家维度。
- `server/src/inventory/mod.rs:4915-4942`：`pickup_dropped_loot_instance` 只校验 2.5m XYZ 距离，成功后 `registry.entries.remove(&instance_id)`，没有比较 `entry.dimension`。
- `server/src/inventory/tsy_loot_spawn.rs:119-127`、`server/src/inventory/tsy_loot_spawn.rs:259-269`：TSY 首次入场遗物写入同一个 `DroppedLootRegistry`，且 entry.dimension = `DimensionKind::Tsy`。
- `server/src/world/tsy_portal.rs:117-120`、`server/src/world/tsy_portal.rs:166-170`：TSY 进出只传送到各自维度的数值坐标；维度隔离依赖 `CurrentDimension`，不是坐标天然隔离。

## 触发路径

1. A 在 TSY 内丢弃物品、死亡掉落，或触发 TSY 首次入场遗物 spawn；server 写入 `DroppedLootRegistry.entries[id]`，`dimension = DimensionKind::Tsy`，`world_pos = [x, y, z]`。
2. `emit_changed_dropped_loot_syncs` 将全量 dropped loot 广播给所有在线 client；wire entry 不带 dimension。
3. B 在 Overworld 走到同数值坐标 `[x, y, z]` 的 2.5m 内；client 的 HUD/世界 billboard/G 键候选都只看 XYZ，能选中该 `instance_id`。
4. B 按 G 发送 `pickup_dropped_item { instance_id }`。
5. server 调 `pickup_dropped_loot_instance(inventory, registry, player_pos, instance_id)`；只要 XYZ 距离 <= 2.5m 就 attach 到 B 背包，并从全局 registry 移除该 TSY 掉落物。

## 反方审查记录

Round 1（反方 subagent，结论 REAL）：

- 尝试寻找隐藏维度/layer 门禁、registry 分区、instance_id 不可见性、已有重复计划，未找到。
- 反方确认 `DroppedLootRegistry` 全局、sync 全量广播、wire/schema 无 dimension、client store/候选无 dimension、server pickup 无 dimension。
- 反方指出 `plan-rat-v1` 只让灵蝗潮消费掉落物时按 `DroppedLootEntry.dimension` 过滤，不覆盖玩家通用 sync/pickup。

Round 2（反方 subagent，结论 REAL，补充边界）：

- 最强质疑：必须两个维度同 XYZ 2.5m 内才触发，TSY 出入口不会自动制造同坐标重叠，影响偏低。
- 驳回：条件苛刻但可达；协议把错维 entry 暴露给客户端，客户端可发 id，server 只按 XYZ 放行。玩家或掉落物只要在两个维度的同数值坐标附近重叠，就能跨维移除 registry 并把物品进错玩家背包。
- 去重：#984 是 dropped loot 断线/切服后短窗口 `DroppedItemStore` 残留，不覆盖同一 server 内跨维 dropped loot sync/pickup 的 server 权威校验。

## Skeleton Fix Plan

P0 server 权威止血：

- 修改 `pickup_dropped_loot_instance` 签名，传入 `player_dimension: DimensionKind`。
- 在取到 `entry` 后先比较 `entry.dimension == player_dimension`；跨维直接返回错误，不 attach、不 bump revision、不 remove registry。
- 更新 `handle_pickup_dropped_item` 调用，使用玩家 `CurrentDimension`，缺省策略必须显式且保守。
- 补同坐标跨维拒绝测试，保证 registry entry 留在原维度。

P1 同步与客户端可见性收口：

- 方案 A：`DroppedLootSync` 按 client 当前维度过滤，只下发同维 dropped loot；维度切换后主动重发当前维度快照。
- 方案 B：给 `DroppedLootEntryV1` / proto / TS schema / client store 增加 `dimension`，client 按当前 world dimension 过滤渲染与 G 键候选；server pickup gate 仍保留为最终权威。
- 若选择 B，必须重建 schema/proto 产物并覆盖 JSON/protobuf 两条路径。

P2 清理相邻风险：

- 检查 `InventoryEventHandler` 里直接写 `DroppedItemStore.putOrReplace` 的 dropped 事件是否也缺维度；不要让 inventory_event 绕过 `DroppedLootSync` 的维度收口。
- 评估 `BongNetworkHandler.clearClientStateOnDisconnect` 是否应补 `DroppedItemStore.clearOnDisconnect()`；这是 #984 的主题，不和本 plan 混修，但 fix 时避免互相打架。

## 验收测试计划

- `server/` 单测：`pickup_dropped_loot_instance` 同坐标同维成功、同坐标跨维拒绝、跨维拒绝不移除 registry、不修改背包 revision。
- `server/` handler 测试：`PickupDroppedItem` 请求读取玩家 `CurrentDimension`，TSY entry + Overworld player 同坐标时拒绝并 resync。
- `server/` sync 测试（若做按维过滤）：Overworld client join 只收到 Overworld drops；TSY client join 只收到 TSY drops；维度切换后收到目标维度快照。
- `agent/packages/schema` / `client` 测试（若 wire 增 dimension）：`DroppedLootEntryV1` 样例包含 dimension；proto bridge 保留 dimension；`DroppedLootSyncHandler` 解析并存储 dimension；`DroppedItemStore.nearestTo` 或上层候选过滤当前维度。
- E2E 手工：A 在 TSY 丢物于 `[x,y,z]`，B 在 Overworld 同 `[x,y,z]` 按 G 不得拾取；B 进入 TSY 后同坐标可正常拾取。

## 风险

- 只做 server pickup gate 可以阻止物品跨维进包，但玩家仍可能看到错维 marker/billboard，体感仍坏；因此 P1 可见性收口应跟进。
- 给 `DroppedLootEntryV1` 增加 dimension 会触碰 server schema、proto、agent schema、client handler/store，范围大于单点 server 修复；需要同步构建产物，避免 JSON/protobuf 漂移。
- 按维过滤 sync 需要处理维度切换时的主动重发，否则 client 可能短暂保留旧维度 dropped loot。
- #984 可能另行修断线残留；本 plan fix 时应避免重复改同一清理点导致冲突。
