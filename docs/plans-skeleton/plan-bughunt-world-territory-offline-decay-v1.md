# plan-bughunt-world-territory-offline-decay-v1（骨架）

> **来源 issue**：#1356。territory influence 聚合只查询带 `Client` 的在线玩家，离线霸主永不衰减。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 领地影响力衰减覆盖持久化的离线霸主，并与在线路径使用同一时间基线 | ⬜ |

## §0 摘要

`territory` 的查询带 `With<Client>`，只遍历在线玩家后计算 influence decay。离线霸主不进入衰减，重连前一直占据领地。

## §1 游玩影响

领地控制权与在线活动脱节；玩家离线后仍永久保持高影响力，其他 NPC/玩家无法通过时间和行动接管。无新增网络 payload。

## §2 复现路径

1. 让玩家成为 territory 霸主并记录 influence。
2. 断线，推进多个 territory decay tick。
3. 观察影响力不变；重新上线后才进入在线 query，造成时间跳变。

## §3 今天 `origin/main` 的证据

- `server/src/world/territory.rs:317-331,339-430` 的 query 带 `With<Client>`，`:339-430` 的衰减循环只处理在线玩家。

## §4 非重复比对

已查 territory、social、NPC influence finished/active plans 与 skeleton 的 offline/decay/query；没有已有骨架覆盖离线霸主。#1356 单独成因明确。

## §5 立项检查记录

- `docs/worldview.md`：查区域控制、领地势力和离线世界持续性。
- `docs/finished_plans/`：查 territory/war/social 的 influence 与持久化约定。
- active plan：查 `territory.rs`、`With<Client>`、decay，未见同一查询改动。
- skeleton：查 #1356、offline influence、territory decay，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无 offline decay 条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：持久化 character/territory membership、在线/离线时间、zone influence state。
- **Outputs**：server territory influence 与控制权更新；现有客户端/agent 状态按原协议读取。
- **共享类型/事件**：复用 territory persistence、influence decay 函数和角色身份；不新造 online-only state。
- **三端契约符号**：server `world::territory` query/decay；agent **无变更**、client **无变更**（离线计算补齐 server 内部结果）。
- **worldview 锚点**：区域势力/领地控制；无真元转移，不调用 `qi_physics`。

## P0 验收

- 玩家离线期间 influence 按同一时间规则衰减，重连不会一次性跳变或永久占领。
- 在线玩家、NPC 和无持久化身份的实体仍遵守既有过滤和上限。
