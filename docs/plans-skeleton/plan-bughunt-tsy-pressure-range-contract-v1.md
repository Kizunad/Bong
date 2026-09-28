# plan-bughunt-tsy-pressure-range-contract-v1（骨架）

> **来源 issue**：#1540。负灵压的 server payload 上限、client clamp 与满强度阈值没有同一范围契约。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 明确 local_neg_pressure 的有效范围并让满强度可达 | ⬜ |

## §0 摘要

client `TsyPressureOverlay.FULL_PRESSURE` 是 1.1，但 `PlayerStateViewModel.create` 将 `localNegPressure` 钳到 `[-1.0, 0.0]`；server `local_neg_pressure_from_sample` 也把负压绝对值 clamp 到 1.0。于是设计的 1.1 满强度、-4° FOV 和最高 vignette alpha 在生产 payload 中不可达。

## §1 游玩影响

负灵压越强，玩家看到的 FOV 收缩和边缘压迫感反而始终差一档，与 server 负灵压环境和 client 的满强度设计不一致。该问题只影响表现，不能通过 client 放大伪造 server 真元状态。

## §2 复现路径

在负灵压样本达到 1.0 或更高时接收 player state，检查 `local_neg_pressure`、`pressureIntensity` 和 `fovOffsetDegrees`；最大 intensity 低于 1.0。

## §3 今天 `origin/main` 的证据

- `server/src/network/mod.rs:2439-2445` 生成 `Some(-neg_pressure.clamp(0.0, 1.0))`；`server/src/schema/server_data.rs:356,1595` 将字段作为 `Option<f32>` 下发。
- `client/src/main/java/com/bong/client/state/PlayerStateViewModel.java:145` 再次 clamp 到 `[-1.0,0.0]`。
- `client/src/main/java/com/bong/client/visual/TsyPressureOverlay.java:12,42-50` 用 `FULL_PRESSURE=1.1` 计算满强度。

## §4 非重复比对

已查 neg-pressure ledger、player state schema、TSY overlay 的 finished/active plans 与 skeleton；现有计划覆盖负灵压扣真元与视觉接线，没有覆盖数值范围契约。#1540 独立保留。

## §5 立项检查记录

- `docs/worldview.md §二 L30-L57`：查负灵域和灵压方向；不改变负灵压抽取或强度定义。
- `docs/finished_plans/`：查 neg-pressure、player-state 和 TSY visual plan，确认 server ledger 与 client表现分层。
- active plan：查 `local_neg_pressure`、`FULL_PRESSURE`、`local_neg_pressure_from_sample`；未见统一范围决议。
- skeleton：查 `neg_pressure`、`TsyPressureOverlay`、`PlayerStateViewModel`；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：terrain sample 的 `neg_pressure`、server `local_neg_pressure` 字段和 client overlay。
- **Outputs**：统一范围下的 `PlayerStateViewModel.localNegPressure`、FOV/vignette；不得由 client 改写 qi_current。
- **共享类型/事件**：复用 `PlayerStateV1`/`ServerDataPayloadV1` 的 `local_neg_pressure`、`PlayerStateViewModel`、`TsyPressureOverlay`；范围常量若变更必须同步 schema 与测试。
- **三端契约符号**：server `network::local_neg_pressure_from_sample`、`player::state`；agent **无变更**，该字段由 server 直接发 client；client `PlayerStateHandler`/`PlayerStateViewModel`/`TsyPressureOverlay`。
- **worldview/qi_physics**：负灵域锚定 `docs/worldview.md §二`；真元抽取继续由 `qi_physics::ledger` 记录，视觉范围修复不能新增或重复 transfer。

## P0 验收

- 选定一个 server-authoritative 上限，使合法负压值能达到声明的满强度；server/client/schema 共享同一范围。
- `pressureIntensity`、FOV 和 vignette 在边界值有稳定断言，非法/非有限输入仍 fail closed。
