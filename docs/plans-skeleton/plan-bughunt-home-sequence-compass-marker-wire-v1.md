# plan-bughunt-home-sequence-compass-marker-wire-v1（骨架）

> **来源 issue**：#1694。灵龛回家序列依赖的 `SPIRIT_NICHE` compass marker 没有进入生产 runtime context。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 让已揭示灵龛坐标进入 `HudRuntimeContext` | ⬜ |
| P1 | 距离、进入、NEW badge 和音效回归 | ⬜ |

## §0 摘要

`HomeSequence.nearestHomeMarker` 只扫描 `runtime.compassMarkers()` 中的 `SPIRIT_NICHE`，但 `BongHud.captureRuntimeContext` 永远传 `List.of()`；生产没有任何 `withCompassMarkers` 调用。因此 `insideHome` 永远为 false，回家面板、settle 动画、音效和 NEW badge 不会触发。

## §1 游玩影响

玩家即使已经揭示灵龛并回到其附近，也看不到“回到灵龛”的整理/结算反馈，且背包收获标记不会按回家序列出现。

## §2 复现路径

完成灵龛坐标揭示后走到灵龛 5 格内，观察 `HudRuntimeContext.compassMarkers` 仍为空，`HomeSequence.update` 每 tick 返回 `State.away()`。

## §3 今天 `origin/main` 的证据

- `client/src/main/java/com/bong/client/loop/HomeSequence.java:48-67,136-151` 只接受 `HudRuntimeContext` marker，并以 `SPIRIT_NICHE` 距离判定进入。
- `client/src/main/java/com/bong/client/BongHud.java:454-475` 构造 runtime context 时固定传 `List.of()`；`HudRuntimeContext.withCompassMarkers` 全仓生产代码无调用。
- server 已有 `social::handle_spirit_niche_coordinate_reveals`（`server/src/social/mod.rs:2242-2280`）和 `SpiritNicheReveal` 事件，但没有与 `HudRuntimeContext.CompassMarker` 对接的现有 S2C marker 字段。

## §4 非重复比对

已查 spirit niche social、directional compass、home sequence 的 finished/active plans 和 skeleton；现有 compass 只消费 TSY/collapse marker，灵龛回家链路没有生产数据源。#1694 独立保留，不把它并入一般 store lifecycle。

## §5 立项检查记录

- `docs/worldview.md`：查灵龛、搜打撤回家与物品整理关键词；不改变灵龛所有权或守护规则。
- `docs/finished_plans/`：查 social spirit niche、directional compass、home sequence；确认 reveal 事件和 HUD marker 之间断链。
- active plan：查 `SpiritNicheReveal`、`HudRuntimeContext`、`BongHud.captureRuntimeContext`、`HomeSequence`，未见接线实现。
- skeleton：查 `compassMarkers`、`SPIRIT_NICHE`、`withCompassMarkers`、`spirit_niche_mark_coordinate`；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：server 灵龛 reveal/coordinate 的 owner、坐标和会话状态；client 玩家位置与 inventory snapshot。
- **Outputs**：`HudRuntimeContext.CompassMarker.Kind.SPIRIT_NICHE`，供 `HomeSequence` 和既有 directional compass 消费；未揭示/失效坐标不显示。
- **共享类型/事件**：复用 `SpiritNicheRevealRequest`/现有 social reveal 语义、`HudRuntimeContext.CompassMarker`、`HomeSequence.State`；当前没有 marker wire 字段，若新增必须同步 Rust schema、proto/JSON bridge 和 client handler。
- **三端契约符号**：server `handle_spirit_niche_coordinate_reveals`/`apply_spirit_niche_reveals`；agent **无变更**，灵龛交互不经 agent IPC；client `BongHud.captureRuntimeContext`、`HudRuntimeContext`、`HomeSequence`。
- **worldview/qi_physics**：回家锚定 `docs/worldview.md` 的灵龛与搜打撤设定；灵龛负灵压费用仍走 server `qi_physics::ledger`，本骨架不重复扣款。

## P0/P1 验收

- 已揭示灵龛 marker 能进入 production runtime context，5 格内 `HomeSequence` 进入 inside 状态。
- 断线/切世界清除旧 marker；未核验坐标不产生回家结算或 NEW badge。
