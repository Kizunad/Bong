# plan-placeable-container-blocks-v1 — 可放置世界容器方块（货箱 / 草药筐 / 死信箱）+ bbmodel

> **来源**：手搓 104 产出物僵尸审计「容器全死（12）」一类里适合作世界方块的 3 个。
> **依赖**：[[plan-workbench-place-runtime-v1]]（放置/破坏/交互底盘）+ [[plan-nested-pack-base-v1]]（容器打开协议 + ContainerState）。**两个都 merge 后才开本 plan。**
> **状态**：骨架（草案）。bbmodel 由本仓维护者按延寿棺同款流程（分部件 gen 脚本 → 逐件预览 → 拼接，3 轮 + PROMISE）设计，在 P2 产出。

把适合作世界方块的容器（`trade_crate` 货箱 / `herb_crate` 草药筐放置版 / `dead_drop_box` 死信箱）实装为可右键放置、破坏、打开搜索的世界方块，配 bbmodel 资产。放置走 [[plan-workbench-place-runtime-v1]] 的 `PlaceableBlockKind` 底盘，打开后的容器内容操作复用套包/外部容器机制。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | `ExternalContainerKind` 扩展 + 容器方块放置/破坏链路（接放置底座） | ⬜ |
| P1 | 打开搜索链路 + IntentHandler（client 右键交互） | ⬜ |
| P2 | bbmodel 资产（货箱/草药筐/死信箱）+ 死信箱阵法防砸 + 端到端验收 | ⬜ |

## 接入面（防孤岛）

- **进料**：[[plan-workbench-place-runtime-v1]] 的 `PlaceableBlockKind` 放置底盘 + `WorkbenchPlace` 同款协议；[[plan-nested-pack-base-v1]] 的容器打开协议；`external_container.rs` 的 `ExternalContainer`（:23）+ `ExternalContainerKind`（:17）+ `pack_loot_into_grid`；`SupplyCoffinInteractIntentHandler.java`（OpenContainer intent 模板）；`CraftCategory::Container` 配方（`workbench_recipes.rs`）；coffin bbmodel 先例（`scripts/models/gen_*_coffin.py` + `render_bbmodel.py`）。
- **出料**：`trade_crate`/`herb_crate`(放置版)/`dead_drop_box` 放下 → 世界 entity 带 `ExternalContainer`；右键搜索打开 → `LootContainer` S2C；内容操作复用 [[plan-nested-pack-base-v1]] / 外部容器 move。
- **共享类型 / event**：`ExternalContainerKind`（`external_container.rs:17`）加 `StorageCrate` / `DeadDrop` 变体；**复用 `ExternalContainer` / `LootContainerOpenV1`，不另造世界容器**；bbmodel 走现有资源包管线（[[plan-resourcepack-v1]]）。
- **跨仓库契约**：server `StorageCrate`/`DeadDrop` `ExternalContainerKind`；client 新建 IntentHandler（照 `SupplyCoffinInteractIntentHandler`）+ `DefaultInteractionHandlers.java:15` 注册 + bbmodel 渲染；**agent 无关**。
- **worldview 锚点**：死信箱 = worldview §九:850 阵法防砸（阵纹 block state，破坏需先破阵）；货箱 = 末法集市搬运/贸易；`dead_drop_box` 解锁需走私者 NPC 师承（`workbench_recipes.rs` 已定 `Mentor { smuggler }`）。
- **qi_physics 锚点**：**无**——容器方块本身不碰真元（内含物若是高灵物的保鲜归 [[plan-container-filter-and-completion-v1]]）。

## P0 — ExternalContainerKind 扩展 + 放置/破坏链路 ⬜

- `external_container.rs:17` `ExternalContainerKind` 加 `StorageCrate` / `DeadDrop`。
- `trade_crate`（`workbench_materials.toml:281`）/`herb_crate`(:292，放置版)/`dead_drop_box`(:237) 加 placeable 标记 + 关联 `PlaceableBlockKind::Container`。
- 放置走 [[plan-workbench-place-runtime-v1]] 的 `handle_*_place` 底盘 → `commands.spawn` 带 `ExternalContainer { kind, container: ContainerState }` 的世界 entity（grid 用各容器 TOML 尺寸）；破坏 → 内容物掉落 + despawn。
- 测试：放置 spawn 带正确 kind/grid 的 ExternalContainer；破坏掉落内容物 + 容器本身；空容器与满容器破坏行为。

## P1 — 打开搜索链路 + IntentHandler ⬜

- 新建 `StorageCrateInteractIntentHandler` / `DeadDropInteractIntentHandler`（照 `SupplyCoffinInteractIntentHandler.java`：crosshair entity 检测 → candidate → dispatch `OpenContainer`）。
- `DefaultInteractionHandlers.java:15` 注册新 handler。
- 打开走 `LootContainerHandler` S2C（按 `source_kind` 区分 StorageCrate/DeadDrop）；内容操作复用 [[plan-nested-pack-base-v1]] / 外部容器移动协议。
- 测试（client）：crosshair 对准容器方块触发 OpenContainer；打开后内容 move 走对应 C2S；range 外不触发。

## P2 — bbmodel 资产 + 死信箱阵法 + 验收 ⬜

- **bbmodel 产出**（按 docs/CLAUDE.md §10.1：分部件 gen 脚本 → 逐件预览 → 拼接，3 轮自我打磨 + `<PROMISE>` 担保；参照 `scripts/models/gen_*_coffin.py`）：
  - `trade_crate` 货箱：六面木板钉成的大箱，4×4 体量，铁角加固。
  - `herb_crate` 草药筐：藤编敞口筐，内衬粗布，可见草药露头。
  - `dead_drop_box` 死信箱：埋地铁箍木箱，正面阵纹发光面（青色微光），防砸阵法视觉。
- **死信箱阵法防砸**（worldview §九:850）：阵纹 block state + 破阵机制（直接破坏被阵纹挡下，需先触发/破解）；解锁绑走私者师承。
- **视听**（可放置方块是玩家可感知）：放置 SFX `block.wood.place`；打开 `block.barrel.open`；死信箱破阵 = `BongSpriteParticle` 青色 `#3AA0C0` 12 粒 burst lifetime 20t + `block.amethyst_block.break` 音效。
- e2e：合成 → 放置 → 右键搜索打开 → 拖入拖出 → 破坏掉回；死信箱破阵流程。

## §8 开放问题（P0 决策门前需收口）

| # | 问题 | 推荐默认 |
|---|------|------|
| 1 | 死信箱首发完整形态（放置+阵法防砸+bbmodel）vs 先随身版？ | **placeable 完整形态**（依赖 [[plan-workbench-place-runtime-v1]] 就绪后）——worldview §九:850 原意就是世界放置的死信投递点。 |
| 2 | `herb_crate` 双形态：随身版（plan-A P5 已做）+ 放置版（本 plan）都要吗？ | 待用户定位。推荐**小筐随身 + 大仓放置**两形态共存，或仅保留随身版（本 plan 去掉 herb_crate）。 |
| 3 | bbmodel block state：vanilla chest 占位 vs `bong_blocks` 自定义方块？ | 跟随 [[plan-workbench-place-runtime-v1]] §8 #1 的决议（先 vanilla 占位，bbmodel 换皮）。 |
| 4 | 货箱/死信箱是否可被其他玩家/NPC 打开（共享/异步投递）？ | 死信箱设计意图是匿名异步投递；MVP 先做自己可开，跨主体投递登记 follow-up。 |

## §10 实施工作流

升 active 时按 docs/CLAUDE.md §6：P0 server 放置 / P1 client 交互 / P2 bbmodel+阵法+e2e，约 3 PR。**P2 含 bbmodel 视觉资产 → 强制走 §10.1 三轮自我打磨 + 终轮 commit `<PROMISE>` 担保块**（拼写 PROMISE）。依赖 [[plan-workbench-place-runtime-v1]] + [[plan-nested-pack-base-v1]] 均 merge 后开。

## Finish Evidence

（迁入前必填）
