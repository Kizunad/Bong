# plan-bughunt-scroll-read-transition-esc-close-loss-v1

> **Finished BugFix plan**。主题：残卷阅读屏在默认开屏转场期间被 `Esc` 取消时，视觉界面关闭但 `scroll_read_closed` 协议终态丢失，导致 client store 与 server 阅读 marker 悬挂。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 第一性原理复现转场取消绕过阅读关闭语义 | ✅ | 2026-07-11 |
| P1 | 最小修复 + 幂等关闭饱和回归 | ✅ | 2026-07-11 |
| P2 | JDK 17 完整门禁、主线同步与对抗验证 | ✅ | 2026-07-11 |

## Bug 摘要

玩家阅读残卷时，server 成功处理 `scroll_read_request` 后会下发 `scroll_open`，client 写入 `ScrollReadStore` 并请求打开 `ScrollReadScreen`。`ScrollReadScreen` 没有在 `ScreenTransitionRegistry` 单独注册，因此默认走 `TransitionConfig.DEFAULT_FADE_200MS` 的 200ms 开屏转场。

如果玩家在这个开屏转场窗口内按 `Esc`，`MixinKeyboardSkillKeys` / `MixinScreenInputLock` 会把按键路由到 `ScreenTransitionController.cancelAndClose()`。该函数取消 active transition 并直接 `setScreen(null)`，不会调用 `ScrollReadScreen.close()`，因此不会执行 `ScrollReadStore.close()`，也不会发送 `scroll_read_closed` C2S。

结果是：屏幕视觉上被取消/关闭，但 client store 仍残留当前阅读快照，server 也收不到关闭事件，`ScrollReading` marker 和循环阅读动画可能一直残留到死亡或断线兜底。

## 对实际游玩体验的影响

玩家点开残卷后立刻按 `Esc` 是很自然的操作，尤其在误点、想快速收起、或双击/连按关闭时容易发生。当前表现会是“书页没打开或刚闪一下就没了”，但角色在别人视角和本地动画层仍可能保持读卷姿态，server 也继续认为玩家处于读卷中。

这不是纯视觉毛刺：`ScrollReadClosed` 是停止循环阅读动画并移除 server `ScrollReading` marker 的正式协议终态。该终态丢失后，玩家只能靠死亡或断线兜底恢复；继续游玩期间会出现读卷动作残留、状态与 UI 脱节、再次打开同一卷没有可靠恢复入口等体验问题。

## 证据定位

- `client/src/main/java/com/bong/client/BongClientFeatures.java:12`：`ENABLE_UI_TRANSITIONS = true`，默认启用 UI 转场。
- `client/src/main/java/com/bong/client/BongClient.java:87`：启动时注册 `ScreenTransitionController`。
- `client/src/main/java/com/bong/client/network/ScrollOpenHandler.java:56`：收到 `scroll_open` 后 `ScrollReadStore.replace(...)`。
- `client/src/main/java/com/bong/client/scroll/ScrollReadScreenBootstrap.java:21-27`：只注册 store listener 和断线清理，没有 tick 轮询自愈。
- `client/src/main/java/com/bong/client/scroll/ScrollReadScreenBootstrap.java:50-52`：store 推入后调用 `client.setScreen(new ScrollReadScreen(offer))`。
- `client/src/main/java/com/bong/client/ui/ScreenTransitionRegistry.java:199-210`：未注册的 screen 走 `getOrDefault(newScreen.getClass()).openSpec()`。
- `client/src/main/java/com/bong/client/ui/TransitionConfig.java:17-24`：默认 screen 转场是 200ms fade。
- `client/src/main/java/com/bong/client/ui/ScreenTransitionController.java:51-60`：转场开始时只保存 callback，真实 `setScreen(newScreen)` 延后到 complete。
- `client/src/main/java/com/bong/client/mixin/MixinKeyboardSkillKeys.java:19-23`：转场输入锁期间 `Esc` 调 `cancelAndClose(client)`。
- `client/src/main/java/com/bong/client/mixin/MixinScreenInputLock.java:15-19`：已有 screen 接收 keyPressed 时也会走同一 `cancelAndClose` 路径。
- `client/src/main/java/com/bong/client/ui/ScreenTransitionController.java:110-113`：`applyDirect(client, null)` 只是直接 `setScreen(null)`。
- `client/src/main/java/com/bong/client/scroll/ScrollReadScreen.java:183-189`：`scroll_read_closed` 只在 `close()` 里通过 `ScrollReadStore.close()` 发送。
- `client/src/main/java/com/bong/client/scroll/ScrollReadStore.java:48-53`：`close()` 是发送 `ClientRequestSender.sendScrollReadClosed()` 并清空 snapshot 的入口。
- `server/src/network/client_request_handler.rs:2716-2724`：成功开卷且有 `anim_id` 时插入 `ScrollReading` marker。
- `server/src/network/client_request_handler.rs:2757-2779`：只有收到 `ScrollReadClosed` 才停止阅读动画并移除 `ScrollReading` marker。

## 触发路径

1. 玩家在背包中对可阅读残卷触发阅读请求。
2. server 接受 `scroll_read_request`，下发 `scroll_open`，并在有 `anim_id` 时插入 `ScrollReading` marker、播放 `bong:read_scroll`。
3. client `ScrollOpenHandler` 写入 `ScrollReadStore`，listener 调 `client.setScreen(new ScrollReadScreen(...))`。
4. 因 `ScrollReadScreen` 未注册专属转场，`ScreenTransitionController` 拦截本次 `setScreen`，开启默认 200ms fade，真实 screen 尚未完成打开。
5. 玩家在 200ms 内按 `Esc`。
6. 输入锁将 `Esc` 改道为 `ScreenTransitionController.cancelAndClose()`，取消待打开 screen 并 `setScreen(null)`。
7. `ScrollReadScreen.close()` 未执行，`ScrollReadStore.close()` 未执行，`scroll_read_closed` 未发送。
8. 视觉上阅读屏消失；client snapshot 与 server `ScrollReading` marker 继续残留。

## 反方审查记录

### Round 1

结论：PASS。

反方重点攻击转场期间 `Esc` 是否真实进入拦截路径、`setScreen(null)` 是否会间接调用 `close()`、是否已有重复 PR、以及 ScrollRead 是否有真实游玩影响。复核后保留：`TransitionInputPolicy` 对 `inputLocked + Esc` 返回 `CANCEL_AND_CLOSE`；从世界画面开屏时即使没有 current screen，`MixinKeyboardSkillKeys` 也会在 `Keyboard.onKey` HEAD 触发；`MinecraftClient.setScreen` 对旧 screen 走 `removed()` 而非 `close()`；`ScrollReadScreen` 没有 `removed()` 兜底；server 的 `ScrollReading` 需要 `ScrollReadClosed` 清理。

### Round 2

结论：PASS。

反方继续攻击默认转场是否真的开启、store 残留是否会自动重开、200ms 窗口是否太窄、是否重复 #942/#950。复核后保留：`ENABLE_UI_TRANSITIONS=true`；`ScrollReadScreen` 未在 registry 注册，走默认 200ms fade；`ScrollReadScreenBootstrap` 只响应 store 变化，没有 tick 轮询，转场取消后 snapshot 非空不会自动触发重开；玩家开卷后立刻按 `Esc` 属于正常路径；#942 是 InsightOffer 本地切屏吞决策，#950 是 agent_ui close reason 丢失，本 bug 是 ScrollRead 转场 ESC 导致 C2S close 丢失和 server 动画 marker 残留。

## Skeleton Fix Plan

- [x] 增加显式 opt-in 的 `PendingOpenCancellationHandler`；`cancelAndClose()` 与 rapid replacement 只结算声明了协议终态且未被同 session screen 延续的 pending screen，不把所有 `Screen.close()` 泛化调用。
- [x] `ScrollReadScreen` 捕获不可复用 `SessionToken`，`close()` / transition cancel / `removed()` 全部复用 token-CAS 幂等终态；旧 screen 的 `removed()` 因 token 不匹配不能误清后来会话。
- [x] 评估 `InsightOfferScreen`、`AgentUiScreen`、`SparringInviteScreen`、`TradeOfferScreen`：本修复不自动改变它们的 close/settle 语义；是否 opt-in 需各自按协议契约独立立项，避免跨题扩散。
- [x] 新增 `ScreenTransitionScrollCloseTest` 与 `ScrollReadScreenBootstrapTest`，覆盖 pending ScrollRead、重复 Esc、rapid replacement continuation、断线 pending 清理、同卷空态重开及“先 direct-close 当前 screen、后结算 pending”重入顺序。

## 验收测试计划

- client 单测：构造 active transition 的 pending `ScrollReadScreen`，模拟 `Esc` 触发 `cancelAndClose()`，断言 `ScrollReadStore.close()` 被执行且 `scroll_read_closed` 只发送一次。
- client 单测：已打开 `ScrollReadScreen` 后关闭、转场取消、重复 Esc 均保持幂等，不重复发送 `scroll_read_closed`。
- server 单测或 e2e：`scroll_read_request` 后模拟 client 发送 `scroll_read_closed`，断言 `StopAnim` 发出且 `ScrollReading` 被移除；再补一条“转场取消路径仍能触发 close”的 client e2e 或集成回归。
- 回归排重：确认 #942 InsightOffer、#950 agent_ui close reason 的行为不被本修复回滚。

## 风险

- 直接让 `cancelAndClose()` 调 `Screen.close()` 可能改变部分 screen 对 `Esc` 的语义，尤其是“关闭即拒绝/取消/提交”的弹窗；需要按 screen 类型区分 pending-open、already-open、server-close 三种来源。
- `removed()` 兜底必须幂等，否则可能和正常 `close()` 重复发送终态。
- 如果统一修复覆盖所有 screen，需要额外检查 owo `BaseOwoScreen` 生命周期，避免 adapter dispose 顺序和协议回调互相递归。

## Finish Evidence

### 落地清单

- P0：`client/src/test/java/com/bong/client/ui/ScreenTransitionScrollCloseTest.java`、`client/src/test/java/com/bong/client/scroll/ScrollReadScreenTest.java`
  - RED 在修复前证明 `cancelAndClose()` 只取消 handle，`ScrollReadStore.snapshot()` 仍残留。
  - fresh validator 追加 RED：当前已打开残卷、pending 为无关 screen 时，Esc 不能只结算 pending；当前残卷被 direct-close 后同样必须发送终态。
  - 本轮追加 RED：空态后复用同一 `ScrollOpenViewModel` 实例时，旧 screen 会通过 ABA 对象身份误结算新会话；旧实现 `1/1 FAIL`。
- P1：`client/src/main/java/com/bong/client/ui/ScreenTransitionController.java`
  - 新增 `PendingOpenCancellationHandler.continuesWith(...)`、`CurrentScreenCancellationHandler`、`cancelPendingOpen(...)` 与 `closeCurrentThenSettlePending(...)`。
  - 先 direct-close 当前 screen，再依次结算显式 opt-in 的 current / pending 协议，避免 A→pending B 时 store listener 重入生成残留 transition。
  - rapid replacement、禁用/零时长转场及 same-screen 清理都会结算被覆盖的 opt-in pending screen；同 token replacement 作为会话延续，不误发终态。
- P1：`client/src/main/java/com/bong/client/scroll/ScrollReadScreen.java`
  - screen 构造时捕获 `SessionToken`；pending/current 取消、普通 `close()` 与 `removed()` 均经同一 token-CAS 幂等发送 `scroll_read_closed` 并清空 store。
- P1：`client/src/main/java/com/bong/client/scroll/ScrollReadStore.java`
  - `AtomicReference<ActiveSession>` 分离 view model 与不可复用 token；同会话同卷刷新保留 token，经过空态后即使复用同一对象也必须换 token。
  - `closeIfCurrent(SessionToken)` 用 CAS 抢占终态，并发 close、旧 screen、transport 重入均不能重复发包或误清后来会话。
- P1：`client/src/main/java/com/bong/client/scroll/ScrollReadScreenBootstrap.java`
  - listener 携带精确 `ActiveSession` 快照；迟到 open/close 任务先经 `isCurrent(...)` 拒绝。
  - current/pending screen 按 token 而非 `scrollId` 判归属；断线空态会精确取消 pending ScrollRead，防止清理后迟到开屏。
- P2：最终同步 `origin/main@d6237cc7`；先合入 botany server，后合入 Dugu HUD 断线清理的 client 代码/测试，均未触及 ScrollRead 文件且完整 client gate 复验通过。`f4035e33` 引入的 transport 拒绝语义已由 `58932dc4` 及后续 token-CAS 回归覆盖。

### 关键 commit

- `815b2cfc`（2026-07-11）：提升残卷转场关闭丢失计划为 active。
- `a701d0aa`（2026-07-11）：锁定残卷开屏转场取消丢失关闭终态（RED）。
- `23941f27`（2026-07-11）：修复残卷开屏转场取消遗漏关闭终态。
- `0de85b76`（2026-07-11）：补齐重复 Esc 幂等与无关 screen 隔离回归。
- `d161135c`（2026-07-11）：依据 validator FAIL 修正 A→B 重入收口顺序。
- `554849f2`（2026-07-11）：合并最新 `origin/main` 并复验。
- `f4035e33`（2026-07-11）：再次合并 `origin/main@7cfcba5f`，同步 transport 拒绝语义。
- `58932dc4`（2026-07-11）：依据 validator FAIL 修复 transport 拒绝时本地 store 悬挂。
- `2f2d5081`（2026-07-11）：合并 `origin/main@307ab4db`，同步新手兴趣点修复。
- `dea0f08b`（2026-07-11）：保护残卷关闭的会话身份，覆盖旧 screen 与 transport 重入替换边界。
- `d1b233e1`（2026-07-11）：结算转场 direct-close 移除的当前残卷并补精确回归。
- `22cec694`（2026-07-11）：保护残卷普通关闭的旧 screen 身份。
- `dfca9786`（2026-07-11）：以不可复用 session token 收口 close/removed/并发终态与 ABA 重开。
- `2e63c386`（2026-07-11）：按 token 收口 bootstrap 迟到任务、rapid replacement 与断线 pending 转场。
- `c76c13b6`（2026-07-11）：合并 `origin/main@340d7776`，无 client 冲突。
- `168fea58`（2026-07-11）：补齐 detached screen、无 live client 与同卷新 token 覆盖边界。
- `84165061`（2026-07-11）：合并 `origin/main@d6237cc7` 的 Dugu HUD 断线清理，并完成同栈复验。

### 测试结果

- RED（JDK 17）：`./gradlew test --tests com.bong.client.ui.ScreenTransitionScrollCloseTest`
  - 修复前 `1 test completed, 1 failed`，失败点为阅读 store 未清空。
- ABA RED（JDK 17）：`./gradlew test --tests 'com.bong.client.scroll.ScrollReadScreenTest.close_oldScreenDoesNotSettleReopenedSessionWhenViewModelInstanceIsReused'`
  - 前 worker 的对象身份/CAS 补丁下 `1 test completed, 1 failed`，失败点为旧 screen 清掉空态后重开的同对象新会话。
- targeted GREEN（JDK 17）：四组定向测试最终 `44/44 PASS`：
  - `ScrollReadScreenTest`：`15/15`；`ScrollReadStoreTest`：`15/15`；
  - `ScrollReadScreenBootstrapTest`：`4/4`；`ScreenTransitionScrollCloseTest`：`10/10`；
  - pending ScrollRead 发送且仅发送一条 `scroll_read_closed`；
  - 重复 Esc 幂等；
  - 无关 pending screen 不误结算；
  - 当前 ScrollRead → 无关 pending screen 时，当前会话恰好结算一次；
  - direct-close 发生在 pending settle 之前。
  - transport 拒绝 `scroll_read_closed` 时仍清空本地 store，不让视觉已关闭的会话永久悬挂。
  - transport 抛 `RuntimeException` 时同样完成本地终态，重复 Esc 保持幂等。
  - 同卷刷新保留 token、空态重开轮换 token、同对象 ABA、并发双 close、removed、rapid replacement continuation 与断线 pending 清理均有专属用例。
- 完整门禁（JDK 17）：`./gradlew test build`
  - 修复后：`BUILD SUCCESSFUL`。
  - 合并 `origin/main@307ab4db` 并完成 transport 拒绝/异常返工后：`3789/3789 PASS`，`BUILD SUCCESSFUL`，产物 `client/build/libs/bong-client-0.1.0.jar`。
  - current-screen 结算返工后：`3792/3792 PASS`，零失败零跳过，`BUILD SUCCESSFUL`。
  - session token / removed / rapid replacement 实现：`3835/3835 PASS`，零失败零错误零跳过，`BUILD SUCCESSFUL`，产物 `client/build/libs/bong-client-0.1.0.jar`（194 MiB）。
  - 合并 `origin/main@340d7776` 并补齐最终饱和边界后：`3838/3838 PASS`，零失败零错误零跳过，`:test` 实际执行，`BUILD SUCCESSFUL`。
  - 最终合并 `origin/main@d6237cc7` 的同栈 Dugu HUD 断线测试后：`3847/3847 PASS`，零失败零错误零跳过，`:test`、remap、assemble、check、build 全部实际执行，`BUILD SUCCESSFUL`。

### 跨仓库核验

- client 终态入口：`ClientRequestSender.sendScrollReadClosed()` → `ClientRequestProtocol.encodeScrollReadClosed()`。
- server 既有消费契约：`ClientRequestV1::ScrollReadClosed` 停止动画并移除 `ScrollReading` marker；本 PR 不修改 server/schema。
- agent/schema 既有 wire 契约：`agent/packages/schema/src/client-request.ts::ScrollReadClosedRequestV1` 与 `ClientRequestV1` union 保留 `type="scroll_read_closed", v=1`；本 PR 不修改 schema 构建产物。
- fresh validator：
  - `d161135c`：`PASS d161135c9295e3880ae4a4a24287471b41271726`。
  - post-merge `554849f2`：`PASS 554849f2bd0ed7da2ac7fd86c0751eb270fb1922`。
  - `f4035e33`：`FAIL`，发现最新 main 的 transport 拒绝会使本地 store 悬挂；已由 `58932dc4` 返工并补回归。
  - `f6b5f6aa`：`FAIL`，发现当前 ScrollRead → 无关 pending screen 时 direct-close 会遗漏当前会话终态；已由 `dea0f08b` + `d1b233e1` 返工并补身份隔离与精确回归。

### 遗留 / 后续

- 其它携带协议终态的 screen 是否实现 `PendingOpenCancellationHandler`，必须按各自 close/decline/replace 契约单独验真；本 plan 不做跨题批量行为改变。
- 无依赖、生产配置、工具链或视觉资产变更。
