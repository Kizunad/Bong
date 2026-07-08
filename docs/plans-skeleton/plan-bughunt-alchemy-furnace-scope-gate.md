# BugHunt: 炼丹炉交互缺少 server 距离/维度门禁

## Bug 摘要

`AlchemyFurnacePlace` 和后续 `AlchemyOpenFurnace` / `AlchemyIgnite` / `AlchemyFeedSlot` / `AlchemyTakeBack` / `AlchemyIntervention` 都以客户端提交的裸 `furnace_pos` 或 `x/y/z` 路由。server 只校验库存、坐标是否存在、owner 是否匹配，没有用玩家当前 `Position` / `CurrentDimension` 做权威距离和同维度门禁。

这不是 #973 的灵龛跨维问题；本 bug 限定在炼丹炉 gameplay 交互。

## 实际游玩体验影响

- 玩家可在离炉很远、传送后、甚至跨维后继续操作同一座炼丹炉。
- 普通客户端也可能触发：`AlchemyScreen` 构造时保存 `furnacePos`，投料、注气、点火、取回继续用这个旧坐标发包；`removed()` 只移除 listener，不做距离/维度失效。
- 放置路径可用合法库存里的炉物品，在任意 Overworld 坐标刷出 `BlockState::FURNACE` 和 `AlchemyFurnace` entity，物品会被扣除，但玩家不需要实际站在目标附近。
- 这会破坏“玩家必须靠近炉体操作”的世界交互体验，并让远程炼丹、远程取丹、跨维操炉成为可能。

## 证据定位

- `server/src/schema/client_request.rs:83`：炼丹炉操作 payload 只带 `furnace_pos`，不带维度。
- `server/src/network/client_request_handler.rs:276`：`AlchemyRequestParams` 没有玩家 `Position` / `CurrentDimension` query。
- `server/src/network/client_request_handler.rs:872`：`AlchemyFurnacePlace` 直接转为 `PlaceFurnaceRequest`。
- `server/src/alchemy/mod.rs:120`：`PlaceFurnaceRequest` 只有 `player` / `pos` / `item_instance_id`。
- `server/src/alchemy/mod.rs:432`：`handle_alchemy_furnace_place` 参数没有玩家位置或维度。
- `server/src/alchemy/mod.rs:445`、`server/src/alchemy/mod.rs:478`、`server/src/alchemy/mod.rs:488`：放置只查占位、消耗物品、spawn 炉体。
- `server/src/alchemy/mod.rs:490`：成功后直接写 `OverworldLayer`。
- `server/src/alchemy/furnace.rs:19`、`server/src/alchemy/furnace.rs:29`：`AlchemyFurnace` 只保存裸坐标 `pos`，没有维度。
- `server/src/network/client_request_handler.rs:11988`、`server/src/network/client_request_handler.rs:12000`：`with_owned_furnace_mut(_with_entity)` 中 `_player` 未使用，只按坐标和 owner 路由。
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:75`、`:85`：客户端 screen 长期持有 `furnacePos`。
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:735`、`:901`、`:909`、`:921`：投料、注气、点火、取回都继续用保存的 `furnacePos`。

对比已有正确门禁：

- `server/src/world/container_open.rs:85`、`:95`：世界容器打开会校验维度和距离。
- `server/src/craft/workbench.rs:116`、`:130`：手搓台打开会读取玩家位置并拒绝远距离。
- `server/src/network/client_request_handler.rs:9899`、`:9911`：NPC 交互会校验同维度和交互距离。

## 触发路径

1. 玩家右键近处炼丹炉打开 `AlchemyScreen`。
2. 客户端保存该炉 `furnacePos`。
3. 玩家保持 screen 打开并走远、被传送，或进入非 Overworld 维度。
4. 玩家在 screen 内投料、点火、注气、调温或取回。
5. server 只按 `furnace_pos` 和 owner 找炉，继续修改远处炉 session / 玩家库存 / 炉体状态。

放置路径：

1. 玩家背包内有合法 `furnace_fantie` 等炉物品。
2. 客户端发送任意 `alchemy_furnace_place` 坐标。
3. server 查到库存实例后立即消耗物品、spawn `AlchemyFurnace` 并写 Overworld 方块。
4. 没有玩家当前位置、维度、目标距离校验。

## 反方审查记录

Round 1：PASS。反方确认 C2S 入口、`PlaceFurnaceRequest`、放置 handler、`AlchemyFurnace` 和 `with_owned_furnace_mut` 都没有 server 侧空间门禁；owner 和库存校验只能防偷炉/凭空造炉，不能防远程或跨维操炉。

Round 2：PASS。反方进一步确认这不是单纯“改客户端才可触发”的安全硬化：普通客户端打开炼丹 UI 后会继续用旧 `furnacePos` 发投料、点火、取回和干预请求。反方建议 fix scope 不应缩小到放置，后续同一裸坐标路由也应统一收口。

重复性检查：开放 PR 中 #973 是灵龛跨维度放置；#870 丹道变异死数据、#880 voidaction 目标区锁 spawn、#886 alchemy HUD 零目标值、#974 丹方残卷学习链路断裂均不同题。`gh pr list --search "alchemy furnace distance OR 炼丹炉 距离 OR furnace dimension OR alchemy dimension OR 炼丹炉 维度"` 返回空。

## Skeleton Fix Plan

- [ ] 在炼丹炉交互层新增统一 scope helper，例如 `resolve_alchemy_furnace_in_scope(player, furnace_pos, ...)`。
- [ ] helper 读取玩家 `Position` 与 `CurrentDimension`；放置和操作均要求玩家在 Overworld，且玩家到目标炉/目标放置方块距离小于炼丹炉交互范围常量加容差。
- [ ] `handle_alchemy_furnace_place` 在任何库存消耗前先校验玩家维度和目标距离；远距/非 Overworld 拒绝必须不扣物品、不 spawn 炉、不写方块。
- [ ] `with_owned_furnace_mut(_with_entity)` 或其调用层统一接入 scope helper，覆盖 `open` / `ignite` / `feed_slot` / `take_back` / `intervention`。
- [ ] 保留现有 owner 独享语义：距离/维度通过后仍必须校验 owner；owner 不匹配仍走原有 Forbidden。
- [ ] 错误反馈对齐现有炼丹 error channel：远距提示“离炼丹炉太远”，跨维提示“此处感应不到炼丹炉”或同类文案。

## 验收测试计划

- [ ] server 单测：近距离 Overworld 放置成功，库存扣 1，spawn 炉体，写方块。
- [ ] server 单测：远距离放置拒绝，库存不变，炉体数量不变，方块不写。
- [ ] server 单测：TSY / 非 Overworld 放置拒绝，库存不变。
- [ ] server 单测：近距离 owner 正确的 open / ignite / feed / intervention / take_back 正常通过。
- [ ] server 单测：开屏后玩家移动到范围外，再发 feed / ignite / take_back / intervention，被拒绝且库存、session、炉体状态不变。
- [ ] server 单测：跨维后操作旧炉被拒绝。
- [ ] server 单测：owner 不匹配仍拒绝，并验证拒绝原因优先级不会泄漏远处他人炉状态。
- [ ] 回归：多炉并行、两个玩家各自操作自己的近处炉不互相影响。
- [ ] 回归：POI / 遗迹炉若已有真实 `AlchemyFurnace` entity 且玩家同维近距，仍可打开；没有 entity 的炉继续按“炉不存在”处理。

## 风险

- 如果炼丹炉未来支持公共炉或 co-owner，scope helper 必须与权限扩展解耦，不能把 owner 独享硬编码进距离 helper。
- `AlchemyFurnace` 当前没有维度字段；短期可要求炼丹炉只存在 Overworld，长期如支持 TSY 炉，需要给炉体持久化维度并迁移坐标路由 key。
- 修复顺序必须先 gate 后消耗库存，否则远距放置仍可能吞炉物品。
- 距离常量需要给客户端右键命中留容差，避免合法贴边放置/开炉被误拒。
