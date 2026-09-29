# plan-bughunt-coffin-same-tick-double-entry-v1

> 骨架：来源 #1447。同 tick 进入不同棺材时 deferred component 插入使两个入口都看到空闲。

## §0 摘要

`server/src/coffin/mod.rs:643-676` 以 deferred command 插入 `CurrentCoffin`/occupied 标记；两个 `enter` 请求在同一调度阶段读取 `current_coffin=None`，各自通过门禁并写入不同棺材，造成双占和旧棺永久锁死。

## §1 游玩影响

玩家快速进入两张棺或被两个系统同时传送时，退出只清一个 `occupied_by`，另一张棺永久显示有人占用。

## §2 复现路径

1. 同 tick 对玩家发两条不同 coffin enter。
2. 在 command flush 前观察两个 handler 都通过 `current_coffin` 检查。
3. flush 后核对玩家组件与两棺 `occupied_by`，出现一对多。

## §3 今天 `origin/main` 根因证据

- `coffin/mod.rs:643-676` 读状态后用 `commands.entity(...).insert` 延迟写入。
- 没有原子 reservation 或按玩家 compare-and-set，第二入口无法看到第一入口的 pending 状态。
- 退出/回收逻辑按单一 current coffin 清理，无法修复双占残留。

## §4 非重复比对

`plan-supply-coffin-same-tick-open-race-v1` 是外部供给棺 loot session race；本骨架是玩家进入普通棺的占用状态机，不共享同一组件写点之外的修法。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“棺、进入、魂魄占用”；`worldview.md §十二` 要求一个角色只有一个当前归宿。
- **finished_plans**：查 `CurrentCoffin`、`occupied_by`、enter/exit；未见 pending reservation。
- **active plan**：查 coffin enter/exit；无同 tick 互斥修复。
- **skeleton**：查 `coffin same tick`、`double entry`；无重复骨架。
- **reminder.md**：查 `coffin`、`occupied_by`、`reservation`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：coffin enter event、玩家 `CurrentCoffin`、coffin registry、tick。
- **Outputs**：单一 `CurrentCoffin`、一个 `occupied_by`、进入/拒绝事件。
- **共享类型或事件**：复用 coffin enter/exit events、`CurrentCoffin`、`occupied_by`；不新增平行锁。
- **server 符号**：`coffin::{enter_coffin,exit_coffin}`、`SupplyCoffinRegistry`/coffin state、lifecycle cleanup。
- **agent**：无变更；棺占用是 server 本地状态。
- **client**：无变更；既有 coffin payload/音效继续使用。
- **worldview 锚点**：`docs/worldview.md §十二` 棺与归宿。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 增加玩家级 pending reservation，原子提交单一 coffin 进入 |
| P1 | ⬜ | 同 tick 双进、跨棺切换、断线清理与幂等退出测试 |

## 来源 issue

- #1447 `[flash-review][major] 同 tick 进两张不同棺致 occupied_by 双占`
