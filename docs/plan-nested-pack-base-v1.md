# plan-nested-pack-base-v1 — 塔科夫式套包系统基础（物品内嵌子容器 + 重量向上累加）

> **来源**：手搓 104 产出物僵尸审计 → 「容器全死（12）」一类的地基。容器调查 workflow（6 维实地核查）综合产出。
> **依赖**：无（套包族的根 plan）。下游 [[plan-container-filter-and-completion-v1]]（其 P1/P2/P3/P4 直接挂本 plan 的 `PackContainer*` 协议 + `pack_{instance_id}` 命名 + `SubContainerPanel` 浮窗 + 5 随身子包 container 化）、[[plan-placeable-container-blocks-v1]]（复用本 plan `ContainerState` 子容器机制；其 §8.1 #5 把 `PackContainer*` 协议列为可选升级路径）均依赖本 plan。本 plan 全部 merge 到 main 后两个下游才开。
> **状态**：骨架（草案）。实地锚点已核实（research 2026-06-10）；§8 已凭 PR #467 Pi review + 用户确认定案三条搬入正文，§8 #4（5 子包 grid 尺寸）/ #5（浮窗拖拽 + z 序 P0 spike）/ #6（持有式套包 ContainerSpec 表示，equip_slot 硬约束冲突）悬留待 §8.1 收口；§8 #7（S2C 套包 open schema 复用 `LootContainerOpenV1`，不存在 `PackContainerOpenV1`）已在 §8.1 #7 跨 plan 契约定案。

给 `ItemInstance` 增加 `Option<ContainerState>` 子容器字段，建立『双击背包格里的容器物品 → 打开可拖拽浮动子背包面板 → 主背包↔子容器双向拖拽 → 关闭持久化回物品实例』的 server↔client↔schema 端到端闭环，并保证子容器内物品重量向上累加进 `calculate_current_weight`、死亡掉落递归展平、快照递归序列化。**进料**：`ItemInstance`/`ContainerState`/`PlacedItemState`（复用现有类型）、`external_container.rs` session 机制（参考）、`InventoryMoveIntent` 协议（镜像）。**出料**：子容器重量进 `calculate_current_weight` 总负重、死亡掉落递归展平、`build_inventory_snapshot` 递归 emit `pack_{instance_id}` 容器给 client。**对应 worldview §十:887「搜打撤」**（背包格容量是「撤」的核心约束，套包直接扩展单次可携带量）。

## 阶段总览

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 浮窗 spike（z 序 + 拖拽可行性）+ 数据模型 `ItemInstance.sub_container: Option<ContainerState>` + 向后兼容序列化 + 嵌套深度护栏 | ⬜ | |
| P1 | 重量递归累加 + 死亡掉落递归展平（三路径） | ⬜ | |
| P2 | schema + server 开/移/关协议（基于 `instance_id` 的子容器 session）+ 临时容器注入 | ⬜ | |
| P3 | `validate_move_semantics` 套包语义 + `build_inventory_snapshot` 递归 emit + 持久化 round-trip | ⬜ | |
| P4 | client：双击打开可拖拽浮动子背包面板（顶层 overlay）+ 双向 drag 路由 + 视听 | ⬜ | |
| P5 | 升级 5 个随身子包 TOML（misc→container）+ 端到端验收 | ⬜ | |

## 接入面（防孤岛）

- **进料**：
  - `ItemInstance`（`server/src/inventory/mod.rs:339`，per-instance 实例带全量状态，optional 字段统一 `#[serde(default, skip_serializing_if = "Option::is_none")]` 模式）。P0 新增字段入口。
  - 复用 `ContainerState`（`mod.rs:323`，`id/name/rows/cols/items: Vec<PlacedItemState>`）作 `sub_container` 类型，**不另造容器类型**；`PlacedItemState`（`mod.rs:332`，`row/col/instance`）。
  - `external_container.rs`（物资棺 open→move→close session 已跑通，`ExternalContainerRegistry` `:41`，`sessions: HashMap<u64, Entity>`，`pack_loot_into_grid` `:62`）作 P2 session 机制参考。
  - `InventoryMoveIntent`（`schema/client_request.rs:297`，`v/instance_id/from/to`）+ `ExternalContainerMove`（`:467`，`session_id/instance_id/from/to`）作 P2 C2S 协议镜像模板。
  - `InventoryInstanceIdAllocator`（`mod.rs:436`，`next_id() -> Result<u64, String>` + `new(start)`）给子容器内物品分配 id。
- **出料**：
  - 子容器内物品重量进 `calculate_current_weight`（`mod.rs:3135`）总负重（P1 改造目标），下游 `sync_overloaded_marker`（`mod.rs:3493`）自动按含子容器总重触发 `OverloadedMarker`（`mod.rs:517`）。
  - 死亡掉落 `apply_termination_drop_on_terminate`（`mod.rs:582`）/ `apply_death_drop_on_revive`（`mod.rs:2811`，分 TSY 路径 + 主世界路径）递归展平 `sub_container.items`（P1 改造目标，三路径全覆盖）。
  - `build_inventory_snapshot`（`network/inventory_snapshot_emit.rs:153`）递归 emit 子容器内物品（`container_id = "pack_{instance_id}"`）给 client（P3 改造目标）。
  - 持久化：`serialize_inventory_json`（`player/state.rs:2053`，做 `serde_json::to_string(inventory)`）—— P0 加字段后因 `ItemInstance: Serialize` 自动继承落盘，无需改 persistence 层本身（仅加 round-trip 测试，见 P3）。
- **共享类型 / event**：复用 `ContainerState` / `PlacedItemState` / `InventoryLocationV1`（`schema/inventory.rs` 的 Container/Equip/Hotbar，`ContainerIdV1 = String`），**不另造容器类型**；新增 `PackContainerOpen/Move/Close` C2S 三变体（镜像 `ExternalContainerMove` `client_request.rs:467`）。复用 `InventoryInstanceIdAllocator` 给子容器内物品分配 id。
- **跨仓库契约（symbol）**：
  - **server**：`PackContainerOpen`/`PackContainerMove`/`PackContainerClose` C2S 变体（`schema/client_request.rs`，新建，镜像 `ExternalContainerMove:467`）；`LootContainerSourceKindV1::PackItem { instance_id: u64 }` 变体（扩展 `schema/server_data.rs:605`，现仅 `SupplyCoffin { grade }` `:606`）；S2C 复用 `LootContainerOpenV1`/`LootContainerUpdateV1`/`LootContainerCloseV1`（`server_data.rs:611+`）。
  - **client**：新建 `SubContainerPanel.java`（参考 `LootContainerPanel.java`）；`ClientRequestSender.sendPackContainerOpen/Move/Close`（新建，参考 `sendExternalContainerMove`）；`DragState.SourceKind.SUB_CONTAINER`（扩展 `client/.../inventory/state/DragState.java:13`，现 `GRID/EQUIP/HOTBAR/QUICK_USE/MERIDIAN/BODY_PART`）。
  - **agent**：**无关**——纯本地背包，无 Redis key、无 IPC 流量、无 narration（背包操作不走天道叙事，research 已核实 schema 无 TS 对端）。
- **worldview 锚点**：worldview §十:887「搜打撤」循环——「撤 → 拿到东西就走，知道什么时候该跑比知道怎么打更重要」（`worldview.md:898`），背包格容量是「撤」的核心约束，套包系统直接扩展单次出门可搜刮量，是 §核心循环:26「匮乏、算计、信息差」的博弈工具。无新境界 / 货币概念。
- **qi_physics 锚点**：**无**——纯物品栏数据流，不碰真元 / 灵气 / 衰减 / 守恒。**红旗自查**：本 plan 不引入任何 `*_DECAY*` / `*_DRAIN*` 常数、不写 `cultivation.qi_current +=` / `zone.spirit_qi -=`、无 `qi_physics::ledger::QiTransfer` 涉及。实地确认 5 个随身子包 `spirit_quality_initial = 0.0` 无灵气属性，套包内物品转移走普通移动路径不触发 `qi_release_to_zone`。（worldview §九:808「物品被从箱子里拿出来触发 inventory 操作扣灵气纯度 1-5%」的转移税不在本族任何 plan scope——shelflife 无一次性扣减接口，扣 spirit_quality 须走 qi_physics ledger 守恒，将来另立 plan 处理（见 reminder.md「§808 转移税甩锅消解」）；本 plan 只搬运数据不改 spirit_quality。）

## P0 — 浮窗 spike + 数据模型 + 序列化 + 嵌套护栏 ⬜

> **P0 前半段是 UI 可行性 spike（§8 #5 决议升 P0）**：全 plan 最大未知是「owo-lib 能否在 `BaseOwoScreen<FlowLayout>` 的 root 上让一个 `Positioning.absolute` 子面板渲染在主网格 flow 子节点（含 `drawMultiCellItems` z=100/200 pass）之上、且可拖拽」。**先验证 UI 可行性再投 P1-P3 server plumbing**——万一做不出，整个双击开浮窗 UX 要趁早重想。spike 产出：一个 hardcoded 假数据的 `SubContainerPanel` 挂 root 顶层、能可见浮在主网格之上、能抓 header 拖动。spike 通过（render 截图确认 z 序 + 拖拽位置变化）才继续 server 数据模型。spike 不通过 → 停下交人工重设计 UX。

- **数据模型**：`inventory/mod.rs:339` `ItemInstance` 新增 `pub sub_container: Option<ContainerState>`，标 `#[serde(default, skip_serializing_if = "Option::is_none")]`（照现有 `mineral_id`/`charges`/`forge_quality` 模式）保证旧快照向后兼容（持久化/Redis/schema sample 不破）。
- **嵌套护栏**：定义 `pub const MAX_PACK_NEST_DEPTH: u8 = 1`（套包内不可再放套包，见 §8 #1 定案）。新增 `fn pack_nest_depth(instance: &ItemInstance) -> u8`：本体无 `sub_container` → 0；有 `sub_container` 但其内物品均无 `sub_container` → 1；任一内物品有 `sub_container` → ≥2。校验函数 `fn validate_nest_depth(instance: &ItemInstance) -> Result<(), String>`：`depth > MAX_PACK_NEST_DEPTH` 返回 `Err(format!("nesting depth {d} exceeds MAX_PACK_NEST_DEPTH={}", MAX_PACK_NEST_DEPTH))`。P3 `validate_move_semantics` 在「Container 物品入套包子容器格」时调此校验拒绝二级嵌套。
- **测试（饱和化，`inventory::*`）**：
  - happy：`ItemInstance` 带 `sub_container`（含 N 个 `PlacedItemState`）的 Clone + Serialize→Deserialize round-trip 完全相等（内容物 instance_id/template_id/row/col 全保留）。
  - 向后兼容 pin：旧无 `sub_container` 字段的 JSON（手写 fixture，含 freshness/mineral_id 等既有 optional）反序列化 → `sub_container == None`，且序列化回去不出现 `"sub_container"` key（`skip_serializing_if` 生效）。
  - 嵌套护栏：`pack_nest_depth` 对 depth=0（无子容器）/1（一级套包）/2（套包内套包）各返回 0/1/2 的穷举 pin；`validate_nest_depth` depth=1 → `Ok`、depth=2 → `Err`（断言错误信息含 `MAX_PACK_NEST_DEPTH`）。
  - 边界：空 `sub_container`（`items: vec![]`）depth=1、round-trip 保持空；`sub_container == None` depth=0。

## P1 — 重量递归累加 + 死亡掉落递归展平 ⬜

**纯 server 逻辑，无视听。**

- **重量递归**：改 `calculate_current_weight`（`mod.rs:3135-3154`）：现三路（containers/equipped/hotbar）只展平一层。新增 `fn instance_total_weight(instance: &ItemInstance) -> f64`：本体 `weight * stack_count` + 递归 `sub_container.items` 各项 `instance_total_weight`。三路求和均改用此函数。**决议：本体 + 内容物全计入总负重**（§8 #2 用户已确认，防「装袋减重」exploit）。`MAX_PACK_NEST_DEPTH=1` 保证递归最多一层，无栈溢出风险。
- **死亡掉落递归展平（三路径全覆盖）**：现 `apply_termination_drop_on_terminate`（`mod.rs:582`，L618 `for placed in container.items.drain(..)`）+ `apply_death_drop_on_revive`（`mod.rs:2811`，TSY 路径 `tsy_death_drop::apply_tsy_death_drop` + 主世界路径）均只 drain 顶层，`sub_container` 内物品**静默丢失**（无 panic 数据丢失 bug，实地确认）。新增 `fn flatten_instance_for_drop(instance: ItemInstance) -> Vec<ItemInstance>`：取出本体（清空其 `sub_container`，避免容器物品落地仍带内容物造成重复）+ 递归取出 `sub_container.items` 各 instance。三路径 drain 时对每个 `placed.instance` 调此函数展平加入掉落列表。
- **测试（饱和化，`inventory::*`）**：
  - happy：套包（本体 weight=2.0）内装 3 件（各 weight=1.0 stack=2）→ `calculate_current_weight` = 空袋差值断言 = 本体 2.0 + 内容物 6.0 = 8.0（断言「含子容器 vs 空袋差值 = 内容物重量 + 本体」，失败信息写明期望来源）。
  - 边界：空套包只计本体重量；嵌套 depth=1 满载递归求和正确。
  - 死亡掉落（三路径各一条专属用例）：① `apply_termination_drop_on_terminate` 套包内 N 物品全部进掉落列表（断言掉落实体数 = 顶层数 + 子容器内数）；② `apply_death_drop_on_revive` TSY 路径同；③ 主世界路径同。每条断言「子容器内物品 instance_id 出现在掉落列表」+「容器物品落地后自身 `sub_container` 已清空」（防内容物双份）。
  - 状态转换 / 联动：`sync_overloaded_marker`（`mod.rs:3493`）按含子容器总重触发 `OverloadedMarker`（pin 装满套包后越过 max_weight 触发标记）。
  > 注：`OverloadedMarker` 当前**无下游惩罚**（孤岛，移速联动归后续 plan），本 plan 只保证重量正确，不补惩罚。

## P2 — schema + 开/移/关协议 + 临时容器注入 ⬜

**纯 server / 协议逻辑，视听归 P4（面板开关 SFX）。**

- **C2S schema**：`schema/client_request.rs` 新增三变体（参照 `ExternalContainerMove:467`，`#[serde(rename_all = "snake_case")]` 对齐）：
  - `PackContainerOpen { v: u32, instance_id: u64 }`
  - `PackContainerMove { v: u32, instance_id: u64, from: InventoryLocationV1, to: InventoryLocationV1 }`
  - `PackContainerClose { v: u32, instance_id: u64 }`
- **S2C schema**：`server_data.rs:605` `LootContainerSourceKindV1` 加变体 `PackItem { instance_id: u64 }`（区分于现 `SupplyCoffin { grade }`）。S2C 复用 `LootContainerOpenV1`/`UpdateV1`/`CloseV1`，`source_kind: PackItem { instance_id }`。
- **session + 临时容器注入**：新建 `PackItemSession` Resource（`HashMap<u64 /*session_id*/, (Entity /*owner*/, u64 /*item_instance_id*/)>`，生命周期参照 `ExternalContainerRegistry` `external_container.rs:41`）。**打开时（`handle_pack_container_open`）**：按 `instance_id` 在 `PlayerInventory` 遍历找到目标容器 `ItemInstance`（**判定依据是 template category == Container，而非 `sub_container.is_some()`**——P0 给 `ItemInstance` 加的 `sub_container` 标 `#[serde(default, skip_serializing_if = "Option::is_none")]`，**新从 registry 造出 / 新发的子包 `sub_container` 必然是 `None`，绝不能假设已是 `Some`**）→ **懒初始化（关键，撞红即不收）**：若目标 `ItemInstance.sub_container == None`，从 `registry` 查该 template 的 `ContainerSpec`（`mod.rs:169`，取 `rows`/`cols`）并把 `sub_container` 就地置为 `Some(ContainerState { id: format!("pack_{instance_id}"), name: <template display_name>, rows: spec.rows, cols: spec.cols, items: vec![] })`（无 `ContainerSpec` 的物品 = 非容器 → `Err`，见错误分支）→ 把其 `sub_container`（现保证 `Some`）以 id `pack_{instance_id}` **临时插入 `PlayerInventory.containers`**（让 `validate_attach_fits`/`validate_move_semantics` 的 `container_id` 字符串匹配能找到目标）→ emit `LootContainerOpenV1 { source_kind: PackItem { instance_id }, rows, cols, placed_items, ... }`。**关闭 / move 提交时（`handle_pack_container_close` / `handle_pack_container_move`）**：把临时容器 `pack_{instance_id}` 的 `items` 写回对应 `ItemInstance.sub_container`，关闭时从 `containers` 移除临时容器 + 清 session（§8 #3 持久化时机定案：内嵌 `sub_container` 随玩家存档落盘；关闭/move 提交触发写回）。
- **server handler**：`handle_pack_container_open/move/close`（平行 `handle_external_container_move`，把「世界实体容器查找」替换为「按 `instance_id` 在 `PlayerInventory` 找 `sub_container`」+ 临时容器注入/写回）。
- **测试（饱和化，`inventory::*` / handler 单测）**：
  - happy：open 存在的套包 instance → emit `LootContainerOpenV1` source_kind=PackItem，placed_items 含子容器内全部物品；临时容器 `pack_{instance_id}` 出现在 `PlayerInventory.containers`。
  - **懒初始化 pin（撞红即说明 open 假设 `sub_container` 已是 Some）**：open 一个 `sub_container == None`（刚从 registry 造出、从未打开过）的子包 → handler 按其 template `ContainerSpec.rows/cols` 自动初始化 `sub_container = Some(ContainerState { id: "pack_{instance_id}", rows, cols, items: vec![] })`（断言初始化后的 grid 尺寸 == 该 template `ContainerSpec` 声明的 rows×cols，且 `items` 为空）；二次 open 同一已初始化子包不重置已有内容（幂等 pin）。
  - move 双向：主背包→子容器（占位/重量校验通过则物品移入临时容器）；子容器→主背包（移出）；子容器内格间移动。每条断言 close 后写回 `ItemInstance.sub_container` 与临时容器一致。
  - 错误分支：open 不存在的 instance_id → `Err`（不 panic）；open 非容器物品（template category 非 Container / registry 查不到 `ContainerSpec`）→ `Err`（不会误把普通物品懒初始化成容器）；move 到越界格 → `Err`。
  - 状态转换：open → move → close 序列后，`PlayerInventory.containers` 不残留 `pack_{instance_id}` 临时容器（关闭清理 pin）；session timeout 清理（参照 `ExternalContainerRegistry` timeout）→ 临时容器移除 + 写回。
  - schema pin：`PackContainerOpen/Move/Close` 三变体正反 sample 序列化对拍；`LootContainerSourceKindV1::PackItem { instance_id }` wire-format pin（照 `server_data.rs:4720` `loot_container_source_kind_supply_coffin_wire_format` 模板）。

## P3 — validate_move 套包语义 + 快照递归 + 持久化 round-trip ⬜

**纯 server 逻辑，无视听。**

- **`validate_move_semantics`（`mod.rs:3703`）套包语义**：现 `:3770` 已有「`Container` 类物品 → Hotbar 拦截」分支。新增「`Container` 类物品放入普通 container slot（非 equip slot）= 套包物品，**免 `equip_slot` 校验**，且若该 Container 物品自身带/将带 `sub_container` 则调 P0 `validate_nest_depth` 拒绝二级嵌套」分支。**保留两条既有路径不变**：① 装备到 BackPack/WaistPouch/ChestSatchel equip slot 才激活 `container_spec` 动态容器（`rebuild_containers_from_equipment`，`backpack-equip-v1` 机制）；② Hotbar 拦截（`:3770`）。**两条路径在代码里显式分叉**（避免同一 Container 物品在「装 equip slot」vs「放 container slot」有两种不一致行为，research 风险 4）。**风险 4 的收口落点在 §8.1 #6**：持有式套包的 spec 表示（held-only / 单独 PackSpec / 复用 equip_slot 但 `rebuild_containers_from_equipment` `mod.rs:3192` 显式排除）必须在 §8.1 拍定后，P3 才能确定「放 container slot」分支如何识别套包物品而不误激活 `rebuild`。本阶段实施前置确认 §8.1 #6 已收口。
- **`build_inventory_snapshot`（`inventory_snapshot_emit.rs:153`）递归 emit**：现只迭代 `inventory.containers` 顶层（`:164-191`）。对每个 `placed.instance.sub_container` 非空者，追加一个 `ContainerSnapshotV1 { id: "pack_{instance_id}", rows, cols, ... }` + 其内物品的 `PlacedInventoryItemV1` 批次（`container_id = "pack_{instance_id}"`），否则 client 打开套包看不到内容。空 `sub_container`（无内容物）不 emit 多余 container（避免快照膨胀）。
- **持久化 round-trip**：P0 加字段后 `serialize_inventory_json`（`player/state.rs:2053`）自动序列化，无需改 persistence 层；本阶段加测试覆盖（§8 #3 定案的持久化时机测试点）。
- **测试（饱和化，`inventory::*` / `network::*`）**：
  - validate happy：Container 物品入普通 container slot 通过（不要求 equip_slot）；装到 BackPack equip slot 仍走 `rebuild_containers_from_equipment`（既有 pin 不破，回归 `validate_move_semantics_accepts_back_pack_equip_to_back_pack_slot` `:8270`）。
  - validate 错误分支：Container 物品入 Hotbar 拒绝（既有 `:3770` 回归 pin）；带 `sub_container` 的 Container 物品再放入另一套包子容器 → `validate_nest_depth` 拒绝（二级嵌套，断言错误信息含 `MAX_PACK_NEST_DEPTH`）。
  - snapshot：含 `sub_container` 的物品 → snapshot 含 `pack_{instance_id}` container + 其 placed_items（断言 container_id 格式 + 内物品数）；空 `sub_container` → 不 emit 额外 container（边界 pin）；嵌套 depth=1 满载递归 emit 正确。
  - 持久化 round-trip：`serialize_inventory_json` → 反序列化后套包内物品全保留（instance_id/位置/数量），含子容器的 inventory 落盘重连后 `sub_container` 内容不丢（端到端 pin，对应 §8 #3）。

## P4 — client 双击开浮动可拖拽子背包面板 + 双向 drag ⬜

> **⚠️ 先例核实（2026-06-10 实地 grep）——「可拖拽浮窗」是半成品，不可照抄**：
> `SkillConfigFloatingWindow.dragBy()`（`combat/inspect/SkillConfigFloatingWindow.java:71`）是**死代码**——`WindowHandle` 接口只暴露 `component()`/`positionAt()`，不暴露 `dragBy`；header 只挂 X 关闭 mouseDown，无拖拽 handler；全仓 `dragBy` **零调用者**；`combat/inspect/` 包**无 `mouseDragged`**。窗口被 `open(technique, 190, 18, ...)` 钉死固定锚点，且嵌在 techniques 标签内容里（非顶层 overlay）。**结论：浮窗能 `positionAt` 定位显示，但「拖拽移动」从未接线、玩家从未真正拖动过。** `LootContainerPanel`（`inventory/InspectScreen.java:144` `lootPanel` 字段）挂在 `outerRow`（水平 FlowLayout 子节点）而非 root 顶层（`:3373` 附近），是**内嵌面板**——item 在它和主网格间拖拽走 `InspectScreen` 的 item drag loop，但**面板本体不可移动**。
> **故 P4 = 从零搭建可拖拽浮窗能力 + root 顶层 overlay 接线，不是 copy。** spike（P0）已验证可行性，本阶段做完整接线。`BotanyDragState.java:16-97`（`tickDrag/recordRenderedBounds/isDragging/deltaX/deltaY`）是面板级拖拽的**已有可用先例**，可照搬给 `SubContainerPanel` 实现窗口拖拽（优于死掉的 `dragBy`）。

- **新建 `SubContainerPanel.java`**（结构照 `LootContainerPanel.java` POJO：`build()` 返回 FlowLayout，暴露 `lootGrid()`/`containerId()`），但 mount 点选 **root 顶层 overlay**（最后添加，`Positioning.absolute(x, y)`），不嵌 backpack 标签内容（否则被主网格盖住 / 被 tab 切换推出屏外）。
- **窗口拖拽接线（本 plan 新建，非复用死 `dragBy`）**：借 `BotanyDragState` 模式——
  - header 加 `mouseDown` 捕获拖拽起点（记 grabOffset），`mouseUp` 结束。
  - `InspectScreen.java` 的 `mouseDragged`（`:1961`，item drag loop 已存在但**不路由窗口拖拽**）加分支：活动子面板 header 命中 → 更新面板 `Positioning.absolute(x+dx, y+dy)` + `clamp` 屏内（clamp 在 positionAt 内）。`mouseClicked`/`mouseReleased` 同步加窗口拖拽起止。
  - 可顺手把 `SkillConfigFloatingWindow` 的死 `dragBy` 一并接活（修 techniques 配置窗拖不动老 bug），但不阻塞本 plan。
- **z 序（硬要求）**：主网格多格物品在 `InspectScreen.drawMultiCellItems()`（`:3060`）走 z=100/200 pass，子面板 + 其内物品必须画在 **z > 200** 或挂为**最后添加的 root 顶层 overlay**（spike P0 已验证哪条可行，按 spike 结论 mount）。
- **双击检测**：`InspectScreen.java` 的 `mountLootPanelIfActive`（`:3232` 附近）同级加：双击背包格里 Container 物品 → `ClientRequestSender.sendPackContainerOpen(instance_id)` → 收 `LootContainerOpenV1 { source_kind: PackItem }` → mount `SubContainerPanel`。
- **drag 路由**：`DragState.java:13` 的 `SourceKind` 加 `SUB_CONTAINER`；`attemptDrop` 多目标加子容器面板格命中；item drag 路由：主背包↔子容器走新 `sendPackContainerMove`（不复用 `sendInventoryMove`，因子容器是 session-scoped 临时容器）。
- **视听（面板开关是玩家可感知）**：
  - **开面板 SFX**：audio_recipe 单层 `{ sound: "block.barrel.open", pitch: 1.2, volume: 0.6, delay_ticks: 0 }`，client 收 `LootContainerOpenV1 { source_kind: PackItem }` 后本地播。
  - **关面板 SFX**：单层 `{ sound: "block.barrel.close", pitch: 1.0, volume: 0.6, delay_ticks: 0 }`，client 发 `sendPackContainerClose` 后本地播。
  - **面板 fade-in**：owo-lib `Component.alpha` 0→1，6 tick 线性 easing（`Easing.LINEAR`），无 fade-out（即时关闭）。
  - **无粒子**（纯 UI）；**无 narration**（背包操作不走天道，接入面 agent 标「无关」）。
- **硬验收（用户点名，撞红即不收）**：
  1. **显示在主容器之上**：双击后面板**可见地**浮在主背包网格上方，不被网格/多格物品 pass 盖住、不被 tab 切换推出屏外。e2e 截图/render 验证 z 序。
  2. **拖拽真实可用**：抓 header 拖动，面板**实际跟随鼠标移动**并 `clamp` 在屏内；松手停在新位置；拖动期间 item 不误拾。（对照死掉的 `SkillConfigFloatingWindow`，本条专测拖拽真被调用 + 位置真变化。）
  3. 子容器格 ↔ 主背包双向 item drag 发对应 `PackContainerMove` C2S；关闭 unmount，位置重开复位/记忆。
- **测试（饱和化，client e2e + 单测）**：
  - 双击 Container 物品格 → 发 `PackContainerOpen`（C2S payload 断言含 instance_id）→ 收 open → 面板 mount + 开面板 SFX。
  - 窗口拖拽：模拟 header mouseDown→mouseDragged→mouseUp → 面板 `Positioning` x/y 实际变化（断言坐标 delta = 鼠标 delta，clamp 在屏内边界）；拖动期间 item drag 不触发（互斥 pin）。
  - z 序 render：截图断言子面板可见浮在主网格 z=200 pass 之上（spike 同款验证）。
  - 双向 drag：子容器格→主背包发 `PackContainerMove`（from=pack container_id，to=main_pack）；反向同。
  - 关闭：发 `PackContainerClose` + 关面板 SFX + unmount。

## P5 — 升级 5 随身子包 TOML + 端到端验收 ⬜

**资产 + e2e，无新视觉资产产出（复用 P4 已建面板）。**

- 把 `herb_pouch`（`workbench_materials.toml:193`）/ `projectile_bag`（`:215`）/ `ore_sack`（`:259`）/ `water_skin`（`:270`）/ `herb_crate`（`:292`）五个随身子包从 `category = "misc"`（现全缺 `[item.container]` 块，纯占位）升为 `category = "container"` 并补 `[item.container]` 块（rows/cols/weight_capacity，**容量见 §8 #4 收口表**）。
  > **⚠️ equip_slot 硬约束冲突（§8 #6，升 active 前 §8.1 必收口）**：`parse_container_spec`（`mod.rs:1532`，实地核验）对 `ContainerSpecToml.equip_slot`（`mod.rs:1514`，**非 `#[serde(default)]`，必填**）强制校验 `equip_slot ∈ {back_pack, waist_pouch, chest_satchel}`（`mod.rs` 的 `EQUIP_SLOT_BACK_PACK/WAIST_POUCH/CHEST_SATCHEL`），否则 registry load **直接 `Err`**。但本 plan 的 5 子包是「放普通背包格、双击打开」的**持有式**物品，非装备槽容器。**直接给它们填 `equip_slot = "back_pack"` 会让 `rebuild_containers_from_equipment`（`mod.rs:3192`）在物品装进 back_pack 槽时生成 body-pocket 容器，与套包持有路径撞车——这正是 P3「research 风险 4：同一 Container 物品装 equip slot vs 放 container slot 两种不一致行为」。** §8.1 #6 必须实地拍定持有式套包的 spec 表示（held-only 变体 / 单独 PackSpec / 复用某 equip_slot 但 `rebuild` 显式排除），不许 P5 实施时随手填 `back_pack` 蒙混过关。
  > **⚠️ TOML merge 协调（reminder.md «A-P5 / B-P3 改同一 TOML»）**：本 P5 与 [[plan-container-filter-and-completion-v1]] P3 改 `workbench_materials.toml` 同 5 条目（本 plan 加 container 块，下游加 `accept` filter）。依赖图（下游在本 plan 后开）保证顺序，实施时注意相邻段 merge conflict。`accept` filter 字段**不在本 plan**（本 plan 只做 container 化 + grid，filter 归下游）。
- **测试（饱和化）**：
  - 资产加载：5 子包 TOML 解析 category=Container + `[item.container]` 块无 panic（registry load pin，撞红即说明 spec 字段写错）；每子包 grid 尺寸 pin（rows×cols 与 §8 #4 表一致）。
  - e2e（一条全链路用例）：从 registry **新造** `ore_sack`（`sub_container == None`）装进主背包格 → 双击打开（首次 open 走 P2 懒初始化按 `ContainerSpec.rows/cols` 建空 grid → 面板浮现）→ 矿石拖入 / 拖出 → `calculate_current_weight` 含子容器内容物 → 触发死亡 → 掉落列表含子容器内全部物品 → 持久化重连后子容器内容还在。**首步专测「全新未打开过的子包」路径**，不允许测试预塞 `sub_container = Some(...)` 掩盖懒初始化空洞。

## §8 开放问题（P0 决策门前需收口）

> #1（嵌套深度=1）/ #2（全计入负重）/ #3（持久化时机）已凭用户确认 + 实地证据在正文定案（依据 PR #467 Pi review + reminder.md «§8 已决项搬正文»），原表保留以备追溯，**实施时以正文 + 下方 §8.1 决议为准**。#4 / #5 / #6 需 §8.1 收口；#7 是跨 plan 契约定案（不需 spike，已在下方 §8.1 #7 直接定案）。

| # | 问题 | 定案 / 状态 |
|---|------|------|
| 1 | 子容器能否再嵌套（套包内放套包）？ | **已定**：`MAX_PACK_NEST_DEPTH=1`，不可再嵌套（用户已确认）。正文 P0 `validate_nest_depth` 已实现，P3 `validate_move_semantics` 拒绝二级嵌套。 |
| 2 | 套包内物品对负重影响：只算本体 vs 本体+内容物？ | **已定**：本体+内容物全计入 `calculate_current_weight`，防「装袋减重」exploit（用户已确认）。正文 P1 `instance_total_weight` 递归求和已实现。 |
| 3 | 子容器持久化时机？ | **已定**：内嵌 `ItemInstance` 随玩家存档落盘（`serialize_inventory_json:2053` 加字段后自动继承）；关闭面板 / move 提交时写回 `ItemInstance.sub_container`。**这是测试点**（P3「持久化 round-trip」+ P2「close 写回」），非独立设计问题。 |
| 4 | 5 随身子包各自 grid（rows×cols）？ | **悬留（P5 前必拍）**：pouch/sack 中、vial 小、crate 大。须与 [[plan-container-filter-and-completion-v1]] 12 容器验收表对齐（该表已列 `herb_pouch` 3×3 / `ore_sack` 3×3 / `projectile_bag` 3×4 / `water_skin` 2×2 / `herb_crate` 4×4），§8.1 收口时确认采该表数值或重校。 |
| 5 | **浮窗拖拽 + z 序基建从零建**（先例 `dragBy` 死代码，见 P4 ⚠️）：子面板挂 root 层还是 tab content 层？ | **悬留（升 active 时 spike 验证）**：推荐挂 **root 顶层 overlay**（`Positioning.absolute`，最后添加，z>主网格 200 pass），用 `BotanyDragState` 模式实现拖拽（已有可用先例）。**已升为 P0 spike**（正文 P0 前半段）——UI 可行性是全 plan 最大未知，spike 不过停下重设计。这是用户点名的最高风险点，不许假设照抄可行。 |
| 6 | **持有式套包的 ContainerSpec 表示**（equip_slot 硬约束冲突，见 P5 ⚠️ / P3 风险 4）：`parse_container_spec`（`mod.rs:1532`）强制 `equip_slot ∈ {back_pack/waist_pouch/chest_satchel}` 必填，持有式套包填哪个？填 `back_pack` 会触发 `rebuild_containers_from_equipment`（`mod.rs:3192`）与持有路径撞车。 | **悬留（P5 前必拍，§8.1 收口）**：候选 (a) 给 `ContainerSpec`/`ContainerSpecToml` 加 `held_only: bool`（`#[serde(default)]`，true 时跳过 equip_slot 校验、`rebuild_containers_from_equipment` 显式排除）；(b) 新建独立 `PackSpec`（与 equip 容器 spec 分离）；(c) 复用某 equip_slot 但在 `rebuild` 内按 template 黑名单排除。§8.1 必须实地确认所选方案的 registry load 不 panic 的最小 spec 字段集 + `rebuild` 排除点 file:line。 |
| 7 | **S2C 套包 open schema：复用 `LootContainerOpenV1` 还是新建 `PackContainerOpenV1`？** | **已定（跨 plan 契约，见下方 §8.1 #7）**：采方案 (a)——S2C 一律复用 `LootContainerOpenV1` + `LootContainerSourceKindV1::PackItem { instance_id }`，**全 plan 不存在 `PackContainerOpenV1` struct**（P2 只新建 `PackContainerOpen/Move/Close` 三个 C2S enum 变体）。下游 [[plan-container-filter-and-completion-v1]] 的 `accept_filter` 加在 `LootContainerOpenV1` 上（**`LootContainerOpenV1` 标 `#[serde(deny_unknown_fields)]` `server_data.rs:610`，加字段是 wire 破坏，须同步改 schema sample**）。 |

> §8.1 收口（#4 / #5 / #6 靠 spike + Explore 核查；#7 已定案）后追加 `## §8.1 决议（pre-P0，YYYY-MM-DD）`，每条带 file:line + plan 章节双锚点（依据 docs/CLAUDE.md §5.1）。决议靠 Explore agent 并行核查代码现状产出，不拍脑袋。

## §8.1 跨 plan 契约定案（pre-P0，2026-06-10）

> 本节只定案不需 spike 的**跨 plan 契约**（§8 #7）。需 UI spike / Explore 核查现状的 #4（grid）/ #5（浮窗）/ #6（持有式 spec）留升 active 时按 §8.1 收口模式补。

### #7 S2C 套包 open schema 契约（消除悬空 `PackContainerOpenV1` symbol）

**决议**：
1. **S2C 一律复用 `LootContainerOpenV1`**（`server_data.rs:611`）+ `LootContainerSourceKindV1::PackItem { instance_id: u64 }`（扩 `server_data.rs:605`）。**全 plan 族不存在 `PackContainerOpenV1` 这个独立 S2C struct**——P2 只新建 `PackContainerOpen` / `PackContainerMove` / `PackContainerClose` 三个 **C2S** `client_request` enum 变体。
2. 下游 [[plan-container-filter-and-completion-v1]] 的 `accept_filter` 字段加在**复用的 `LootContainerOpenV1` 本体**上（其 L37/125/144 已是此写法，无需改引用）；套包路径因复用同一 schema 自动覆盖 filter，不需第二个 schema。
3. **wire 破坏告知（关键）**：`LootContainerOpenV1` 标 `#[serde(deny_unknown_fields)]`（`server_data.rs:610`，实地核验）。下游给它加 `accept_filter` 字段属 wire 破坏——**下游 P4 实施时必须连同 schema sample 一起改**（`agent/packages/schema/samples/*.json` 双端校验 + `.proto` 同步），不可只加 Rust 字段。本根 plan P2 新建的 `PackItem` source_kind 变体同理需配 wire-format sample（已在 P2 测试项「`LootContainerSourceKindV1::PackItem` wire-format pin」覆盖）。

**落点**：`server/src/schema/server_data.rs:605`（`LootContainerSourceKindV1` 扩 `PackItem`）/ `server_data.rs:610-611`（`LootContainerOpenV1` `deny_unknown_fields` + 下游加 `accept_filter`）/ 本 plan P2 §「S2C schema」+ 接入面「跨仓库契约」/ 下游 [[plan-container-filter-and-completion-v1]] L35/37/125/144（已对齐，无悬空 symbol）。

## §10 实施工作流

依据 docs/CLAUDE.md §六。本 plan scope = 6 PR（P0 含 spike，P1-P5 各一），多 PR 序列化（不拆多 plan，§6.3）。**纯逻辑 + 数据 + UI 接线，无 NBT 建筑 / bbmodel 资产 → §10.1 三轮打磨 + `<PROMISE>` 不适用**（P4 client 浮窗是从零接线的交互能力，非视觉资产产出；但其「显示在主容器之上 + 拖拽真实可用」硬验收用 render 截图 e2e 兜，等价多轮 review 的视觉核验）。

### §10.1 PR 拆分点（依赖顺序，前一个 merge 后开下一个）

1. **PR-1（P0）**：UI 浮窗 spike（hardcoded 假数据验证 z 序 + 拖拽可行）+ `ItemInstance.sub_container` 数据模型 + 嵌套护栏 + 向后兼容序列化测试。**spike 不过则停下交人工**，过了再提 PR。
2. **PR-2（P1）**：`calculate_current_weight` 递归 + 死亡掉落三路径递归展平。纯 server，依赖 PR-1 字段。
3. **PR-3（P2）**：`PackContainerOpen/Move/Close` schema + `LootContainerSourceKindV1::PackItem` + `PackItemSession` + 临时容器注入/写回 handler。依赖 PR-1/2。
4. **PR-4（P3）**：`validate_move_semantics` 套包语义分叉 + `build_inventory_snapshot` 递归 emit + 持久化 round-trip 测试。依赖 PR-3。
5. **PR-5（P4）**：client `SubContainerPanel` root 顶层 overlay + `BotanyDragState` 式窗口拖拽 + 双击开 + 双向 drag 路由 + 开关 SFX + fade-in + 三条硬验收 e2e。依赖 PR-3/4。
6. **PR-6（P5）**：5 随身子包 TOML 升 container + grid + 全链路 e2e。依赖 PR-1..5（同改 `workbench_materials.toml`，注意与下游 plan merge）。

### §10.2 每 PR 独立 subagent（context 隔离）

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...本 PR 范围 + 测试要求（饱和化口径）+ 依赖前置确认...\n\nultrathink"
)
```
主线只接收 subagent result（200-500 token），不亲自跑实施；merge 命令主线亲自做。

### §10.3 CodeRabbit ScheduleWakeup 等待协议

每 PR 走完整等待：`gh pr checks <PR>` `pending` → `ScheduleWakeup delaySeconds=1200`，最多 3 回合（60 min）；`fail` 按 commands/consume-plan.md step 7 严重性桶处理；修完 review **必重等 CR re-review**，不自判通过；前一个 PR 未 APPROVED/收敛不开下一个。

### §10.4 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan plan-nested-pack-base-v1` 后即可下班，醒来看本 plan 是否在 `docs/finished_plans/`。全自动：测试/CI 失败 ≤2 轮有限修复，review 意见自行判断采纳，仅严重设计问题（如 P0 spike 不过需重想 UX）/反复修不过才交人工。

## Finish Evidence

（迁入 finished_plans/ 前必填：落地清单 / 关键 commit hash+日期 / 测试结果命令+数量 / 跨仓库核验 server·client symbol / 遗留依赖其他 plan 的待办）
</content>
</invoke>
