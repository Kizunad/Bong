# plan-bughunt-supply-coffin-same-tick-open-race-v1

> 骨架：来源 #1374。同一 tick 的供给棺打开请求在 session reservation 前均可 roll。

## §0 摘要

`server/src/supply_coffin/interact.rs:195-223` 先生成随机 loot、构造 `ExternalContainer`，再在 `:270` 用 deferred command 插入组件；`inventory/external_container.rs:55-65` 的 registry 只登记 session 到 entity，没有同 tick 的目标占用 reservation。两个请求都可能读到空目标并各自 roll。

## §1 游玩影响

玩家双击或两名玩家同时开同一供给棺时，可能重复获得 roll、后一个 session 覆盖前一个组件，部分 loot 无法访问且棺锁死。

## §2 复现路径

1. 同一 tick 发送两个 `OpenSupplyCoffinEvent`。
2. 两个系统迭代均看到目标没有 `ExternalContainer`，各自调用 `roll_loot`/`allocate_session`。
3. 观察 registry 有两个 session 而实体最终只保留一次 deferred 插入。

## §3 今天 `origin/main` 根因证据

- `interact.rs:195-212` 在任何独占登记前执行随机 roll 与 session 分配。
- `:215-223,270` 只在 command 队列 flush 时插入 `ExternalContainer`。
- `external_container.rs:55-65` registry 没有目标级 compare-and-set 或 pending reservation。

## §4 非重复比对

`plan-supply-coffin-cooldown-restart-rollback-v1` 处理重启冷却回滚；`plan-coffin-same-tick-double-entry-v1` 处理普通棺的 enter 防重入。本骨架限定供给棺的外部 session 与 loot roll 原子性。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“供给棺、开棺、物资唯一性”；`worldview.md §十二` 的秘境物资不应重复生成。
- **finished_plans**：查 `ExternalContainerRegistry`、`SupplyCoffinGrade`、`LootContainerOpenV1`；无 pending reservation 契约。
- **active plan**：查 `allocate_session`、`OpenSupplyCoffinEvent`；未见同 tick 互斥实现。
- **skeleton**：查 `supply coffin race`、`pending reservation`；无同根因骨架。
- **reminder.md**：查 `supply_coffin`、`same tick`、`session`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`OpenSupplyCoffinEvent`、`ExternalContainerRegistry`、棺 grade/位置、当前 tick。
- **Outputs**：单一 `LootContainerOpenV1`、一个 `ExternalContainer` session、一次 `roll_loot`。
- **共享类型或事件**：复用 `ExternalContainer`、`ExternalContainerRegistry`、`LootContainerOpenV1`、`ExternalContainerKind::SupplyCoffin`。
- **server 符号**：`supply_coffin::interact::open_supply_coffin`、`inventory::external_container`、`network::server_data`。
- **agent**：无变更；供给棺 session 不经 Redis。
- **client**：无变更；仍消费一个 `LootContainerOpenV1`，重复请求只收到既有拒绝/重开结果。
- **worldview 锚点**：`docs/worldview.md §十二` 秘境物资与唯一掉落。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 在 roll 前建立目标级 pending reservation，保证同 tick 只有一个 opener |
| P1 | ⬜ | 并发请求、序列重开、payload 失败回滚和 registry 一致性测试 |

## 来源 issue

- #1374 `[flash-review][major] 同tick双开同一物资棺重复roll并丢失loot`
