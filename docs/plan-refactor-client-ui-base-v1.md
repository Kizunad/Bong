# plan-refactor-client-ui-base-v1 — Client UI 可替换库边界 + Screen/Store/Intent/Bootstrap 分层 + SVG HUD 表现后端（重构轨 R7）

> 所属总纲：`docs/plans-skeleton/plan-refactor-master-v1.md`。一句话：在不改变 server/schema/wire 行为的前提下，把 client UI 的状态读取、用户意图、屏幕生命周期、列表协调和 bootstrap 注册抽成库无关契约，最终以 owo XML 作为唯一生产 UI 宿主，并将 HUD 矢量表现统一收口到 NanoSVG 解析、自有 tessellation 和 Minecraft GUI 提交；以当前 28 个 Screen 和 `InspectScreen` 为迁移对象，为后续替换 owo-lib 提供单一切换边界。

## Integration Preflight（2026-08-25）

按 `docs/CLAUDE.md:7-21` 的防孤岛流程复核后再更新本 plan：

- **正典**：已检查 `docs/worldview.md:1-35`；本计划只做 client UI 基础设施，不新增境界、经济、世界事件或真元/灵气公式，因此不改 worldview，也不创建新的 worldview 锚点。
- **已完成计划**：已检索 `docs/finished_plans/`，重点对照 `plan-client.md`、`plan-HUD-v1.md`、`plan-alchemy-client-v1.md`、`plan-agent-ui-data-v1.md`、`plan-agent-ui-close-reason-drop-v1.md` 及相关 client/session/UI 结论；R7 只抽取现有 Store、HUD、agent UI close 和 client screen 的外部行为，不重新拥有这些 domain。
- **进行中计划**：已枚举 `docs/plan-*.md`，重点核对 `plan-refactor-client-store-lifecycle-v1.md`、`plan-refactor-wire-s2c-v1.md`、`plan-client-login-ux-v1.md` 以及 alchemy/forge/lingtian session UI bugfix plans；R7 将 Store 断线清理留给 R2、bridge/router 留给 R6，不改其 owner 文件。
- **骨架与 reminder**：已检查 `docs/plans-skeleton/plan-refactor-master-v1.md`、`docs/plans-skeleton/reminder.md:1-28` 和全部 UI/client 相关 skeleton；没有同名的 R7 child skeleton，也没有 reminder 条目要求另建 UI contract。master skeleton 保留计划族的 Wave、ownership、headless 总约束；本文件作为 R7 active child 只负责 client UI contract、adapter、Screen/HUD/keybind 和 viewport seam，拆分理由是避免把 9 条重构轨道的跨轨裁决与单轨实施细节混在同一份可消费 plan 中。
- **HUD 现状**：生产链路由 `HudRenderCallback.EVENT` → `BongHud.render` → `BongHudOrchestrator.buildCommands` → `BongHud.renderCommands(DrawContext)` 提交；当前盘点为 98 个 production Java 文件、62 个 `HudRenderLayer`、46 个 `*HudPlanner`、3 个独立 renderer、67 个 HUD 测试文件，另有爆脉、虚蚀、幻觉、共鸣锁等直接 `DrawContext` overlay。`renderSurface` 仅用于测试，不是生产入口；client 当前没有 NanoSVG/JNI/native 依赖或跨平台 native 打包体系。

## Pre-P0 Decisions（2026-09-02）

### HUD 统一 SVG 表现后端

**决议**：HUD 的几何表现统一由 runtime SVG 描述，NanoSVG 仅解析受限 SVG 子集，自有 `SvgTessellator` 生成不可变 `SvgMesh`，最终严格通过 Minecraft GUI API 提交。HUD Store、semantic planner、C2S intent、S2C payload 和 `HudRenderLayer` 不依赖 parser/native；动态文字与物品图标继续使用 Minecraft GUI 的文字/item API，作为明确例外。

**代码探索证据**：

- 注册入口是 `client/src/main/java/com/bong/client/BongClient.java:82-89`，其中 `HudRenderCallback.EVENT.register(BongHud::render)` 位于 UI bootstrap 之前。
- 生产入口是 `client/src/main/java/com/bong/client/BongHud.java:66-73`；命令收集在 `BongHud.java:124-151`，旧 primitive `DrawContext` 分支在 `BongHud.java:153-253`。
- semantic planner 主入口是 `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:124-145`；layer 枚举和顺序由 `client/src/main/java/com/bong/client/hud/HudRenderLayer.java:3-80` 固定。
- `renderSurface` 仅出现在测试 harness；当前 `client/build.gradle:45-51` 没有 NanoSVG/JNI/native 依赖，因此 P4 必须显式交付 parser ABI、受控 native loader、资源校验与平台失败诊断。

**边界与交付顺序**：P4 交付 `HudRenderBackend`、`SvgHudAssetRegistry`、`NanoSvgParser`、`SvgDocument`、`SvgMesh`、`SvgTessellator`、`MinecraftGuiMeshEmitter` 和首个真实 layer；P5 迁移 62 个 layer 与所有直接 overlay。`RenderLayer.getGui()` 在 1.20.1 中使用 `POSITION_COLOR + QUADS`，每个 tessellated triangle 必须编码为退化 quad `(a,b,c,c)`；禁止独立 framebuffer、实体渲染 layer、浏览器/Canvas/NanoVG、直接 OpenGL program。长期 fixture 使用 `ui-svg-hud-contract.tsv` 与 `ui-svg-hud-inventory.tsv`，具体签名、白名单、预算、失败语义和截图门以后续章节为准。

## 0. 改写目的与不可变范围

旧版 R7 以 `BongScreenBase extends BaseOwoScreen` 为公共基类，能改善当前 owo 屏幕，但不能作为未来 UI 库迁移边界。本版将 R7 从“owo UI 重构”改为“UI contract-first + adapter migration”计划：

1. **库无关核心**：状态读取、订阅生命周期、列表 diff/reconcile、intent dispatch、Screen open policy、bootstrap module contract 不得 import owo、Fabric widget 或 vanilla drawable。
2. **单一生产适配路径**：当前 28 个 Screen 全部使用 owo XML 模板；删除后的历史条目只保留在迁移前盘点，不再建立 `VanillaScreenHost` 或保留 vanilla 兼容宿主。
3. **协议不变**：不修改 server、TypeBox shape、protobuf envelope、Redis key、`ClientRequestProtocol` 编码或 `bong:server_data`/`bong:client_request` channel。现有 sender/handler 行为只通过 adapter 复用。
4. **所有权不变**：R2 仍独占 Store 断线清理，R6 仍独占网络 receiver/bridge/router，R9 仍独占 cast domain；R7 只消费它们冻结的外部契约。HUD SVG 只消费已有 semantic snapshot，不改变 Store、wire、schema 或 server 数值。
5. **分批迁移**：先落 contract、fake、source gate 和一条 owo XML 垂直切片，再批量迁移 28 个 Screen；不得把 28 个 Screen、109 个 Store 和 80 个 Handler 混成一次提交。

## 1. 基线证据（2026-08-24 复核）

- client production Java 文件约 **1022** 个；当前真实 Screen **28** 个：22 个 owo host、6 个 vanilla host。`TechniqueScrollReadScreen` 是 helper。逐文件基线：`client/src/test/resources/bong/ui/screen-inventory.tsv`。
- P0R 冻结基线为 **29 个 production Screen**；旧的 `DiffListWidget<T, K, C extends Component>`、92 个 `Sizing.fill(100)` 站点和 Screen-local listener/unsubscriber 语义只作为迁移对拍，不构成 library-neutral core；`ClientThreadMarshal` 只冻结纯 helper API。
- 当前 inventory 中有 12 个 owo `CODE` 实现、8 个现有 `OWO_XML_TEMPLATE`、2 个 `XML_MODEL` 运行时入口和 6 个 vanilla Screen；6 个 vanilla Screen 全部列为 owo XML 重写对象。旧的 `BongScreenBase<R extends ParentComponent>` 和 `DiffListWidget<T,K,C extends Component>` 没有生产调用者，已从代码库移除；后续 neutral contract 不再为它们保留兼容壳。
- `InspectScreen.java` 约 4647 行，同时持有 tab 组合、Store snapshot/listener intake、drag/drop、context menu、tooltip、hotbar 和 overlay arbitration；它必须保持唯一 screen-level intake，tab panel 不得重复订阅同一 Store。
- production `*Store.java` 共 **109** 个（R2 的 108 个业务 Store 加 lifecycle infrastructure 口径）；R2 的 `SessionScopedStore` 只有 `clearOnDisconnect()`，不是 UI subscription contract。现有 listener/remove-listener 形态不统一，必须通过 R7 read adapter 归一，不能假定所有 Store 已有同一接口。
- `client/network` Handler 约 **80** 个；主路径为 `ProtoServerDataBridge -> ServerDataRouter -> domain Handler -> Store/HUD/Screen`。Handler 负责解析和写入 domain state，UI 不得反向依赖 Handler。
- `ClientRequestSender` 现有大量静态 `sendXxx` 入口（审计口径 117 个 public sending entry），同时存在 `void`、本地 transport `boolean`、tracked `request_id` 和 generic JSON 入口。UI 不能把该类当库无关 API；R7 只在其上提供 typed intent adapter。
- `BongClient.onInitializeClient()` 以显式顺序注册 network、HUD、keybind、Screen bootstrap、render/audio 等模块；R7 只收编 UI/HUD/keybind 子集，不改变 `BongNetworkHandler.register()` 的 channel 注册顺序。
- owo `Sizing.fill(100).inflate(space, ...)` 返回完整 `space`，不是同轴兄弟的剩余空间。现有 92 个 token（87 个 executable）以及 15 个 executable `clearChildren()` 站点已由 `fill100-inventory.tsv` 锁定，不能机械全量替换。

## 2. 目标架构与稳定数据流

目标态只允许以下依赖方向：

```text
server/protobuf/JSON
  -> BongNetworkHandler / ProtoServerDataBridge / ServerDataRouter (R6)
  -> domain Handler
  -> domain Store + immutable Snapshot/ViewModel (R2 owns lifecycle)
  -> semantic UiSurfaceProjection / UiStateSource / UiViewModelAdapter (R7)
  -> UiScreenController + local template registry
  -> OwoScreenHost (唯一生产宿主，XML template)

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
- server/agent 只拥有语义 surface、immutable view data 和 allowed action；client 只拥有本地 owo XML 模板、布局和交互呈现。任何 HTML/CSS/JS/DOM/像素坐标不得跨这条边界。
- bot/headless client 消费同一 `surface_id`、`template_id`、`action_id`、参数校验和 authoritative receipt；它跳过模板、渲染和物理输入，但不能走另一套“测试专用”业务接口。

### 2.1 UI backend 决议

- **唯一生产实现**：`owo + XML`。当前 28 个 production Screen 的组件树、布局和静态文案均由本地 XML 模板描述；Java 只保留 controller、状态绑定、事件/intent wiring 和少量无法声明式表达的渲染桥接。
- **vanilla 全量退出**：当前 6 个 vanilla Screen 不建立兼容宿主，统一重写为 owo XML。`VanillaScreenHost`、`net.minecraft.client.gui.widget.*`、`addDrawableChild` 和 Screen-local 手工绘制不能进入重构后的生产 UI。
- **owo 代码构建全量退出**：P0R 基线的 13 个 `CODE` 实现中，P2 已先完成 `CraftScreen` 的 XML host 规范化；当前剩余 12 个 `CODE` 实现必须改为本地 `UIModel` XML template，具体为 `client/src/test/resources/bong/ui/screen-inventory.tsv:3,11,16,18-25,29`。两个运行时 XML 入口分别由 `client/src/main/java/com/bong/client/agentui/AgentUiScreen.java:49` 和 `client/src/main/java/com/bong/client/ui/DynamicXmlScreen.java:11` 定义，并对应 inventory `:2`、`:30`；两者仍需把模板来源规范化为本地白名单 XML。决议落点：本节 §4.3 与 P4。
- **语义与模板分离**：server/agent 仍只交换 semantic surface；client 通过 `template_id` 选择本地 owo XML 模板。Agent UI 的 server-supplied raw XML 仅保留为 legacy compatibility input，不作为新 Screen 模板来源。
- **撤回第三方宿主路线**：本计划不引入 MCEF、JCEF、CinemaMod、原生窗口、HTML/CSS/JS 页面或额外进程；未来若需要第二种生产宿主，另立计划并重新做 compatibility gate。
- **稳定边界**：library-neutral contract 只面向 `OwoScreenHost`；backend capability 只登记 `OWO`，`VANILLA` 仅保留在迁移前统计证据中。

## 3. 接入面与跨轨所有权

### 3.1 进料

- **状态**：R2 管理的 session Store、persistent config Store、HUD state Store 的 immutable snapshot；R7 通过 read adapter 暴露给 UI。
- **网络**：R6 的 `ProtoServerDataBridge`、`ServerDataRouter`、`ServerDataHandler` 已完成 client-thread receive boundary；R7 不重新 marshal 网络 receiver。
- **输入**：现有 keybinding、Screen widget callback、HUD/overlay gesture；迁移后均由 owo XML host 转换成 typed intent。
- **启动**：`BongClient.onInitializeClient()` 的 UI/HUD/keybind bootstrap call set；网络、render、audio、debug module 不被 R7 统一吞并。
- **worldview / qi_physics**：纯 client 基础设施，不新增玩法、境界、经济或真元/灵气公式；不调用、不修改 `qi_physics`，不改变任何守恒 ledger。SVG 只消费已有 semantic HUD snapshot，不改变 Store、wire、schema 或 server 数值。
- **共享类型**：HUD 表现边界冻结为 `HudRenderBackend`、`SvgHudAssetRegistry`、`NanoSvgParser`、`SvgDocument`、`SvgMesh`、`SvgTessellator`、`MinecraftGuiMeshEmitter`；签名、线程约束和失败语义由 `ui-svg-hud-contract.tsv` 固定。

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
| `client/ui/contract/**`、UI adapters、Screen/HUD/keybind 结构 | R7 | 新增库无关契约和 owo XML 实现；不引入 vanilla 宿主、第三方宿主或原生 provider。 |
| cast domain reducer/store/AV semantics | R9 | R7 只消费 view-model/intent contract。 |
| `BongClient` UI/HUD/keybind bootstrap call set | R7 | 只迁 UI 子集到显式 registry，保留 network/render/audio order。 |
| server、agent、worldgen、worldview、qi ledger | 其他 owner | R7 不触碰。 |

## 4. 库无关公共契约（P0 必须冻结）

库无关类型按职责放在 `client/src/main/java/com/bong/client/ui/{contract,state,intent,bootstrap}/`；这些包的源码不得出现 owo、`BaseOwoScreen`、`FlowLayout`、`net.minecraft.client.gui.widget` 或具体 UI library import。adapter 只允许出现在 `client/ui/adapter/owo/`，并且所有 production Screen 模板必须通过 owo `UIModel` XML 加载。

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

- `client/ui/adapter/owo/OwoScreenHost`：兼容 `BaseOwoScreen`、`OwoUIAdapter`、`FlowLayout` 和 `UIModel` XML；只负责把 controller/view-model 映射到 owo component tree。
- 不建立 `client/ui/adapter/vanilla/VanillaScreenHost`。vanilla `Screen`、`Drawable`、`ClickableWidget` 仅作为迁移前源码审计对象，不能成为重构后的生产依赖。
- `BongScreenBase` 已删除；P2 若确实需要 owo host 基类，必须在 adapter 包内以真实生产切片为理由重新引入，不能恢复一个无调用者的根目录 legacy 类型。
- `DynamicXmlScreen` 继续属于 owo adapter；模板白名单和 XML 安全策略不迁入 library-neutral contract。

adapter 不等于 library-neutral contract：owo component 与 XML 模板细节锁在 owo adapter 包。若未来要支持第二种生产宿主，必须另立 plan，不得把具体组件树偷渡成公共 wire schema。

### 4.4 `UiListReconciler<T,K>`

列表核心只处理 key、顺序、patch 和 detached rebuild，不持有 owo component：

- equal key + equal order：只 patch，不替换 mounted row。
- reorder/add/remove：先 detached 创建完整 replacement，全部成功后整体 swap；失败保留旧 committed state。
- null list/item/key、duplicate key：mutation 前 fail-fast。
- patch 异常不回滚外部 row mutation，但内部 committed sequence 保持上一版本，下一次从第一行完整重试；patcher 必须幂等。
- 组件 identity、selection、callback、scroll 保留语义由 adapter 验证；scroll offset 不通过不存在的 owo 0.11.2 API 伪造。

`OwoDiffListWidget` 只实现 owo renderer bridge；不建立 `VanillaDiffListWidget`。原 `DiffListWidget<T,K,C extends Component>` 已删除，不再保留根目录兼容实现。

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

server/agent **不得**下发 owo XML、HTML、CSS、JavaScript、任意 URL、DOM id 或像素坐标。client 用 `template_id` 在本地白名单中选择 owo XML 模板；同一 view data 和 action registry 只由 owo adapter 消费。现有 `UiOpen.xml`、`agent_ui_request` 的 raw XML 是迁移阻塞项，只能在兼容期保留，不能成为新 Screen 的生产输入。

本次 R7 不偷偷新增 wire union：P0R 先冻结 `UiSurfaceProjection`/action registry 的消费接缝和 source gate；若现有 `ServerDataV1` 无法表达某个语义 surface，由 R6/schema 以及对应 server/agent owner 另立 amendment，按 TypeBox source、generated mirror 和 atomic activation 规则接入。新 wire 未合入前，R7 只能从现有 authoritative payload 构造同形 projection 或使用 test fixture，不能把本地 ViewModel 伪装成服务端事实。

### 4.8 `UiViewport` 与响应式布局契约

当前代码证据表明这条边界必须集中收口：`MixinMouse.java:100-116` 和 `BotanyHudBootstrap.java:58-69` 都把 `MinecraftClient.mouse` 的 physical window 坐标按 `getScaledWidth()/getWidth()`、`getScaledHeight()/getHeight()` 换算为 GUI logical 坐标；`BongHud.java:131-143`、`:243-251`、`:528-541` 则统一从 `getWindow().getScaledWidth/Height()` 生成 HUD 输入、绘制覆盖层和测量文字。相反，`AlchemyScreen.java:664-710`、`ForgeScreen.java:365-375`、`InspectScreen.java:2255-2375` 仍在 Screen 内直接进行 `mouseX/mouseY` 命中和 grid 换算，正是迁移期间要被 `UiViewport`/layout policy 覆盖的耦合点。

当前依赖事实也必须写明：`client/build.gradle:45-51` 只有 Minecraft、Fabric、owo 依赖，`client/gradle.properties:1-19` 没有第三方 browser provider；因此第三方宿主不是已有 production API，也不进入 R7 的 adapter seam。

布局必须由可测试的纯策略计算，不在 controller、Store 或业务 intent 中写死屏幕像素。公共输入至少区分 Minecraft 窗口/帧缓冲 physical px 和 Minecraft GUI logical px，并显式记录 `gui_scale` 与 window scale factor。

`UiViewport` 至少包含 `logical_width`、`logical_height`、`framebuffer_width`、`framebuffer_height`、`gui_scale`、`device_pixel_ratio` 和 safe insets；`UiLayoutPolicy.measure(UiViewport, ViewModel)` 输出确定性的 `UiLayoutSnapshot`（layout mode、content rect、控件 bounds、focus order、overflow policy 和 hit regions）。同一输入必须得到同一快照，供 Java adapter 和 headless geometry test 共同消费。

冻结以下不变量：

- 业务层只使用逻辑坐标和 design tokens；不得把窗口 physical px 或某一 GUI scale 当作业务尺寸。
- GUI logical 尺寸优先使用 Minecraft 提供的 scaled viewport；鼠标、键盘焦点和 Screen 输入必须使用同一套正/逆变换，禁止在各 Screen 内散落手工比例换算。
- 字体 token 不随 viewport 宽度连续缩放；空间不足时只能换 `COMPACT`/`REGULAR`/`WIDE` 布局、换行、堆叠、滚动或折叠次要操作。主要操作不能被裁切、遮挡或变成不可点击的隐形区域。
- 每个 interactive hit region 必须落在 safe rect 内；除显式声明的 overlay group 外不得重叠；文本按实际字体 metrics 测量，不能溢出父容器。resize 只能更新同一 host 的布局，不得重复创建组件或订阅。
- P0R 必须根据实际 client window 限制冻结 `MIN_SUPPORTED_VIEWPORT`（默认验收下限为 `320x240`）。低于下限仍需进入 fail-safe compact/scroll 模式并保留关闭路径，但不得把“不支持”尺寸的绿灯算作完整布局支持。

固定回归矩阵至少覆盖：`320x240`、`400x240`、`640x360`、`854x480`、`1000x700`、`1024x768`、`1280x720`、`1365x768`、`1920x1080`、`2560x1080`、`3440x1440`、`1080x1920`；每个尺寸至少跑 GUI scale `1/2/3/4`，window scale `1.0/1.25/1.5/2.0`，并额外覆盖 odd aspect 和 resize 中间态。矩阵是 geometry/input contract，不要求 bot 启动真实渲染器。

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
3. **adapter geometry**：同一 ViewModel 在 owo XML host 的布局和输入映射测试，另行验证，不把像素或真实渲染引入 bot 主路径。

`scripts/bot/_agent_ui_helpers.py` 已有 `bong:agent_ui_cmd`、`bong:agent_ui_request`、`bong:agent_ui_close`、`bong:agent_ui_response` 的 request shape、按钮回执、dismiss、关闭和负向路径；P0R 必须把这些 helper 重定位为 semantic driver 的兼容实现，并标出仍依赖 raw XML 的路径。raw XML 路径在新 semantic surface 未完成前只能作为 legacy regression，不能成为新 adapter 的验收门。

## HUD SVG 表现后端契约（R7 P4/P5）

R7 的最终目标是：HUD 的几何表现统一由 SVG 资源描述，NanoSVG 只负责解析，自有 tessellator 负责把路径变成不可变 mesh，最终仍在 Minecraft 1.20.1 的 GUI 渲染阶段提交。HUD 的 Store、semantic planner、C2S intent、S2C payload 和 `HudRenderLayer` 不依赖 NanoSVG；SVG 是可替换的表现后端，不是业务模型。

### 1. 生产链路与类型边界

```text
Store / server snapshot
  -> BongHudOrchestrator（semantic frame）
  -> HudRenderBackend
  -> SvgHudAssetRegistry（白名单资源 + 缓存）
  -> NanoSvgParser（只解析）
  -> SvgDocument（不可变 Java 数据）
  -> SvgTessellator（只做几何）
  -> SvgMesh（不可变三角形）
  -> MinecraftGuiMeshEmitter（DrawContext GUI buffer）
```

- `HudRenderBackend`、`SvgHudAssetRegistry`、`NanoSvgParser`、`SvgDocument`、`SvgMesh`、`SvgTessellator`、`MinecraftGuiMeshEmitter` 是 R7-owned 类型，签名、线程约束和失败语义冻结在 `client/src/test/resources/bong/ui/ui-svg-hud-contract.tsv`。
- `NanoSvgParser` 的 public API 不暴露 native pointer、arena、C struct 或 parser 生命周期；native 结果必须在 adapter 内复制成受约束的 Java 数值，native 解析器不得直接触碰 `DrawContext`、`RenderSystem` 或 OpenGL。
- `SvgHudAssetRegistry` 只接受 `assets/bong-client/svg/hud/` 下的资源和显式 manifest；拒绝 `..`、外部 URI、网络 URL、`DOCTYPE`/`ENTITY`、超出大小/节点/曲线段/顶点预算的资源。资源重载时完成 parse+tessellate，渲染帧只读取 immutable cache。
- dynamic HUD 值通过受限 binding（颜色、opacity、visibility、transform、clip 范围和 progress）注入 mesh；禁止每帧字符串拼 SVG、重新 parse 或重新 tessellate。动态文字保留 `DrawContext`/`TextRenderer` 提交，物品图标保留 Minecraft GUI item renderer；两者属于明确的 GUI 合规例外，不得回流到 planner。

### 2. NanoSVG/native 交付边界

- P4 必须落一个可审计的 NanoSVG adapter 和 native loader：固定 NanoSVG 版本、源码许可证、ABI 版本、资源校验和加载错误诊断；至少打包 Linux x86_64、Linux aarch64、Windows x86_64、macOS x86_64、macOS arm64 五个平台，平台库不存在或 ABI 不匹配时 fail closed 并给出可定位日志，不静默切换到另一套 SVG parser。
- native 库只能由 Fabric client 在受控资源路径解包后加载；不得允许任意路径、环境变量或网络内容指定 native library。JNI 层只提供 parser 函数，所有 tessellation、mesh 生命周期和 GUI 提交留在 Java 侧。
- NanoSVG 的文本、image、filter、mask、foreignObject、gradient、外部 stylesheet 等不在 V1 支持范围；资源校验必须明确拒绝或记录 unsupported，而不是生成半成品 mesh。路径子集至少覆盖 `path`、`rect`、`circle`、`ellipse`、`polygon`、transform、opacity、fill-rule、stroke、join、cap、洞和 cubic bezier。

### 3. Minecraft GUI 提交约束

- emitter 只能接收 `DrawContext`、当前 GUI `MatrixStack` 和已缓存 `SvgMesh`；禁止世界坐标、`VertexConsumerProvider` 的实体 layer、独立 framebuffer、浏览器/Canvas/NanoVG、直接自建 OpenGL program 或 `RenderSystem` draw call。
- 1.20.1 的 `RenderLayer.getGui()` 是 `POSITION_COLOR + QUADS`，不是三角形 buffer。V1 emitter 必须把每个 tessellated triangle `(a,b,c)` 编码为退化 quad `(a,b,c,c)`，经 `context.getVertexConsumers().getBuffer(RenderLayer.getGui())` 提交，并在 `DrawContext.draw(...)` 边界内批量 flush；不得把三角形直接塞进 QUADS buffer。
- GUI layer 的 alpha、depth、blend、z 顺序必须沿用 Minecraft GUI phase；`HudRenderLayer` 的现有顺序是唯一 layer order，mesh 不能靠浮点 z 或世界深度重排。发射器必须拒绝 NaN/Infinity、越界坐标和超过顶点预算的 mesh。
- `BongHud.renderCommands` 最终只负责收集 frame、应用 `ScreenHudVisibility` 和调用 `HudRenderBackend`；P5 完成后不得再按 `isRect`、`isTexturedRect`、`isEdgeVignette` 等旧 primitive 分支直接画 HUD。

### 4. 全量迁移与删除门

- `ui-svg-hud-inventory.tsv` 逐行登记全部 62 个 `HudRenderLayer`、46 个 `*HudPlanner`、直接 HUD overlay（爆脉、虚蚀、幻觉、共鸣锁等）、对应 SVG asset、dynamic binding、文字/物品 GUI 例外和测试 owner；允许多个 layer 复用同一 SVG asset，但每个 layer 必须有独立可核验 binding。
- P4 只允许首个真实 layer（优先已恢复 main path 的 `QI_RADAR` 或同等复杂 layer）接入 `SvgHudBackend`，同时保留旧 backend 作为受测迁移过渡；过渡 fallback 只能在 P4/P5 中存在，不能进入最终归档状态。
- P5 必须让 inventory 中所有 layer 和直接 overlay 经过同一个 backend，迁移所有 planner 的 semantic output，补齐 malformed/unsupported/native unavailable/tessellation failure/资源重载/visibility/lifecycle 分支测试，并删除旧的逐组件 DrawContext shape path、测试专用 `renderSurface` 以及生产 fallback。删除前必须有旧/新截图 parity 证据和全量 inventory 对拍，不能以“主 HUD 看起来正常”代替。

## 6. 阶段总览

- ✅ 2026-07-30 **旧 P0 盘点基线**：迁移前真实 Screen **29** 个、92 fill、15 clearChildren、keybind 冲突、R2/R6 ownership fixture 已存在；仅 docs/tests/resources，未改变 production behavior。后续已删除一个退役 Screen，当前盘点为 28 个。
- ✅ 2026-08-25 **P0R contract rebase + semantic/owo migration gate**：补齐 library-neutral contract、semantic surface/action 接缝、Store read adapter、typed intent、bootstrap registry、依赖方向和迁移 exemption；冻结 owo XML 唯一生产宿主、`UiViewport`/layout policy 与 resolution matrix；更新长期 fixtures，不生成 production adapter。
- **历史 foundation 已清理，不计入 neutral P1 完成**：`5e40e5ced` 的 owo 专用 `DiffListWidget` 和 `5822dd51a` 的 owo 专用 `BongScreenBase` 没有生产调用者，已连同测试和契约登记删除；`b48dd162c` 的 `BongKeybindRegistry` 仍因生产 bootstrap 接入而保留。旧实现的行为证据不再作为长期 API。
- ✅ 2026-08-26 **P1 core contract + fake/headless projection**：落地 `ui/contract/**`、reconciler、scope、intent result、bootstrap graph、`UiViewport`/`UiLayoutPolicy`；提供不依赖渲染器的 `UiSurfaceProjection`/`UiDriver` fake 和 `StoreUiStateSource`；contract、intent、state、headless 包均未依赖 owo、vanilla widget、Minecraft 或具体 UI 库。
- ✅ 2026-08-27 **P2 owo XML adapter + bootstrap reference slice**：唯一 owo XML host、Craft wide/compact 本地模板、host 生命周期、分阶段 bootstrap 和真实 Fabric/owo 截图/交互门均已落地；Store/Intent 解耦留给 P3。
- ✅ 2026-08-30 **P3 Store/Intent 边界迁移批次 A**：用 semantic surface + 本地 owo XML template 接通同一 controller/view-model/typed intent，再迁移 `AlchemyScreen`、`CraftScreen`、`TradeOfferScreen`、`LootContainerScreen` 及其 panel；UI 不再直接引用 sender/handler；bot 用同一 action id 完成 roundtrip；保留现有 wire 与 server authoritative semantics，wire 形状变更按 R6/schema amendment 原子接入。
- ⬜ **P4 Screen 批量迁移 + input/thread/open/scale policy + SVG HUD 后端 vertical slice**：将 6 个 vanilla Screen 和剩余 12 个 owo CODE Screen 全部重写为 owo XML，连同现有 2 个 XML_MODEL Screen 一并纳入唯一宿主；`CraftScreen` 已由 P2 完成 XML host 规范化，P3 继续处理其 semantic state/intent 边界。迁移 keybind registry、`ClientThreadMarshal`、`ScreenOpenPolicy`、fill 风险、identity-sensitive list 和 responsive layout；普通 hotkey 不重放；同时落地 NanoSVG parser/native adapter、不可变 `SvgDocument`/`SvgMesh`、自有 tessellator、Minecraft GUI emitter、runtime SVG 资源白名单和首个真实 HUD layer 接线。
- ⬜ **P5 InspectScreen tab-first 拆解**：shell 只做一次 Store intake、一次 subscription scope 和交互 arbitration；tab panel 只接 immutable ViewModel + intent callback；不与 R10 server inventory 内部重排同窗口。
- ⬜ **P6 Insight/HUD/Bootstrap 收口**：`offer_id` 保留到 ViewModel/Store/Screen；exact offerId settlement；Sparring invite 只消费 server-authoritative combat snapshot；恢复 `BongHudOrchestrator` qi radar main path；完成剩余 UI bootstrap registry 迁移。
- ⬜ **P7 全量验收 + 归档**：semantic/headless contract、source gate、Java 17 build、bot UI roundtrip、UI C2S smoke、reconnect freshness、resolution/input geometry matrix、真实客户端五大屏回归全部通过；`ui-svg-hud-inventory.tsv` 的 62 个 layer 和直接 overlay 全部命中同一 `HudRenderBackend`，并完成旧 primitive path、`renderSurface` 与生产 fallback 删除后，补 Finish Evidence 并归档被完整吸收的计划。

## 7. 分阶段交付物与验收抓手

### P0R — contract rebase + semantic/owo migration gate（ZERO production behavior change）

- **模块**：`client/ui/{contract,state,intent,bootstrap}/` 的契约 fixture（含 `UiStateSourceMode`、`UiSurfaceProjection`、`UiActionRegistry`、`UiViewport`）；长期保留 `ui-contract.tsv`、`screen-adapters.tsv`、`store-state-sources.tsv`、`intent-boundary.tsv`、`ui-dependency-allowlist.tsv`、`ui-bootstrap-modules.tsv`、`semantic-surface.tsv`、`viewport-matrix.tsv`、`ui-backend-capabilities.tsv`、`ui-svg-hud-contract.tsv` 和 `ui-svg-hud-inventory.tsv`。

- **交付**：29 Screen adapter classification、UI import dependency rules、Store subscription semantics、Intent local-transport semantics、BongClient UI bootstrap module inventory；冻结 semantic surface 的必需 identity/revision/action 字段和 legacy raw XML 隔离；登记 `UiDriver` 外部接口；冻结 `OWO` 唯一 production backend capability、`MIN_SUPPORTED_VIEWPORT`、逻辑/physical 坐标转换和 odd-resolution matrix；登记 62 个 HUD layer、46 个 planner、直接 overlay 与 SVG backend owner。

- **测试**：`R7FoundationContractTest`、`R7ScreenInventoryContractTest`、`R7UiDependencyContractTest`、`R7BootstrapInventoryContractTest`、`R7StoreStateSourceContractTest`、`R7BackendCapabilitiesContractTest`、`R7SemanticViewportContractTest`；focused fixture 从 production source 派生并提供具体文件/符号诊断。全树 production digest 仅作为 P0R 一次性历史举证，不作为后续阶段的长期门禁。SVG backend 的 parser/tessellator/emitter contract 在 P4 单独验证，不能用全树 digest 代替。

长期门禁只覆盖 R7-owned 的 screen、state、intent、bootstrap、semantic 和 viewport 接缝；客户端其他资源、并行 PR 新增的 production 文件以及无关美术资产不属于 R7 contract drift。若需要证明某次阶段确实没有生产变更，应在该阶段单独生成一次性审计证据，不更新长期 fixture。

- **跨仓库**：不在 R7 内直接改 schema/proto/Redis/CustomPayload；现有 `proto/bong/envelope.proto`、`agent/packages/schema/src/server-data.ts`、`scripts/bot/_agent_ui_helpers.py` 的 raw XML 耦合登记为 R6/schema/agent amendment 输入，未完成 atomic activation 前只保留 legacy regression。

#### P0R Finish Evidence

- **落地清单**：新增 source-derived Screen adapter、Store source、bootstrap order、semantic/intent/viewport、UI dependency、backend capability 和 focused contract tests；长期 fixture 统一使用 `ui/` 语义文件名，不再使用阶段性 `r7-` 前缀。未新增 `client/src/main/java` production adapter、Screen migration 或 wire/schema 字段；P2 已完成 `CraftScreen` 的 XML host，当前 6 个 vanilla、12 个 owo CODE、8 个既有 XML 模板与 2 个 XML_MODEL 入口仍是 P3-P4 的明确迁移债务。
- **关键证据**：P0R 原始快照的 production Java source tree SHA-256 为 `66502bbf20e7be0999576c612eac5b53d23c81ca9c5e8c3cad91e67bc3558f2b`，该摘要是一次性历史证据，不再覆盖后续资源或并行 PR 的变更；迁移前 Screen inventory 对拍为 29 个（15 owo、14 vanilla），当前删除一个后为 28 个（22 owo host、6 vanilla）；BongClient UI bootstrap 对拍为 30 个模块；Store source fixture 对拍通过。
- **测试结果**：`./gradlew test --tests 'com.bong.client.ui.R7*ContractTest' --tests com.bong.client.insight.InsightOfferScreenTest --tests com.bong.client.insight.InsightOfferStoreTest -x runGametest` 通过；Java 17 `flock /tmp/bong-gradle.lock -c "cd client && ./gradlew test build"` 通过，包含 3 个 Fabric GameTest。
- **跨仓库核验**：未修改 server、agent/schema、protobuf、Redis key、CustomPayload 或 R2/R6 owner 文件；semantic surface 继续与模板实现分离，现有 raw XML 仍标记为 legacy 输入，后续 wire cutover 仍由 R6/schema/agent amendment 负责。
- **遗留 / 后续**：P1 library-neutral core、P2 Craft XML reference slice 和 P3 状态/意图边界迁移已完成；P4-P7 仍待实施。无生产引用的 legacy foundation 已删除，下一阶段继续迁移 6 个 vanilla Screen、12 个剩余 owo CODE Screen 和 2 个 XML_MODEL 运行时入口；`VANILLA` 仅留在迁移前统计，第三方 host 明确 OUT_OF_SCOPE。

### P1 — library-neutral core ✅ 2026-08-26

- **模块**：`client/ui/contract/**`、`client/ui/intent/**`、`client/ui/state/**`、`client/ui/headless/**`；纯 Java fake 不依赖 Minecraft widget 或具体 adapter。
- **已有实现边界**：无生产引用的 `BongScreenBase` 和 `DiffListWidget` 已删除，不向 `ui/contract/**` 暴露兼容 API。`BongKeybindRegistry` 的全局注册与冲突校验已落地，P4 只处理其余生产覆盖、线程/打开策略和迁移收口。
- **交付**：scope LIFO/error aggregation、subscription close/idempotence、reconciler commit/retry、typed intent result、bootstrap dependency graph、semantic surface projection/action registry、`UiDriver` fake、`UiViewport`/`UiLayoutPolicy` 的纯函数实现。
- **测试**：empty→items、equal keys、reorder/add/remove、duplicate/null、patch failure/full retry、rebuild create failure、late callback、double close、dependency cycle/missing/duplicate/idempotent register；surface revision/session/action validation；driver invalid/expired/duplicate/timeout/close；viewport safe rect、compact/regular/wide、text/hit-region overflow 和 coordinate round-trip；每条失败信息带行为原因。定向 UI 测试和完整客户端 `test build` 均通过。
- **跨仓库**：不新增 wire；intent encoder 通过既有 sender contract tests 对拍。

### P2 — owo XML adapter + bootstrap reference slice ✅ 2026-08-27

- **模块**：`client/ui/adapter/owo/**`、`client/ui/preview/**`、owo XML template registry、`assets/bong/owo_ui/{craft,craft-compact}.xml`、`CraftScreen`、对应 bootstrap。
- **交付**：`CraftScreen` 已接入唯一 owo XML host；Screen removed/close/tick/input cleanup 一致；动态 XML 只留在明确的 legacy compatibility path，新 Screen 只使用受白名单保护的本地 XML；不建立 vanilla host。Craft 外框以 `viewport - 20 x viewport - 12` 连续填满安全区，在 `660x360` 断点选择三栏或 compact 纵向滚动模板，不再在断点两侧跳回固定面板尺寸。server/agent 仍只看语义 `template_id`，不知道本地布局变体。
- **真实渲染门**：`client/ui-preview-harness.json` + `runClientUiPreview` 使用固定本地 Store fixture 打开真实 production Screen，严格对拍 framebuffer、GUI scale、逻辑 viewport、所选模板和关键组件 bounds，再由 `ScreenshotRecorder` 输出 PNG + metadata。首批覆盖 `320x240` minimum、`401x241` odd 和 `683x384` wide；必须等资源重载与 owo adapter 初始化成功，禁止把离线 XML 模拟图或初始化失败后的空帧算 PASS。
- **测试**：fake ViewModel 到 owo XML host 的绑定行为；最低/odd viewport 的 bounds、文字、hit region、focus order和输入逆变换；subscription 不泄漏；adapter close 后 late state/intent no-op；bootstrap registration order/once；生产源码 gate 禁止新的 vanilla Screen 和 owo CODE root。截图供人工视觉复核，自动 PASS 由 metadata、模板选择、adapter ready 和 geometry 断言决定，不做脆弱的整图 hash 比对。
- **跨仓库**：Craft 既有 C2S/S2C type、request identity、server rejection semantics 完整保留；未修改 server、agent/schema、protobuf、Redis key 或 wire shape。

#### P2 Finish Evidence

- **落地清单**：`OwoXmlScreenHost`、`OwoXmlTemplateRegistry` 和 `OwoXmlHostLifecycle` 组成唯一生产 XML 宿主；`craft.xml` / `craft-compact.xml` 及 `CraftScreen` 完成 wide/compact reference slice；`ClientUiBootstrap` 仅迁入 `screen_transition`、`craft_screen` 两个模块，并由 `UiBootstrapRegistry` 按依赖闭包 exactly-once 注册；`UiPreviewClient`、`UiPreviewResultFile` 与 `runClientUiPreview` 构成真实客户端截图门。
- **关键证据**：正式矩阵 `320x240 compact`、`401x241 compact`、`683x384 wide` 全部通过；奇异矩阵 `659x360`、`660x359`、`660x360`、`1001x241`、`321x641`、`1001x721 @ GUI scale 3`、`997x263` 全部通过。`659x360 -> 660x360` 的安全区外框只连续变化 1 px；GUI scale 在 framebuffer 就绪后应用，部分截图、viewport mismatch、初始化失败均使 Gradle 任务失败。
- **测试结果**：adapter/preview/bootstrap 定向测试 29 条通过；真实 renderer 在 settle 后验证安全区、关键控件 in-bounds、中心点命中、physical/logical 坐标往返、Tab 焦点顺序与 compact scroll 内容范围。最终 Java 17 `test build` 与正式 `runClientUiPreview` 结果记录在本阶段 PR。
- **跨仓库核验**：改动仅位于 client 与本 plan；`ClientRequestSender`、Craft Store/S2C handler、server/schema/agent 均保持原契约，XML 模板只存在于 client 本地资源。
- **遗留 / 后续**：本阶段只验证 host/lifecycle/bootstrap/viewport reference slice。`CraftScreen` 仍直接消费现有 Store/Sender，semantic ViewModel、`UiStateSource` 与 typed `UiIntentSink` 接线属于 P3；其余 28 个 Screen 的 XML 化属于 P4，未在本阶段提前迁移。

### P3 — state/intent boundary migration A

- **模块**：`AlchemyScreen`、`CraftScreen`、`TradeOfferScreen`、`LootContainerScreen`、相关 panels/bootstrap、`client/ui/state/**`、`client/ui/intent/**`。
- **交付**：先用 semantic surface + 本地 owo XML template 跑通一条完整 vertical slice；Screen 不直接依赖 `ClientRequestSender`、`ClientRequestProtocol`、network Handler；所有 Store 读取经 `UiStateSource`/ViewModel，所有输入经 typed sink；明确交易显式 picker 和 inventory `instance_id`；`SemanticUiDriver` 用同一 action registry 跑对应 bot roundtrip。
- **测试**：非法参数、过期 session、late callback 和关闭后 intent 全部 fail closed；bot 不使用像素点击且能验证 open/action/receipt/revision/rejection/close/session isolation；Craft/Alchemy/Trade/Loot 的 server authoritative roundtrip、无 selection refusal、transport accepted 与 server accepted 分离、断线后 scope/Store 不串会话；existing UI C2S smoke 对拍。
- **跨仓库**：R2 lifecycle、R6 router、schema/proto 不改；CraftStore 只消费 M-09 冻结 contract。

#### P3 批次 A 验证证据

- **落地模块**：`alchemy/{AlchemyScreenViewModel,AlchemyUiStateSource,AlchemyScreenController,AlchemyIntent,AlchemyClientIntentSink}.java`、`social/{TradeOfferScreenViewModel,TradeOfferUiStateSource,TradeOfferScreenController,TradeOfferIntent,TradeOfferClientIntentSink}.java`、`inventory/{LootContainerScreenViewModel,LootContainerUiStateSource,LootContainerScreenController,LootContainerIntent,LootContainerClientIntentSink}.java`；Craft 的同型边界沿用 P3 Craft slice；`ui/headless/SemanticUiDriver.java` 接入同一 action registry。
- **边界结果**：目标 Screen/Panel 不再直接引用 Store、`ClientRequestSender`、`ClientRequestProtocol` 或 handler；所有状态通过 `UiStateSource` → immutable ViewModel，所有输入通过 guarded typed `UiIntentSink`；关闭 scope、late callback、过期 session、重复 request、stale revision 和 transport/local rejection 均 fail closed。
- **身份与会话**：Trade picker 只接受当前 authoritative inventory 中明确的 `instance_id`，选择对象消失时不自动换选；Loot panel 按 `session_id` 卸载旧实例，禁止跨会话继续发送 move/close。
- **测试结果**：定向 P3 JUnit 套件 15 条通过；同一 Gradle 任务额外执行的 Fabric GameTest 门 3 条通过；完整 `./gradlew test build` 通过，5044 条 JUnit 测试、0 failures、0 errors；未修改 server、agent/schema、protobuf、Redis key 或 wire shape。

### P4 — 全 Screen、keybind、线程、open policy 与 SVG HUD vertical slice

- **模块**：28 个 Screen（6 个 vanilla 重写 + 12 个剩余 owo CODE XML 化 + 8 个既有 owo XML 模板 + 2 个 XML_MODEL）、`BongKeybindRegistry`、`ClientThreadMarshal`、`ScreenOpenPolicy`、`ScreenTransitionController`、`fill100-inventory.tsv` 相关站点；`client/src/main/java/com/bong/client/hud/svg/**` 的 NanoSVG/native adapter、asset registry、`SvgDocument`/`SvgMesh`、tessellator、GUI emitter 与首个真实 HUD layer。
- **交付**：在已有 `BongKeybindRegistry` 全局注册和冲突校验基础上，补齐剩余 production keybind constructor 覆盖；28 个 Screen 全部通过 owo XML template registry 接入并保留 adapter classification、scope owner；2 个现有 XML/runtime 入口也必须收口到本地 template；四个现有 `client.execute` consumer 逐个验真；普通 hotkey drop、passive social offer defer、system terminal priority 固定；所有 Screen 使用 `UiLayoutPolicy`，不在 controller 写死 viewport px；同时接入首个真实 HUD layer 的 `SvgHudBackend`，保留旧 backend 仅作受测迁移过渡。
- **测试**：source gate 禁止 raw network/sender imports、vanilla Screen/widget imports 和 owo CODE root；keybind physical duplicate/vanilla reservation/UNKNOWN；thread already-on/off-thread/null executor；open policy 35 条 vectors；fill geometry 与 clearChildren identity tests；resolution matrix 全尺寸/GUI scale 的 no-overlap/no-clipping/in-bounds/text-fit/hit-test；resize 不重复组件或订阅；`NanoSvgParserTest`、`SvgTessellatorTest`、`MinecraftGuiMeshEmitterTest` 覆盖白名单、malformed/native failure、曲线/洞/stroke/fill-rule/transform/opacity、NaN/Infinity/预算拒绝和 triangle→degenerate-quad；Java 17 full gate。
- **阻塞点状态（2026-08-30）**：`combat_hud_state.combat_active` 已由 server `CombatState.in_combat_until_tick` 计算，经 proto、Java handler/store 接入；`SparringInviteScreenBootstrap` 仅消费 `authoritativeSnapshot().combatActive()`，缺少合法权威快照时返回 `UNKNOWN` 并 fail closed。该项解除，但 P4 其余 Screen/keybind/thread/open/scale 交付仍未完成。
- **边界**：R6 receive boundary 不重复 marshal；combat snapshot 缺失时 social policy fail closed。

### P5 — InspectScreen tab-first

- **模块**：`inventory/InspectScreen.java`、`InspectScreenBootstrap`、equipment/cultivation/skills/techniques/craft panels、ViewModel/controller/intent adapters。
- **交付**：shell 保留 root composition、唯一 snapshot/subscription intake、input/render routing、drag/drop/context/tooltip/hotbar/overlay arbitration；tab panel 只接 immutable ViewModel + callback，不重复订阅。
- **测试**：tab switch 不新增 listener；旧 listener exactly-once close；drag/drop、tooltip、context overlay 跨 tab 行为不变；list identity/selection/scroll 保留；P3 迁移的 sender import gate继续通过。
- **边界**：不等待、不改 R10 server inventory core，只守现有 view-model/wire contract。

### P6 — Insight/HUD/bootstrap 收口与全量 SVG 迁移

- **模块**：`InsightOfferHandler`、`HeartDemonOfferHandler` 的窄转换接缝、`InsightOfferViewModel/Store/Screen/Bootstrap`、`SparringInviteScreenBootstrap`、`BongHudOrchestrator`、剩余 UI bootstrap，以及 `ui-svg-hud-inventory.tsv` 登记的全部 layer/overlay。
- **交付**：`offer_id` 不丢失；distinct offer + reused trigger 不合并；exact offerId claim/compare-and-clear exactly-once；social invite 读取 server-authoritative combat snapshot；qi radar main path 恢复；UI registry 清单与 BongClient call set 收口；62 个 layer 和直接 overlay 统一经过同一个 `HudRenderBackend`，动态文字/物品图标保留明确的 Minecraft GUI 例外。
- **测试**：`insight-settlement.tsv` 全 terminal causes、stale A/duplicate callback 不影响 B、combat notify/silent/open/expire、缺 combat producer fail closed、凝脉及以上 `HudRenderLayer.QI_RADAR` 与 negative-qi/TSY false-signal/nearby markers；inventory 全量对拍、malformed/unsupported/native unavailable、资源重载、visibility/lifecycle 和 SVG 截图 parity。
- **跨仓库**：`InsightDecision` 当前仅 `trigger_id`/`choice_idx`；未新增 offer_id wire，不宣称 wire-level offer isolation。

### P7 — 验收与归档

- **测试**：Java 17 `flock /tmp/bong-gradle.lock -c "cd client && ./gradlew test build"`；semantic `UiDriver` bot roundtrip（`scripts/bot/**`）；`ui_c2s_smoke`；`reconnect_state_freshness`；`viewport-matrix.tsv` geometry/input gate；`runClientUiPreview` 真实 Fabric/owo 多 viewport 截图门；必要的 `runClient` 五大屏人工回归；source/contract gates。
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
4. 无生产调用者的 `BongScreenBase` / `DiffListWidget` 是否应删除？
5. vanilla 与 owo 是否共享同一个 Screen controller/view-model 契约？
6. InspectScreen 是否按 tab-first 拆解，是否与 R10 server inventory 同窗口？
7. `ScreenOpenPolicy` 的 passive invite 是否战斗中延迟、通知一次，还是立即丢弃？
8. HUD 是否将 `docs/svg` 设计稿升级为 runtime SVG，以及 NanoSVG/native、tessellation 和 Minecraft GUI 提交边界如何固定？

全部在下节收口；原问题保留作历史回溯，实施以 §9.1/§9.2 为准。

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

**决议**：无生产调用者的 `BongScreenBase` / `DiffListWidget` 已删除，不保留空壳 compatibility host。新 controller/view-model/scope 不得依赖 `BaseOwoScreen`；所有 Screen 统一接入 owo XML host，vanilla Screen 只作为迁移前审计记录。

**落点**：`client/src/main/java/com/bong/client/ui/contract/**`、`ui/adapter/owo/**`；本 plan §4.3、P1/P2。

### #5 Inspect 拆解与 R10 解耦

**决议**：采用 tab-first；shell 保留唯一 intake 和交互 arbitration；tab panel 只接 immutable ViewModel + intent callback；不与 R10 server inventory 内部文件重排同窗口。

**落点**：`client/src/main/java/com/bong/client/inventory/InspectScreen.java`；本 plan §7 P5。

### #6 Open policy

**决议**：passive social invite 保留 domain Store；战斗/已有屏时首次同 identity `DEFER_NOTIFY`，重复 `DEFER_SILENT`，空屏且 TTL 有效才 `OPEN`；普通 hotkey 永不排队重放；Insight 按 exact `offer_id` settlement；system terminal 按优先级抢占。

该决议同时冻结 `BLOCK_DROP` 的普通 hotkey 结果（契约向量见
`client/src/test/resources/bong/ui/screen-open-policy.tsv:17-19`，断言见
`client/src/test/java/com/bong/client/ui/R7FoundationContractTest.java:265-266`）；当前
production policy seam 是 `SparringInviteScreenBootstrap.decide(...)`（含 combat fail-closed、
首次 `DEFER_NOTIFY`、重复 `DEFER_SILENT`，见
`client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java:64-119`），其行为回归
见 `client/src/test/java/com/bong/client/social/SparringInviteScreenBootstrapTest.java:231-265`。
Insight lifecycle 必须证明 stale offer A 不能清除 offer B；精确 `offer_id` compare-and-clear
见 `client/src/main/java/com/bong/client/insight/InsightOfferStore.java:93-160`，旧屏回归见
`client/src/test/java/com/bong/client/insight/InsightOfferScreenTest.java:101-120`。

**落点**：`client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java`、
`client/src/main/java/com/bong/client/insight/InsightOfferScreenBootstrap.java`、
`client/src/main/java/com/bong/client/insight/InsightOfferStore.java`、
`client/src/main/java/com/bong/client/ui/ScreenTransitionController.java`；本 plan §4.3、§7 P4/P6。

### #7 网络与 UI 边界

**决议**：R6 的 receive-boundary 是唯一网络线程入口；R7 不改 `BongNetworkHandler.register()`、`ProtoServerDataBridge`、`ServerDataRouter`。UI 只能消费 Store/ViewModel 并调用 typed intent；source gate 阻止 UI 直接依赖 Handler/proto/sender。

**落点**：`client/src/main/java/com/bong/client/BongNetworkHandler.java:108-331`、`network/ProtoServerDataBridge.java`、`network/ServerDataRouter.java`；本 plan §2、§5、P4。

### #8 Semantic surface 与 headless driver

**决议**：server/agent 不再以 XML/HTML/JS 描述 UI；跨端只交换带 `surface_id`、`template_id`、`session_id`、`revision`、immutable view data 和 typed `allowed_actions` 的语义 surface。client 通过本地 owo XML template registry 渲染，bot 通过同一 action registry 和 authoritative receipt 消费；R7 不擅自改现有 wire，raw XML 只作为 legacy compatibility input，真正 wire cutover 由 R6/schema/agent amendment 按 atomic activation 完成。

**调研依据**：legacy proto `UiOpen.xml` 位于 `proto/bong/envelope.proto:2731-2735`；当前 Agent UI 的 raw XML schema 位于 `agent/packages/schema/src/payloads/agent-ui.ts:50-88` 和 `agent/packages/schema/src/server-data.ts:1994-2022`；server 清洗并经专属 JSON channel 下发的路径位于 `server/src/network/agent_ui.rs:415-498`；client 运行时解析入口位于 `client/src/main/java/com/bong/client/agentui/AgentUiScreen.java:136-152,437-448`；本地模板白名单位于 `client/src/main/java/com/bong/client/ui/adapter/owo/OwoXmlTemplateRegistry.java:13-30,61-72`；bot 对当前四字段 raw payload 的验证位于 `scripts/bot/_agent_ui_helpers.py:68-145`。

**落点**：R7 `ui/contract/{surface,headless}/**`、`scripts/bot/` semantic driver；上述 legacy/raw XML 入口只作为 R6/schema/agent amendment 的接入证据，不是新 semantic surface；本 plan §2、§4.7、§5.4、P0R/P3。

### #9 Viewport、缩放与输入坐标

**决议**：公共 UI 只接受 `UiViewport` 的 logical dimensions 和显式 scale metadata；`UiLayoutPolicy` 以约束/布局模式处理 compact/regular/wide，不假设 16:9。physical px 与 MC GUI scale 的转换集中在 owo XML adapter，输入使用同一逆变换；最低 `320x240`、odd aspect、超宽/超窄/竖屏、GUI scale 1-4 和 resize 中间态全部进入 geometry/input contract。现有 physical→logical 证据是 `client/src/main/java/com/bong/client/mixin/MixinMouse.java:100-116`、`client/src/main/java/com/bong/client/botany/BotanyHudBootstrap.java:58-69`；现有 scaled viewport/HUD 消费证据是 `client/src/main/java/com/bong/client/BongHud.java:131-143,243-251,528-541`；现有 Screen-level hit-test 耦合证据是 `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:664-710`、`client/src/main/java/com/bong/client/forge/ForgeScreen.java:365-375`、`client/src/main/java/com/bong/client/inventory/InspectScreen.java:2255-2375`。

**落点**：`client/src/main/java/com/bong/client/ui/contract/UiViewport.java`、`UiLayoutPolicy.java`、`ui/adapter/owo/**`；本 plan §4.8、P2/P4/P7。

## §9.2 决议索引

HUD SVG 的范围、代码探索证据和实施前置条件已在本 plan 开头的 `Pre-P0 Decisions` 收口；本节只保留索引，避免 P4/P5 实施者误以为该决议是在 P0 之后才生效。详细契约见《HUD SVG 表现后端契约（R7 P4/P5）》及 `ui-svg-hud-contract.tsv`、`ui-svg-hud-inventory.tsv`。

## 10. 实施工作流

### 10.1 适用边界

纯 client 逻辑与 UI adapter 重构，不产出 NBT、worldgen layout、模型或贴图；允许新增 runtime SVG 资源、NanoSVG native adapter、Java tessellator、GUI emitter 及截图 fixture。SVG HUD 资源按视觉资产纪律执行 3 轮 screenshot review，终轮 commit 需带 `<PROMISE>`。每个逻辑单元使用中文 atomic commit，并带真实执行模型 `Model:` trailer；不改 `docs/worldview.md`、`docs/library/`、schema shape 或依赖版本。

### 10.2 多 PR 依赖顺序

1. **PR-1 / P0R contract rebase**：只改本 plan、R7 fixture/resource、master ownership 描述；ZERO production behavior change。
2. **PR-2 / P1 library-neutral core**：`ui/contract`、state adapter、intent result、reconciler、bootstrap graph fake；不迁生产 Screen。
3. **PR-3 / P2 owo XML adapter reference**：唯一 owo XML host；以 `CraftScreen` 为第一条垂直切片；UI registry 只登记 `OWO` 和 resolution/input geometry gate。
4. **PR-4 / P3 state/intent boundary A**：semantic surface + owo XML vertical slice、`SemanticUiDriver` bot roundtrip，以及 Alchemy/Craft/Trade/Loot；迁移直接 sender/handler import，wire 变更只走 R6/schema atomic amendment。
5. **PR-5 / P4 full Screen/input/scale policy + SVG backend slice**：剩余 Screen、keybind、thread marshal、open policy、fill/list 和 responsive viewport 迁移；落地 NanoSVG/native adapter、asset registry、不可变 document/mesh、自有 tessellator、GUI emitter 和首个真实 HUD layer。
6. **PR-6 / P5 Inspect split + 全量 HUD 表现迁移**：tab-first shell/panels，行为不变；按 `ui-svg-hud-inventory.tsv` 迁移 62 个 layer、直接 overlay 和 `HudRenderCommand` primitive adapter，保留文字/物品 GUI 例外。
7. **PR-7 / P6 parity、极端分辨率和删除旧路径**：完成 Insight/HUD/bootstrap 收口、SVG screenshot parity、`320x240`/`401x241`/`683x384`/超宽/竖屏和 GUI scale 1-4 验证，删除旧 primitive DrawContext path、`renderSurface` 和生产 fallback。
8. **PR-8 / P7 archive**：所有阶段、SVG acceptance 和被吸收计划具备 Finish Evidence 后归档。

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

- `ui/contract` 无 owo/vanilla/widget import；UI source gate 无 network Handler/proto/sender 越权 import，server/agent 语义 surface 无 XML/HTML/JS/DOM/像素坐标。
- 28 Screen 全部有 adapter/lifecycle classification；迁移清单明确 6 个 vanilla 重写项、12 个剩余 owo CODE XML 化项、8 个既有 owo XML 模板和 2 个 XML_MODEL 项，无未登记的 raw Screen exception。
- Store subscription close、disconnect cleanup、late callback、跨 session freshness 全部通过；R2 registry 仍是唯一断线清理入口。
- `UiDriver` 与 Java client 共享 action registry、参数校验、`ClientRequestProtocol` 编码和 authoritative receipt；bot semantic roundtrip 能覆盖 UI 功能而不依赖渲染/输入设备；local transport accepted 与 server result 分离。
- `UiViewport`/`UiLayoutPolicy` 在固定最低、奇怪、超宽、超窄和竖屏矩阵下通过 no-overlap/no-clipping/in-bounds/text-fit/hit-test/coordinate-roundtrip；GUI scale、window scale 和 owo XML input mapping 对拍。
- `BongClient` UI/HUD/keybind module registry 的 owner/dependency/order/idempotence pin 全绿；network/render/audio/debug registration 未被误收编。
- 62 个 `HudRenderLayer`、46 个 planner 和全部直接 HUD overlay 均经同一 `HudRenderBackend`；奇怪分辨率 `320x240`、`401x241`、`683x384`、超宽、竖屏及 GUI scale 1-4 的 screenshot harness 证明 framebuffer 非空、mesh bounds 不越界、alpha/layer order 正确；旧 primitive DrawContext 分支、`renderSurface` 与生产 fallback 已删除。
- Java 17 full gate、`ui_c2s_smoke`、`reconnect_state_freshness` 及必要 `runClient` 真实 Screen 回归通过。

终态补充 `## Finish Evidence`：阶段落点、关键 commit、测试命令/数量、server/agent/client symbol 对拍、遗留项；全部阶段完成后迁入 `docs/finished_plans/plan-refactor-client-ui-base-v1.md`。
