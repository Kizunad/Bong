# plan-bughunt-lingtian-harvest-full-inventory-spill-v1

> 骨架：来源 #1829。灵田收获在背包满时只记录 warning，仍清空作物和种子。

## §0 摘要

`server/src/lingtian/systems.rs:1484-1496` 与 `:1515-1525` 对物品/种子写入背包失败只 `warn`；`:1539-1548` 仍清空 crop。没有地面掉落或原子回滚，作物和种子在满背包时凭空消失。

## §1 游玩影响

玩家收获前未清理背包会损失成熟作物和下一轮种子，灵田生产链被不可预期地截断。

## §2 复现路径

1. 将背包填满，等待灵田 crop 成熟。
2. 触发 harvest/growth tick。
3. 观察 add item 返回失败、日志 warning，但 crop/seed 状态仍被清空。

## §3 今天 `origin/main` 根因证据

- `lingtian/systems.rs:1484-1496` 产物入包失败仅日志。
- `:1515-1525` 种子同样失败不补偿。
- `:1539-1548` 无条件清空作物，缺少地面 spill/transaction rollback。

## §4 非重复比对

`plan-bughunt-forge-outcome-full-inventory-loss-v1`/alchemy takeback 处理锻造/炼丹满包；本骨架只处理 lingtian crop/seed 收获，不能用通用“忽略失败”语义。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“灵田、收获、种子、生产”；`worldview.md §七/§八` 要求产出不因容量静默消失。
- **finished_plans**：查 `lingtian`、crop harvest、inventory add/spill；未见该收获事务补偿。
- **active plan**：查 lingtian systems；无同一满包修复。
- **skeleton**：查 `lingtian full inventory`、`spill`、`seed`；无重复骨架。
- **reminder.md**：查 `lingtian`、`crop`、`inventory full`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：成熟 crop、seed outcome、玩家 inventory capacity、world position/zone。
- **Outputs**：背包物品或 `DroppedItemEvent`，成功后才清空 crop/seed；失败保持可重试。
- **共享类型或事件**：复用 inventory add result、`DroppedLootRegistry`/`DroppedItemEvent`、`Crop`/`Lingtian` components。
- **server 符号**：`lingtian::systems::{harvest,clear_crop}`、`inventory` drop funnel。
- **agent**：无变更；灵田收获不经 agent。
- **client**：无变更；既有 inventory/drop payload 足以表现 spill。
- **worldview 锚点**：`docs/worldview.md §七` 灵田生态与 §八`生产`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 以 add-or-spill 原子结果为准，失败不清空 crop/seed |
| P1 | ⬜ | 满包、部分容量、多产物、重复 tick 和重连回归测试 |

## 来源 issue

- #1829 `[flash-review][major] 收获满背包时作物种子被销毁而非掉地`
