# Bong · plan-input-binding-v1

通用环境交互键体系。**G 键是统一环境交互入口**：TSY 容器搜刮、地面物品拾取、玩家/NPC 互动、未来资源点/灵龛激活都通过一个客户端路由器分发，避免继续注册多个互相抢同一默认键的 `KeyBinding`。

状态：Active（待实施，2026-05-02 从骨架升级）

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | `InteractKeyRouter` 框架 + 唯一 G KeyBinding | 新路由器可注册 handler，空路由/冲突/屏幕打开时行为有测试 |
| **P1** ⬜ | 迁移现有两个 G 用户：地面物品拾取 + 社交交易 | 旧 G 行为保持，交易优先于拾取，不再有重复 G keybinding |
| **P2** ⬜ | TSY 容器搜刮接入 G，并补齐 C2S search 契约 | G 可触发 `start_search`，取消/进度 payload 双端 schema 对齐 |
| **P3** ⬜ | 扩展点与优先级 pin 测试 | 新增 handler 只注册，不改核心路由；优先级表锁死 |

**世界观锚点**：纯工程 plan，无新增 worldview 语义。决策来源为 `docs/plans-skeleton/plan-gameplay-journey-v1.md` §O.1 #5：TSY 搜刮和 item 摄取统一绑定到 G。

**交叉引用**：
- `docs/finished_plans/plan-tsy-container-v1.md` §5.3：旧设计写 E/USE，本 plan 改为 G。
- `docs/plan-inventory-v1.md`：dropped loot 已有 `G` pickup，需迁入统一路由。
- `docs/plan-HUD-v1.md` §7：F1-F9 快捷使用、E/I/K 面板键保持不变。
- `docs/plans-skeleton/plan-gameplay-journey-v1.md` §O.1/#5、§Q.1：本 plan 是 Wave 0 派生硬依赖。

---

## 接入面 Checklist

- **进料**：
  - `client/src/main/java/com/bong/client/inventory/DroppedItemPickupBootstrap.java` 当前直接注册 `GLFW_KEY_G`，从 `DroppedItemStore.nearestTo(...)` 发 `pickup_dropped_item`。
  - `client/src/main/java/com/bong/client/social/TradeOfferScreenBootstrap.java` 当前也直接注册 `GLFW_KEY_G`，准星命中 `PlayerEntity` 时发 `trade_offer_request`。
  - `client/src/main/java/com/bong/client/tsy/ExtractInteractionBootstrap.java` 当前使用 Y/U，不在 P1 强迁；作为 P3 后续 handler 接口样本。
  - `agent/packages/schema/src/container-interaction.ts` 已有 `StartSearchRequestV1` / `CancelSearchRequestV1`，但尚未接入 `ClientRequestV1` union、Rust schema 和客户端发送器。
- **出料**：
  - 新增 `client/src/main/java/com/bong/client/input/InteractKeyRouter.java`：唯一环境交互分发器。
  - 新增 `client/src/main/java/com/bong/client/input/InteractionKeybindings.java`：唯一注册 G 的入口。
  - 新增 `client/src/main/java/com/bong/client/input/InteractIntent.java` / `InteractCandidate.java` / `IntentHandler.java` / `InteractPriorityResolver.java`。
  - 旧 `DroppedItemPickupBootstrap`、`TradeOfferScreenBootstrap` 不再各自注册 G，只提供 handler 注册/状态处理。
- **共享类型 / event**：
  - 复用现有 `pickup_dropped_item`、`trade_offer_request`、`trade_offer_response`、`start_extract_request`、`cancel_extract_request` 等具体 C2S intent。
  - v1 **不新增泛化服务端 `InteractRequest`**；泛化只存在于 client 本地路由，server 继续接收领域 intent，避免把服务端 handler 变成大 switch。
  - TSY 搜刮使用 `StartSearchRequestV1` / `CancelSearchRequestV1`，需补 `type: "start_search"` / `type: "cancel_search"` 并纳入 `ClientRequestV1`。
- **跨仓库契约**：
  - client：`ClientRequestProtocol.encodePickupDroppedItem`、`encodeTradeOfferRequest`、新增 `encodeStartSearch` / `encodeCancelSearch`。
  - agent/schema：`ClientRequestV1` union、schema registry、samples/generated JSON。
  - server：`server/src/schema/client_request.rs` 的 `ClientRequestV1::StartSearch` / `CancelSearch`，以及 `server/src/network/client_request_handler.rs` 分发。
- **worldview 锚点**：无。保持纯输入/协议收口，不改世界观、物价、修炼数值。

---

## 现状审计（2026-05-02）

1. `DroppedItemPickupBootstrap` 和 `TradeOfferScreenBootstrap` 同时注册默认 G；Fabric 允许用户重映射，但默认体验会出现同键多消费者，各自 tick poll 的顺序由注册顺序隐式决定。
2. `DroppedItemPickupBootstrap` 已经用 G 触发 `ClientRequestSender.sendPickupDroppedItem(nearest.instanceId())`，并依赖 `DroppedItemStore.nearestTo` 的稳定 tie-break。
3. `TradeOfferScreenBootstrap` 已经用 G 触发 `ClientRequestSender.sendTradeOfferRequest("entity:" + hit.getEntity().getId(), item.instanceId())`，目标解析在 server `resolve_trade_offer_target`。
4. TSY 容器搜刮的 S2C schema 和 client HUD 状态已有雏形：`container-interaction.ts`、`SearchHudState`、`SearchProgressHudPlanner`；但 C2S search request 没进入 `agent/packages/schema/src/client-request.ts` union，Rust `ClientRequestV1` 与 `ClientRequestProtocol` 也没有 start/cancel search。
5. `docs/plan-HUD-v1.md` 已定 F1-F9 是快捷使用，E/I/K 是 UI 面板；本 plan 不碰这些键。

---

## §0 设计轴心

- [ ] **唯一入口**：client 只注册一个默认 G 的 `KeyBinding`，所有环境交互走 `InteractKeyRouter`。
- [ ] **领域 handler 自治**：每个 handler 只回答“现在能不能处理”和“命中后如何发送已有 intent”，路由器不理解背包、交易、容器内部细节。
- [ ] **显式优先级**：同一 tick 多个候选时按 priority 排序，再用距离/稳定序兜底；不再依赖 bootstrap 注册顺序。
- [ ] **server 仍权威**：client 只做候选选择，合法性仍由 server 的既有 handler 校验。
- [ ] **不与 F 冲突**：F1-F9 是快捷使用栏，G 是环境交互。1-9 / E / I / K / R / V 保持现状。
- [ ] **可扩展但不预留胖接口**：新增环境交互只新增一个 `IntentHandler`，不修改 `InteractKeyRouter` 核心逻辑。

---

## §1 优先级路由表

| 优先级 | Intent | v1 handler | 触发条件 | 出站请求 |
|---:|---|---|---|---|
| 100 | `SearchContainer` | `TsyContainerSearchHandler` | 5 格内 raycast/准星命中可搜刮容器 | `start_search` |
| 90 | `TalkNpc` / `TradePlayer` | `TradeOfferIntentHandler` | 准星命中 `PlayerEntity` 且背包有可交易 item | `trade_offer_request` |
| 80 | `ActivateShrine` | P3 stub | 灵龛/灵眼目标，当前只锁接口 | 当前不发送 |
| 70 | `PickupDroppedItem` | `DroppedItemPickupIntentHandler` | `DroppedItemStore.nearestTo(...) != null` | `pickup_dropped_item` |
| 60 | `HarvestResource` | P3 stub | 灵草/矿物等资源点，当前只锁接口 | 当前不发送 |
| 0 | `None` | router fallback | 无候选 | 不发送请求；可记录 debug log |

冲突规则：
- 准星命中容器 > 准星命中玩家/NPC > 最近掉落物。
- 同优先级候选按距离升序；距离相等时按 handler 注册序稳定排序。
- 打开任意 Screen 时，G 不触发环境交互；Screen 内快捷键由各 Screen 自己处理。

---

## §2 P0 · Client Router 框架

目标：先建立可测的本地路由，不改变任何 server 协议。

交付物：
- [ ] `client/src/main/java/com/bong/client/input/InteractIntent.java`
  - enum：`SearchContainer`、`TradePlayer`、`TalkNpc`、`ActivateShrine`、`PickupDroppedItem`、`HarvestResource`、`None`。
- [ ] `client/src/main/java/com/bong/client/input/InteractCandidate.java`
  - 字段：`InteractIntent intent`、`int priority`、`double distanceSq`、`String debugLabel`。
  - 工厂方法拒绝负 priority / NaN distance。
- [ ] `client/src/main/java/com/bong/client/input/IntentHandler.java`
  - `Optional<InteractCandidate> candidate(MinecraftClient client)`。
  - `boolean dispatch(MinecraftClient client, InteractCandidate candidate)`。
- [ ] `client/src/main/java/com/bong/client/input/InteractPriorityResolver.java`
  - 纯函数：从候选列表选唯一 winner。
  - 空列表返回 `Optional.empty()`。
- [ ] `client/src/main/java/com/bong/client/input/InteractKeyRouter.java`
  - 注册 handler，按 `InteractPriorityResolver` 选择 winner 并调用 dispatch。
  - 不依赖具体领域 store，不 import inventory/social/tsy 包。
- [ ] `client/src/main/java/com/bong/client/input/InteractionKeybindings.java`
  - 唯一注册 `key.bong-client.interact`，默认 `GLFW_KEY_G`，category `category.bong-client.controls`。
  - `ClientTickEvents.END_CLIENT_TICK` 内消费 `wasPressed()`，`currentScreen != null` 时直接 return。
- [ ] `client/src/main/java/com/bong/client/BongClient.java`
  - 注册 `InteractionKeybindings.register()`。

测试：
- [ ] `client/src/test/java/com/bong/client/input/InteractPriorityResolverTest.java`
  - 空候选、单候选、多优先级、同优先级距离、同距离稳定排序、非法候选。
- [ ] `client/src/test/java/com/bong/client/input/InteractKeyRouterTest.java`
  - 无 handler 不发送、候选 dispatch 只触发 winner、dispatch false 不触发 fallback、Screen 打开时路由不执行。
- [ ] `client/src/test/java/com/bong/client/input/InteractionKeybindingsTest.java`
  - 默认 key 为 G；只注册一个环境交互 key 的 translation key。

验收命令：

```bash
cd client && ./gradlew test build
```

---

## §3 P1 · 迁移现有 G 用户

目标：把已经上线的两个 G 行为迁到统一路由，保持玩家可见行为不变。

交付物：
- [ ] `client/src/main/java/com/bong/client/inventory/DroppedItemPickupBootstrap.java`
  - 删除本类自己的 `KeyBindingHelper.registerKeyBinding(... GLFW_KEY_G ...)`。
  - 暴露或迁移为 `DroppedItemPickupIntentHandler`：候选来自 `DroppedItemStore.nearestTo(...)`，dispatch 发 `sendPickupDroppedItem(instanceId)`。
  - 保留 dropped-loot store / renderer / S2C handler，不改 inventory 协议。
- [ ] `client/src/main/java/com/bong/client/social/TradeOfferScreenBootstrap.java`
  - 删除本类自己的 G key 注册；保留 incoming offer screen tick 处理。
  - 新增 `TradeOfferIntentHandler`：准星命中 `PlayerEntity` 且 `firstTradeItem(...) != null` 才产出候选。
  - dispatch 仍发 `sendTradeOfferRequest("entity:<protocol_id>", offered_instance_id)`。
- [ ] `client/src/main/java/com/bong/client/BongClient.java`
  - 在 `InteractionKeybindings.register()` 后注册 P1 handlers。
  - `DroppedItemPickupBootstrap.register()` 只做非 keybinding 初始化；`TradeOfferScreenBootstrap.register()` 只做 incoming offer tick。
- [ ] `client/src/main/java/com/bong/client/input/DefaultInteractionHandlers.java`
  - 集中注册 P1 handlers，避免 `BongClient` 直接 import 过多领域类。

行为锁定：
- [ ] 附近只有 dropped loot：按 G 仍发送 `pickup_dropped_item`。
- [ ] 准星命中玩家且背包有可交易 item：按 G 发送 `trade_offer_request`。
- [ ] 准星命中玩家但无可交易 item：trade handler 不产出候选，可 fallback 到 dropped loot。
- [ ] 准星命中玩家且附近也有 dropped loot：交易优先。
- [ ] 打开 InspectScreen / TradeOfferScreen / 任意 Screen：G 不触发环境交互。

测试：
- [ ] `client/src/test/java/com/bong/client/inventory/DroppedItemPickupIntentHandlerTest.java`
  - nearest null / nearest valid / dispatch 发正确 instance id。
- [ ] `client/src/test/java/com/bong/client/social/TradeOfferIntentHandlerTest.java`
  - crosshair none、crosshair 非玩家、无可交易 item、有 item、target id 编码。
- [ ] `client/src/test/java/com/bong/client/input/DefaultInteractionHandlersTest.java`
  - trade > pickup，trade missing item 后 pickup fallback。
- [ ] `client/src/test/java/com/bong/client/network/ClientRequestSenderTest.java`
  - 复用现有 sender backend，断言 P1 dispatch payload 不变。

验收命令：

```bash
cd client && ./gradlew test build
```

---

## §4 P2 · TSY 容器搜刮接入 G

目标：把 `plan-tsy-container-v1` 的容器搜刮入口从旧 E/USE 设计切到 G，并补齐当前缺失的 C2S 契约。

### 4.1 Schema / 协议收口

- [ ] `agent/packages/schema/src/container-interaction.ts`
  - `StartSearchRequestV1` 增 `type: Type.Literal("start_search")`。
  - `CancelSearchRequestV1` 增 `type: Type.Literal("cancel_search")`。
- [ ] `agent/packages/schema/src/client-request.ts`
  - import 并纳入 `ClientRequestV1` union。
  - 新增导出类型，避免 schema registry 单独生成但总 union 不认识。
- [ ] `agent/packages/schema/src/schema-registry.ts`
  - 保持 `clientRequestStartSearchV1` / `clientRequestCancelSearchV1`，生成 JSON 应包含 `type`。
- [ ] `agent/packages/schema/samples/`
  - 新增 `client-request.start-search.sample.json`、`client-request.cancel-search.sample.json`。
  - 新增 invalid sample：缺 `type`、负 `container_entity_id`。
- [ ] `server/src/schema/client_request.rs`
  - 新增 `ClientRequestV1::StartSearch { v, container_entity_id }`。
  - 新增 `ClientRequestV1::CancelSearch { v }`。
  - serde tag 与 TypeBox `type` 对齐。
- [ ] `client/src/main/java/com/bong/client/network/ClientRequestProtocol.java`
  - 新增 `encodeStartSearch(long containerEntityId)` / `encodeCancelSearch()`。
- [ ] `client/src/main/java/com/bong/client/network/ClientRequestSender.java`
  - 新增 `sendStartSearch(long containerEntityId)` / `sendCancelSearch()`。

### 4.2 Client handler

- [ ] `client/src/main/java/com/bong/client/tsy/TsyContainerStateStore.java`
  - 保存 `ContainerStateV1` 最新快照，按 `entity_id` 覆盖，depleted 时仍保留但 handler 不产出候选。
  - 提供 `nearestInteractable(MinecraftClient client, double maxDistance)`。
- [ ] `client/src/main/java/com/bong/client/network/ContainerInteractionHandler.java`
  - 接 `ContainerStateV1` / `SearchStartedV1` / `SearchProgressV1` / `SearchCompletedV1` / `SearchAbortedV1`，更新 `TsyContainerStateStore` 与 `SearchHudState`。
  - 接入 `ServerDataRouter` 或现有 server-data 分发通道；若当前 server 尚无对应 `ServerData` variant，本 P2 同步补。
- [ ] `client/src/main/java/com/bong/client/tsy/TsyContainerSearchIntentHandler.java`
  - priority 100。
  - 候选只接受 5 格内、未 depleted、未被其他玩家占用的容器。
  - dispatch 发 `sendStartSearch(container_entity_id)`。
- [ ] `client/src/main/java/com/bong/client/input/DefaultInteractionHandlers.java`
  - 注册 `TsyContainerSearchIntentHandler`，优先级高于 trade/pickup。

### 4.3 Server handler

- [ ] `server/src/network/client_request_handler.rs`
  - `StartSearch` 分支校验玩家 position / dimension / range，转发到 TSY container search 系统。
  - `CancelSearch` 分支取消当前玩家 search session。
  - 缺少 TSY search event/resource 时 log warn 并拒绝，不 panic。
- [ ] 若 `server/src/world/tsy_container_search.rs` 尚未暴露 request API：
  - 增加小接口 `request_start_search(player: Entity, container_entity_id: u64)` / `request_cancel_search(player: Entity)` 或 Bevy Event。
  - 保持 search 业务仍在 world 模块，network 只做解析和转发。

测试：
- [ ] `agent/packages/schema/tests/container-interaction.test.ts`
  - `StartSearchRequestV1` / `CancelSearchRequestV1` 必须含 type；负 id / 缺 type 拒绝。
- [ ] `agent/packages/schema/tests/client-request.test.ts` 或现有 schema registry 测试
  - `ClientRequestV1` 接受 start/cancel search sample。
- [ ] `server/src/schema/client_request.rs::tests`
  - start/cancel search roundtrip；缺 type / 负 id reject。
- [ ] `server/src/network/client_request_handler.rs::tests`
  - start_search 正常转发、out-of-range 拒绝、missing resource 不 panic、cancel_search 无 session 幂等。
- [ ] `client/src/test/java/com/bong/client/tsy/TsyContainerSearchIntentHandlerTest.java`
  - depleted / occupied / out-of-range 不产出；可搜容器 dispatch payload 正确；container > trade > pickup。
- [ ] `client/src/test/java/com/bong/client/network/ClientRequestProtocolTest.java`
  - start/cancel search JSON 与 schema sample 对齐。

验收命令：

```bash
cd agent && npm run build && (cd packages/schema && npm test)
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd client && ./gradlew test build
```

---

## §5 P3 · 扩展点与防回归

目标：把未来交互接入方式锁死，但不实现未到期玩法。

交付物：
- [ ] `client/src/main/java/com/bong/client/input/ReservedInteractionIntents.java`
  - 文档化 `ActivateShrine`、`HarvestResource`、`TalkNpc`、`StartExtract` 的推荐 priority。
  - 不注册空 handler，不制造运行时分支。
- [ ] `client/src/test/java/com/bong/client/input/InteractionPriorityContractTest.java`
  - pin 优先级表：container 100、trade/player 90、shrine 80、pickup 70、resource 60。
- [ ] `client/src/test/java/com/bong/client/input/NoDuplicateDefaultGKeybindingTest.java`
  - 扫描/构造注册入口，确保只有 `InteractionKeybindings` 使用 `GLFW_KEY_G` 作为环境交互默认键。
- [ ] 更新 `client/src/main/resources/assets/bong-client/lang/zh_cn.json` / `en_us.json`
  - 增加 `key.bong-client.interact` 文案。
  - 旧 pickup/trade key translation 若不再注册 keybinding，可保留作兼容文案，不作为 active key 显示。

非目标：
- [ ] 不做长按 G / Shift+G / Ctrl+G。
- [ ] 不做目标重叠 UI 选择器。
- [ ] 不迁移 F1-F9、1-9、E、I、K、R、V。
- [ ] 不新增通用 server `InteractRequest`。
- [ ] 不实现灵龛/资源点具体玩法，只锁定 handler 接入方式。

验收命令：

```bash
cd client && ./gradlew test build
```

---

## §6 总体验收

必须全部通过：

```bash
cd agent && npm run build && (cd packages/schema && npm test)
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd client && ./gradlew test build
```

手动/联调检查：
- [ ] 默认键位设置只出现一个“交互 / Interact”，默认 G。
- [ ] 只有地面掉落物时按 G 能拾取，server log 收到 `pickup_dropped_item`。
- [ ] 准星命中玩家且有可交易 item 时按 G 发交易，不拾取附近掉落物。
- [ ] TSY 容器 5 格内按 G 发 `start_search`，进度 HUD 能进入 searching。
- [ ] 搜刮中主动取消走 `cancel_search`，server 发 `SearchAbortedV1 { reason: "cancelled" }`。
- [ ] 打开 InspectScreen / TradeOfferScreen 时按 G 不触发环境交互。

---

## §7 风险与缓解

| 风险 | 缓解 |
|---|---|
| 一个 G 迁移导致旧行为丢失 | P1 先复用现有 payload，测试直接断言 JSON 不变 |
| router 变成领域大泥球 | router 不 import inventory/social/tsy；领域逻辑留在 handler |
| TSY search schema 已生成但未进总 union，出现“生成物看似存在、运行时不识别” | P2 必须同时改 TypeBox union、Rust enum、Java protocol、samples |
| 多个候选争抢用户意图 | priority contract test 锁定 container > trade > pickup |
| Screen 内 G 误触发环境交互 | `InteractionKeybindings` 在 `currentScreen != null` 时直接 return |

---

## §8 进度日志

- 2026-05-01：骨架创建。由 `plan-gameplay-journey-v1` §O.1 #5 派生。
- 2026-05-02：升级为 Active plan。实地审计现有 G key 冲突、dropped loot pickup、social trade、TSY container schema 缺口，明确 v1 只做客户端统一路由 + 既有 intent 复用 + TSY search 契约补齐。
