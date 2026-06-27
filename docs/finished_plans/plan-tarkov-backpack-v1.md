# plan-tarkov-backpack-v1 — 塔科夫式套包系统（嵌套容器 + 上身渲染）

> **主题**：把已落地的 `plan-layered-equip-v1` / `plan-backpack-equip-v1`（身体槽 worn 层 + `pack_<instance_id>` 容器命名约定）升级为塔科夫式套包——非空背包可连货整体卸下、卸下后内含物 spill / overflow 安全掉落、双击打开穿戴背包件的内含物视图、重量递归上卷、拖入持久化、穿戴背包件在玩家身上渲染。**不重造 equipped 模型，是其增强**。
>
> **核心红线（对抗 islands#1 / verifiable#1 / scope#1 已坐实）**：`rebuild_containers_from_equipment`（`server/src/inventory/mod.rs:3813`）唯一生产调用者是 `handle_backpack_break`（`mod.rs:3996`），而 `handle_backpack_break` 在整个 `client_request_handler.rs` 中**零生产调用**（grep 验证）。`apply_inventory_move`（`mod.rs:3007`）只做 detach→attach→bump_revision，**从不触发 rebuild**。因此 P0 删除「非空拒卸」后，spill/overflow/掉落链路在当前架构里是**未接线孤岛**——P0 的首要交付物是把 rebuild + overflow→掉落事件接进 move 路径，而非「复用同款掉落链路」的模糊描述。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 嵌套数据模型（`owner_instance_id`）+ 移除非空拒卸 + **把 rebuild+overflow→掉落接进 move 路径** + 多背包重映射 + 持久化回填（回填先于孤儿检测） | ✅ | 2026-06-27 |
| P1 | 重量递归上卷 pin 测试（含 orphan double-count 边界）+ 背包自重占上限语义收口 | ✅ | 2026-06-27 |
| P2 | 跨包/包内移动 + 拖入持久化 + 穿戴态门控（server+client 双侧）+ schema 漂移修复（仅 TS）+ 超限软门控固化 | ✅ | 2026-06-27 |
| P3 | client 双击打开穿戴背包件容器视图（`WornContainerPanel`，发包走 `sendInventoryMove`）+ `owner_instance_id` 全栈下发（单 PR） | ✅ | 2026-06-27 |
| P4 | 背包上身渲染（**先 GeckoLib 4.x player-attach API spike → 选型 → 接线**，TPV） | ✅ | 2026-06-27 |
| P5 | 视听反馈（卸包/装包/拖入差异化）+ 平衡（容量/自重参数 + 嵌套深度 2 层封顶固化） | ✅ | 2026-06-27 |

---

## 接入面（坐实）

### 进料（已落地，本 plan 站在其上）
- `plan-layered-equip-v1` / `plan-backpack-equip-v1`：身体槽 worn 层（`SlotContents.worn: Vec<ItemInstance>` LIFO 栈，`server/src/inventory/mod.rs:557`）；`pack_<instance_id>` 容器命名约定（`container_id_for_worn_pack` / `worn_pack_instance_from_container_id`，`mod.rs` 内）。
- `rebuild_containers_from_equipment`（`mod.rs:3813`）：扫 worn 层 container_spec 件确保 `pack_<id>` 容器存在、孤儿 spill、刷新 `max_weight`；返回 overflow `Vec<ItemInstance>`。
- `handle_backpack_break`（`mod.rs:3996`）：当前唯一调 rebuild 的封装，返回 `BackpackBreakOutcome.spilled_items`；**注意：本函数当前无任何生产调用者**（仅测试引用，见 `mod.rs:11421+`）。
- `instantiate_inventory_from_loadout`（`mod.rs:1109`）：静态占位 `LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID = "pack_grass_pouch"`（`mod.rs:98`，**仅单个占位**）→ 运行时按 `first_worn_pack_instance`（`mod.rs:1137`）单件重映射。

### 出料 / 跨仓库契约 symbol
| 方向 | symbol | 文件 |
|------|--------|------|
| server→client S2C | `InventorySnapshotV1` / `ContainerSnapshotV1` | `server/src/schema/inventory.rs:240`、`server/src/network/inventory_snapshot_emit.rs:163` |
| server→client S2C 范式参考 | `loot_container_open` / `loot_container_update`（背包件走 `inventory_snapshot`，不另起 session） | `client/.../network/LootContainerHandler.java` |
| client→server C2S | `InventoryMoveIntent`（`container_id` open string，携带 `pack_<id>`；handler `handle_inventory_move` `client_request_handler.rs:9742`） | `server/src/schema/client_request.rs:331`、`agent/packages/schema/src/client-request.ts:173` |
| client→server C2S（不复用） | `ExternalContainerMove`（loot 走此路 + `session_id`，背包件**禁止**走此路） | `client_request_handler.rs` 的 external-container 分支 |
| schema TS | `ContainerSnapshotV1` / `ContainerIdV1` / `InventorySnapshotV1` | `agent/packages/schema/src/inventory.ts:14,311,323` |
| schema sample | `inventory-snapshot.sample.json` / `server-data.inventory-snapshot.sample.json` / `client-request.inventory-move-intent.sample.json` | `agent/packages/schema/samples/` |

### 共享类型 / 关键 symbol
- Rust：`ContainerSpec`（`mod.rs:211`）、`ContainerState`（`mod.rs:392`）、`PlacedItemState`（`mod.rs:401`）、`ItemInstance`（`mod.rs:408`）、`SlotContents`（`mod.rs:557`）、`LoadoutSpec`（`mod.rs:383`）、`PlayerInventory`（`mod.rs:664`）、`apply_inventory_move`（`mod.rs:3007`）、`validate_move_semantics`（`mod.rs:4440` 附近）。
- 重量：`calculate_current_weight`（`mod.rs:3730`，三路 flat 求和：equipped + container + 其他，不重叠）、`compute_max_weight`（`mod.rs:3791`，`#[allow(dead_code)]`）、`worn_container_items`（`mod.rs:3770`）、`sync_overloaded_marker`（`mod.rs:4186`）、`OverloadedMarker`（`mod.rs:735`）、`BASE_CARRY_CAPACITY=15.0`（`mod.rs:107`）、`body_mass.rs:59`（`carried_mass`）。
- 持久化：`flush_changed_player_inventories`（`server/src/player/mod.rs:754`）、`persist_player_inventory_slice_in_sqlite`（`state.rs:1785`）、`load_player_inventory_from_sqlite`（`state.rs:1120`）、`inventory_has_orphan_pack_container`（`state.rs:1101`）、`INVENTORY_SCHEMA_VERSION=2`（`state.rs:36`）。
- client：`InspectScreen.java`（双击 / drop 分支 L1871/L1987/L2231）、`LootContainerPanel.java`（**仅复用 owo 布局 + dispose 模式**）、`EquipmentPanel.java` / `EquipSlotComponent.java`、`InventoryEquipRules.isContainer()`（`InventoryEquipRules.java:281`）、`BackpackGridPanel`、`DragState`、`InventoryStateStore`、`ClientRequestSender.sendInventoryMove`（`ClientRequestSender.java:107`）、`InventorySnapshotHandler.parseContainers`。
- 渲染：`ArmorRenderBootstrap`（`client/.../armor/ArmorRenderBootstrap.java:28-34`，**唯一已验证可用的 `LivingEntityFeatureRendererRegistrationCallback` 注册先例**）、`ArmorFeatureRenderer`（`armor/ArmorFeatureRenderer.java:33`，SML/OBJ 管线）、`MutationFeatureRenderer`（`dandao/MutationFeatureRenderer.java`，**已知未注册孤岛 + GeckoLib sub-model 渲染留空**，作反面教材）、`WhaleModel.java`（GeckoLib **entity** 用法参考：`GeoModel<T extends GeoAnimatable>` + `GeckoLibUtil.createInstanceCache`）、`BongClient.java:139`、geo 资产 `grass_pouch_back.geo.json` / `grass_pouch_front.geo.json`（`client/src/main/resources/assets/bong/geo/`）、entity 贴图 `assets/bong/textures/entity/grass_pouch.png`。
- 版本：GeckoLib **4.4.9**（`client/gradle.properties:19`），Fabric MC 1.20.1。

### worldview 锚点
- §五《装备分层穿戴》`docs/worldview.md:552-558`：「容器按其形制穿在对应部位……穿上即随身储物，自重计入负重（§十七）」。**套包/嵌套语义已被正典覆盖**：背包是 worn 件、自重计入负重、按形制穿对应部位，无需新增正典条款（决议 #6）。**本 plan 不改 worldview.md**。若 P5 平衡阶段引入「背篓套腰囊」等多层嵌套形制术语需正典化，则**单独人工 PR 走人工审，agent 不得自动改 `worldview.md`**。

### qi_physics 锚点
- 与 qi 守恒无直接耦合：套包搬运/卸下不产生 / 消耗灵气，不入 qi_physics ledger。唯一接触点为 `lingering_owner_qi`（`ItemInstance` 字段，`mod.rs:408`）——物品携带的滞留 owner 灵气在跨容器移动时**随 instance 走，不重算、不复制、不蒸发**（移动是同一 instance 的位置变更）。P0 须有 pin 测试锁住「跨包移动后 `lingering_owner_qi` 守恒不变」。

---

## P0 — 嵌套数据模型 + 移除非空拒卸 + rebuild/overflow 接线 ✅ 2026-06-27

**目标**：建立背包件 ↔ 容器的语义归属字段；移除「非空拒卸」硬门；**把 `rebuild_containers_from_equipment` + overflow→掉落事件真正接进 move 路径**（修复孤岛红线）；多背包占位全量重映射；老存档无缝兼容（回填先于孤儿检测）。

### 决议依据（见 ## 决议 #1 / #2）
采用**方案 A：`ContainerState` 加 `owner_instance_id: Option<u64>`**，否决方案 B（内含物挂进 `ItemInstance`）。

### 交付物
1. **`ContainerState` 加字段**（`mod.rs:392`）：
   ```rust
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub owner_instance_id: Option<u64>,
   ```
2. **`rebuild_containers_from_equipment`**（`mod.rs:3813`）：创建/刷新 `pack_<id>` 容器时写 `owner_instance_id = Some(instance_id)`。
3. **移除非空拒卸分支**（`validate_move_semantics`，`mod.rs:4440` 附近）：删除 `!container.items.is_empty()` 返回 `Err` 的判断 + 同步删除/更新关联的 `plan-layered-equip-v1 P0.2` 注释。
4. **【红线】rebuild + overflow→掉落接进 move 路径**（`handle_inventory_move` `client_request_handler.rs:9742`）：
   - 在 move 成功后，识别「卸下 worn 背包件」语义：`from` 为 `Equip{Worn}` 且被移走 instance 的 `container_spec.is_some()`。
   - 命中时**显式调用 `rebuild_containers_from_equipment(inventory, registry)`**（不再依赖任何不存在的 Bevy auto-system），取其返回的 overflow `Vec<ItemInstance>`。
   - 将 overflow 逐件转为掉落物事件（用 `handle_backpack_break` 同款 `DroppedItemEvent` 链路 / `BackpackBreakOutcome.spilled_items` 同款发送），**禁止静默丢失**。
   - 交付物必须落到具体调用层：rebuild 与 overflow 处理写在 `handle_inventory_move` 的 worn-pack 卸下分支（或抽 `apply_inventory_move` 内联同等逻辑并把 overflow 附到 `InventoryMoveOutcome`），二选一并在交付物里写明选哪个。
   - **同步**：穿背包路径（`to` 为 `Equip{Worn}` 且 instance 有 container_spec）也须触发 rebuild，确保 `pack_<id>` 容器被即时创建并出现在下一帧 snapshot（否则 P3 双击无容器可开）。
5. **多背包重映射 + loadout 占位策略**（`instantiate_inventory_from_loadout` `mod.rs:1137-1168`，决议 #2 衔接 scope#2）：
   - 把单件 `first_worn_pack_instance` 重映射改为**遍历所有 worn container_spec 件**逐一映射占位容器。
   - **明确占位策略**：`LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID` 当前仅 `"pack_grass_pouch"` 单个占位。若 loadout 含多背包，须**动态新建**第 2+ 个 `pack_<instance_id>` 容器（不依赖 toml 预配多占位）：第一个 worn pack 复用占位 id 重映射，其余 worn pack 由 rebuild 路径新建容器。交付物写明：「占位仅服务第一个 worn pack，其余 worn pack 走 `rebuild_containers_from_equipment` 动态建容器」。
6. **持久化回填（顺序硬约束）**（`load_player_inventory_from_sqlite` `state.rs:1120`，决议 #1 衔接 verifiable#2）：
   - 反序列化后**先回填**：遍历 containers，对 `pack_<id>` 且 `owner_instance_id == None` 者，用 `worn_pack_instance_from_container_id` 解析前缀回填 `Some(instance_id)`。
   - **回填必须先于孤儿检测**。`INVENTORY_SCHEMA_VERSION` **不 bump**（`serde(default)` 旧存档读为 `None`，内存层回填，无 SQL migration）。
   - 语义明确：回填是**纯内存层每次加载重算**（不写回 DB），下次加载从前缀重新解析，幂等。
7. **孤儿检测同步**（`inventory_has_orphan_pack_container` `state.rs:1101`，衔接 verifiable#2）：在**回填后**运行；判据保持「`pack_<id>` 容器的 owner instance 不在任何身体槽 worn 层」。`owner_instance_id` 已回填，前缀路径与 owner 路径结果一致，避免误判合法新格式容器、防 #736 污染误删存档。

### 测试（`inventory::*` / 集成 ≥ 9，含 e2e）
- 删 `validate_move_semantics_rejects_unequip_backpack_when_container_nonempty`（`mod.rs:11048`）→ 改为 `validate_move_semantics_allows_unequip_backpack_when_container_nonempty`（断言返回 Ok）。
- 保留 `validate_move_semantics_allows_unequip_backpack_when_container_empty`。
- `rebuild_sets_owner_instance_id_on_pack_container`：穿背包后 `pack_<id>.owner_instance_id == Some(id)`。
- `unequip_nonempty_backpack_spills_contents_into_other_container`：内含物 spill 进存活容器。
- `unequip_nonempty_backpack_overflow_drops_items_not_lost`：目标容器满 → overflow 转掉落事件（断言 spilled count 守恒、非空）。
- `equip_pack_creates_pack_container_via_rebuild`：穿包后 `pack_<id>` 容器即时存在（验证 #4 穿装 rebuild 接线）。
- `instantiate_remaps_all_worn_pack_placeholders`：多背包 loadout——第一件复用占位、其余动态建容器，全部容器 id 正确（loadout fixture 预置两件 worn pack）。
- `load_backfills_owner_instance_id_for_legacy_pack_container`：旧存档（无字段）加载后回填正确，且回填发生在孤儿检测前（断言旧 `pack_<id>` 容器未被误删）。
- `orphan_detection_runs_after_backfill_no_false_positive`：合法新格式容器在回填后不被误判孤儿。
- `move_item_across_packs_preserves_lingering_owner_qi`：qi_physics 锚点，跨包移动 `lingering_owner_qi` 守恒。
- **e2e（`server/tests/`）** `e2e_unequip_nonempty_pack_drops_overflow_not_lost`：经 `handle_inventory_move` 真路径卸非空背包 → rebuild 触发 → overflow 进掉落事件队列（不绕过 handler 直测内部，锁住接线）。

### 跨端契约
`owner_instance_id` 本阶段**仅服务端内部使用，不下发**（client 可从 `pack_` 前缀反解）。`ContainerSnapshotV1` 暂不加字段，避免 `#[serde(deny_unknown_fields)]`（`schema/inventory.rs:240`）触发老客户端反序列化报错。下发推迟到 P3 全栈单 PR（决议 #4）。

---

## P1 — 重量递归上卷 ✅ 2026-06-27

**目标**：用 pin 测试锁住「flat 求和 == 递归上卷」的数学等价性，**包含 orphan double-count 危险边界**（衔接 verifiable#4），并固化「背包自重是否占负重上限」语义。

### 决议依据（见 ## 决议 #3）
- `calculate_current_weight`（`mod.rs:3730`）三路 flat 求和经核实**不重叠**：背包自重走 equipped（worn 件），内含物走 container（`pack_<id>.items`），背包件本身不在任何 `ContainerState.items` 里。**签名不改**。
- 决议 #3：采用「**背包自重不额外占上限**」——`compute_max_weight = BASE + Σ worn 背包 weight_capacity`；背包自重已在 `current_weight` 计一次，不在 `max` 侧二次扣减。本阶段以测试固化，不改公式。
- **危险边界**（verifiable#4）：若背包件被卸入 `body_pocket` / 另一容器而其旧 `pack_<id>` 容器未被 rebuild 清除（P0 修复前的可达状态），背包件自重走 container_weight、孤儿容器内含物也走 container_weight，需测试如实反映「rebuild 前如实计入 / rebuild 后清除不再 double-count」。

### 交付物
- **不改** `calculate_current_weight` / `compute_max_weight` 公式（决议固化现状语义）。
- `#[allow(dead_code)]` on `compute_max_weight`（`mod.rs:3791`）：本阶段加 pin 测试后**保留该 allow**（仍由 `rebuild_containers_from_equipment` 间接驱动 `max_weight`，非直接调用），交付物显式声明保留，避免 consume agent 误删触发 `-D warnings`。
- 说明：`max_weight` 由 `rebuild_containers_from_equipment` 一次性刷新；磨损降容量留 P5。

### 测试（`inventory::*` ≥ 7 pin 测试）
- `calculate_current_weight_counts_item_in_nested_container`：外层 worn 包 + grid 内物品 → `current = 包自重 + 内物品自重`。
- `calculate_current_weight_no_double_count_for_worn_pack`：worn 背包自重不被 container_weight 重复计（穿戴态）。
- `calculate_current_weight_after_unequip_pack_no_double_count_orphan_container`（verifiable#4 危险边界）：背包卸入 body_pocket 但 orphan 容器未清 → 断言 rebuild 前如实计入 orphan 内含物；调 `rebuild_containers_from_equipment` 后 orphan 清除、不再 double-count（状态转换锁定 P0 修复语义）。
- `compute_max_weight_no_backpacks_returns_base`（已有 `mod.rs:10827`，保留）。
- `compute_max_weight_worn_pack_self_weight_not_added_to_max`：固化决议 #3。
- `overloaded_marker_triggers_when_nested_pack_contents_exceed_limit`：内含物超限 → marker 挂上。
- `overloaded_marker_clears_after_removing_nested_item`：状态转换 A→B→A。
- `body_mass_carried_mass_consistent_with_current_weight`（`body_mass.rs:59`）：**纯防回归 pin 测，本阶段不改 body_mass 代码**（衔接 scope 轻伤 B，仅锁一致性）。

### 跨端契约
`InventoryWeightV1.current/max`（`inventory_snapshot_emit.rs:229`）语义不变，client 显示不改；P3 的「包重 vs 总重」由 client 本地从 snapshot 累加（见 P3）。

---

## P2 — 跨包/包内移动 + 拖入持久化 + 穿戴态门控 ✅ 2026-06-27

**目标**：物品可拖入任意穿戴中 `pack_<id>` 容器并持久化；新增「目标背包件必须穿戴」门控（**server + client 双侧**，衔接 islands#3）；schema 漂移修复（仅 TS）；超限软门控固化（决议 #5）。

### 交付物
1. **拖入持久化路径核实**（`handle_inventory_move` `client_request_handler.rs:9742`）：`to.container_id = "pack_<id>"` 已可路由（线性搜 `PlayerInventory.containers`）；持久化经 `flush_changed_player_inventories`（`player/mod.rs:754`）自动落盘，无额外入口。
2. **穿戴态门控（server 侧）**（`validate_move_semantics`，决议 #2/#5）：`to` 为 `Container{container_id:"pack_<id>"}` 时，校验该容器 `owner_instance_id`（P0 已回填）对应 instance 当前在某身体槽 worn 层；非穿戴（已卸到手持/格子的背包）拒绝拖入，返回带修复线索 Err：「背包未穿戴，无法放入内含物」。
3. **穿戴态门控（client 侧，衔接 islands#3）**：`InspectScreen.attemptDrop()` 的容器 drop 分支（L2274-2285）增门控——`containerId.startsWith("pack_")` 时查 `InventoryStateStore.snapshot().equippedSlots()` 校验对应 worn 件存在，否则不乐观落位 + 给 toast（避免「拖进去瞬间弹回且无提示」）。**方案二选一并写明**：要么 client 门控，要么 server 下发 snapshot 前 rebuild 清掉已卸背包容器使其不出现在 UI tab；本 plan 选 client 门控（server snapshot 仍含穿戴中容器）。
4. **超限拒绝（决议 #5：软门控）**：维持现状——超重仅打 `OverloadedMarker`（`sync_overloaded_marker` `mod.rs:4186`），**不拒绝拖入/拾取/移动**。本阶段仅补测试固化契约，硬门控留独立 plan。

### 测试（`inventory::*` ≥ 6 + e2e）
- `move_item_into_worn_pack_container_succeeds`（happy）。
- `move_item_into_unworn_pack_container_rejected`（错误分支）。
- `move_item_into_nonexistent_pack_container_rejected`（错误分支）。
- `move_item_between_two_worn_packs_succeeds`（跨包，状态转换）。
- `move_into_pack_when_overloaded_still_succeeds`（决议 #5 软门控）。
- `move_into_full_pack_rejected_no_fit`（边界：目标容器满）。
- **e2e** `e2e_drag_item_into_pack_persists_across_reload`：move intent → 落盘 → 重载 → 物品仍在 `pack_<id>`。

### 跨端契约
- C2S `InventoryMoveIntent`（`client-request.ts:173` / `client_request.rs:331`）契约不变，`container_id` open string 已支持 `pack_<id>`。
- **schema 漂移修复（仅 TS 侧，衔接 verifiable#3 / scope#3）**：
  - **Rust `ContainerIdV1 = String`（`schema/inventory.rs:13`）已是开放字符串，无需修改**——server 反序列化 `pack_<id>` 本就通过，agent 实施时**禁止动 Rust 侧 ContainerIdV1**（避免误触 deny_unknown_fields）。
  - TS `ContainerIdV1`（`inventory.ts:14-18`，当前 3-literal union `main_pack`/`small_pouch`/`front_satchel`）→ `Type.String({ pattern: "^(body_pocket|pack_\\d+)$", minLength:1, maxLength:64 })`（保留 `body_pocket` + `pack_<数字>` 形态，比裸 `Type.String()` 更紧、不丢全部静态安全；衔接 verifiable#3）。
  - `containers` 约束（`inventory.ts:323`，当前 `minItems:3,maxItems:3`）→ `minItems:1,maxItems:16`（对齐 Rust `INVENTORY_CONTAINER_MAX=16`；下限 1 因 `body_pocket` 恒存）。
  - sample 对拍：`inventory-snapshot.sample.json` / `server-data.inventory-snapshot.sample.json` 加 `pack_1007` 容器 + `placed_items`；`client-request.inventory-move-intent.sample.json` 加 `container_id:"pack_1007"` 的 from/to 案例。
  - **`npm run build -w @bong/schema` 重建 dist**（衔接「pull 后重建 schema dist」坑）+ `cd agent/packages/schema && npm test` 双端对拍，列为 P2 CI gate 交付物。
- **天道 agent 验证**（衔接 islands#9）：确认天道 agent 的 `InventoryMoveIntent` 处理不对 `container_id` 做 literal enum 白名单过滤、不会静默丢弃 `pack_<id>` move 指令（grep agent handler，写入交付物核验步骤）。

---

## P3 — client 双击打开穿戴背包件容器视图 ✅ 2026-06-27

**目标**：双击装备槽内穿戴的背包件，在 InspectScreen 右侧挂出其内含物视图，支持拖入拖出。`owner_instance_id` 全栈下发（单 PR 全栈落地）。

### 决议依据（见 ## 决议 #4）
方案 A（client 本地渲染）：背包件容器已在 `InventorySnapshot` 里（P0 穿装即建容器、下发 snapshot），双击直接渲染本地状态，**无 C2S round-trip**；拖入拖出走已有 `sendInventoryMove`。

### 交付物
1. **【发包路线硬约束，衔接 islands#4 / scope#4】**：`WornContainerPanel` **仅复用 `LootContainerPanel` 的 owo 布局结构（`BackpackGridPanel` + dispose listener 解绑模式）**；发包**强制走 `ClientRequestSender.sendInventoryMove`**（编码 `InventoryMoveIntent`），**禁止调用 `ClientRequestSender.sendExternalContainerMove`**（loot 专用，需 session_id，走 `ExternalContainerMove` handler——协议层根本不同，接错即孤岛）。交付物正文写明此禁令。
2. **`InspectScreen.java` 双击计时**：新增 `lastEquipClickTimeMs` / `lastEquipClickSlot` / `DOUBLE_CLICK_WINDOW_MS=400`；在 `mouseClicked` button==0 的 EquipSlot 分支（L1871）判双击 + `InventoryEquipRules.isContainer(top)` → `openWornContainerPanel(top, slotType)`。owo 无 clickCount，计时在 Screen 级 `mouseClicked()` 手算。
3. **新建 `WornContainerPanel.java`**：以 `containerId + InventoryModel` 驱动，挂入 `outerRow`。
   - **snapshot 订阅接入点（衔接 islands#5）**：数据源是全局 `InventoryStateStore`（非 loot 专属 store）。交付物必须明确 `WornContainerPanel` 在 `build()` 时向 `InventoryStateStore` 注册 snapshot 变更 listener（核实 `InventoryStateStore` 的 listener 接口形态；若无现成接口则参 `InventorySnapshotHandler` 回调模式接一个），`dispose()`/`removed()` 时解绑，保证 server resync 后 panel 刷新、显示态不与实际态发散。**禁止留 stub**。
4. **`InspectScreen.attemptDrop()`（L2231，loot 分支后、activeGrid 分支前）**：增 `wornContainerPanel` 识别 → `wg.canPlace` → `wg.place` → `dispatchMoveIntent(dragged, fromLoc, ContainerLoc(wg.containerId(), row, col))`（走 `sendInventoryMove`）。
5. **pickup 分支**（`mouseClicked` L1987 loot 分支后）增 wornContainerPanel 识别。
6. **`removed()`（L197）** 增 `wornContainerPanel.dispose()`；**`tick()`（L1699）** 增双击 timer 维护（如需）。
7. **包重 vs 总重显示**（决议 #3）：`WornContainerPanel` 标题区显示「包重 = 包自重 + 该容器内含物递归和」（client 本地从 snapshot 该容器 `placed_items` 累加）；InspectScreen 状态条显示「总重 = `InventoryWeightV1.current`」。
8. **`InventoryEquipRules.isContainer()`（L281）** 白名单随背包件 template_id 同步（`worn_grass_pouch`/`grass_pouch`）。
9. **`owner_instance_id` 全栈下发（决议 #4，衔接 islands#6 / verifiable#5 —— 单 PR 全栈，消除版本错配）**，落地顺序在同一 PR 内：
   - Rust `ContainerSnapshotV1`（`schema/inventory.rs:240`）加 `#[serde(default, skip_serializing_if="Option::is_none")] pub owner_instance_id: Option<u64>`（`deny_unknown_fields` 已在，需 `default` 容旧 sample）。
   - `build_inventory_snapshot`（`inventory_snapshot_emit.rs:163`）填 `worn_pack_instance_from_container_id(&container.id)`。
   - TS `ContainerSnapshotV1`（`inventory.ts:311`）加 `owner_instance_id: Type.Optional(SafeIntegerV1)`；sample 加字段；`npm run build -w @bong/schema` + `npm test`。
   - client `InventorySnapshotHandler.parseContainers` 解析新字段（缺省兼容）。**核实 Java JSON 解析器对 unknown/缺失字段的容忍**（Gson 默认忽略未知字段；确认 `parseContainers` 不 `failOnUnknownProperties`，写入核验步骤）。
   - **全栈同一 PR 落地，无跨 PR 版本错配窗口**（衔接 verifiable#5）。

### 测试
- client `./gradlew test`：`WornContainerPanelTest` —— containerId 驱动 populate、canPlace 边界、`InventoryStateStore` listener 注册/dispose 解绑（断言解绑后不再收回调）。
- 双击计时单测：400ms 窗口内同槽两击触发、超窗不触发、不同槽不触发（状态转换）。
- 拖入分支单测：drop 到 wornContainerPanel grid → 发出 `inventoryMove` intent（断言 from/to payload 结构 + **断言走 sendInventoryMove 而非 sendExternalContainerMove**）。
- `owner_instance_id` 解析单测：含/缺字段两路兼容。
- e2e 手验（验收记录入后续 Finish Evidence）：穿背包 → 双击打开 → 拖入物品 → snapshot 回刷 → 重连仍在。

### 跨端契约
复用现有 `InventoryMoveIntent`，无新 C2S 类型；`owner_instance_id` 新增字段全栈单 PR 落地（见交付物 #9）。

---

## P4 — 背包上身渲染（破草包 TPV） ✅ 2026-06-27

**目标**：穿戴的背包件（破草包）在玩家身上（TPV / F5）渲染，补齐「配了 geo 模型却没上身」缺口。

### 资产核实结论
- geo 资产**存在**：`grass_pouch_back.geo.json`（`geometry.bong.grass_pouch_back`，背面）、`grass_pouch_front.geo.json`（前胸），贴图 `assets/bong/textures/entity/grass_pouch.png`（**非** GUI 图标）。格式 GeckoLib geo.json。**无需补模型**，纯接线。

### 决议依据（见 ## 决议 #5）+ 渲染技术风险（衔接 islands#7 / scope#5）
- **GeckoLib 版本 4.4.9，仓库内无任何「在 vanilla player `FeatureRenderer` 里驱动 GeckoLib `GeoModel`」先例**。`MutationFeatureRenderer`（`dandao/`）正因「GeckoLib sub-model attachment 渲染」无法直接落地而留空、且从未注册（已知孤岛反面教材）。仓库 GeckoLib 用法（`WhaleModel` 等）都绑 **entity**（`GeoModel<T extends GeoAnimatable>` + instance cache + controller），player `FeatureRenderer` 不是 `GeoAnimatable`、无 instance cache，**直接调 `GeoModel.getBone()` 在 render tick 拿不到正确 packed light / pose / state machine，会崩或静默无渲染**。
- **草案的「手动 getBone()」写法不存在先例、不可默认开箱可用**——本阶段**第一交付物是 API spike**。

### 交付物
1. **【P4 前置 spike，必须先做】GeckoLib 4.x player-attach 渲染选型**：调查并落定「MC 1.20.1 + GeckoLib 4.4.9 下给 player 叠 geo 附件的正确 API」。候选：
   - (a) `GeoArmorRenderer` / `GeoRenderLayer`（GeckoLib 4.x layer API）挂到 `PlayerEntityRenderer`；
   - (b) 把 `grass_pouch_back.geo.json` 预 bake 成 vanilla `ModelPart` / `BakedModel`，走纯 vanilla `MatrixStack` + `renderLayer`（参 `MutationFeatureRenderer` 的 vanilla overlay 思路，不用 GeckoLib runtime）；
   - (c) 降级 flat quad。
   - 交付物必须写明：**「spike 结论 = X，最终渲染路线 = Y」**，不得保留「手动 getBone」假设。
2. **新建 `WornPackFeatureRenderer.java`**（`client/.../armor/`，按 spike 选型实现）：继承 `FeatureRenderer<AbstractClientPlayerEntity, PlayerEntityModel<...>>`；`render()` 读 `InventoryStateStore.snapshot().equippedSlots().get(EquipSlotType.CHEST).worn()`，**按 `container_spec != null` 过滤**（衔接 verifiable 轻伤——`container_spec` 存在判定，与 `InventoryEquipRules.isContainer()` L281 逻辑对齐，**不取 `category`**，category 是 display 分类）。
3. **`WornPackModelRegistry.java`**（或 renderer 内 hardcode）：`template_id → (geoPath/bakedModel, texturePath)` 映射（`worn_grass_pouch`/`grass_pouch`）。
4. **新建 `WornPackRenderBootstrap.java`**：`LivingEntityFeatureRendererRegistrationCallback.EVENT.register(...)` 挂到 `PlayerEntityRenderer`，**严格参 `ArmorRenderBootstrap`（`ArmorRenderBootstrap.java:28-34`，唯一已验证可用注册先例），勿参 `MutationFeatureRenderer`（未注册）**。
5. **`BongClient.java:139`** 紧跟 `ArmorRenderBootstrap.register()` 后加 `WornPackRenderBootstrap.register()`。
6. **挂点 + pivot 校准**：破草包默认挂**背面**（`grass_pouch_back.geo.json`，pivot Y=14 torso 中段，Z+3≈0.375 格），Bedrock geo 坐标 ↔ `PlayerEntityModel` torso 经 `MatrixStack` translate/scale 校准；真机 F5 目测调位（验收入后续 Finish Evidence）。前胸 front 变体留 P5。

### 测试
- client `./gradlew test build`：registry 映射 happy/缺失分支单测；feature renderer 过滤单测（CHEST worn 含护甲+背包，**只渲染 `container_spec != null` 件**）。
- 资源包 sha1（衔接坑）：本阶段仅新增 Java + 复用既有 geo，**不动 `assets/bong/` 资产**，预期无 sha1 变更；若 spike 选型 (b) 需新增 baked 资产或动 geo，**则必须同步 `resourcepack.rs` + committed manifest 的 sha1/size**（避免 Build resource pack CI 红），交付物写明。
- e2e 手验：F5 第三人称看到背包在背上、位置正确、不穿模。**仅 TPV**——FPV 看不到自己背包，无 FPV 入口（vanilla 行为正确）。

### 跨端契约
纯 client 渲染，无 server/schema 改动；数据源为 P0/P3 已下发的 `equipped` snapshot。

---

## P5 — 视听反馈 + 平衡 ✅ 2026-06-27

**目标**：卸包/装包/拖入差异化视听反馈 + 容量/自重/嵌套深度参数标定。

### 交付物
1. **音效/粒子（差异化，衔接 skill_av_wiring + skill_av_diff 硬约束）**：卸非空背包（落地音 + 物品散落粒子）、装包（布料窸窣音）、拖入背包（轻 thunk）——三类**各自差异化** animation/粒子/SFX，禁单方向 stub。server emit VFX/SFX event，client 注册消费。
2. **平衡参数标定 + 嵌套深度 2 层封顶（决议 #1）**：固化嵌套深度上限 = **2 层**（worn 包 → grid → 物品；放入 grid 的背包件不被 `rebuild_containers_from_equipment` 展开为容器，只展开 worn 件，数据模型天然封顶）。`core.toml` 标定破草包 `weight_capacity`/`base_weight`/rows×cols；更高阶套包（背篓/封灵匣）另标新模板。
3. **磨损降容量（可选）**：背包磨损是否降 `weight_capacity` 的设计决策；若做需 `rebuild_containers_from_equipment` 重算 `max_weight`（依赖 P1 rebuild 触发）。
4. **front 挂点变体（P4 延伸）**：若引入前挂形制容器，启用 `grass_pouch_front.geo.json`，按 template→挂点映射。

### 测试
- server VFX/SFX emit 单测（每类反馈差异化 payload，断言三类 payload 互不相同）。
- client 消费单测（音效/粒子注册命中）。
- 平衡回归：放入 grid 的背包件**不展开为可访问容器**的回归测试（固化 2 层封顶）。

### 跨端契约
- VFX/SFX event：server emit → client consume，schema sample 对拍（按 `server_data payload` 加字段清单：proto + 2 struct + 2 From + convert + emit + schema regenerate）。
- worldview：若 P5 引入新形制术语需正典化 → **单独人工 PR 走人工审，agent 不自动改 `worldview.md`**。

---

## 决议

### #1 — 嵌套深度上限 + 数据模型方案
**深度上限 = 2 层**（worn 背包 → 其 grid → 物品）。**数据模型采用方案 A：`ContainerState` 加 `owner_instance_id: Option<u64>`**。
- 理由：方案 A 最小改动面，是对现有 `pack_<id>` 命名约定的**语义增强而非替换**；`serde(default)` 旧存档读为 `None` 完全兼容，**无需 SQL migration / 不 bump `INVENTORY_SCHEMA_VERSION`**，回填在 `load_player_inventory_from_sqlite` 内存层每次加载重算（幂等，不写回 DB）。
- 否决方案 B（内含物挂进 `ItemInstance.container_contents`）：递归语义渗透到所有 inventory 操作（move/fit/attach/weight/snapshot），与 `ContainerState` 平展列表双轨并存造成模型分裂，`rebuild_containers_from_equipment` 同步极难维护，代价远大于收益。
- 深度 2 层由数据模型天然封顶：放入 grid 的背包件不会被 rebuild 展开为容器（只展开 worn 件），无 3 层可操作嵌套。P5 加回归测试固化。

### #2 — 非空背包卸下后去哪
**走正常 move 路径，由 `handle_inventory_move` 显式触发 `rebuild_containers_from_equipment`；内含物 spill 进存活容器，放不下的 overflow 转掉落物事件（连货掉地）**。门控：背包卸到手持/格子后变非穿戴状态，**不可再被拖入**新内含物（P2 双侧穿戴态门控）。
- 理由（衔接 islands#1 / verifiable#1 / scope#1 坐实）：`rebuild_containers_from_equipment` 当前唯一生产封装 `handle_backpack_break` **无任何生产调用者**，`apply_inventory_move` 从不调 rebuild。spill/overflow/掉落链路在 move 路径**完全未接线**。P0 必须在 `handle_inventory_move` 卸 worn-pack 分支显式调 rebuild + 处理 overflow→`DroppedItemEvent`，并以 e2e 锁住（不绕过 handler 直测内部）。掉地连货符合塔科夫式直觉。

### #3 — UI 包重 vs 总重
**总重 = server 下发 `InventoryWeightV1.current`（已是全层 flat 求和，三路不重叠，数学等价递归上卷）；包重 = client 本地从 snapshot 该 `pack_<id>` 容器 `placed_items` 自重累加 + 背包件自重**。`compute_max_weight` 维持「背包自重不额外占上限」。
- 理由：`calculate_current_weight` 三路 flat 求和已正确展开所有层，无需改签名；包重是 client 显示派生，无需 server 额外字段。verifiable#4 的 orphan double-count 危险边界由 P1 专项 pin 测试锁定（rebuild 前如实计入 / rebuild 后清除）。

### #4 — `owner_instance_id` 是否下发 client
**下发，P3 单 PR 全栈落地**。`ContainerSnapshotV1` 加可选 `owner_instance_id`，client 双击时直接读 owner 关联 worn 件，免前缀解析。
- 理由：便利字段。`deny_unknown_fields`（`schema/inventory.rs:240`）风险由「Rust schema 加字段(default 容旧) + TS schema + sample + dist rebuild + server emit + client 解析**全在同一 PR**」消解——P0 不下发（仅服务端内部），P3 全栈一并加，无跨 PR 版本错配窗口（衔接 islands#6 / verifiable#5）。核实 Java 解析器忽略未知字段配置写入交付物。

### #5 — 上身渲染挂点 + 超限拒绝策略
**挂点**：破草包默认挂**背面**（`grass_pouch_back.geo.json`），渲染路线由 **P4 前置 spike 落定**（GeckoLib 4.x player-attach 正确 API，候选 GeoRenderLayer / vanilla baked-model overlay / flat quad），**禁用草案的「手动 getBone」未验证写法**；新建独立 `WornPackFeatureRenderer`，严格参 `ArmorRenderBootstrap` 注册，仅 TPV。资产已存在无需补模型。
- 理由（渲染，衔接 islands#7 / scope#5）：仓库无 player `FeatureRenderer` 驱动 GeckoLib `GeoModel` 先例，`MutationFeatureRenderer` 为此留空且未注册。直接 `getBone()` 在 player feature renderer 会崩/静默。必须 spike 选型而非假设开箱可用。
**超限拒绝**：**保持软门控**——超重仅打 `OverloadedMarker` debuff，不拒绝拖入/拾取/移动。
- 理由（超限）：现状 `add_item`/`apply_move` 无重量门控，超重是 debuff 而非硬墙；改硬门破坏「捡了再丢」手感且需大面积加门控+测试，超出本 plan 核心诉求。塔科夫式重量惩罚由 debuff 表达，硬拒绝留独立 plan。

### #6 — 与 layered-equip 衔接 + worldview
套包是 `plan-layered-equip-v1` / `plan-backpack-equip-v1` worn 背包的**内含物视图升级 + 渲染补齐 + 移动语义放宽 + spill/overflow 接线**，**不重造 equipped 模型**：背包件仍在身体槽 worn 层（破草包 equip_slot=chest），`pack_<id>` 命名约定保留，仅增 `owner_instance_id` 语义字段。worldview §五已锚定「容器按形制穿对应部位、自重计入负重」，**本 plan 不改 worldview.md**；若 P5 引入多层嵌套/新形制术语需正典化，**单独人工 PR 走人工审（agent 不得自动改 `worldview.md`）**。

---

## §10 实施工作流（consume-plan 编排）

> 本 plan scope = 6 PR（P0..P5），按 `docs/CLAUDE.md §六` 写本节。consume-plan agent 跑这份 plan 时按 §10 执行。**所有开放问题已在 ## 决议 #1-#6 pre-P0 收口，可直接进 P0 实施**。

### §10.1 视觉资产多轮打磨（仅 P4）

P4「背包上身渲染」是视觉资产 TODO，强制 **3 轮自我打磨 + `<PROMISE>` 担保**（`docs/CLAUDE.md §6.1`）：
1. Round 1 first cut（spike 选型落定 + renderer 接线 + 注册）→ commit `(round 1/3)`
2. Round 2 自我 review（`render_*` 工具 / 真机 F5 截图验证挂点/比例/不穿模）→ 修 → commit `(round 2/3)`
3. Round 3 终轮（pivot 校准 + 与 geo 资产一致性）→ 修 → commit `(round 3/3)`，末尾写 `<PROMISE>...</PROMISE>` 块（拼写 PROMISE）。

P0-P3、P5（纯逻辑 / schema / UI 交互）**不适用多轮**，按常规 atomic commit + 测试全绿。P5 视听规格已内联在 P5 阶段块（音效/粒子三类差异化），实施时随 server 逻辑一并落地。

### §10.2 多 PR 序列化拆分点（依赖顺序，前一个 merge 后开下一个）

| PR | 阶段 | 范围 | 触及层 |
|----|------|------|--------|
| PR-1 | **P0** | 嵌套数据模型 `owner_instance_id` + 移除非空拒卸 + **rebuild/overflow→掉落接进 move 路径（红线）** + 多背包重映射 + 持久化回填（先于孤儿检测） | server only（+ e2e） |
| PR-2 | **P1** | 重量递归上卷 pin 测试（含 orphan double-count 边界）+ 自重占上限语义固化 | server only |
| PR-3 | **P2** | 跨包/包内移动 + 拖入持久化 + 穿戴态门控（server+client 双侧）+ **schema 漂移修复（仅 TS）** + 软门控固化 | server + schema(TS) + client |
| PR-4 | **P3** | client 双击打开 `WornContainerPanel`（发包走 `sendInventoryMove`）+ `owner_instance_id` **全栈下发（单 PR 全栈，消除版本错配）** | server + schema + client |
| PR-5 | **P4** | 背包上身渲染（先 GeckoLib 4.x player-attach spike → 选型 → 接线，TPV）— **3 轮 + PROMISE** | client only |
| PR-6 | **P5** | 视听反馈（卸/装/拖入差异化）+ 平衡（容量/自重 + 嵌套深度 2 层封顶固化） | server + client + schema |

- **PR 边界硬约束**：每个 PR 内 server emit 必有对应 client/schema consumer 接线（参 `feedback_spawn_chain_wiring`，禁 emit-only 孤岛）。
- **schema 改动**（PR-3 TS-only / PR-4 全栈）必须 `npm run build -w @bong/schema` 重建 dist + `cd agent/packages/schema && npm test` 双端对拍，列为该 PR CI gate（参 `project_schema_dist_rebuild`）。
- **worldview 不动**：本 plan 任何 PR 都不改 `docs/worldview.md`（决议 #6）。

### §10.3 PR 实施用独立 subagent（context 隔离，`docs/CLAUDE.md §6.4`）

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...本 PR 范围 + 必读 §10.1 多轮（仅 P4）+ 饱和化测试要求...\n\nultrathink"
)
```
- 主线只接收 subagent result（PR url + 摘要），不亲自跑实施，每 PR 后清理 context。
- 共享主 worktree（避免 nested worktree）；若并发环境需隔离则 `git worktree lock`（参 `feedback_consume_worktree_lock`）。
- subagent 只负责实施 + 提 PR，**不等 review**；等待逻辑归主线。

### §10.4 CodeRabbit / Pi 等待协议（`docs/CLAUDE.md §6.5`）

- `gh pr checks <PR>`：`pass`→merge；`pending`→`ScheduleWakeup 1200s` 等下回合；`fail`→按严重性桶处理。
- 修完 review 意见**必须重等 CR re-review**，不自判"我修好了应该过"（`feedback_wait_coderabbit_approve`）。
- 等 CodeRabbit + Pi agent（github-actions）两个 bot 都确认无阻塞、Pi 写 ✅ Approve 才合（CR 限流时以 e2e 绿 + 无 Major 收敛为准）。
- 每 PR 各自走完整等待协议，前一个未收敛不开下一个。

### §10.5 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan tarkov-backpack-v1` 后即可下班：主线串行跑 PR-1..PR-6，每个走「subagent 实施 → 等 CR/Pi → 修 → 重等 → merge」，全绿后在末 PR `git mv docs/plan-tarkov-backpack-v1.md docs/finished_plans/` 并填 `## Finish Evidence`（落地清单 / 关键 commit / 测试结果 / 跨仓库核验 / 遗留）。醒来看 plan 是否已在 `finished_plans/`。

---

## Finish Evidence

塔科夫式套包系统（嵌套容器 + 上身渲染 + 差异化视听）全 6 PR（P0-P5）落地，2026-06-27 归档。

### 落地清单（阶段 → 真实模块/文件）

- **P0 — 嵌套数据模型 + rebuild/overflow 接线**：
  - `server/src/inventory/mod.rs`：`ContainerState.owner_instance_id: Option<u64>`、`rebuild_containers_from_equipment`（写 `owner_instance_id`）、`rebuild_and_drop_overflow`（卸非空背包 overflow→`DroppedLootRegistry`）、`instantiate_inventory_from_loadout`（多背包重映射）。
  - `server/src/network/client_request_handler.rs`：`handle_inventory_move` worn-pack 穿/卸分支显式调 `rebuild_and_drop_overflow` + resync。
  - `server/src/player/state.rs`：`load_player_inventory_from_sqlite` 回填 `owner_instance_id`（先于孤儿检测）、`inventory_has_orphan_pack_container`。
  - `server/tests/tarkov_backpack_p0_e2e.rs`：经 `rebuild_and_drop_overflow` seam 锁卸非空背包 overflow 不丢失。
- **P1 — 重量递归上卷 pin**：`server/src/inventory/mod.rs`（`calculate_current_weight` / `compute_max_weight` 公式不改，pin 测试固化决议 #3 + orphan double-count 边界）。
- **P2 — 跨包移动 + 穿戴态门控 + schema 漂移修复（TS）**：
  - `server/src/inventory/mod.rs`：`validate_move_semantics` 穿戴态门控（`to=pack_<id>` 校验 owner worn）。
  - `client/src/main/java/com/bong/client/inventory/InspectScreen.java`：client 侧门控 + toast。
  - `agent/packages/schema/src/inventory.ts`：`ContainerIdV1` 收紧为 `^(body_pocket|pack_\d+)$` pattern、`containers` minItems:1/maxItems:16；sample 对拍。
- **P3 — 双击容器视图 + owner_instance_id 全栈下发**：
  - `client/.../inventory/WornContainerPanel.java`（双击打开、发包走 `sendInventoryMove`）、`InspectScreen.java`（双击计时 + drop/pickup 分支）、`InventoryEquipRules.isContainer()`。
  - `server/src/schema/inventory.rs`：`ContainerSnapshotV1.owner_instance_id`；`server/src/network/inventory_snapshot_emit.rs`：`build_inventory_snapshot` 填字段。
  - `agent/packages/schema/src/inventory.ts`：`ContainerSnapshotV1.owner_instance_id`；`client/.../InventorySnapshotHandler` 解析。
- **P4 — 背包上身渲染（TPV，spike→route b）**：`client/.../armor/WornPackFeatureRenderer.java`、`WornPackModelRegistry.java`、`WornPackRenderBootstrap.java`、`BongClient.java`（注册）；复用 `grass_pouch_back.geo.json` + entity 贴图，spike 结论 = vanilla baked-model overlay（不用 GeckoLib runtime）。
- **P5 — 视听反馈 + 平衡 + 2 层封顶**：
  - `server/src/network/gameplay_vfx.rs`：`INVENTORY_PACK_UNEQUIP/EQUIP/STOW` 常量、`PackMoveVfx` 枚举、`classify_pack_move`、`pack_move_request`（三类差异化 payload）+ pin 测试。
  - `server/src/network/client_request_handler.rs`：`handle_inventory_move` 经 `classify_pack_move` emit 差异化 `VfxEventRequest`。
  - `client/.../visual/particle/PackOperationVfxPlayer.java`（三 Kind 差异化粒子 + 内联 audio recipe）、`VfxBootstrap.java`（注册三路由）、`assets/bong/audio_recipes/inventory_pack_{unequip,equip,stow}.json`。
  - `server/src/inventory/mod.rs`：2 层封顶回归测试 `rebuild_does_not_expand_container_item_placed_inside_grid_two_layer_cap` + 平衡 sanity `grass_pouch_balance_values_parse_from_core_toml`。
  - `server/assets/items/core.toml`：破草包(8.0/3×3/自重0.25) / 小草包(10.0 升级款) 数值标定 + 2 层封顶注释。

### 关键 commit（hash + #PR + 一句话）

- 定稿 `7c3656758`（#758）— plan §决议 #1-#6 pre-P0 收口 + §10 工作流定稿。
- P0 `4b255485a`（#760）— 嵌套数据模型 `owner_instance_id` + 移除非空拒卸 + rebuild/overflow→掉落接进 move 路径 + 多背包重映射 + 持久化回填。
- P1 `efa6772d9`（#762）— 重量递归上卷 pin 测试（固化决议 #3，不改公式）。
- P2 `7a153389a`（#763）— 跨包/包内移动穿戴态门控（server+client）+ schema 漂移修复（仅 TS）。
- P3 `6f9bb2ceb`（#765）— 双击打开穿戴背包件容器视图 + `owner_instance_id` 全栈下发。
- P4 `c53d43eaa`（#773）— 破草包上身渲染（TPV，spike→route b vanilla baked-model overlay）。
- P5 本 PR（`feat/tarkov-backpack-p5`）— 卸/装/拖入三类差异化视听反馈 + 嵌套深度 2 层封顶固化 + 破草包/小草包平衡数值标定。

### 测试结果

- P0：`cargo test` 全绿（9909 → 含 9 inventory 单测 + e2e `e2e_unequip_nonempty_pack_drops_overflow_not_lost`）。
- P1：`cargo test` 全绿（+7 重量 pin 测试）。
- P2：`cargo test`（+6 移动门控 + e2e）+ `cd agent/packages/schema && npm test` 双端对拍全绿。
- P3：`cargo test` + schema `npm test` + `cd client && ./gradlew test` 全绿（含 `WornContainerPanelTest` + owner_instance_id 解析）。
- P4：`cd client && ./gradlew test build` 全绿（3014 测试，含 registry 映射 + feature renderer 过滤）。
- P5（本 PR）：
  - server `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：全绿（lib 9952 passed，含新增 `classify_pack_move_routes_each_branch_to_distinct_category` / `pack_move_request_payloads_are_mutually_distinct` / `pack_move_request_payloads_serialize_within_schema_contract` / `rebuild_does_not_expand_container_item_placed_inside_grid_two_layer_cap` / `grass_pouch_balance_values_parse_from_core_toml`；resourcepack 16 passed 含 `committed_manifest_matches_default_constants`）。
  - schema `npm run build -w @bong/schema && npm run generate -w @bong/schema && cd packages/schema && npm test`：726 passed（VFX 走 `bong:vfx_event` JSON 通道，复用既有 spawn_particle schema，无新字段；regenerate 后 generated/ 无漂移）。
  - client `./gradlew test build`（Java 17）：3022 passed / 0 failed（P4 3014 → +8：新增 `PackOperationVfxBootstrapTest` 注册+event_id 对齐 3 例、`PackOperationAudioRecipeAssetTest` 三类 recipe 资产对拍+互不相同 5 例）。
  - 资源包：新增 3 个 `audio_recipes/*.json` 进 audio 子包（file_count 51→54），`scripts/build-resourcepack.sh` 重建 → 同步 `client/resourcepack/manifest.json`（sha1 `b1c4e20…`、size 72279732）+ `server/src/network/resourcepack.rs` 常量；`python3 -m unittest scripts/test_build_resourcepack.py` 4 passed。

### 跨仓库核验（server / schema / client 命中 symbol）

- **server**：`ContainerState.owner_instance_id`、`rebuild_and_drop_overflow`、`rebuild_containers_from_equipment`、`validate_move_semantics`（穿戴态门控）、`gameplay_vfx::{classify_pack_move, pack_move_request, PackMoveVfx, INVENTORY_PACK_UNEQUIP/EQUIP/STOW}`、`handle_inventory_move`（emit 接线）。
- **schema**：`ContainerIdV1`（`^(body_pocket|pack_\d+)$` pattern）、`ContainerSnapshotV1.owner_instance_id`；VFX 复用 `VfxEventSpawnParticleV1`（无新字段）。
- **client**：`WornContainerPanel`、`WornPackFeatureRenderer`、`WornPackRenderBootstrap`、`PackOperationVfxPlayer.{UNEQUIP_EVENT, EQUIP_EVENT, STOW_EVENT, Kind, audioRecipe}`、`VfxBootstrap`（注册三路由）。

### 遗留 / 后续

- **真机 F5 调位**（P4）：`WornPackFeatureRenderer` 的 OFFSET_X/Y/Z 为 tunable，破草包背面挂点比例/穿模需真机 F5 目测微调（资产正确、接线完成，纯数值校准）。
- **front 挂点变体（P5 #4，未做）**：`grass_pouch_front.geo.json` 前胸挂点变体本 PR **未启用**——当前无前挂形制容器模板（破草包默认背面），启用属新形制引入、超出 P5 收口范围，留后续 plan（按 template→挂点映射接）。
- **磨损降容量（P5 #3，决议 #5，未做）**：默认不做（避免破坏「捡了再丢」手感 + 需 rebuild 重算 max_weight 全链改动），留独立 plan。
- **硬重量门控（决议 #5，未做）**：维持软门控（超重仅 `OverloadedMarker` debuff，不拒绝拖入/拾取/移动）。硬拒绝留独立 plan。
- **hint 面板**：`plan-inventory-hint-panel-v1`（用户自理）。
- **worldview**：本 plan 全程未改 `docs/worldview.md`（决议 #6：§五已锚定容器穿戴/自重计负重，套包/嵌套语义被正典覆盖）。
