# plan-bughunt-npc-territory-key-collision-v1（骨架）

> **来源 issue**：#1401。领地聚合 key 使用线性哈希，碰撞时把不同领地的影响力合并。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | territory 聚合使用无碰撞的完整身份 key，影响力和成员保持分区 | ⬜ |

## §0 摘要

`server/src/npc/territory.rs` 以线性组合生成聚合 key；不同坐标/territory identity 可以得到相同 key，后续 map 聚合会把两个领地当成一个。

## §1 游玩影响

NPC 领地的威胁、影响力或归属统计会串区，导致错误的 AI 决策和领地显示。该计划不改变影响力衰减公式。

## §2 复现路径

1. 构造两组会产生相同线性 hash 的领地坐标/身份。
2. 运行 territory aggregation，观察 map 只有一个 key、成员/影响力相加。
3. 比较使用完整身份后应有的两个独立聚合结果。

## §3 今天 `origin/main` 的证据

- `server/src/npc/territory.rs:306-368` 在 `:322` 使用线性 key 聚合，未保留足够的 territory identity 防碰撞。

## §4 非重复比对

已查 `plan-npc-ai`、territory active plans、world territory skeleton 的 key/影响力关键词；没有已有骨架覆盖 NPC territory 聚合 key。#1401 单独成因明确。

## §5 立项检查记录

- `docs/worldview.md`：查区域归属、领地/宗门边界和 NPC 社会规则。
- `docs/finished_plans/`：查 territory/war/social 计划，确认现有身份字段。
- active plan：查 `territory.rs` aggregation、key/hash、influence，未见同一实现改动。
- skeleton：查 #1401、线性 hash、territory key collision，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无 territory key 条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：territory identity、坐标/zone、成员和影响力样本。
- **Outputs**：server 内部按完整身份分开的聚合结果。
- **共享类型/事件**：复用既有 territory identity/zone 类型，改 key 表示，不新增相似 territory event。
- **三端契约符号**：server `npc::territory` 聚合；agent **无变更**、client **无变更**（现有输出字段仍表示每个 territory）。
- **worldview 锚点**：区域/领地边界；不涉及 qi_physics。

## P0 验收

- 构造碰撞对时仍产生两个独立聚合；同一完整身份仍稳定合并。
- 领地影响力、成员和输出排序不因 key 选择而串区。
