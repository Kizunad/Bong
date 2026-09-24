# plan-bughunt-client-identity-panel-stale-session-v1（骨架）

> **骨架（草案）**。一句话主题：client identity panel / HUD 走静态 `IdentityPanelStateStore`，但断线不清、面板也不订阅后续刷新，导致**重连后短窗口可展示上个会话身份数据，且一旦在旧快照上开面板，后续新快照只改文字不改按钮回调**，形成可操作的错 UI。

> 立项动机：本轮聚焦 client overlay / screen / surface sidepaths，避开 toast cross-session、visual tide sky、dash HUD、tool/weapon HUD leak 等已出题。该问题命中 HUD + Screen 双路径，且是高置信 code-level 真 bug，不依赖推测性时序。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | identity 面板跨 session stale snapshot + 按钮回调冻结 | fix_pr | ✅ 2026-07-11 |

## P0 — identity 面板跨 session stale snapshot + 按钮回调冻结

- **问题定义（fix_pr）**：`client/src/main/java/com/bong/client/identity/IdentityPanelStateStore.java:19-20,54-57` 把 `IdentityPanelState` 放在 static store 里，生产代码只有 `replace` / listener API，没有任何 disconnect 清理；全仓 `rg "IdentityPanelStateStore\\.addListener|IdentityPanelStateStore\\.removeListener"` 为 0 命中，说明 Screen 也没订阅它。与此同时，`IdentityPanelScreenBootstrap.java:21-24` 只注册了 tick hotkey，**没有** `ClientPlayConnectionEvents.DISCONNECT`；总断线兜底 `BongNetworkHandler.java:131-170` 清了十多个 store，也**没清** identity store。结果是：断线/切服后，上一会话的 identity snapshot 会继续留在 client 内存。

- **复现路径**：
  1. 在角色 A 所在 server / 世界中，确保 `identity_panel_state` 已下发，右下角 HUD 能看到 `[#id] 名字`（`IdentityHudCornerLabel.java:34-52` 直接读 store）。
  2. 断线并立刻重连到另一个角色 B，或同角色但 identity 数据已不同的 server。
  3. 在新会话 fresh `identity_panel_state` 到达前，HUD 仍会显示上一会话 identity；此时立刻按 `O` 打开身份面板。
  4. 面板 `init()` 在 `IdentityPanelScreen.java:25-58` 用**旧快照**创建按钮、决定 `active` 状态，并把 `entry.identityId()` 捕获进 `switchIdentityCommand(...)` 回调。
  5. 随后新会话 payload 到达时，`render()` 只在 `IdentityPanelScreen.java:62-80` 重新读取 store 并重画文字；**旧按钮不会重建，也没有 listener 驱动 refresh**。
  6. 结果是：屏上文字可变成新会话 identity 列表，但按钮文案、可点击态、以及点击后发送的 `identity switch <id>` 仍绑定旧会话快照。

- **根因链路**：
  1. `IdentityPanelStateStore` 设计上是 volatile snapshot + listener store（`IdentityPanelStateStore.java:10-14`），但生产路径没人消费 listener，store 事实上退化成“只写不清的全局静态缓存”。
  2. `IdentityPanelScreenBootstrap` 允许玩家在任意 tick 按 `O` 开屏（`IdentityPanelScreenBootstrap.java:27-32,45-50`），并不等待“已收到本会话第一份 authoritative snapshot”。
  3. `IdentityPanelScreen.init()` 把 cooldown、active identity、entry 列表全部固化到按钮树里（`IdentityPanelScreen.java:35-57`）。
  4. `IdentityPanelScreen.render()` 却又改为每帧读最新 snapshot 只刷新文字（`IdentityPanelScreen.java:64-79,88-110`），造成“文字新、按钮旧”的 split-brain UI。
  5. server 首份 fresh payload 不是由开屏同步请求触发，而是靠 `emit_identity_panel_state_payloads` 在 game tick 上发送（`server/src/network/identity_panel_emit.rs:16-40`）；因此 reconnect / 高延迟 / 切服窗口内，client 确实可能先读到 stale store 再晚收到 fresh payload。

- **影响面**：
  - `IdentityHudCornerLabel` 会在新会话短窗口内展示上一会话身份名，造成 HUD 串味。
  - `IdentityPanelScreen` 会把旧快照的 identityId、冷却态、active 行冻结进按钮，后续 fresh payload 只能改字，不能改按钮行为。
  - `sendIdentityCommand()` 走 slash command（`IdentityPanelScreen.java:147-155`），server 虽是权威校验，但 client 仍可能向玩家展示错误的“当前 / 可切换 / 冷却可用”状态，并在点击时发送错误 identityId。

## 这个 bug 对实际游玩体验的影响

- 玩家重连、切服、或网络抖动后，会先看到上一局/上一角色的身份名，破坏“当前披着哪张面具”这一核心信息的可信度。
- 更糟的是，若此时打开身份面板，界面可能显示新身份列表，但按钮仍在按旧身份 ID 行事；玩家会遭遇“明明点的是这一行，实际发的是另一行命令”或“文字写冷却结束，按钮却还是灰的”这类典型错 UI。
- 身份系统直接关联匿名博弈、洗白、冷藏与 NPC 观感。面板不可信会让玩家不敢依赖 client UI 做切身份决策，只能反复关开面板或靠试错命令确认，体验上非常割裂。

## 修复建议

1. 在 disconnect 路径显式清空 `IdentityPanelStateStore`，与 `BongNetworkHandler` 里其他 client store 的断线清理对齐。
2. `IdentityPanelScreen` 二选一：
   - 要么改成真正订阅 store，snapshot 变化时重建按钮树/刷新 `active` 与回调；
   - 要么改成“开屏时只读一次 authoritative snapshot”，并禁止 stale/未初始化状态下开屏。
3. 若继续保留 HUD 常驻 identity label，JOIN 后最好有“尚未同步 identity snapshot”空态，而不是沿用上个会话残值。

## 反方裁决

- **第 1 轮反方论点**：这只是 client UI 小瑕疵，server 会校验 `/identity`，所以不算真 bug。
  - **驳回理由**：server 权威只能防止越权，不能抹掉“按钮绑定错 identityId / 冷却态 / 当前态”的 player-facing 错误。这里不是纯显示误差，而是**可点击控件的行为与屏上文字脱钩**，属于真实交互 bug。

- **第 2 轮反方论点**：join 后 server 很快就会发 fresh `identity_panel_state`，窗口太短，不值得立项。
  - **驳回理由**：问题不在“窗口短不短”，而在**一旦面板在旧快照上 init，后来的 fresh payload 也不会修正按钮树**。也就是说，哪怕 stale 只持续几百毫秒，错误回调也会持续到玩家关掉这个 screen 为止。

## 反方裁决执行说明

- 当前会话没有可用的 subagent / delegate 通道可再开一轮外部怀疑者审阅，因此这里采用**退化处理**：基于源码证据手工完成两轮反方裁决，并把反方论点与驳回理由显式记录在案。

## 审计来源

bug-hunt round（worktree `bughunt-loop-20260705-by-client-overlay`，范围限定 client overlay / screen / surface sidepaths）。本条为 **report-only**；未改源码，只落一份 skeleton。候选经“disconnect 清理缺口 → stale store → 开屏固化回调 → render 不重建按钮”链路逐步收敛，未与 toast cross-session、visual tide sky、dash HUD、tool/weapon HUD leak 既有题目重叠。

## Finish Evidence

**验证结论**：skeleton 描述的两个子问题均为**真 bug**，已用代码现状（含修复前 `git show` 对照）核实：

1. **断线不清**：修复前 `client/src/main/java/com/bong/client/BongNetworkHandler.java` 的 `clearClientStateOnDisconnect()`（约 28 项 store 清理清单）里没有任何 `IdentityPanelStateStore` 调用；该 store（`client/src/main/java/com/bong/client/identity/IdentityPanelStateStore.java:19`）是跨 session 存活的 `static volatile snapshot`，此前只有测试专用 `resetForTest()`。断线重连后 HUD 角标（`IdentityHudCornerLabel`）和刚打开的身份面板会短暂展示上一局的身份数据。
2. **按钮回调冻结**：`IdentityPanelScreen.init()`（`client/src/main/java/com/bong/client/identity/IdentityPanelScreen.java:25-58`）把当时快照的 `identityId`/`cooldownPassed()` 固化进 `ButtonWidget.onPress` lambda，`render()`（同文件 61-81 行）只重画文字、不重建按钮；一旦面板在某个快照上 `init()` 完成，后续新快照到达只能改屏上文字，改不动按钮绑定的旧 `identityId`。

**落地清单**：
- `client/src/main/java/com/bong/client/identity/IdentityPanelStateStore.java` — 新增生产态 `clearOnDisconnect()`（复用 `replace(IdentityPanelState.empty())`，保证清空快照同时通知监听者，不误清监听者列表）。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java` — `clearClientStateOnDisconnect()` 接入 `IdentityPanelStateStore.clearOnDisconnect()`。
- `client/src/main/java/com/bong/client/identity/IdentityPanelScreenBootstrap.java` — `register()` 新增 `IdentityPanelStateStore.addListener(...)`，回调 `onStoreChanged(IdentityPanelState)`：面板打开期间任何一次 store 更新（含断线清空）都用全新 `IdentityPanelScreen()` 实例整个替换当前面板，逼它重新 `init()` 拿新鲜数据重建按钮；`MinecraftClient.getInstance()==null` 时 short-circuit。

**关键 commit**：
- `ba8fbf88`（2026-07-11）修复 plan-bughunt-client-identity-panel-stale-session-v1：断线清理清单补上 IdentityPanelStateStore
- `fff0335a`（2026-07-11）修复 plan-bughunt-client-identity-panel-stale-session-v1：面板订阅 store 消除按钮回调冻结

**测试结果**：
- 新增 `client/src/test/java/com/bong/client/identity/IdentityPanelStateStoreTest.java`（6 用例：清空非空快照 / 已空态幂等 / 通知单个监听者 / 通知多个监听者 / 不误清监听者注册表 / reconnect 后 `replace()` 仍生效）。
- 新增 `client/src/test/java/com/bong/client/identity/IdentityPanelScreenBootstrapTest.java`（2 用例：`onStoreChanged` 在无头环境 `MinecraftClient.getInstance()==null` 时对空态/非空态均 null-safe）。
- `client/src/test/java/com/bong/client/BongNetworkHandlerTest.java` 按既有三段式追加 2 用例（`disconnectClearsIdentityPanelStateStoreToPreventStaleSessionIdentityLeak` / `disconnectClearingIdentityPanelStateStoreDoesNotBlockNewSessionSnapshotAfterReconnect`），`@AfterEach` 补 `IdentityPanelStateStore.resetForTest()`。
- `cd client && ./gradlew test build` 全绿（`BUILD SUCCESSFUL`，13 actionable tasks）。
- 对抗验证：无上下文 read-only validator（opus）对 HEAD `fff0335ab5fea6e82eb35ca7c8fac3be4756cd17` 独立复核代码 + 编译 + 目标测试类，结论 `VERDICT: PASS`（附非阻塞观察：`IdentityPanelEntry` 未覆盖 `equals/hashCode`，当前测试未因此产生假阳性；`onStoreChanged` 未做 state 去重，面板打开时收到推送会清空正在输入的 `nameField`——均记为后续可选优化，不阻塞本次修复）。

**跨仓库核验**：本修复为 client-only（vanilla Fabric Screen + static store），无需 server/agent 侧改动；server 端 `identity_panel_state` payload 契约未变。

**遗留 / 后续**（超出本 plan 范围，供后续 plan 参考）：
- `IdentityPanelEntry` 可考虑补 `equals/hashCode`（或改 `record`），避免未来新增依赖值相等比较的测试产生假阳性。
- `IdentityPanelScreenBootstrap.onStoreChanged` 未做 snapshot 去重，面板打开期间任何一次 store 推送都会整份重建（含清空正在输入的 `nameField`）；若后续发现频繁推送影响输入体验，可加 `equals` 比对后跳过无变化的重建。
