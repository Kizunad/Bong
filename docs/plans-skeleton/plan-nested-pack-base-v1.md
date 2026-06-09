# plan-nested-pack-base-v1 — 塔科夫式套包系统基础（物品内嵌子容器 + 重量向上累加）

> **来源**：手搓 104 产出物僵尸审计 → 「容器全死（12）」一类的地基。容器调查 workflow（6 维实地核查）综合产出。
> **依赖**：无（4-plan 套包族的根）。下游 [[plan-container-filter-and-completion-v1]]、[[plan-placeable-container-blocks-v1]] 均依赖本 plan。
> **状态**：骨架（草案）。实地锚点已核实，开放问题带推荐默认，P0 前需 §8.1 收口。

给 `ItemInstance` 增加 `Option<ContainerState>` 子容器字段，建立『双击背包格里的容器物品 → 打开可拖拽浮动子背包面板 → 主背包↔子容器双向拖拽 → 关闭持久化回物品实例』的 server↔client↔schema 端到端闭环，并保证子容器内物品重量向上累加进 `calculate_current_weight`、死亡掉落递归展平、快照递归序列化。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 数据模型：`ItemInstance.sub_container: Option<ContainerState>` + 向后兼容序列化 + 嵌套深度护栏 | ⬜ |
| P1 | 重量递归累加 + 死亡掉落递归展平 | ⬜ |
| P2 | schema + server 开/移/关协议（基于 `instance_id` 的子容器 session） | ⬜ |
| P3 | `validate_move_semantics` 套包语义 + 快照递归 emit | ⬜ |
| P4 | client：双击打开可拖拽浮动子背包面板 + 双向 drag 路由 | ⬜ |
| P5 | 升级 5 个随身子包 TOML（misc→container）+ 端到端验收 | ⬜ |

## 接入面（防孤岛）

- **进料**：`ItemInstance`（`inventory/mod.rs:337`，per-instance 实例带全量状态）；复用 `ContainerState`（`mod.rs:322`）/ `PlacedItemState`（`mod.rs:331`）作子容器类型；`external_container.rs`（物资棺 open→move→close session 已跑通）作打开机制参考；`InventoryMoveIntent`（`schema/client_request.rs:288`）协议。
- **出料**：子容器内物品重量进 `calculate_current_weight`（`mod.rs:3133`）总负重；死亡掉落 `apply_death_drop_on_revive` / `apply_termination_drop_on_terminate` 递归展平；`network/inventory_snapshot_emit.rs:232` 递归 emit 给 client。
- **共享类型 / event**：复用 `ContainerState` / `PlacedItemState` / `InventoryLocationV1`（`schema/inventory.rs:204` 的 Container/Equip/Hotbar），**不另造容器类型**；新增 `PackContainerOpen/Move/Close` C2S（镜像 `ExternalContainerMove` `client_request.rs:458`）。复用 `InventoryInstanceIdAllocator`（`mod.rs:435`）给子容器内物品分配 id。
- **跨仓库契约**：server `PackContainerOpenV1`/`PackContainerMoveV1`/`PackContainerCloseV1`（`schema/server_data.rs` 或镜像 `LootContainerOpenV1:611`）；client `SubContainerPanel.java` + `ClientRequestSender.sendPack*`；**agent 无关**（纯本地背包，无 Redis/IPC 流量）。
- **worldview 锚点**：末法残土搜打撤携带（背包格子是核心交互面，套包决定一次出门能带多少）；无新境界 / 货币概念。
- **qi_physics 锚点**：**无**——纯物品栏数据流，不碰真元 / 灵气 / 衰减。（保鲜类容器对 spirit_quality 的影响归 [[plan-container-filter-and-completion-v1]]，走 shelflife，不在本 plan。）

## P0 — 数据模型 + 序列化 + 嵌套护栏 ⬜

- `inventory/mod.rs:337` `ItemInstance` 新增 `pub sub_container: Option<ContainerState>`，标 `#[serde(default, skip_serializing_if = "Option::is_none")]` 保证旧快照向后兼容（持久化/Redis/schema sample 不破）。
- 定义 `MAX_PACK_NEST_DEPTH = 1`（套包内不可再放套包，见 §8.1 #1）。新增 `fn pack_nest_depth(instance) -> u8` 校验。
- 测试：旧无 `sub_container` 字段的 JSON 反序列化默认 None（向后兼容 pin）；嵌套深度护栏（depth=1 通过、depth=2 拒绝）；`ItemInstance` Clone/Serialize round-trip 含子容器。

## P1 — 重量递归累加 + 死亡掉落递归展平 ⬜

- 改 `calculate_current_weight`（`mod.rs:3133-3153`）：三路求和（containers/equipped/hotbar）时，对每个 `PlacedItemState.instance.sub_container` 递归求和子物品 `weight * stack`。**决议：本体 + 内容物全计入总负重**（§8.1 #2）。
- 改死亡掉落 `apply_death_drop_on_revive` / `apply_termination_drop_on_terminate`：`drain` containers 时递归展平 `sub_container.items`，否则套包内物品死亡静默丢失（无 panic 的数据丢失 bug）。
- 测试：套包内 N 物品计入 `calculate_current_weight`（断言含子容器 vs 空袋差值 = 内容物重量）；死亡掉落产出列表含子容器内全部物品；`sync_overloaded_marker`（`mod.rs:3491`）按含子容器总重触发。
  > 注：`OverloadedMarker` 当前**无下游惩罚**（孤岛），本 plan 只保证重量正确，不补惩罚（移速联动归后续）。

## P2 — schema + 开/移/关协议 ⬜

- `schema/client_request.rs` 新增三 C2S 变体（参照 `ExternalContainerMove:458`）：`PackContainerOpen { v, instance_id: u64 }` / `PackContainerMove { v, instance_id, from: InventoryLocationV1, to: InventoryLocationV1 }` / `PackContainerClose { v, instance_id }`。
- server handler：`handle_pack_container_*`（平行 `handle_external_container_move` `client_request_handler.rs:9540`），把「世界实体容器查找」替换为「在 `PlayerInventory` 按 `instance_id` 找到 `ItemInstance.sub_container`」。
- session：新建 `PackItemSession` Resource（`HashMap<u64, (owner_entity, item_instance_id)>`），生命周期参照 `ExternalContainerRegistry`（`external_container.rs:41`）。
- S2C：复用 / 镜像 `LootContainerOpenV1:611` / `LootContainerUpdateV1:622` / `LootContainerCloseV1`，`source_kind` 区分 `PackItem { instance_id }`。
- 测试：open 不存在 instance_id 拒绝；move 跨容器（主背包↔子容器双向）校验占位/重量；close 持久化回 `ItemInstance`；session timeout 清理。

## P3 — validate_move 套包语义 + 快照递归 ⬜

- `validate_move_semantics`（`mod.rs:3701`）：明确「`Container` 类物品放入普通 container slot = 套包物品，免 `equip_slot` 校验」分支；保留「装备到 BackPack/WaistPouch/ChestSatchel 才激活 `container_spec` 动态容器」路径（`rebuild_containers_from_equipment` `mod.rs:3190`）不变；保留 Hotbar 拦截（`mod.rs:3767`）。
- `network/inventory_snapshot_emit.rs:232`：snapshot 递归发子容器内物品（`container_id = "pack_{instance_id}"`），否则 client 打开套包看不到内容。
- 测试：Container 物品入 container slot 通过、入 Hotbar 拒绝；snapshot 含 `pack_*` container 的 placed_items；空子容器不 emit 多余 container。

## P4 — client 双击开浮动可拖拽子背包面板 + 双向 drag ⬜

- 新建 `SubContainerPanel`（照 `LootContainerPanel.java` POJO：`build()` 返回 FlowLayout，暴露 `lootGrid()`/`containerId()`；照 `SkillConfigFloatingWindow.java:65` 的 `positionAt`/`dragBy` 实现 header 拖拽移动 + `clamp` 边界约束）。
- `InspectScreen.java` 的 `mountLootPanelIfActive` 同级加双击检测分支（双击背包格里的 Container 物品 → `sendPackContainerOpen` → mount `SubContainerPanel`）。
- `DragState.java` 的 `SourceKind` 加 `SUB_CONTAINER`；`attemptDrop` 多目标加子容器面板格命中；drag 路由复用 `sendInventoryMove` / 新 `sendPackContainerMove`。
- **视听**（面板开关是玩家可感知）：开面板 SFX = `block.barrel.open` pitch 1.2 vol 0.6；关 = `block.barrel.close` pitch 1.0；面板 fade-in 6 tick（owo-lib alpha 0→1）。无粒子（纯 UI）。
- 测试（client）：双击 Container 物品 mount 面板；面板 header 拖拽移动 + clamp 屏内；子容器格↔主背包双向 drag 发对应 C2S；关闭 unmount。

## P5 — 升级 5 随身子包 TOML + 端到端验收 ⬜

- 把 `herb_pouch`(`workbench_materials.toml:193`)/`ore_sack`(:259)/`projectile_bag`(:215)/`water_skin`(:270)/`herb_crate`(:292) 五个随身子包从 `category=misc` 升为 `category=container` 并补 `[item.container]` 块（rows/cols/weight_capacity，容量见 §8.1 #4）。
- e2e：装进主背包格子 → 双击打开 → 物品拖入拖出 → 重量累加 → 死亡掉落含子容器物品 → 持久化重连后子容器内容还在，全链路一条用例。

## §8 开放问题（P0 决策门前需收口）

| # | 问题 | 推荐默认（待 §8.1 收口确认） |
|---|------|------|
| 1 | 子容器能否再嵌套（套包内放套包）？ | **MAX_PACK_NEST_DEPTH=1**：不可再嵌套。无限深度需 weight/death-drop/snapshot 全递归 + 护栏，复杂度不值。**用户已倾向接受深度=1。** |
| 2 | 套包内物品对负重的影响：只算本体 vs 本体+内容物？ | **本体+内容物全计入** `calculate_current_weight`，防「装袋减重」exploit。**用户已确认全计入。** |
| 3 | 子容器持久化时机？ | 内嵌 `ItemInstance` 随玩家存档落盘；关闭面板 / move 时 revision bump 触发写回。需测 `persistence/mod.rs` 落盘路径含嵌套。 |
| 4 | 5 随身子包各自 grid（rows×cols）？ | pouch/sack 中（3×3~3×4）、vial 小（1×2~2×2）、crate 大（4×4）。具体见 [[plan-container-filter-and-completion-v1]] 容量表。 |

> §8 收口后追加 `## §8.1 决议（pre-P0，YYYY-MM-DD）`，每条带 file:line + plan 章节双锚点（依据 docs/CLAUDE.md §5.1）。

## §10 实施工作流

升 active 时按 docs/CLAUDE.md §6 补全：scope ≥ 4 PR → 多 PR 序列化（P0-P1 数据底盘 / P2-P3 协议 / P4 client / P5 资产+e2e），每 PR 独立 subagent（opus + ultrathink）+ CR ScheduleWakeup 等待协议。本 plan 无建筑/bbmodel 资产，§10.1 多轮不适用。

## Finish Evidence

（迁入 finished_plans/ 前必填：落地清单 / 关键 commit / 测试结果 / 跨仓库核验 / 遗留）
