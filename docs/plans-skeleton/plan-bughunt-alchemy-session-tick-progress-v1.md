# plan-bughunt-alchemy-session-tick-progress-v1

> 骨架：来源 #1460。生产炼丹没有推进 session elapsed_ticks，中途投料窗口永远不可达。

## §0 摘要

`server/src/alchemy/session.rs:114-133` 以 `elapsed_ticks` 判断中途投料窗口；生产 `alchemy/mod.rs` 没有对应 session tick 系统，只有测试在 `:609-610` 手工推进。因此多段丹方一直停在 `Flawed`/初始阶段。

## §1 游玩影响

玩家按丹方说明在正确时间投料也不会命中窗口，材料和真元被消耗后只能得到失败结果，中途交互玩法不可用。

## §2 复现路径

1. 启动含 12+ tick 投料窗口的炼丹 session。
2. 等待生产 tick，不发送额外请求。
3. 观察 `elapsed_ticks` 保持初始值，`session_tick` 相关窗口永远不变；测试因手工调用而通过。

## §3 今天 `origin/main` 根因证据

- `alchemy/session.rs:114-133` 的窗口判定只依赖 `elapsed_ticks`。
- `alchemy/mod.rs` 生产系统没有遍历 active sessions 并递增 elapsed；全仓对应调用仅在测试 `:609-610`。
- 因而测试/生产调度语义分叉。

## §4 非重复比对

`plan-alchemy-v1` 定义炼丹步骤与结果，`plan-bughunt-alchemy-furnace-slot-takeback-v1` 处理槽位同步；本骨架只接通 session 时间推进。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“炼丹、投料窗口、火候”；`worldview.md §八` 要求火候按时间推进。
- **finished_plans**：查 `AlchemySession`、`elapsed_ticks`、session tick；未见生产推进器。
- **active plan**：查 alchemy session/mod；无同一系统接线修复。
- **skeleton**：查 `alchemy session tick`、`elapsed_ticks`、`Flawed`；无重复骨架。
- **reminder.md**：查 `alchemy`、`elapsed_ticks`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：active `AlchemySession`、`GameTick`、recipe step timing。
- **Outputs**：session progress、投料窗口结果、`AlchemyOutcomeEvent`/snapshot。
- **共享类型或事件**：复用 `AlchemySession`、`AlchemyStep`、`AlchemyOutcomeEvent`；不新增客户端计时权威。
- **server 符号**：`alchemy::session::{tick,advance}`、`alchemy::mod`、`network::alchemy_snapshot_emit`。
- **agent**：无变更；炼丹 session 不经 agent IPC。
- **client**：无变更；客户端显示 server 下发的 progress，不能自行推进权威 tick。
- **worldview 锚点**：`docs/worldview.md §八` 炼丹火候。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 将 active session tick 纳入生产调度并定义取消/暂停语义 |
| P1 | ⬜ | 多 recipe、重启/断线、边界 tick 与重复 tick 回归测试 |

## 来源 issue

- #1460 `[flash-review][major] 中途投料窗口永不可命中`
