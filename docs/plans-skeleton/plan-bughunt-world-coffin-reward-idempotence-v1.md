# plan-bughunt-world-coffin-reward-idempotence-v1（骨架）

> **来源 issue**：#1392。tutorial coffin 先 add item，成功后才写 opened marker/hook；崩溃窗口可重复领取奖励。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 开棺奖励判重与物品发放具备崩溃可恢复的幂等顺序 | ⬜ |

## §0 摘要

`spawn_tutorial` 的开棺链路在 `add_item` 后才记录 `opened_coffin_pos`/hook。进程在两者之间退出时，重连可再次通过“未打开”判断并重复发放奖励。

## §1 游玩影响

玩家能复制教程奖励，破坏早期资源经济和开棺事件统计；修复必须保留背包满、离线和重试时的可靠物品语义。

## §2 复现路径

1. 触发教程 coffin 开棺，使物品已经进入 inventory。
2. 在 `opened_coffin_pos`/hook 写入前模拟进程崩溃或事务中断。
3. 重连再次开棺，观察同一位置再次 add reward。

## §3 今天 `origin/main` 的证据

- `server/src/world/spawn_tutorial.rs:635-662` 先 add inventory，成功后才写 `opened_coffin_pos` 与 hook 标记；中间没有持久化幂等点。

## §4 非重复比对

已查 coffin/container、tutorial、inventory finished/active plans 与 skeleton 的 `opened_coffin_pos`、reward idempotence；没有已有计划覆盖此崩溃窗口。#1392 单独成因明确。

## §5 立项检查记录

- `docs/worldview.md`：查死信箱/教程开棺、骨币经济和奖励边界；不改奖励内容。
- `docs/finished_plans/`：查 spawn tutorial、coffin/container、inventory 持久化约定。
- active plan：查 `spawn_tutorial`、`opened_coffin_pos`、reward transaction，未发现同入口修改。
- skeleton：查 #1392、coffin reward、idempotence、crash window，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无 tutorial coffin 条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：玩家/位置/教程 coffin identity、inventory add result、opened marker/hook 存储。
- **Outputs**：一次性奖励、持久化打开状态和重试/失败结果。
- **共享类型/事件**：复用 `opened_coffin_pos`、tutorial hook、inventory transaction；不另造奖励记录。
- **三端契约符号**：server `spawn_tutorial`、inventory persistence；agent **无变更**、client **无变更**（奖励协议不改变，只保证一次性）。
- **worldview 锚点**：教程棺材/死信箱和骨币经济；不新增 qi_physics 流动。

## P0 验收

- 在任意崩溃/重试点最多产生一次奖励；已标记 opened 的位置永不重复发放。
- 背包满或 add 失败时 marker 与奖励状态可恢复，不丢失也不重复。
