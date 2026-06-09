# plan-block-placement-base-v1 — 通用方块放置底座（补 workbench place/interact/break stub + WorkbenchPlace 协议）

> **来源**：手搓 104 产出物僵尸审计。「容器调查」发现可放置容器与「放置类 17 个僵尸物品」共享同一个放置底座，而该底座（`craft/workbench.rs:55-61` 三个 system）至今是 stub。用户决议：抽独立 plan，一处实装、两处复用。
> **依赖**：无。下游 [[plan-placeable-container-blocks-v1]] 及未来「放置类 17」plan 依赖本 plan。
> **状态**：骨架（草案）。`CoffinPlace` 已证明放置链路可行，本 plan 把同款底盘推广为通用放置底座。

`craft/workbench.rs` 已有 `WorkbenchBlock` 组件、`WorkbenchOpenPayload`、`is_within_workbench_range` 纯函数和测试，但 `handle_workbench_place` / `handle_workbench_interact` / `handle_workbench_break` 三个 Bevy system 仍是 stub（NOTE 标「将在 PR-3 实装」，PR-3 已 merge 但 system 没补）。结果：工作台**合得出却放不下** → 整棵 104 手搓配方树在世界里打不开。本 plan 补齐放置底盘，解锁所有可放置设施。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | `WorkbenchPlace`/`WorkbenchInteract` C2S 协议 + 实装 `workbench.rs` 三个 stub system | ⬜ |
| P1 | client 放置交互 + ghost 预览 + 破坏掉回物品 | ⬜ |
| P2 | 抽通用 `PlaceableBlockKind` 底盘 + 工作台端到端验收 | ⬜ |

## 接入面（防孤岛）

- **进料**：`craft/workbench.rs` 已有 `WorkbenchBlock`（:17）/`WorkbenchOpenPayload`（:28）/`WORKBENCH_INTERACT_RANGE`（:39）/`is_within_workbench_range`（:47）；`world/bong_blocks.rs` 的 `place_bong_block`/`remove_bong_block`/`is_bong_block` 自定义方块读写底座；`world/block_break.rs` 的 `DiggingEvent → set_block(AIR)` 统一破坏 apply system（已注册 Update）；`CoffinPlace`（`schema/client_request.rs:144`）/ `AlchemyOpenFurnace`（:83）已实装的放置/打开模板。
- **出料**：放下的方块 = 世界 entity（带 `WorkbenchBlock` 等组件）；右键交互 → range check → emit open payload（工作台 UI / 容器搜索）；破坏 → `DiggingEvent` → 掉回对应物品。
- **共享类型 / event**：新增 `WorkbenchPlace`/`WorkbenchInteract` C2S（**镜像 `CoffinPlace` 既有结构**，不另造放置协议）；复用 `set_block` / `bong_blocks` 读写 API，**不另造方块底座**。
- **跨仓库契约**：server `WorkbenchPlaceV1 { x, y, z, item_instance_id }`；client `ClientRequestSender.sendWorkbenchPlace` + ghost 预览 + 复用 `InteractIntent.OpenContainer`（`input/InteractIntent.java:8` 已存在）；**agent 无关**。
- **worldview 锚点**：手搓系统落地面（工作台/炼丹炉/容器是末法散修营地的基础设施）；无新境界 / 货币。
- **qi_physics 锚点**：**无**——纯方块放置/破坏，不碰真元。

## P0 — WorkbenchPlace 协议 + 三 stub 实装 ⬜

- `schema/client_request.rs` 新增（紧邻 `CoffinPlace:144`）：`WorkbenchPlace { v, x: i32, y: i32, z: i32, item_instance_id: u64 }` / `WorkbenchInteract { v, entity_id: u64 }`。
- 实装 `workbench.rs:55-61` 三个 system：
  - `handle_workbench_place`：校验目标格非 solid → `set_block` / `place_bong_block` → 消耗 1 个对应 item（背包扣除）→ `commands.spawn` 带 `WorkbenchBlock` 的世界 entity。
  - `handle_workbench_interact`：crosshair entity → `is_within_workbench_range` → emit `WorkbenchOpenPayload`（接现有工作台 UI）。
  - `handle_workbench_break`：`DiggingEvent` 命中 `WorkbenchBlock` → `remove_bong_block` → spawn 掉落物（掉回工作台 item）→ despawn entity。
- dispatch：`client_request_handler.rs:779-820`（`CoffinPlace`/`PlaceFurnace` 同款 place dispatch 段）加 `WorkbenchPlace`/`WorkbenchInteract` 臂。
- 测试：place 占位校验（目标 solid 拒绝）；place 扣 item + spawn entity；interact range 内/外；break 掉回 item + despawn；schema 正反 sample 对拍。

## P1 — client 放置交互 + ghost 预览 + 破坏 ⬜

- `ClientRequestSender.sendWorkbenchPlace(x,y,z,instanceId)`；放置 ghost 预览（半透明方块跟 crosshair，照 `CoffinPlace` client 侧若有则复用）。
- 打开走已存在的 `InteractIntent.OpenContainer` + `InteractKeyRouter`（`input/InteractKeyRouter.java:38`）路由；破坏走原版 digging → server `DiggingEvent`。
- **视听**：放置 SFX = `block.wood.place`；ghost 预览半透明 tint（白 opacity 0.4）；破坏走原版方块破坏粒子/音效。
- 测试（client）：放置请求字段正确；ghost 跟随 crosshair；range 外不可放（红 ghost）。

## P2 — 通用 PlaceableBlockKind 底盘 + 验收 ⬜

- 抽 `PlaceableBlockKind` 枚举（`Workbench` / 预留 `Container` / `Furnace` ...），`place/interact/break` 三 system 按 kind 分发，让工作台先跑通；下游 [[plan-placeable-container-blocks-v1]] 直接加 `StorageCrate`/`DeadDrop` 变体即可，不重写底盘。
- e2e：放工作台 → 右键打开手搓 UI → 合成 → 破坏掉回工作台。**这一步实测解锁「工作台合得出放不下」的 headline 阻塞。**

## §8 开放问题（P0 决策门前需收口）

| # | 问题 | 推荐默认 |
|---|------|------|
| 1 | 方块表示：复用 vanilla block state（crafting_table/chest）vs `bong_blocks` 自定义？ | **vanilla 占位先跑通**，`bong_blocks` 后续换皮/换 bbmodel（解耦放置逻辑与外观）。 |
| 2 | 放置占位校验粒度？ | `set_block` 前查目标格非 solid + 不在玩家碰撞箱内。 |
| 3 | 同种设施可否放多个/可堆叠？ | **可多个**，无全局唯一约束。 |
| 4 | 放置后实体持久化（重连后还在）？ | 走世界 entity 持久化；若现有 chunk 持久化不覆盖动态 entity，登记 follow-up。 |

## §10 实施工作流

升 active 时按 docs/CLAUDE.md §6：P0 协议+server / P1 client / P2 通用化+e2e，约 2-3 PR。纯逻辑无 bbmodel，§10.1 多轮不适用。可放置容器的 bbmodel 在 [[plan-placeable-container-blocks-v1]] 做。

## Finish Evidence

（迁入前必填）
