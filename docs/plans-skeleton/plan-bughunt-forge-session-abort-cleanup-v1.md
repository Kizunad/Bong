# plan-bughunt-forge-session-abort-cleanup-v1

> 骨架：来源 #1436、#1828。锻造会话清理只删除 Done，断线/取消/失败会永久占台。

## §0 摘要

`server/src/forge/session.rs:211-230` 的 `cleanup_completed` 只 retain 非 `Done` 以外的条目；生产 `forge/mod.rs:1258-1269` 仅调用该函数。非 Done 的 `Aborted`/断线/超时会留在 session map，继续阻止同一砧台并造成内存泄漏。

## §1 游玩影响

玩家中途下线或取消锻造后，砧台显示仍被占用，无法开始新会话；长服运行会积累失效 session。

## §2 复现路径

1. 在 ForgeStation 创建 session，断线、超时或触发非 Done 失败。
2. 运行生产清理 tick，观察 entry 仍在 map。
3. 再次使用同一 station，命中旧 session 的占用门禁。

## §3 今天 `origin/main` 根因证据

- `forge/session.rs:211-230` cleanup predicate 只识别 `ForgeSessionStatus::Done`。
- `forge/mod.rs:1258-1269` 没有断线/超时/取消状态转换再清除的补偿逻辑。
- `ForgeSession` 的 station/owner 引用因此长时间存活。

## §4 非重复比对

`plan-forge-v1` 处理正常步骤，`plan-bughunt-morph-disconnect-release-save-order-v1` 处理 morph 保存顺序；本骨架只处理 forge session 的终态分类和回收。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“锻造台、断线、材料锁定”；`worldview.md §八` 要求失败材料可追溯且工作台可再用。
- **finished_plans**：查 `ForgeSessionStatus`、`cleanup_completed`、disconnect；无完整终态清理协议。
- **active plan**：查 forge session cleanup；无同一占用泄漏修复。
- **skeleton**：查 `forge session abort`、`cleanup_completed`、`station lock`；无重复骨架。
- **reminder.md**：查 `forge`、`session`、`断线`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`ForgeSession`、owner/client disconnect、timeout/cancel/failure outcome、station registry。
- **Outputs**：终态结果、材料返还/掉落、session registry 删除、forge snapshot inactive。
- **共享类型或事件**：复用 `ForgeSessionStatus`、`ForgeOutcomeEvent`、`ForgeSessionStore`；不另造占台标志。
- **server 符号**：`forge::session::{cleanup_completed,ForgeSessionStore}`、`forge::mod`、`network::forge_snapshot_emit`。
- **agent**：无变更；forge session 不走 agent IPC。
- **client**：无变更；复用结束快照的 `active=false`。
- **worldview 锚点**：`docs/worldview.md §八` 锻造台与资源归还。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 定义所有终态并统一断线/取消/超时清理与补偿 |
| P1 | ⬜ | 多会话、重复清理、断线重连和长期容量回归测试 |

## 来源 issue

- #1436 `[flash-review][major] 玩家中途下线后 ForgeSession 永不回收`
- #1828 `[flash-review][major] 非 Done 锻造会话永不清理`
