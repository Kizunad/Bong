# plan-bughunt-shiling-xian-initial-seed-v1

> 骨架：来源 #1416。噬灵藓扩散只从已有植株读取，没有生产首株来源。

## §0 摘要

`server/src/botany/shiling_xian.rs:245-261` 的扩散系统只遍历已有 `ShiLingXian` 并为邻格生成后代；生产 spawn 链没有初始种子、配方或世界事件，空世界因此永远没有第一株。

## §1 游玩影响

噬灵藓相关采集、危险地块和生态叙事在新世界从未出现，依赖它的配方成为不可获得内容。

## §2 复现路径

1. 新建没有 `ShiLingXian` 的 zone/存档。
2. 等待 botany tick、重启并重新 hydrate。
3. 观察扩散系统没有源 entity，数量永远为零。

## §3 今天 `origin/main` 根因证据

- `shiling_xian.rs:245-261` 的唯一生产逻辑以已有植株为输入。
- 全仓 spawn/hydrate 入口没有首株 seed、worldgen 标记或管理员投放的生产契约。
- 因此扩散是闭合的空集合，不会凭时间自发产生植株。

## §4 非重复比对

`plan-botany-growth-cost-harvest-ledger-v1` 处理既有植物的成长扣费；`plan-botany-v1` 处理 registry/采集落地。本骨架只定义首株来源和一次性初始化，不改扩散公式。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“噬灵藓、灵植、荒野首株”；`worldview.md §七` 要求生态有可追溯来源。
- **finished_plans**：查 `ShiLingXian`、botany spawn、seed；未找到生产首株契约。
- **active plan**：查 `spawn_shiling_xian`、hydrate botany；无正在接入的首株来源。
- **skeleton**：查 `shiling`、`initial seed`、`first spawn`；无同根因骨架。
- **reminder.md**：查 `噬灵藓`、`seed`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：worldgen zone、botany registry、存档 hydrate、可配置 seed source。
- **Outputs**：首个 `ShiLingXian` entity、后续扩散事件、采集可用性。
- **共享类型或事件**：复用 `ShiLingXian`、`PlantRegistry`、`BotanySpawnEvent`；不另造平行植物类型。
- **server 符号**：`botany::shiling_xian::{spawn,spread}`、`PlantRegistry`、persistence hydrate。
- **agent**：无变更；植物 spawn 不进入 agent schema。
- **client**：无变更；复用既有植物 entity/粒子渲染。
- **worldview 锚点**：`docs/worldview.md §七` 灵植生态与末法荒野。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 定义 deterministic 首株来源并接入新世界/存档初始化 |
| P1 | ⬜ | 无源 zone、重复 hydrate、跨维度扩散回归测试 |

## 来源 issue

- #1416 `[flash-review][major] 噬灵藓无初始播种`
