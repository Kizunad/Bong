# plan-tsy-discovery-ui-target-fallback-v1（骨架）

> **骨架（草案）**。一句话主题：`agent/packages/tiandao/src/runtime.ts` 的 `processTsyZoneActivatedForUi()` 在 `tsy_zone_activated.player_id` 未命中当前 `world_state.players` 时，会把本应发给“首次踏入该 TSY 的触发玩家”的 `tsy_discovery` 面板，错误 fallback 到 `state.players[0]`。由于 server 端只认 `target_player` 是否在线，不校验“这个人是不是该 TSY 的触发者”，结果是**无关在线玩家会收到并接管别人的秘境发现面板，原触发者反而收不到，且被误投递者现有面板还可能被静默替换**。

> 立项动机：这不是单纯的 UI 小瑕疵，而是 `schema -> tiandao runtime -> agent_ui session -> 后续 button_click 推演` 整条消费链的选人错误。`TsyZoneActivatedV1.player_id` 与 server 注释都已把语义锁成“触发 first-enter 的 canonical_player_id”，agent 侧仍保留“找不到就发给第一个在线玩家”的降级分支，并且测试把该错误行为固化成绿灯。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | TSY 发现面板 target fallback 错发 / 顶掉他人 session / 意图注入错人 | fix_pr | ⬜ |

## P0 — TSY 发现面板 target fallback 错发 / 顶掉他人 session / 意图注入错人

- **现象**：`agent/packages/tiandao/src/runtime.ts:1046-1050` 明确先按 `event.player_id` 找触发玩家；若未命中，则直接 fallback 到 `state.players[0]`。随后 `triggerUi({ scenario:"tsy_discovery", targetPlayer })` 会把 `targetPlayer.uuid` 写入 `AgentUiRequestCommandV1.target_player`，server 端再按这个 id 精确路由给 client。
- **复现路径**：
  1. A 玩家触发某个 `tsy_zone_activated`，server bridge 按契约写出 `player_id=A`。
  2. 到 agent 本轮 fresh-state 处理时，`state.players` 中已经没有 A（例如 A 刚掉线 / world_state 尚未包含 A / 同 tick 顺序差导致 agent 看到的是另一批在线玩家）。
  3. `processTsyZoneActivatedForUi()` 命不中 `event.player_id` 后选中 `state.players[0]`，把 `tsy_discovery` 面板发给 B。
  4. `agent/packages/tiandao/src/ui/xmlTemplates.ts:90-101` 的 TSY 面板含 `enter_realm` / `observe_only` / `dismiss` 三个按钮；B 的点击又会经 `AgentUiRuntime.drainPendingButtonClicks()` 注入后续 LLM 推演上下文（`agent/packages/tiandao/src/runtime.ts:1296-1300`、`agent/packages/tiandao/tests/button-click-context.test.ts` 全链路锁定）。
- **根因链路**：
  1. `agent/packages/schema/src/tsy.ts:114-136` 已把 `TsyZoneActivatedV1.player_id` 定义为“触发 first-enter 的玩家 canonical_player_id”，并写明 agent 应直接用它选人。
  2. `server/src/world/tsy_lifecycle.rs:222-227` 再次把语义钉死为“写入 `player_id` 字段，让 agent 直接拿到‘该发 TSY 面板给谁’”。
  3. 但 tiandao runtime 在 `agent/packages/tiandao/src/runtime.ts:1048-1050` 仍保留 `?? state.players[0]` 兜底，把“找不到触发者”错误地降级成“找任意在线玩家”。
  4. `server/src/network/agent_ui.rs:364-387` 只检查 `target_player` 是否对应在线 canonical 玩家；一旦 B 在线，这条误投递不会被拦截。
  5. `server/src/network/agent_ui.rs:433-490` 还是单玩家单 session 语义：如果 B 此时已有别的天道面板，新的错发 TSY 面板会把旧 session 标记为 `Replaced` 并下发 close。
  6. 更糟的是，`agent/packages/tiandao/tests/runtime.test.ts:2319-2336` 当前把“player_id 找不到时 fallback 到第一个在线玩家”写成通过用例，说明这不是偶发漏判，而是被测试固定下来的错误行为。
- **这个 bug 对实际游玩体验的影响**：
  - A 玩家明明是首次踏入活坍缩渊的人，却可能完全收不到“踏入探寻 / 神识探查 / 离开”的发现面板。
  - B 玩家即使与该 TSY 无关，也会突然收到不属于自己的秘境发现 UI；若他点击 `enter_realm` / `observe_only`，这些意图会被当成真实玩家输入，继续喂给 tiandao 推演。
  - 若 B 当时正看别的天道面板，server 会把旧 session 直接 `Replaced`，体感就是“自己的面板被陌生 TSY 提示顶掉了”。
  - 多人在线时，这会把“谁发现了秘境、谁应当做决定”的 ownership 搅乱，属于真实可感的错人提示与错人交互，而不是纯日志级问题。
- **影响面**：
  - `agent/packages/tiandao/src/runtime.ts`：TSY 发现 UI 生产路径。
  - `agent/packages/tiandao/src/ui/*`：`tsy_discovery` 面板按钮与 `button_click` 注入链。
  - `server/src/network/agent_ui.rs`：错误 target 一旦在线就会落成真实 session，并可能替换其现有 session。
  - 测试面：`agent/packages/tiandao/tests/runtime.test.ts:2319-2336` 当前在保护错误语义。
- **建议修复范围 / 模块**：
  - 优先收口 `agent/packages/tiandao/src/runtime.ts` 的 `processTsyZoneActivatedForUi()`：`player_id` 未命中时应记录并跳过，而不是改投他人。
  - 同步改 `agent/packages/tiandao/tests/runtime.test.ts`：把现有 fallback 正例改成“miss 时不触发 UI、只记 warn”的负例 pin。
  - 若担心 world_state / event 时序短窗，可单独评估“缓存最近在线玩家索引”或“重试一轮”类方案，但**不能**再使用“任意在线玩家接盘”作为降级语义。
- **验收抓手**：
  1. `player_id` 命中时，target 仍精确等于触发玩家 canonical id。
  2. `player_id` 未命中时，不发送 `AgentUiRequestCommandV1`，且日志明确记 miss 原因。
  3. 已在线的无关玩家不会因别人的 `tsy_zone_activated` 被创建 / 替换 session。
  4. `enter_realm` / `observe_only` button_click 只会来自真正的触发者面板，不再出现“错人点击进入 LLM 上下文”。

## 反方裁决摘要

1. **Round 1（退化处理：本会话无可用 subagent 工具，改由主代理手工反方裁决）**
   反方论点：fallback 是刻意的产品决策，目的是“至少保证有人看到提示”，不算 bug。
   驳回理由：`TsyZoneActivatedV1.player_id` 与 `server/src/world/tsy_lifecycle.rs:222-227` 都把语义写成“该发给谁”，不是“任意在线玩家都可代收”。这条链路讨论的是 ownership，不是送达率；把别人的 discover prompt 投给第一个在线玩家，本质上是在伪造触发者。

2. **Round 2（退化处理：继续由主代理做代码级反方裁决）**
   反方论点：即便发错，server 也会靠 `target_player` / realm gate / `allowed_button_ids` 把误投递挡住，最多只是 harmless 提示。
   驳回理由：`server/src/network/agent_ui.rs:364-387` 只校验该 `target_player` 是否在线；只要 B 在线，误投递就会落成真实 session。`server/src/network/agent_ui.rs:442-490` 还会替换 B 的旧 session。再加上 `agent/packages/tiandao/src/ui/xmlTemplates.ts:90-101` 与 button_click 注入测试，说明误投递后的点击会继续影响后续推演，不是 harmless no-op。

## 开放问题

1. 若修复后选择“miss 即跳过”，是否需要额外 telemetry 统计 `player_id` miss 频率，以便判断是偶发顺序窗还是更深的 world_state/Redis 时序问题？
2. `processTsyZoneActivatedForUi()` 当前注释仍写着“若秘境内无人则选第一个玩家”，修复时应同步清理注释与相关测试，避免以后再次回归。

## 审计来源

bughunt 定点轮（范围仅 `agent runtime / tiandao / schema` 消费链，避开 locust warning duration contract drift、insight offer context clobber 一类已知题）。证据来自 `agent/packages/schema/src/tsy.ts`、`agent/packages/tiandao/src/runtime.ts`、`agent/packages/tiandao/tests/runtime.test.ts`、`server/src/world/tsy_lifecycle.rs`、`server/src/network/agent_ui.rs` 的闭环人工复核。附带说明：本地尝试运行 `npm test -w @bong/tiandao -- --run tests/runtime.test.ts -t "falls back to first online player when player_id not found in state"` 时，当前 worktree 因缺本地 TS 工具链在 `tsc: not found` 处停止，故本轮结论以静态证据为主、未附执行日志。
