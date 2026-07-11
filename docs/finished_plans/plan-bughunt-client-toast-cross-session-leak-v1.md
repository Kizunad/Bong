# plan-bughunt-client-toast-cross-session-leak-v1

> 一句话主题：client HUD 的单槽 `BongToast.activeToast` 在断线/切服/重连时从未清场，导致上一局残留的 warning / era / event / inventory toast 会在下一次进世界后的前几秒继续显示，形成 **跨 session 的 stale toast 泄漏**。

> 立项动机：当前 `BongNetworkHandler` 的 DISCONNECT 清单已经显式收过多类“旧状态跨 session 续命”问题（`realm_collapse`、`TiandaoPresenceStore` 等），但 toast 单例没有纳入同一清场路径。该缺口属于 client HUD / toast / runtime state bridge 的活路径：不需要服务器出错，只要玩家在 toast 未过期前断线再重连，就会把上一局提示串进下一局。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `BongToast` 断线未清导致跨 session 串 toast | fix_pr | ✅ 2026-07-11 |

## P0 — `BongToast` 断线未清导致跨 session 串 toast

- **现象**：`BongToast` 用 `static volatile BongToast activeToast` 保存当前 toast，`show(...)` 只负责覆盖写入，`current(now)` 只会在**自然过期**后把它清空；除测试专用的 `resetForTests()` 外，没有任何生产态“主动清零”入口（`client/src/main/java/com/bong/client/hud/BongToast.java:19-20,65-83,156-157`）。
- **复现路径**：
  1. 在任意 server 内触发一个 3-8 秒的 client toast。活链很多，例如 `event_alert` 会在 `applyDispatch()` 里走 `dispatch.alertToast().ifPresent(alertToast -> BongToast.show(...))`，narration toast 也会走 `dispatch.toastNarrationState().ifPresent(...)`（`client/src/main/java/com/bong/client/BongNetworkHandler.java:818-829`）。
  2. 在 toast 过期前立刻断线、切服，或从一个 server 直接连到另一个 server。
  3. 重新进世界后的首批 HUD 帧，`ToastHudRenderer.append()` 仍会从 `BongToast.buildCommand()` 读到旧 toast，并通过 `BongHudOrchestrator` 正常渲染出来（`client/src/main/java/com/bong/client/hud/ToastHudRenderer.java:11-24`，`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:151-153`）。
- **根因链路**：
  1. 生产态所有 toast 最终都落到 `BongToast.activeToast` 这个全局单槽（`BongToast.show(...)`，`client/src/main/java/com/bong/client/hud/BongToast.java:65-76`）。
  2. `BongNetworkHandler` 的 DISCONNECT 回调手工清了几十个 store，还专门注释了“避免上一 server 的状态跨 session 续命”，但清单里没有 `BongToast`（`client/src/main/java/com/bong/client/BongNetworkHandler.java:128-170`）。
  3. 因为 `activeToast` 没被清，下一次 JOIN 之前/之后它都保持旧值；只要 `expiresAtMillis` 尚未到达，`current(now)` 就会把它当作合法活跃 toast 返回，而不是视为脏状态（`client/src/main/java/com/bong/client/hud/BongToast.java:79-85`）。
  4. `BongHudOrchestratorTest.activeToastSurvivesLaterNonToastNarrationUntilExpiry()` 还把“旧 toast 在没有新 toast 覆盖时会持续存活到自然过期”钉成了既有契约（`client/src/test/java/com/bong/client/hud/BongHudOrchestratorTest.java:178-194`）。这个契约在单 session 内没问题，但一旦缺少 disconnect reset，就把“同一局内的持续显示”错误外溢成了“跨局污染”。
- **为什么这是 bug，不是设计**：
  - 本仓已经把“断线后旧 HUD/runtime 状态继续渲染”视为明确 bug，并在同一 DISCONNECT 块里多次补洞：`realm_collapse` 注释直接写了“避免上一 server 的倒计时跨 session 续命”，`TiandaoPresenceStore` 注释也写了“漏掉会导致断线重连后旧 presence 继续渲染/播放”（`client/src/main/java/com/bong/client/BongNetworkHandler.java:128-130,157-160`）。
  - toast 是同类 runtime HUD 状态，只是恰好还没被纳入清场清单；没有任何地方表明“上一局的提示应该故意带入下一局”。
- **这个 bug 对实际游玩体验的影响**：
  - 玩家会在新一局/新 server 的开场几秒看到**与当前场景无关**的旧提示，例如上一局的“天道警示”“盾已碎裂”“交易邀请已过期”“该手已持械，请先卸下再更换”等。这类 stale toast 位置居中、优先级高，极容易被误读成当前局刚发生的新事件。
  - 对刚进世界的判断影响尤其直接：玩家可能把上一局的危险警示当成当前 server 的实时威胁，或把上一局的操作拒绝文案误以为当前背包/装备状态异常，造成错误决策和短暂恐慌。
- **建议修复范围 / 模块**：
  - 主修 `client/src/main/java/com/bong/client/hud/BongToast.java` 与 `client/src/main/java/com/bong/client/BongNetworkHandler.java`。
  - 方向建议：给 `BongToast` 增加生产态 `clearOnDisconnect()` / `clear()`（不要复用 `resetForTests()` 这种测试语义入口），并在现有 DISCONNECT 清场块里显式调用；同时补一条“活跃 toast -> disconnect -> reconnect 前不应残留”的回归测试。
  - 可选连带复核：`BongHudStateStore` 同样未进 DISCONNECT 清单，但它牵涉 `zoneState` / `visualEffectState` / `narrationState` 的更大面，建议在 fix PR 里单独裁定是否同批处理，避免把本题和已知 `zone_info stale` 线索混卷。
- **验收抓手**：
  1. 先造一个未过期 toast，执行 disconnect/reset 路径后，`BongToast.current(now)` 必须返回 empty。
  2. reconnect 后在收到任何新 toast 之前，HUD 首帧不应渲染旧 toast。
  3. 现有“同一 session 内旧 toast 会存活到自然过期”的契约不能被误伤；也就是说，只清断线边界，不改日常 show/current 语义。

## 反方裁决摘要

- **退化处理说明**：当前会话没有可用 subagent / delegate 能力，本轮反方裁决退化为主代理手工做两轮默认怀疑复核；以下结论均基于现读代码与既有测试交叉验证。
- **Round 1 反方论点**：“这也许不是 bug，toast 反正几秒后会自然消失，最多是短暂视觉残留。”
  - **驳回理由**：问题不在“是否最终会消失”，而在“是否跨局泄漏”。本仓已经把跨 session 续命视为明确错误并逐项在 DISCONNECT 清场；toast 没有理由成为唯一例外。并且 3-8 秒对刚进世界的首屏提示已经足够长，玩家能明确看到并误判。
- **Round 2 反方论点**：“JOIN / reconnect 也许会立刻发新 toast 覆盖旧 toast，所以玩家未必看得到。”
  - **驳回理由**：JOIN 路径只 `markConnected(...)`，并没有任何 `BongToast.clear/reset`；`applyDispatch()` 也只有在后续真收到新 toast payload 时才会覆盖写入（`client/src/main/java/com/bong/client/BongNetworkHandler.java:172-175,818-829`）。没有新 toast 的首批帧一定会读到旧值；即便有新的连接状态 toast 覆盖，那也只是“用另一条无关 toast 掩盖旧脏数据”，不是状态正确。

## 开放问题

1. fix PR 是否只收 `BongToast`，还是顺手把 `BongHudStateStore` 也纳入同一 disconnect reset 族？建议先拆清，避免和既有 `zone_info stale` 线索重叠。
2. disconnect 清场是否值得统一抽成一个“HUD runtime state reset”入口，避免未来再漏新的 static singleton。

## 审计来源

bughunt 定点轮（范围只看 client HUD / toast / overlay / runtime state bridge，避开已点名的 `zone_info stale`、`locust warning duration drift`、`silent signal runtime bridge`、`movement dash HUD`、`tool weapon HUD leak`）。结论为 **report-only**：本次只新增 skeleton，不改源码。

## Finish Evidence

### 验证结论

**第一性原理复核确认：真 bug**，skeleton 结论成立。BugFix subagent 独立读取 `BongToast.java` / `BongNetworkHandler.java` 复核：

- `BongToast.activeToast` 确为 `private static volatile` 单槽（`client/src/main/java/com/bong/client/hud/BongToast.java:19`）；`show(...)` 只覆盖写（65-77），`current(now)` 只在 `expiresAtMillis` 过期后清空（79-86），除测试专用 `resetForTests()`（原 156-158）外无生产态清零入口。
- `BongNetworkHandler.clearClientStateOnDisconnect()`（原 857-901）确认挂在 `ClientPlayConnectionEvents.DISCONNECT`（132-133）上，清单里覆盖了 `RealmCollapseHudStateStore`、`TiandaoPresenceStore`、`CraftStore` 等十余个 static store，但**唯独没有 `BongToast`**，与同函数内其余注释一致确认"跨 session 续命"是本仓已知反模式。
- 开放问题 #1（是否顺带收 `BongHudStateStore`）核实：`BongHudStateStore` 的断线清理属于**另一个已在跑的独立 skeleton** `plan-bughunt-hud-state-session-reset`（远端分支 `origin/bugfix/plan-bughunt-hud-state-session-reset`，commit `4236b32b`），与本 plan 不重叠，本 PR **不touch** `BongHudStateStore.java`，避免跨 PR 撞车。

### 落地清单

- `client/src/main/java/com/bong/client/hud/BongToast.java`：新增生产态 `clearOnDisconnect()`（不复用 `resetForTests()`），把 `activeToast` 复位为 `empty()`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java`：`clearClientStateOnDisconnect()` 尾部补上 `BongToast.clearOnDisconnect()` 调用 + 说明注释。
- `client/src/test/java/com/bong/client/hud/BongToastTest.java`：新增 4 条用例——`clearOnDisconnectRemovesActiveUnexpiredToast` / `clearOnDisconnectIsIdempotentOnAlreadyEmptyState` / `clearOnDisconnectDoesNotBlockSubsequentShowAfterReconnect` / `clearOnDisconnectDoesNotAffectSameSessionNaturalExpiryContract`（回归既有同 session 存活到自然过期契约不被误伤）。
- `client/src/test/java/com/bong/client/BongNetworkHandlerTest.java`：新增 2 条集成用例——`disconnectClearsBongToastToPreventCrossSessionLeak` / `disconnectClearingBongToastDoesNotBlockNewToastAfterReconnect`；`@AfterEach` 补 `BongToast.resetForTests()` 防跨测试污染。

### 关键 commit

- `c9b043f7`（2026-07-11）：docs(plan): plan-bughunt-client-toast-cross-session-leak-v1 骨架升 active
- `75c644f034519c1fd5a0050766f8fa22b348cee8`（2026-07-11）：修复：断线清理清单补上 BongToast，防跨 session 串旧 toast

### 测试结果

- `cd client && ./gradlew test --tests "com.bong.client.hud.BongToastTest" --tests "com.bong.client.BongNetworkHandlerTest"` → `BUILD SUCCESSFUL`（BongToastTest 10/10、BongNetworkHandlerTest 6/6，含新增 6 条用例全绿）。
- `cd client && ./gradlew test build` → `BUILD SUCCESSFUL`（全量 client 单测 + `check` + `assemble` 全绿）。

### 跨仓库核验

- **client**：`BongToast.clearOnDisconnect()` ↔ `BongNetworkHandler.clearClientStateOnDisconnect()` 单点接线，命中 symbol：`BongToast`、`BongNetworkHandler`、`ClientPlayConnectionEvents.DISCONNECT`。
- **server / agent**：本 bug 纯 client HUD runtime state，不涉及 server↔agent↔client IPC 契约，无跨仓库 symbol 改动。

### 对抗验证

无上下文 read-only validator（`Explore` agent）对 HEAD `75c644f034519c1fd5a0050766f8fa22b348cee8` 独立复核：确认 disconnect 挂钩真实存在、修复正确关闭 gap、既有同 session 契约未被误伤、新测试非 tautological（实测跑绿）、无 `BongHudStateStore` 越界改动、导入无编译错误 —— **结论 PASS**。

### 遗留 / 后续

- 开放问题 #2（"HUD runtime state reset" 统一入口）本 plan 不处理，留给未来若再漏 static singleton 时按需立新 plan/骨架讨论；`clearClientStateOnDisconnect()` 目前逐项显式调用的模式已被本仓库多个先例（`plan-craft-session-reconnect-lock-v1`、F19、F9 等）验证足够可维护，不阻塞本次归档。
- `BongHudStateStore` 断线清理由独立在跑的 `plan-bughunt-hud-state-session-reset` 负责，不在本 plan 范围。
