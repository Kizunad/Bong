# plan-bughunt-era-ambiance-transition-continuity-v1（骨架）

> **来源 issue**：#1788。时代天象连续切换时必须从当前视觉值过渡，而不是从 zone 基线重启。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 让 `previous` 成为真实插值起点，连续 payload 不回跳 zone 基线 | ⬜ |

## §0 摘要

`EraAmbianceState.accept` 把旧 `target` 写进 `previous` 并注明用于插值，但 `interpolateSkyTintRgb` 始终从调用方传入的 zone `baseRgb` 开始，`interpolateFogDensityDelta` 也只按 target 从 0 增长。过渡尚未完成时换 era 会先跳回 zone 基线，再向新目标渐变。

## §1 游玩影响

灾劫、变革或演绎时代快速切换时，天空和雾会闪回基线，产生明显的视觉跳变；不影响 era 服务器状态或音频 payload。

## §2 复现路径

连续接收两个不同 `era_ambiance` payload，第二个到达时第一个尚未完成 transition；观察 `previous` 被写入却不参与插值，第二次帧 0 从 zone baseline 开始。

## §3 今天 `origin/main` 的证据

- `client/src/main/java/com/bong/client/era/EraAmbianceState.java:25,38-44` 存储 `previous` 但只把旧 target 赋值给它。
- `EraAmbianceState.java:64-91` 的 sky/fog 插值分别调用 `blendRgb(baseRgb, target, t)` 与 `target.fogDensityDelta()*t`，全仓没有读取 `previous`。
- `server/src/network/era_ambiance_emit.rs:218-251` 通过 `bong:era_ambiance` 连续推送时代 payload；`EraAmbianceHandler` 负责接收并调用 `EraAmbianceState.accept`。

## §4 非重复比对

已查 era state、era ambiance emit、zone atmosphere 和 audio transition 的 finished/active plans 与 skeleton；已有计划规定 era 参数和断线 reset，没有覆盖连续视觉插值起点。#1788 独立保留。

## §5 立项检查记录

- `docs/worldview.md §一 L9-L30`：查末法时代基调与时代变化表现；不新增时代规则。
- `docs/finished_plans/`：查 era state、zone atmosphere、audio ambiance 计划。
- active plan：查 `EraAmbianceState.previous/target`、`interpolateSkyTintRgb`、`interpolateFogDensityDelta`、`era_ambiance_emit`，未见 previous 接入。
- skeleton：查 `previous`、`baseRgb`、`fogDensityDelta`、`bong:era_ambiance`；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：server `era_ambiance` payload 的 sky tint/fog delta/transition ticks、当前 zone 基线和 client 当前有效视觉值。
- **Outputs**：连续、单调的 client sky/fog 过渡；payload 字段保持兼容。
- **共享类型/事件**：复用 `EraAmbiancePayload`、`EraAmbianceHandler`、`EraAmbianceState` 和 `bong:era_ambiance` channel；不新增 agent 消息。
- **三端契约符号**：server `era_ambiance_emit::era_ambiance_on_era_changed_system`；agent **无变更**，时代通道不经 agent；client `EraAmbianceHandler`/`EraAmbianceState`/zone planner。
- **worldview/qi_physics**：时代基调锚定 `docs/worldview.md §一`；天空/雾表现不改真元流，所有资源结算仍由 server ledger 负责。

## P0 验收

- 第二个 payload 在第 0 tick 从上一次可见值开始，不回跳 zone baseline；transition 完成后精确到 target。
- reset/断线和单个 payload 的既有行为保持不变。
