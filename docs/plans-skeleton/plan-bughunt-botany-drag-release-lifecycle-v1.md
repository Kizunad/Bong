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
- [ ] 在 `inputLocked == false` 时，screen-open LEFT PRESS 仍完全归 screen，不启动 Botany drag；无 active drag 的 screen-open RELEASE 为 no-op。`inputLocked == true` 的 LEFT PRESS 则由下一条规定的 transition-first 仲裁消费。
- [ ] 仲裁优先级固定为 `TransitionInputPolicy.shouldBlockMouse` 先于 screen/drag 分支：当 `currentScreen != null` 且 `inputLocked == true` 时，LEFT PRESS 由 transition lock 消费，Botany drag state 不改变；同样条件下 LEFT RELEASE 不被该 policy 拦截，随后执行 non-consuming teardown 并放行 screen/vanilla。P1 必须锁定这两个可观察结果。
- [ ] 不修改 `TransitionInputPolicy` 或右键盾牌路径的行为与设计 authority；P1 仅经共享 `MixinMouse` 仲裁边界锁定它们当前的可观察结果，作为本次左键改动的 no-regression guard。

## P1 — Regression closure

- [ ] `screen_open_release_ends_drag_without_consuming_event`：前置为 interactive harvest session、有效 rendered bounds、无 transition lock、`isDragging() == false`；先在无 screen 时经 `MixinMouse.bong$captureHarvestPanelDrag` HEAD callback 发送 panel 内 LEFT PRESS，断言该 PRESS 的 `CallbackInfo` 被取消且 `BotanyDragState.isDragging()` 明确由 false 变为 true，再打开 screen 发送 LEFT RELEASE。断言该 RELEASE 的 `CallbackInfo` 未取消且 `BotanyDragState.isDragging()` 明确由 true 变为 false；除 drag ownership 外 session identity、delta/bounds 和 transition state 不得改变。
- [ ] `screen_open_press_never_starts_botany_drag`：前置为 interactive session、有效 bounds、`currentScreen != null`、`inputLocked == false`、鼠标位于 panel 内且当前未拖拽；经同一 HEAD callback 发送 LEFT PRESS。断言 `CallbackInfo` 未取消、`isDragging()` 仍为 false；delta/bounds、session identity 和 transition state 不得改变。
- [ ] `screen_open_release_without_drag_is_noop`：前置为 interactive session、`currentScreen != null`、`inputLocked == false`、`isDragging() == false`，并经同一 HEAD callback 发送 LEFT RELEASE。断言 `CallbackInfo` 未取消、drag ownership 仍为 false；delta/bounds、session identity 和 transition state 不得改变。
- [ ] `normal_panel_release_still_ends_and_consumes_drag`：前置为 interactive session、`currentScreen == null`、`inputLocked == false`、有效 bounds、`isDragging() == false`；经同一 `MixinMouse.bong$captureHarvestPanelDrag` HEAD callback 发送 panel 内 LEFT PRESS 后发送 LEFT RELEASE。断言 PRESS 的 `CallbackInfo` 被取消且 ownership 明确由 false 变为 true，RELEASE 的 `CallbackInfo` 被取消且 ownership 明确由 true 变为 false；已提交 delta/bounds、session identity 和 transition state 不得改变。
- [ ] `panel_outside_press_then_release_cannot_consume_stale_drag`：前置为同一 interactive session、有效 bounds、无 screen、`isDragging() == false`；先经真实 callback 发送 panel 内 LEFT PRESS，断言其被取消且 ownership 由 false 变为 true，再打开 screen 经真实 callback 发送 LEFT RELEASE，断言其未取消且 ownership 由 true 变为 false，随后关闭 screen；在 panel 外经同一 callback 发送 LEFT PRESS 与 LEFT RELEASE。断言这两个外部事件的 `CallbackInfo` 均未取消、ownership 始终为 false；delta/bounds、session identity 和 transition state 不得改变。
- [ ] `screen_open_release_preserves_committed_drag_geometry`：前置为 interactive session、`inputLocked == false`、有效 rendered bounds、`isDragging() == false`、已记录的 session identity 与 transition state；经同一 `MixinMouse.bong$captureHarvestPanelDrag` HEAD callback 发送 panel 内 LEFT PRESS，断言该 PRESS 被取消且 ownership 由 false 变为 true，再用 `tickDrag` 提交非零 delta，随后打开 screen发送 LEFT RELEASE。断言该 RELEASE 的 `CallbackInfo` 未取消、`dragging` 由 true 变为 false，且 teardown 前后的 `deltaX`、`deltaY`、panel bounds、session identity 和 transition state 完全相同。
- [ ] `mixin_mouse_transition_lock_preserves_arbitration_priority`：前置为 interactive session、有效 bounds、`currentScreen == null`、`inputLocked == false`；先经真实 `MixinMouse.bong$captureHarvestPanelDrag` HEAD callback 发送 panel 内 LEFT PRESS，并用 `tickDrag` 提交非零 delta，断言该 PRESS 的 `CallbackInfo` 被取消且 Botany ownership 已由 false 变为 true。随后打开 screen、置 `inputLocked == true` 并记录此时的 delta/bounds、session identity、transition state；经同一 callback 发送 LEFT PRESS，断言其 `CallbackInfo` 被 transition lock 取消且既有 ownership 仍为 true、记录状态不变；再发送 LEFT RELEASE，断言其 `CallbackInfo` 未取消、ownership 明确由 true 变为 false，且 delta/bounds、session identity、transition state 不变，从而锁定 transition-first、screen-open-second 的优先级。只断言取消结果、ownership、geometry、session 与 transition outcome，不断言内部调用顺序。
- [ ] `mixin_mouse_right_shield_press_release_preserves_arbitration`：前置为 interactive session、`currentScreen == null`、`inputLocked == false`、有效 bounds、已记录 session identity 与 transition state、off-hand 为 `InventoryEquipRules.isShieldPublic` 的公开盾牌；经同一 `MixinMouse.bong$captureHarvestPanelDrag` HEAD callback 发送 RIGHT PRESS 后 RIGHT RELEASE。断言 PRESS 的 `CallbackInfo` 被取消、RaiseShield observable outcome 出现且 `bong$shieldRightHeld` 为 true；断言 RELEASE 的 `CallbackInfo` 被取消、LowerShield observable outcome 出现且 ownership 清除；Botany drag state、delta/bounds、session identity 和 transition state 不得因右键改变。
- [ ] `mixin_mouse_screen_open_right_release_lowers_without_consuming`：前置为 interactive session、有效 bounds、已记录 session identity 与 transition state、`currentScreen == null`、`inputLocked == false`、公开盾牌，经同一 `MixinMouse.bong$captureHarvestPanelDrag` HEAD callback 建立 RIGHT shield ownership 后打开 screen发送 RIGHT RELEASE。断言 LowerShield observable outcome 出现、`bong$shieldRightHeld` 清除且 RELEASE 的 `CallbackInfo` 未取消并继续交给 screen/vanilla；Botany drag state、delta/bounds、session identity 和 transition state 不得改变。
- [ ] `no_interactive_session_passes_left_input_without_drag_mutation`：前置为 `HarvestSessionStore.snapshot()` 非 interactive、无 screen、`inputLocked == false`、已记录 `BotanyDragState` 的 ownership、delta/bounds、session identity 与 transition state；经真实 `MixinMouse.bong$captureHarvestPanelDrag` HEAD callback 分别发送 LEFT PRESS 与 LEFT RELEASE。断言两次 `CallbackInfo` 均未取消，drag ownership、delta/bounds、session identity 与 transition state 均不改变，且不产生 Botany 侧消费。
- [ ] `invalid_rendered_bounds_passes_left_input_without_drag_mutation`：前置为 interactive session、无 screen、`inputLocked == false`，但 `BotanyDragState` 未记录 bounds 或 bounds 宽/高为零或负值，且已记录 ownership、delta/bounds、session identity 与 transition state；经同一真实 callback 在 panel 逻辑位置发送 LEFT PRESS 与 LEFT RELEASE。断言两次 `CallbackInfo` 均未取消，drag ownership、delta/bounds、session identity 与 transition state 均不改变，且不产生 Botany 侧消费。
- [ ] 所有上述测试声明都必须从 `MixinMouse.bong$captureHarvestPanelDrag` 的真实仲裁边界取得可观察结果；可增加唯一的 package-private `MixinMouse` callback harness，生命周期仅限 client test invocation，不拥有第二份 drag/shield 状态、不复制分支条件。若 Fabric mixin callback 不能 headless 驱动，P1 必须保持 ⬜ 且记录 `[BLOCKED]`：失败的精确 callback seam、已尝试的 harness 接线与失败原因；该记录不是行为验收，不得以 wiring pin 或说明替代任一 P1 回归。只有使真实 callback 可驱动的 harness，或调度方明确决定改变范围，才能解除该 blocker。

## 可核验 symbols

- `MixinMouse.bong$captureHarvestPanelDrag`
- `BotanyDragState.onLeftButton`
- `BotanyDragState.endDragWithoutConsuming`（最终名称可等义）
- `TransitionInputPolicy.shouldBlockMouse`
- `BotanyDragStateTest`

## 非本 plan 交付物

以下是邻接观察，不属于 `docs/finished_plans/plan-bughunt-r7-findings-v1.md:53-67` Finding Mapping #10 分配给本 successor 的 r7 #10；不得在实现 PR 顺手扩大范围：

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
