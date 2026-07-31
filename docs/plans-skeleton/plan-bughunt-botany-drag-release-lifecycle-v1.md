# plan-bughunt-botany-drag-release-lifecycle-v1（骨架）

> **GOAL**：修复 r7 #10 的 Botany LEFT RELEASE ownership：拖拽中打开 screen 后，`BotanyDragState.dragging` 必须结束，但属于 screen/vanilla 的 RELEASE 不得被 Botany 消费；关屏后的首个 click pair 不得再被 stale drag 吞键。
>
> **Canonical owner**：`docs/finished_plans/plan-bughunt-r7-findings-v1.md:53-67` Finding Mapping #10。当前 `origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14` 仍未修复；同表 #9 Agent UI 已由 PR #709 结案，不属于本 plan。
>
> **Delivery**：按根 `CLAUDE.md` BugFix 工作流，一个 skeleton = 一个修复 subagent = 一个常驻 slot = 一个 PR；不由 `/consume-plan` 消费。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | screen-open LEFT RELEASE non-consuming teardown | ⬜ |
| P1 | r7 #10 的行为回归与完整 client gate | ⬜ |

## 接入面

- **进料**：`client/src/main/java/com/bong/client/mixin/MixinMouse.java:43-119` 在 `Mouse.onMouseButton` HEAD 仲裁 LEFT PRESS/RELEASE；`HarvestSessionStore.snapshot()` 提供采集会话。
- **出料**：`BotanyDragState` 决定种植 HUD 是否拥有该左键事件；非 Botany 所有的 RELEASE 必须继续交给 screen/vanilla。
- **共享类型**：复用 `BotanyDragState`、`HarvestSessionViewModel`、`HarvestSessionStore` 与现有 `TransitionInputPolicy`；不新增第二套 drag flag。
- **跨仓库 / worldview / qi_physics**：纯 Fabric client 输入修复；不改 server、wire、采集规则、世界观或真元流。

## 第一性验真

- `BotanyDragState.java:16-28,55-70` 用静态 `dragging` 保存所有权；任何后续 LEFT RELEASE 只要看到 `dragging` 就返回 true，调用方因此 cancel 事件。
- `MixinMouse.java:97-103` 在 `currentScreen != null` 时早退，拖拽中开屏后的 RELEASE 到不了 `onLeftButton(0, ...)`，`dragging` 遗留。
- 同一 interactive session 关屏后，panel 外 PRESS 会放行但不清旧 `dragging`；随后的 RELEASE 在 `MixinMouse.java:116-117` 被 stale drag 返回 true 并 `ci.cancel()`，错误行为可达。
- `BotanyDragStateTest.java:16-87` 只覆盖直接 press/release、session change 与 disconnect；没有 screen-open release ownership 回归。

## P0 — LEFT RELEASE ownership closure

- [ ] 为 `BotanyDragState` 提供幂等的 non-consuming drag teardown（名称可等义）。具体 owner 是该类现有的静态 `volatile boolean dragging`；该 helper 是 screen-open RELEASE 路径唯一允许执行的 non-consuming teardown 写入点，只把 `dragging` 置为 `false`，保留当前 session 已提交的 delta/bounds。`MixinMouse` 只能调用该 helper，不得直接改写 drag state。
- [ ] `MixinMouse` 收到 LEFT RELEASE 且 `currentScreen != null` 时先 teardown，再直接放行 screen/vanilla；不得调用会返回“应 cancel”的消费型 `onLeftButton(0, ...)`。
- [ ] screen-open LEFT PRESS 仍完全归 screen，不启动 Botany drag；无 active drag 的 screen-open RELEASE 为 no-op。
- [ ] 不修改 `TransitionInputPolicy` 或右键盾牌路径的行为与设计 authority；P1 仅经共享 `MixinMouse` 仲裁边界锁定它们当前的可观察结果，作为本次左键改动的 no-regression guard。

## P1 — Regression closure

- [ ] `screen_open_release_ends_drag_without_consuming_event`
- [ ] `screen_open_press_never_starts_botany_drag`
- [ ] `screen_open_release_without_drag_is_noop`
- [ ] `normal_panel_release_still_ends_and_consumes_drag`
- [ ] `panel_outside_press_then_release_cannot_consume_stale_drag`
- [ ] `screen_open_release_preserves_committed_drag_geometry`：经同一 `MixinMouse.bong$captureHarvestPanelDrag` HEAD callback，先建立并 `tickDrag` 提交非零 delta 与有效 rendered bounds，再在 screen-open LEFT RELEASE 后断言事件未取消、`dragging` 已结束，且 teardown 前后的 `deltaX`、`deltaY`、panel bounds 完全相同。
- [ ] `mixin_mouse_transition_lock_preserves_arbitration_priority`：通过同一 `MixinMouse.bong$captureHarvestPanelDrag` HEAD callback，在 `ScreenTransitionController.inputLocked()` 为 true 时断言 LEFT PRESS 被消费且 `BotanyDragState` 不改变；断言 LEFT RELEASE 不被 `TransitionInputPolicy.shouldBlockMouse` 拦截并继续到 screen/vanilla。测试只断言 `CallbackInfo` 的取消结果与 drag ownership，不断言内部调用顺序。
- [ ] `mixin_mouse_right_shield_press_release_preserves_arbitration`：通过同一 HEAD callback，在无 screen 且 off-hand 为公开盾牌时断言 RIGHT PRESS 消费事件并产生一次 RaiseShield、`bong$shieldRightHeld` 取得 ownership；随后 RIGHT RELEASE 消费事件并产生一次 LowerShield、ownership 清除；同时断言 Botany drag state 不被右键改变。
- [ ] `mixin_mouse_screen_open_right_release_lowers_without_consuming`：沿同一 callback 先建立右键盾牌 ownership，再打开 screen 发送 RIGHT RELEASE；断言产生 LowerShield、ownership 清除且事件继续交给 screen/vanilla。
- [ ] 上述三条必须从 `MixinMouse.bong$captureHarvestPanelDrag` 的真实仲裁边界取得可观察结果：测试可增加唯一的 package-private `MixinMouse` callback harness，生命周期仅限 client test invocation，不拥有第二份 drag/shield 状态、不复制分支条件；若 Fabric mixin callback 无法 headless 驱动，P1 evidence 必须记录具体阻塞原因与该 seam 的生产接线 pin，不能以孤立的 `TransitionInputPolicy` 或 payload encoder 测试替代。
- [ ] 若 mixin 无法直接 headless 驱动，可抽最小纯 policy 供行为测试；仍须用接线 pin 证明 `MixinMouse` 生产路径调用该 policy/non-consuming teardown，不能只测孤立 state helper。

## 可核验 symbols

- `MixinMouse.bong$captureHarvestPanelDrag`
- `BotanyDragState.onLeftButton`
- `BotanyDragState.endDragWithoutConsuming`（最终名称可等义）
- `TransitionInputPolicy.shouldBlockMouse`
- `BotanyDragStateTest`

## 非本 plan 交付物

以下是邻接观察，不属于 PR #1304 Mapping 分配给本 successor 的 r7 #10；不得在实现 PR 顺手扩大范围：

- window focus loss、terminal session 无 RELEASE 与 hidden-panel tick drag 的通用 hardening。
- disconnect store registry、`BotanyHudBootstrap` / `BotanyHudPlanner` 生命周期重构。
- 不改 `TransitionInputPolicy`、右键盾牌仲裁或其行为/设计 authority；P1 可仅通过共享 `MixinMouse` 仲裁边界观察并锁定其既有结果，作为左键改动的 no-regression guard。
- panel anchor/clamp/偏移持久化、采集模式、请求 payload 与 server 判定。
- `scripts/build-token.sh` 的创建及 V 轨交付。

## 验收与安全边界

- Client gate：若实现时 `scripts/build-token.sh` 已存在，必须从 `client/` 目录按其真实 client CLI 运行；否则从 `client/` 目录执行 `flock /tmp/bong-gradle.lock -c './gradlew test build'`。只触 client 栈。
- 严禁本地运行 `scripts/test-tmux-shutdown-order.sh`、`scripts/test-server-lifecycle.sh` 或任何调用它们的 suite；GitHub e2e 保留该覆盖。
- push 前 `git fetch origin && git merge origin/main`；exact-HEAD fresh-context read-only validator PASS 后才能 push；本轮不触发 `/review`，由调度方统一收集 verdict。
- P0/P1 全部 ✅ 后补 `## Finish Evidence` 并归档；实现与归档仍保持唯一 BugFix PR。
