# plan-bughunt-npc-patrol-progress-v1（骨架）

> **来源 issue**：#1762。NPC 到达当前巡逻锚点时先把 `current_target` 设为同一 `anchor_index` 的下一个值，再推进 index，首个 tick 会空转一拍。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 到达锚点后一次性选择并推进到真正的下一锚点 | ⬜ |

## §0 摘要

`patrol_npcs` 的到点分支在更新 `anchor_index` 前调用 `zone.patrol_target(patrol.anchor_index)`，随后才调用 `next_anchor_index`。下一次 tick 才会把新 index 转成目标，造成空转和节奏偏差。

## §1 游玩影响

巡逻 NPC 在每个锚点停顿额外一 tick；多个 NPC 同步时会出现不自然的停顿和路径拥挤，但不会改变战斗或真元。

## §2 复现路径

1. 建立两个 `patrol_anchors`，把 NPC 放在第一个锚点。
2. 跑 `patrol_npcs` 连续 tick，记录 `anchor_index/current_target` 和 `Navigator` goal。
3. 第一 tick 后 index 已递增但 goal 仍由旧 index 派生，下一 tick 才切换。

## §3 今天 `origin/main` 的证据

- `server/src/npc/patrol.rs:99-113` 到点分支在 `:105` 先读取当前 `anchor_index`，再递增；goal 在后面的 `set_goal` 才使用新字段。
- `next_anchor_index` 位于同文件 `:120-125`，确认推进规则本身没有越界问题。

## §4 非重复比对

已查 `plan-npc-ai`、active plan 和 skeleton 的 `patrol_npcs`、`Navigator`、`next_anchor_index`；没有覆盖该 tick 顺序。#1762 单独成因明确，不并入迁移 timeout 或社交缓存计划。

## §5 立项检查记录

- `docs/worldview.md`：查 NPC 行为、区域巡逻和 zone 关键词；不改正典。
- `docs/finished_plans/`：查 `plan-npc-ai-v1` 与 zone patrol 交付物。
- active plan：查 `NpcPatrol`、`Navigator`、`patrol_npcs`，未见同入口改动。
- skeleton：查 #1762、`next_anchor_index`、`current_target`，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无巡逻条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：`NpcPatrol`、`Zone::patrol_anchors`、当前位置、`Navigator`。
- **Outputs**：server 更新 `anchor_index/current_target` 并设置导航 goal。
- **共享类型/事件**：复用 `NpcPatrol`、`Navigator`、`Zone`；不新增事件。
- **三端契约符号**：server `patrol_npcs`；agent **无变更**、client **无变更**（导航是服务端内部行为，现有实体同步不变）。
- **worldview 锚点**：区域巡逻属于 NPC 行为；无真元流动，不调用 `qi_physics`。

## P0 验收

- 到点的同一 update 后 `current_target` 和 goal 都指向下一锚点。
- 空 anchors、单 anchor、季节半径缩放仍保持原有 fallback 行为。
