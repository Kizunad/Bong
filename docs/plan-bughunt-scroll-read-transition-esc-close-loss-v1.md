# plan-bughunt-scroll-read-transition-esc-close-loss-v1

> **Active BugFix plan**。主题：残卷阅读屏在默认开屏转场期间被 `Esc` 取消时，视觉界面关闭但 `scroll_read_closed` 协议终态丢失，导致 client store 与 server 阅读 marker 悬挂。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 第一性原理复现转场取消绕过阅读关闭语义 | ⏳ | — |
| P1 | 最小修复 + 幂等关闭饱和回归 | ⬜ | — |
| P2 | JDK 17 完整门禁、主线同步与对抗验证 | ⬜ | — |

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

- [ ] 给需要协议终态的 Screen 增加统一的 transition-cancel close hook，`cancelAndClose()` 取消 pending/open screen 时必须调用 screen 自身的 close/settle 语义，而不是只 `setScreen(null)`。
- [ ] 对 `ScrollReadScreen` 增加最小兜底：无论是 `close()`、`removed()`、还是转场取消，都必须幂等执行 `ScrollReadStore.close()`，确保最多发送一次 `scroll_read_closed`。
- [ ] 评估 `InsightOfferScreen`、`AgentUiScreen`、`SparringInviteScreen`、`TradeOfferScreen` 等同样把协议终态放在 `close()` 的 screen，避免修 ScrollRead 后留下同类转场取消缺口。
- [ ] 为 `ScreenTransitionController.cancelAndClose()` 增加面向 pending newScreen 的单测，覆盖“open transition 尚未 complete 时按 Esc”的路径。

## 验收测试计划

- client 单测：构造 active transition 的 pending `ScrollReadScreen`，模拟 `Esc` 触发 `cancelAndClose()`，断言 `ScrollReadStore.close()` 被执行且 `scroll_read_closed` 只发送一次。
- client 单测：已打开 `ScrollReadScreen` 后关闭、转场取消、重复 Esc 均保持幂等，不重复发送 `scroll_read_closed`。
- server 单测或 e2e：`scroll_read_request` 后模拟 client 发送 `scroll_read_closed`，断言 `StopAnim` 发出且 `ScrollReading` 被移除；再补一条“转场取消路径仍能触发 close”的 client e2e 或集成回归。
- 回归排重：确认 #942 InsightOffer、#950 agent_ui close reason 的行为不被本修复回滚。

## 风险

- 直接让 `cancelAndClose()` 调 `Screen.close()` 可能改变部分 screen 对 `Esc` 的语义，尤其是“关闭即拒绝/取消/提交”的弹窗；需要按 screen 类型区分 pending-open、already-open、server-close 三种来源。
- `removed()` 兜底必须幂等，否则可能和正常 `close()` 重复发送终态。
- 如果统一修复覆盖所有 screen，需要额外检查 owo `BaseOwoScreen` 生命周期，避免 adapter dispose 顺序和协议回调互相递归。
