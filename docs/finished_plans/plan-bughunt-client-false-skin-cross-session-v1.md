# plan-bughunt-client-false-skin-cross-session-v1（骨架）

> **骨架（草案）**。一句话主题：`false_skin_state` 这条 client state 链是**仅 `Changed/Removed` 才发包**的事件驱动 HUD store，但 client 断线时**没有任何生产清理**；因此上一局的蜕壳伪皮层数 / 污染负载会跨 session 残留，并在下一局**无限续命**，直到真的再次收到一条 `false_skin_state`。

> 立项动机：本轮 bughunt 聚焦 client state / runtime store 路径，避开近期已出题的 toast cross-session、dugu v2 HUD disconnect bleed、zone_info stale、ui-state 题。该题命中一条更隐蔽也更高置信的模式：**store 有写入、有渲染、server 只有增量发包，但 client 断线没有 reset，且新 session 默认也不补 baseline empty payload**。

## 核心结论

- **bug**：`FalseSkinHudStateStore` 会在断线 / 切服 / 重连后保留上一 session 的活跃快照；若新 session 玩家没有伪皮且期间没有触发 `FalseSkin` / `StackedFalseSkins` 的 `Changed` 或 `Removed`，旧 HUD 会一直显示。
- **影响 HUD**：`FalseSkinStackHud`（伪皮层数/品质块）+ `ContamLoadHud`（污染负载条）两块都会继续渲染。
- **严重度判断**：`major`
  - 不是纯调试脏数据；会持续误导玩家当前是否仍穿着伪皮、剩余几层、污染承载多少。
  - 不是“一帧自愈”或“1s 后自愈”；在“新 session 没伪皮、也没任何 false_skin_state 新事件”的正常路径下会**无限残留**。

## 复现路径

1. 在 session A 用蜕壳流角色穿上伪皮，触发 `false_skin_state`，让 HUD 上出现“伪皮”层数块和“污 xx%”负载条。
2. 直接断线回主菜单，或切到另一个世界 / 服务器。
3. 进入 session B，使用**没有伪皮**的角色，且这局开始后不触发任何 `FalseSkin` / `StackedFalseSkins` 组件变化。
4. 观察 client HUD：上一局的伪皮层数块和污染条仍继续显示。

### 预期

- 断线后所有 session-bound 的伪皮 HUD 状态立即清空。
- 新 session 在未收到任何伪皮状态前，应呈现 `State.NONE`。

### 实际

- `FalseSkinHudStateStore` 保留旧快照。
- `BongHudOrchestrator` 每帧继续读取该旧快照并喂给 `FalseSkinStackHud` / `ContamLoadHud`。
- 因 server 这条链只在 `Changed/Removed` 时发包，新 session 若无变化，就没有任何 empty payload 来覆盖旧值。

## 证据链 / 根因链路

### 1. client 断线清理表漏掉 `FalseSkinHudStateStore`

- `client/src/main/java/com/bong/client/BongNetworkHandler.java:131-170` 的 `ClientPlayConnectionEvents.DISCONNECT` 已清大量 session-bound store：
  - `RealmCollapseHudStateStore.clearOnDisconnect()`
  - `TiandaoPresenceStore.clear()`
  - `HalfStepRechallengeStore.clear()`
  - `CrackReadingHudStateStore.clear()`
  - `ResonanceLockHudStateStore.clear()`
- **但没有** `FalseSkinHudStateStore`。
- `client/src/main/java/com/bong/client/combat/store/FalseSkinHudStateStore.java:56-71` 只有：
  - `snapshot`
  - `replace`
  - `resetForTests`
- 即：**生产代码无 `clear()/clearOnDisconnect()` 调用点**，只有测试重置入口。

### 2. store 一旦被写入，就会直接驱动 HUD 常驻渲染

- `client/src/main/java/com/bong/client/combat/handler/FalseSkinStateHandler.java:30-39`
  - 收到 `false_skin_state` 后直接 `FalseSkinHudStateStore.replace(nextFalseSkinState)`。
- `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:343-352`
  - 每帧读 `FalseSkinHudStateStore.snapshot()`。
  - 结果直接交给 `FalseSkinStackHud.buildCommands(...)` 和 `ContamLoadHud.buildCommands(...)`。
- `client/src/main/java/com/bong/client/hud/FalseSkinStackHud.java:30-47`
  - 只要 `safeState.active()` 为真，就持续 emit 伪皮 HUD 命令。
- `FalseSkinHudStateStore.State.active()`（`FalseSkinHudStateStore.java:46-48`）
  - 条件只是 `layersRemaining > 0`，**没有 expiry / session generation / connection guard**。

### 3. server 这条链不是 baseline 心跳，而是纯增量 changed/removed 发包

- `server/src/network/false_skin_state_emit.rs:51-88`
  - `emit_false_skin_state_payloads` 只扫 `Changed<FalseSkin>` 和 `RemovedComponents<FalseSkin>`。
- `server/src/network/false_skin_state_emit.rs:91-133`
  - `emit_tuike_v2_false_skin_state_payloads` 只扫 `Changed<StackedFalseSkins>` 和 `RemovedComponents<StackedFalseSkins>`。
- 这意味着：
  - 断线本身不会经过 `RemovedComponents` 给旧 client 发 empty。
  - 新 session 若玩家本来就没伪皮，也不会天然触发 `Changed/Removed`，因此**不会有 baseline empty payload 覆盖旧 store**。

## 根因归纳

这是一个**“增量推送 store 被当成会话态使用，但断线没 reset，server 也不补 baseline”**的典型交叉缺口：

1. `FalseSkinHudStateStore` 是静态全局快照。
2. `FalseSkinStateHandler` 只会写，不会在 session 边界清。
3. `BongNetworkHandler` 的 disconnect 清理表漏了它。
4. server `false_skin_state` 发射器又不是周期性全量同步，而是 `Changed/Removed` 增量。
5. 所以旧值一旦写入，跨 session 后没有任何一边负责把它归零。

## 这个 bug 对实际游玩体验的影响

- 玩家切服、重连、或从上一局退回再进另一局后，HUD 仍会显示“自己还穿着几层伪皮、还背着多少污染”。
- 这会直接误导战斗与资源判断：玩家可能以为自己还有替尸容错、还能继续吃污染，实际新角色/新局根本没有伪皮。
- 因为这块 HUD 不是瞬态 toast，而是会持续挂在 mini-body 附近，体感上更像“角色状态被串档”，会明显破坏对当前局角色状态的信任。

## 影响面

- client：
  - `BongNetworkHandler` disconnect reset 清单
  - `FalseSkinHudStateStore`
  - `FalseSkinStateHandler`
  - `BongHudOrchestrator`
  - `FalseSkinStackHud`
  - `ContamLoadHud`
- server：
  - `false_skin_state_emit.rs` 的 changed/removed-only 发包模型
- 覆盖范围：
  - `tuike` 旧 `FalseSkin`
  - `tuike_v2` 的 `StackedFalseSkins`

## 修复建议

### 推荐修法（最小正确修）

- 在 `client` 断线清理总表中显式清空 `FalseSkinHudStateStore`。
- 最好补一个正式生产 API：`FalseSkinHudStateStore.clear()` 或 `clearOnDisconnect()`，不要复用 `resetForTests()`。

### 加固修法（防再犯）

- 对所有“server 仅增量推送、client 用静态 store 持有”的 HUD/store 做一次 session-bound 审计。
- 给这类 store 建统一约束：
  - 要么 disconnect 必清。
  - 要么 JOIN 后 server 必发 baseline empty/full snapshot。
  - 二者至少满足其一。

## 反方裁决

> 退化说明：本会话没有可继续调用的 subagent 能力，未能再开独立子代理做外部对抗审查。以下两轮反方裁决由主代理按“先反方、后驳回”的方式手工完成，并把论点与驳回理由完整记录。

### 第一轮反方

- **反方论点**：`false_skin_state_emit.rs` 已经处理 `RemovedComponents<FalseSkin>` / `RemovedComponents<StackedFalseSkins>`，所以玩家没伪皮时最终会收到 empty payload，问题不成立。
- **驳回理由**：
  - 这只覆盖“同一 session 内组件被卸下/移除”的路径。
  - 断线切 session 时，旧 client 不会等到新 session 的 `RemovedComponents`。
  - 更关键的是，新 session 若角色天生就没有伪皮，server 侧根本**没有 removed 事件可发**；而 emit 系统又不做 baseline 全量广播，所以旧值不会被覆盖。

### 第二轮反方

- **反方论点**：就算 `FalseSkinHudStateStore` 没清，`DerivedAttrsStore` 在 disconnect 已 reset，相关 HUD 可能会一起消失，影响有限。
- **驳回理由**：
  - `BongHudOrchestrator` 对伪皮 HUD 的数据源不是 `DerivedAttrsStore`，而是直接读 `FalseSkinHudStateStore.snapshot()`。
  - `FalseSkinStackHud` / `ContamLoadHud` 的显示条件只看 `safeState.active()`，也就是 `layersRemaining > 0`。
  - 因此 `DerivedAttrsStore` 已清并不能阻止这两块 HUD 继续渲染，残留是独立成立的。

## 建议落地方式

- 走 `fix_pr` 比较合适，改动应很局部：
  - client disconnect reset 补 `FalseSkinHudStateStore.clear()/clearOnDisconnect()`
  - 回归测试覆盖“先写 active，再模拟 disconnect，HUD store 归零”
- 若维护者想顺手补架构债，可另起一个小 plan 把同类增量 store 的 session reset 规则统一。

## 审计来源

- bughunt 目标范围：client state / runtime store
- 排除项：toast cross-session、dugu v2 HUD disconnect bleed、zone_info stale、ui-state 近期题
- 结论性质：**report-only**

## Finish Evidence

**验证结论**：skeleton 描述为**真 bug**，已实地读代码核实：

1. **断线不清**：修复前 `client/src/main/java/com/bong/client/combat/store/FalseSkinHudStateStore.java` 只有 `snapshot()` / `replace()` / `resetForTests()`（测试专用），没有任何生产态清理入口；`client/src/main/java/com/bong/client/BongNetworkHandler.java` 的 `clearClientStateOnDisconnect()`（挂在 `ClientPlayConnectionEvents.DISCONNECT`）清理清单里完全没有它。
2. **server 只做增量推送**：`server/src/network/false_skin_state_emit.rs` 的 `emit_false_skin_state_payloads` / `emit_tuike_v2_false_skin_state_payloads` 只扫 `Changed<FalseSkin>` / `RemovedComponents<FalseSkin>`（及 `StackedFalseSkins` 等价物），`server/src/network/mod.rs` 未给这条链注册任何 join-time / 周期性 baseline 全量重发（对比 `craft_emit::emit_recipe_list_on_join` 等其他确实有 join baseline 的链路）。断线切 session 时旧 client 不会等到新 session 的 removed 事件；新 session 若角色本身没有伪皮，也压根没有 removed 事件可发。
3. **HUD 无其他兜底**：`BongHudOrchestrator.java` 每帧直接读 `FalseSkinHudStateStore.snapshot()`，喂给 `FalseSkinStackHud` / `ContamLoadHud`，两者仅以 `state.active()`（`layersRemaining > 0`）为渲染门槛，无 session-id / connection-state 等其他守卫。`FalseSkinStateHandler` 收包后无条件 `replace(...)`，同样无 session 校验。

**落地清单**：
- `client/src/main/java/com/bong/client/combat/store/FalseSkinHudStateStore.java` — 新增生产态 `clearOnDisconnect()`，把静态 `snapshot` 复位为 `State.NONE`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java` — `clearClientStateOnDisconnect()` 接入 `FalseSkinHudStateStore.clearOnDisconnect()`。

**关键 commit**：
- `84b5ad62`（2026-07-11）骨架转正：plan-bughunt-client-false-skin-cross-session-v1
- `f21d7bc8`（2026-07-11）修复 FalseSkinHudStateStore 断线不清导致伪皮 HUD 跨 session 残留

**测试结果**：
- `client/src/test/java/com/bong/client/BongNetworkHandlerTest.java` 按既有三段式追加 2 用例（`disconnectClearsFalseSkinHudStateStoreToPreventCrossSessionResidualHud` / `disconnectClearingFalseSkinHudStateStoreDoesNotBlockNewSessionSnapshotAfterReconnect`），`@AfterEach` 补 `FalseSkinHudStateStore.resetForTests()`。
- `cd client && ./gradlew test build` 全绿（`BUILD SUCCESSFUL`，13 actionable tasks，`BongNetworkHandlerTest` 13/13 通过含新增 2 例）。
- 对抗验证：无上下文 read-only validator（Explore agent）对 HEAD `f21d7bc8583043cd5814a6828d944914c3ade3d0` 独立复核 `FalseSkinHudStateStore`/`BongNetworkHandler`/`BongHudOrchestrator`/`false_skin_state_emit.rs`/新测试 + 单独跑 `./gradlew test --tests "com.bong.client.BongNetworkHandlerTest"`，结论 `VERDICT: PASS`，无遗留 concern。

**跨仓库核验**：本修复为 client-only（static store + disconnect 事件清理），未改 server 侧 `false_skin_state` payload 契约；server 端 `false_skin_state_emit.rs` 的增量发包模型保持不变（本修复不需要 server 补 baseline，client 断线清理已足够消除残留）。

**遗留 / 后续**（超出本 plan 范围）：
- skeleton 附带的「加固修法」——对所有「server 仅增量推送、client 用静态 store 持有」的 HUD/store 做统一 session-bound 审计——未在本次范围内处理，留给后续同类 bughunt 题目按需覆盖。
