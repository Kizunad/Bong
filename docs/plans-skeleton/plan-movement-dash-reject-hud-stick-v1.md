# plan-movement-dash-reject-hud-stick-v1（骨架）

> **骨架（草案）**。一句话主题：`movement` 主链把 `rejected_action` 当成持久状态挂在 `MovementState` 上，`emit_movement_state_payloads` 又会在后续每次 `Changed<Stamina>` 时原样重发；client `MovementStateStore.replace()` 每次看到非空 `rejected_action` 都把 `rejectedAtMs` 重置成“刚刚被拒绝”。结果是：**玩家在 dash 冷却中或体力不足时多按一次 V，本应只闪红 0.3s 的拒绝提示，会在整个体力恢复窗口内反复续命，Movement HUD 也被持续钉在屏幕上**。

> 立项动机：这是 `server/src/movement/` → `client/.../movement/` → `client/.../hud/` 的直接断链，玩家正常高频操作就能稳定触发；影响不是“测试里才看得见的字段脏”，而是 **冲刺失败提示常驻、干扰战斗读屏**。问题位于近期 movement overhual 主路径，局部明确，适合先立 skeleton-only PR 收口，再出 fix PR。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | dash 拒绝提示被 `rejected_action` 粘成持续闪红 / HUD 常驻 | fix_pr | ⬜ |

## P0 — dash 拒绝提示被 `rejected_action` 粘成持续闪红 / HUD 常驻

- **现象**：`server/src/movement/mod.rs:364-370` 一旦 reject dash，就把 `movement.rejected_action = Some(Dash)` 写进 `MovementState`；但同文件仅在**下一次成功动作**时才于 `:373` 清空，中间没有“发完一次拒绝提示就复位”的路径。`tick_movement_actions` / 其他 movement tick 也不会清它。
- **为什么会持续重发**：`emit_movement_state_payloads` 的 query 过滤是 `Added<MovementState> || Changed<MovementState> || Changed<Stamina>`（`server/src/movement/mod.rs:475-486`），而 payload 序列化 `MovementState::to_payload()` 又直接把 `self.rejected_action` 原样塞进 `MovementStateV1`（`:511-528`）。与此同时，`combat::lifecycle::stamina_tick` 每 `STAMINA_TICK_INTERVAL_TICKS = 4` tick 更新一次 `stamina.current`（`server/src/combat/components.rs:20`，`server/src/combat/lifecycle.rs:279-306`），也就是 **体力恢复期会每 0.2s 触发一次新的 movement_state 下发**，且都带着同一个陈旧 `rejected_action`。
- **client 侧为什么会把陈旧字段当成“新拒绝”**：`client/src/main/java/com/bong/client/movement/MovementStateStore.java:21-34` 只要看到 `normalized.rejectedAction()` 非空，就无条件把 `hudActivityAtMs` 和 `rejectedAtMs` 刷成 `nowMs`；它不区分这是“刚发生的新 reject”还是“server 又把旧 reject 重发了一次”。`MovementHudPlanner` 再用 `REJECT_FLASH_MS = 300`（`client/.../hud/MovementHudPlanner.java:10-13`）和 `state.rejectedRecently()`（`:65-74`）决定是否画红色 reject flash，因此 **0.2s 一次的重复包会让本应 0.3s 结束的闪红几乎一直续杯**。
- **为什么这是 bug，不是设计**：`docs/finished_plans/plan-movement-v1.md:104-107` 明确把 reject 反馈定义为“`stamina` 不足时按冲刺 → 弧线闪红 0.3s 提示”；client 实现也把这个窗口硬编码成 300ms。当前行为却变成“只要体力还在变，旧 reject 就反复被当作新 reject”，明显违背 one-shot 提示语义。
- **对实际游玩体验的影响**：玩家最常见的操作是 dash 后马上再按一次 V 试探冷却，或在低体力时连按 V 逃命。现在这会导致 **红色 reject 面板几乎整段恢复期都在闪 / 刷新，dash HUD 的 3s 悬停计时也被不断重置**。体感上像是“冲刺一直出错”而不是“一次失败提示”，会遮掉其它近战/受击 HUD 反馈，尤其在连续走位、被追击、濒死拉扯时最明显。
- **建议修复范围 / 模块**：优先收口 `server/src/movement/mod.rs` 与 `client/src/main/java/com/bong/client/movement/MovementStateStore.java`。推荐把 `rejected_action` 明确定义成 **edge-triggered one-shot 字段**：要么 server 在首次 emit 后立刻清空，要么改成带 sequence/timestamp 的瞬时拒绝事件；如果保留现有 payload 形状，client 至少也要去重“相同 reject 且无新动作/无新 reject 序列”的重复包，不能每次都刷新 `rejectedAtMs`。
- **验收抓手**：至少补 3 组 pin。1) 服务器侧：一次 reject 后，后续仅 `Changed<Stamina>` 的 movement_state payload 不应继续携带旧 `rejected_action`，或必须附带可去重序列。2) client 侧：同一个 reject payload 被重复喂入时，`rejectedAtMs` 不能每次刷新。3) 端到端：成功 dash 一次后立刻在冷却内再按 V，红色 reject flash 应在约 300ms 内结束，HUD 应在约 3.5s 内恢复 auto-hide，而不是跟着体力恢复期一起常驻。

## 反方裁决摘要

1. **Round 1 反方怀疑**：也许 `rejected_action` 虽然留在 `MovementState` 里，但后续没有新的 movement_state payload，因此 client 实际只会看到一次红 flash。
   **裁决**：不成立。`emit_movement_state_payloads` 明确把 `Changed<Stamina>` 也算进发送条件（`server/src/movement/mod.rs:475-486`），而 `stamina_tick` 每 4 tick 会更新 `stamina.current`（`server/src/combat/lifecycle.rs:279-306`），恢复窗口天然会持续重发 payload。
2. **Round 2 反方怀疑**：也许 client 已经对重复 reject 做了去重，或 `rejectedRecently(300ms)` 足以吞掉重复包。
   **裁决**：不成立。`MovementStateStore.replace()` 只要看到非空 `rejectedAction` 就直接把 `rejectedAtMs = nowMs`（`client/.../MovementStateStore.java:21-34`），没有比较“是否同一 reject”；而服务器重发间隔 0.2s 小于 `REJECT_FLASH_MS = 300ms`（`MovementHudPlanner.java:10-13,65-74`），因此重复包会持续把闪红窗口往后推。

## 审计来源

bug-hunt 定点轮（收窄 `server/src/movement/`、`server/src/player/`、`client/.../movement`、`client/.../hud`、`client/.../input` 主路径，外加直接相邻的 `combat::lifecycle`）。候选经主代理实地代码复核后保留。当前结论是 **report-only**：先提交 skeleton plan，把可达链路、玩家影响、修复面与验收抓手讲清，再由后续 fix PR 单独落地。
