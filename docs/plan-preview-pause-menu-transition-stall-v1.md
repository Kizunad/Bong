# plan-preview-pause-menu-transition-stall-v1（骨架）

> **骨架（草案）**。一句话主题：preview harness 在尝试“每 tick 强关弹出 UI”时，会和全局 `setScreen` 过渡层互相打架，导致 `GameMenuScreen` 一类带过渡的界面**永远关不掉**，最终把 worldgen/preview 截图遮住或直接拖到超时。范围聚焦 `client preview + ui transition + setScreen` 交互链，未触及 craft/social renown/tribulation/botany/dying elder 近期题。

> 立项动机：本轮 Bughunt 聚焦 `server/src/preview`、`client/.../preview`、`client/.../ui` 和相关交互链。这里命中的是一条高置信 preview/ui 真 bug：preview harness 的“兜底清屏”逻辑假设 `setScreen(null)` 会立刻生效，但仓库已经把几乎所有 `setScreen` 统一接入过渡拦截；只要焦点切换真的弹出暂停菜单，preview harness 自己就会把关闭动画反复打断，截图永远回不到世界画面。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 preview harness 强关 UI 与过渡层冲突，暂停菜单可永久卡屏 | fix_pr | ⬜ |

## P0 — 🔴 preview harness 强关 UI 与过渡层冲突，暂停菜单可永久卡屏

- **结论**：`client/src/main/java/com/bong/client/preview/PreviewSession.java:64-73` 在 `WAIT_WORLD` 之后的**每一个 tick**都执行“若 `currentScreen != null` 则 `client.setScreen(null)`”；但 `client/src/main/java/com/bong/client/mixin/ScreenSetMixin.java:12-23` 会把绝大多数 `MinecraftClient.setScreen(...)` 统一交给 `ScreenTransitionController.interceptSetScreen(...)`，而 `client/src/main/java/com/bong/client/BongClientFeatures.java:12` 默认启用 `ENABLE_UI_TRANSITIONS=true`。这意味着 preview harness 并不是“直接关屏”，而是在**每 tick 重开一次关闭动画**。
- **根因链路**：
  - `PreviewSession.onTick` 的注释已经写明真实触发场景：`pauseOnLostFocus=false` 仍需要“防 WSLg / xvfb 焦点切换弹 `GameMenuScreen` 遮住截图”，所以作者自己承认**焦点抖动时会冒出暂停菜单**（`PreviewSession.java:69-72`）。
  - `ScreenTransitionRegistry` 给 `GameMenuScreen` 配的是 `FADE 150ms` 开关动画，而不是 `NONE`（`client/src/main/java/com/bong/client/ui/ScreenTransitionRegistry.java:84-86`）。
  - `ScreenTransitionController.interceptSetScreen` 在发现已有 active transition 时，会先 `previous.handle().cancel()`，再创建新的 transition；真正的 `applyDirect(client, nextScreen)` 只挂在新 transition 的完成回调上（`client/src/main/java/com/bong/client/ui/ScreenTransitionController.java:45-60`）。
  - `ScreenTransition.TransitionHandle.complete()` 若该 handle 已被 `cancel()`，会直接 `return`，不会跑 callback（`client/src/main/java/com/bong/client/ui/ScreenTransition.java:164-175`）。
  - 因此一旦 preview harness 遇到 `GameMenuScreen`，它会在 20TPS 下反复调用 `setScreen(null)`；每次新调用都会取消上一次 150ms fade，导致**没有任何一次 close transition 能走到 `complete()`**，屏幕就会一直停留在旧的 `GameMenuScreen`。
- **复现路径**：
  1. 启动 preview 场景：`cd client && ./gradlew runClientPreview`，或走 worldgen snapshot/preview harness 常规链路。
  2. 保持 `BONG_PREVIEW_HARNESS=1`，进入 `WAIT_CHUNKS` / `SETTLE` / `SHOOT` 任一 `WAIT_WORLD` 之后的阶段。
  3. 在 WSLg / xvfb / 远控窗口里制造一次焦点切换或暂停菜单弹出，让 `client.currentScreen` 变成 `GameMenuScreen`。
  4. preview harness 进入 `client.setScreen(null)` 每 tick 重试；与此同时 `ScreenSetMixin` + `ScreenTransitionController` 每 tick 取消旧 fade、重建新 fade。
  5. 结果是暂停菜单不消失，后续截图要么直接拍到菜单遮罩，要么流程一直卡在“有 screen 盖住世界”的异常状态直到超时退出。
- **影响面**：
  - 直接影响 `client/.../preview/PreviewSession` 驱动的 worldgen/preview 截图链。
  - 同类风险也波及任何“在 tick loop 里反复 `setScreen(null)` 兜底”的截图/演示 harness；本仓库 `WeaponScreenshotHarness` 采用了同一模式，但本 skeleton 只把 preview 链列为主 bug，不扩题。
  - server 侧 `server/src/preview/mod.rs` 的传送、视距和装饰逻辑本身可以正常运行，但最终产物会被 client UI 层遮挡，形成“server 已经把镜头送到位，client 却没法回到世界画面”的跨层割裂。
- **这个 bug 对实际游玩体验的影响**：
  - 在预览服、演示服、录屏验收或 worldgen PR 看图场景里，玩家/开发者看到的不是目标地形，而是卡住的暂停菜单或被 UI 遮住的世界；结果是截图误判、演示失败、PR 预览失真，而且问题只在焦点切换等真实桌面环境下触发，排查成本很高。
- **修复建议**：
  - preview harness 的“强关屏幕”路径必须绕过动画层，不能继续走普通 `client.setScreen(null)`。
  - 候选做法 A：preview harness 检测到 `currentScreen != null` 时，直接调用 `ScreenTransitionController.cancelAndClose(client)`，先清 active transition 再 `applyDirect(..., null)`。
  - 候选做法 B：当 `BONG_PREVIEW_HARNESS=1` 时，为 preview harness 关闭界面提供专用 bypass，不让 `ScreenSetMixin` / `interceptSetScreen` 接管这一类“紧急清屏”请求。
  - 候选做法 C：把 `GameMenuScreen`（至少 close path）在 preview harness 模式下改成 `Type.NONE`；这比 A/B 更弱，只治这一类 screen，不治别的带过渡弹窗。

## 反方裁决

> 本会话未提供可用的 subagent / delegate 工具，因此两轮反方裁决均由当前会话手工完成；这里如实记录退化处理和驳回理由。

### Round 1

- **反方论点**：`client.options.pauseOnLostFocus = false` 已经在 `PreviewSession.onTick` 设置了，正常情况下根本不会弹 `GameMenuScreen`；既然 screen 不会出现，就谈不上“关不掉”。
- **驳回理由**：
  - 若作者确信 screen 不会再出现，就没有必要保留 `if (client.currentScreen != null && phase != Phase.WAIT_WORLD) { client.setScreen(null); }` 这一整段兜底。
  - 代码注释明确写的是“防 WSLg / xvfb 焦点切换弹 `GameMenuScreen` 遮住截图”，说明该现象不是理论猜测，而是已经命中过的真实运行环境问题。
  - 一旦 `currentScreen` 真的变为非空，后续 bug 就是确定性的：preview harness 每 tick 重试关闭，transition 层每 tick 取消旧动画，没有随机因素。

### Round 2

- **反方论点**：就算 `setScreen(null)` 被过渡层拦截，关闭动画也只有 150ms，也许几帧后自然会完成，screen 还是能关掉。
- **驳回理由**：
  - `ScreenTransitionController.interceptSetScreen` 在 active transition 存在时会**先 cancel 旧 handle**，再创建新 handle；preview harness 是**每 tick**调用一次 `setScreen(null)`，频率约 50ms，小于 `GameMenuScreen` 的 150ms close duration。
  - 被 cancel 的 handle 在 `TransitionHandle.complete()` 里不会执行 callback；而真正把 `currentScreen` 置空的动作恰恰就在 callback 里。
  - 这不是“偶发慢一点”，而是“每次快要完成时都被下一 tick 重新打断”，所以是一个稳定的 livelock，而不是性能抖动。

## 开放问题

1. 是否应把“截图/录屏 harness 的紧急清屏”统一抽成一条公共 API，避免 preview、weapon 等 harness 各自手写 `setScreen(null)` tick loop？
2. 是否需要为 `ScreenTransitionController` 增补 pin 测试：同一 `oldScreen -> null` 在 transition 未完成前被重复请求时，不能无限 cancel 而导致 callback 永不落地？
3. 是否需要在 preview CI 里加入一个“屏幕上不允许残留 `currentScreen`”的 telemetry / log 断言，避免以后再次静默回归？

## 审计来源

bughunt-loop-20260705-ar，范围只看 `server/src/preview`、`client/.../preview`、`client/.../ui` 及相关交互链；本轮未修改源码、未跑修复，只做静态搜索、链路核对、反方裁决和 skeleton 记录。结论为 **1 个高置信 preview/ui 真 bug**：preview harness 的强关 UI 逻辑与全局 screen transition 拦截互相冲突，`GameMenuScreen` 可被卡成永久遮挡层。
