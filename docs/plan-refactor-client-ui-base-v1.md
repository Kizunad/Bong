# plan-refactor-client-ui-base-v1 — Client UI 可替换库边界 + Screen/Store/Intent/Bootstrap 分层（重构轨 R7）

> 所属总纲：`docs/plans-skeleton/plan-refactor-master-v1.md`。一句话：在不改变 server/schema/wire 行为的前提下，把 client UI 的状态读取、用户意图、屏幕生命周期、列表协调和 bootstrap 注册抽成库无关契约，再由 owo 与 vanilla 两条 adapter 实现；以 29 个 Screen 和 `InspectScreen` 为迁移对象，为后续替换 owo-lib 提供单一切换边界。

## 0. 改写目的与不可变范围

旧版 R7 以 `BongScreenBase extends BaseOwoScreen` 为公共基类，能改善当前 owo 屏幕，但不能作为未来 UI 库迁移边界。本版将 R7 从“owo UI 重构”改为“UI contract-first + adapter migration”计划：

1. **库无关核心**：状态读取、订阅生命周期、列表 diff/reconcile、intent dispatch、Screen open policy、bootstrap module contract 不得 import owo、Fabric widget 或 vanilla drawable。
2. **双适配路径**：现有 15 个 owo Screen 和 14 个 vanilla Screen 都必须有明确的 adapter/lifecycle 归属；不把 vanilla Screen 留在第二套隐式生命周期里。
3. **协议不变**：不修改 server、TypeBox shape、protobuf envelope、Redis key、`ClientRequestProtocol` 编码或 `bong:server_data`/`bong:client_request` channel。现有 sender/handler 行为只通过 adapter 复用。
4. **所有权不变**：R2 仍独占 Store 断线清理，R6 仍独占网络 receiver/bridge/router，R9 仍独占 cast domain；R7 只消费它们冻结的外部契约。
5. **无大爆炸重写**：先落 contract、fake、source gate 和一条 owo/一条 vanilla 垂直切片，再批量迁移；不得先同时改 29 个 Screen、109 个 Store 和 80 个 Handler。

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
  -> UiStateSource / UiViewModelAdapter (R7)
  -> UiScreenController
  -> owo adapter OR vanilla adapter

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
| `client/ui/contract/**`、UI adapters、Screen/HUD/keybind 结构 | R7 | 新增库无关契约和 owo/vanilla 实现。 |
| cast domain reducer/store/AV semantics | R9 | R7 只消费 view-model/intent contract。 |
| `BongClient` UI/HUD/keybind bootstrap call set | R7 | 只迁 UI 子集到显式 registry，保留 network/render/audio order。 |
| server、agent、worldgen、worldview、qi ledger | 其他 owner | R7 不触碰。 |

## 4. 库无关公共契约（P0 必须冻结）

库无关类型按职责放在 `client/src/main/java/com/bong/client/ui/{contract,state,intent,bootstrap}/`；这些包的源码不得出现 owo、`BaseOwoScreen`、`FlowLayout`、`net.minecraft.client.gui.widget` 或具体 UI library import。adapter 只能出现在 `client/ui/adapter/{owo,vanilla}/`。

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

库无关 controller 只依赖 immutable `ViewModel`、`UiStateSource` 和 typed `UiIntentSink`：

```java
public interface UiIntent {}

public interface UiScreenController<M> {
    M viewModel();
    void onIntent(UiIntent intent);
    void onOpen(UiScreenScope scope);
    void onClose();
}
```

具体 UI library 只实现 host/adapter，不定义业务规则：

- `client/ui/adapter/owo/OwoScreenHost`：兼容 `BaseOwoScreen`、`OwoUIAdapter`、`FlowLayout` 和动态 XML；只负责把 controller/view-model 映射到 owo component tree。
- `client/ui/adapter/vanilla/VanillaScreenHost`：兼容 vanilla `Screen`、`Drawable`、`ClickableWidget`；只负责布局、输入和绘制。
- `BongScreenBase` 若保留，只能是 `OwoScreenHost` 的兼容实现，不得被写入 `ui/contract` 或作为新 Screen 的业务依赖。
- `DynamicXmlScreen` 继续属于 owo adapter；模板白名单和 XML 安全策略不迁入 library-neutral contract。

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

## 6. 阶段总览

- ✅ 2026-07-30 **旧 P0 盘点基线**：29 Screen、92 fill、15 clearChildren、keybind 冲突、R2/R6 ownership fixture 已存在；仅 docs/tests/resources，未改变 production behavior。
- ⬜ **P0R contract rebase**：补齐 library-neutral contract、Store read adapter、typed intent、bootstrap registry、依赖方向和迁移 exemption；更新 fixtures，不生成 production adapter。
- ⬜ **P1 core contract + fake**：落地 `ui/contract/**`、reconciler、scope、intent result、bootstrap graph 的纯 client 测试实现；contract 包 zero owo/vanilla dependency。
- ⬜ **P2 双 adapter + bootstrap reference slice**：实现 owo/vanilla host；选择一个 owo Screen（`CraftScreen`）和一个 vanilla Screen（`TradeOfferScreen`）接入 controller/scope；把两者 bootstrap 纳入 registry。
- ⬜ **P3 Store/Intent 边界迁移批次 A**：迁移 `AlchemyScreen`、`CraftScreen`、`TradeOfferScreen`、`LootContainerScreen` 及其 panel；UI 不再直接引用 sender/handler；保留现有 wire 与 server authoritative semantics。
- ⬜ **P4 Screen 批量迁移 + input/thread/open policy**：15 个 owo 与 14 个 vanilla Screen 分批归入 adapter；迁移 keybind registry、`ClientThreadMarshal`、`ScreenOpenPolicy`、fill 风险和 identity-sensitive list；普通 hotkey 不重放。
- ⬜ **P5 InspectScreen tab-first 拆解**：shell 只做一次 Store intake、一次 subscription scope 和交互 arbitration；tab panel 只接 immutable ViewModel + intent callback；不与 R10 server inventory 内部重排同窗口。
- ⬜ **P6 Insight/HUD/Bootstrap 收口**：`offer_id` 保留到 ViewModel/Store/Screen；exact offerId settlement；Sparring invite 只消费 server-authoritative combat snapshot；恢复 `BongHudOrchestrator` qi radar main path；完成剩余 UI bootstrap registry 迁移。
- ⬜ **P7 全量验收 + 归档**：contract/source gate、Java 17 build、UI C2S smoke、reconnect freshness、真实客户端五大屏回归全部通过后，补 Finish Evidence 并归档被完整吸收的计划。

## 7. 分阶段交付物与验收抓手

### P0R — contract rebase（ZERO production behavior change）

- **模块**：`client/ui/{contract,state,intent,bootstrap}/` 的契约 fixture（含 `UiStateSourceMode`）；新增/更新 `r7-ui-contract.tsv`、`r7-screen-adapters.tsv`、`r7-store-state-sources.tsv`、`r7-intent-boundary.tsv`、`r7-ui-dependency-allowlist.tsv`、`r7-ui-bootstrap-modules.tsv`。
- **交付**：29 Screen adapter classification、UI import dependency rules、Store subscription semantics、Intent local-transport semantics、BongClient UI bootstrap module inventory。
- **测试**：`R7FoundationContractTest`、`R7ScreenInventoryContractTest`、`R7UiDependencyContractTest`、`R7BootstrapInventoryTest`；production source hash 对拍，确认 no production behavior change。
- **跨仓库**：schema/proto/Redis/CustomPayload 零变更；只引用既有 R2/R6 symbols。

### P1 — library-neutral core

- **模块**：`client/ui/contract/**`、`client/ui/intent/**`、`client/ui/state/**`；纯 Java fake 不依赖 Minecraft widget。
- **交付**：scope LIFO/error aggregation、subscription close/idempotence、reconciler commit/retry、typed intent result、bootstrap dependency graph。
- **测试**：empty→items、equal keys、reorder/add/remove、duplicate/null、patch failure/full retry、rebuild create failure、late callback、double close、dependency cycle/missing/duplicate/idempotent register；每条失败信息带行为原因。
- **跨仓库**：不新增 wire；intent encoder 通过既有 sender contract tests 对拍。

### P2 — owo/vanilla adapter + reference slice

- **模块**：`client/ui/adapter/owo/**`、`client/ui/adapter/vanilla/**`、`CraftScreen`、`TradeOfferScreen`、对应 bootstrap。
- **交付**：同一 controller/view-model/intent contract 分别渲染到 owo 与 vanilla；Screen removed/close/tick/input cleanup 一致；Dynamic XML 只在 owo adapter 中保留。
- **测试**：同一 fake ViewModel 在两个 adapter 上的行为对拍；subscription 不泄漏；adapter close 后 late state no-op；bootstrap registration order/once。
- **跨仓库**：Craft/Trade 既有 C2S/S2C type、request identity、server rejection semantics 完整保留。

### P3 — state/intent boundary migration A

- **模块**：`AlchemyScreen`、`CraftScreen`、`TradeOfferScreen`、`LootContainerScreen`、相关 panels/bootstrap、`client/ui/state/**`、`client/ui/intent/**`。
- **交付**：Screen 不直接依赖 `ClientRequestSender`、`ClientRequestProtocol`、network Handler；所有 Store 读取经 `UiStateSource`/ViewModel，所有输入经 typed sink；明确交易显式 picker 和 inventory `instance_id`。
- **测试**：Craft/Alchemy/Trade/Loot 的 server authoritative roundtrip、无 selection refusal、transport accepted 与 server accepted 分离、断线后 scope/Store 不串会话；existing UI C2S smoke 对拍。
- **跨仓库**：R2 lifecycle、R6 router、schema/proto 不改；CraftStore 只消费 M-09 冻结 contract。

### P4 — 全 Screen、keybind、线程与 open policy

- **模块**：15 owo + 14 vanilla Screen、`BongKeybindRegistry`、`ClientThreadMarshal`、`ScreenOpenPolicy`、`ScreenTransitionController`、`r7-fill100-inventory.tsv` 相关站点。
- **交付**：每个 Screen 有 adapter classification 和 scope owner；所有 production keybind constructor 经 global registry；四个现有 `client.execute` consumer 逐个验真；普通 hotkey drop、passive social offer defer、system terminal priority 固定。
- **测试**：source gate 禁止 raw network/sender imports；keybind physical duplicate/vanilla reservation/UNKNOWN；thread already-on/off-thread/null executor；open policy 35 条 vectors；fill geometry 与 clearChildren identity tests；Java 17 full gate。
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

- **测试**：Java 17 `flock /tmp/bong-gradle.lock -c "cd client && ./gradlew test build"`；`ui_c2s_smoke`；`reconnect_state_freshness`；必要的 `runClient` 五大屏人工回归；source/contract gates。
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

## 10. 实施工作流

### 10.1 适用边界

纯 client 逻辑与 UI adapter 重构，不产出 NBT、worldgen layout、模型或贴图，不适用视觉资产三轮打磨。每个逻辑单元使用中文 atomic commit，并带真实执行模型 `Model:` trailer；不改 `docs/worldview.md`、`docs/library/`、schema shape 或依赖版本。

### 10.2 多 PR 依赖顺序

1. **PR-1 / P0R contract rebase**：只改本 plan、R7 fixture/resource、master ownership 描述；ZERO production behavior change。
2. **PR-2 / P1 library-neutral core**：`ui/contract`、state adapter、intent result、reconciler、bootstrap graph fake；不迁生产 Screen。
3. **PR-3 / P2 adapter reference**：owo + vanilla host；`CraftScreen` + `TradeOfferScreen` 两条垂直切片；UI registry 接入两者。
4. **PR-4 / P3 state/intent boundary A**：Alchemy/Craft/Trade/Loot；迁移直接 sender/handler import，保持 wire 不变。
5. **PR-5 / P4 full Screen/input policy**：剩余 Screen、keybind、thread marshal、open policy、fill/list 迁移。
6. **PR-6 / P5 Inspect split**：tab-first shell/panels，行为不变。
7. **PR-7 / P6 integration + acceptance**：Insight/HUD/bootstrap 收口，完整 client gate、UI C2S smoke、reconnect evidence。
8. **PR-8 / P7 archive**：仅在所有阶段和被吸收计划具备 Finish Evidence 后归档。

前一 PR 的最终 HEAD 未通过 Java 17 gate、fresh-context validator、review、e2e 并 merge 前，不得开始下一阶段。任何 HEAD 变化都使旧 validator/e2e 证据失效。

### 10.3 每个 PR 的闭环门

1. 在独立 worktree/branch 实施，不修改脏 main checkout，不越界改 R2/R6/server owner 文件。
2. `git fetch origin` 后紧邻 `git merge origin/main`；merge 触及受影响文件即重跑该阶段全部测试。
3. Client 使用 Java 17 串行门禁：`flock /tmp/bong-gradle.lock -c "cd client && ./gradlew test build"`。
4. 每个阶段最终 SHA 启动 explicit-worktree、read-only fresh-context validator；validator 必须回报 HEAD SHA 对拍。
5. source/contract gate 必须从 production source 派生集合，不用手写数量假绿；生产代码不得调用 test reset seam。
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

- `ui/contract` 无 owo/vanilla/widget import；UI source gate 无 network Handler/proto/sender 越权 import。
- 29 Screen 全部有 adapter/lifecycle classification；无未登记的 raw Screen exception。
- Store subscription close、disconnect cleanup、late callback、跨 session freshness 全部通过；R2 registry 仍是唯一断线清理入口。
- UI intent 编码与现有 `ClientRequestProtocol`/`ClientRequestSender` tests 对拍；local transport accepted 与 server result 分离。
- `BongClient` UI/HUD/keybind module registry 的 owner/dependency/order/idempotence pin 全绿；network/render/audio/debug registration 未被误收编。
- Java 17 full gate、`ui_c2s_smoke`、`reconnect_state_freshness` 及必要 `runClient` 真实 Screen 回归通过。

终态补充 `## Finish Evidence`：阶段落点、关键 commit、测试命令/数量、server/agent/client symbol 对拍、遗留项；全部阶段完成后迁入 `docs/finished_plans/plan-refactor-client-ui-base-v1.md`。
