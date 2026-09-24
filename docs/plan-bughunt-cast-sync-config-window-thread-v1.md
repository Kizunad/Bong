# plan-bughunt-cast-sync-config-window-thread-v1（Active）

> BugHunt 线程 C7（client-ui 第七轮）验证记录。历史问题是 `cast_sync` 到达时功法配置窗口可能由 network thread 直接变更 UI；当前 `origin/main@6d7e1699b` 已通过客户端窗口重构消除该路径，本计划只记录主线复验结论并归档。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | 第一性原理验真与线程边界对拍 | fix_pr | ✅ 2026-09-24 |
| P1 | 主线现状确认：server_data route 与窗口刷新均在 client thread | fix_pr | ✅ 2026-09-24 |
| P2 | 回归契约与完整 client 门禁 | fix_pr | ✅ 2026-09-24 |
| P3 | 计划收口、归档与 PR | fix_pr | ✅ 2026-09-24 |

## Bug 摘要

历史版本的 `bong:server_data` receiver 在 network thread 中同步执行 `ServerDataRouter.route()`。当 payload 是 `cast_sync` 且 phase 进入 `casting` 时，`CastSyncHandler` 会同步 `CastStateStore.replace()`，进而调用 `SkillConfigPanelManager` listener，在后台线程直接清理 owo 组件树。

这不是“缺少 cast_sync”的问题，而是“cast_sync 已到达后在错误线程触发 UI mutation”。`CastStateStore` 会 catch `RuntimeException`，所以不应表述为必崩；真实风险是后台线程直接改 UI 造成偶发异常、配置窗无提示消失、组件树竞态或 UI 卡住。当前主线已删除旧 listener 路径，并把 server_data 的 route、handler、store 副作用和窗口刷新全部绑定到 client thread。

## 对实际游玩体验的影响

玩家打开背包的功法页并编辑某招配置时，如果服务端同时推送 `cast_sync(casting)`，配置浮窗可能在非渲染线程被关闭。实际体验是正在编辑的参数窗口突然消失，保存/取消状态不明确；更坏情况下 owo 组件树在后台线程被改动，可能出现 UI 卡顿、输入失焦、日志吞掉异常后界面状态不一致。

## 证据定位

- `client/src/main/java/com/bong/client/BongNetworkHandler.java:dispatchServerDataPayload` 只在接收回调中复制 payload 并调用 `clientExecutor.accept(...)`；`processServerDataPayload` 内才执行 proto bridge、`ServerDataRouter.route()`、handler 和 store 副作用。
- `client/src/test/java/com/bong/client/BongServerDataThreadingTest.java` 的 `routeAndApplyDispatchRunInOneOrderedClientTask` 与 `castSyncStoreListenerWaitsForClientThread` 锁定 route、`CastSyncHandler`、`CastStateStore` listener 均等待 client task 执行。
- `client/src/main/java/com/bong/client/combat/inspect/SkillConfigWindows.java` 已替代 `SkillConfigPanelManager`：它不注册 `CastStateStore` UI listener，而是在 `refresh()` 中按 casting 状态关闭窗口。
- `client/src/main/java/com/bong/client/ui/window/UiWindowRuntime.java` 在 `ClientTickEvents.END_CLIENT_TICK` 调用 `SKILL_CONFIGS.refresh()`，关闭动作因此发生在 client thread。
- 主线提交 `0b6aff68b` 删除 `SkillConfigPanelManager.java` 及其测试，接入统一窗口管理器和线程边界测试。

## 触发路径

1. 历史版本中玩家打开 InspectScreen 的功法配置浮窗。
2. 服务端异步推送 `bong:server_data` / `cast_sync`，phase 为 `casting`。
3. 旧 receiver 会在 network thread 直接跑 `ROUTER.route()` → `CastSyncHandler.handle()` → `CastStateStore.replace()`。
4. 旧 `SkillConfigPanelManager.onCastStateChanged()` 在同一 network thread 触发 `close()`，直接修改 owo `FlowLayout host`。
5. 今日主线先在 `clientExecutor` 中处理完整 route 链，再由 `UiWindowRuntime` 的 client tick 调用 `SkillConfigWindows.refresh()`，旧路径已不存在。

注意：不要把“在 InspectScreen 内按 1-9 技能键”作为主复现路径；`MixinKeyboardSkillKeys` 在 `currentScreen != null` 时会 return。本 bug 的主触发是服务端异步 `cast_sync(casting)` 到达。

## 反方审查记录

### Round 1

反方结论：PASS。未能推翻候选。关键意见：

- 历史 Fabric receiver 的 handler 在 network thread，旧 `ROUTER.route()` 确实发生在任何 `client.execute` 之前。
- 旧 `SkillConfigPanelManager` listener 曾在真实 UI 路径注册并直接改 `host`；该类已由主线提交 `0b6aff68b` 删除。
- 当前 `BongServerDataThreadingTest` 证明 route 与 store listener 在 client task 执行。
- 需收窄措辞：不是所有 `cast_sync` 都触发，只有进入 `isCasting()` 的状态会关窗；`CastStateStore` catch runtime exception，所以不能写“必崩”。

### Round 2

反方结论：PASS。补充收窄：

- 不重复 #987：#987 是“缺配置时 server 不推 cast_sync，client 本地施法条无人纠偏”；本问题是“cast_sync 已到达后，handler 同步触发 UI listener”。
- 历史 InspectScreen → TechniquesTabPanel → SkillConfigPanelManager 路径已被统一窗口工作台替换，今日对应入口是 `UiWindowRuntime.openSkillConfig()`。
- 验收必须证明 `cast_sync` handler/store listener 不在 network callback 直接 mutate UI；当前线程测试已满足这一契约。

## 验证结论

- 旧本地提交 `0ae073a92` 的局部 executor 注入方案不再移植：目标 `SkillConfigPanelManager` 已在主线窗口重构中删除。
- 今日主线采用更完整的方案：`BongNetworkHandler` 把 bridge、route、handler、store side effect 一起排入一个 client task；窗口层由 `SkillConfigWindows.refresh()` 在 client tick 关闭 casting 状态下不可用的窗口。
- 语义保持不变：只有 `CastState.isCasting()` 时配置窗口不可用，`idle`、`complete`、`interrupt` 不会错误提交配置；异常捕获不是线程边界验收依据。

## 验收证据

- `BongServerDataThreadingTest.routeAndApplyDispatchRunInOneOrderedClientTask`：network callback 返回前不执行 route 或 handler，client task 内按序执行 route → apply。
- `BongServerDataThreadingTest.castSyncStoreListenerWaitsForClientThread`：`cast_sync` 在 client task 执行前不替换 `CastStateStore`，执行后 listener 线程为 client thread。
- `SkillConfigWindowsTest.castingAndRemovedTechniqueRejectSaveBeforeTheNextRefresh`：casting 时保存被拒绝，下一次 `refresh()` 关闭窗口；移除功法后同样不会提交旧配置。
- JDK 17 client 完整门禁 `scripts/build-token.sh gradle test build`：3 个 required GameTests 通过，Gradle 输出 `BUILD SUCCESSFUL`。

## 风险

- 当前 `server_data` 全链路 client-task 调度改变了历史 handler 的执行线程，但 `BongServerDataThreadingTest` 已覆盖顺序、单次投递和后续 payload 继续处理。
- `SkillConfigWindows.refresh()` 依赖 client tick，窗口关闭最多延迟一个 tick；这是可观察且安全的 UI 生命周期语义，不应恢复 network-thread 直接清理组件树。
- `CastStateStore` 仍捕获 listener `RuntimeException`，因此回归必须继续断言线程与窗口状态，不能只依赖“没有 crash”。

## 遗留 / 后续

- 本 bug 修复最初于 2026-07-18 在本地完成，旧 claim/PR 流程中断后未推送；本次确认主线已在 2026-09-20 的 `0b6aff68b` 重构中先行解决。
- 旧本地分支 `backup/plan-bughunt-cast-sync-config-window-thread-v1-local-20260924` 保留四个提交，待 PR 合并后由主干统一清理。

## Finish Evidence

### 落地清单

- `client/src/main/java/com/bong/client/BongNetworkHandler.java`：`server_data` payload 统一排入单一 client task，route 与 handler 不在 network callback 中执行。
- `client/src/main/java/com/bong/client/combat/inspect/SkillConfigWindows.java`：配置窗口通过 client-tick `refresh()` 感知 casting 状态，不从 `CastStateStore` 回调直接修改 UI。
- `client/src/main/java/com/bong/client/ui/window/UiWindowRuntime.java`：`ClientTickEvents.END_CLIENT_TICK` 调度配置窗口刷新。
- `client/src/test/java/com/bong/client/BongServerDataThreadingTest.java`、`client/src/test/java/com/bong/client/combat/inspect/SkillConfigWindowsTest.java`：线程与窗口生命周期契约已在主线测试覆盖。
- `docs/finished_plans/plan-bughunt-cast-sync-config-window-thread-v1.md`：本计划归档并记录主线已修复结论。

### 第一性原理验真

- `origin/main@6d7e1699b` 不再包含 `SkillConfigPanelManager`；主线提交 `0b6aff68b` 删除旧 UI listener，并由统一窗口工作台负责配置窗口生命周期。
- `receiveServerDataPayload` 只复制 buffer 并排队；`processServerDataPayload` 在 client task 内完成 bridge、route、`CastSyncHandler` 和 `CastStateStore.replace`，因此历史 network-thread mutation 已被主线修复。

### 关键 commit

- `0ae073a92`（2026-07-18）：旧本地局部线程修复，因目标类已删除而未复用。
- `0b6aff68b`（2026-09-20）：主线客户端窗口重构，删除 `SkillConfigPanelManager`，接入统一 `SkillConfigWindows` 与 client-tick 刷新。
- `6d7e1699b`（2026-09-24）：本次验证使用的最新主线基线。

### 测试结果

- `JAVA_HOME=/home/serverkizuna/opt/jdk-17.0.19+10 PATH=/home/serverkizuna/opt/jdk-17.0.19+10/bin:$PATH scripts/build-token.sh gradle test build`：通过。
- GameTest：3 个 required tests 全部通过。
- Gradle：`BUILD SUCCESSFUL`，无失败任务。

### 跨仓库核验

- client：`server_data → client task → route → CastSyncHandler → CastStateStore` 链路及 `SkillConfigWindows.refresh()` 均在 client thread。
- server / schema / agent：本计划未改动，`cast_sync` payload 契约保持不变。

### 遗留 / 后续

- 本修复最初于 2026-07-18 在本地完成，本次确认主线已先行解决，并按 docs-only 方式收口。
- 旧本地备份分支按任务卡保留，待 PR 合并后由主干处理；本次新增提交均使用 `Model: gpt-6-luna`。
