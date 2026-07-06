# plan-bughunt-forge-station-place-gate-v1

## §0 摘要

`ForgeStationPlace` 信任客户端坐标，服务端缺少同维、距离、可放置门禁。持有真实锻炉物品且 `station_tier` 匹配的异常客户端，可从远处或非 Overworld 维度发送 `forge_station_place`，让服务端在 Overworld 指定坐标生成 `WeaponForgeStation` 并写入 `ANVIL`。

本 plan 仅是 BugHunt Skeleton Plan，不包含实际修复。

## §1 实际游玩体验影响

- 玩家可被异常客户端远程落砧堵门、覆盖可替换方块或污染 spawn/公共区域附近地形，现场玩家看到的是 Overworld 某处凭空出现锻炉/铁砧。
- 跨维场景下，玩家人在 TSY 或其他维度时仍可能把锻炉落到 Overworld 坐标，破坏“玩家只能操作当前世界附近方块”的基本预期。
- 滥用不是免费刷方块：攻击者必须拥有真实锻炉物品，且请求里的 `station_tier` 必须与物品模板匹配；但服务端会消耗该物品并完成远程落砧。

## §2 复现路径

1. 玩家背包内准备一个真实锻炉物品，例如 `fan_iron_anvil`。它是工作台配方产物：`server/src/craft/workbench_recipes.rs:1263-1269`。
2. 玩家站在远离目标坐标的位置，或处于非 Overworld 维度。
3. 通过 `bong:client_request` 发送：

```json
{"type":"forge_station_place","v":1,"x":0,"y":64,"z":0,"item_instance_id":<真实实例>,"station_tier":1}
```

4. 现状预期：服务端只要确认该实例在背包且 tier 匹配，就消耗物品，在 Overworld 的 `(0,64,0)` 生成锻炉站点并写 `ANVIL`。
5. 修复后预期：远距、跨维、目标不可放置、目标与玩家碰撞等情况全部拒绝，且不消耗物品。

## §3 根因证据

- `server/src/schema/client_request.rs:645-653` 暴露 `ForgeStationPlace { x, y, z, item_instance_id, station_tier }`，坐标完全来自客户端。
- `server/src/network/client_request_handler.rs:2511-2530` 收到 `ForgeStationPlace` 后直接构造 `PlaceForgeStationRequest { player, pos, item_instance_id, station_tier }`，没有读取玩家 `Position` 或 `CurrentDimension`。
- `server/src/forge/station.rs:84-89` 的 request 结构只有 `player/pos/item_instance_id/station_tier`，没有维度、玩家位置或交互上下文。
- `server/src/forge/station.rs:92-99` 的处理系统查询 `Query<&mut ChunkLayer, With<OverworldLayer>>`，没有玩家位置/维度查询。
- `server/src/forge/station.rs:103-164` 只校验同 tick/既有锻炉占位、背包实例、模板 tier，然后 `commands.spawn(...)` 并 `set_block(req.pos, BlockState::ANVIL)`；没有距离、当前维度、chunk loaded、Y 范围、目标可替换、玩家碰撞门禁。
- 对比：`server/src/world/container_open.rs:35-96` 会读取玩家/容器 `Position + CurrentDimension` 并校验同维与 4.5 格；`server/src/craft/workbench.rs:115-137` 至少校验玩家位置与工作台范围。

## §4 非重复比对

- #981 是炼丹炉交互缺少服务端距离/维度门禁。
- #973 是坍缩渊灵龛放置缺少维度门禁。
- #1004 是制作台跨维同坐标误拆。
- `docs/plans-skeleton/plan-bughunt-forge-c2s-session-wiring-v1.md` 聚焦 forge 起炉、翻页、学习、步骤推进 C2S 断链，不覆盖 `ForgeStationPlace` 放置坐标门禁。
- `docs/finished_plans/plan-forge-leftovers-v1.md §1.3` 只要求 schema/handler/consume/spawn 与基础测试，未列距离/维度/可放置验收。

## §5 修复计划骨架

### P0 服务端权威门禁

- 扩 `PlaceForgeStationRequest` 或 `handle_place_station_request` 查询玩家 `Position + CurrentDimension`。
- 当前实现只写 `OverworldLayer`，先保守拒绝非 Overworld 玩家请求；未来若要多维锻炉，再给 station entity/schema 补维度字段并改用 `DimensionLayers`。
- 增加距离门禁，使用与 block/placeable 交互一致的服务端判定口径。
- 在消耗物品前校验目标 chunk loaded、Y 范围、目标可替换、玩家碰撞。
- 复用或抽取 `world::block_place::can_place_block`，避免继续裸 `set_block` 绕过普通放置规则。
- 失败路径全部不消耗物品，不生成站点，不写 Overworld 方块。

### P1 测试与反馈

- server 单测覆盖：合法近距放置仍消耗并生成站点；远距拒绝不消耗；非 Overworld 拒绝且不写 Overworld；目标实心/不可替换拒绝；玩家碰撞拒绝。
- bot e2e 黑盒补强：dev give 锻炉物品后发送远距 `forge_station_place` payload，断言拒绝反馈与连接保持。
- 若当前协议没有可观察拒绝信号，先补最小 chat 或 telemetry，再写 bot 断言。

## §6 验证计划

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- `bash scripts/bot-e2e.sh`，重点跑/新增 production forge 场景。
- 手工或 bot 复现矩阵：近距合法、远距非法、TSY/非 Overworld 非法、目标不可替换非法、玩家碰撞非法。

## §7 接入面与守恒说明

- 进料：`ClientRequestV1::ForgeStationPlace`、`PlayerInventory`、`ItemRegistry`、玩家 `Position/CurrentDimension`、Overworld/Dimension layer。
- 出料：`WeaponForgeStation` entity、Overworld 方块状态、库存 snapshot。
- 跨端契约：C2S payload 字段不必变；服务端新增权威拒绝语义即可。
- qi_physics：本问题不涉及真元/灵气转移，不新增 qi 常数或 ledger 流。

## §8 对抗复核结论

- 候选证据：服务端从 C2S 直接接收坐标，锻炉放置 handler 缺少玩家位置/维度/可放置校验，并写死 Overworld layer。
- 反方质疑：普通客户端 `targetPlacementPos()` 大概率只发近处坐标；攻击者必须持有真实锻炉物品且 tier 匹配，不能表述为任意无限刷方块；需确认不重复 forge C2S 断链题。
- 修正/反驳：正常客户端不是安全边界；表述收窄为“持有真实锻炉物品且 tier 匹配的异常客户端可远程/跨维落砧”；重复比对确认既有 #981/#973/#1004 与 forge session wiring 均不覆盖 `ForgeStationPlace` 放置门禁。
- 反方最终裁决：通过。候选高置信、非重复，适合开 Skeleton Plan PR；修复需明确目标 chunk loaded、Y 范围、目标可替换、玩家碰撞、失败不消耗物品，并先拒绝非 Overworld。
