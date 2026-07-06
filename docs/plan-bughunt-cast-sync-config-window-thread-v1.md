# BugHunt: cast_sync 网络线程关闭功法配置浮窗

> BugHunt 线程 C7（client-ui 第七轮）skeleton-only 记录。范围限定 `client/src/main/java/com/bong/client/` 的 UI / HUD / input / session 路径；本文件只固化问题、证据、修复抓手与验收计划，不消费、不归档 plan，不修改实际代码。

## Bug 摘要

`bong:server_data` 的 legacy Fabric receiver 在 network thread 中同步执行 `ServerDataRouter.route()`。当 payload 是 `cast_sync` 且 phase 进入 `casting` 时，`CastSyncHandler` 会同步 `CastStateStore.replace()`，进而同步调用 `SkillConfigPanelManager` 注册的 listener。该 listener 在后台线程直接 `close()` 功法配置浮窗并执行 `host.clearChildren()`，把 owo UI 组件树修改放到了 render thread 之外。

这不是“缺少 cast_sync”的问题，而是“cast_sync 已到达后在错误线程触发 UI mutation”。`CastStateStore` 会 catch `RuntimeException`，所以不应表述为必崩；真实风险是后台线程直接改 UI 造成偶发异常、配置窗无提示消失、组件树竞态或 UI 卡住。

## 对实际游玩体验的影响

玩家打开背包的功法页并编辑某招配置时，如果服务端同时推送 `cast_sync(casting)`，配置浮窗可能在非渲染线程被关闭。实际体验是正在编辑的参数窗口突然消失，保存/取消状态不明确；更坏情况下 owo 组件树在后台线程被改动，可能出现 UI 卡顿、输入失焦、日志吞掉异常后界面状态不一致。

## 证据定位

- `client/src/main/java/com/bong/client/BongNetworkHandler.java:245` 使用 `ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "server_data"), ...)` legacy overload。Fabric 1.20.1 networking API 源码 javadoc 明确该 handler 在 network thread 运行，访问 game state 需用 `client.execute(...)` 回 render thread。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:267` 在 receiver 回调里同步 `ROUTER.route(jsonPayload, readableBytes)`；`client.execute(...)` 只包后续 `applyDispatch` 分支（同文件 `:286-298`），不包 route / handler。
- `client/src/main/java/com/bong/client/network/ServerDataRouter.java:158` 注册 `cast_sync` handler；`ServerDataRouter.route()` 在 `:314-315` 直接同步 `handler.handle(envelope)`。
- `client/src/main/java/com/bong/client/network/CastSyncHandler.java:60` 同步调用 `CastStateStore.replace(next)`。
- `client/src/main/java/com/bong/client/combat/CastStateStore.java:98-104` 同步遍历 listeners 并在调用线程执行。
- `client/src/main/java/com/bong/client/inventory/InspectScreen.java:566-570` 默认构造功法 tab / `TechniquesTabPanel`，不是懒加载。
- `client/src/main/java/com/bong/client/combat/inspect/TechniquesTabPanel.java:81-83` 构造 `SkillConfigPanelManager`。
- `client/src/main/java/com/bong/client/combat/inspect/SkillConfigPanelManager.java:48` 注册 `CastStateStore` listener；`:109-110` 在 `state.isCasting()` 时 `close()`；`:81-84` 的 `close()` 直接 `host.clearChildren()`。
- 同类正确写法对照：`client/src/main/java/com/bong/client/inventory/InspectScreen.java:556-560` 对 `SkillSetStore` listener 明确注释“避免网络线程 mutate owo-lib 组件”，并通过 `MinecraftClient.getInstance().execute(...)` 回主线程刷新 UI。

## 触发路径

1. 玩家打开 InspectScreen，进入功法 tab。
2. 选中有 schema 的招式，点击 `TechniqueDetailCard` 的“配置”齿轮；`SkillConfigPanelManager.open()` 将配置浮窗挂到 `configLayer`。
3. 服务端异步推送 `bong:server_data` / `cast_sync`，phase 为 `casting`。
4. network thread 中的 receiver 直接跑 `ROUTER.route()` → `CastSyncHandler.handle()` → `CastStateStore.replace()`。
5. `SkillConfigPanelManager.onCastStateChanged()` 在同一 network thread 触发 `close()`，直接修改 owo `FlowLayout host`。

注意：不要把“在 InspectScreen 内按 1-9 技能键”作为主复现路径；`MixinKeyboardSkillKeys` 在 `currentScreen != null` 时会 return。本 bug 的主触发是服务端异步 `cast_sync(casting)` 到达。

## 反方审查记录

### Round 1

反方结论：PASS。未能推翻候选。关键意见：

- Fabric legacy `registerGlobalReceiver(Identifier, PlayChannelHandler)` 的 handler 在 network thread。
- `ROUTER.route()` 确实发生在任何 `client.execute` 之前。
- `SkillConfigPanelManager` 的 listener 在真实 UI 路径会注册，且 `close()` 直接改 `host`。
- 需收窄措辞：不是所有 `cast_sync` 都触发，只有进入 `isCasting()` 的状态会关窗；`CastStateStore` catch runtime exception，所以不能写“必崩”。

### Round 2

反方结论：PASS。补充收窄：

- 不重复 #987：#987 是“缺配置时 server 不推 cast_sync，client 本地施法条无人纠偏”；本问题是“cast_sync 已到达后，handler 同步触发 UI listener”。
- InspectScreen → TechniquesTabPanel → SkillConfigPanelManager 路径可达。
- 配置齿轮可打开，但本地 1-9 技能键在 screen 打开时被挡住；主触发必须写服务端异步推送。
- 验收必须证明 `cast_sync` handler/store listener 不在调用线程直接 mutate UI；只 catch/log 异常不算修复。

## Skeleton Fix Plan

- [ ] 选择修复边界：
  - 方案 A：在 `BongNetworkHandler.registerServerDataChannel()` 中把 `ROUTER.route(...)` 以及 handler 副作用整体投递到 `client.execute(...)`；需审计解析/日志/连接状态是否仍保持预期。
  - 方案 B：在 `SkillConfigPanelManager` 的 `CastStateStore` listener 内做主线程 marshal，只把 `close()` 放到 render thread；局部风险小，但不能修复其它未来 listener 的同类问题。
- [ ] 保持语义：`state.isCasting()` 才关闭配置窗；`idle` / `complete` / `interrupt` 不应关闭。
- [ ] 不用吞异常代替修复；异常 catch 只能防崩，不能解决后台线程改 UI。
- [ ] 对照 `InspectScreen` 的 `SkillSetStore` listener 主线程转发模式，建立统一 helper 或局部封装，避免重复踩线程边界。

## 验收测试计划

- [ ] 单测：构造 `SkillConfigPanelManager` + fake `FlowLayout`/host 记录器，从 fake network thread 触发 `CastStateStore.replace(CASTING)`，断言 `host.clearChildren()` 不在调用线程同步执行，而是投递到 main-thread executor。
- [ ] 单测：`CastStateStore.replace(IDLE/COMPLETE/INTERRUPT)` 不关闭已打开配置窗。
- [ ] 路由级测试（若选方案 A）：`bong:server_data` 的 `cast_sync` handler 副作用在 `client.execute` 回调中发生，`ROUTER.route` 不在 network callback 中直接触发 UI listener。
- [ ] 回归：`SkillConfigPanelManagerTest.selectionChangeAndCastStartCloseWindow` 仍通过，即主线程 cast start 会关闭窗口。
- [ ] 手动验证（JDK 17，client 栈）：打开 InspectScreen → 功法 tab → 配置浮窗，模拟/触发服务端推 `cast_sync(casting)`，确认窗口关闭不产生日志异常、UI 不失焦卡住。

## 风险

- 若在网络层整体迁移到 `client.execute`，可能改变部分纯 store handler 的到达顺序与日志线程，需要审计 `ServerDataRouter` 下所有 handler 的线程假设。
- 若只在 `SkillConfigPanelManager` 局部修，风险小但同类 listener 仍可能存在，需至少 grep `CastStateStore.addListener` / `InventoryStateStore.addListener` / `SkillSetStore.addListener` 的 UI mutation 模式。
- `CastStateStore` 当前 catch `RuntimeException` 会隐藏部分异常症状，修复验证不能只依赖“没有 crash”；必须断言线程投递行为。
