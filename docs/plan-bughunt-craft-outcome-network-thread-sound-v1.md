# plan-bughunt-craft-outcome-network-thread-sound-v1

> **活跃 plan skeleton**。一句话主题：`craft_outcome kind=completed` 经 `bong:server_data` 到达时，`CraftOutcomeHandler` 在 Fabric network thread 同步写 `CraftStore`，当前打开的 `CraftScreen` / `WorkbenchScreen` outcome listener 会在切回 client 主线程前直接改 screen 字段并调用 `client.player.playSound(...)`，违反 Fabric client networking 线程契约。

> 排重说明：这不是 #1016 的 `cast_sync -> SkillConfigPanelManager` 配置窗入口；本案落点是 `craft_outcome -> CraftStore -> Craft/Workbench 完成反馈`。但两者共享 `server_data` route 在 network thread 执行 handler side effect 的根因，后续修复可以合并覆盖。

## Bug 摘要

- `ClientPlayNetworking.registerGlobalReceiver(Identifier, PlayChannelHandler)` 的旧 API handler 运行在 network thread；Fabric 源码注释要求读完 buffer 后，访问 game state 必须用 `client.execute(...)` 切到 render/client thread。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:245-299` 在 `bong:server_data` receiver 中先同步执行 `ROUTER.route(jsonPayload, readableBytes)`，只有后续 `applyDispatch(...)` 分支被 `client.execute(...)` 包住。
- `client/src/main/java/com/bong/client/network/ServerDataRouter.java:314-315` 同步调用 `handler.handle(envelope)`。
- `client/src/main/java/com/bong/client/network/CraftOutcomeHandler.java:26-44` 在 `handle()` 内直接 `CraftStore.recordOutcome(...)`。
- `client/src/main/java/com/bong/client/craft/CraftStore.java:99-102` 同步通知 outcome listeners。
- `client/src/main/java/com/bong/client/craft/CraftScreen.java:47-52` 与 `client/src/main/java/com/bong/client/craft/WorkbenchScreen.java:51-56` 的 completed 分支会先 `flashTicks = 6`、`playCompleteSound()`，然后才 `scheduleRefresh(...)`。
- `playCompleteSound()` 读取 `MinecraftClient.getInstance().player` 并调用 `client.player.playSound(...)`（`CraftScreen.java:296-300`、`WorkbenchScreen.java:271-275`），属于 player/world/sound client state 访问，不应在 network thread 执行。

## 对实际游玩体验的影响

玩家在手搓台或末法制作台完成制作时，完成反馈可能在网络线程触发。常见可见影响是完成音效偶发丢失、完成闪光不同步、完成瞬间卡顿；更坏情况下，sound/player/world/camera 状态被非主线程访问可能触发难复现的客户端异常。这个问题发生在正常制作完成路径上，不需要异常操作。

## 证据定位

- `BongNetworkHandler.registerServerDataChannel`：`ROUTER.route(...)` 在 receiver 内同步执行，`client.execute(...)` 只包 `applyDispatch`。
- `ServerDataRouter.route`：直接 `handler.handle(envelope)`。
- `CraftOutcomeHandler.handle`：`completed` / `failed` 都同步 `CraftStore.recordOutcome(...)`。
- `CraftStore.recordOutcome`：同步遍历 `outcomeListeners`。
- `CraftScreen` / `WorkbenchScreen`：打开 screen 时注册 listener；completed listener 在排队刷新前直接写 `flashTicks` 并播放完成音效。
- 反证也已确认：普通 `recipe/session/unlock` 刷新路径最终会走 `scheduleRefresh(...)->client.execute(...)`，所以本 bug 不应被描述为“所有 craft listener 都直接跨线程刷新 Owo UI”。

## 触发路径

1. 玩家打开 `CraftScreen` 或 `WorkbenchScreen`，screen 注册 `CraftStore` outcome listener。
2. 玩家开始一次制作并等待 server 完成。
3. server 通过 `bong:server_data` 推送 `type=craft_outcome`、`kind=completed`。
4. Fabric network thread 调用 receiver，执行 `ROUTER.route(...) -> CraftOutcomeHandler.handle(...) -> CraftStore.recordOutcome(...)`。
5. 当前 screen 的 outcome listener 在 `client.execute` 前执行 `flashTicks = 6` 和 `client.player.playSound(...)`。

## 反方审查记录

- **Round 1 反方**：也许 craft screen listener 已经统一 `client.execute`，不会跨线程改 UI。
  **结论**：部分成立，需收窄。`recipe/session/unlock` 以及最终刷新确实会 `client.execute`；但 `completed` outcome listener 在调度前已经写 `flashTicks` 并调用 `playCompleteSound()`。
- **Round 2 反方**：也许 `playSound` 只是轻量包装，`flashTicks` 只是 int，不足以算真实 bug。
  **结论**：未推翻。`player.playSound(...)` 会触达 player/world/sound state，属于 Fabric 注释禁止的 network thread game state access；`flashTicks` 单独风险较低，但仍是 screen 字段的跨线程写。
- **排重裁决**：不重复 #1016 的 cast_sync 配置窗入口；共享底层 root cause，修复时应一起覆盖 `server_data` handler side effect。

## Skeleton Fix Plan

- P0：把 `bong:server_data` 的 parse/route 副作用边界收紧。推荐方向是 receiver 只读 buffer / decode payload，所有 `ROUTER.route(...)` 与 handler side effect 都在 `client.execute(...)` 内执行；或把 handler 契约改成纯解析 `ServerDataDispatch`，所有 store/UI/sound side effect 统一在主线程 apply。
- P0：为 `CraftScreen` / `WorkbenchScreen` 的 outcome listener 加局部兜底：completed 分支里的 `flashTicks` 和 `playCompleteSound()` 必须与刷新一起排入 `client.execute`，避免修复主路由前继续踩线程。
- P1：审计 `ServerDataHandler.handle()` 中直接写 store、toast、sound、screen 的 handler，确认它们在新线程边界下全部只从主线程执行。

## 验收测试计划

- 新增 client 单测或 harness：模拟非主线程调用 `CraftOutcomeHandler.handle(completed)`，断言 outcome listener 不直接访问 `MinecraftClient.player` / sound，副作用被排入主线程调度器。
- 覆盖 `CraftScreen` 与 `WorkbenchScreen` 两条 completed outcome：完成音效、完成闪光、输出预览刷新都只在 client thread 执行。
- 回归 #1016 同根因：`cast_sync` 到达时不得在 network thread 关闭/刷新功法配置浮窗。
- 手动验证：在手搓台和末法制作台各完成一次制作，确认完成音效和闪光稳定出现，客户端日志无线程相关异常。

## 风险

- 修复 `server_data` 主路由线程边界会影响多个 handler，范围比本 bug 的 craft 入口更大；需要避免把大量 JSON/proto 解析也无脑挪到 render thread 导致卡顿。
- 若只做 Craft/Workbench 局部兜底，#1016 等同根因入口仍可能存在；若做主路由修复，则要检查所有 handler 的测试是否默认同步 side effect。
- 本案风险等级为中等：不是稳定崩溃型，但违反 Fabric 明确线程契约，且命中正常制作完成路径。
