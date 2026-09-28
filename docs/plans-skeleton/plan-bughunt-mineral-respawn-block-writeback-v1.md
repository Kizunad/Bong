# plan-bughunt-mineral-respawn-block-writeback-v1

> 骨架：来源 #1462。矿脉重生只生成实体/index，没有把矿石写回已加载 chunk 的方块。

## §0 摘要

`server/src/mineral/respawn.rs:29-68` 生成 `MineralOreNode` 和 registry/index，但没有 `set_block` 或 chunk writeback。已加载区块的旧空气仍是空气，实体与可采方块脱节。

## §1 游玩影响

玩家看到重生计时完成，却无法挖到矿；重登/区块重载前后表现不一致，矿产经济失去确定性。

## §2 复现路径

1. 挖掉一个已加载 chunk 的矿石，等待 respawn tick。
2. 观察 registry 出现新的 `MineralOreNode`，对应坐标方块仍为空气。
3. 通过挖掘/重载检查实体与方块的可交互性。

## §3 今天 `origin/main` 根因证据

- `mineral/respawn.rs:29-68` 仅 spawn node、更新 index。
- 没有调用 terrain/chunk block mutation；采集入口以方块状态为准时找不到矿。
- 重建实体并不能替代已加载世界的 block writeback。

## §4 非重复比对

`plan-mineral-respawn-tick-restart-drift-v1` 处理重启 tick 漂移；`plan-mineral-v1` 处理矿物 registry。本骨架只处理实体与方块状态的一致写回。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“矿脉、矿物、重生、采掘”；`worldview.md §九` 允许矿物作为资源而非货币，必须可采。
- **finished_plans**：查 `MineralOreNode`、respawn、chunk set_block；未见已加载区块 writeback。
- **active plan**：查 mineral respawn/restart；无同一方块落地修复。
- **skeleton**：查 `respawn block`、`MineralOreNode`、writeback；无重复骨架。
- **reminder.md**：查 `mineral`、`respawn`、`chunk`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：respawn tick、矿脉坐标/模板、`ChunkLayer`/terrain provider、`MineralRegistry`。
- **Outputs**：方块状态、`MineralOreNode`、可挖掘事件与掉落。
- **共享类型或事件**：复用 `MineralOreNode`、`MineralRegistry`、block mutation funnel；不另造矿物实体。
- **server 符号**：`mineral::respawn`、`world::terrain/chunk`、`block_break`/`MineralDropEvent`。
- **agent**：无变更；矿物方块不是 agent IPC。
- **client**：无变更；客户端通过既有 chunk/block update 得到正确方块。
- **worldview 锚点**：`docs/worldview.md §九` 矿物与资源。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 统一 respawn 实体、registry 与已加载 chunk 方块写回的原子入口 |
| P1 | ⬜ | 已加载/未加载 chunk、重启、重复 respawn 与挖掘回归测试 |

## 来源 issue

- #1462 `[flash-review][major] 矿脉重生只重建实体不重放方块`
