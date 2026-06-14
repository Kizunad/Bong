# plan-placeable-container-blocks-v1 — 可放置世界容器方块（货箱 / 草药箱 / 死信箱）+ bbmodel

> **来源**：手搓 104 产出物僵尸审计「容器全死（12）」一类里适合作世界方块的 3 个（`trade_crate` / `herb_crate` / `dead_drop_box`）。
> **依赖**：[[plan-workbench-place-runtime-v1]]（放置/破坏/交互底盘 + `PlaceableBlockKind` enum）+ [[plan-nested-pack-base-v1]]（容器打开协议 + `ContainerState` 子容器机制）。**两个 plan 都 merge 到 main 后才开本 plan。** 升 active 判断门：① workbench-place-runtime **P1（`PlaceableBlockKind` 抽象）** 已合入；② workbench-place-runtime **§8 #3 entity 表示确实落地为「纯 entity + bbmodel」**（非 `bong_blocks`）——本 plan 整条交互/渲染层按此假设写满 P0-P3，上游若改走 bong_blocks 则本 plan 需重写，升 active 前必须核实上游实际落地路线。
> **状态**：✅ 已完成并归档（2026-06-14）。P0/P1/P2/P3 分别由 PR #549/#551/#552/#553 合入 main，§8 开放问题按实现决议收口。

把适合作世界方块的容器（`trade_crate` 货箱 / `herb_crate` 灵草箱放置版 / `dead_drop_box` 死信箱）实装为可右键放置、破坏、打开搜索的世界 entity 容器，配 bbmodel 资产。**进料**：放置走 [[plan-workbench-place-runtime-v1]] 的 `PlaceableBlockKind` 派发底盘，容器 state 复用 `external_container.rs` 的 `ExternalContainer` + `ContainerState`。**出料**：放下 → 世界 entity 带 `ExternalContainer { kind, container }`；右键 → `LootContainerOpenV1` S2C；内容操作复用 coffin 同款 `ExternalContainerMove`/`Close` 协议（[[plan-nested-pack-base-v1]] 的 `PackContainer*` 协议为可选升级路径，见 §8.1 #5）。**对应 worldview §九:850（盲盒死信箱）+ §九 搬运/集市经济。**

## 阶段总览

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | `ExternalContainerKind` 扩 `StorageCrate`/`DeadDrop` + 三 TOML placeable 标记 + 放置/破坏链路（接 `PlaceableBlockKind` 底盘） | ✅ | 2026-06-14 |
| P1 | schema `LootContainerSourceKindV1` 扩两变体 + **新增通用容器 open 路径**（泛化 C2S/独立 open system，**非复用 coffin open**）+ `ExternalContainerKind→LootContainerSourceKindV1` 映射 + client IntentHandler | ✅ | 2026-06-14 |
| P2 | bbmodel 资产（货箱/灵草箱/死信箱）+ `BongEntityModelKind` 三 variant + 渲染接入 + 视听 + e2e | ✅ | 2026-06-14 |
| P3 | 死信箱阵法防砸（worldview §九:850 物品化灰 + 破阵机制）+ 破阵 VFX + 端到端验收 | ✅ | 2026-06-14 |

## 接入面（防孤岛）

- **进料**：
  - [[plan-workbench-place-runtime-v1]] P1 的 `PlaceableBlockKind` enum（其骨架 §P1 已声明含 `StorageCrate` / `DeadDrop` 变体）+ 通用 `handle_*_place` / `handle_*_break` 派发（`craft/workbench.rs:55-61` 三 stub 由该 plan 实装）。
  - `inventory/external_container.rs:17` `ExternalContainerKind`（现仅 `SupplyCoffin { grade }`）+ `:23` `ExternalContainer` ECS component（含 `session_id`/`container`/`opened_by`/`timeout_wall_secs`/`source_kind`，结构体本体直接复用免改）+ `:41` `ExternalContainerRegistry` + `:62` `pack_loot_into_grid`。
  - `inventory/mod.rs:322` `ContainerState`（grid 容器类型）+ `:1276` `find_free_slot`。
  - `world/block_place.rs:23` `BlockPlaceRequest` + `:65` `handle_block_place_requests`（**注意 `:212` 当前硬性拒绝 `category != ItemCategory::Block`，容器放置必须走 `PlaceableBlockKind` 派发层绕过此 gate，见 P0**）。
  - `client/.../inventory/SupplyCoffinInteractIntentHandler.java:25-93`（crosshair `EntityHitResult` 检测 + 距离校验 + dispatch 模板，新建两个 IntentHandler 照抄改 kind + send 方法）。**注意**：现有 C2S 是 coffin 专属 `ClientRequestSender.sendSupplyCoffinOpen(int entityId)`（`ClientRequestSender.java:489`，编码 `SupplyCoffinOpen { entity_id }`，`schema/client_request.rs:462`），**不存在通用 `sendOpenContainer`**——P1 必须新增通用 open C2S（泛化 `SupplyCoffinOpen`→`ContainerOpen` 或新建独立变体，见 P1）。
  - `client/.../input/DefaultInteractionHandlers.java:14-18`（`registerDefaults()` 注册点，现注册 6 个 handler）。
  - `client/.../network/LootContainerHandler.java:14`（处理 `LootContainerOpen/Update/Close` S2C，已实装）。
  - bbmodel 管线：`scripts/models/gen_trade_crate.py` / `gen_herb_crate.py` / `gen_dead_drop_box.py`（三脚本已存在）+ `scripts/models/render_bbmodel.py`（预览）；延寿棺先例 `scripts/models/gen_mundane_coffin.py`。
- **出料**：
  - 三容器放下 → server `commands.spawn` 世界 entity，带 `ExternalContainer { kind: StorageCrate/DeadDrop, container: ContainerState }`（grid 用各容器 TOML 尺寸）+ `BongEntityModelKind` 渲染标记。
  - 右键搜索打开 → `LootContainerOpenV1`（`server_data.rs:609`）S2C，`source_kind` 区分 `StorageCrate`/`DeadDrop`。
  - 内容 move/close 复用 coffin 同款 `ExternalContainerMove` / `ExternalContainerClose`（`schema/client_request.rs:458`）；破坏 → 若 `opened_by.is_some()` 先向打开者 emit `LootContainerCloseV1 { reason: ContainerDestroyed }`（新增变体，照 `supply_coffin/lifecycle.rs:133`）→ `ExternalContainer.container` 内容物逐项掉落（DeadDrop 非破阵破坏改走「化灰」，见 P3）+ entity despawn。
- **server open 链路（不可复用 coffin handler，必须新增）**：现有 `handle_supply_coffin_interact`（`supply_coffin/interact.rs:71`）开头即 `let Some(active) = registry.active.get(&ev.target).cloned() else { ... };`（`:84`）——**只认 `SupplyCoffinRegistry.active` 注册的 entity**，StorageCrate/DeadDrop 永不进该 registry，故「复用 coffin open system」对货箱/死信箱**根本不工作**；且 source_kind 在 `:129`/`:208` 硬编 `LootContainerSourceKindV1::SupplyCoffin { grade }`，无 `ExternalContainerKind→LootContainerSourceKindV1` 映射分支。C2S 路由 `client_request_handler.rs:1942`（`ClientRequestV1::SupplyCoffinOpen`）转 `SupplyCoffinOpenRequest` 事件，也是 coffin 专属。**P1 必须新增通用 open 路径**（见 P1）。
- **共享类型 / event**：
  - 复用 `ExternalContainer` / `ExternalContainerRegistry` / `LootContainerOpenV1` / `LootContainerUpdateV1` / `LootContainerCloseV1`（`server_data.rs:605-642`）的**数据结构与 S2C 协议**，**不另造世界容器组件**；但 **open 路径必须新增**（coffin 的 open handler 因 `SupplyCoffinRegistry.active` gate + 硬编 source_kind 不可复用，见上「server open 链路」）。
  - `ExternalContainerKind`（`external_container.rs:17`）加 `StorageCrate` / `DeadDrop` 变体；`LootContainerSourceKindV1`（`server_data.rs:605`）镜像加 `StorageCrate` / `DeadDrop` 变体；`LootContainerCloseReasonV1`（`server_data.rs:629`）加 `ContainerDestroyed` 变体（**不复用 `CoffinDestroyed`**，避免 client 按棺文案处理货箱/死信箱）。
  - bbmodel 走现有 entity 模型资源包管线（[[plan-resourcepack-v1]] + `BongEntityModelKind`），不引新管线。
- **跨仓库契约（symbol）**：
  - **server**：`ExternalContainerKind::StorageCrate` / `::DeadDrop`（`external_container.rs:17`）；`LootContainerSourceKindV1::StorageCrate` / `::DeadDrop`（`server_data.rs:605`，**纯 Rust 服务端 schema，无 TS 对端——agent 不消费 loot container 流量，研究已核实 `agent/packages/schema/src/` 无 `LootContainerSourceKind` 定义**）；**新增** 通用 open C2S 变体（泛化 `ClientRequestV1::SupplyCoffinOpen`→`ContainerOpen` 或独立变体，`client_request.rs:462`）+ open system + `external_kind_to_source_kind` 映射函数（见 P1）。
  - **client**：新建 `StorageCrateInteractIntentHandler` / `DeadDropInteractIntentHandler`（照 `SupplyCoffinInteractIntentHandler`）+ `DefaultInteractionHandlers.java:14` 注册 + 通用 open C2S send（`ClientRequestSender.java:489` 现有 `sendSupplyCoffinOpen`，P1 泛化/新建）+ `BongEntityModelKind` 新增 `TRADE_CRATE` / `HERB_CRATE_PLACED` / `DEAD_DROP_BOX`（**raw_id 166/167/168**——[[plan-workbench-place-runtime-v1]] §8 #3 已先占 165 给 `WORKBENCH`，本 plan 顺延占 166/167/168，见 §8.1 #3，reminder.md 登记）+ 对应 `*Renderer` + `BongEntityRenderBootstrap` 注册。
  - **agent**：**无关**——无 Redis key、无 IPC 流量、无 narration（容器为本地世界交互）。
- **worldview 锚点**：
  - 死信箱 = **worldview §九:850（盲盒死信箱）**——「箱子遭受非对应破坏时，内部阵法启动——物品化灰 + 原地引爆毒气雷」「全程双方不见面」。阵法防砸 + 化灰是 P3 正典根。
  - 货箱/灵草箱 = **worldview §九（搬运/集市经济）**——货箱是末法集市搬运/贸易载体（§九:836 货币逼迫流转，搬运工经济）。
  - `dead_drop_box` 解锁需走私者师承（`craft/workbench_recipes.rs:519-523` 已声明 `Mentor { smuggler }`，与本 plan 吻合）。
- **qi_physics 锚点**：**无**——容器方块本身不碰真元/灵气/衰减（内含高灵物的保鲜/逸散归 [[plan-container-filter-and-completion-v1]]，走 shelflife，不在本 plan）。**红旗自查**：本 plan 不引入任何 `*_DECAY*` / `*_DRAIN*` / 衰减常数，不写 `qi_current +=` / `zone.spirit_qi -=`，无 ledger `QiTransfer` 涉及。死信箱「物品化灰」是物品 despawn（`ItemInstance` 移除），不涉及真元守恒（化灰物品若携带 `spirit_quality > 0` 不回流 zone——这是物品销毁不是真元蒸发，与 qi 守恒律无关，见 P3 §决议）。

## P0 — ExternalContainerKind 扩展 + 放置/破坏链路 ⬜

**纯 server 逻辑，无视听。**

- **`inventory/external_container.rs:17`** `ExternalContainerKind` 加两变体：
  - `StorageCrate { is_herb: bool }`（`trade_crate` 与 `herb_crate_placed` 共用变体，`is_herb` 区分模型/打开音/筛选；筛选实际生效归 [[plan-container-filter-and-completion-v1]]，本 plan 只存 flag）。
  - `DeadDrop`（无字段；阵法状态 P3 加）。
- **TOML 标记**（三条物品加 placeable 关联，依赖 workbench-place-runtime 的 placeable 字段约定）：
  - `server/assets/items/workbench_materials.toml:281` `trade_crate`：加 `placeable = "storage_crate"`（关联 `PlaceableBlockKind::StorageCrate`），保持 `category = "misc"`、`grid_w=2`/`grid_h=2`。
  - `:292` `herb_crate`：**新建独立 template `herb_crate_placed`**（避免与 [[plan-nested-pack-base-v1]] P5 把 `herb_crate` 升 `category=container` 随身版的冲突——`ItemCategory` 只能一种，见 §8.1 #2）。`herb_crate_placed` 走 `placeable = "storage_crate"` + `is_herb` 路径。`herb_crate` 随身版归 nested-pack。
  - `:237` `dead_drop_box`：加 `placeable = "dead_drop"`，保持 `grid_w=2`/`grid_h=2`。
- **`PlaceableBlockKind` 派发**：在 [[plan-workbench-place-runtime-v1]] P1 的 `handle_*_place` 通用派发里，`StorageCrate` / `DeadDrop` 分支 `commands.spawn` 世界 entity，挂 `ExternalContainer { session_id: registry.allocate_session(entity), container: ContainerState::new(rows, cols), opened_by: None, timeout_wall_secs: 0, source_kind }`（grid 尺寸：货箱/灵草箱 `4×4`、死信箱 `3×3`，见 §8.1 #3）+ `Position`。**绕过 `block_place.rs:212` 的 `Block` category gate**：放置请求按 `template.placeable` 字段路由进 `PlaceableBlockKind` 派发，不进 `block_item_to_state` 路径。
- **破坏链路**：新建 system `handle_container_block_break`（`block_drop.rs` 无此逻辑，是新增）——容器 entity 被破坏时：① **若 `ExternalContainer.opened_by.is_some()`**（破坏时正被某玩家打开）→ 向该玩家 emit `LootContainerCloseV1 { session_id, reason: ContainerDestroyed }` 强制关客户端 UI（**参照 `supply_coffin/lifecycle.rs:133-145` 的 coffin destroyed 关闭范式**——其检查 `ext.opened_by` 后 `send_close_payload` + inventory snapshot）；② 遍历 `ExternalContainer.container` 内 `PlacedItemState` 逐项 `spawn_dropped_loot`（复用现有掉落实体）+ 掉落容器本身物品 + `commands.entity(e).despawn()` + `registry.remove_session`。DeadDrop 非破阵破坏的「化灰」特例归 P3（P0 先做通用掉落 + 强制关闭，P3 覆盖 DeadDrop 化灰分支，**化灰时仍须先强制关闭打开者 UI**）。
- **新增 close reason 变体**：`LootContainerCloseReasonV1`（`server_data.rs:629`，现仅 `Timeout`/`Distance`/`PlayerClosed`/`CoffinDestroyed`）**新增 `ContainerDestroyed` 变体**——**不复用 `CoffinDestroyed`**（语义错误：货箱/死信箱非棺，client 标题/文案按 coffin 处理会错）。连带：`proto_convert.rs:1455` close-reason 映射加 `ContainerDestroyed => "container_destroyed"` + schema roundtrip pin（照 `:4680` 现有四变体枚举测试）+ client `LootContainerHandler.java` 处理新 reason（关 UI，文案区分）。
- **测试声明（饱和化，`external_container::*` / `world::block_*::*`）**：
  - happy：放置 `trade_crate` spawn 的 entity 带 `ExternalContainerKind::StorageCrate { is_herb: false }` + grid `4×4`；`dead_drop_box` → `DeadDrop` + `3×3`；`herb_crate_placed` → `StorageCrate { is_herb: true }`。
  - 状态转换：placed（spawn）→ broken（despawn + session removed）；session id 单调递增且 `ExternalContainerRegistry.sessions` 正确登记/移除。
  - 边界：空容器破坏只掉落容器本身物品（无内容物掉落项）；满容器（grid 全占）破坏掉落 = 内容物全列 + 容器本身。
  - **open 中破坏 → 强制关闭**：`opened_by = Some(player)` 时破坏容器 → 断言该玩家收到 `LootContainerCloseV1 { reason: ContainerDestroyed }`（对照 `opened_by = None` 时破坏不 emit close）；close reason 枚举 roundtrip pin 含 `ContainerDestroyed`。
  - 错误分支：未知 `placeable` 值拒绝 spawn；非 placeable 物品走原 `Block` gate 不误入容器派发；`category != Block` 且无 `placeable` 字段 → 原 `:212` reject 行为不变（回归 pin）。

## P1 — schema 扩展 + 打开搜索链路 + IntentHandler ⬜

**视听见 §视听·P1。**

- **schema**：`server_data.rs:605` `LootContainerSourceKindV1` 加 `StorageCrate { is_herb: bool }` / `DeadDrop`（`rename_all = "snake_case"`，与现有 `SupplyCoffin` 对齐）。补 schema roundtrip 测试（照 `:4719` `loot_container_source_kind_supply_coffin_wire_format` 模板，每个新变体一条 wire-format pin）。**无需改 `agent/packages/schema/`**（确认 loot container schema 无 TS 对端）。
- **server 打开（新增通用 open 路径，非复用 coffin）**：coffin 的 `handle_supply_coffin_interact`（`supply_coffin/interact.rs:71`）因 `registry.active.get(&ev.target)`（`:84`）gate + 硬编 `source_kind: SupplyCoffin`（`:129`/`:208`）**不可复用**。二选一实装：
  - **方案 A（泛化 C2S，推荐）**：把 coffin 专属 `ClientRequestV1::SupplyCoffinOpen { entity_id }`（`client_request.rs:462`）泛化为 `ContainerOpen { entity_id }`，在 `client_request_handler.rs:1942` 分支按「target entity 是否带 `ExternalContainer` 组件」而非「是否在 `SupplyCoffinRegistry.active`」路由（coffin entity 也带 `ExternalContainer`，向后兼容）。
  - **方案 B（独立 open system）**：新建 `handle_container_open` system 直接 `Query<&mut ExternalContainer>`（不经 `SupplyCoffinRegistry`），消费独立 C2S 变体。
  - 命中后：`ExternalContainer.opened_by = Some(player)` → emit `LootContainerOpenV1 { session_id, source_kind: external_kind_to_source_kind(&ext.source_kind), rows, cols, placed_items, timeout_wall_secs }`。**新增映射函数** `external_kind_to_source_kind(ExternalContainerKind) -> LootContainerSourceKindV1`（`SupplyCoffin→SupplyCoffin`、`StorageCrate{is_herb}→StorageCrate{is_herb}`、`DeadDrop→DeadDrop`）——现有代码无此映射，须新建。距离/timeout 校验逻辑可抽出 coffin 现有 `OPEN_RANGE_*` 常量（`supply_coffin/interact.rs`）复用，但 system 本体新增。
- **client IntentHandler**（照 `SupplyCoffinInteractIntentHandler.java:25-93`）：
  - 新建 `client/.../inventory/StorageCrateInteractIntentHandler.java`：crosshair `EntityHitResult` + `BongEntityModelKind == TRADE_CRATE || HERB_CRATE_PLACED` + 距离校验 → 调 P1 新增的通用 open C2S（方案 A 走泛化后的 `ClientRequestSender.sendContainerOpen(entityId)`，方案 B 走新 send 方法）。**不存在 `sendOpenContainer`**——现有方法是 coffin 专属 `sendSupplyCoffinOpen`（`ClientRequestSender.java:489`），P1 须泛化或新建（见 P1）。
  - 新建 `client/.../inventory/DeadDropInteractIntentHandler.java`：kind 检测 `== DEAD_DROP_BOX`。
  - `DefaultInteractionHandlers.java:18` `registerDefaults()` 末尾 `router.register(new StorageCrateInteractIntentHandler()); router.register(new DeadDropInteractIntentHandler());`。
- **打开渲染**：`LootContainerHandler.java:14` 已处理 `LootContainerOpenV1`，按 `source_kind` 走标题/打开音区分（StorageCrate/DeadDrop）。内容 move/close **复用 coffin 同款** `ExternalContainerMove` / `ExternalContainerClose`（`client_request.rs:458`）——research 确认 nested-pack 的 `PackContainer*` 协议尚未实装，本 plan **不依赖**它，走 fallback 外部容器协议（升级路径见 §8.1 #5）。
- **测试声明（client `*IntentHandlerTest` + server open path）**：
  - happy：crosshair 对准 `trade_crate` entity（range 内）→ dispatch 通用 open C2S；server 收 open → 命中带 `ExternalContainer` 的非 coffin entity → emit `LootContainerOpenV1` source_kind=StorageCrate（断言 `external_kind_to_source_kind` 映射正确）。
  - 映射 pin：`external_kind_to_source_kind(SupplyCoffin{grade})→SupplyCoffin{grade}`、`StorageCrate{is_herb:false}→StorageCrate{is_herb:false}`、`StorageCrate{is_herb:true}→...{is_herb:true}`、`DeadDrop→DeadDrop` 各一条（穷举三 kind + coffin 向后兼容）。
  - 边界：range 外（> 交互距离）不触发（照 coffin `OPEN_RANGE_*` gate）；非容器 entity 在 crosshair → 不误触（无 `ExternalContainer` 组件 / kind mismatch）。
  - 回归 pin：泛化 open 路径不破坏 coffin——coffin entity（带 `ExternalContainer { source_kind: SupplyCoffin }`）走新路径仍 emit source_kind=SupplyCoffin。
  - 错误分支：open 已被他人打开的容器（`opened_by` 非 None）→ 拒绝；session 不存在 → reject。
  - schema pin：`LootContainerSourceKindV1::StorageCrate { is_herb }` 正反 sample 序列化对拍；`DeadDrop` 同。

## P2 — bbmodel 资产 + 渲染接入 + 视听 + e2e ⬜

**视觉资产 → 强制走 docs/CLAUDE.md §6.1 三轮自我打磨 + 终轮 commit `<PROMISE>` 担保块。**

- **bbmodel 产出**（分部件 gen 脚本 → 逐件预览 → 拼接；脚本已存在，需跑 + 用户 Blockbench 手改）：
  - `trade_crate` 货箱：`scripts/models/gen_trade_crate.py` 已建（实心板箱身 0.94³ MC 格 + 铁角件）。`local_models/TradeCrate.bbmodel` 已存在。
  - `herb_crate_placed` 灵草箱：`scripts/models/gen_herb_crate.py`（**用户已手改 `HerbCrate.bbmodel` 升 fmt5.0，默认只刷预览，须显式 `--write` 才覆盖——勿误覆盖**，见 memory）。`local_models/HerbCrate.bbmodel` 已存在。
  - `dead_drop_box` 死信箱：`scripts/models/gen_dead_drop_box.py` 已建（低矮铁箍木箱 0.94×0.72×0.84 + 正面阵纹发光窗 array zone 青光）。**`local_models/DeadDropBox.bbmodel` 尚不存在——P2 需先跑脚本生成，再交用户手改**（照 HerbCrate 流程）。
- **`BongEntityModelKind` 接入**（`BongEntityModelKind.java`）：新增 `TRADE_CRATE`（raw_id 166）/ `HERB_CRATE_PLACED`（167）/ `DEAD_DROP_BOX`（168），连号紧随 [[plan-workbench-place-runtime-v1]] 的 `WORKBENCH=165`（其 §8 #3 已先占 165；`Baolongwang=164`，:193 注释）。**升 active 时与 workbench plan 核对 165 实际落地，避免撞号。** 新建 `TradeCrateRenderer` / `HerbCratePlacedRenderer` / `DeadDropBoxRenderer`（照现有 coffin renderer），在 `BongEntityRenderBootstrap` 注册。
- **视听·P2（放置/打开/破坏，玩家可感知）**：
  - **放置 SFX**：新建 audio_recipe `server/assets/audio/recipes/container_place.json` —— 单层 `block.wood.place` pitch=0.9 volume=0.8 delay_ticks=0；死信箱另加第二层 `block.gravel.place` pitch=0.8 volume=0.6 delay_ticks=2（埋地感）。
  - **打开 SFX**：货箱/灵草箱新建 `container_open.json` —— `block.barrel.open` pitch=1.0 volume=0.7 delay_ticks=0（参照 `supply_coffin_open_common.json` 结构）；灵草箱（`is_herb`）pitch=1.1（藤编轻响）。死信箱打开 `container_open_deaddrop.json` —— `block.chest.open` pitch=0.7 volume=0.6 + `block.amethyst_block.chime` pitch=0.5 volume=0.4 delay_ticks=3（阵纹微鸣）。
  - **破坏 SFX**：新建 `container_break.json` —— `block.wood.break` pitch=0.8 volume=0.9 delay_ticks=0（参照 `coffin_break.json`）。
  - 放置落座尘土粒子（照 workbench 同款）：`BongSpriteParticle` burst 4 颗，颜色 `#8B7355`，lifetime 12 tick，spawn 模式 burst（容器底部 0.1 格高随机散布），贴图复用现有 dust sprite，无新 vfx_event（本地客户端 spawn）。
- **测试声明**：bbmodel 文件存在性（CI 检查 `local_models/*.bbmodel` 或导出 geo）；`BongEntityModelKind` raw_id 唯一性 pin（166/167/168 不与现有冲突，165 已归 [[plan-workbench-place-runtime-v1]] `WORKBENCH`）；renderer 注册 pin（三 renderer 在 bootstrap 注册）；audio_recipe JSON schema 校验（4 条 recipe 解析无误）。
- **e2e**：合成（走私者师承解锁 dead_drop_box）→ 放置 → 渲染出 bbmodel → 右键搜索打开 → 拖入/拖出内容（走 `ExternalContainerMove`）→ 破坏 → 内容物 + 容器掉回地面。client e2e 截图验证三模型渲染正确。

## P3 — 死信箱阵法防砸 + 破阵 VFX + 验收 ⬜

**worldview §九:850 正典实装；视听见 §视听·P3。**

- **阵法状态**：`ExternalContainerKind::DeadDrop` 加阵法字段 `{ ward_active: bool, owner: Entity }`（放置后默认 `ward_active = true`，`owner` = 放置玩家 entity，作毒气雷 `CombatEvent.attacker` 与「持有者本人」判定依据）。死信箱 entity 加 `DeadDropWard` marker component（阵纹激活态）。
- **毒气伤害规格**（参照 `zhenfa::BlastTrap` 范式，`zhenfa/mod.rs:2055-2090`）：破阵时半径 **3.0 格**内 query 玩家，逐个 emit `CombatEvent { attacker: ward.owner, target, body_part: Torso, wound_kind: <bruise/poison-equiv>, damage: 8.0, contam_delta: 0.15, source: AttackSource::Melee, ... }` + `ApplyStatusEffectIntent { target, kind: StatusEffectKind::Slowed, magnitude: 0.4, duration_ticks: TICKS_PER_SECOND * 4 }`（**复用现有 `Slowed` 变体，不新增 Poison**）。毒气持续 **2 tick 内单次结算**（非持续 DoT，MVP 一次性引爆）。**`CombatEvent.attacker` 必填——用 `ward.owner` 满足约束**（与 BlastTrap 用 trap owner 同源）。
- **破阵机制**（worldview §九:850「非对应破坏」）：
  - **正确开启**（持有者本人右键 → P1 的 open 路径）→ 正常搜刮，不触发阵法。
  - **非对应破坏**（非持有者破坏、或非破阵直接砸）→ 触发阵法：① 容器内所有 `ItemInstance` **直接 despawn（物品化灰，不掉落）**——覆盖 P0 通用 `handle_container_block_break` 的 DeadDrop 分支（化灰是物品销毁，非真元蒸发，不涉 ledger，见 接入面 qi_physics 锚点）；② 原地触发毒气雷 AoE 伤害。**实地核验（2026-06-10）**：`combat/events.rs:81` `StatusEffectKind` **无 Poison/Gas 变体**（只有 Bleeding/Slowed/.../ContaminationBoost，后者是 alchemy 污染压力标记非毒气伤害）；`CombatEvent`（`events.rs:169`）是单 target + **必填 `attacker: Entity`** 的逐目标伤害，无「无 attacker 的范围陷阱伤害」原语。**不存在 emit-only 可复用的毒气 AoE damage 事件**——dugu_v2 `poison_mist`（`combat/dugu_v2/skills.rs:270/396/526`）仅是 `emit_vfx` 视觉 + caster/target 相对的招式逻辑。
    - **真实可复用原语**：`zhenfa` 的 **`ZhenfaKind::BlastTrap`** 是「已落地的放置陷阱型 AoE damage」——其 tick system 半径扫描目标后 `combat_events.send(CombatEvent { attacker: snapshot.owner / instance.owner, target, damage: blast_damage(qi), contam_delta, ... })`（`zhenfa/mod.rs:2055-2090`、`:2277-2290`）+ 可选 `ApplyStatusEffectIntent`（`:2268`），`blast_damage` 见 `zhenfa/trap_content.rs:186`。即 **trap 用 owner（投递者 entity）当 attacker** 绕过「CombatEvent 必填 attacker」约束。
    - **本 plan 落地方式**：DeadDrop 需在 entity 上存投递者 entity（见下「阵法状态」补 `owner` 字段），破阵时新建一个**针对死信箱的轻量 AoE 扫描逻辑**（半径内玩家 query → 逐个 `CombatEvent { attacker: owner, target, ... }` + `ApplyStatusEffectIntent`），**参照 BlastTrap 范式但不直接复用其 ZhenfaInstance 数据结构**（死信箱不是 zhenfa 阵法实例）。毒气伤害用 `contam_delta`（污染）+ 一个减速/眩晕 StatusEffect（如 `Slowed`/`Stunned`/`Disoriented`，复用现有变体，**不新增 Poison 变体**）。**具体数值见下「毒气伤害规格」。**
  - **破阵**（MVP 边界，见 §8.1 #4）：MVP 仅做「非持有者破坏 → 化灰 + 毒气」；「破解阵法后无害破坏」归 follow-up（避免 P3 引入完整破阵小游戏）。
- **破阵 VFX**（玩家可感知，server emit `bong:vfx_event` → client player）：
  - 新建 `client/.../visual/particle/DeadDropBreakPlayer.java`（照 `FormationActivatePlayer.java:16` 结构，`VfxBootstrap.java:55` 注册）。
  - 粒子：`BongSpriteParticle` burst **12 颗**，颜色青色 `#3AA0C0`（阵纹色），lifetime **20 tick**，spawn 模式 radial（容器中心半径 0.6 格球面外散，速度 0.15 格/tick），贴图复用现有 formation glyph sprite，`bong:vfx_event` id = `dead_drop_ward_break`。
  - 毒气二段：`BongSpriteParticle` continuous 8 颗/tick × 10 tick，颜色毒绿 `#6B8E23` opacity 0.6，lifetime 30 tick，spawn radial（地面 1.5 格半径扩散）。
  - **破阵 SFX**：新建 `dead_drop_ward_break.json` —— layer1 `block.amethyst_block.break` pitch=0.8 volume=1.0 delay_ticks=0（阵纹碎）；layer2 `entity.generic.explode` pitch=1.2 volume=0.7 delay_ticks=4（毒气雷）；layer3 `block.fire.ambient` pitch=0.6 volume=0.5 delay_ticks=6（毒气弥散）。
- **测试声明**：
  - 状态转换：DeadDrop placed（`ward_active=true`）→ owner open（不触发，正常搜刮）→ owner close；DeadDrop placed → 非 owner 破坏（触发：内容 despawn + 半径内每玩家收到 `CombatEvent { attacker == ward.owner, damage==8.0, contam_delta==0.15 }` + `ApplyStatusEffectIntent { kind: Slowed }` + `dead_drop_ward_break` vfx_event emit）。
  - AoE 边界：半径 3.0 格内玩家受击、3.0 格外玩家不受击（off-by-one 距离 pin）；`CombatEvent.attacker` 恒等于 `ward.owner`（非 None，满足必填约束）。
  - 边界：空死信箱被非法破坏 → 仍触发阵法（毒气）但无化灰物品；满死信箱化灰后 0 掉落（断言地面无掉落实体，对照 P0 通用掉落分支被覆盖）；**open 中被非法破坏 → 打开者仍先收到 `LootContainerCloseV1 { reason: ContainerDestroyed }` 再化灰**（化灰路径不得吞掉强制关闭）。
  - 错误分支：owner 自己破坏（持有者主动废弃）→ §8.1 #4 决议行为（推荐：owner 破坏走 P0 通用掉落不触发阵法）。
  - VFX：`dead_drop_ward_break` event_id 唯一性 + client player 注册 pin。
- **e2e**：放置死信箱 → 第二玩家砸 → 物品化灰（地面无掉落）+ 毒气 AoE + 青色破阵 VFX + 三层 SFX 全触发。

## §视听规格汇总（内联引用上方阶段块）

| 阶段 | 类型 | 规格摘要 |
|------|------|----------|
| P2 | SFX 放置 | `container_place.json` `block.wood.place` p0.9 v0.8；死信箱 +`block.gravel.place` p0.8 v0.6 d2 |
| P2 | SFX 打开 | `container_open.json` `block.barrel.open` p1.0 v0.7（草药箱 p1.1）；死信箱 `container_open_deaddrop.json` `block.chest.open` p0.7 + `block.amethyst_block.chime` p0.5 d3 |
| P2 | SFX 破坏 | `container_break.json` `block.wood.break` p0.8 v0.9 |
| P2 | 粒子 落座尘土 | `BongSpriteParticle` burst 4 颗 `#8B7355` lifetime 12t（无 vfx_event，本地 spawn） |
| P3 | 粒子 破阵 | `BongSpriteParticle` burst 12 颗 `#3AA0C0` lifetime 20t radial；vfx_event `dead_drop_ward_break`；`DeadDropBreakPlayer` |
| P3 | 粒子 毒气 | `BongSpriteParticle` continuous 8/tick×10t `#6B8E23` opacity 0.6 lifetime 30t radial |
| P3 | SFX 破阵 | `dead_drop_ward_break.json` 三层：`block.amethyst_block.break` p0.8 v1.0 d0 / `entity.generic.explode` p1.2 v0.7 d4 / `block.fire.ambient` p0.6 v0.5 d6 |

> **narration**：本 plan **无 narration**——容器交互是本地世界操作，天道不参与（接入面 agent 标「无关」）。死信箱破阵不广播（worldview §九:850「全程双方不见面」，匿名性要求不暴露阵法事件给天道）。

## §8 开放问题（P0 决策门前需收口）

> 调研证据已能拍板的直接在下方定案（带依据 file:line）；真正悬留的列入待 §8.1 收口。**实施前必须追加 `## §8.1 决议（pre-P0，YYYY-MM-DD）`，每条带 file:line + plan 章节双锚点。**

**已凭证据定案（写入正文，原表保留追溯）**：

- **#1 死信箱首发完整形态 vs 随身版** → **placeable 完整形态**。依据 worldview §九:850「在荒野深处埋一个上锁的特制储物箱」原意即世界放置投递点，无随身版语义。（已写入 P0/P3）
- **#3 bbmodel block state（vanilla 占位 vs bong_blocks）** → **纯 entity + bbmodel 渲染**（照 `SupplyCoffinInteractIntentHandler` 的 `EntityHitResult` crosshair 路径，已有先例；避免 research 风险 2 的 `BlockHitResult` 无先例路径）。依据 `block_place.rs:212` 拒绝非 Block category + coffin entity 模式成熟。**与 [[plan-workbench-place-runtime-v1]] §8 #3 已对齐**——该 plan §8 #3（plan-workbench-place-runtime-v1.md:177-179）已凭证据定案为「纯 entity + bbmodel」（`coffin/mod.rs:1827` `BongVisualEntity` spawn 先例），并显式声明本 plan §8 #3「已预期跟随其纯 entity 决议、下游已对齐」。**两 plan 同一结论互相引用，无须各写各的。** ⚠️**升 active 硬约束**：核对 workbench plan 升 active/落地时 entity 表示**确实**收口为纯 entity；若上游临时改走 `bong_blocks` 路线，本 plan 整条 `EntityHitResult` crosshair 交互 + `BongEntityModelKind` 渲染路径需重写——届时停下交人工，不得自行延续纯 entity 假设。（已写入 P0/P1）

**悬留待 §8.1（真正未决）**：

| # | 问题 | 推荐默认 |
|---|------|------|
| 2 | `herb_crate` 双形态：随身版（[[plan-nested-pack-base-v1]] P5 升 `category=container`）+ 放置版（本 plan）是否共存？ | **新建独立 `herb_crate_placed` template 作放置版**（`ItemCategory` 只能一种，研究风险 4：同 template_id 不能既 placeable 世界方块又 container 背包格）。随身 `herb_crate` 归 nested-pack，放置 `herb_crate_placed` 归本 plan。**须与 nested-pack P5 同步确认命名 + TOML 不冲突**（research 已确认 nested-pack P5 改 `herb_crate:292`）。 |
| 4 | 死信箱「破阵」MVP 深度：仅阻断/触发阵法 vs 完整破解小游戏？owner 自己破坏行为？ | **MVP：非持有者破坏 → 化灰 + 毒气（P3）；owner 破坏 → P0 通用掉落不触发**。完整破阵小游戏（买家循坐标无害取物）归 follow-up。owner 破坏行为需用户拍板（推荐通用掉落，废弃自己的死信箱不该自爆）。 |
| 5 | 内容操作协议：fallback `ExternalContainerMove` vs 升级 [[plan-nested-pack-base-v1]] 的 `PackContainer*`？ | **MVP 走 `ExternalContainerMove`（coffin 同款，research 确认 nested-pack 协议未实装，本 plan 不阻塞等它）**。若 nested-pack 先 merge，升级为 `PackContainer*` 是可选增强（统一容器协议），但**非本 plan 依赖**。升 active 时按 nested-pack 实际进度二选一。 |

> 升 active 时同步更新 `reminder.md:17`：[[plan-container-filter-and-completion-v1]] 12 容器验收表「落地阶段」列标 `trade_crate`→本 plan-P0、`herb_crate_placed`→本 plan-P0、`dead_drop_box`→本 plan-P0/P3。

## §8.1 决议（pre-P0，2026-06-10）

- **#1 死信箱形态**：按 §8 #1 定案为世界可放置完整形态，不做随身版；实现证据为 `server/assets/items/workbench_materials.toml:226` 的 `dead_drop_box` + `:229` 的 `placeable = "dead_drop"`，并由 P0/P3 的 `ContainerBlockKind::DeadDrop` / `DeadDropWard` 路径消费。
- **#2 草药箱双形态**：按 §8 #2 推荐默认新建 `herb_crate_placed` 放置版，保留 `herb_crate` 随身版给 nested-pack；实现证据为 `server/assets/items/workbench_materials.toml:294` 的 `herb_crate_placed` + `:297` 的 `placeable = "storage_crate"`，P0 归档证据记录 `StorageCrate { is_herb: true }`。
- **#3 raw_id / 尺寸分配**：按 §P2 约束使用纯 entity + bbmodel，`TRADE_CRATE/HERB_CRATE_PLACED/DEAD_DROP_BOX` 顺延占 raw_id 166/167/168；实现证据为 `client/src/main/java/com/bong/client/entity/BongEntityModelKind.java:253` / `:264` / `:275` 与 `server/src/world/container_block.rs:552` / `:581` / `:610` 的 marker raw id 测试，容器 grid 由 `server/src/world/container_block.rs:156` 写入 `ExternalContainer`。
- **#4 死信箱破阵 MVP**：按 §8 #4 收口为 owner 破坏走普通掉落、非 owner 破坏化灰 + 毒气，不做完整破解小游戏；实现证据为 `server/src/world/container_block.rs:287` 的 owner fallback 判定、`:300` 的 `trigger_dead_drop_ward` 与 `server/src/world/container_block.rs:56` / `:147` 的 `DeadDropWard` 激活态。
- **#5 内容操作协议**：按 §8 #5 继续复用 `ExternalContainerMove` / `ExternalContainerClose`，新增通用 open 仅负责世界容器打开入口；实现证据为 `server/src/schema/client_request.rs:504` 的 `ContainerOpen`、`:516` 的 `ExternalContainerMove` 与 `server/src/world/container_open.rs:30` 的 `ContainerOpenRequest`。

## §10 实施工作流

升 active 时按 docs/CLAUDE.md §6 执行。本 plan scope = 4 PR，多 PR 序列化（不拆多 plan，§6.3）。

### §10.1 视觉资产三轮打磨（P2 强制）

P2 含 bbmodel 视觉资产 → **强制走 §6.1 三轮自我打磨**：Round 1 first cut（gen 脚本跑出三模型 + `render_bbmodel.py` 预览）`(round 1/3)` → Round 2 截图比例/铁角/阵纹窗 review 修 `(round 2/3)` → Round 3 与 spec 一致性 + 视觉叙事（货箱搬运感/草药箱藤编/死信箱埋地阵纹）`(round 3/3)`，终轮 commit 末尾写 `<PROMISE>` 担保块（拼写 PROMISE）：已检查比例/铁角加固/阵纹发光窗/视觉叙事/spec 一致。**注意 `gen_herb_crate.py` 用户已手改，默认不 `--write` 覆盖；`DeadDropBox.bbmodel` 需先跑生成器再交用户手改。**

### §10.2 PR 拆分点（依赖顺序，前一个 merge 后开下一个）

1. **PR-1（P0）**：`ExternalContainerKind` 扩展 + 三 TOML placeable + 放置/破坏链路（纯 server，依赖 workbench-place-runtime P1 已合）。
2. **PR-2（P1）**：`LootContainerSourceKindV1` schema 扩 + roundtrip 测试 + 打开链路 + 两 client IntentHandler + 注册。
3. **PR-3（P2）**：bbmodel 三资产（三轮打磨 + PROMISE）+ `BongEntityModelKind` + renderer + 视听 audio_recipe + e2e。
4. **PR-4（P3）**：死信箱阵法防砸 + 化灰/毒气 + 破阵 VFX/SFX + e2e。

### §10.3 subagent 配置

每 PR 起独立 subagent（`subagent_type: "claude"`，`model: "opus"`，prompt 末尾 `ultrathink`），共享主 worktree（非 nested）。主线只接收 result（200-500 token）+ 亲自 merge。subagent 只实施 + 提 PR，不等 review。

### §10.4 CR 等待协议

每 PR 走 §6.5：`gh pr checks` `pending` → `ScheduleWakeup delaySeconds=1200`，最多 3 回合（60 min）；`fail` 按严重性桶处理，修完**重等 CR re-review**，不自判通过。前一 PR APPROVED/收敛才开下一个。

### §10.5 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan` 后即可下班，醒来看本 plan 是否已迁入 `docs/finished_plans/`。全自动：测试/CI 失败 ≤2 轮有限修复，review 意见自行判断采纳，仅严重设计问题/反复修不过才交人工。

## Finish Evidence

### 落地清单

- **P0 · 放置/破坏底盘**：`server/src/inventory/external_container.rs` 增加 `ExternalContainerKind::StorageCrate` / `DeadDrop` 与 `external_kind_to_source_kind` 基础；`server/src/world/container_block.rs` / `server/src/world/block_place.rs` 接通 `trade_crate`、`herb_crate_placed`、`dead_drop_box` 的实体容器放置、破坏、强制关闭与掉落；`server/assets/items/workbench_materials.toml` 写入 `placeable = "storage_crate"` / `"dead_drop"`；`LootContainerCloseReasonV1::ContainerDestroyed` 与 client close reason 处理已补齐。
- **P1 · 通用打开链路**：`server/src/world/container_open.rs` 新增 `ContainerOpenRequest` 通用 open system；`server/src/schema/client_request.rs` / `proto/bong/envelope.proto` / `server/src/network/client_request_handler.rs` 接入 `ContainerOpen` C2S；`LootContainerSourceKindV1::StorageCrate` / `DeadDrop` 与 serde/proto roundtrip pin 已落地；client 新增 `StorageCrateInteractIntentHandler`、`DeadDropInteractIntentHandler`、`ClientRequestSender.sendContainerOpen` 并注册到 `DefaultInteractionHandlers`。
- **P2 · 模型/渲染/视听**：`local_models/TradeCrate.bbmodel`、`HerbCrate.bbmodel`、`DeadDropBox.bbmodel` 及 `client/src/main/resources/assets/bong/geo/*.geo.json` / `textures/entity/*_intact.png` / `animations/*.animation.json` 已导出；`BongEntityModelKind.TRADE_CRATE/HERB_CRATE_PLACED/DEAD_DROP_BOX` raw_id 166/167/168、`TradeCrateRenderer` / `HerbCratePlacedRenderer` / `DeadDropBoxRenderer` 与 `BongEntityRenderBootstrap` 注册已落地；`container_place*`、`container_open*`、`container_break` audio recipe 和放置尘土粒子已接入。
- **P3 · 死信箱阵法防砸**：`DeadDropWard { owner, ward_active }` 放置默认激活；非 owner 破坏触发化灰、session 移除、打开者 `ContainerDestroyed` close、3.0 格水平 AoE `CombatEvent` + `ApplyStatusEffectIntent(Slowed)`；`server/src/network/gameplay_vfx.rs` 暴露 `bong:dead_drop_ward_break`，`server/assets/audio/recipes/dead_drop_ward_break.json` 与 `client/src/main/java/com/bong/client/visual/particle/DeadDropBreakPlayer.java` 已完成破阵 VFX/SFX。

### 关键 commit / PR

- **P0**：`95333d313af4462276e12b4765228ef0b9aa9dec`（2026-06-14）`plan-placeable-container-blocks-v1 P0：接通可放置容器底盘`，PR #549。
- **P1**：`9b4c69d0f06bd57bb73dce066802a3a0e4f34bdc`（2026-06-14）`plan-placeable-container-blocks-v1 P1：通用容器打开链路`，PR #551。
- **P2**：`448bd60e2597ff04fede695155d68bf70c4245f9`（2026-06-14）`plan-placeable-container-blocks-v1 P2：接入容器模型与视听`，PR #552。
- **P3**：`a33a308bb84eb6c9662eeea9f54b91e2a2c93207`（2026-06-14）`plan-placeable-container-blocks-v1 P3：死信箱阵法防砸`，PR #553。

### 测试结果

- **P0 本地**：`cargo test container_block`、`cargo test block_place`、`cargo test placeable_container_templates_load_from_workbench_materials_toml`、`cargo test loot_container_close`、`./gradlew --max-workers=2 test --tests "com.bong.client.network.LootContainerHandlerTest"`、`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`、`./gradlew --max-workers=2 test build` 全绿；PR #549 `e2e` check SUCCESS。
- **P1 本地**：`cargo test container_open`、`cargo test external_kind_to_source_kind`、`cargo test lifecycle_manages_only_supply_coffins`、`cargo test loot_container_source_kind`、`cargo test c2s_container_open`、`cargo test client_request`、server full `cargo test`（9064 passed, 1 ignored，`full_app_startup` 1 passed）、client protocol / sender / intent handler / default handler / loot handler 测试与 `./gradlew --max-workers=2 test build` 全绿；PR #551 `e2e` check SUCCESS。
- **P2 本地**：`./gradlew --max-workers=2 test build`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test container_block`、`cargo test entity_model`、`cargo test audio`、`cargo test world::block_place::tests::breaking_`、server full `cargo test`、`git diff --check origin/main...HEAD` 全绿；PR #552 `e2e` 与 `Build resource pack` check SUCCESS。
- **P3 本地**：`cargo fmt --check && CARGO_BUILD_JOBS=2 cargo clippy --all-targets -- -D warnings`、server full `cargo test`（9074 passed, 0 failed, 1 ignored，`full_app_startup` 1 passed）、`./gradlew --max-workers=2 test build`、`git diff --check origin/main..HEAD` 全绿；review fix 后 `cargo test dead_drop -- --nocapture`、`DeadDropBreakPlayerTest`、`VfxRegistryTest` 全绿；PR #553 `e2e` check SUCCESS。

### 跨仓库核验

- **server**：`ExternalContainerKind::StorageCrate` / `DeadDrop`、`external_kind_to_source_kind`、`ContainerOpenRequest`、`LootContainerSourceKindV1::StorageCrate` / `DeadDrop`、`ContainerBlockKind::DeadDrop`、`DeadDropWard`、`DEAD_DROP_WARD_BREAK`、`dead_drop_ward_break` audio recipe 均在运行路径有消费者。
- **client**：`BongEntityModelKind.TRADE_CRATE` / `HERB_CRATE_PLACED` / `DEAD_DROP_BOX`、三 renderer、`StorageCrateInteractIntentHandler`、`DeadDropInteractIntentHandler`、`DeadDropBreakPlayer.EVENT_ID`、`VfxBootstrap` 注册与对应测试均已落地。
- **proto/schema**：`ContainerOpen` C2S、`LootContainerSourceKindV1`、`LootContainerCloseReasonV1::ContainerDestroyed` 的 wire-format / roundtrip pin 已覆盖；agent TypeScript schema 无 loot container 对端，按 plan 判定无 agent 改动。
- **资产**：三 `.bbmodel`、三 geo、三 texture、三 animation、四类容器音频 recipe、P2 render preview 均已提交；P2 终轮 commit 含 `<PROMISE>` 三轮打磨担保。

### 遗留 / 后续

- `PackContainer*` 统一内容操作协议仍归 [[plan-nested-pack-base-v1]] 后续升级；本 plan MVP 按决议继续复用 `ExternalContainerMove` / `ExternalContainerClose`。
- 死信箱完整「破解阵法后无害破坏」小游戏未纳入 P3，后续若做应新立 plan；当前 MVP 是 owner 正常破坏、非 owner 化灰 + 毒气。
- 草药箱筛选、保鲜、shelflife 与高灵物逸散归 [[plan-container-filter-and-completion-v1]]，本 plan 仅落地可放置世界容器与视听闭环。
