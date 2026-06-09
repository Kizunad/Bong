# Bong · plan-block-lifecycle-v1 · Active

> **状态**：⏳ active（2026-06-09 升级，user 拍板）。博弈流程设计 + 实地核查 + pre-P0 收口（§11.1/§11.2）三步完成,blocking 地基已通,可进 P0。前置无依赖（接现有 inventory/custom-block/hotbar 已落地系统）。

> **一句话主题**：把原版方块「破坏获取 → 入 Bong 自有背包 → 选中持握 → 朝向放置 → 第一人称切手」全流程迁移到 Bong 自有系统（block_drop 掉落表 + CustomPayload 背包 + BlockPlace 放置 payload + vanilla HeldItemRenderer fake-stack 切手），全程零接触原版 hotbar held-slot、零消费 valence InteractBlockEvent。

## 阶段总览

| 阶段 | 状态 | 一句话 | 验收日期 |
|------|------|--------|----------|
| pre-P0 | ✅ | **持握态 SSOT + 选中入口 + 命名**（blocking 决策门）：走下层 1-9 SkillBar `Kind.ITEM` + `SkillBarStore.selectedSlot` 静态字段 + `InspectScreen` 绑槽 UI + 6 方块正典名（§11.1 + §11.2 已收口） | 2026-06-09 |
| P0 | ⬜ | 获取：扩 `block_drop_for` 软方块（DIRT/SAND/GRAVEL）手破即掉 + `BlockDropEntry.required_tool` 工具门控 + drop count 锁 1:1 防 dupe + 改 pin 测试 | YYYY-MM-DD |
| P1 | ⬜ | 入包：`ItemCategory::Block` + 登 TOML（materials.toml `[[item]]`）+ 复用 `add_item_to_player_inventory`；**不加 `ItemInstance.block_kind` 字段**（template_id 即身份锚） | YYYY-MM-DD |
| P2 | ⬜ | 放置协议三端契约：`ClientRequestV1::BlockPlace`（server serde + agent TypeBox 塞进 Union + samples 对拍 + dist 重建） | YYYY-MM-DD |
| P3 | ⬜ | server 落块 consumer：`place_block_for_kind` 单一分叉函数 + **最小 canPlace 校验**（replaceable/碰撞/Y/chunk-loaded）+ 扣减 + S2C | YYYY-MM-DD |
| P4 | ⬜ | client 放置 wiring + 第一人称切手（`BlockVanillaIconMap` + 两 held mixin EMPTY 分支 + `swingHand`）+ icon PNG | YYYY-MM-DD |
| P5 | ⬜ | 端到端 e2e + 扩展点收口（`place_block_for_kind` 文档化未来 bong 陷阱方块接法）+ smoke-test | YYYY-MM-DD |

**世界观锚点**：无直接修仙正典锚点，本 plan 定位为**纯沙盒环境交互基建**（仿 plan-custom-block-v1 的定位）。方块物品命名走正典字符串（残灰土 / 碎石 / 粗木之类，而非裸 `dirt`/`cobblestone`），materials.toml 全部物品命名惯例不允许裸 vanilla id 漂移——命名拍板见 §11 开放问题。放置打开后的多人 griefing 治理（出生点/灵龛/阵眼区堆方块封门）是已知缺口，最小 spawn-zone 保护 + reach 校验范围待定，见 §11。

**交叉引用**：
- `docs/finished_plans/plan-custom-block-v1.md` —— bong_blocks.json 单一事实源 + codegen + `place_bong_block` + client `BongBlocks.java` 注册范式（**本 plan 凡俗方块一律绕开 `place_bong_block` 走裸 `ChunkLayer::set_block`**，仅未来 bong 陷阱方块走 custom-block 路线）
- `docs/finished_plans/plan-inventory-v1.md` / `plan-inventory-v2.md` —— Bong 自有背包（CustomPayload，非 vanilla inventory）+ ItemInstance/ItemTemplate/ItemCategory + 满包静默丢 UX 开放问题
- `docs/finished_plans/plan-hotbar-modify-v1.md` / `plan-hotbar-modify-v2.md` —— SkillBar 1-9 战斗栏 + SkillBarKeyRouter + Kind.ITEM 空枝（本 plan 在此承载方块选中态）
- `docs/finished_plans/plan-item-visual-v1.md` —— `/gen-image item` 批量出 PNG icon 落 `textures/gui/items/{id}.png` 约定路径，server 零参与图标

---

## §0 接入面 Checklist

| 接入面 | 命中 |
|--------|------|
| **进料**（本 plan 消费的上游） | valence `DiggingEvent`（破坏检测，`block_break.rs:20`，已稳定不碰）；玩家持握态（§11.1 锁定：下层 1-9 SkillBar `Kind.ITEM` 选中槽 `selectedSlot`，**非** `equipped(MAIN_HAND)`）；玩家 `PlayerInventory`（持有校验 / 扣减） |
| **出料**（本 plan 产出的下游） | 背包：`add_item_to_player_inventory` 入包 + `send_inventory_snapshot_to_client`；世界：`ChunkLayer::set_block` 落块 → valence 帧末自动广播 `BlockUpdateS2c`/`ChunkDeltaUpdateS2c`（S2C 零工作量）；client：FP fake-stack 渲染 + `swingHand` 挥手 |
| **共享类型 / event** | server Bevy event `BlockPlaceRequest`；放置分叉函数 `place_block_for_kind(template_id, target_face)`；掉落门控字段 `BlockDropEntry.required_tool: Option<GatheringToolKind>` |
| **跨仓库契约 · server** | `ClientRequestV1::BlockPlace`（`schema/client_request.rs`）；`ItemCategory::Block`（`inventory/mod.rs:225`）；`BlockPlaceRequest` event；`block_place_tx`（`network/client_request_handler.rs`，仿 `zhenfa_place_tx`：字段声明 `:286` / `.send()` dispatch `:1316`） |
| **跨仓库契约 · client** | `sendBlockPlace` / `encodeBlockPlace`（`ClientRequestProtocol`/`ClientRequestSender`）；`BlockVanillaIconMap`；SkillBar `selectedSlot`（新增指针）+ 复用 `SkillBarEntry.Kind.ITEM`（**不新增 BLOCK kind**）+ `sendSkillBarBindItem` 接 UI（§11.1 #1）；`player.swingHand(Hand.MAIN_HAND)`（全仓 0 处需新增） |
| **跨仓库契约 · schema** | `agent/packages/schema/src/client-request.ts` `BlockPlaceRequestV1` Type.Object **显式塞进 `ClientRequestV1` Type.Union(:982)** + `samples/*.json` 正反对拍 + `npm run build -w @bong/schema` 重建 dist。**透传面 = 1 面**（仅 BlockPlace payload）：因不加 `ItemInstance.block_kind`、template_id 即身份锚，inventory schema/view/handler/client model 四面零联动（消解三方都点出的「block_kind client 虚透传」红旗） |
| **worldview 锚点** | 纯沙盒基建，无直接正典锚（仿 custom-block-v1）；方块物品命名走正典中文字符串（待 §11 拍板），不允许裸 vanilla id 漂移 |
| **qi_physics 锚点** | **不涉及**（dev 路径与 gameplay 守恒账本无关；破坏/放置不动 qi ledger） |

---

## §P0 获取链路：扩掉落表 + 工具门控 + 防 dupe

**模块**：`server/src/world/block_drop.rs`（**不碰** `block_break.rs`——抹 AIR 判定 `apply_default_block_break:32` 已稳定）。

**实测前提（已核验，纠正三方案分歧）**：
- `block_drop_for(block_drop.rs:38)` 当前 STONE/COBBLESTONE/ANDESITE/DIORITE/GRANITE **已掉 `stone_chunk`**（`block_drop.rs:48-56`，实测）——保真案 P0 称其为 `None` 是硬事实错误，**本 plan 弃之，不改其现有掉落经济**。
- 当前落 `_ => None`(`block_drop.rs:67`) 的软方块 = DIRT/SAND/GRAVEL 等，这些才是 P0 要加 `BlockDropEntry` 的对象。
- pin 测试 = `air_and_unrecognized_blocks_have_no_drop`(`block_drop.rs:278`)，含 `DIRT`(:280)/`BEDROCK`(:281)/`SAND`(:282) **三条断言**，新增 DIRT/SAND 掉落必须同步改这条 pin（不止一条）。

**交付物**：
1. `block_drop_for`(`block_drop.rs:38`) 加 match arm：`BlockState::DIRT`/`COARSE_DIRT`/`SAND`/`GRAVEL` 等当前落 `_ => None` 的软方块 → `Some(BlockDropEntry{ template_id: <正典 id>, min_count: 1, max_count: 1, required_tool: None })`（手破即掉）。**COBBLESTONE/STONE 系不动**（已掉 stone_chunk）。
2. `BlockDropEntry`(`block_drop.rs:32`) 加字段 `required_tool: Option<GatheringToolKind>`（手破方块=None；若给某软方块设工具门控则 `Some(...)`）。
3. `apply_block_drops`(`block_drop.rs:104`) 命中 entry 后、`roll_count` 前插工具门控：复用 `equipped_gathering_tool`(`gathering/tools.rs:222`，只查 equipped 主手/双手、破损不算、不回退 hotbar；比 mineral `pickaxe_tier` 通用)，不满足 `required_tool` 则 skip 掉落但方块照样被 `apply_default_block_break` 抹 AIR（与 vanilla「工具决定掉不掉、不决定破不破」一致）。**单 `get_mut` 同时读工具 + 写掉落**（避免 Bevy 同组件 `&`+`&mut` 双 Query panic）。
4. **drop count 锁 1:1 防 dupe**（硬约束）：可放置方块的 `min_count=max_count=1`，**不可照 `crude_wood` 抄 1-2 区间**，否则「破坏掉 1-2 个 + 放置只消耗 1」形成净增殖刷物。
5. 退让顺序保持：`MineralOreIndex.lookup → spirit niche → spiritwood` 三道 skip 在前，新方块判定插在其后、`block_drop_for` 命中之后（`block_drop.rs:134` 范式）。
6. 放置/掉落 layer 用 `dimension_layers.entity_for(dimension)`(`apply_block_drops` 已有 `:152-154`)，**不用** zhenfa 的 `With<OverworldLayer>`（否则 tsy 维度静默失败）。

**测试声明**：`block_drop::*` 加：① happy（DIRT 手破掉 1 个对应物品进背包）② drop count 锁 1:1（破坏只产 1，断言 `max_count==1`）③ 工具门控三分支（手破方块无工具掉 / 设了门控的方块缺工具不掉但方块 AIR / 持对应工具掉）④ Creative 模式不掉物（**注意两个不同函数**：破坏抹 AIR 走 `should_apply_default_break`，条件是 `(Start,Creative) | (Stop,Survival)`——Creative+Start **也抹方块**；但掉落走 `apply_block_drops` 内 `should_drop` 仅 `(Stop,Survival)` → Creative 删块无掉落、Survival 须 Stop 才掉。plan 早期把 `should_apply_default_break` 误写成 `should_drop`、漏了 Creative+Start 抹块臂，此处纠正）⑤ **改 `air_and_unrecognized_blocks_have_no_drop`(:278)**：DIRT/SAND 改为 `is_some()`，BEDROCK 保留 `is_none()`。`cargo test world::block_drop` + `cargo clippy --all-targets -- -D warnings`。

**抓手 grep**：`required_tool` / `fn block_drop_for` / `BlockState::DIRT =>` / pin 测试名 `air_and_unrecognized_blocks_have_no_drop`。

---

## §P1 入包表示：ItemCategory::Block + 登 TOML

**模块**：`server/src/inventory/mod.rs` + `server/assets/items/materials.toml`。

**关键收敛（嫁接保真案路 B，弃复用案默认手搓 + 弃 block_kind 字段）**：走「登 TOML」路线（固定 vanilla 方块集合、登记量可控），**不新增 `ItemInstance.block_kind` 字段**——`template_id` 本身即身份锚，放置时 server 端 `(template_id, target_face) → BlockState` 映射还原方块。此收敛把 block_kind 跨仓库透传面从 5 面降到 0 面（仅 BlockPlace payload 一面），直接消解三方都点出的「block_kind client 虚透传」红旗（client `InventoryItem` 实测无 `mineral_id`，证明该透传路是虚的）。

**交付物**：
1. `ItemCategory::Block`(`inventory/mod.rs:225` enum 新变体) + `parse_item_category`(`mod.rs:1950`) 加 `"block" => Ok(Block)` + `default_max_stack_count_for_category`(`mod.rs:1588`) 给 Block 返 64（「轻 + 可堆叠」成默认）。
2. `server/assets/items/materials.toml` 加 `[[item]]` 块：每个可放置方块一条（`id`=正典字符串 id / `name`=正典中文名（见 §11）/ `category="block"` / `grid_w=1` / `grid_h=1` / `base_weight`=小值 / `rarity` / `spirit_quality_initial` / `description`；`max_stack_count` 可省→按 category 默认 64）。注意 `ItemTemplatesToml`/`ItemTemplateToml` 用 `#[serde(deny_unknown_fields)]`(`mod.rs:1410/1416`)，写错字段名启动 panic。
3. 扫遍所有 `match category` 处确认 `Block` 变体被覆盖（尤其 `mod.rs:3777` 装备规则——`ItemCategory::Block` **不允许装入装备槽 MAIN_HAND**（校验保持不动），持握态走 client 端 SkillBar `selectedSlot`（§11.1），不经 server 装备槽；漏一处 match 编译报错兜底，但语义规则逐个确认）。
4. **`ItemInstance` 不动**（不加 block_kind），因此 `stack_identity_matches`/`runtime_instance_from_template`/`footprint_probe` 等构造点零改动——`template_id` 已是首个比对项，不同方块天然不混栈。
5. 入包路径：简单方块走通用 `add_item_to_player_inventory`(`mod.rs:1081`，已含 merge→find_free_slot→满包 Err)，靠 ItemRegistry 模板的 grid/weight/rarity 快照。P0 的 `apply_block_drops` 末尾 `add_item_to_player_inventory(..., entry.template_id, count, now_tick)`(`block_drop.rs:180`) 直接命中（template_id 必须在 ItemRegistry 有模板，否则 Err 静默丢——本 plan 登 TOML 保证有模板）。

**测试声明**：`inventory::*` 加：① `ItemCategory::Block` 序反 sample 对拍 ② `parse_item_category("block")` happy ③ `default_max_stack_count_for_category(Block)==64` ④ materials.toml 含新 `[[item]]` 块时 `load_item_registry` 不 panic ⑤ 同 template_id 方块可堆叠合并 / 不同 template_id 方块不混栈（复用 `stack_identity_matches` 现有断言）。`cargo test inventory` + `cargo clippy`.

**抓手 grep**：`ItemCategory::Block` / `default_max_stack_count_for_category` / `category = "block"`（materials.toml）。

---

## §P2 放置协议三端契约：BlockPlace payload

**模块**：`server/src/schema/client_request.rs` + `agent/packages/schema/src/client-request.ts` + `agent/packages/schema/samples/*.json` + `client/.../ClientRequestProtocol.java` + `client/.../ClientRequestSender.java`。

**绝不消费 `InteractBlockEvent`**（valence 转发的纯原版右键包、Bong 全仓 0 消费者、纯转发无校验，接它=从零造放置/反作弊/序列号；它是与 Bong 放置模式正交的死代码，不是待接入口）。照抄 `ZhenfaPlace`(`client_request.rs:248`) / `CoffinPlace`(`:144`) / `ForgeStationPlace`(`:566`) 的 `{x,y,z,item_instance_id}` 模板。

**交付物**：
1. server `ClientRequestV1::BlockPlace { v, x, y, z, item_instance_id, target_face }`(`client_request.rs`，照 `ZhenfaPlace:248`)。`target_face` 复用 `TrapTargetFace`(从 `trap_content` import，`client_request.rs:20`)——**但若实现期发现 `TrapTargetFace` 跨模块借语义过重，提一个中性 `BlockFace` / valence `Direction`**（registry+保真两案的 cross-repo 都标了这层耦合）。含 serde 正反 pin 测试（仿 `:1683` ZhenfaPlace 解析测试）。
2. agent `client-request.ts` 加 `BlockPlaceRequestV1` Type.Object 并**显式塞进 `ClientRequestV1` Type.Union(`:982`)**（cross-repo 案点出的最易漏面——加了 Type.Object 不塞 Union 等于没加）+ `samples/*.json` 正反对拍 + `npm test`（schema）+ **`npm run build -w @bong/schema` 重建 dist**（否则 agent 启动 ESM export not found 崩，见 memory）。
3. client `ClientRequestProtocol.encodeBlockPlace(BlockPos placePos, long itemInstanceId, targetFace)`（新增方法；照抄 `encodeForgeStationPlace`（**真实行 `ClientRequestProtocol.java:645`，非 :1199**——:1199 是无关私有 `envelope()` helper）的 `{type,v,x,y,z,item_instance_id,…}` envelope，注意每种 Place 有专属附加字段：forge 带 `station_tier`、block 加 `target_face`) + `ClientRequestSender.sendBlockPlace`(`ClientRequestSender.java:499` dispatch，`bong:client_request` 单通道 force-send)。

**测试声明**：server `schema::client_request::*` BlockPlace serde 正反；agent schema `npm test` 三端对拍（samples 双端校验）；client `gradlew test` 覆盖 `encodeBlockPlace` envelope 结构。跨仓库 symbol：`ClientRequestV1::BlockPlace`(server) / `BlockPlaceRequestV1`(agent，在 Union 内) / `block_place` type(JSON) 三端命中。

**抓手 grep**：`BlockPlace`（三仓）/ `block_place`（JSON）/ `BlockPlaceRequestV1`（agent）。

---

## §P3 server 落块 consumer + canPlace 校验 + 扣减 + S2C

**模块**：`server/src/network/client_request_handler.rs`（dispatch）+ `server/src/world/block_place.rs`（新模块，consumer + 分叉函数）。

**交付物**：
1. `client_request_handler.rs` 加 `block_place_tx`（仿 `zhenfa_place_tx`：`ClientRequestDispatchParams` **字段声明在 `:286`**、真正的 `.send(ZhenfaPlaceRequest{…})` **dispatch 在 `:1316`**，含 `v` 版本校验 arm），反序列化 `ClientRequestV1::BlockPlace` → emit `BlockPlaceRequest` event。
2. `BlockPlaceRequest` Bevy event `{ client, x, y, z, item_instance_id, target_face }`（仿 `ZhenfaPlaceRequest`）。
3. consumer `handle_block_place_requests`（仿 `handle_zhenfa_place_requests`，**真实位置 `server/src/zhenfa/mod.rs:893`**——非 client_request_handler.rs；新 consumer 落独立模块 `world/block_place.rs`，范式参考 zhenfa/mod.rs）：
   - 校验持有：`inventory_item_by_instance`(`mod.rs:2690`) 查 item_instance_id → 取 template_id（必须 `ItemCategory::Block`）。
   - **最小 canPlace 校验（硬交付物，不是 polish）**：目标格当前 BlockState 必须是 AIR / replaceable（草/雪等）才放；不放进玩家碰撞箱（放置坐标不与任一玩家碰撞箱相交）；Y 在维度边界内；chunk loaded（`place_bong_block` 已含 chunk-loaded，裸 `set_block` 必须自查）。任一不满足 → 拒绝（不扣物、可选 client 反馈）。**理由**：凡俗方块绕开 `place_bong_block` 走裸 `ChunkLayer::set_block`（盲写覆盖），比 vanilla 还宽松（vanilla 有 canPlace），必须补这点底线，否则可覆盖任意方块/把方块放进自己脚下卡住。
   - 落块：调 `place_block_for_kind(template_id, target_face)`【**嫁接 registry 案的单一分叉函数，但弃整套 registry/Spec/behavior enum**】——内部判定：①凡俗方块 → `(template_id, target_face) → vanilla BlockState`（`block_item_to_state`，与 `block_drop_for` 互逆）→ 裸 `ChunkLayer::set_block`（**不经 `place_bong_block`**，否则 `is_bong_block` 拒绝 vanilla state 返 `Err(NonBongBlock)`，`bong_blocks.rs:25` 实测 + `place_rejects_vanilla_block` pin 佐证）；②未来 bong 陷阱方块 → `place_bong_block`。这是唯一对外扩展挂点，未来接入只加一条映射。
   - 扣减：`consume_item_instance_once`(`mod.rs:2725`) 扣 1（归零删 PlacedItemState）+ `send_inventory_snapshot_to_client`。
   - layer：用 `dimension_layers.entity_for(dimension)`（`block_drop.rs:152` 范式）而非 zhenfa 的 `With<OverworldLayer>`，否则 tsy 维度放置静默失败。
4. **S2C 零工作量**：`ChunkLayer::set_block` 让 valence 帧末自动给所有 viewer 广播 `BlockUpdateS2c`/`ChunkDeltaUpdateS2c`（loaded chunk 标 changed section）。vanilla 方块 client 天然认识，无需 `BongBlocks.java` 注册。
5. `block_item_to_state` 签名预留 target_face 入参：`(template_id, target_face) → BlockState`（**函数而非静态查表**），P0 只做单一 template→单一 state 的凡俗方块，但签名留 target_face 给未来带 property 方块（原木 axis、陷阱 armed/facing）。

**测试声明**：`world::block_place::*`：① 放置成功（方块写入 + inventory 扣 1 + revision bump + snapshot 发出）② 持有校验失败（instance 不存在 / 非 Block category）拒绝 ③ canPlace 拒绝（目标格非 AIR 不可替换 / 放进玩家碰撞箱 / Y 越界 / chunk 未加载）④ **分叉 pin**：vanilla 方块走 `set_block` 成功、（mock）bong 方块走 `place_bong_block` 路径 ⑤ 多人/跨维度 layer 选对。`cargo test world::block_place`.

**抓手 grep**：`place_block_for_kind` / `handle_block_place_requests` / `block_place_tx` / `block_item_to_state`.

---

## §P4 client 放置 wiring + 第一人称切手

**模块**：`client/.../mixin/MixinClientPlayerInteractionManagerAlchemy.java` + `client/.../block/BlockVanillaIconMap.java`(新) + `client/.../mixin/MixinPlayerEntityHeldItem.java` + `client/.../mixin/MixinHeldItemRenderer.java` + `client/.../combat/SkillBarKeyRouter.java`(改 ITEM 分支) + SkillBar `selectedSlot`(§11.1) + icon PNG。

**视听规格 = 复用 vanilla HeldItemRenderer fake-stack，拒绝 PlayerAnimator**。理由（memory PlayerAnimator 四大坑 + 实测注释 2026-04-14）：`BongAnimationPlayer` 默认 `FirstPersonMode.THIRD_PERSON_MODEL`，FP 持物时被 vanilla item 渲染路径盖掉；38 条 `player_animation/*.json` 是第三人称招式，套 FP 切手失效；四大坑（循环单帧衰减到 defaultValue / 无 IK 致 `leg.pitch` 断腿 / body 走 MatrixStack / bend 需 bendy-lib 独立 mod 否则静默 no-op）。FP 切手一律 vanilla 渲染管线。

**交付物（视听规格内联到引用级精度——嫁接保真案精度）**：
1. **`BlockVanillaIconMap`**（新，照抄 `WeaponVanillaIconMap.createStackFor:30` 的 template_id→vanilla fake ItemStack **缓存单例**模式）：方块 template_id → `new ItemStack(对应 vanilla BlockItem)`（残灰土→`Items.DIRT`、碎石→`Items.COBBLESTONE` 等；未来 bong 方块→`BongBlocks` 注册的 `BlockItem.asItem()`）。
2. **FP 注入点**：`MixinPlayerEntityHeldItem.getMainHandStack @RETURN`(注入方法在 `MixinPlayerEntityHeldItem.java:43`，:37 是类声明) 的 EMPTY 分支 + `MixinHeldItemRenderer.bong$overrideHeldItemsForBongWeapons TAIL`(`MixinHeldItemRenderer.java:49`) 的 mainHand 改写处，补「主手是方块物品 → 塞 fake BlockItem stack」分支。**读取源必须是 §11.1 锁定的 SkillBar `selectedSlot`，不是 `WeaponEquippedStore`/`equipped()`**（见 §11 第 1 条；微端 `getMainHandStack()` 恒 EMPTY，gameplay 不读 fake stack）。
3. **equip dip 动画 = vanilla 内置，零自写**：vanilla `HeldItemRenderer.updateHeldItems` 对 mainHand 字段 item identity 变化自带 `equipProgress 0→1 lerp`（`MathHelper.lerp`，约 ~6 tick，下沉至屏幕底再抬起到持握位）——**identity 变即触发，Bong 不调参**，靠 fake stack 切换驱动。vanilla 持 BlockItem 时 `HeldItemRenderer` 天然渲染「方块在手心」FP 姿态（BLOCK transform），无需自写持块手势。
4. **放置挥手 = `player.swingHand(Hand.MAIN_HAND)`**（vanilla 6-tick `handSwingProgress` 单次，**全仓 0 处需新增**）：放置成功（收到 server inventory snapshot 确认扣减，或乐观本地）时调用。
5. **放置 wiring**：`MixinClientPlayerInteractionManagerAlchemy.bong$alchemyInteractBlock`(`:69`) HEAD 链加分支（放在 coffin/furnace/zhenfa 之后）：主手是方块物品 && instanceId>0 → `placePos = hit.getBlockPos().offset(hit.getSide())`（vanilla「贴着被点击面放」语义）→ `sendBlockPlace(placePos, instanceId, hit.getSide())` → `cir.setReturnValue(SUCCESS)` 吃掉原版包。**必须 setReturnValue** 否则原版包 + `MineralProbe` mixin（`:33` 不 cancel 放行）重复触发；mixin 顺序由 `bong-client.mixins.json` 列序 + priority 决定，确认放置判定不被 probe 空手分支抢先。
6. **icon = 纯 PNG（方案 A，零渲染逻辑改动）**：为每个方块物品出 `client/src/main/resources/assets/bong-client/textures/gui/items/{template_id}.png`（`/gen-image item` 或等轴裁切方块贴图），`ItemIconRegistry.textureIdForItemId`(`:75`) 自动命中，`GridSlotComponent.drawItemTexture`(`:147`) 与 HUD 共用。**拒绝方案 B**（改 GridSlotComponent 加 BlockRenderManager 分支——全仓无先例、HUD+背包共解析改一处要保两处过、成本高）。

**测试声明**：client `gradlew test`：`BlockVanillaIconMap.createStackFor` 返正确 BlockItem + 缓存命中；`SkillBarKeyRouter` BLOCK 分支返 `ci.cancel`；`encodeBlockPlace` envelope 结构。手测清单（runClient）：1-9 选方块槽 → FP 出现方块持握 + equip dip → 右键方块 → server 落块 → S2C 回显 → `swingHand` 挥手 → 背包减 1 + 背包格显方块 PNG。

**抓手 grep**：`BlockVanillaIconMap` / `sendBlockPlace` / `swingHand` / `selectedSlot`（SkillBar 持握态读取源，§11.1）。

---

## §P5 端到端 e2e + 扩展点收口

**交付物**：
1. **e2e 用例**：client 1-9 选中方块槽 → 右键方块 → 发 `block_place` CustomPayload → server canPlace 校验 + `set_block` + 扣 inventory → S2C 方块出现 + inventory snapshot 反映 → client 渲染方块 + 背包减 1，全链路一条 e2e。**破坏闭环**：破坏 DIRT → 入背包 → 放回去 → 再破坏，验证 1:1 守恒（防 dupe 端到端兜底）。
2. **icon 资产**：`textures/gui/items/{block}.png` 全量出图（`/gen-image item`），程序化扫透明度防假透明（memory gen-image 透明随机失败）。
3. **扩展点文档化**：`place_block_for_kind` 单一分叉函数的未来 bong 陷阱方块接法——①`bong_blocks.json` 追加方块（boolean prop 顺序严格 `"true","false"` 对齐 MC BooleanProperty，否则 raw state 错位）→ cargo build codegen `BlockState::BONG_*`（ID 从 24141 接续）+ client `gradlew build` 的 `generateBongBlockIds` 任务（`client/build.gradle:114`，fail-fast 对齐 raw ID）；②`block_item_to_state` 加一条映射返 bong BlockState → 走 `place_bong_block` 分支；③「被踩触发」行为自建（zhenfa proximity 范式 `handle_zhenfa_trigger_requests`，**真实位置 `server/src/zhenfa/mod.rs:1169`**——非 client_request_handler.rs，无引擎级 on-step hook）+ per-block 状态存 server-side registry resource（valence 无 block-entity NBT，等 plan-persistence-v1）。**本 plan 仅留挂点不实装陷阱行为**。
4. `scripts/smoke-test.sh` 通过。

**抓手 grep**：e2e 测试文件名 / `place_block_for_kind`（扩展点注释）。

---

## §11 开放问题（P0 决策门前需收口）

> 每条可独立 Explore 核查；标 **【blocking】** 的必须在 pre-P0 / P0 前收口，否则整条链路触发条件恒假或走不到。

1. **【blocking · ✅ 方向已定 2026-06-09 → §11.1 #1，细节落点待 §10.0】持握态 single-source-of-truth + 放置 wiring 地基**：现有 5 个 `*Place` 先例（coffin/furnace/zhenfa 等）在 `MixinClientPlayerInteractionManagerAlchemy`(`:88`) 读 `InventoryStateStore.snapshot().equipped().get(MAIN_HAND)`（**client 端缓存的 server-pushed 装备快照，非直连 server**；MainHand 还有第二层 two_hand 互斥校验在 `inventory/mod.rs:3786-3793`），但 server `inventory/mod.rs:3777` 硬性拒绝非 weapon/tool/hoe 装入 MAIN_HAND，`ItemCategory::Block` 永进不去 → 照抄先例则放置链触发条件恒假（spawn-chain-wiring 红旗翻版）。**用户拍板**：走**下层 1-9 SkillBar**（复用 `Kind.ITEM` 空枝 + 补 selectedSlot 指针，**不新建平行 store、不放宽 MainHand 校验**），详 §11.1 #1。
2. **【blocking 前置 · ✅ 方向已定 2026-06-09 → §11.1 #1，细节落点待 §10.0】选中方块进入待放态的 UI/协议入口**：`sendSkillBarBindItem` 已定义但全仓零 UI 调用（只有 `sendSkillBarBindSkill` 在 `TechniquesTabPanel:215` 真用），`SkillBarConfig` 无 `selectedIndex`。**用户拍板**：复用 `sendSkillBarBindItem`(`ClientRequestSender.java:286`，已有协议 `encodeSkillBarBindItem:972`，仅缺 UI 触发) + 新增 selectedSlot 指针，详 §11.1 #1。
3. **【P3 交付物 · 不能推后】放置位置最小 canPlace 校验**：见 §P3——`ChunkLayer::set_block` 盲写覆盖，凡俗方块绕开 `place_bong_block` 后连 replaceable/碰撞/目标格占用/Y 边界/chunk-loaded 都没了。「目标格非 AIR / 可替换才放、不放进玩家碰撞箱」是放置正确性底线，必须进 P3 deliverable。
4. **【§开放 · 不阻塞 P0】多人 griefing 治理**：全仓 0 个 build/place 保护，worldview 唯一锚是「灵龛 5 格内他人无法破坏方块」（且坐标暴露即失效）。放置打开后任何人可在出生点/他人灵龛旁/阵眼区堆方块封门。最小 spawn-zone 保护 + reach 距离校验的范围与数值待定，**显式标注为已知缺口**。
5. **【§开放 · 数值】满包静默丢失 UX 反馈**：方块破坏频率远高于采矿/采药，满包 warn 丢弃无任何提示。是否在本 plan 给最小反馈（client actionbar/toast「背包满，方块未拾取」）还是接已存在的 `DroppedItemEvent` 落地链（cross-repo 已核实该系统存在），待定。
6. **【§开放 · 接口选型】`block_item_to_state` 带 property 方块**：对原木 axis、未来陷阱 armed/facing 必须是 `(template_id, target_face) → BlockState` 的**函数**而非静态查表；P0 只做单一 template→单一 state，函数签名需预留 target_face 入参（已写进 §P3 交付物 5）。
7. **【§开放 · worldview · ✅ 已定 2026-06-09 → §11.1 #7】方块物品命名**：裸 vanilla id（dirt/cobblestone）违反 materials.toml 全部修仙正典命名惯例（灵草/石块/粗木）。**用户拍板：走正典中文名**（残灰土/碎石/粗石）。「纯沙盒基建」定位（机制无境界/经济锚）与「正典命名」（物品名面子对齐世界观）二者不冲突，详 §11.1 #7。
8. **【§开放 · 防 dupe · 已写 P0 硬约束】drop count 锁 1:1**：可放置方块 `min=max=1`，不可照 `crude_wood` 抄 1-2 区间（已写进 §P0 交付物 4，此处复述为决策门提醒）。

---

## §11.1 决议（pre-P0 收口，2026-06-09，用户拍板）

> 以下针对 §11 中 2 条 blocking + 命名共 3 条开放问题的方向决议。**方向已锁，最细粒度 file:line 落点（select-block 回路的字段命名 / selectedSlot 存哪个 store / HUD 高亮改源）在 §10.0 由 Explore agent 并行核查后补全**，本节给到「可据此开 §10.0」的精度。

### #1+#2 持握态 SSOT + 选中入口 —— 走下层 1-9 SkillBar 的 `Kind.ITEM` 空枝

**决议**：

1. 方块「选中持握待放」走**现有下层 1-9 SkillBar**（用户拍板「我们原本就有两层 hotbar，走下面那层 1-9」），**不新建平行 store、不放宽 server MainHand 校验**（`inventory/mod.rs:3777` 对非 weapon/tool/hoe 的 MAIN_HAND 拒绝**保持不动**，Block 永不进装备槽，放置链不读 `equipped(MAIN_HAND)`）。
2. 方块以 `SkillBarEntry.Kind.ITEM`（`SkillBarEntry.java:7` 已有此变体 + 工厂 `SkillBarEntry.item(templateId, ...)` `:32`）绑定到 1-9 槽，`entry.id()` = 方块 template_id。**不新增 `Kind.BLOCK`**——ITEM 已够，id 即身份。
3. 补一个**选中槽指针**：`SkillBarConfig`（`:6`）现状只有 `slots[]`+`cooldownUntilMs[]`、无 `selectedIndex`（战斗 cast 是按键即发、无"选中"持续态）。新增 `selectedSlot` 状态（落 `SkillBarStore` 还是轻量独立 store 由 §10.0 定）= 放置链的**持握态 SSOT**。
4. `SkillBarKeyRouter.route`（`:27`）对 `Kind.ITEM` 当前返 `PASS_THROUGH`（`:32` 空枝、按键不做事）→ 改为：ITEM 槽按 1-9 → 设 `selectedSlot` + cancel 按键（避免落到 vanilla held-slot）。`Kind.SKILL` 即时 cast 行为不变（同槽位可分别是技能或方块）。
5. **三处读同一个 `selectedSlot`**（防 spawn-chain-wiring 红旗 = 多处各读各的）：① 放置判定 `MixinClientPlayerInteractionManagerAlchemy.bong$alchemyInteractBlock`（`:69`）读"selectedSlot 的 ITEM 是否 Block-category 模板"；② FP fake-stack 注入（`MixinPlayerEntityHeldItem:43` / `MixinHeldItemRenderer:49`）读同一个；③ HUD 高亮（`QuickBarHudPlanner` selectedSlot 形参 `:144`、当前读 vanilla `player.getInventory().selectedSlot`（`BongHud.java:280`）→ 改读 SkillBar `selectedSlot`）。
6. **选中入口回路**：`sendSkillBarBindItem(slot, templateId)`（`ClientRequestSender.java:286`，已定义、零 UI 调用；协议 `encodeSkillBarBindItem` `ClientRequestProtocol.java:972` 已有；server 侧 SkillBar 已能存 ITEM 绑定）接 UI——背包/InspectScreen 把方块物品绑到 1-9 槽，仿绑技能范式 `TechniquesTabPanel.java:215`。**仅缺 UI 触发，协议链已全通**。

**落点**：`client/.../combat/SkillBarKeyRouter.java:32`（ITEM 分支改路由设 selectedSlot）/ `SkillBarConfig.java:6`（加 selectedSlot 或新轻量 store）/ `SkillBarEntry.java:7,32`（复用 `Kind.ITEM`）/ `ClientRequestSender.java:286` + `TechniquesTabPanel.java:215`（bind UI 范式）/ `MixinClientPlayerInteractionManagerAlchemy.java:70`（inject 方法 :71、读 equipped :88）/ `MixinPlayerEntityHeldItem.java:43` / `MixinHeldItemRenderer.java:49` / `QuickBarHudPlanner.java:144` + `BongHud.java:280`（HUD selectedSlot 改源）—— plan §P4 + §0 接入面「共享类型」行。

**§10.0 待核查**：selectedSlot 存哪（SkillBarStore 静态 vs 新 store）；选中态的"取消/切换"语义（按 SKILL 槽是否清 selectedSlot、切走方块槽是否保留）；server 是否需感知 selectedSlot（放置 payload 已带 `item_instance_id`，server 不必知道选中槽——倾向 selectedSlot 纯 client 态，仅驱动 mixin 读取 + HUD）。

### #7 方块物品命名 —— 正典中文名

**决议**：

1. 方块物品走**正典中文名**（用户拍板）：残灰土 / 碎石 / 粗石之类，**不用裸 vanilla id**。
2. `materials.toml` `[[item]]` 的 `name` = 正典中文（残灰土/碎石/粗石之类），`id` = **英文 snake_case**（对齐仓库现有惯例 `spirit_grass`(灵草)/`spirit_wood`(灵木)/`ash_spider_silk`(拟态灰烬蛛丝)，**非拼音**——核查纠正：仓库无 `lingcao`/`shikuai` 拼音 id）。注意 `stone_chunk`（碎石/石块，COBBLESTONE 掉落）**已存在**，软方块需另起 id（dirt→如 `ash_soil`、sand→`coarse_sand`、gravel→`gravel_grit`，具体 §10.0 拍板防撞名），与 §P1 交付物 2 一致。
3. plan 头部「纯沙盒基建」定位**保留**——指机制层无境界/经济/守恒锚点；命名层走正典是物品名对齐世界观，二者不冲突。

**落点**：`server/assets/items/materials.toml`（新 `[[item]]` name/id 正典化）/ plan §P1 交付物 2 + §0 worldview 锚点行 + 头部世界观锚点段。

**§10.0 待核查**：~~每个可放置方块的具体正典中文名拍板~~ → **已收口于 §11.2 C**。

---

## §11.2 决议（§10.0 细节落点收口，2026-06-09，3 Explore agent 实地翻代码）

> 收口 §11.1 标的 3 个「§10.0 待核查」+ §11 #1/#2 的最细落点。全部 Explore 翻代码产出,带 file:line。**收口后 pre-P0 决策门通过,P0/P1 可开。**

### A — selectedSlot SSOT 存放 + 设置 + 生命周期（收口 §11.1 #1 待核查）

**决议**：

1. **存放**：selectedSlot 加进 `SkillBarStore`（`client/.../combat/SkillBarStore.java`）当**静态 volatile int 字段**（`private static volatile int selectedSlot = -1;` + getter/setter + `resetForTests` 重置），**不新建独立 store、不塞进 immutable `SkillBarConfig`**。理由：SkillBarStore 已是 SkillBar 状态唯一 SSOT；selectedSlot 是 mutable 交互态、不适合进 immutable snapshot；独立 store 变三层徒增读错源风险。范式同 `CastStateStore` 的 volatile static 容器,但无需监听器/状态机（selectedSlot 只是 int 指针）。
2. **设置**：`SkillBarKeyRouter.route()`（`:27`）的 `Kind.ITEM` 分支（当前 `:32` 返 `PASS_THROUGH` 空枝）→ 改为 `SkillBarStore.setSelectedSlot(slot)` + 返新枚举值 `RouteResult.ITEM_SELECTED`（`:10` RouteResult 加变体）；`shouldCancelHotbarKey()`（`:16-19`）把 `ITEM_SELECTED`/`ITEM_DESELECTED` 纳入"取消 vanilla 按键"return 条件（避免落到原版 held-slot）。`MixinKeyboardSkillKeys`（`:45`）现有 cancel 逻辑已覆盖,只需 router 改到位。
3. **生命周期**（范式参 `CastStateStore.interrupt` `:64-70` 幂等设计）：① 按 SKILL 槽 cast 前清 selectedSlot(-1)（方块持握与技能 cast 互斥）；② 切到另一 ITEM 槽 = 覆盖 selectedSlot（流畅切方块）；③ **toggle**——再按当前已选中的 ITEM 槽 → setSelectedSlot(-1)（`RouteResult.ITEM_DESELECTED`,对标 vanilla F 键切手的可反转直觉）；④ 放完最后一个方块（stack 归零/该槽 unbind）→ **保守方案**：不直接改 selectedSlot 字段,由 HUD 渲染时检查"该槽是否还有物品"、无则降级渲染为"无选中"（对齐 server inventory snapshot 唯一真值,避免乐观本地误判）。
4. **server 不感知 selectedSlot**：放置 payload 已带 `item_instance_id`（server 据此 `inventory_item_by_instance` 查模板 + 校验 Block category + 映射 BlockState），selectedSlot 对 server 链路冗余。它是**纯 client 交互态**,只驱动 mixin 读取 + HUD 高亮；多人各读各的互不影响。

**落点**：`SkillBarStore.java`（加 selectedSlot 字段+getter/setter+reset）/ `SkillBarKeyRouter.java:10,16-19,27,32`（RouteResult 加值 + route ITEM 分支改 + shouldCancel 扩展）→ plan §P4 FP 注入读 `SkillBarStore.selectedSlot()`。

### B — select-block UI 触发点（收口 §11 #2 待核查）

**决议**：

1. **现状核实**：select-block UI **不存在**——`InspectScreen`（`client/.../inventory/InspectScreen.java`,I 键开,5 tab：装备/修仙/技艺/功法/手搓）右侧 `BackpackGridPanel`（`inventory/component/BackpackGridPanel.java`）展示背包物品（`GridSlotComponent` 1×1,icon 走 `textures/gui/items/{itemId}.png`),但 `availablePillMenuActions()`（`:2256-2279`）右键菜单**无"绑到 1-9 槽"项**,方块物品无绑定入口。
2. **范式 = 照抄绑技能**：`TechniquesTabPanel` 绑技能 = 选功法 → 点 1-9 槽 → `InspectScreen.mouseClicked`（`:1865-1871`）调 `bindSelectedTechniqueToSlot` → `sendSkillBarBindSkill(slot, id)`（`TechniquesTabPanel.java:215`）+ 本地 `SkillBarStore.updateSlot`。
3. **最小落点**：扩 `InspectScreen.availablePillMenuActions()`（`:2256`）——对 Block-category 物品加"绑到槽 N"右键项,触发调 `ClientRequestSender.sendSkillBarBindItem(slot, item.itemId())`（`:286`,已定义零调用）+ 本地 `SkillBarStore.updateSlot(slot, SkillBarEntry.item(templateId, name, 0, 0, "textures/gui/items/"+templateId+".png"))`。协议链已全通,只补这个 UI 触发。
4. **HUD 显示**：`QuickBarHudPlanner.appendCombatRow()`（`:142-205`）现仅 `kind()==SKILL`（`:159`）走 `LoadoutIconLayer.buildSkillIconCommands` 读 iconTexture；补 `kind()==ITEM` 分支同样读 `entry.iconTexture()`（绑定时存的 PNG 路径）渲染方块 icon。

**落点**：`InspectScreen.java:2256`（availablePillMenuActions 加 Block 绑定项 + mouseClicked 复用绑槽路径）/ `ClientRequestSender.java:286`（sendSkillBarBindItem 接 UI）/ `QuickBarHudPlanner.java:159`（ITEM kind icon 分支）/ `SkillBarEntry.item` 工厂（`SkillBarEntry.java:32`）→ plan §P4 交付物补"绑定 UI + HUD ITEM icon"。

### C — 初始方块正典命名集（收口 §11.1 #7 待核查）

**worldview 锚点澄清**：`worldview.md §二:36` 已有正典「**残灰方块**」（灵气下降→土石沙化的**环境地貌**方块,踩上减速+留脚印),但那是 terrain-layer 环境概念,**非可破坏掉落的凡俗材料方块**。本 plan 的方块物品是凡俗材料层（土屑/荒沙等）,与环境残灰方块**主题呼应但实体区分**。命名走末法衰败基调（灰/残/枯/尘/蚀,禁活力词),多数是「中性无直接正典锚」的生态必需品,诚实标注。

**P0 手破即掉软方块**（当前落 `_ => None`、`min=max=1`、无工具）：

| Vanilla | id (snake_case) | name (正典) | 锚点 |
|---|---|---|---|
| DIRT | `earth_crumb` | 土屑 | 中性,末法土壤崩解 |
| COARSE_DIRT | `hardened_soil` | 硬化土 | 中性,呼应 §二 土壤变性 |
| SAND | `barren_sand` | 荒沙 | 中性,§二「土石沙化」景观 |
| GRAVEL | `weathered_stone` | 风化碎石 | 中性,与 `stone_chunk` 区分 |
| CLAY | `raw_clay_lump` | 陶土块 | 中性,供加工 `clay_pot` |

**P1 工具门控示范**（演示 `required_tool`）：

| Vanilla | id | name | 工具 |
|---|---|---|---|
| OBSIDIAN | `obsidian_shard` | 黑曜碎片 | 需镐 tier≥1,缺工具不掉(方块照 AIR) |

防撞已核：6 个 id 均不与 materials.toml 现有 28 条重名；`stone_chunk`(碎石,COBBLESTONE) 已存在故 GRAVEL 用 `weathered_stone` 区分。SNOW/DEEPSLATE/灵晶 等留后续扩展,不进 P0。最终集 P0 实施可微调,但 id/name 风格锁定如上。

> **v1 范围不对称（已知,有意）**：`stone_chunk`(COBBLESTONE/STONE 掉落)**可获取但 v1 不可放置**——本 plan 只把上表软方块纳入可放置集合,不把现有 `stone_chunk` 反向做成 placeable(避免动其现有掉落经济 + 控 P0 范围)。玩家能采到 stone_chunk 却放不回石头,是有意的 MVP 边界,非遗漏;后续可补 stone_chunk→COBBLESTONE 的 `block_item_to_state` 映射开放放置。

**落点**：`server/assets/items/materials.toml`（6 条新 `[[item]]`）/ `block_drop.rs:block_drop_for`（6 条 match arm）→ plan §P0 交付物 1 + §P1 交付物 2。

---

## §10 实施工作流（scope ≥ 4PR）

依赖顺序串行拆分，每个 §10.N 一次 `consume-plan` 到 merge（一个 PR 只动本 plan）：

- **§10.0 pre-P0 细节落点（Explore）✅ 已收口（2026-06-09 → §11.2）**：selectedSlot 存 `SkillBarStore` 静态字段 + 4 条生命周期语义（§11.2 A）、select-block UI = 扩 `InspectScreen.availablePillMenuActions` 加 Block 绑槽右键项（§11.2 B）、初始方块正典命名集 6 条（§11.2 C）。blocking 地基已通,P0 可开。
- **§10.1 = P0 + P1（server 纯后端，无 client 依赖）**：扩 `block_drop` + 工具门控 + `ItemCategory::Block` + 登 TOML。隔离 subagent：sonnet 实施（写代码无 opus 例外）+ opus verify 兜底。CR + Pi agent 双 bot approve 后 merge。`ScheduleWakeup ~3000s` 防长等掉 cache。
- **§10.2 = P2（三端 schema 契约，依赖 §10.1 的 ItemCategory::Block）**：`ClientRequestV1::BlockPlace` 三端对齐 + dist 重建。schema 改动连同 sample 一起改。
- **§10.3 = P3（server consumer，依赖 §10.2 payload）**：`place_block_for_kind` 分叉 + canPlace + 扣减。canPlace 校验是本 PR 硬交付物。
- **§10.4 = P4（client wiring，依赖 §10.0 决策 + §10.2 payload + §10.3 consumer）**：放置 wiring + FP 切手 + icon。建筑/视觉资产无强 3 轮要求（PNG 走 gen-image），但 FP 手感需 runClient 手测。
- **§10.5 = P5（e2e + 收口）**：端到端 + 扩展点文档化 + smoke-test。

**等待协议**：每 PR push 后挂 `pr-watch`，等 CodeRabbit + Pi agent 两 bot 都确认无阻塞（Pi agent 写 ✅ Approve）才 merge；自己的 PR 自己盯到 merge。consume PR 提交前跑对峙自检 workflow（opus4.8 主导 + 多 sonnet 并行对立观点 → opus 逐点裁决）。opus 并发 ≤3。

---

## §进度日志

- `2026-06-09` 骨架创建，博弈流程设计（3 提案 × 复用优先/扩展性优先/原版保真 + 4 视角对抗 + opus 仲裁，21 agents）。裁定赢家=复用优先骨架，嫁接保真案 hand_animation 精度 + 登 TOML 入包路线，嫁接 registry 案 `place_block_for_kind` 单一分叉函数（弃整套 registry/behavior enum）。纠正保真案「COBBLESTONE→None」硬事实错误（实测已掉 stone_chunk）。明确不加 `ItemInstance.block_kind`（template_id 即身份锚，透传面 5→0）。8 条开放问题，其中持握态 SSOT + 选中入口 2 条 blocking 需 pre-P0 收口。
- `2026-06-09` 用户拍板 3 条决议（→ §11.1）：① 持握态 + 选中入口走**下层 1-9 SkillBar**（复用 `Kind.ITEM` 空枝 + 补 `selectedSlot` 指针 + `sendSkillBarBindItem` 接 UI，不新建 store、不放宽 MainHand 校验、不新增 `Kind.BLOCK`）；② 方块物品走**正典中文名**（残灰土/碎石/粗石）。落点已核实存在（`SkillBarEntry.Kind.ITEM:7` / `SkillBarKeyRouter.java:32` PASS_THROUGH 空枝 / `sendSkillBarBindItem:286` 零 UI 调用 / `SkillBarConfig` 无 selectedIndex）。剩 selectedSlot 存哪、取消语义、UI 触发点、具体方块正典名 → §10.0 Explore 收口。
- `2026-06-09` 实地核查（sonnet workflow，4 路逐条翻代码核 50 条承重声明：34 ✅证实 / 8 ⚠️行号漂移 / 5 ❌错 / 0 查无）。**修正 5 处真错**：① W1 `should_apply_default_break` 误写 `should_drop` 且漏 Creative+Start 抹块臂（§P0 测试④）；② W2 `zhenfa_place_tx` 字段声明 :286、`.send()` dispatch 实际 :1316（§0/§P3）；③ W3 `handle_zhenfa_place_requests` 在 `zhenfa/mod.rs:893` 非 client_request_handler.rs（§P3）；④ W4 `handle_zhenfa_trigger_requests` 同在 `zhenfa/mod.rs:1169`（§P5）；⑤ W5 `encodeForgeStationPlace` 真实 :645 非 :1199、envelope 多 `station_tier` 字段（§P2）。另纠命名惯例错：仓库用英文 snake_case（`spirit_grass`）非拼音，软方块 id 另起防撞 `stone_chunk`（§11.1#7）。S6 澄清 `equipped()` 是 client 缓存非直连 server。8 处行号漂移已批量修锚点。**核查后地基判定:可信,改完 pre-P0 ready（仅剩 §10.0 细节落点）**。
- `2026-06-09` §10.0 细节落点收口（3 Explore agent 实地翻代码 → §11.2）：① selectedSlot 存 `SkillBarStore` 静态 volatile 字段 + 4 条生命周期（SKILL cast 清/切槽覆盖/toggle 取消/放完 HUD 降级）+ 纯 client 态 server 不感知；② select-block UI = 扩 `InspectScreen.availablePillMenuActions:2256` 给 Block 物品加"绑到槽 N"右键项 → `sendSkillBarBindItem`,HUD `QuickBarHudPlanner:159` 补 ITEM icon 分支；③ 初始方块命名集 6 条（`earth_crumb`土屑/`hardened_soil`硬化土/`barren_sand`荒沙/`weathered_stone`风化碎石/`raw_clay_lump`陶土块 + `obsidian_shard`黑曜碎片工具示范）,揪出 worldview §二 正典「残灰方块」是环境层、与凡俗材料方块区分。**pre-P0 决策门通过,升 active。**
- `2026-06-09` PR #459 升 active,Pi agent `/review` ✅ 建议合并(90%+ 引用准确)。修 Pi 提的:① `ln.java` 幽灵引用 → 真实 `BongBlocks.java`(行 22/116)+ `generateln` → 真实 gradle 任务 `generateBongBlockIds`(`client/build.gradle:114`,行 150);② `BlockDropEntry` 行号 :31→:32;③ 补 §11.2 C「stone_chunk 可获取但 v1 不可放置」的有意不对称说明。Pi 误报的 `BongHud:280`(实测 :280 确是 selectedSlot)未动。
