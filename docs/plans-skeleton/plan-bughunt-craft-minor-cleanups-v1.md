# plan-bughunt-craft-minor-cleanups-v1

> **来源 issue**：#1346。
> 一句话主题：制作台拆除用延迟 `Despawned` 标记，未在同一帧为同一实体建立去重占用，重复挖掘会重复返还物品。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | workbench break 的实体去重占用与发放/掉落原子顺序 | ⬜ |
| P1 | 重复 DiggingEvent、失败返还和维度边界回归 | ⬜ |

## §0 摘要

`handle_workbench_break` 在 `server/src/craft/workbench.rs:197-221` 以坐标查询制作台，但 query 没有 `Without<Despawned>`；`break_placeable` 直到 `:286-297` 才通过 deferred `Commands` 插入标记。两个同帧 `DiggingEvent` 都能在命令应用前命中同一个 `WorkbenchBlock`，并分别在 `:239-285` 发放或掉落 `workbench_item`，造成一台制作台返还两份物品。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` 的凡物制作台、物品归还和交易物品守恒关键词；不改变制作台配方或距离规则。
- **finished_plans**：查 workbench recipe、block-place funnel、inventory grant/duplicate prevention 归档计划，确认同帧拆除去重尚未由已有计划拥有。
- **active plan**：查 `docs/plan-*.md` 的 `WorkbenchBlock`、`break_placeable`、`DiggingEvent`、inventory grant；没有一份覆盖本实体的 deferred-despanw 窗口。
- **skeleton**：查 `plan-block-break-integration-v1.md`、inventory interaction 和 craft close/pause 骨架；它们处理通用破坏或手搓会话，不提供 workbench break claim。
- **reminder.md**：查 `docs/plans-skeleton/reminder.md` 的 `WorkbenchBlock`、`break_placeable`、`Despawned`、`workbench_item`；仓内有该文件，但没有同帧返还去重登记。

## §2 接入面与跨仓契约

- **Inputs**：server `DiggingEvent`、玩家 `GameMode`/`Position`/`CurrentDimension`、`WorkbenchBlock` query、`PlayerInventory` 与 `DroppedLootRegistry`。
- **Outputs**：同一 workbench entity 在一次成功返还/掉落后最多产生一个 `workbench_item`，随后由 `break_placeable` 标记 `Despawned`；返还失败且掉落也失败时不标记、不丢失实体。
- **共享类型/事件**：复用 `WorkbenchBlock`、`DiggingEvent`、`break_placeable(PlaceableBlockKind::Workbench)`、`add_item_to_player_inventory`、`spawn_template_dropped_loot` 和既有 inventory snapshot；不新增 client item schema。
- **三端契约符号**：server `craft::handle_workbench_break`、`world::block_place::break_placeable`、`network::inventory_snapshot_emit::send_inventory_snapshot_to_client`；client **无变更**，依据是客户端只接收既有 inventory snapshot；agent **无变更**，制作台拆除是 server gameplay，不经过 Redis IPC。
- **worldview/qi**：制作台是凡物，不涉及 `Cultivation.qi_current` 或 `QiTransfer`；修复只保护物品实例守恒，不能以重复返还测试替代真元守恒测试。

## §3 游玩影响与复现

生存模式向同一制作台在一个 Update 内发送两个 Stop 状态的 `DiggingEvent`（或用 bot/伪造包达到同样事件序列）。两个事件都通过坐标 query，玩家背包或地面最终出现两个 `workbench_item`，而原实体只在 deferred command 阶段才带 `Despawned`。

## §4 `origin/main` 根因证据

- `server/src/craft/workbench.rs:194-221` 的 `workbenches` query 没有 `Without<Despawned>`，同帧重复事件可重复命中实体。
- `server/src/craft/workbench.rs:239-285` 在实体标记前完成每次 inventory grant/ground drop，第二个事件不会看到第一个事件的 deferred 状态。
- `server/src/world/block_place.rs:518-531` 的 `break_placeable` 只调用 `commands.entity(entity).insert(Despawned)`，不是立即从 query 移除的 claim。

## §5 非重复比对

`plan-block-break-integration-v1.md` 负责通用破坏 funnel 和距离/保护门；craft close/pause 负责手搓 session；本骨架只补 workbench entity 的同帧 claim 与物品发放顺序，不重复改通用方块协议。

## §6 修复计划骨架

- **P0**：在 workbench break query 加 `Without<Despawned>`，并在本次 Update 内以 `WorkbenchBreakClaims`（或等价的本地/Resource 集合）原子占用 entity；只有返还背包或成功生成地面掉落后才调用 `break_placeable`，重复事件直接跳过。
- **P1**：返还和掉落都失败时释放 claim、保留制作台；覆盖两个同帧事件、跨 tick 重复事件、背包满、掉落成功、异维同坐标和已 `Despawned` 实体，确保每个实体最多一份物品。

## §7 验证计划

实现后只跑 server 栈 fmt、clippy、cargo test，重点 `craft::workbench`、`world::block_place` 和 inventory grant；agent/client 无代码改动，不跑其门禁。
