# plan-refactor-client-ui-base-v1 — Client UI 可替换库边界 + Screen/Store/Intent/Bootstrap 分层（重构轨 R7）

> 所属总纲：`docs/plans-skeleton/plan-refactor-master-v1.md`。一句话：在不改变 server/schema/wire 行为的前提下，把 client UI 的状态读取、用户意图、屏幕生命周期、列表协调和 bootstrap 注册抽成库无关契约，再由 owo、vanilla 与 MCEF-compatible browser 三条 adapter 实现；以 29 个 Screen 和 `InspectScreen` 为迁移对象，为后续替换 owo-lib 提供单一切换边界。

## Integration Preflight（2026-08-25）

按 `docs/CLAUDE.md:7-21` 的防孤岛流程复核后再更新本 plan：

- **正典**：已检查 `docs/worldview.md:1-35`；本计划只做 client UI 基础设施，不新增境界、经济、世界事件或真元/灵气公式，因此不改 worldview，也不创建新的 worldview 锚点。
- **已完成计划**：已检索 `docs/finished_plans/`，重点对照 `plan-client.md`、`plan-HUD-v1.md`、`plan-alchemy-client-v1.md`、`plan-agent-ui-data-v1.md`、`plan-agent-ui-close-reason-drop-v1.md` 及相关 client/session/UI 结论；R7 只抽取现有 Store、HUD、agent UI close 和 client screen 的外部行为，不重新拥有这些 domain。
- **进行中计划**：已枚举 `docs/plan-*.md`，重点核对 `plan-refactor-client-store-lifecycle-v1.md`、`plan-refactor-wire-s2c-v1.md`、`plan-client-login-ux-v1.md` 以及 alchemy/forge/lingtian session UI bugfix plans；R7 将 Store 断线清理留给 R2、bridge/router 留给 R6，不改其 owner 文件。
- **骨架与 reminder**：已检查 `docs/plans-skeleton/plan-refactor-master-v1.md`、`docs/plans-skeleton/reminder.md:1-28` 和全部 UI/client 相关 skeleton；没有同名的 R7 child skeleton，也没有 reminder 条目要求另建 UI contract。master skeleton 保留计划族的 Wave、ownership、headless 总约束；本文件作为 R7 active child 只负责 client UI contract、adapter、Screen/HUD/keybind 和 browser/viewport seam，拆分理由是避免把 9 条重构轨道的跨轨裁决与单轨实施细节混在同一份可消费 plan 中。

## 0. 改写目的与不可变范围

旧版 R7 以 `BongScreenBase extends BaseOwoScreen` 为公共基类，能改善当前 owo 屏幕，但不能作为未来 UI 库迁移边界。本版将 R7 从“owo UI 重构”改为“UI contract-first + adapter migration”计划：

1. **库无关核心**：状态读取、订阅生命周期、列表 diff/reconcile、intent dispatch、Screen open policy、bootstrap module contract 不得 import owo、Fabric widget 或 vanilla drawable。
2. **三适配路径**：现有 15 个 owo Screen 和 14 个 vanilla Screen 都必须有明确的 adapter/lifecycle 归属；MCEF-compatible browser 作为可选第三宿主，CinemaMod 只作为 1.20.1/JCEF 参考实现，不把 vanilla Screen 留在第二套隐式生命周期里。
3. **协议不变**：不修改 server、TypeBox shape、protobuf envelope、Redis key、`ClientRequestProtocol` 编码或 `bong:server_data`/`bong:client_request` channel。现有 sender/handler 行为只通过 adapter 复用。
4. **所有权不变**：R2 仍独占 Store 断线清理，R6 仍独占网络 receiver/bridge/router，R9 仍独占 cast domain；R7 只消费它们冻结的外部契约。
5. **无大爆炸重写**：先落 contract、fake、source gate、1.20.1 browser compatibility spike 和一条 owo/一条 vanilla 垂直切片，再批量迁移；不得先同时改 29 个 Screen、109 个 Store 和 80 个 Handler。

## 1. 基线证据（2026-08-24 复核）

- client production Java 文件约 **1022** 个；真实 Screen **29** 个：15 个直接继承 owo `BaseOwoScreen`，14 个直接继承 vanilla `Screen`。`TechniqueScrollReadScreen` 是 helper，`LegacyAssignPanel.java` 是无 `Screen.java` 后缀的真实 Screen。逐文件基线：`client/src/test/resources/bong/ui/r7-screen-inventory.tsv`。
- 15 个 owo Screen 当前直接依赖 `BaseOwoScreen`/`FlowLayout`/owo `Component`；14 个 vanilla Screen 直接依赖 `Screen`/`addDrawableChild`。因此 `BongScreenBase<R extends ParentComponent>` 和 `DiffListWidget<T,K,C extends Component>` 只能是 adapter 实现，不能继续作为跨库公共契约。
- `InspectScreen.java` 约 4647 行，同时持有 tab 组合、Store snapshot/listener intake、drag/drop、context menu、tooltip、hotbar 和 overlay arbitration；它必须保持唯一 screen-level intake，tab panel 不得重复订阅同一 Store。
- production `*Store.java` 共 **109** 个（R2 的 108 个业务 Store 加 lifecycle infrastructure 口径）；R2 的 `SessionScopedStore` 只有 `clearOnDisconnect()`，不是 UI subscription contract。现有 listener/remove-listener 形态不统一，必须通过 R7 read adapter 归一，不能假定所有 Store 已有同一接口。
- `client/network` Handler 约 **80** 个；主路径为 `ProtoServerDataBridge -> ServerDataRouter -> domain Handler -> Store/HUD/Screen`。Handler 负责解析和写入 domain state，UI 不得反向依赖 Handler。
- `ClientRequestSender` 现有大量静态 `sendXxx` 入口（审计口径 117 个 public sending entry），同时存在 `void`、本地 transport `boolean`、tracked `request_id` 和 generic JSON 入口。UI 不能把该类当库无关 API；R7 只在其上提供 typed intent adapter。
- `BongClient.onInitializeClient()` 以显式顺序注册 network、HUD、keybind、Screen bootstrap、render/audio 等模块；R7 只收编 UI/HUD/keybind 子集，不改变 `BongNetworkHandler.register()` 的 channel 注册顺序。
- owo `Sizing.fill(100).inflate(space, ...)` 返回完整 `space`，不是同轴兄弟的剩余空间。现有 92 个 token（87 个 executable）以及 15 个 executable `clearChildren()` 站点已由 `r7-fill100-inventory.tsv` 锁定，不能机械全量替换。

## 2. 目标架构与稳定数据流

目标态只允许以下依赖方向：

```text
server/protobuf/JSON
  -> BongNetworkHandler / ProtoServerDataBridge / ServerDataRouter (R6)
  -> domain Handler
  -> domain Store + immutable Snapshot/ViewModel (R2 owns lifecycle)
  -> semantic UiSurfaceProjection / UiStateSource / UiViewModelAdapter (R7)
  -> UiScreenController + local template registry
  -> owo adapter OR vanilla adapter OR MCEF-compatible browser adapter

UI input
  -> typed UiIntentSink (R7)
  -> existing ClientRequestSender / ClientRequestProtocol
  -> bong:client_request
```

禁止反向依赖：

- Screen、widget、HUD renderer 不解析 `ServerDataEnvelope`、protobuf 或 JSON。
- Screen、widget、HUD renderer 不直接调用 `*ServerDataHandler`、`ServerDataRouter` 或 `ProtoServerDataBridge`。
- UI adapter 不直接操作 Store 的静态业务字段；只能消费 `UiStateSource.snapshot()` 和订阅信号。
- UI 不把“本地 transport 已接受”当作 server gameplay 成功；权威结果仍来自 S2C state/receipt。
- network Handler 不直接构造 library-specific widget；需要打开 UI 时只提交 domain offer/state，由 bootstrap/transition owner 决策。
- server/agent 只拥有语义 surface、immutable view data 和 allowed action；client 只拥有本地模板、布局和交互呈现。任何 HTML/XML/JS/DOM/像素坐标不得跨这条边界。
- bot/headless client 消费同一 `surface_id`、`template_id`、`action_id`、参数校验和 authoritative receipt；它跳过模板、渲染和物理输入，但不能走另一套“测试专用”业务接口。

### 2.1 CinemaMod/MCEF 事实与兼容性决策门

本计划把“CinemaMod/MCEF”拆成两个不同层次，不能把项目名直接当成 UI framework：

- `CinemaMod/CinemaMod` 的 Fabric 1.20.1 分支（commit `ca334ca557a0c241671b9738ffd0e9475610aa83`）是视频影院 mod，不是通用 UI toolkit。它在 `fabric/cef/**` 自带 JCEF，`CefBrowserCinemaRenderer` 把 off-screen browser 的 BGRA buffer 上传成 OpenGL texture，`VideoRequestBrowser` 手工转发鼠标/键盘事件，`CefRenderMixin` 每帧驱动 CEF message loop，`CefInitMixin` 负责 native 解包和 `jcef.path`。其 README 仍把“convert CinemaMod to use MCEF 2”列为未完成 blocker。
- `DimasKama/mcef-modern` 是独立的 MCEF API。当前文档化入口是异步 `MCEFApi.initialize()`、`MCEFBrowser.resize/setFocus/onMouse*/onKey*/getTexture*/close()`；但当前公开 Maven metadata 只有 MC `1.21.10`、`1.21.11`、`26.1`、`26.2` 版本，没有 MC 1.20.1 artifact。当前 API 还直接引用新版本 Minecraft input/GPU 类型，不能未经 port/compatibility release 编译进本 client。
- 因此 R7 **不**直接依赖 CinemaMod 的内部 `com.cinemamod.fabric.cef.*`，不复制或 Jar-in-jar CEF native，不把当前 MCEF Modern 版本号写入 `client/gradle.properties`。MCEF/CinemaMod 只能作为运行时可选 capability；1.20.1 provider、平台 native、Linux/Windows/macOS、Java 17、GPU context、窗口 resize、输入转发和 client shutdown 全部通过 compatibility gate 后，才允许进入生产 adapter。

目标 browser 数据流：

```text
UiScreenController + immutable ViewModel
  -> McefBrowserHost (Screen lifecycle owner)
  -> local packaged HTML/CSS/JS document
  -> typed JS intent bridge
  -> UiIntentSink -> existing ClientRequestSender
```

browser 宿主的约束：

- 首期只加载 `assets/bong/ui/**` 的本地打包页面或受控 resource scheme；不允许服务端下发任意 URL、HTML、JavaScript，也不允许把 raw `owo-ui` XML 直接喂给 Chromium。
- Java → browser 只推送序列化后的 immutable ViewModel/snapshot；browser → Java 只接受 registry 中的 `intent_id` 和经过 schema/参数校验的 typed intent，不开放任意 Java method 或 sender 调用。
- MCEF 初始化必须异步且可取消/失败；UI 不阻塞 client thread 等待 native 下载。未就绪或初始化失败时由 `UiBackendRegistry` 按策略 fallback 到 vanilla/owo，并记录 capability 状态。
- `McefBrowserHost` 独占 browser、texture/view、focus、mouse/key/char forwarding、resize 和 close；controller、Store、Handler 不 import MCEF/JCEF。1.20.1 OpenGL texture 与后续 MCEF GPU texture view 的差异只存在于 adapter 内。
- 页面、JS bridge、native browser、Screen 被移除时必须 exactly-once close；late callback、late JS intent、旧 request_id 一律 fail closed。native resource 泄漏、后台 CEF message-loop、跨线程 DOM 操作和任意导航都属于 P0 blocker。

审计来源：CinemaMod 1.20.1 代码 [`CinemaMod/CinemaMod@ca334ca`](https://github.com/CinemaMod/CinemaMod/tree/ca334ca557a0c241671b9738ffd0e9475610aa83/fabric)；MCEF Modern API [`DimasKama/mcef-modern`](https://github.com/DimasKama/mcef-modern)；MCEF Modern 当前 Maven metadata [`net.dimaskama:mcef-modern`](https://maven.dimaskama.net/releases/net/dimaskama/mcef-modern/maven-metadata.xml)。

## 3. 接入面与跨轨所有权

### 3.1 进料

- **状态**：R2 管理的 session Store、persistent config Store、HUD state Store 的 immutable snapshot；R7 通过 read adapter 暴露给 UI。
- **网络**：R6 的 `ProtoServerDataBridge`、`ServerDataRouter`、`ServerDataHandler` 已完成 client-thread receive boundary；R7 不重新 marshal 网络 receiver。
- **输入**：现有 keybinding、vanilla input、Screen widget callback、HUD/overlay gesture；均转换成 typed intent。
- **启动**：`BongClient.onInitializeClient()` 的 UI/HUD/keybind bootstrap call set；网络、render、audio、debug module 不被 R7 统一吞并。
- **worldview / qi_physics**：纯 client 基础设施，不新增玩法、境界、经济或真元/灵气公式；不调用、不修改 `qi_physics`，不改变任何守恒 ledger。

### 3.2 出料

- UI adapter 的渲染树、HUD render command、Screen transition decision。
- typed `UiIntentSink` 到既有 `ClientRequestSender`；wire payload 和 server ACK 语义不改变。
- subscription/cleanup 的 library-neutral lifecycle evidence。

### 3.3 跨仓库契约（只消费，不改形状）

- **server**：R7 只消费 server 现有 `ServerDataV1` producer、`server/src/network/client_request_handler.rs` 的 C2S request handling 和各 domain authoritative receipt/state；不新增 server endpoint。
- **schema/agent**：TypeBox `ClientRequestV1` / `ServerDataV1` 及其 JSON samples 是现有 wire source of truth；R7 不新增 union member、字段或 Redis key，不把 UI-only ViewModel 写回 schema。
- **client**：`ClientRequestProtocol` → `ClientRequestSender` 是唯一既有 C2S transport；`ProtoServerDataBridge` → `ServerDataRouter` → `ServerDataHandler` 是唯一 S2C receive path；R7 只在两端外层加 adapter/source gate。
- **验收**：UI intent 的 encoded payload 与现有 C2S sender tests 对拍，Store/ViewModel 的 authoritative update 与现有 S2C handler tests 对拍；任何 adapter replacement 都不能要求 server/schema/agent 改形状。

### 3.4 所有权矩阵

| 范围 | owner | R7 规则 |
|---|---|---|
| Store 业务字段、S2C hydration、断线清理 | R2 / domain owner | R7 不改字段语义；只写 read adapter 和 UI view-model mapping。 |
| `BongNetworkHandler.register()`、receiver、bridge、router | R6 | R7 不改 channel registration；Insight `offer_id` 只按窄接缝协调。 |
| `ClientRequestProtocol` 编码与 server request handler | 既有 network/C2S owner | R7 不改编码；intent adapter 复用 sender。 |
| `client/ui/contract/**`、UI adapters、Screen/HUD/keybind 结构 | R7 | 新增库无关契约和 owo/vanilla/MCEF browser 实现；MCEF native/provider 依赖需先过 compatibility gate。 |
| cast domain reducer/store/AV semantics | R9 | R7 只消费 view-model/intent contract。 |
| `BongClient` UI/HUD/keybind bootstrap call set | R7 | 只迁 UI 子集到显式 registry，保留 network/render/audio order。 |
| server、agent、worldgen、worldview、qi ledger | 其他 owner | R7 不触碰。 |

## 4. 库无关公共契约（P0 必须冻结）

库无关类型按职责放在 `client/src/main/java/com/bong/client/ui/{contract,state,intent,bootstrap}/`；这些包的源码不得出现 owo、`BaseOwoScreen`、`FlowLayout`、`net.minecraft.client.gui.widget`、MCEF/JCEF 或具体 UI library import。adapter 只能出现在 `client/ui/adapter/{owo,vanilla,mcef}/`。

### 4.1 `UiStateSource<S>`、`UiSubscription` 与 source mode

```java
public interface UiStateSource<S> {
    S snapshot();
    UiSubscription subscribe(Consumer<? super S> listener);
}

public interface UiSubscription extends AutoCloseable {
    @Override void close();
    boolean isClosed();
}
```

source mode 是 adapter inventory 的显式字段，而不是隐藏实现：`PUSH` 由 Store listener 驱动，`PULL_ON_OPEN` 只在 Screen open 读取一次，`PULL_ON_TICK` 只能由已登记的 client tick owner 驱动。迁移 UI 库不能改变 source mode。

冻结规则：

- `snapshot()` 是当前唯一可读状态；不得暴露可变内部集合。
- `subscribe()` 只承诺后续变化信号，不承诺立即回调；绑定器先读 snapshot，再订阅，避免重复首帧。
- `close()` exactly-once、幂等；关闭后不得再收到回调。listener 异常不得破坏 Store 状态更新，异常传播策略由 adapter 测试固定。
- 注册、回调、关闭均在 client thread 完成；R6 receive-boundary 负责网络线程切换，R7 不在 network 文件叠加第二层 marshal。
- 该接口不等于 `SessionScopedStore`；断线清理仍由 R2 registry 调用 `clearOnDisconnect()`。

R7 提供 `StoreUiStateSource<S>` wrapper，把现有静态 Store 的 `snapshot`/listener/remove-listener 适配为上述形状。P0R 对每个被 UI 消费的 source 登记 `PUSH`、`PULL_ON_OPEN` 或 `PULL_ON_TICK` 模式；没有 listener 的 Store 只能使用显式 client-thread invalidation signal 或登记的本地 tick refresh，不在 UI 层轮询网络或直接读内部字段。

### 4.2 `UiScreenScope` 与生命周期

```java
public interface UiScreenScope {
    void onOpen();
    void addCleanup(Runnable cleanup);
    void onTick(long nowMs);
    void close();
    boolean isClosed();
}
```

实现契约：

- cleanup 按登记顺序确定、关闭时 LIFO、exactly-once；`close()` 先标记 closed，再执行 cleanup。
- cleanup、business `onRemoved`、library host teardown 任一步抛异常时，后续阶段仍执行；首个异常为 primary，后续按顺序 `addSuppressed`。
- `close()` 只关闭该 Screen 的订阅、tick、input handle 和 pending UI callback；绝不调用 R2 `clearOnDisconnect()`。
- late refresh、late intent、重复 `removed()` 均 fail-closed/no-op，不重新挂载已关闭 Screen。

### 4.3 `UiScreenController` 与 adapters

库无关 controller 只依赖 immutable `ViewModel`、`UiStateSource` 和一个按领域参数化的 typed `UiIntentSink`；controller 不接收裸 `UiIntent`，adapter 只能通过该 sink 发送允许的动作：

```java
public interface UiIntent {}

public interface UiScreenController<M, I extends UiIntent> {
    M viewModel();
    UiIntentSink<I> intentSink();
    void onOpen(UiScreenScope scope);
    void onClose();
}
```

`UiScreenController<M, I>` 的 `I` 必须与 `UiIntentSink<I>` 一致；adapter 不得把 `UiIntent` 向下转型或绕过 sink 调用 sender。需要多个动作域的 Screen 组合多个窄 sink/controller，而不是退回一个 117 方法的总接口。

具体 UI library 只实现 host/adapter，不定义业务规则：

- `client/ui/adapter/owo/OwoScreenHost`：兼容 `BaseOwoScreen`、`OwoUIAdapter`、`FlowLayout` 和动态 XML；只负责把 controller/view-model 映射到 owo component tree。
- `client/ui/adapter/vanilla/VanillaScreenHost`：兼容 vanilla `Screen`、`Drawable`、`ClickableWidget`；只负责布局、输入和绘制。
- `client/ui/adapter/mcef/McefBrowserHost`：兼容 1.20.1 provider 的 browser lifecycle、local HTML document、texture presentation 和 Minecraft 输入转发；只负责 browser host，不负责 Store、wire、sender 或业务规则。CinemaMod 的旧 `CefBrowserCinema` 只能作为 compatibility spike 的参考实现，不能成为 R7 公共 API。
- `BongScreenBase` 若保留，只能是 `OwoScreenHost` 的兼容实现，不得被写入 `ui/contract` 或作为新 Screen 的业务依赖。
- `DynamicXmlScreen` 继续属于 owo adapter；模板白名单和 XML 安全策略不迁入 library-neutral contract。

browser adapter 不等于 library-neutral contract：HTML/CSS/JS 组件、DOM id、JS bridge message shape、CEF texture 类型和 MCEF lifecycle 均锁在 `ui/adapter/mcef/**`。若后续要让同一动态 UI 同时渲染到 owo/vanilla/browser，必须另立 neutral UI AST；不得把 owo XML 或 HTML DOM 偷渡成公共 wire schema。

### 4.4 `UiListReconciler<T,K>`

列表核心只处理 key、顺序、patch 和 detached rebuild，不持有 owo/vanilla component：

- equal key + equal order：只 patch，不替换 mounted row。
- reorder/add/remove：先 detached 创建完整 replacement，全部成功后整体 swap；失败保留旧 committed state。
- null list/item/key、duplicate key：mutation 前 fail-fast。
- patch 异常不回滚外部 row mutation，但内部 committed sequence 保持上一版本，下一次从第一行完整重试；patcher 必须幂等。
- 组件 identity、selection、callback、scroll 保留语义由 adapter 验证；scroll offset 不通过不存在的 owo 0.11.2 API 伪造。

`OwoDiffListWidget`、`VanillaDiffListWidget` 只实现 renderer bridge；原 `DiffListWidget<T,K,C extends Component>` 不再是公共 API。

### 4.5 `UiIntentSink` 与发送语义

不新建一个包含 117 个方法的巨型接口。按领域定义小型 typed sink，例如：

- `InventoryIntentSink`：move/equip/discard/pickup，携带 immutable `instance_id`/location。
- `CraftIntentSink`：start/cancel/quantity，复用 `CraftStore` 的 accepted identity。
- `AlchemyIntentSink`、`ForgeIntentSink`、`InsightIntentSink`、`SocialIntentSink`：按现有 sender/protocol 语义分域。

最小公共形状为：

```java
public interface UiIntentSink<I extends UiIntent> {
    UiIntentResult dispatch(I intent);
}
```

`UiIntentResult` 只表达本地 transport，不表达 server 业务结果：

```java
public record UiIntentResult(Kind kind, String reason, String requestId) {
    public enum Kind { LOCAL_ACCEPTED, LOCAL_REJECTED, LOCAL_ERROR }
}
```

`reason` 和 `requestId` 可为空的组合由各 domain sink contract pin；`LOCAL_ACCEPTED` 不得被命名为 accepted-by-server。

每个 sink 的实现只负责把 intent 映射到既有 `ClientRequestSender`；返回值统一表达**本地 transport**结果：

```text
LOCAL_ACCEPTED { optional request_id }
LOCAL_REJECTED { reason }
LOCAL_ERROR { reason }
```

不得把 `LOCAL_ACCEPTED` 命名为 server accepted；server 成功/拒绝仍由 S2C authoritative state/receipt 回写 Store。tracked request 必须保留 request id，void sender 不被伪装成有 ACK。

### 4.6 `UiBootstrapModule` 与 `UiBootstrapRegistry`

```java
public interface UiRuntime {}

public interface UiBootstrapModule {
    String id();
    Set<String> dependencies();
    void register(UiRuntime runtime);
}
```

规则：

- registry 显式静态列出 UI/HUD/keybind module；禁止 reflection/annotation discovery/构造器自注册。
- `id` 全局唯一、依赖必须存在且无环；拓扑排序结果可观察并由测试 pin。
- `registerAll()` exactly-once；重复调用不重复注册 Fabric callback、KeyBinding、HudRenderCallback 或 tick。
- `BongNetworkHandler.register()` 在 registry 之前完成；`ScreenTransitionController` 先于 Screen bootstrap；HUD callback 只保留现有唯一 owner。
- 只收编 `Screen/HUD/keybind` 注册。render/audio/debug/Iris/资源包模块继续由 `BongClient` 原顺序显式注册，避免无关范围膨胀。

### 4.7 语义 UI surface 与前后端完全分离

R7 冻结的是**消费端接缝**，不是让 server/agent 知道某个 UI 库的组件树。目标语义 surface 只允许携带以下与渲染库无关的数据：

- `surface_id`、`template_id`、`session_id`、单调 `revision` 和有效期/关闭原因；
- immutable、带版本的 view data；集合必须有稳定 identity，不能依赖数组位置；
- `allowed_actions`：稳定 `action_id`、参数 schema、当前可用性和机器可读拒绝原因；
- 可选的本地化 message key、severity、icon id 等有限 presentation hint，不携带布局坐标。

server/agent **不得**下发 owo XML、HTML、CSS、JavaScript、任意 URL、DOM id 或像素坐标。client 用 `template_id` 在本地白名单中选择 owo/vanilla/MCEF 模板；同一 view data 和 action registry 必须能被三种 adapter 消费。现有 `UiOpen.xml`、`agent_ui_request` 的 raw XML 是迁移阻塞项，只能在兼容期保留，不能成为新 UI 的生产输入。

本次 R7 不偷偷新增 wire union：P0R 先冻结 `UiSurfaceProjection`/action registry 的消费接缝和 source gate；若现有 `ServerDataV1` 无法表达某个语义 surface，由 R6/schema 以及对应 server/agent owner 另立 amendment，按 TypeBox source、generated mirror 和 atomic activation 规则接入。新 wire 未合入前，R7 只能从现有 authoritative payload 构造同形 projection 或使用 test fixture，不能把本地 ViewModel 伪装成服务端事实。

### 4.8 `UiViewport` 与响应式布局契约

当前代码证据表明这条边界必须集中收口：`MixinMouse.java:100-116` 和 `BotanyHudBootstrap.java:58-69` 都把 `MinecraftClient.mouse` 的 physical window 坐标按 `getScaledWidth()/getWidth()`、`getScaledHeight()/getHeight()` 换算为 GUI logical 坐标；`BongHud.java:131-143`、`:243-251`、`:528-541` 则统一从 `getWindow().getScaledWidth/Height()` 生成 HUD 输入、绘制覆盖层和测量文字。相反，`AlchemyScreen.java:664-710`、`ForgeScreen.java:365-375`、`InspectScreen.java:2255-2375` 仍在 Screen 内直接进行 `mouseX/mouseY` 命中和 grid 换算，正是迁移期间要被 `UiViewport`/layout policy 覆盖的耦合点。

当前 provider 事实也必须写明：`client/build.gradle:45-51` 只有 Minecraft、Fabric、owo 依赖，`client/gradle.properties:1-19` 没有 MCEF/JCEF 版本；因此 browser 坐标/texture/provider 接口是 P2 compatibility seam，不是已有 production API。CinemaMod/MCEF 的外部事实和 provider gate 仍以本 plan §2.1 为准。

布局必须由可测试的纯策略计算，不在 controller、Store 或业务 intent 中写死屏幕像素。公共输入至少区分四种坐标空间：Minecraft 窗口/帧缓冲 physical px、Minecraft GUI logical px、browser CSS px、browser/CEF texture physical px，并显式记录 `gui_scale`、window scale factor 和 device-pixel ratio（DPR）。

`UiViewport` 至少包含 `logical_width`、`logical_height`、`framebuffer_width`、`framebuffer_height`、`gui_scale`、`device_pixel_ratio` 和 safe insets；`UiLayoutPolicy.measure(UiViewport, ViewModel)` 输出确定性的 `UiLayoutSnapshot`（layout mode、content rect、控件 bounds、focus order、overflow policy 和 hit regions）。同一输入必须得到同一快照，供 Java adapter 和 headless geometry test 共同消费。

冻结以下不变量：

- 业务层只使用逻辑坐标和 design tokens；不得把窗口 physical px、MCEF texture px 或某一 GUI scale 当作业务尺寸。
- GUI logical 尺寸优先使用 Minecraft 提供的 scaled viewport；browser CSS viewport 与其一一映射，texture 尺寸按 `round(css * DPR)` 计算并限制在 provider 能力范围内。鼠标、触摸、键盘焦点和 browser 输入必须使用同一套正/逆变换，禁止 CinemaMod 式散落的手工比例换算。
- 字体 token 不随 viewport 宽度连续缩放；空间不足时只能换 `COMPACT`/`REGULAR`/`WIDE` 布局、换行、堆叠、滚动或折叠次要操作。主要操作不能被裁切、遮挡或变成不可点击的隐形区域。
- 每个 interactive hit region 必须落在 safe rect 内；除显式声明的 overlay group 外不得重叠；文本按实际字体/浏览器 metrics 测量，不能溢出父容器。resize 只能更新同一 host 的布局和 texture，不得重复创建 native browser/resource。
- P0R 必须根据实际 client window 限制冻结 `MIN_SUPPORTED_VIEWPORT`（默认验收下限为 `320x240`）。低于下限仍需进入 fail-safe compact/scroll 模式并保留关闭路径，但不得把“不支持”尺寸的绿灯算作完整布局支持。

固定回归矩阵至少覆盖：`320x240`、`400x240`、`640x360`、`854x480`、`1000x700`、`1024x768`、`1280x720`、`1365x768`、`1920x1080`、`2560x1080`、`3440x1440`、`1080x1920`；每个尺寸至少跑 GUI scale `1/2/3/4`，window scale/DPR `1.0/1.25/1.5/2.0`，并额外覆盖 odd aspect、resize 中间态和 texture 尚未就绪。矩阵是 geometry/input contract，不要求 bot 启动真实渲染器。

## 5. UI 状态、网络 Handler 与 Bootstrap 的外部接口纪律

### 5.1 Handler → Store

- Handler 解析 `ServerDataEnvelope`/generated payload，做校验、revision/freshness 和 domain dispatch。
- Handler 只写 Store、生成 transient `ServerDataDispatch` 或提交 domain offer；不得直接持有 Screen/widget 引用。
- Store 暴露 `snapshot()` 和 R7 `UiStateSource` adapter；Store 不 import `com.bong.client.ui.adapter.*`。
- Screen/HUD 只消费 ViewModel；不得从 `BongNetworkHandler`、`ServerDataRouter` 或 Handler 取数据。

### 5.2 UI → Intent → Sender

- Screen/widget/input 只调用 domain `UiIntentSink`，不直接 import `ClientRequestProtocol`/`ClientRequestSender`。
- 迁移期间允许 compatibility adapter 在 `client/ui/intent/**` 调用旧 sender；source gate 只允许该目录直接依赖 sender。
- intent 必须带现有请求需要的 identity（如 `instance_id`、`session_id`、`offer_id`/`trigger_id`、坐标/slot），UI 不猜测 server state。
- 不做 client optimistic success；等待 Store/S2C receipt 更新 ViewModel。

### 5.3 BongClient bootstrap

- `BongClient.onInitializeClient()` 保持 network → UI runtime → UI modules 的依赖顺序。
- P2 起将 UI/HUD/keybind call set 迁移到 `UiBootstrapRegistry`; 每批迁移一组并保留旧 module 的 idempotent `register()` 语义。
- `BongNetworkHandler.register()`、`IrisBootstrap.register()`、render/audio/debug 注册不因 UI registry 重构被移动。
- source gate 固定 UI module 清单、owner、依赖、注册顺序和 duplicate registration 行为。

### 5.4 Headless UI driver 与 bot e2e

`UiDriver` 是 semantic UI contract 的无渲染消费方，不是第二套 gameplay API。它与 Java client 共享 action registry、参数校验、`ClientRequestV1` 编码和 authoritative result projection：

```text
semantic UiSurfaceProjection
  -> UiDriver.open(surface_id/session_id)
  -> UiDriver.dispatch(action_id, typed args)
  -> same UiIntentSink / ClientRequestV1
  -> bong:server_data + correlated receipt
  -> UiDriver.awaitRevision/awaitReceipt
```

bot 只允许使用真实 production wire（`Bot.intent(...)`、`bong:client_request`、`proto_min.py` 解码的 `bong:server_data`）以及服务端明确授权的 fixture/setup；不得调用 Java Store、屏幕私有 callback、像素坐标、截图 OCR、raw XML/HTML/JS 或 dev 命令绕过核心动作。dev 命令若用于铺垫，必须与核心 action/receipt 断言分段记录，不能算作 headless 闭环证据。

P0R 冻结 `UiDriver` 的最小外部接口：`open`、`snapshot`、`listActions`、`dispatch`、`awaitRevision`、`awaitReceipt`、`close`；每个方法都带 session identity、revision/request identity 和超时结果。`dispatch` 先做同一 action registry 的参数/availability 校验，非法 action、过期 session、权限不足、重复 request、超时和关闭后的 late result 都必须可观察且无副作用。成功判据是权威 receipt/state transition，不是 transport write 成功。

bot e2e 分三层记录：

1. **contract pin**：surface/action shape、稳定 identity、revision 单调、枚举和 invalid payload；
2. **semantic roundtrip**：open → action → server mutation/拒绝 receipt → projection 更新/关闭；覆盖 happy path、边界、权限、过期、重复、超时和跨 session isolation；
3. **adapter geometry**：同一 ViewModel 在 owo/vanilla/MCEF 的布局和输入映射测试，另行验证，不把像素或真实渲染引入 bot 主路径。

`scripts/bot/_agent_ui_helpers.py` 已有 `bong:agent_ui_cmd`、`bong:agent_ui_request`、`bong:agent_ui_close`、`bong:agent_ui_response` 的 request shape、按钮回执、dismiss、关闭和负向路径；P0R 必须把这些 helper 重定位为 semantic driver 的兼容实现，并标出仍依赖 raw XML 的路径。raw XML 路径在新 semantic surface 未完成前只能作为 legacy regression，不能成为新 adapter 的验收门。

## 6. 阶段总览

- ✅ 2026-07-30 **旧 P0 盘点基线**：29 Screen、92 fill、15 clearChildren、keybind 冲突、R2/R6 ownership fixture 已存在；仅 docs/tests/resources，未改变 production behavior。
- ⬜ **P0R contract rebase + semantic/browser compatibility gate**：补齐 library-neutral contract、semantic surface/action 接缝、Store read adapter、typed intent、bootstrap registry、依赖方向和迁移 exemption；登记 CinemaMod/MCEF 事实、1.20.1 provider 方案、capability fallback、`UiViewport`/layout policy 与 resolution matrix；更新 fixtures，不生成 production adapter。
- ⬜ **P1 core contract + fake/headless projection**：落地 `ui/contract/**`、reconciler、scope、intent result、bootstrap graph、`UiViewport`/`UiLayoutPolicy` 的纯 client 测试实现；提供不依赖渲染器的 `UiSurfaceProjection`/`UiDriver` fake；contract 包 zero owo/vanilla/MCEF dependency。
- ⬜ **P2 三 adapter + bootstrap reference slice**：实现 owo/vanilla host；完成 MCEF 1.20.1 compatibility spike（初始化、local HTML、texture、输入、resize、close、shutdown）；在最低和 odd viewport 矩阵跑 layout/input geometry；选择一个 owo Screen（`CraftScreen`）和一个 vanilla Screen（`TradeOfferScreen`）接入 controller/scope；把 backend capability 和三类 bootstrap 纳入 registry。
- ⬜ **P3 Store/Intent 边界迁移批次 A + semantic/browser vertical slice**：用 semantic surface + 一条本地 HTML/CSS/JS 页面接通同一 controller/view-model/typed intent，再迁移 `AlchemyScreen`、`CraftScreen`、`TradeOfferScreen`、`LootContainerScreen` 及其 panel；UI 不再直接引用 sender/handler；bot 用同一 action id 完成 roundtrip；保留现有 wire 与 server authoritative semantics，wire 形状变更按 R6/schema amendment 原子接入。
- ⬜ **P4 Screen 批量迁移 + input/thread/open/scale policy**：15 个 owo 与 14 个 vanilla Screen 分批归入 adapter；迁移 keybind registry、`ClientThreadMarshal`、`ScreenOpenPolicy`、fill 风险、identity-sensitive list 和 responsive layout；普通 hotkey 不重放。
- ⬜ **P5 InspectScreen tab-first 拆解**：shell 只做一次 Store intake、一次 subscription scope 和交互 arbitration；tab panel 只接 immutable ViewModel + intent callback；不与 R10 server inventory 内部重排同窗口。
- ⬜ **P6 Insight/HUD/Bootstrap 收口**：`offer_id` 保留到 ViewModel/Store/Screen；exact offerId settlement；Sparring invite 只消费 server-authoritative combat snapshot；恢复 `BongHudOrchestrator` qi radar main path；完成剩余 UI bootstrap registry 迁移。
- ⬜ **P7 全量验收 + 归档**：semantic/headless contract、source gate、Java 17 build、bot UI roundtrip、UI C2S smoke、reconnect freshness、resolution/input geometry matrix、真实客户端五大屏回归全部通过后，补 Finish Evidence 并归档被完整吸收的计划。

## 7. 分阶段交付物与验收抓手

### P0R — contract rebase + semantic/browser compatibility gate（ZERO production behavior change）

- **模块**：`client/ui/{contract,state,intent,bootstrap}/` 的契约 fixture（含 `UiStateSourceMode`、`UiSurfaceProjection`、`UiActionRegistry`、`UiViewport`）；新增/更新 `r7-ui-contract.tsv`、`r7-screen-adapters.tsv`、`r7-store-state-sources.tsv`、`r7-intent-boundary.tsv`、`r7-ui-dependency-allowlist.tsv`、`r7-ui-bootstrap-modules.tsv`、`r7-browser-backend-compatibility.tsv`、`r7-semantic-surface.tsv`、`r7-viewport-matrix.tsv`。

- **交付**：29 Screen adapter classification、UI import dependency rules、Store subscription semantics、Intent local-transport semantics、BongClient UI bootstrap module inventory；冻结 semantic surface 的必需 identity/revision/action 字段和 legacy raw XML 隔离；登记 `UiDriver` 外部接口；记录 CinemaMod 1.20.1/JCEF、MCEF Modern 版本和目标 provider 的兼容性证据，明确 provider 未就绪时的 fallback；冻结 `MIN_SUPPORTED_VIEWPORT`、逻辑/physical/browser 坐标转换和 odd-resolution matrix。

- **测试**：`R7FoundationContractTest`、`R7ScreenInventoryContractTest`、`R7UiDependencyContractTest`、`R7BootstrapInventoryTest`、`R7BrowserBackendCompatibilityTest`、`R7SemanticSurfaceContractTest`、`R7ViewportMatrixContractTest`；production source hash 对拍，确认 no production behavior change。

- **跨仓库**：不在 R7 内直接改 schema/proto/Redis/CustomPayload；现有 `proto/bong/envelope.proto`、`agent/packages/schema/src/server-data.ts`、`scripts/bot/_agent_ui_helpers.py` 的 raw XML 耦合登记为 R6/schema/agent amendment 输入，未完成 atomic activation 前只保留 legacy regression。

### P1 — library-neutral core

- **模块**：`client/ui/contract/**`、`client/ui/intent/**`、`client/ui/state/**`、`client/ui/headless/**`；纯 Java fake 不依赖 Minecraft widget 或 browser。
- **交付**：scope LIFO/error aggregation、subscription close/idempotence、reconciler commit/retry、typed intent result、bootstrap dependency graph、semantic surface projection/action registry、`UiDriver` fake、`UiViewport`/`UiLayoutPolicy` 的纯函数实现。
- **测试**：empty→items、equal keys、reorder/add/remove、duplicate/null、patch failure/full retry、rebuild create failure、late callback、double close、dependency cycle/missing/duplicate/idempotent register；surface revision/session/action validation；driver invalid/expired/duplicate/timeout/close；viewport safe rect、compact/regular/wide、text/hit-region overflow 和 coordinate round-trip；每条失败信息带行为原因。
- **跨仓库**：不新增 wire；intent encoder 通过既有 sender contract tests 对拍。

### P2 — 三 adapter + bootstrap reference slice

- **模块**：`client/ui/adapter/{owo,vanilla,mcef}/**`、`CraftScreen`、`TradeOfferScreen`、对应 bootstrap、1.20.1 browser provider compatibility seam。
- **交付**：同一 semantic surface/controller/view-model/intent contract 分别渲染到 owo 与 vanilla；browser adapter 完成 local HTML、texture、input、resize、async init/failure fallback、close/shutdown；Screen removed/close/tick/input cleanup 一致；Dynamic XML 只在 owo adapter 中保留；所有 adapter 共享 `UiLayoutSnapshot`，不共享 DOM/XML。
- **测试**：同一 fake ViewModel 在 owo、vanilla 与 browser host 上的行为对拍；browser 未安装/初始化失败/texture 尚未就绪时 fallback；最低/odd viewport 的 bounds、文字、hit region、focus order 和输入逆变换；subscription 不泄漏；adapter close 后 late state/JS intent no-op；bootstrap registration order/once。
- **跨仓库**：Craft/Trade 既有 C2S/S2C type、request identity、server rejection semantics 完整保留。

### P3 — state/intent boundary migration A + browser vertical slice

- **模块**：`AlchemyScreen`、`CraftScreen`、`TradeOfferScreen`、`LootContainerScreen`、相关 panels/bootstrap、`client/ui/state/**`、`client/ui/intent/**`。
- **交付**：先用 semantic surface + `assets/bong/ui/**` 的本地 HTML/CSS/JS 页面跑通一条完整 browser vertical slice；Screen 不直接依赖 `ClientRequestSender`、`ClientRequestProtocol`、network Handler；所有 Store 读取经 `UiStateSource`/ViewModel，所有输入经 typed sink；明确交易显式 picker 和 inventory `instance_id`；`SemanticUiDriver` 用同一 action registry 跑对应 bot roundtrip。
- **测试**：browser JS bridge 只允许登记的 `intent_id`、非法参数/任意导航/late callback 全部 fail closed；bot 不使用像素点击且能验证 open/action/receipt/revision/rejection/close/session isolation；Craft/Alchemy/Trade/Loot 的 server authoritative roundtrip、无 selection refusal、transport accepted 与 server accepted 分离、断线后 scope/Store 不串会话；existing UI C2S smoke 对拍。
- **跨仓库**：R2 lifecycle、R6 router、schema/proto 不改；CraftStore 只消费 M-09 冻结 contract。

### P4 — 全 Screen、keybind、线程与 open policy

- **模块**：15 owo + 14 vanilla Screen、`BongKeybindRegistry`、`ClientThreadMarshal`、`ScreenOpenPolicy`、`ScreenTransitionController`、`r7-fill100-inventory.tsv` 相关站点。
- **交付**：每个 Screen 有 adapter classification 和 scope owner；所有 production keybind constructor 经 global registry；四个现有 `client.execute` consumer 逐个验真；普通 hotkey drop、passive social offer defer、system terminal priority 固定；所有 Screen 使用 `UiLayoutPolicy`，不在 controller 写死 viewport px。
- **测试**：source gate 禁止 raw network/sender imports；keybind physical duplicate/vanilla reservation/UNKNOWN；thread already-on/off-thread/null executor；open policy 35 条 vectors；fill geometry 与 clearChildren identity tests；resolution matrix 全尺寸/GUI scale/DPR 的 no-overlap/no-clipping/in-bounds/text-fit/hit-test；resize 不重复 native resource；Java 17 full gate。
- **边界**：R6 receive boundary 不重复 marshal；combat snapshot 缺失时 social policy fail closed。

### P5 — InspectScreen tab-first

- **模块**：`inventory/InspectScreen.java`、`InspectScreenBootstrap`、equipment/cultivation/skills/techniques/craft panels、ViewModel/controller/intent adapters。
- **交付**：shell 保留 root composition、唯一 snapshot/subscription intake、input/render routing、drag/drop/context/tooltip/hotbar/overlay arbitration；tab panel 只接 immutable ViewModel + callback，不重复订阅。
- **测试**：tab switch 不新增 listener；旧 listener exactly-once close；drag/drop、tooltip、context overlay 跨 tab 行为不变；list identity/selection/scroll 保留；P3 迁移的 sender import gate继续通过。
- **边界**：不等待、不改 R10 server inventory core，只守现有 view-model/wire contract。

### P6 — Insight/HUD/bootstrap 收口

- **模块**：`InsightOfferHandler`、`HeartDemonOfferHandler` 的窄转换接缝、`InsightOfferViewModel/Store/Screen/Bootstrap`、`SparringInviteScreenBootstrap`、`BongHudOrchestrator`、剩余 UI bootstrap。
- **交付**：`offer_id` 不丢失；distinct offer + reused trigger 不合并；exact offerId claim/compare-and-clear exactly-once；social invite 读取 server-authoritative combat snapshot；qi radar main path 恢复；UI registry 清单与 BongClient call set 收口。
- **测试**：`r7-insight-settlement.tsv` 全 terminal causes、stale A/duplicate callback 不影响 B、combat notify/silent/open/expire、缺 combat producer fail closed、凝脉及以上 `HudRenderLayer.QI_RADAR` 与 negative-qi/TSY false-signal/nearby markers。
- **跨仓库**：`InsightDecision` 当前仅 `trigger_id`/`choice_idx`；未新增 offer_id wire，不宣称 wire-level offer isolation。

### P7 — 验收与归档

- **测试**：Java 17 `flock /tmp/bong-gradle.lock -c "cd client && ./gradlew test build"`；semantic `UiDriver` bot roundtrip（`scripts/bot/**`）；`ui_c2s_smoke`；`reconnect_state_freshness`；`r7-viewport-matrix` geometry/input gate；必要的 `runClient` 五大屏人工回归；source/contract gates。
- **归档**：所有阶段状态更新为 `✅ YYYY-MM-DD`，补 `## Finish Evidence`（模块路径、commit、测试、server/agent/client symbol、遗留项），只归档被本轨完整吸收的计划。

## 8. 吸收清单与已知边界

| finding | R7 处理 |
|---|---|
| `alchemy-screen-fill-overflow` / `alchemy-screen-fill100-eviction` | canonical duplicate；P3/P4 修复并以 geometry fixture pin。 |
| `techniques-tab-scroll-bounce` | P1 reconciler + P5 tab migration；不机械 clear/rebuild。 |
| `botany-rkey-backlog-dispatch` | P4 keybind registry；blocked/inactive presses drain，不 replay。 |
| `client-input-keybind-collision` | P4 global registry；T/L/O/U/R 冲突和 vanilla reservation source gate。 |
| `dying-elder-give-dan-input` | effective binding 驱动 HUD；UNKNOWN 显示“未绑定”，不创建第二默认 G。 |
| `trade-offer-first-item-autopick` | P3 typed picker；必须选择 exact `instance_id`。 |
| `hud-qi-radar-mainpath-regression` | P6 恢复 `BongHudOrchestrator` main path。 |
| `client-insight-offer-strand` | P6 exact offerId settlement + transition cancellation。 |
| `v-sparring-invite-screen-hijack` | P6 social policy；无 authoritative combat snapshot 时 fail closed。 |
| `cast-sync-config-window-thread` / `mineral-probe-result-network-thread-ui` | 已由 R6 receive boundary 修复，R7 不重复接线。 |
| `surface-stash-search-hud-label-gap` | 已修复，不重复实现。 |
| `preview-config-dead-server` / `weather-visual-overlay-collapse` | out-of-track，保留各自 owner。 |

## 9. 开放问题（P0R 决策门前需收口）

1. `UiStateSource` 是否立即回调首个 snapshot，还是由 binder 先 snapshot 再订阅？
2. `UiIntentSink` 是否建立一个巨型统一接口，还是按领域拆小接口？
3. `UiBootstrapRegistry` 是否吞并 BongClient 的全部 register call，还是只收编 UI/HUD/keybind 子集？
4. `BongScreenBase` 是否继续作为生产公共基类？
5. vanilla 与 owo 是否共享同一个 Screen controller/view-model 契约？
6. InspectScreen 是否按 tab-first 拆解，是否与 R10 server inventory 同窗口？
7. `ScreenOpenPolicy` 的 passive invite 是否战斗中延迟、通知一次，还是立即丢弃？

全部在下节收口；原问题保留作历史回溯，实施以 §9.1 为准。

## §9.1 决议（pre-P0 contract rebase，2026-08-24）

### #1 State source 首帧语义

**决议**：`UiStateSource.subscribe()` 只通知后续变化，不保证立即回调；`UiStateBinder` 在 client thread 先读取一次 `snapshot()`，再登记 subscription。这样既兼容已有 Store listener，也避免 mount 时重复 patch。

**落点**：`client/src/main/java/com/bong/client/ui/contract/UiStateSource.java`、`UiSubscription.java`；本 plan §4.1、P1。

### #2 Intent interface 形状

**决议**：按领域拆小型 typed sink，不创建 117 方法总接口；sink 只适配现有 sender，并统一返回 local transport result。server acceptance/rejection 只由 S2C Store/receipt 表示。

**落点**：`client/src/main/java/com/bong/client/ui/intent/**`、`network/ClientRequestSender.java`（只读依赖）；本 plan §4.5、§5.2、P3。

### #3 Bootstrap scope

**决议**：registry 只管理 Screen/HUD/keybind；network、render、audio、debug、Iris 保持 `BongClient` 原有顺序。UI registry 以显式 dependency graph 证明顺序和 idempotence，不引入反射。

**落点**：`client/src/main/java/com/bong/client/ui/bootstrap/**`、`BongClient.java:81-165`；本 plan §4.6、§5.3、P2/P6。

### #4 Screen base ownership

**决议**：`BongScreenBase` 不再是 library-neutral public API；它最多作为 owo adapter compatibility host。新 controller/view-model/scope 不得依赖 `BaseOwoScreen`。vanilla Screen 同等接入 `VanillaScreenHost`。

**落点**：`client/src/main/java/com/bong/client/ui/contract/**`、`ui/adapter/{owo,vanilla}/**`；本 plan §4.3、P1/P2。

### #5 Inspect 拆解与 R10 解耦

**决议**：采用 tab-first；shell 保留唯一 intake 和交互 arbitration；tab panel 只接 immutable ViewModel + intent callback；不与 R10 server inventory 内部文件重排同窗口。

**落点**：`client/src/main/java/com/bong/client/inventory/InspectScreen.java`；本 plan §7 P5。

### #6 Open policy

**决议**：passive social invite 保留 domain Store；战斗/已有屏时首次同 identity `DEFER_NOTIFY`，重复 `DEFER_SILENT`，空屏且 TTL 有效才 `OPEN`；普通 hotkey 永不排队重放；Insight 按 exact `offer_id` settlement；system terminal 按优先级抢占。

**落点**：`SparringInviteScreenBootstrap.java`、`InsightOfferScreenBootstrap.java`、`ScreenTransitionController.java`；本 plan §4.3、§7 P4/P6。

### #7 网络与 UI 边界

**决议**：R6 的 receive-boundary 是唯一网络线程入口；R7 不改 `BongNetworkHandler.register()`、`ProtoServerDataBridge`、`ServerDataRouter`。UI 只能消费 Store/ViewModel 并调用 typed intent；source gate 阻止 UI 直接依赖 Handler/proto/sender。

**落点**：`client/src/main/java/com/bong/client/BongNetworkHandler.java:108-331`、`network/ProtoServerDataBridge.java`、`network/ServerDataRouter.java`；本 plan §2、§5、P4。

### #8 Semantic surface 与 headless driver

**决议**：server/agent 不再以 XML/HTML/JS 描述 UI；跨端只交换带 `surface_id`、`template_id`、`session_id`、`revision`、immutable view data 和 typed `allowed_actions` 的语义 surface。client 通过本地模板 registry 渲染，bot 通过同一 action registry 和 authoritative receipt 消费；R7 不擅自改现有 wire，raw XML 只作为 legacy adapter，真正 wire cutover 由 R6/schema/agent amendment 按 atomic activation 完成。

**落点**：R7 `ui/contract/{surface,headless}/**`、`scripts/bot/` semantic driver；现有 `proto/bong/envelope.proto`、`agent/packages/schema/src/server-data.ts`、`scripts/bot/_agent_ui_helpers.py` 记录为跨轨接入证据；本 plan §2、§4.7、§5.4、P0R/P3。

### #9 Viewport、缩放与输入坐标

**决议**：公共 UI 只接受 `UiViewport` 的 logical dimensions 和显式 scale metadata；`UiLayoutPolicy` 以约束/布局模式处理 compact/regular/wide，不假设 16:9。physical px、MC GUI scale、browser CSS px、DPR texture px 的转换集中在 adapter，输入使用同一逆变换；最低 `320x240`、odd aspect、超宽/超窄/竖屏、GUI scale 1-4、DPR 1.0-2.0 和 resize 中间态全部进入 geometry/input contract。现有 physical→logical 证据是 `client/src/main/java/com/bong/client/mixin/MixinMouse.java:100-116`、`client/src/main/java/com/bong/client/botany/BotanyHudBootstrap.java:58-69`；现有 scaled viewport/HUD 消费证据是 `client/src/main/java/com/bong/client/BongHud.java:131-143,243-251,528-541`；现有 Screen-level hit-test 耦合证据是 `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:664-710`、`client/src/main/java/com/bong/client/forge/ForgeScreen.java:365-375`、`client/src/main/java/com/bong/client/inventory/InspectScreen.java:2255-2375`；browser provider 尚不存在，`client/build.gradle:45-51` 与 `client/gradle.properties:1-19` 是 P2 compatibility seam 的基线。

**落点**：`client/src/main/java/com/bong/client/ui/contract/UiViewport.java`、`UiLayoutPolicy.java`、`ui/adapter/{owo,vanilla,mcef}/**`；本 plan §4.8、P2/P4/P7。

## 10. 实施工作流

### 10.1 适用边界

纯 client 逻辑与 UI adapter 重构，不产出 NBT、worldgen layout、模型或贴图，不适用视觉资产三轮打磨。每个逻辑单元使用中文 atomic commit，并带真实执行模型 `Model:` trailer；不改 `docs/worldview.md`、`docs/library/`、schema shape 或依赖版本。

### 10.2 多 PR 依赖顺序

1. **PR-1 / P0R contract rebase**：只改本 plan、R7 fixture/resource、master ownership 描述；ZERO production behavior change。
2. **PR-2 / P1 library-neutral core**：`ui/contract`、state adapter、intent result、reconciler、bootstrap graph fake；不迁生产 Screen。
3. **PR-3 / P2 adapter reference**：owo + vanilla host，加上 MCEF 1.20.1 compatibility spike；`CraftScreen` + `TradeOfferScreen` 两条垂直切片；UI registry 接入 backend capability 和 resolution/input geometry gate。
4. **PR-4 / P3 state/intent boundary A**：semantic surface/browser vertical slice、`SemanticUiDriver` bot roundtrip，以及 Alchemy/Craft/Trade/Loot；迁移直接 sender/handler import，wire 变更只走 R6/schema atomic amendment。
5. **PR-5 / P4 full Screen/input/scale policy**：剩余 Screen、keybind、thread marshal、open policy、fill/list 和 responsive viewport 迁移。
6. **PR-6 / P5 Inspect split**：tab-first shell/panels，行为不变。
7. **PR-7 / P6 integration + acceptance**：Insight/HUD/bootstrap 收口，完整 client gate、UI C2S smoke、reconnect evidence。
8. **PR-8 / P7 archive**：仅在所有阶段和被吸收计划具备 Finish Evidence 后归档。

前一 PR 的最终 HEAD 未通过 Java 17 gate、fresh-context validator、review、e2e 并 merge 前，不得开始下一阶段。任何 HEAD 变化都使旧 validator/e2e 证据失效。

### 10.3 每个 PR 的闭环门

1. 在独立 worktree/branch 实施，不修改脏 main checkout，不越界改 R2/R6/server owner 文件；semantic wire amendment 未合入前，R7 只做 declared/test-only projection，不接新 production traffic。
2. `git fetch origin` 后紧邻 `git merge origin/main`；merge 触及受影响文件即重跑该阶段全部测试。
3. Client 使用 Java 17 串行门禁：`flock /tmp/bong-gradle.lock -c "cd client && ./gradlew test build"`。
4. 每个阶段最终 SHA 启动 explicit-worktree、read-only fresh-context validator；validator 必须回报 HEAD SHA 对拍。
5. source/contract gate 必须从 production source 派生集合，不用手写数量假绿；生产代码不得调用 test reset seam；bot UI 证据必须包含真实 wire、action id、request/revision/receipt 对拍，不能用像素或截图替代。
6. push 后确认 PR head 等于已验证 SHA，独立评论 `/review`；review 修复产生新 HEAD 时重新 fetch/merge、全量门禁、validator、e2e 和 review。
7. R7 实施 agent 不 merge；orchestrator 在 review/e2e 绿后按 plan 顺序收口。

### 10.4 PR 实施上下文隔离

R7 每个 PR 使用独立实施上下文；主线只调度、读取结论、等待 review/e2e 和收口，不在同一上下文连续堆叠多个 PR：

```text
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "实现本 PR 的限定阶段，先读本 plan 对应章节和 ownership；完成代码、测试、validator、PR 后只回报结论。\nultrathink"
)
```

实施 agent 不等待 review；review finding、返工和新 HEAD 的完整门禁由调度上下文重新派发。每个 PR 仍使用独立 worktree/branch，禁止共享脏工作区或绕过 Java 17/global-lock gate。

### 10.5 终态验收

- `ui/contract` 无 owo/vanilla/widget/MCEF import；UI source gate 无 network Handler/proto/sender 越权 import，server/agent 语义 surface 无 XML/HTML/JS/DOM/像素坐标。
- 29 Screen 全部有 adapter/lifecycle classification；无未登记的 raw Screen exception。
- Store subscription close、disconnect cleanup、late callback、跨 session freshness 全部通过；R2 registry 仍是唯一断线清理入口。
- `UiDriver` 与 Java client 共享 action registry、参数校验、`ClientRequestProtocol` 编码和 authoritative receipt；bot semantic roundtrip 能覆盖 UI 功能而不依赖渲染/输入设备；local transport accepted 与 server result 分离。
- `UiViewport`/`UiLayoutPolicy` 在固定最低、奇怪、超宽、超窄和竖屏矩阵下通过 no-overlap/no-clipping/in-bounds/text-fit/hit-test/coordinate-roundtrip；GUI scale、window scale、DPR 和 browser texture/input mapping 对拍。
- `BongClient` UI/HUD/keybind module registry 的 owner/dependency/order/idempotence pin 全绿；network/render/audio/debug registration 未被误收编。
- Java 17 full gate、`ui_c2s_smoke`、`reconnect_state_freshness` 及必要 `runClient` 真实 Screen 回归通过。

终态补充 `## Finish Evidence`：阶段落点、关键 commit、测试命令/数量、server/agent/client symbol 对拍、遗留项；全部阶段完成后迁入 `docs/finished_plans/plan-refactor-client-ui-base-v1.md`。
