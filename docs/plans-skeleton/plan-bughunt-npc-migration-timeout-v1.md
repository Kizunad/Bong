# plan-bughunt-npc-migration-timeout-v1（骨架）

> **来源 issue**：#1754。散修 farming migration 进入 `Executing` 后只有距离/目标更新，没有超时或失败转移。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | migration 执行状态拥有时钟/尝试上限，目标不可达时可观察地退出或重试 | ⬜ |

## §0 摘要

`farming_brain` 对 `Executing` 只做 distance check 和 `set_goal`，目标区块、导航器或 zone 不可用时状态永久保持，NPC 不能回到可调度状态。

## §1 游玩影响

散修会卡在迁移动作，长期失去耕作、社交和其他 AI 行为；状态表持续占用且看似仍在执行。没有新的真元路径。

## §2 复现路径

1. 让 NPC 进入 migration `Executing`，目标设置为不存在或不可达位置。
2. 持续推进 farming tick，使距离永远不满足完成条件。
3. 观察 `Executing` 永不超时，也不产生失败/重选目标。

## §3 今天 `origin/main` 的证据

- `server/src/npc/farming_brain.rs:715-760` 的 `Executing` 分支只检查距离并调用 `set_goal`，没有 elapsed tick、deadline 或 retry budget。

## §4 非重复比对

已查 `plan-npc-ai`、farming/migration active plan 和 skeleton 的 `Executing`、`set_goal`、timeout 关键词；没有现有骨架覆盖该状态机。#1754 单独成因，不与巡逻锚点合并。

## §5 立项检查记录

- `docs/worldview.md`：查散修/耕作、区域迁移和 NPC 生存行为；不改世界观。
- `docs/finished_plans/`：查 NPC AI/farming 计划及现有失败状态约定。
- active plan：查 `farming_brain`、`Executing`、migration deadline，未发现同一分支修改。
- skeleton：查 #1754、`set_goal`、migration timeout，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无 farming migration 条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：migration state、目标位置/zone、`CultivationClock` 或等价 tick、Navigator 状态。
- **Outputs**：迁移成功/失败/重试状态和 NPC 导航目标；没有新增跨仓 payload。
- **共享类型/事件**：复用 farming brain 状态、`Navigator`、zone registry；不另造 timeout 状态。
- **三端契约符号**：server `farming_brain`；agent **无变更**、client **无变更**（NPC AI 内部状态不改变 IPC/UI schema）。
- **worldview 锚点**：散修迁移与区域生活规则；不涉及 qi_physics。

## P0 验收

- 不可达目标在确定 deadline/attempt budget 后退出 `Executing`，并留下可审计原因。
- 可达目标仍按现有成功路径完成，重试不会重复创建 NPC 或物品。
