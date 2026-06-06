# plan-exploration-probe-return-v1：神识感知 / 顿悟 回程整链

> **一句话主题**：三条 server→client **回程断链**——server 每帧算出结果却没人发：① 神识感知**矿脉**（`MineralProbeResponse`）、② 神识感知**保鲜**（`FreshnessProbeResponse`）、③ 修炼**顿悟**（`InsightOffer`，突破/濒死/炼器触发）。client UI 全就绪，缺的只是 server 侧 S2C emit system + `ServerDataPayloadV1` 变体 + 整条 proto 链 + client 路由。本 plan 是纯**接线**，不造新玩法。

## 阶段总览

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 神识感知矿脉 S2C 回执 + 右键矿块触发 + 结果显示（actionbar + SFX） | ✅ | 2026-06-07 |
| P1 | 神识感知保鲜 C2S 触发 + S2C 回执（复用 `freshness_update` 链） | ✅ | 2026-06-07 |
| P2 | 修炼顿悟 `InsightOffer` S2C 桥（突破/濒死/炼器，复用 InsightOfferScreen） | ✅ | 2026-06-07 |

来源：`project_broken_links_audit`（2026-06-02 全仓玩法断链审计，主题 4「exploration-probe-return-paths」）经 `find-gameplay-broken-links` workflow（2026-06-07）逐条对当前代码 grep 复核，三条均 `stillBroken:true`。

---

## 接入面（防孤岛 · 必读 `docs/CLAUDE.md` §二）

- **进料**（server 侧已产出的 ECS 事件，本 plan 只负责把它们排空送出）：
  - `MineralProbeResponse`（`server/src/mineral/events.rs:123`，由 `mineral/probe.rs` 的 `resolve_mineral_probe_intents` 产出）
  - `FreshnessProbeResponse`（`server/src/shelflife/probe.rs:34`，由 `resolve_freshness_probe_intents`@`probe.rs:88` 产出）
  - `InsightOffer`（`server/src/cultivation/insight.rs:374`，在 `cultivation/insight_flow.rs:212` fallback 路径 + `network/mod.rs:2263` agent-fed 路径两处 `send`）
- **出料**：S2C `ServerDataPayloadV1` 包 → `send_server_data_payload` → client `ServerDataRouter` 字符串路由 → 各自 store/screen：
  - 矿脉 → **新** `MineralProbeResultHandler`（actionbar 文本 + SFX）
  - 保鲜 → 复用 `FreshnessStore` ← `ProcessingServerDataHandler`（`freshness_update` 类型，`ServerDataRouter.java:186` 已注册，**无新 handler**）
  - 顿悟 → 复用 `InsightOfferStore.replace()` → `InsightOfferScreen`（`InsightOfferScreenBootstrap` 监听 store，HeartDemon 路径已证可用）
- **复用类型 / 不另造**：`InsightOfferV1`（`server/src/schema/cultivation.rs`，已存在，不新建）、`FreshnessUpdateV1`（`server/src/schema/processing.rs:27`，已存在）。新增仅 `MineralProbeResultV1`（S2C）+ `FreshnessProbe` 变体（C2S）。emit system **逐字镜像** `server/src/network/tribulation_heart_demon_offer_emit.rs`（唯一已验证可用模板）。
- **跨仓库契约**：
  - proto：`proto/bong/envelope.proto` 的 `ServerDataEnvelope` oneof 新增 `mineral_probe_result` / `insight_offer` 两个 field；C2S 在 `client_request.proto`（若存在）/对应 envelope 新增 `freshness_probe`
  - server：`ServerDataType::{MineralProbeResult, InsightOffer}`、`ClientRequestV1::FreshnessProbe`
  - client：`ProtoServerDataBridge.CASE_TO_TYPE`（`MINERAL_PROBE_RESULT`/`INSIGHT_OFFER`）、`ServerDataRouter` type 串 `mineral_probe_result` / `insight_offer`、C2S `encodeFreshnessProbe`
  - agent schema：`agent/packages/schema` 补 `MineralProbeResultV1` / `FreshnessProbe` TypeBox + sample（双端校验；`InsightOfferV1` 已有则补 sample）
- **worldview 锚点**：
  - 两枚探针 = **神识感知**（`worldview.md` §境界表 line 69「凝脉…能感知区域灵气精确值」+ §神识 line 517-519「高境修士施神识→看到当前真元色/携带/经脉路径痕迹」）。门槛 **凝脉**（`shelflife/probe.rs` `MIN_PROBE_REALM_RANK: u8 = 2` 已强制；mineral 侧 `MineralProbeDenialReason::RealmTooLow` 已强制）。
  - 顿悟 = `worldview.md` line 480/490「特性（顿悟 effect…）是横向能力树」+ line 378「顿悟…挑准破绽偷一波」。突破/濒死时天道给修士一次抉择。
- **qi_physics 锚点**：**none**。三条全是**只读传输**已解算好的状态（剩余储量 / 保鲜快照 / 顿悟选项描述符）。`freshness` payload 里的 `current_qi` 是**信息性 float**，不发生任何转移；不新增、不需要 `qi_physics::ledger::QiTransfer`。顿悟 **effect 应用**留在既有 `InsightChosen` handler（`insight_flow.rs`），不在本传输 plan 范围。

---

## 背景：三条断链的 grep 证据

| # | 断链 | producer（存在） | consumer（缺失） | 证据 |
|---|------|------------------|------------------|------|
| 1 | 矿脉回执从不发 | `mineral/probe.rs` 写 `EventWriter<MineralProbeResponse>`；C2S 已通（`client_request_handler.rs:1554`→`MineralProbeIntent`；client `encodeMineralProbe`@`ClientRequestProtocol.java:598`/`sendMineralProbe`@`ClientRequestSender.java:127`） | 全仓无 `EventReader<MineralProbeResponse>`（非测试）；`schema/server_data.rs` 0 处 Mineral；`ServerDataRouter` 无 `mineral_probe_result`；`sendMineralProbe` 无任何 mixin/screen 调用点 | `grep -rn 'EventReader.*MineralProbeResponse' server/src` → 0；`grep -c Mineral server/src/schema/server_data.rs` → 0 |
| 2 | 保鲜既无 C2S 触发也无 S2C 回执 | `shelflife/probe.rs:88` `resolve_freshness_probe_intents`；client 显示侧全就绪（`FreshnessStore`/`FreshnessTooltipHook`/`ProcessingServerDataHandler` 已处理 `freshness_update`@`ServerDataRouter.java:186`） | `schema/client_request.rs` 无 `FreshnessProbe`；`client_request_handler.rs` 无 `FreshnessProbeIntent` 触发臂；`network/` 内 0 处实例化 `FreshnessUpdateV1`；client 无 `encodeFreshnessProbe` | `grep -n FreshnessProbe server/src/schema/client_request.rs` → 0；`grep -rn FreshnessUpdateV1 server/src/network/` → 0 |
| 3 | 顿悟事件落地即丢 | `insight_flow.rs:212` + `network/mod.rs:2263` 两路 `send(InsightOffer{…})`；`InsightOfferV1` schema 已存在（`schema/cultivation.rs`）；client `InsightOfferScreen`/`Store`/`Bootstrap` 全就绪，C2S `insight_decision` 已通（`encodeInsightDecision`@`ClientRequestProtocol.java:277`） | 全仓无 `EventReader<InsightOffer>`（非测试）；`schema/server_data.rs` 0 处 InsightOffer；`ServerDataRouter` 无 `insight_offer`；screen **仅**经 `heart_demon_offer` 打开（`HeartDemonOfferHandler.java:42`） | `grep -rn 'EventReader.*InsightOffer' server/src` → 0；`grep insight_offer ServerDataRouter.java` → 0 |

未在范围（已核实**非**断链，勿动）：alchemy 炮制 session（`alchemy_snapshot_emit.rs:43`→`AlchemySessionHandler` 全链已通）；tribulation 心魔顿悟（独立 `heart_demon_offer` 变体已通）。

---

## ⚠️ 新 `ServerDataPayloadV1` 变体的「整条 proto 链」清单（红旗预防）

> 历史教训（`feedback_workflow_consume_plan_gotchas` / combat-feedback plan 纠偏）：**新增一个 `ServerDataPayloadV1` 变体若漏补 proto 链，client 会静默收不到 HUD**。Rust 侧穷尽 match 漏写=编译失败（fail-loud，好）；client `CASE_TO_TYPE` 漏写=静默丢包（坏）。**实施铁律**：把 `heart_demon_offer` / `HeartDemonOffer` 当作已验证范本，`grep -rni 'heart_demon_offer\|HeartDemonOffer' server client proto` 列出它命中的每一处，新变体逐处镜像。命中清单（P0 矿脉 / P2 顿悟各走一遍）：

1. `proto/bong/envelope.proto`：新 message + `ServerDataEnvelope` oneof 新 field（`proto_gen.rs` 由 `build.rs` prost 自动重生，**勿手改** `proto_gen.rs`）
2. `server/src/schema/server_data.rs`：`ServerDataType`（@139 区）+ `ServerDataPayloadV1`（@265 区）+ `ServerDataPayloadWireV1`（@995 区）+ 双向 `From`（wire↔payload，@2100/@2647 区）+ `payload_type()` match（@3179 区）
3. `server/src/network/agent_bridge.rs:59` `payload_type_label()`：新 match 臂 → type 串
4. `server/src/schema/proto_convert.rs:458` `server_data_to_proto_payload()`：穷尽 match 新臂 → `Payload::Xxx(bong::Xxx{…})`（若有反向 proto→payload 同补）
5. server emit system（新文件）+ `network/mod.rs` 注册
6. client `ProtoServerDataBridge.java` `CASE_TO_TYPE.put(PayloadCase.XXX, "xxx")` ← **最易漏的静默丢包点**
7. client `ServerDataRouter.java` `handlers.put("xxx", handler)` + 新 handler 类
8. `agent/packages/schema` TypeBox + sample（双端 round-trip pin）

> **P1 保鲜不新增 S2C 变体**——复用既有 `freshness_update`/`FreshnessUpdateV1`，故 P1 不走上面 1-7 的 S2C 半边，只补 **C2S** 半边（`ClientRequestV1::FreshnessProbe` 同理需走 client_request 的 proto/schema 链）。

---

## P0 — 神识感知矿脉 S2C 回执 + 触发 + 显示

**目标**：玩家（凝脉+）右键矿块 → server 已有 resolver 算出矿名 + 剩余储量 → 经新 S2C 回执 → client actionbar 显示「『赤铜矿脉』灵脉 · 余 23 缕」+ 一声神识轻鸣。Denied 各分支给对应灰字提示。

### 交付物（可 grep 核验）

- **schema/proto 链**（按上节清单走完整 8 步）：
  - `MineralProbeResultV1`（`server/src/schema/server_data.rs`）：扁平化 `MineralProbeResult`——`{kind: "found"|"denied", mineral_id?: String, remaining_units?: u32, display_name_zh?: String, denial_reason?: String}`；`denial_reason` 取 `MineralProbeDenialReason` 5 变体 snake_case（`realm_too_low`/`out_of_range`/`not_mineral_ore`/`stale_ore_index`/`mineral_not_registered`）
  - `ServerDataType::MineralProbeResult` + `payload_type_label` → `"mineral_probe_result"`
  - `proto/bong/envelope.proto`：`MineralProbeResult` message + oneof field
- **emit system**：`server/src/network/mineral_probe_emit.rs` 的 `fn emit_mineral_probe_results(mut clients: Query<(Entity,&mut Client),With<Client>>, mut responses: EventReader<MineralProbeResponse>)`——镜像 `emit_heart_demon_offer_payloads`：读事件→建 `ServerDataV1::new(ServerDataPayloadV1::MineralProbeResult(..))`→`serialize_server_data_payload`→只发给 `ev.player` 对应 entity。`network/mod.rs` 注册到 `Update`。
- **client 触发**：新 mixin `MixinClientPlayerInteractionManagerMineralProbe`（镜像既有 `MixinClientPlayerInteractionManagerAlchemy` 的 use-on-block 钩子），右键矿块且手持空手/神识焦点时调 `ClientRequestSender.sendMineralProbe(x,y,z)`（callsite 当前为 0）。
- **client 显示**：`com/bong/client/network/MineralProbeResultHandler.java` + `ProtoServerDataBridge.CASE_TO_TYPE`（`MINERAL_PROBE_RESULT`）+ `ServerDataRouter.handlers.put("mineral_probe_result", …)`。
- **agent schema**：`MineralProbeResultV1` TypeBox + 正反 sample。

### 视听规格（`docs/CLAUDE.md` §四 强约束 · 矿脉是**新增**可感知面）

- **Found 显示**：actionbar overlay 文本（`InGameHud.setOverlayMessage`，hotbar 上方），格式 `「{display_name_zh}」灵脉 · 余 {remaining_units} 缕`，按丰度上色：`>50` → `#6EE7B7`（青），`10–50` → `#FCD34D`（琥珀），`<10` → `#F87171`（赤）。停留 ~60 tick 自带淡出。
- **Denied 显示**：actionbar 灰字 `#9CA3AF`，按 reason：`realm_too_low`→「神识未及，凝脉方可感矿脉」；`out_of_range`→「神识探之不及」；`not_mineral_ore`→「此处并无灵脉」；`stale_ore_index`/`mineral_not_registered`→「灵脉模糊，难以辨形」。
- **音效**：Found → vanilla `block.amethyst_block.chime` pitch 1.4 / volume 0.3 / delay 0（清越神识轻鸣）；Denied → `block.note_block.bass` pitch 0.6 / volume 0.2（一声低钝）。
- **粒子（可选增强，不阻塞 P0）**：Found 时矿块顶面 6× `minecraft:enchant`（复用，**不新增贴图**），20 tick 上浮。核心反馈是 actionbar+chime，粒子缺失不算未完成。
- **HUD/动画/narration**：无新 HudRenderLayer、无玩家动画、无 narration（神识掠过是即时反馈，不惊动天道）。

### 测试（饱和化 · `docs/CLAUDE.md` Testing）

- `mineral_probe_emit::*` ≥6：Found→payload 字段对拍；5 个 `MineralProbeDenialReason` 各一条 denied 映射；只发给触发玩家、旁观者 0 包（镜像 heart_demon `*_is_sent_only_to_tribulator`）；`MineralProbeResultV1` proto/wire round-trip。
- schema sample pin（正 + 负各 1）。
- `payload_type_label_matches` 扩一条：`ServerDataType::MineralProbeResult` → `"mineral_probe_result"`（防 wire/label 不一致）。

---

## P1 — 神识感知保鲜 C2S 触发 + S2C 回执（复用 freshness_update）

**目标**：玩家检视背包内灵草/丹药（凝脉+）→ 发 `FreshnessProbe{slot}` → server 已有 `resolve_freshness_probe_intents` 算出保鲜快照 → emit 复用既有 `freshness_update` 链 → client `FreshnessStore`/`FreshnessTooltipHook`（已就绪）在 tooltip 显示保鲜%。

### 交付物（可 grep 核验）

- **C2S 链**：`ClientRequestV1::FreshnessProbe{slot}`（`server/src/schema/client_request.rs` + 对应 proto/client_request 链）；`client_request_handler.rs` 新 match 臂——按 slot 查 inventory instance_id，构造 `FreshnessProbeIntent{player, instance_id, issued_at_tick: <当前 tick>}` 并 `send_event`。
- **client 触发**：`encodeFreshnessProbe(slot)`@`ClientRequestProtocol.java` + `sendFreshnessProbe(slot)`@`ClientRequestSender.java` + 检视手势（见 §8.1 决议）。
- **emit system**：`server/src/network/freshness_probe_emit.rs` 的 `fn emit_freshness_probe_results(EventReader<FreshnessProbeResponse>, clients, inventories)`：
  - `ProbeResult::Precise{current_qi, initial_qi, ..}` → `FreshnessUpdateV1{ item_uuid: instance_id.to_string(), freshness: (current_qi / initial_qi).clamp(0.0,1.0), profile_name: <从该玩家 inventory 按 instance_id 反查 freshness.profile> }`。**注意**：`ProbeResult::Precise` **不带** `profile_name`，必须在 emit 时用 inventory query 反查物品 NBT 的 profile（`resolve_one_probe` 已示范同款 inventory lookup），不可凭空填。
  - `ProbeResult::Denied{reason}`：不发 `freshness_update`（避免污染 store），改发一条 actionbar 灰字提示（同 P0 Denied 风格，reason 文案：`realm_too_low`/`item_not_found`/`no_freshness`/`profile_not_registered`）。
  - 经既有 `"freshness_update"` type 串发出，**不新增 client handler**（`ProcessingServerDataHandler`@`ServerDataRouter.java:186` 已 `FreshnessStore.upsert`）。
- **agent schema**：`FreshnessProbe` C2S TypeBox + sample。

### 视听规格

- 保鲜显示**复用既有** `FreshnessTooltipHook`（已建），无新视觉资产。仅 Denied 走 P0 同款 actionbar 灰字。触发音效：`block.amethyst_block.chime` pitch 1.2 / volume 0.25（与矿脉同族略低，区分"物"与"矿"）。

### 测试 ≥7

- `freshness_probe_emit::*`：每个 `DecayTrack` 一条 Precise→`FreshnessUpdateV1` 字段映射（重点验 `freshness=current_qi/initial_qi` 与 `profile_name` 反查正确）；4 个 `ProbeDenialReason` 各一条 → 不发 freshness_update、改发 actionbar；client 不存在时 no-send；`item_uuid` 用 instance_id。
- `client_request` schema pin（`FreshnessProbe` 正反 sample）；handler 臂：合法 slot→`FreshnessProbeIntent`，越界 slot→拒绝不 panic。

---

## P2 — 修炼顿悟 InsightOffer S2C 桥（突破/濒死/炼器）

**目标**：玩家突破/濒死/炼器触发顿悟 → server 两路 `InsightOffer` 事件 → 新 S2C 桥 → client `InsightOfferStore.replace()` → `InsightOfferScreen` 弹出抉择面板（HeartDemon 路径已证此 UI 可用）。

### 交付物（可 grep 核验）

- **schema/proto 链**（走 §「proto 链清单」8 步；**复用** `InsightOfferV1`@`schema/cultivation.rs`，不新建 schema 形状）：
  - `ServerDataPayloadV1::InsightOffer(InsightOfferV1)` + wire 臂 + 双向 From + `payload_type()` + `payload_type_label` → **`"insight_offer"`**（与 `heart_demon_offer` 区分，**类型串绝不撞**）
  - `proto/bong/envelope.proto`：`InsightOffer` message（镜像 `InsightOfferV1` 字段 `offer_id/trigger_id/character_id/choices[]`）+ oneof field
- **emit system**：`server/src/network/cultivation_insight_offer_emit.rs` 的 `fn emit_cultivation_insight_offers(EventReader<InsightOffer>, clients, players)`——镜像 heart_demon emit。映射 `InsightOffer{entity, trigger_id, choices}` → `InsightOfferV1`：`character_id` 由 `entity` 反查玩家 character_id（player 组件 query）；`choices` 由域 `InsightChoice` → `InsightChoiceV1{category, effect_kind, magnitude, flavor_text, narrator_voice, alignment?, cost_*?}`。**两路 producer（`insight_flow.rs:212` fallback + `network/mod.rs:2263` agent-fed）共用同一 `InsightOffer` 事件队列，单个 EventReader 一并排空**。
  - `network/mod.rs` 注册。
- **client handler**：`com/bong/client/network/InsightOfferHandler.java` + `ProtoServerDataBridge.CASE_TO_TYPE`（`INSIGHT_OFFER`）+ `ServerDataRouter.handlers.put("insight_offer", …)`。
  - ⚠️ **不是** `HeartDemonOfferHandler` 的盲拷：`InsightChoiceV1` 形状不同（无 `choice_id`/`title`/`effect_summary`/`style_hint`，有 `effect_kind`/`magnitude`/`flavor_text`/`cost_*`）。handler 须把 `InsightChoiceV1` 合成进 `InsightOfferViewModel`/`InsightChoice`：`choiceId="insight_choice_"+i`；`title` 由 `category`+`effect_kind` 派生；`effect_summary` 由 `effect_kind`+`magnitude`(+`cost_*`) 组词；`flavor=flavor_text`；`alignment=InsightAlignment.parse(alignment)`。`InsightOfferV1` 不带 `trigger_label`/`realm_label`/`composure`/`quota_*`，用 `HeartDemonOfferHandler` 同款 `fallback(...)` 默认值（顿悟语境文案，如 `"顿悟临身"` / `"道机一现"`）。
- **agent schema**：`InsightOfferV1` sample（已有 struct，补 sample）+ ServerData union pin。

### 视听规格

- 顿悟面板**复用既有** `InsightOfferScreen`（已建），无新视觉资产。仅需开屏音效（与 heart_demon 区分顿悟的"喜"vs心魔的"危"）：`block.beacon.activate` pitch 1.0 / volume 0.4 + `entity.player.levelup` pitch 1.2 / volume 0.3（道机一现的清亮上扬），随 store 触发 screen 时播放（client 侧 `InsightOfferScreenBootstrap` 打开钩子内）。
- narration（可选）：顿悟开屏时天道一句（scope=player，style=narrative），示例：①「这一刻，天地为你停了半息——你看见了三条岔路。」②「道机一现，错过便是错过。」③「真元在窍穴里打转，等你一句话。」（仅当 agent narration 已订阅 insight 通道时生效；非本 plan 强制交付，列此供后续接线参考。）

### 测试 ≥6 + e2e

- `cultivation_insight_offer_emit::*`：单 offer→payload 字段对拍；多 choice round-trip（含 `cost_*` Optional 字段在/缺两态）；只发给 `ev.entity` 对应玩家、旁观者 0 包；空 choices 守卫（不发或发 noop）；fallback 路径与 agent-fed 路径**两路都被同一 reader 排空**各一条。
- **e2e**（镜像 heart_demon e2e）：`InsightOffer` 事件 → emit → 解 payload → 断言 `InsightOfferStore` 被填（client 侧若有测试夹具）/ 或 server 侧断言 `"insight_offer"` 类型串经 `payload_type_label` 正确产出且不等于 `"heart_demon_offer"`。
- 类型串隔离 pin：`insight_offer` ≠ `heart_demon_offer`，两 handler 互不串台。

---

## §8 开放问题（P0 决策门前需收口）

- **#1 矿脉结果显示面**：actionbar overlay vs 聊天行 vs 物品 tooltip。producer payload（矿名+剩余）三者皆可承载，但当前无任何矿脉显示 widget，是真 UX 岔路。
- **#2 探针触发手势**：矿脉=世界方块 / 保鲜=背包槽，目标域不同。专用键位+raycast vs 复用神识焦点物 vs 右键目标。`encode*` 已定，差"哪个输入手势触发"。

## §8.1 决议（pre-P0 收口，2026-06-07）

### #1 矿脉结果显示面

**决议**：
1. 采用 **actionbar overlay 文本**（`InGameHud.setOverlayMessage`），非聊天行（不刷屏）、非 tooltip（矿块无 tooltip 宿主）。
2. Found/Denied 文案 + 上色 + 停留时长见 P0 视听规格；实现走 `MineralProbeResultHandler`。
3. 拒绝聊天行：神识感知是高频即时动作，聊天行会污染历史；拒绝 tooltip：世界方块非物品栏对象，无 tooltip 渲染上下文。

**落点**：`client/.../network/MineralProbeResultHandler.java`（新建，依据 P0 交付物）/ plan P0 视听规格。

### #2 探针触发手势

**决议**：
1. **矿脉**：use-on-block mixin（`MixinClientPlayerInteractionManagerMineralProbe`，镜像 `MixinClientPlayerInteractionManagerAlchemy` 的 use-interaction 钩子），右键矿块、手持空手或神识焦点物时触发 `sendMineralProbe(x,y,z)`。server 侧 `MineralProbeDenialReason::NotMineralOre` 已兜非矿块，client 不预判矿种。
2. **保鲜**：背包槽**检视手势**——在 inventory screen 内对槽位按住默认键（沿用 owo-lib/既有 inspect 约定；若无既有约定则 Shift+右键槽位）触发 `sendFreshnessProbe(slot)`。目标是 inventory slot index，非世界坐标。
3. 拒绝"统一一个键位"：两者目标域（世界方块 vs 背包槽）天然不同，强行统一反而增加歧义。

**落点**：`client/.../mixin/MixinClientPlayerInteractionManagerMineralProbe.java`（新建，依据 P0）/ `client/.../ClientRequestSender.java`（`sendFreshnessProbe`，依据 P1）/ plan P0·P1 交付物。

> §8 原表保留作历史回溯；**实施时以 §8.1 决议为准**。

---

## §10 实施工作流（consume-plan 按此执行）

### §10.1 多 PR 序列化（3 PR，依赖顺序）

1. **PR-1 = P0**（矿脉）：首个走完整 proto 链的变体，确立"新 ServerDataPayloadV1 变体 8 步清单"的可复制范式。独立 merge 后再开 PR-2。
2. **PR-2 = P1**（保鲜）：复用 `freshness_update`，只补 C2S 半边，最轻。
3. **PR-3 = P2**（顿悟）：触 cultivation schema + 与 HeartDemon 形状区分，最需小心，放最后。

每 PR 自带饱和测试全绿方可合；前一个 merge/收敛后才开下一个。

### §10.2 实施铁律

- 每个新 `ServerDataPayloadV1` 变体 **必走 §「proto 链清单」8 步**，并以 `grep -rni 'heart_demon_offer\|HeartDemonOffer' server client proto` 自检命中清单逐处镜像。client `CASE_TO_TYPE` 漏写=静默丢包，P0/P2 各加一条"类型串经 `payload_type_label` 产出正确"的 pin 测试兜底。
- emit system 逐字镜像 `tribulation_heart_demon_offer_emit.rs`（只发给触发者、序列化错误 `log_payload_build_error` 后 continue）。
- 纯逻辑接线，**不适用** 3 轮 NBT 打磨 / `<PROMISE>`（无建筑/layout/复杂视觉资产）。
- qi_physics 守恒**不涉及**（只读传输），无需 `QiTransfer`。

### §10.3 子 agent 与 CR 等待

- 每 PR 起独立 subagent（`subagent_type: "claude"`, `model: "opus"`, prompt 末 `ultrathink`），主线只收 result。
- CR/Pi 等待走 `ScheduleWakeup`（1200s/回合，≤3 回合），修完意见**重新等 re-review**，不自判通过（对齐 `feedback_wait_coderabbit_approve`）。

### §10.4 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan` 后即可下班；醒来看本 plan 是否已迁入 `docs/finished_plans/`（全 3 PR P0-P2 ✅ + 下方 Finish Evidence 填毕）。

---

## Finish Evidence

实施于 2026-06-07，单次 `/consume-plan exploration-probe-return-v1` 全自动消费（worktree `auto/plan-exploration-probe-return-v1`，17 atomic commits + 设计收口 + 3 维 opus 对抗审查 + 2 轮 fix）。

### 落地清单

**P0 神识感知矿脉 S2C 回执（✅）**
- proto：`proto/bong/envelope.proto` — `MineralProbeResult` message + `ServerDataEnvelope` oneof field
- server schema：`server/src/schema/server_data.rs` — `ServerDataType::MineralProbeResult` + `ServerDataPayloadV1::MineralProbeResult(MineralProbeResultV1)` + Wire 变体 + 双向 From + `payload_type()` + `agent_bridge.rs` `payload_type_label`→`"mineral_probe_result"` + `proto_convert.rs` 穷尽 match
- server emit：`server/src/network/mineral_probe_emit.rs`（读 `EventReader<MineralProbeResponse>`，只发触发者；含 ≥6 单测）+ `network/mod.rs` 注册
- client：`MineralProbeResultHandler.java`（actionbar overlay：Found 按丰度上色 #6EE7B7/#FCD34D/#F87171 + `amethyst_block.chime`，Denied 灰字 per 5 reason + `note_block.bass`）+ `MixinClientPlayerInteractionManagerMineralProbe.java`（右键矿块→`sendMineralProbe`）+ `ProtoServerDataBridge` CASE_TO_TYPE + `ServerDataRouter` 路由 + `bong-client.mixins.json`
- agent schema：`server-data.ts` `ServerDataMineralProbeResultV1` + 正反 sample + `schema.test.ts` pin

**P1 神识感知保鲜 C2S 触发 + S2C 回执（✅）**
- C2S：`server/src/schema/client_request.rs` `ClientRequestV1::FreshnessProbe { instance_id }`（**经审查从 slot 改为 instance_id**，镜像 ApplyPill 约定，消除多容器 tab 歧义）+ proto + round-trip pin；`client_request_handler.rs` 臂（用 canonical `inventory_item_by_instance_borrow` 校验归属 containers+equipped+hotbar→`FreshnessProbeIntent`）
- client：`ClientRequestProtocol.encodeFreshnessProbe(instanceId)` + `ClientRequestSender.sendFreshnessProbe` + `InspectScreen` Shift+右键槽位→`sendFreshnessProbe(item.instanceId())`
- server emit：`server/src/network/freshness_probe_emit.rs`（`ProbeResult::Precise`→`FreshnessUpdateV1`：freshness=current_qi/initial_qi clamp，profile_name 由 instance_id 反查；复用既有 `"freshness_update"` 类型串，无新 client handler；Denied 走 actionbar EventAlert）
- agent schema：`client-request.ts` `FreshnessProbeRequestV1` + sample

**P2 修炼顿悟 InsightOffer S2C 桥（✅）**
- server：`ServerDataPayloadV1::InsightOffer(InsightOfferV1)`（复用 `schema/cultivation.rs` 既有 InsightOfferV1）走整条 proto 链，type 串 `"insight_offer"`（与 `heart_demon_offer` 隔离）；`server/src/network/cultivation_insight_offer_emit.rs`（单 `EventReader<InsightOffer>` 一并排空 `insight_flow.rs:212` fallback + `network/mod.rs:2263` agent-fed 两路；character_id 由 entity 反查）
- client：`InsightOfferHandler.java`（`InsightChoiceV1`→`InsightOfferViewModel` 合成 deriveTitle/deriveEffectSummary/categoryZh，fallback 默认）+ `InsightOfferScreenBootstrap` 开屏 SFX `beacon.activate`(0.4/1.0)+`player.levelup`(0.3/1.2) + CASE_TO_TYPE + ServerDataRouter
- agent schema：`server-data.ts` `ServerDataInsightOfferV1` + 修正 sample（去 v 包装、真实 effect_kind、省略缺省 optional）+ pin

**跨切关联**：`EventKind::Generic`（freshness Denied 走 EventAlert）三端对齐——server serde `"generic"` + proto `EVENT_KIND_GENERIC=11` + agent TypeBox `common.ts` `"generic"`。

### 关键 commit（worktree `auto/plan-exploration-probe-return-v1`，2026-06-07）

- `82098bf18` P0 MineralProbeResultV1 整条 proto 链 + emit system
- `65c6dc322` P0 client handler + mixin + agent schema 整条链路
- `b73cc9d2a` P1 神识感知保鲜 C2S 触发 + S2C 回执
- `d56963a60` / `a779c2627` / `0e6953c5d` P2 InsightOffer proto 链 + emit / client S2C 桥 / schema sample
- `132703ce0` / `77054f7dd` fix(D) FreshnessProbe slot→instance_id 消歧 + round-trip pin
- `3c5e4ad50` test(C) FreshnessProbe handler 测试 ；`512418ce8` fix(A) ServerDataInsightOfferV1 + sample pin ；`9ea5524af` fix(E) EventKind "generic" 对齐 ；`e88fa8297` feat(F) 顿悟开屏 SFX
- `eb98aabf4` fix(server) FreshnessProbe gate 扩至 containers+equipped+hotbar 对齐 resolver
- `e4075cd24` test(client) MineralProbeResultHandler/InsightOfferHandler/encodeFreshnessProbe 饱和测试

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → **7371 passed, 0 failed**（含 mineral/freshness/insight emit 各饱和单测 + FreshnessProbe handler 5 测 + client_request round-trip pin）
- `cd agent && npm run build && (cd packages/schema && npm test)` → **544 passed**（含 insight_offer/mineral sample union pin + FreshnessProbe C2S + EventKind generated 快照）
- `cd client && ./gradlew test build` → **2076 tests, 1 failed**（唯一失败 `BongEntityModelAssetTest.blockbenchSourcesExistForEveryGameEntity` 系 pre-existing：gitignored `local_models/*.bbmodel` 缺失，本 PR 未触及任何 entity-model 资产，origin/main 同样失败）。新增 client 测试 44 例（MineralProbeResultHandlerTest 22 / InsightOfferHandlerTest 17 / ClientRequestProtocolTest 5）全绿

### 跨仓库核验

- **server**：`ServerDataPayloadV1::{MineralProbeResult, InsightOffer}`、`ClientRequestV1::FreshnessProbe{instance_id}`、`EventKind::Generic`、emit systems `{mineral,freshness,cultivation_insight_offer}_probe_emit` / `_emit`
- **agent**：`ServerDataV1` union + `ServerDataMineralProbeResultV1` / `ServerDataInsightOfferV1` / `FreshnessProbeRequestV1`、`EventKind` `"generic"`
- **client**：type 串 `mineral_probe_result` / `insight_offer`（新 handler）+ `freshness_update`（复用）；`ProtoServerDataBridge` PayloadCase `MINERAL_PROBE_RESULT` / `INSIGHT_OFFER`
- **proto**：`proto/bong/envelope.proto` `MineralProbeResult` / `InsightOffer` / `FreshnessProbe` message + oneof

### 遗留 / 后续

- qi_physics 不涉及（三条均只读传输，无 `QiTransfer`，符合守恒律红线）。
- 顿悟 narration（plan §P2 可选项）未接：需 agent 端订阅 insight 通道后产出个性化文案，属 `agent-observation-feeds` 主题（断链审计 runner-up，待立 plan）范围。
- `FreshnessProbe.slot`→`instance_id` 的容器 tab 修复同时覆盖了原 plan §P1「slot」措辞——以 instance_id 为最终契约。
