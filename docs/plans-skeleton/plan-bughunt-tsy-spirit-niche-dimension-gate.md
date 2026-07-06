# BugHunt: 坍缩渊可消耗基座写入无效灵龛锚点

## Bug 摘要

`server/src/social/mod.rs::handle_spirit_niche_place_requests` 没有在放置灵龛前拒绝 `CurrentDimension != Overworld` 的玩家。玩家在坍缩渊内通过库存菜单使用 `niche_base` 时，server 会消耗基座、写入 `SpiritNiche` / `Lifecycle.spawn_anchor` / SQLite，但灵龛方块和视觉都按主世界处理。

这违反 `worldview.md §十一 L928` 和 `worldview.md §十六.五 L1569`：灵龛不能设置在活坍缩渊内，坍缩渊内无安全点。

## 实际游玩体验影响

玩家把珍贵的一次性 `niche_base` 带入坍缩渊后，库存 UI 仍提供“放置灵龛”。点击后物品会被消耗，但当前维度看不到可用灵龛，归家检测也不会在坍缩渊生效；更糟的是角色会获得一个无维度裸坐标锚点，后续复活、可视化、灵龛保护可能按主世界同坐标解释，造成“道具没了、灵龛没出现、复活点污染”的体验断裂。

## 证据定位

- `client/src/main/java/com/bong/client/inventory/InspectScreen.java`：
  - `availablePillMenuActions` L2937-L2955：只要 `isSpiritNicheBase(item)` 就添加“放置灵龛”，没有维度 gate。
  - `dispatchPlaceSpiritNicheAt` L3036-L3054：只校验 item/template，发送 x/y/z/item_instance_id。
  - `targetPlacementPos` L3180-L3193：只从准星或玩家坐标取 `BlockPos`，不读取维度。
- `client/src/main/java/com/bong/client/network/ClientRequestProtocol.java` L703-L709：`spirit_niche_place` C2S payload 没有 dimension 字段。
- `server/assets/items/workbench_materials.toml` L795-L804：`niche_base` 是可制作基座，`spirit_quality_initial = 0.5`。
- `server/src/world/tsy_filter.rs` L49-L64：TSY 入场过滤只改 `spirit_quality` 和 `display_name`，不改 `template_id`，所以过滤后仍能被 server 识别为 `niche_base`.
- `server/src/social/mod.rs::handle_spirit_niche_place_requests`：
  - L1638-L1671：校验距离和 item template，但没有检查当前维度。
  - L1683-L1689：先 `consume_item_instance_once` 消耗基座。
  - L1704-L1709：读取 `CurrentDimension` 只用于 zone qi 查询。
  - L1726-L1739：写入 `SpiritNiche`、`spawn_anchor`、registry、component。
  - L1754-L1760：固定写 `OverworldLayer` 的 `LODESTONE`。
- `server/src/social/components.rs` L228-L239：`SpiritNiche` 只有 owner/pos/状态，没有 dimension。
- `server/src/social/mod.rs` L2668-L2670、L3024-L3038：`social_spirit_niches` 只持久化 pos_x/pos_y/pos_z，没有 dimension。
- `server/src/world/entity_model.rs::sync_spirit_niche_visuals` L340-L360：灵龛视觉固定挂到 `layers.overworld`。
- `server/src/player/home_return.rs` L44-L48、L96-L98：归家检测跳过非 Overworld 玩家。

## 触发路径

1. 玩家携带 `niche_base` 进入坍缩渊。
2. TSY 入场过滤把 `spirit_quality` 归零并改显示名，但保留 `template_id = "niche_base"`。
3. 玩家打开库存检查菜单，客户端仍展示“放置灵龛”。
4. 客户端发送 `spirit_niche_place { x, y, z, item_instance_id }`。
5. server 因缺少非主世界 early reject，消耗 `niche_base`，写入无维度灵龛状态和裸坐标 `spawn_anchor`。
6. 方块/视觉/归家逻辑按主世界处理，玩家当前坍缩渊内没有有效安全点。

## 反方审查记录

- 第一轮质疑：
  - 查找是否已有 server 保护：未发现 `CurrentDimension != Overworld` 拒绝，维度只参与 zone qi 查询。
  - 查找客户端是否禁止 TSY 菜单：未发现，菜单和发包都不带维度 gate。
  - 查找 `niche_base` 是否无法带入 TSY：不成立，TSY 过滤不改 template_id。
  - 查找开放 PR 覆盖：`gh pr list --search "spirit_niche OR 灵龛"` 仅见 #851、#945、#901；维度/主世界相关搜索无结果。
  - 初裁：倾向通过。
- 第二轮补证：
  - 补充 `workbench_materials.toml`、`tsy_filter.rs`、`worldview.md`、持久化 schema、视觉与归家固定主世界证据。
  - 让步：未新增测试，当前为源码路径静态复现。
  - 终裁：通过。反方认为这是缺少权威 server gate，不是应扩展成跨维灵龛。

## Skeleton Fix Plan

- [ ] 在 `handle_spirit_niche_place_requests` 中，在 `consume_item_instance_once` 之前读取 `CurrentDimension`，若不是 `DimensionKind::Overworld` 则拒绝并给玩家明确反馈。
- [ ] 保持客户端隐藏/提示仅作为 UX 增强；server gate 必须是最终权威。
- [ ] 不扩展 `SpiritNiche` / `spawn_anchor` / SQLite 为跨维模型；按 canon 固定“灵龛仅主世界”。
- [ ] 为 TSY 放置请求补回归测试：不消耗物品、不写 `spawn_anchor`、不插 `SpiritNiche` component、不更新 registry、不持久化、不写主世界 `LODESTONE`。
- [ ] 为主世界正常放置保留回归：仍消耗 `niche_base`、写锚点、写 registry/component/persistence、写主世界方块。
- [ ] 评估线上脏数据清理：已有无维度灵龛是否可能来自 TSY 坐标，需要单独迁移或人工清理说明。

## 验收测试计划

- `server/` 单测：构造 `CurrentDimension(DimensionKind::Tsy)` 玩家、带 `niche_base`、近距离发 `SpiritNichePlaceRequest`，断言请求被拒绝且所有状态无副作用。
- `server/` 单测：构造 Overworld 玩家走同一请求，断言既有放置行为不回归。
- `server/` 单测：验证拒绝发生在 `consume_item_instance_once` 之前，玩家 inventory 中 `niche_base` 仍存在且 revision 不变。
- 可选联调：进入坍缩渊后库存菜单可隐藏或显示不可用提示，但即使客户端误发 C2S，server 仍拒绝。

## 风险

- 如果线上已经存在 TSY 内触发留下的无维度灵龛，修复 gate 不会自动清理旧数据。
- 当前 `SpiritNiche`、`spawn_anchor`、SQLite 均无 dimension；本 plan 不应顺手扩 schema，否则会把一个门禁 bug 变成跨维灵龛设计改动。
- 修复点必须在物品消耗前；放在消耗后只会把“无效锚点”变成“吞道具但拒绝”。
