# plan-workbench-place-runtime-v1 — 工作台放置 runtime + 通用放置物底盘

> **一句话主题**：补上 `craft/workbench.rs` 三个 stub system（place/interact/break）让"工作台合得出却放不下"的链路闭环——放下 `workbench_item` → spawn 带 `WorkbenchBlock` 的世界 entity（解锁 `craft/session.rs:293` 恒假的"靠近才能合成"约束）→ 右键 emit `WorkbenchOpen` S2C 打开 `WorkbenchScreen` → 破坏返还物品；并抽象 `PlaceableBlockKind` 派发底盘供容器类放置物（[[plan-placeable-container-blocks-v1]]）直接进料。
>
> **本骨架并入并替代原 `plan-block-placement-base-v1`**（2026-06-10 废弃）：原骨架与 [[plan-block-lifecycle-v1]] 高度重叠（都讲方块放置底层），lifecycle P0-P4 已落地通用 `block_place` 三端协议（`world/block_place.rs` 实测已实装，文档自报 ⬜ 是 doc-drift，见 reminder.md:22），剩余的"带交互世界实体的放置物（工作台/容器）"runtime 归本 plan。
>
> **依赖**：[[plan-block-lifecycle-v1]] P4（client 放置 wiring：`sendBlockPlace` + SkillBar `selectedSlot` + 两 held mixin）合入 main —— **仅 P2 client 放置入口依赖；server P0/P1 不依赖**（见 §8.1 #4）。
>
> **状态**：✅ 已完成并归档（2026-06-11）。P0/P1 由 PR #493 合入，P2 由 PR #494 合入；§8 开放问题按实现决议收口。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | `craft/workbench.rs` 三 stub 实装（place→spawn entity / interact→emit WorkbenchOpen / break→返还）+ `workbench_item` 接入放置管线 | ✅ | 2026-06-11 |
| P1 | `PlaceableBlockKind` 抽象（带交互世界实体的通用派发底盘）+ 通用 break 回收 | ✅ | 2026-06-11 |
| P2 | client 交互闭环（WorkbenchInteractIntentHandler）+ Workbench bbmodel 接入 + 视听（改写 `WorkbenchConstants.java` 三 SFX 常量值 → vanilla ID） | ✅ | 2026-06-11 |

---

## §0 接入面 Checklist（防孤岛）

| 接入面 | 命中 |
|--------|------|
| **进料** | `world/block_place.rs:23` `BlockPlaceRequest` event（已稳定，lifecycle 落地，本 plan 复用作唯一放置进料，**不另造 WorkbenchPlace 协议**）；`world/block_place.rs:65` `handle_block_place_requests`（现单一路径：Block category → `block_item_to_state` → 裸 `set_block`，本 plan 在此插 `PlaceableBlockKind` 分叉）；`world/block_break.rs:32` `apply_default_block_break`（消费 `DiggingEvent`，本 plan 加 entity despawn + 返还分支）；`craft/workbench.rs:47` `is_within_workbench_range`（已实装，被 `craft_emit.rs:197` 真实调用）；工作台配方系统（`docs/finished_plans/plan-workbench-recipes-v1.md`，finished）；`local_models/Workbench.bbmodel`（PR #468 已重做） |
| **出料** | 放置后世界 entity 带 `WorkbenchBlock`（`craft/workbench.rs:18`）+ `BongVisualEntity { kind: BongVisualKind::Workbench }`（渲染壳，照 coffin）→ 解锁 `craft/session.rs:293` `recipe.station.is_some() && !deps.has_nearby_workbench` 恒假的真实约束（`craft_emit.rs:187` `has_nearby_workbench` 查询此 entity）；`PlaceableBlockKind` enum + 通用 `place_placeable` / `break_placeable` 派发 → [[plan-placeable-container-blocks-v1]] P0 直接进料；server emit `ServerDataPayloadV1::WorkbenchOpen` → client `ServerDataRouter.java:236` 已注册 `workbench_open` handler 打开 `WorkbenchScreen`（client 侧已就位，等 server emit） |
| **共享类型 / event** | 复用 lifecycle `BlockPlaceRequest`（`block_place.rs:23`）作唯一放置进料；复用现有 `WorkbenchBlock`（`craft/workbench.rs:18`）component（已定义，补 spawn 调用）；复用 `WorkbenchOpenPayload`（`workbench.rs:29`，已定义未 emit）作 interact system 内部载体；复用 `BongVisualEntity` / `BongVisualKind`（`entity_model.rs:67`）渲染壳；**不另造 `WorkbenchPlace` C2S / 不另造 WorkbenchSpawn event**（原 base 骨架的独立 C2S 方案废弃，统一走 BlockPlaceRequest + kind 派发，消解近义重名红旗） |
| **跨仓库契约 · server** | `WorkbenchBlock`（`craft/workbench.rs:18`）+ `WORKBENCH_ITEM_TEMPLATE`（`workbench.rs:42`）；新增 `PlaceableBlockKind`（`world/block_place.rs`，P1）；新增 `WORKBENCH_ENTITY_KIND: EntityKind = EntityKind::new(165)`（`world/entity_model.rs`，紧随 Baolongwang=164）+ `BongVisualKind::Workbench`（`entity_model.rs:67`）；新增 `ServerDataPayloadV1::WorkbenchOpen`（`schema/server_data.rs:272`，`rename_all = "snake_case"` → JSON `type:"workbench_open"`）；复用 `BlockPlaceRequest`（`block_place.rs:23`） |
| **跨仓库契约 · client** | 新建 `WorkbenchInteractIntentHandler`（照 `inventory/SupplyCoffinInteractIntentHandler.java`，crosshair `EntityHitResult` → C2S `sendOpenContainer` 或新 `sendWorkbenchOpen`，见 §8.1 #2）+ `input/DefaultInteractionHandlers.java:14` 注册；新增 `BongEntityModelKind.WORKBENCH`（raw_id **165**，紧随 Baolongwang=164，`BongEntityModelKind.java:193`）+ `WorkbenchRenderer`（照 coffin renderer）+ `BongEntityRenderBootstrap` 注册；`WorkbenchScreenBootstrap.handler()`（`ServerDataRouter.java:236`，已注册，等 server emit）；**改写** `craft/WorkbenchConstants.java:15-21` 的 `SFX_PLACE`/`SFX_BREAK`/`SFX_OPEN` 三常量值（当前自定义 `bong:block.workbench.*` ID 无音频资产 → 改为 vanilla `minecraft:block.wood.place` 等，见 §P2 #3 + §8.1 #5），4 条 craft SFX（`SFX_CRAFT_*`，`:23-33`）+ 3 组 VFX（`:35-68`）保留给 recipes plan（reminder.md:26） |
| **跨仓库契约 · agent** | **不参与**——工作台放置/交互是本地世界操作，无 Redis key、无 IPC 流量、无 narration（天道不参与） |
| **worldview 锚点** | 工作台 = **凡人制造业入口**，对应 worldview §九 经济（制作→交易→循环链起点，`docs/worldview.md:836` §九 货币逼迫流转的搬运/集市经济）；"凡物不设数量限制"决议沿用（`craft/workbench.rs:10` §8.1 #3 已代码化注释）。定位同 [[plan-block-lifecycle-v1]] / `plan-custom-block-v1`：**纯沙盒制造基建**，无境界/修炼正典锚 |
| **qi_physics 锚点** | **无**。实地核查确认 `craft/workbench.rs` / `block_place.rs` / `block_break.rs` 均不碰真元/灵气 ledger。**红旗自查**：本 plan 不引入任何 `*_DECAY*` / `*_DRAIN*` / 衰减常数，不写 `qi_current +=` / `zone.spirit_qi -=`，无 `QiTransfer` 涉及。工作台放置/破坏纯物品收支（消耗/返还 1 个 `workbench_item`），与守恒律无关。注：制作 session 内的真元消耗走 `craft_emit.rs:51` `WorldQiAccount`（既有路径），不在本 plan 范围 |

---

## §P0 — workbench.rs 三 stub 实装 + workbench_item 接入放置管线 ✅

> **红旗根因**（reminder.md:22 + research）：`craft/workbench.rs:58-61` 三个 system 至今只是注释（"将在 PR-3 实装"，PR-3 已 merge 仍未补），是**所有容器类放置阻塞的根因**。`craft/session.rs:293` 的 `recipe.station.is_some() && !deps.has_nearby_workbench` 校验已实装但**恒假拒绝所有工作台配方**——因为 `WorkbenchBlock` entity 从未被 `commands.spawn`（全仓 grep `commands.spawn`+`WorkbenchBlock` 零命中）。

**模块**：`server/src/craft/workbench.rs`（三 system 实装）+ `server/src/world/block_place.rs`（放置分叉）+ `server/src/world/block_break.rs`（破坏分叉）+ `server/src/world/entity_model.rs`（EntityKind + BongVisualKind）+ `server/src/schema/server_data.rs`（WorkbenchOpen payload）+ `server/assets/items/core.toml:414`（category 修正）。

**交付物**：

1. **`workbench_item` 接入放置管线**（先解阻塞）：
   - `core.toml:414` `workbench_item` 的 `category = "misc"` → **`category = "block"`**（`ItemCategory::Block`，`inventory/mod.rs` 已定义解析，lifecycle P1 落地）。**保持 `grid_w=2`/`grid_h=2`**（2×2 物品，与工作台语义符）。block category 默认 `max_stack_count=64`（`inventory/mod.rs:1588`），与凡物制作台符。
   - **不走 `block_item_to_state`**（该函数只映射 vanilla 地形方块 → BlockState，工作台不是 vanilla 方块）。`block_place.rs:65` 现路径在 `block_item_to_state` 返 None 即 reject——本 plan 在 `block_template_id_for_request`（`block_place.rs:185`）/ `handle_block_place_requests` 内**先按 `template.placeable` 字段（P1 引入）/ `WORKBENCH_ITEM_TEMPLATE` 命中 → 走 `PlaceableBlockKind` 分叉，跳过 `block_item_to_state` gate**（P0 先硬编 `workbench_item` 命中，P1 抽象成 `placeable` 字段路由）。

2. **`handle_workbench_place`**（`craft/workbench.rs`，从注释变真 system）：
   - 由 `handle_block_place_requests` 命中 `workbench_item` 后调用（或独立消费 `BlockPlaceRequest` 过滤 `workbench_item`，二选一见 §8.1 #1）。
   - 校验：持有 `workbench_item`（`inventory_item_by_instance_borrow`）+ canPlace（复用 `block_place.rs:255` `can_place_block`：目标格 AIR/replaceable + 不卡玩家碰撞箱 + Y 边界 + chunk loaded）。
   - 放置：`consume_item_instance_once` 扣 1 → `commands.spawn((WorkbenchBlock { placed_by: req.client, placed_at_tick: now }, BongVisualEntity { kind: BongVisualKind::Workbench, source: None }, Position(...)))`（照 `coffin/mod.rs:1827` BongVisualEntity spawn 范式）。**不写世界方块**（纯 entity 表示，见 §8.1 #1）。`send_inventory_snapshot_to_client`。
   - **不设 per-chunk 数量限制**（`workbench.rs:10` §8.1 #3 决议沿用，材料成本已限滥放）。

3. **`handle_workbench_interact`**（`craft/workbench.rs`）：
   - 玩家右键命中 `WorkbenchBlock` entity（C2S intent，P2 client 触发；P0 server 侧消费）→ 校验 `is_within_workbench_range`（`workbench.rs:47`，已实装）→ emit `ServerDataPayloadV1::WorkbenchOpen {}`（新增 variant，无字段或仅 entity_id；client `workbench_open` handler 仅开 UI 不读字段，`WorkbenchScreenBootstrap.java:30`）→ client `ServerDataRouter.java:236` 打开 `WorkbenchScreen`。
   - **填补 emit-only 反向缺口**：`WorkbenchOpenPayload`（`workbench.rs:29`）已定义但全仓零 send（research 红旗）——P0 必须真正 emit `ServerDataPayloadV1::WorkbenchOpen`，否则 client handler 永远收不到（消解"emit-only 无 consumer / consumer 无 emitter"孤岛红旗，本 plan 双向接通）。

4. **`handle_workbench_break`**（`craft/workbench.rs` + `block_break.rs` 分叉）：
   - 监听 `DiggingEvent`（`block_break.rs:32` 已消费）→ 命中位置有 `WorkbenchBlock` entity → 返还 `workbench_item` × 1 到玩家背包（`add_item_to_player_inventory`，背包满则 `spawn_dropped_loot` 掉地）+ `commands.entity(e).despawn()`。
   - **破坏不掉 BlockState**（工作台是纯 entity，世界方块仍是 AIR，无需抹方块）。

5. **`WorkbenchOpen` schema**：`server_data.rs:272` `ServerDataPayloadV1` 加 `WorkbenchOpen {}`（或 `{ entity_id: i32 }`；`rename_all="snake_case"` → JSON `type:"workbench_open"`，匹配 `ServerDataRouter.java:236` 注册名）。补 wire-format pin（照现有变体 roundtrip 测试模板）。**无 agent TS 对端**（agent 不消费此流量，与 `LootContainerSourceKindV1` 同为纯 Rust 服务端 schema）。

6. **EntityKind + BongVisualKind**：`entity_model.rs` 加 `WORKBENCH_ENTITY_KIND: EntityKind = EntityKind::new(165)`（紧随 `COFFIN_BRONZE=163` 后 Baolongwang 取 164，本 plan 取 165）+ `BongVisualKind::Workbench`（`entity_model.rs:67` enum 加变体 + match arm）。

**测试声明（饱和化，`craft::workbench::*` / `world::block_place::*` / `world::block_break::*`）**：
- **happy**：放置 `workbench_item` → spawn entity 带 `WorkbenchBlock { placed_by, placed_at_tick }` + `BongVisualEntity { kind: Workbench }` + `Position`；背包扣 1；snapshot 发出。
- **状态转换**：placed（spawn）→ interact（range 内 → emit `WorkbenchOpen`；range 外 → 不 emit）→ broken（despawn + 返还 `workbench_item`×1）。`has_nearby_workbench`：spawn 前 = false（恒假回归 pin）；spawn 后 3 格内玩家查询 = true（解锁 `session.rs:293` 真实约束，断言 station=Workbench 配方此时可合成）。
- **边界**：interact 恰好 3.0 格（Chebyshev）= 允许（`workbench.rs:47` 边界，复用现有测试）；3.1 格 = 拒绝；背包满时破坏 → 返还物品 `spawn_dropped_loot` 掉地（断言地面有 1 个 `workbench_item` 掉落实体）。
- **错误分支**：`workbench_item` category 仍是 misc（回归保护：改对 TOML 后断言 `ItemCategory::Block`）；canPlace 失败（目标非 AIR / 卡玩家碰撞箱 / chunk 未加载）→ 不扣物不 spawn；interact 命中非 `WorkbenchBlock` entity → 不 emit；非持有者破坏 `WorkbenchBlock` → 仍返还（工作台无归属保护，凡物，对照容器 plan 的 DeadDrop 阵法）。
- **schema pin**：`ServerDataPayloadV1::WorkbenchOpen` 序列化 JSON `type` 字段 == `"workbench_open"`（与 client 注册名对拍，撞红即捕获 router 名漂移）。
- **回归 pin**：`block_item_to_state("workbench_item", _)` 仍返 `None`（确认工作台不误入 vanilla 方块映射路径）；非 workbench 的 vanilla 方块（如 `earth_crumb`）放置走原 `set_block` 路径不变。

**抓手 grep**：`fn handle_workbench_place` / `fn handle_workbench_interact` / `fn handle_workbench_break` / `WorkbenchOpen` / `BongVisualKind::Workbench` / `WORKBENCH_ENTITY_KIND` / `category = "block"`（core.toml workbench_item）。

---

## §P1 — PlaceableBlockKind 抽象 ✅

> **下游契约**：[[plan-placeable-container-blocks-v1]] §0 已声明依赖本 P1 的 `PlaceableBlockKind` enum 含 `StorageCrate` / `DeadDrop` 变体 + 通用 `handle_*_place` / `handle_*_break` 派发（其骨架明确"升 active 判断门：workbench-place-runtime P1 已合入"）。本 P1 必须产出该 enum + 派发底盘，命名与下游一致。

**模块**：`server/src/world/block_place.rs`（enum + 派发）+ `server/src/inventory/mod.rs`（`ItemTemplate.placeable` 字段）+ `server/assets/items/core.toml`（`workbench_item` 加 `placeable` 标记）。

**交付物**：

1. **`PlaceableBlockKind` enum**（`world/block_place.rs`）：
   - P0 落地变体：`Workbench`。
   - **预声明下游变体（接口先于实现锁定）**：`StorageCrate { is_herb: bool }` / `DeadDrop`（[[plan-placeable-container-blocks-v1]] P0 填实 spawn 逻辑；本 plan 只定义 enum 变体 + 在派发 match 留 `todo!`/明确占位分支，**变体命名与下游骨架严格一致**——`StorageCrate`/`DeadDrop`，见 placeable-container §0）。
   - `#[derive(Debug, Clone, PartialEq)]`，`rename_all="snake_case"` 若需序列化（P0/P1 为纯 server enum，暂不上 wire）。

2. **`placeable` 字段 + 路由**：
   - `ItemTemplate`（`inventory/mod.rs`）加 `placeable: Option<String>` 字段（serde，`#[serde(default)]`；注意 `ItemTemplateToml` 用 `deny_unknown_fields`，TOML 写错字段名启动 panic，新字段必须同步加 struct）。
   - `core.toml:414` `workbench_item` 加 `placeable = "workbench"`（关联 `PlaceableBlockKind::Workbench`）。
   - `block_template_id_for_request`（`block_place.rs:185`）/ `handle_block_place_requests`：先查 `template.placeable` → 有值则 `placeable_kind_from_str(s) → PlaceableBlockKind` → 走 `place_placeable` 派发（**绕过 `block_item_to_state` 的 Block-state gate**，与 placeable-container §P0 "绕过 `:212` Block category gate"对齐）；无值则走原 vanilla `block_item_to_state` → `set_block` 路径不变。

3. **通用派发函数**：
   - `place_placeable(kind: PlaceableBlockKind, commands, pos, placed_by, now) -> Result<Entity, BlockPlaceRejectReason>`：按 kind match → `Workbench` 分叉 `commands.spawn((WorkbenchBlock{..}, BongVisualEntity{kind: Workbench}, Position))`（P0 的 spawn 逻辑迁入此处统一）；`StorageCrate`/`DeadDrop` 分支 todo（下游填）。
   - `break_placeable`（或 `handle_container_block_break` 雏形）：通用回收——`Workbench` → 返还 `workbench_item` + despawn；下游容器 → 内容物逐项掉落（[[plan-placeable-container-blocks-v1]] P0 `handle_container_block_break` 填实，本 plan 留接口签名 + Workbench 分支）。

**测试声明（饱和化，`world::block_place::*`）**：
- **happy**：`workbench_item`（`placeable="workbench"`）放置 → `place_placeable(Workbench, ..)` spawn 带 `WorkbenchBlock` 的 entity。
- **每变体专属 pin**：`PlaceableBlockKind::Workbench` 命中正确分支；`placeable_kind_from_str("workbench") == Some(Workbench)`、`"storage_crate" == Some(StorageCrate{..})`、`"dead_drop" == Some(DeadDrop)`（接口先于实现锁定，下游接 impl 不改这条解析测试）。
- **状态转换**：placed → broken（`break_placeable(Workbench)` 返还 + despawn）。
- **错误分支**：未知 `placeable` 值（如 `"nonsense"`）→ `placeable_kind_from_str` 返 None → reject spawn（不 panic）；无 `placeable` 字段的 vanilla 方块 → 走原 `block_item_to_state` 路径不误入派发（回归 pin）；`category != Block` 且无 `placeable` → 原 `block_place.rs:212` reject 行为不变。
- **接口完整性 pin**：`StorageCrate`/`DeadDrop` 变体 match 分支存在（即使 P1 内为 todo，编译穷举保证下游接入时只换 impl 不动 match 骨架）。

**抓手 grep**：`enum PlaceableBlockKind` / `fn place_placeable` / `fn break_placeable` / `fn placeable_kind_from_str` / `placeable:` (ItemTemplate) / `placeable = "workbench"`（core.toml）。

---

## §P2 — client 交互闭环 + Workbench bbmodel 接入 + 视听 ✅

> **视觉资产 → 强制走 docs/CLAUDE.md §6.1 三轮自我打磨 + 终轮 commit `<PROMISE>` 担保块。**
> **依赖 [[plan-block-lifecycle-v1]] P4** 仅限"client 放置入口"（SkillBar `selectedSlot` 选中 `workbench_item` + `sendBlockPlace` 发 BlockPlace）——若 P4 未合，P2 的 client 放置走不通，但**交互（右键打开）/ 渲染 / 视听三块不依赖 P4，可先行**（见 §8.1 #4）。

**模块**：`client/src/main/java/com/bong/client/`（IntentHandler + Renderer + `WorkbenchConstants` 三 SFX 常量值改写 + 接线）+ `local_models/Workbench.bbmodel` + `server/assets/audio/recipes/`。

**交付物**：

1. **client 交互闭环 — `WorkbenchInteractIntentHandler`**（照 `inventory/SupplyCoffinInteractIntentHandler.java`）：
   - crosshair `EntityHitResult` + `BongModeledEntity.modelKind() == BongEntityModelKind.WORKBENCH` + 距离校验（`MAX_INTERACT_DISTANCE_SQ`，照 coffin 5.0²）→ `InteractCandidate.of(InteractIntent.OpenContainer, ...)` → dispatch `ClientRequestSender.sendWorkbenchOpen(entityId)`（新 C2S 或复用 `sendOpenContainer`，见 §8.1 #2）。
   - `input/DefaultInteractionHandlers.java:14` `registerDefaults()` 末尾 `router.register(new WorkbenchInteractIntentHandler());`。
   - server 侧 `handle_workbench_interact`（P0 已实装）消费此 C2S，校验 range，emit `WorkbenchOpen`。**禁止 vanilla entity hack**（不用 armor stand 充碰撞箱，走 Marker entity + C2S IntentHandler，memory `feedback_no_vanilla_hacks`）。

2. **Workbench bbmodel 接入**（`local_models/Workbench.bbmodel` PR #468 已重做存在）：
   - `BongEntityModelKind.java` 新增 `WORKBENCH(raw_id=165, ...)`（紧随 Baolongwang=164，`BongEntityModelKind.java:193` 注释链；与 server `WORKBENCH_ENTITY_KIND=165` 1:1）。**与 [[plan-placeable-container-blocks-v1]] 已对齐**：该骨架正文（:39/:99）+ reminder.md:30 已把货箱/草药箱/死信箱顺延为 **166/167/168**（`TRADE_CRATE`/`HERB_CRATE_PLACED`/`DEAD_DROP_BOX`），本 plan 占 165 无撞号。两 plan 升 active 时核对 165 实际落地即可（见 §8 收口注 + reminder.md:30）。
   - 新建 `WorkbenchRenderer`（照现有 coffin renderer：`*Renderer extends ...`，加载 geo + 贴图）+ `BongEntityRenderBootstrap` 注册。

3. **视听·P2** —— stub 当前零 `SoundRecipePlayer.play()` / 零 `VfxPlayer` 调用，P2 必须**实装接线或删除无法落地的部分**：

   **音效 — 决议：复用 vanilla 音效（不出 bong 自定义音频资产）**：
   - `WorkbenchConstants.java:15-21` 当前三常量是**自定义命名空间 ID**（`SFX_PLACE="bong:block.workbench.place"` / `SFX_BREAK="bong:block.workbench.break"` / `SFX_OPEN="bong:block.workbench.open"`），这些 ID **无对应音频资产**——若直接 `SoundRecipePlayer.play()` 会静默 no-op。P2 **不出 bong 自定义音频资产**（凡物制作台无需专属音色），改为 **P2 直接改写这三个常量值** 为 vanilla recipe id（见下表 `recipe 文件` 列的 basename，无前缀），常量从"自定义 sound ID"语义改为"audio_recipe id"语义（与 `SoundRecipePlayer.play(id)` 入参一致）。
   - **新建 audio_recipe JSON**（schema 照 `tuike_cast.json`：`id` / `layers[].{sound,volume,pitch,delay_ticks}` + 顶层 `priority` / `attenuation` / `category` / `bus`；**`sound` 字段带 `minecraft:` 前缀**；**约束：`id` 字段必须等于文件 basename**，否则 registry load 不命中）：
   - **enum 取值实地核实**（`server/src/schema/audio.rs:30/48/59`）：`attenuation` ∈ `{player_local, world_3d, global_hint, zone_broadcast, SELF, MELEE, AREA, WORLD}`；`category` ∈ `{MASTER,PLAYERS,HOSTILE,AMBIENT,VOICE,BLOCKS}`；`bus` ∈ **`{COMBAT, ENVIRONMENT, UI}`**（无 `WORLD` bus——勿编造）。本表用 `attenuation="world_3d"`（3D 定位，照 `coffin_break.json`）+ `category="BLOCKS"` + `bus="ENVIRONMENT"`。
   | recipe 文件（id=basename） | 触发 | 层 | sound（含 `minecraft:` 前缀） | pitch | volume | delay_ticks | priority | attenuation | category | bus |
   |------|------|----|------|------|------|------|------|------|------|------|
   | `server/assets/audio/recipes/workbench_place.json` | 放置（`SFX_PLACE`→`"workbench_place"`） | 1 | `minecraft:block.wood.place` | 0.9 | 0.8 | 0 | 50 | `world_3d` | `BLOCKS` | `ENVIRONMENT` |
   | | | 2 | `minecraft:block.chain.place` | 1.1 | 0.4 | 2（铁钉微响） | | | | |
   | `workbench_break.json` | 破坏（`SFX_BREAK`→`"workbench_break"`） | 1 | `minecraft:block.wood.break` | 0.8 | 0.7 | 0 | 50 | `world_3d` | `BLOCKS` | `ENVIRONMENT` |
   | `workbench_open.json` | 右键打开（`SFX_OPEN`→`"workbench_open"`） | 1 | `minecraft:block.barrel.open` | 1.1 | 0.5 | 0（木盖推开） | 50 | `world_3d` | `BLOCKS` | `ENVIRONMENT` |

   - **`WorkbenchConstants.java` 三常量值改写明细**：`SFX_PLACE` `bong:block.workbench.place` → `"workbench_place"`；`SFX_BREAK` `bong:block.workbench.break` → `"workbench_break"`；`SFX_OPEN` `bong:block.workbench.open` → `"workbench_open"`（值 = audio_recipe id = 文件 basename）。**抓手 grep**：`SFX_PLACE = "workbench_place"`。
   - 制作进行中/完成/失败（`SFX_CRAFT_START`/`TICK`/`DONE`/`FAIL`，`WorkbenchConstants.java:23-33`）属**制作 session 视听**，归 `plan-workbench-recipes-v1`（finished）范围——P2 **不接这 4 条、不改这 4 个常量值**，在 plan 内显式标注"这 4 条 SFX 常量非本 plan 范围，制作 session 视听已归 recipes plan"（避免越界，stub 注释保留）。
   - 接线点：放置/破坏/打开三事件触发 `SoundRecipePlayer.play(WorkbenchConstants.SFX_PLACE/...)`（`audio/SoundRecipePlayer.java:23` 单例，`BongClient.java:112` 已 bootstrap）。

   **粒子（放置落座尘土，本地 client spawn，无 vfx_event）**：
   - 基类 `BongSpriteParticle`（`visual/particle/BongSpriteParticle.java:14`），spawn 模式 **burst 4 颗**，颜色 `#8B7355`（土褐），lifetime **12 tick**，spawn 位置工作台底部 0.1 格高随机散布，速度 0.05 格/tick 向外+轻微下沉，贴图复用现有 dust sprite（不新增）。无 `bong:vfx_event`（client 本地 spawn，照 placeable-container P2 同款落座尘土）。
   - **VFX 常量取舍**：`WorkbenchConstants` 的 3 组 VFX（制作中丹砂红 `#8B3A3A` burst / 完成 burst / 真元蓝白 `#A8C4E0` beam）全部属**制作 session 视听**，归 recipes plan——P2 **不实装这 3 组**（与 4 条 craft SFX 同理）。P2 仅实装"放置落座尘土"这一条放置态粒子（新加，非 stub 列表内）。**stub 决议**：7 SFX 中 place/break/open 三条 P2 改写常量值为 vanilla recipe id（见 #3）+ 接线，4 条 craft SFX + 3 组 VFX 常量值不动、留给 recipes plan 接活（在 `WorkbenchConstants.java` 注释标注归属，不删——下游 plan 会用）。

4. **测试声明**：
   - **client**（`WorkbenchInteractIntentHandlerTest`）：happy（crosshair 对准 WORKBENCH entity range 内 → dispatch open）；边界（range 外 > 5 格不触发）；错误（非 WORKBENCH kind entity 在 crosshair → 不误触，kind mismatch）。
   - **bbmodel/渲染**：`BongEntityModelKind.WORKBENCH` raw_id 唯一性 pin（165 不与现有 146-164 冲突）；`WorkbenchRenderer` 在 `BongEntityRenderBootstrap` 注册 pin；`local_models/Workbench.bbmodel` 存在性（CI 检查）。
   - **audio**：3 条 audio_recipe JSON schema 解析无误（`workbench_place/break/open.json`）。
   - **e2e**：（依赖 lifecycle P4）SkillBar 选中 `workbench_item` → `sendBlockPlace` → server spawn → 渲染出 Workbench bbmodel → 走近 3 格内 → 右键 → emit `WorkbenchOpen` → client 打开 `WorkbenchScreen` → 此时 station=Workbench 配方可合成（联动 P0 `has_nearby_workbench`）→ 破坏 → 返还 `workbench_item`。截图验证模型渲染 + WorkbenchScreen 打开。

**抓手 grep**：`WorkbenchInteractIntentHandler` / `BongEntityModelKind.WORKBENCH` / `WorkbenchRenderer` / `sendWorkbenchOpen` / `workbench_place.json` / `SoundRecipePlayer.play("workbench`。

---

## §视听规格汇总（内联引用上方阶段块）

| 阶段 | 类型 | 规格摘要 |
|------|------|----------|
| P2 | SFX 放置 | `workbench_place.json`（id=basename）L1 `minecraft:block.wood.place` p0.9 v0.8 d0 + L2 `minecraft:block.chain.place` p1.1 v0.4 d2；priority 50 / attenuation world_3d / category BLOCKS / bus ENVIRONMENT |
| P2 | SFX 破坏 | `workbench_break.json` `minecraft:block.wood.break` p0.8 v0.7 d0；priority 50 / world_3d / BLOCKS / ENVIRONMENT |
| P2 | SFX 打开 | `workbench_open.json` `minecraft:block.barrel.open` p1.1 v0.5 d0；priority 50 / world_3d / BLOCKS / ENVIRONMENT |
| P2 | 粒子 落座尘土 | `BongSpriteParticle` burst 4 颗 `#8B7355` lifetime 12t（底部 0.1 格散布，无 vfx_event 本地 spawn） |

> **narration**：本 plan **无 narration**——工作台放置/交互是本地世界操作，天道不参与（接入面 agent 标"不参与"）。

> **非本 plan 范围视听**（`WorkbenchConstants.java` stub 剩余项，归 `plan-workbench-recipes-v1`）：4 条 craft SFX（`SFX_CRAFT_START/TICK/DONE/FAIL`）+ 3 组制作 session VFX（丹砂红 `#8B3A3A` / 完成 burst / 真元蓝白 `#A8C4E0` beam）。在常量注释标归属，不删。

---

## §8 开放问题（P0 决策门前需收口）

> 调研证据已能拍板的直接在下方定案（带依据 file:line）；真正悬留的列入待 §8.1 收口。**实施前必须追加 `## §8.1 决议（pre-P0，YYYY-MM-DD）`，每条带 file:line + plan 章节双锚点。**

**已凭证据定案（写入正文，原表保留追溯）**：

- **#3 工作台 entity 表示：纯 entity vs vanilla 占位 vs bong_blocks** → **纯 entity + bbmodel 渲染**（照 `coffin/mod.rs:1827` `BongVisualEntity` spawn + `SupplyCoffinInteractIntentHandler` 的 `EntityHitResult` crosshair 路径，已有成熟先例）。依据：① coffin 容器同款纯 entity 模式成熟（`COFFIN_*_ENTITY_KIND` + `BongVisualKind`）；② [[plan-block-lifecycle-v1]] §P3 凡俗 vanilla 方块走裸 `set_block`，但工作台需要交互 + bbmodel 外观，vanilla `crafting_table` 占位无法挂自定义模型 + 交互需额外 DiggingEvent 监听，复杂度高于纯 entity；③ [[plan-placeable-container-blocks-v1]] §8 #3 已预期"跟随本 plan §8 #1 的决议（纯 entity）"。**容器 plan 与本 plan entity 表示必须一致**（已统一为纯 entity，下游骨架已对齐，无需改下游）。（已写入 P0/P1）
- **#2 "靠近才能合成"启用时机：P0 即强制 vs 配置开关** → **P0 落地即强制**。依据 `craft/session.rs:293` `recipe.station.is_some() && !deps.has_nearby_workbench` 校验**已存在且恒假**（`craft_emit.rs:187` `has_nearby_workbench` 查询从未 spawn 的 `WorkbenchBlock`）——P0 实装 spawn 后此校验自动生效，**手搓配方（station=None，`session.rs:598` 默认 `has_nearby_workbench:true`）不受影响**，仅 station=Workbench 配方需靠近。无需配置开关。（已写入 P0 测试声明）
- **#3（原 base 骨架）容器类 vanilla 占位决议** → 容器沿用本 plan 纯 entity 决议（见上 #3），[[plan-placeable-container-blocks-v1]] §8.1 #3 已对齐"纯 entity + bbmodel"，无须额外改下游。（已对齐）

- **#5 工作台 SFX：复用 vanilla vs 出 bong 自定义音频资产** → **复用 vanilla**。依据：① `WorkbenchConstants.java:15-21` 三常量当前是自定义 ID `bong:block.workbench.{place,break,open}`，全仓无对应音频资产（grep 确认），直接 `SoundRecipePlayer.play()` 静默 no-op；② 凡物制作台无需专属音色，照 `coffin_break.json` 用 `minecraft:block.wood.*` 即可。**实施动作 = 改写常量值**（`bong:block.workbench.place` → `"workbench_place"` 等，值改为 audio_recipe id = 文件 basename，语义从"sound ID"变"recipe id"），非"接活已声明 ID"——原 plan"接活 stub"措辞不准确，已在 §P2 #3 + 头部表 + 跨仓库契约·client 改为"改写常量值"。audio_recipe JSON schema 照 `tuike_cast.json`（`sound` 带 `minecraft:` 前缀 + `priority`/`attenuation`/`category`/`bus`，enum 取值见 §P2 #3，`id`==basename）。4 条 craft SFX + 3 组 VFX 不动，留 recipes plan。（已写入 §P2 #3 + §视听规格汇总）

**悬留待 §8.1（真正未决）**：

| # | 问题 | 推荐默认 |
|---|------|------|
| 1 | **工作台 place system 归属**：`handle_workbench_place` 独立消费 `BlockPlaceRequest`（过滤 `workbench_item`）vs `handle_block_place_requests` 内 `PlaceableBlockKind` 分叉调用 → 决定 P0/P1 边界。 | **P1 的 `place_placeable` 统一派发**（P0 先在 `handle_block_place_requests` 硬编 `workbench_item` 命中调 spawn 逻辑，P1 重构为 `placeable` 字段路由 + `place_placeable`）。避免两套并行放置消费者（近义重名红旗）。升 active 确认 P0/P1 是否合并为单 PR（scope 小，见 §10.2）。 |
| 2 | **client 打开 C2S 协议**：复用 coffin 的 `sendOpenContainer`（按 entity id）vs 新建 `sendWorkbenchOpen` 专用 C2S？ | **新建 `sendWorkbenchOpen(entityId)`**（工作台打开语义 = 开 `WorkbenchScreen` 制作 UI，非开容器 grid，与 `OpenContainer` 语义不同；coffin/container 走 `LootContainerOpen`，工作台走 `WorkbenchOpen`，避免协议复用导致 server 端 dispatch 分不清打开目标）。但若实现期发现 server 端可统一按 entity 的 component 类型路由（`WorkbenchBlock` vs `ExternalContainer`），可复用单一 open C2S——升 active 时按 `client_request_handler.rs` dispatch 实际结构二选一。 |
| 4 | **P2 client 放置入口对 lifecycle P4 的依赖耦合度**：P2 是否必须等 lifecycle P4 合入？ | **解耦**：P2 的"交互（右键打开）+ 渲染 + 视听"三块不依赖 P4，可独立实施测试；仅"client 放置入口"（选中 `workbench_item` → `sendBlockPlace`）依赖 P4 的 SkillBar `selectedSlot` + `sendBlockPlace` wiring。**P2 e2e 完整路径依赖 P4，但 P2 大部分交付物可在 P4 前完成**。升 active 时核 lifecycle P4 是否已合（reminder.md:22 标 P4 在 worktree 分支），未合则 P2 e2e 标"待 P4 合后补"，其余先行。 |

> 升 active 时核对 `reminder.md`：① raw_id 占号协调已登记在 reminder.md:30（本 plan 占 165 `WORKBENCH`，[[plan-placeable-container-blocks-v1]] 顺延 166/167/168，且该 sibling 正文 :39/:99 已为 166/167/168），核对 165 实际落地避免撞号即可，**无需再改 sibling raw_id**；② 确认 lifecycle P4 合入状态（决定 P2 e2e 时机）。

---

## §10 实施工作流

升 active 时按 docs/CLAUDE.md §6 执行。本 plan scope = 3 阶段、估 3 PR（P0+P1 可能合并为 1-2 PR，P2 1 PR）。虽 < 4 PR 边界，但因含 client 视觉资产（bbmodel）+ 跨 server/client 契约，仍写 §10 明确拆分点与三轮打磨。

### §10.1 视觉资产三轮打磨（P2 强制）

P2 含 Workbench bbmodel 渲染接入 → **强制走 §6.1 三轮自我打磨**：Round 1（`local_models/Workbench.bbmodel` PR #468 已重做，跑 `scripts/models/render_bbmodel.py` 预览确认接入正确）`(round 1/3)` → Round 2（截图 client 内渲染：比例/灵木板纹/铁钉骨钉细节 review 修）`(round 2/3)` → Round 3（与 worldview「灵木板钉凡铁骨钉的粗木台子」`core.toml:420` 描述一致性 + 制造业入口视觉叙事）`(round 3/3)`，终轮 commit 末尾写 `<PROMISE>` 担保块（拼写 PROMISE）：已检查比例/灵木纹/铁钉加固/视觉叙事/spec 一致。**Workbench.bbmodel 已存在（PR #468），P2 主要工作是接入渲染管线，不重跑生成器覆盖。**

### §10.2 PR 拆分点（依赖顺序，前一个 merge 后开下一个）

1. **PR-1（P0+P1）**：三 stub 实装 + `workbench_item` category 修正 + `PlaceableBlockKind` 抽象 + `WorkbenchOpen` schema + EntityKind/BongVisualKind（纯 server + schema，可合一 PR，scope 适中）。**P0/P1 边界见 §8.1 #1**（P0 硬编命中、P1 抽象 `placeable` 路由，自然分两次 commit 或一 PR 两段）。
2. **PR-2（P2）**：client `WorkbenchInteractIntentHandler` + `BongEntityModelKind.WORKBENCH` + `WorkbenchRenderer`（三轮打磨 + PROMISE）+ 3 audio_recipe + 落座尘土粒子 + `WorkbenchConstants` 三条 SFX 常量值改写 + 接线 + e2e（e2e 依赖 lifecycle P4，未合则标"待补"）。

### §10.3 subagent 配置

每 PR 起独立 subagent（`subagent_type: "claude"`，`model: "opus"`，prompt 末尾 `ultrathink`），共享主 worktree（非 nested）。主线只接收 result（200-500 token）+ 亲自 merge。subagent 只实施 + 提 PR，不等 review。

### §10.4 CR 等待协议

每 PR 走 §6.5：`gh pr checks` `pending` → `ScheduleWakeup delaySeconds=1200`，最多 3 回合（60 min）；`fail` 按严重性桶处理，修完**重等 CR re-review**，不自判通过。前一 PR APPROVED/收敛才开下一个。

### §10.5 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan` 后即可下班，醒来看本 plan 是否已迁入 `docs/finished_plans/`。全自动：测试/CI 失败 ≤2 轮有限修复，review 意见自行判断采纳，仅严重设计问题/反复修不过才交人工。

## Finish Evidence

### 落地清单

- **P0**：`server/src/craft/workbench.rs` 实装 `handle_workbench_place` / `handle_workbench_interact` / `handle_workbench_break`，`server/src/schema/server_data.rs` 接入 `WorkbenchOpen`，`server/src/world/entity_model.rs` 固定 `WORKBENCH_ENTITY_KIND=165` 与 `BongVisualKind::Workbench`，`server/assets/items/core.toml` 将 `workbench_item` 接入 block/placeable 路由。
- **P1**：`server/src/world/block_place.rs` 落地 `PlaceableBlockKind` / `placeable_kind_from_str` / `place_placeable` / `break_placeable`，`server/src/inventory/mod.rs` 支持 `ItemTemplate.placeable`，工作台放置绕过 vanilla `block_item_to_state` gate 并走通用派发底盘。
- **P2**：`client/src/main/java/com/bong/client/craft/WorkbenchInteractIntentHandler.java`、`ClientRequestSender.sendWorkbenchOpen`、`WorkbenchRenderer`、`BongEntityModelKind.WORKBENCH`、`WorkbenchPlaceDust`、`workbench.geo.json` / `workbench.animation.json` / `workbench_intact.png` 完成交互、渲染与放置粒子；`server/assets/audio/recipes/workbench_{place,break,open}.json` 与 `WorkbenchConstants` 三个 recipe id 接通放置/破坏/打开音效。
- **资源包**：`server/src/network/resourcepack.rs` 与 `client/resourcepack/manifest.json` 已同步最新资源包 sha/size/file_count，CI 的 resourcepack 产物校验通过。

### 关键 commit

- `20c7054cf` · 2026-06-11 · `feat(workbench-place-runtime-v1): P0/P1 制作台放置底盘 (#493)`
- `f7abe6213` · 2026-06-11 · `feat(workbench-place-runtime-v1): 完成制作台客户端交互与资源 (#494)`

### 测试结果

- `cd server && BONG_SKIP_SKIN_PREFETCH=1 CARGO_BUILD_JOBS=2 nice -n 10 ionice -c3 cargo fmt --check`
- `cd server && BONG_SKIP_SKIN_PREFETCH=1 CARGO_BUILD_JOBS=2 nice -n 10 ionice -c3 cargo test -j 2 workbench`
- `cd server && BONG_SKIP_SKIN_PREFETCH=1 CARGO_BUILD_JOBS=2 nice -n 10 ionice -c3 cargo test -j 2 proto_convert`
- `cd server && BONG_SKIP_SKIN_PREFETCH=1 CARGO_BUILD_JOBS=2 nice -n 10 ionice -c3 cargo test -j 2 audio`
- `cd server && BONG_SKIP_SKIN_PREFETCH=1 CARGO_BUILD_JOBS=2 nice -n 10 ionice -c3 cargo test -j 2 client_request_handler`
- `cd client && JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn nice -n 10 ionice -c3 ./gradlew test --max-workers=2`
- `cd client && JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn nice -n 10 ionice -c3 ./gradlew build --max-workers=2`
- `python3 -m unittest scripts/test_build_resourcepack.py`
- CI：PR #494 `Build resource pack` 通过；PR #494 `E2E Redis Smoke / e2e` 通过（schema、agent、server release build、server cargo test、Task 13 smoke/e2e 全部成功）。

### 跨仓库核验

- **server**：`WorkbenchOpenRequest`、`ClientRequestV1::WorkbenchOpen`、`ServerDataPayloadV1::WorkbenchOpen`、`PlaceableBlockKind::Workbench`、`WORKBENCH_ENTITY_KIND`、`WORKBENCH_PLACE_AUDIO_RECIPE_ID` / `WORKBENCH_BREAK_AUDIO_RECIPE_ID` / `WORKBENCH_OPEN_AUDIO_RECIPE_ID` 均已命中。
- **client**：`WorkbenchInteractIntentHandler`、`ClientRequestProtocol.encodeWorkbenchOpen`、`ClientRequestSender.sendWorkbenchOpen`、`BongEntityModelKind.WORKBENCH`、`BongEntityRenderBootstrap`、`WorkbenchRenderer`、`WorkbenchPlaceDust`、`WorkbenchConstants.SFX_PLACE = "workbench_place"` 均已命中。
- **proto/schema**：`proto/bong/envelope.proto` 包含 S2C `WorkbenchOpen` 与 C2S `WorkbenchOpenReq`，`server/src/schema/proto_convert.rs` 覆盖 C2S/S2C roundtrip。
- **agent**：不参与；本 plan 无 Redis key、无 IPC narration、无天道侧 schema 改动。

### 遗留 / 后续

- `StorageCrate` / `DeadDrop` 的实际放置与破坏逻辑留给 `plan-placeable-container-blocks-v1`，本 plan 已提供 `PlaceableBlockKind` 接口与工作台分支。
- 4 条 craft session SFX 与 3 组制作 session VFX 仍归 `plan-workbench-recipes-v1` / 后续制作反馈 plan，不在本 plan 范围内改动。
