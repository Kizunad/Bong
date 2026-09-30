# plan-bughunt-botany-trample-harvest-race-v1

> 骨架：来源 #1405。非会话踩踏只检查已收获/已踩踏标志，没有排除正在进行的 HarvestSession。

## §0 摘要

`server/src/botany/harvest.rs:828-885` 的 `detect_non_session_trample` 仅以 `harvested`/`trampled` 判定；正在采集的植物仍可被非会话踩踏路径处理，之后 harvest session 继续结算，产生双重状态和灵气计费。

## §1 游玩影响

玩家采集时另一名玩家踩过植物，会同时触发踩踏损失与采集收益，植物灵气、掉落和耐久不一致。

## §2 复现路径

1. 创建未收获植物并启动 `HarvestSession`。
2. 在 session 完成前运行非会话移动/踩踏检测。
3. 观察植物被标记 trampled，随后 session 仍成功结算。

## §3 今天 `origin/main` 根因证据

- `harvest.rs:828-885` 的非会话检测 query 没有 `HarvestSessionStore`/植物 session owner 过滤。
- 只跳过 `harvested` 与 `trampled` 状态，不能表达“已被某 session 保留”。
- 完成路径仍可从 session 读取同一植物并发放掉落/qi 结果。

## §4 非重复比对

`plan-gathering-tool-bind-v1` 关注工具绑定，`plan-botany-growth-cost-harvest-ledger-v1` 关注成长成本守恒；本骨架只处理踩踏与 harvest session 的互斥。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“灵植、采集、踩踏、灵气”；`worldview.md §七` 要求采集行为与生态状态一致。
- **finished_plans**：查 `HarvestSession`、`detect_non_session_trample`、`Plant`；未见 session reservation 门禁。
- **active plan**：查 botany harvest/trample；无同一竞态修复。
- **skeleton**：查 `trample`、`HarvestSession`、`harvested`；无同根因骨架。
- **reminder.md**：查 `botany`、`trample`、`harvest session`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：玩家移动/碰撞、`Plant`、`HarvestSessionStore`、tick/位置。
- **Outputs**：一次性 `HarvestOutcome`/植物状态、掉落与 qi ledger 结果。
- **共享类型或事件**：复用 `HarvestSession`、`Plant`、`HarvestEvent`；不另造植物占用表。
- **server 符号**：`botany::harvest::{detect_non_session_trample,complete_harvest}`、`HarvestSessionStore`。
- **agent**：无变更；botany session 不经 agent IPC。
- **client**：无变更；客户端只渲染既有植物/掉落事件。
- **worldview 锚点**：`docs/worldview.md §七` 灵植生态与采集。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 给正在进行的 session 建立植物级互斥，非会话踩踏 fail closed |
| P1 | ⬜ | 并发踩踏、取消、完成、重复 tick 的最小契约测试 |

## 来源 issue

- #1405 `[flash-review][major] 非会话踩踏不排除正在采集的植物`
