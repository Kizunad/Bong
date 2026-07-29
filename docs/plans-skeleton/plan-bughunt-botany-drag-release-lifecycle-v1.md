# plan-bughunt-botany-drag-release-lifecycle-v1（骨架）

> **骨架（草案）**。一句话主题：把 `BotanyDragState.dragging` 收束为当前无屏幕、窗口聚焦且采集会话可交互时的瞬时鼠标所有权；UI 接管、失焦或会话终止时只结束种植拖拽而不消费属于下一层的 LEFT RELEASE，阻断陈旧状态在关屏后吞掉首个松键。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | LEFT RELEASE ownership：screen-open teardown 与 vanilla/UI 放行 | ⬜ |
| P1 | tick/session/focus/disconnect lifecycle hardening + 全边界回归 | ⬜ |

## 接入面

- **进料**：`client/src/main/java/com/bong/client/mixin/MixinMouse.java:43-119` 在 `Mouse.onMouseButton` HEAD 仲裁 LEFT PRESS/RELEASE；`HarvestSessionStore.snapshot()` 提供当前采集会话；`BotanyHudPlanner` 记录本帧 panel bounds。
- **出料**：`BotanyDragState` 决定种植 HUD 是否拥有本次左键输入与累计偏移；`BotanyHudBootstrap.onEndClientTick` 在持有拖拽时跟踪鼠标，UI screen/vanilla 控件必须继续收到不属于种植 HUD 的 RELEASE。
- **共享类型 / event**：复用 `BotanyDragState`、`HarvestSessionViewModel`、`HarvestSessionStore`、`BotanyHudBootstrap`、`BotanyHudPlanner` 与 `TransitionInputPolicy`；禁止新增第二套静态 drag flag 或分散 disconnect callback。
- **跨仓库契约**：纯 Fabric client 输入生命周期修复；不改 server、agent、TypeBox/protobuf、CustomPayload 或采集请求 payload。
- **worldview 锚点**：无新增世界观/数值语义；只恢复 `plan-botany-v1` 已落地采集 HUD 的输入可用性。
- **qi_physics 锚点**：不读写真元/灵气、不新增物理常数或 ledger 事件。

## Canonical Finding Mapping

- 唯一 canonical owner 来自 `docs/finished_plans/plan-bughunt-r7-findings-v1.md:53-67` 的 r7 #10；Finding Mapping 将 `MixinMouse` screen-open 早退导致的 Botany stale drag 分类为 `independent-domain-fix`，指定 successor `plan-bughunt-botany-drag-release-lifecycle-v1`。
- 历史正文 `:29-32` 记录“拖拽中开屏 → LEFT RELEASE 被早退漏收 → 关屏后首个 release 被 stale drag 消费”的原始场景；本 plan 以当前 `origin/main` 重新验真，不继承旧行号或未证实扩展。
- 本 plan 不承接同表 #9 Agent UI close 泄漏；该 finding 已由 PR #709 修复并在 Mapping 中结案。

## 第一性验真（`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`，2026-07-29）

1. **PRESS 建立全局所有权**：`client/src/main/java/com/bong/client/botany/BotanyDragState.java:16-28,55-70` 保存静态 `dragging`；命中上次渲染 bounds 的 LEFT PRESS 后置 `true` 并消费，任何后续 LEFT RELEASE 只要看到 `dragging` 就置 `false` 并消费，不再检查 release 是否仍属于 panel。
2. **screen-open 早退漏收 teardown**：`MixinMouse.java:97-103` 在 LEFT 分支先检查 `client.currentScreen != null` 并直接 return，因此拖拽中打开 inventory/其他 screen 后发生的 RELEASE 不会到达 `BotanyDragState.onLeftButton(0, ...)`。
3. **关屏后错误消费真实可达**：screen 开闭不会改变 `lastSessionId`；同一个 interactive harvest session 仍在时，`maybeResetForSession` 不触发。关屏后一次 panel 外 LEFT PRESS 由 `onLeftButton(1)` 放行，但 stale `dragging` 仍为 true；随后的 RELEASE 被 `onLeftButton(0)` 返回 true，`MixinMouse.java:116-117` 执行 `ci.cancel()`，吞掉本应属于 vanilla/其他 HUD 的松键。
4. **tick 会继续拖动隐藏 panel**：`BotanyHudBootstrap.java:58-74` 在检查 session interactive/mode 前就对任意 `dragging` 调 `tickDrag`，且未检查 `currentScreen` 或窗口焦点；开屏或 alt-tab 后即使 release 没进 mixin，偏移仍随鼠标更新。
5. **现有 reset 不覆盖屏幕/失焦**：`BotanyDragState.java:84-113` 仅在 session id 变化或 disconnect 时 reset；`BotanyHudPlanner.java:119-124` 的 session reset 依赖 HUD planner 运行，无法替代输入边界 teardown。
6. **现有测试漏集成边界**：`BotanyDragStateTest.java:16-87` 只直接测试 press/release、session change、disconnect；`BotanyHudBootstrapTest.java:21-55` 只测 disconnect ownership。没有 screen-open、release 放行、失焦或 terminal session 测试，也未 pin `MixinMouse` 早退与 teardown 的先后顺序。

## P0 — LEFT RELEASE ownership closure

- [ ] 在 `BotanyDragState` 新增幂等 `endDragWithoutConsuming()`（名称可等义）：仅结束当前 dragging ownership，保留玩家已提交的 `deltaX/deltaY` 与当前 session 的 rendered bounds；返回值不得被解释为“取消本次 Mouse 事件”。
- [ ] `MixinMouse` LEFT RELEASE 在 `currentScreen != null` 时必须先调用 non-consuming teardown，再直接放行 screen/vanilla；不得通过 `onLeftButton(0, ...)` 补发，因为该 API 的 `true` 契约明确要求 `ci.cancel()`，会把 teardown 与事件消费混成一条路径。
- [ ] `client == null` 仍直接放行且不得解引用；无 active drag 时 screen-open RELEASE 必须 no-op；LEFT PRESS 在 screen-open 时继续完全归 screen，不得启动 Botany drag。
- [ ] 保持 `TransitionInputPolicy.shouldBlockMouse(...)` 的现有最高优先级；其当前 policy 仅在 transition 锁定时拦 PRESS、明确放行 RELEASE（`ScreenTransitionTest` 已 pin），所以 RELEASE 仍能进入本 plan 的 teardown，禁止改 transition 模块。
- [ ] 饱和回归：`screen_open_release_ends_drag_without_consuming_event`、`screen_open_press_never_starts_botany_drag`、`screen_open_release_without_drag_is_noop`、`normal_panel_release_still_ends_and_consumes_drag`、`panel_outside_press_then_release_cannot_consume_stale_drag`。
- [ ] 可核验 production symbols：`MixinMouse.bong$captureHarvestPanelDrag`、`BotanyDragState.onLeftButton`、`BotanyDragState.endDragWithoutConsuming`（名称可等义）。

## P1 — Lifecycle hardening and integration pins

- [ ] 在 `BotanyHudBootstrap.onEndClientTick` 的 `tickDrag` 之前统一计算 drag eligibility：client/player 存在、`currentScreen == null`、窗口聚焦、`HarvestSessionViewModel.interactive()`；任一不满足即 non-consuming teardown，且本 tick 不更新 delta。
- [ ] 会话完成/中断/empty 但 session id 尚未换代时也必须结束 drag；不要依赖 `BotanyHudPlanner.maybeResetForSession`，planner 只负责布局 projection，不拥有原始输入生命周期。
- [ ] 继续复用 `BotanyHudBootstrap.clearOnDisconnect -> BotanyDragState.clearOnDisconnect` 的中央 adjunct cleanup；断线仍清 delta/bounds/session id，普通 screen-open/focus-loss teardown 只清 dragging，二者语义和测试必须分开。
- [ ] 若 `MixinMouse` 私有注入方法无法在 headless JUnit 驱动，抽取最小纯 policy（例如 `BotanyDragLifecyclePolicy`）表达 `screenOpen/focused/interactive/action -> {teardown, consume}`，mixin 与 tick bootstrap 共同调用；不得靠只读源码字符串测试代替行为测试。接线扫描只负责 pin mixin 真调用 policy/non-consuming API。
- [ ] 饱和回归：`screen_open_without_release_ends_drag_on_tick`、`focus_loss_ends_drag_without_moving_panel`、`terminal_session_ends_drag_without_resetting_delta`、`active_focused_session_keeps_dragging_and_updates_delta`、`disconnect_clear_ends_drag_and_invalidates_bounds`、`reopen_same_session_after_screen_teardown_keeps_committed_delta_but_not_dragging`。
- [ ] 可核验 symbols：`BotanyHudBootstrap.onEndClientTick`、`BotanyDragLifecyclePolicy`（若抽取）、`HarvestSessionViewModel.interactive`、`BotanyDragState.clearOnDisconnect`、`BotanyHudPlanner.plan` 及上述 test symbols。

## 范围边界 / 已排除项

- 不改 panel anchor、尺寸、clamp 或偏移持久化；本 plan 只修 transient drag ownership。
- 不改采集 session 状态机、manual/auto 模式、按键、请求协议或 server 判定。
- 不重构右键盾牌仲裁、`TransitionInputPolicy` 或全客户端通用 pointer-capture 框架；只确保 Botany 不遗留自己的 LEFT ownership。
- 不新增 HUD/VFX/SFX/动画或图标；这是不可见的输入生命周期修复，现有 Botany 表现保持不变。

## §8 开放问题（P0 前需追加 §8.1 决议）

1. 是否需要抽 `BotanyDragLifecyclePolicy` 以行为测试 mixin/tick 共同语义；若不抽，必须给出可执行的 mixin test harness，不能只测试 `BotanyDragState` 私有实现。

> 该项须按 `docs/CLAUDE.md §五` 追加当前 `file:line + plan 章节` 双锚点决议后再实施；不得由 bugfix subagent 临场改变事件所有权。窗口焦点入口已由当前 pinned Yarn 收口为 `MinecraftClient.isWindowFocused()`，不再作为开放问题。

## §10 实施工作流

### §10.1 BugFix 单 skeleton / 单 PR

本文件走根 `CLAUDE.md` BugFix 专用流程：一个 skeleton = 一个修复 subagent = 一个常驻 slot = 一个 PR。subagent promotion 后重新验真，以 P0 ownership fix、P1 lifecycle/tests 为中文原子 commit，最终补 `## Finish Evidence` 并归档；禁止交给 `/consume-plan` 或拆分为多个 implementation PR。

### §10.2 验收门

- 在 `client/` 下通过 `flock /tmp/bong-gradle.lock -c './gradlew test build'` 运行完整门禁；只触 client 栈，不跑 cargo/npm/worldgen。
- 不运行 `scripts/test-tmux-shutdown-order.sh`、`scripts/test-server-lifecycle.sh` 或任何调用它们的本地 suite；关停覆盖留给 GitHub e2e。
- 对显式 slot 绝对路径 + exact HEAD SHA 启动 fresh-context read-only validator；任何 HEAD 变化都重验。push 前紧邻 `git fetch origin && git merge origin/main`，merge 带进改动则重跑 client gate/validator。
- push 唯一 bugfix 分支后开一个 PR并独立评论 `/review`；返工留在同 PR。提交均为中文原子 commit，带 `Model: <精确模型 id>` 与 `Co-Authored-By` trailer。

### §10.3 归档前本地验收

- 完整重现并锁死：panel PRESS → 打开 screen → screen 内 RELEASE → 关闭 screen → panel 外 click；screen RELEASE 不被 Botany cancel，关屏后的 click pair 也不被 stale drag 吞键。
- screen-open、失焦、terminal session 三种无 RELEASE 边界都结束 dragging 且停止 delta 更新；同 session 恢复后保留已提交偏移，disconnect 则完整清 delta/bounds/session。
- P0/P1 全部 ✅、client gate 与 exact-HEAD validator PASS 后填写 `## Finish Evidence` 并迁入 `docs/finished_plans/`，再 push、开唯一 PR、评论 `/review`；review/e2e 后置意见只在同 PR 返工。
