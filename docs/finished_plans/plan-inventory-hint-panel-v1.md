# plan-inventory-hint-panel-v1 — 库存操作拒绝原因警示面板

**主题**：背包/装备界面里很多操作被 server 静默拒绝（拖入弹回），玩家不知道**为什么**。把 server 侧现在只写进 WARN 日志、从不下发 client 的拒绝原因（境界不足 / worn cap 满 / 手持互斥 / 双手锁定 / 分类不符 等）结构化后下发，用**失败 toast + tab/槽 hover 预警**两条通道显式告诉玩家。源于真机：拖伪皮进胸槽弹回（`realm too low for SpiderSilk`）全是静默回弹，体验差。

> **本 plan 性质**：纯 client UX + 一条新的 server→client 结构化拒绝原因 payload。无 worldview 玩法锚点（UI 辅助），无 qi_physics、无守恒。文案里的境界名走正典映射。

## 阶段总览

| 相位 | 交付物 | 状态 |
|------|--------|------|
| **P0** | server 拒绝原因结构化 —— `InventoryMoveRejectReason` enum + 收敛 `Result` + 境界门控并入 + **新增专属 `InventoryMoveRejectedV1` S2C payload**（proto 双端）+ emit | ✅ 2026-07-03 |
| **P1** | client 失败 toast —— 新增 `InventoryMoveRejectedHandler`（活链路）→ 文案表（境界名走 `RealmLabel`）→ `BongToast.show(String,int,long,long)` | ✅ 2026-07-03 |
| **P2** | tab/槽 hover 预警 —— 照抄 `ItemTooltipPanel`+`updateTooltipFromHover`，覆盖空槽（传槽身份），只覆盖静态规则 | ✅ 2026-07-03 |
| **P3** | 视听规格收尾 —— toast 时长/文字色/前缀、hover 面板位置/配色/淡入淡出、不遮挡拖拽 ghost | ✅ 2026-07-03 |

验收日期：2026-07-03（全相位 consume 完成，7 commit + 博弈 ready + 全绿）。

## 接入面（跨仓库契约，已坐实 + 博弈复核修正）

- **进料**：
  - server `server/src/inventory/mod.rs` 的拒绝路径 —— `validate_move_semantics`（`mod.rs:4664`，现返回 `Result<(), String>`）/ `apply_inventory_move`（`mod.rs:3158`，现返回 `Result<InventoryMoveOutcome, String>`）；伪皮胸槽境界门控是**另一条独立硬编码**路径（`network/client_request_handler.rs:9896-9925`，命中只 `tracing::warn!` + `resync_snapshot`，连 `Err` 都不走）。
  - client tab/槽 hover —— 逐帧轮询 `InspectScreen.updateTooltipFromHover`（`InspectScreen.java:3990-4018`），装备槽分支现走 `hovered = eq.representative()`（空槽返回 null，见 §P2 空槽处理）；owo `mouseEnter().subscribe()` 也可用（`InspectScreen.java:1631-1633` 有拖拽悬停切 tab 先例）。
- **出料**：
  - **新增专属 S2C payload `InventoryMoveRejectedV1`**（不复用泛型 EventAlert，见 §8.1 #2 决议），结构化字段 `{reason: String(tag), required_realm: Option<String>, slot: Option<String>, cap: Option<u32>}`。走 `ServerDataPayloadV1::InventoryMoveRejected` variant，模式**照抄 `MineralProbeResultV1`**（`server/src/network/mineral_probe_emit.rs:24` `ServerDataV1::new(ServerDataPayloadV1::MineralProbeResult(v1))`）。
  - client 侧**新增 `InventoryMoveRejectedHandler`**（实现 `ServerDataHandler`，经 `ServerDataRouter` 注册 —— 与活的 `client/network/*Handler` 同族），消费 → 组装文案 → `BongToast.show(String,int,long,long)`（`hud/BongToast.java:72-77` 活重载）。**⚠️ 勿引用死代码**：`BongEventAlertOverlay` + 非 network 包的 `client/EventAlertHandler.java` 都挂在**零调用方**的 `BongServerPayloadRouter` 下（全仓 grep 无调用 = 死代码）。
- **共享类型 / 复用（博弈实地核验后的真实映射）**：仓库拒绝原因先例的落地通道各不相同——`ProbeDenialReason` 走 `EventAlert`（`freshness_probe_emit.rs:72`，唯一走 EventAlert 的）/ `MineralProbeDenialReason` 走**专属** `ServerDataPayloadV1::MineralProbeResult`（`mineral_probe_emit.rs:24`）/ `CastRejectReason` 走 `CastSyncV1`（`client_request_handler.rs:9440/9456`，注释「不新增 S2C 变体」指复用 CastSync 通道）。**惯例 = 拒绝原因走专属结构化 payload**，故本 plan 新增 `InventoryMoveRejectedV1` 专属 payload（结构化字段可饱和断言），而非塞泛型 `EventAlert` 自由文本 message。新 `InventoryMoveRejectReason` enum 仿 `CastRejectReason`（`cultivation/skill_registry.rs:20-34`）/ `MineralProbeDenialReason` 结构。
- **跨仓库契约 symbol**：`InventoryMoveRejectReason`（server 内部 enum）；`InventoryMoveRejectedV1` / `ServerDataPayloadV1::InventoryMoveRejected`（**新 wire payload，proto envelope.proto + proto_convert.rs + agent/packages/schema/samples 双端**，走 `reference_server_data_payload_field` 六点流程）；`ProtoServerDataBridge.CASE_TO_TYPE` 加映射 + typeString 常量；client `InventoryMoveRejectedHandler` / `BongToast.show(String,int,long,long)` / `RealmLabel.displayName`。境界 tag 跨端：server `realm_to_string`（`schema/cultivation.rs:321-330`，输出 `"Condense"` 等英文 tag）↔ client `RealmLabel.displayName`（`util/RealmLabel.java:9-26`，英文 tag → 醒灵/引气/凝脉/固元/通灵/化虚）。
- **worldview 锚点**：无玩法正典依赖（UI 辅助）。按需显示、不常驻（[[feedback_hud_immersive_minimal]] / [[feedback_hud_conditional]]）；文案境界名用正典（[[feedback_worldview_canonical]]）。
- **qi_physics 锚点**：无（纯 UI + 一条拒绝原因下发，不碰真元/守恒）。
- **红旗自检**：不自产自消（接 server 拒绝路径 + client HUD）；不新增守恒常数；跨端新增一条专属 payload（双端 sample 对齐）+ client handler。

---

## P0 — server 拒绝原因结构化 + 新增专属 payload ✅ 2026-07-03

**目标**：把当前散落成裸 `String` + 只进日志的拒绝原因收敛成 typed enum，经一条**专属结构化 payload** 下发 client。

**交付物 / 抓手**：
- 新增 `server/src/inventory/` 内 `enum InventoryMoveRejectReason`（仿 `CastRejectReason` @ `cultivation/skill_registry.rs:20-34`），变体覆盖实测拒绝分支（`mod.rs` 内）：
  - `WornStackNotTop`（`mod.rs:4693`）、`ForbiddenInHotbar{category}`（`mod.rs:4708-4746` 6 类）、`PackDetached`（`mod.rs:4777`）、`HeldWornMismatch`（`mod.rs:4815`）、`EquipCategoryMismatch`/`OffHandTypeMismatch`（`mod.rs:4842/4851`）、`HandOccupied`/`TwoHandedLocksOther`（`mod.rs:4866/4883/4903`）、`ArmorDurabilityZero`（`mod.rs:4923`）、`ArmorSlotMismatch`（`mod.rs:4938`）、`PackEquipSlotMismatch`（`mod.rs:4950`）、`WornCapFull{slot, cap}`（`mod.rs:4967`）、`TargetOutOfBounds`/`TargetOccupied`（`mod.rs:4535/4614/4623` 等）
  - `RealmTooLow{required_realm}` —— **把 `client_request_handler.rs:9896-9925` 那条独立硬编码境界门控并入**（目前最大技术债：它连 `Result` 都不走）。`required_realm` 存 `realm_to_string` 英文 tag（如 `"Condense"`）。
- `validate_move_semantics` / `apply_inventory_move` 的 `Result<_, String>` 收敛成 `Result<_, InventoryMoveRejectReason>`（`?` 传播链一并改）。保留 `reason.to_log_string()` 供既有 `tracing::warn!` 继续打日志。
- **新增专属 wire payload `InventoryMoveRejectedV1`**（走 `reference_server_data_payload_field` 六点双端流程）：
  - `proto/bong/envelope.proto`：新增 `InventoryMoveRejectedV1` message（`reason` string tag / `required_realm` optional string / `slot` optional string / `cap` optional uint32）+ 挂进 `ServerDataEnvelope` oneof payload；`schema/server_data.rs`：`ServerDataPayloadV1::InventoryMoveRejected` variant + wire 版本；`proto_convert.rs`：内部 → proto From（穷举 match 补全）；`agent/packages/schema/samples/*.json` 双端 sample。
  - **wire 形状安全**（吸取 `plan-wire-format-bridge-v1` 教训）：`reason` 用 **string tag**（不用 proto enum，避免枚举前缀 noOp）；`cap` uint32（< Long.MAX_VALUE，桥层 `normalizeNumericStrings` 自动归一）；无坐标字段。
  - `ProtoServerDataBridge.CASE_TO_TYPE` 加 `INVENTORY_MOVE_REJECTED → "inventory_move_rejected"` 映射；`ServerDataRouter` 注册新 typeString → `InventoryMoveRejectedHandler`。
- emit：`client_request_handler.rs` 的 `Err(reason)` 分支（`:10105-10119`）+ false_skin 境界门控分支（`:9914`）旁，除 `resync_snapshot` 外 emit 一条 `InventoryMoveRejectedV1`（reason.to_wire_tag() + required_realm 等）。
- **测试声明**：`inventory::` 每个 `InventoryMoveRejectReason` 变体一条专属 case（构造触发条件 → 断言 `apply_inventory_move` 返回该变体）；下发链契约测：拒绝 → 断言 emit 了带正确 `reason`/`required_realm`/`cap` 字段的 `InventoryMoveRejectedV1`；proto round-trip sample 对拍；境界门控并入后：伪皮胸槽拒绝 → 断言走 enum + emit payload 而非旧 warn-only。
- **⚠️ 既有测试迁移（列入工作量）**：收敛 `Result<_, String>` → enum 会让 `server/src/inventory/mod.rs` 里现有 **~72 处** `error.contains("...")` / `err.contains("...")` 字符串断言（如 `:8373/8491/8516/8541/8569`）全部失效，须改写为按枚举变体断言（`assert!(matches!(err, InventoryMoveRejectReason::ArmorSlotMismatch))`）。这批迁移是 P0 交付物的一部分。

---

## P1 — client 失败 toast ✅ 2026-07-03

**目标**：拒绝发生时飘一条红色警示,把原因用正典语感文案告诉玩家。

**交付物 / 抓手**：
- **新增 `client/network/InventoryMoveRejectedHandler.java`**（实现 `ServerDataHandler`，经 `ServerDataRouter` 注册），**模式照抄活的 mineral probe result handler**（同族 network handler → dispatch 落地）。**⚠️ 活链路唯一正解**：handler 解析 payload → 组装文案 → `BongToast.show(String, int, long, long)`（`hud/BongToast.java:72-77` 重载）。**不经过** `toastText/toastColor(NarrationState)`（`:160-176`，那条只被 `show(NarrationState,...)` narration 管线触达）；**不引用** `BongEventAlertOverlay` / `client/EventAlertHandler.java`（死代码）。
- 文案表覆盖全部 reject 变体：worn cap 满（带 slot+cap 数值）/ 手持互斥 / 双手锁定 / 分类不符 / 护甲耐久 0 / **境界不足**。
- **境界名走正典**：`reason=="realm_too_low"` 时用 `RealmLabel.displayName(required_realm)`（`util/RealmLabel.java:9-26`）现场把英文 tag 转 "凝脉" 等中文,**不在 server 端硬编中文**（避免 server/client 措辞漂移）。
- **前缀 + 颜色由 client 组装**：`show(String,int,...)` 直接显示传入文本 → "天道警示：" 前缀在 handler 组装文案字符串时手动拼（不依赖 NarrationState 样式）；颜色由 `int` 参数传（见 §P3）；时长由 `durationMillis` 参数传（不依赖 EventAlert 的 severity/duration_ticks 机制）。
- **测试声明**：client 单测——每个 reason → 断言 toast 文案正确（尤其境界不足 → "……凝脉……" 而非英文 tag；worn cap 满 → 带 slot/cap 数值）；契约测断可观察文案 + 传入 BongToast 的 color/duration 参数,不绑渲染内部。

---

## P2 — tab/槽 hover 预警 ✅ 2026-07-03

**目标**：hover inventory tab / 装备槽时主动显示该容器/槽约束,不必等失败才提示。

**交付物 / 抓手**：
- 照抄 `ItemTooltipPanel`（`inventory/component/ItemTooltipPanel.java`，固定宽 196px、`computeRequiredHeight` 动态高）+ `updateTooltipFromHover`（`InspectScreen.java:3990-4018`）的轮询式 hover 检测框架，新增一块 "约束说明" 区。
- **⚠️ 空槽场景（核心价值，博弈补）**：hover 装备槽预看 cap/可装类别的价值恰在"槽是空的、还没触发失败"时。但 `updateTooltipFromHover` 现走 `hovered = eq.representative()`（空槽 `representative()` 返回 null → 不出面板）。必须额外把 `EquipSlotComponent.slotType()`（槽身份，与是否有物品无关，`EquipSlotComponent.java:57`）传给约束面板；`ItemTooltipPanel.setHoveredItem(InventoryItem)`（`ItemTooltipPanel.java:48`）现签名只接 item，需扩一个槽身份通道（重载 / 新方法），否则做出的是"只在槽已占用/已满时才提示"的缩水版。
- **边界（防 hover 说谎）**：P2 hover **只覆盖静态规则**——`worn_cap` 是纯常量（`mod.rs:636-643`，Head/Feet=2、Chest/Legs=3），可 client 镜像一份；但 `worn_cap_bonus`（`mod.rs:659-661`，P5 占位，未来由境界/功法派生）一旦启用无法纯 client 推算 → 动态加成类拒绝**仍走 P1 toast**，hover 文案不承诺动态结果。
- 避 owo `Sizing.fill(100)` 顶飞（[[feedback_owo_fill_overflow]]）：面板走绝对定位 / `Positioning.relative`（`EquipmentPanel.java:16-36` 已因这条教训改绝对坐标表 `SLOT_LAYOUT`）。
- **测试声明**：client 单测——hover 各槽（含空槽）→ 断言约束文案正确;worn_cap 满的槽 hover → 显示 "已满" 且与 P1 toast 文案一致。

---

## P3 — 视听规格收尾 ✅ 2026-07-03

**HUD / 屏幕效果规格**（内联，达可实现精度，博弈复核后修正）：
- **失败 toast**（走 `BongToast.show(String,int,long,long)` 活重载）：
  - **文字**：文案字符串带 "天道警示：" 前缀（client 组装，不依赖 NarrationState 样式）。
  - **文字色**（`int` 参数）：红色告警 `0xFFAA55`（复用 `BongToast.WARNING_COLOR`，`BongToast.java:13`）或自定 `0xFFCC5555`。
  - **背景**：`BongToast` 背景**恒为** `0x88000000`（`BongToast.java:14` `BACKGROUND_COLOR`，`int color` 参数只驱动文字色、不可配背景）——不承诺可配底色。
  - **时长**：`durationMillis=3000`（`show` 的第 4 参，直接控时长）。同一 reason 500ms 内去重防刷屏（handler 内维护 last-shown map）。
  - ~~`0xFFCC3333`~~ **勘误**：该值是 `LootContainerScreen/Panel.TIMER_BAR_URGENT`，与 BongToast 无关，已删。
- **hover 约束面板**：`ItemTooltipPanel` 风格,宽 196px,背景 `0xE0141414`,1px 边框 `0xFF3A3A3A`;约束逐行——满足=绿 `0xFF66BB66`,不满足=红 `0xFFCC5555`;绝对定位贴 hover 目标右侧,超右边界翻转到左侧;**不遮挡拖拽 ghost**（拖拽进行中隐藏 hover 面板）。fade-in 4 tick。
- **narration**：无（纯 UI 提示,不走天道 narration）。

**测试声明**：视听为 client 渲染,以 P1/P2 的文案/结构/参数契约测为准;P3 只调参不新增可断言逻辑。

---

## §8 开放问题（P0 决策门前需收口）

1. 警示走「失败后 toast」还是「hover 预警」还是两者都要？（P1 vs P2 取舍）
2. reject reason 是新 S2C payload 还是塞 inventory_snapshot？
3. 文案与 worldview 语感对齐（境界名用正典：醒灵/引气/…）。

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-02，Explore agent 核查 + sonnet 博弈自检复核修正）

### #1 警示走 toast 还是 hover 还是两者都要？

**决议**：
1. **两者都要,分 P 且优先级不同**。P1 失败 toast 优先——拒绝已真实发生（真机症状 "静默回弹"）。P2 tab/槽 hover 预警作增强（含空槽预看）。
2. **活链路唯一正解**：toast 走 `client/network/InventoryMoveRejectedHandler`（新建，经 `ServerDataRouter` 注册）→ `BongToast.show(String,int,long,long)`（`hud/BongToast.java:72-77`）。**博弈更正**：`BongEventAlertOverlay` + 非 network 包 `client/EventAlertHandler.java` 挂在**零调用方** `BongServerPayloadRouter` 下 = 死代码，勿引用；`toastText/toastColor(NarrationState)`（`:160-176`）的 SYSTEM_WARNING 样式只被 `show(NarrationState)` 触达，inventory 走的 `show(String,int)` 拿不到 → "天道警示：" 前缀由 client 组装文案字符串实现、颜色由 `int` 参数传（见 §P3）。
3. **边界**：hover 只覆盖静态规则（`worn_cap` 常量），动态加成类（`worn_cap_bonus`）仍走 P1 toast,hover 不说谎。空槽预看需传 `EquipSlotComponent.slotType()`（§P2）。

**落点**：`client/network/InventoryMoveRejectedHandler.java`（新建）/ `hud/BongToast.java:72-77`（活重载）/ `inventory/component/ItemTooltipPanel.java:48` + `InspectScreen.java:3990-4018` + `EquipSlotComponent.java:57`（hover）/ `server/src/inventory/mod.rs:636-643,659-661`（cap 常量 vs 动态 bonus 边界）/ plan §P1 §P2。

### #2 reject reason 是新 S2C payload 还是塞 inventory_snapshot？

**决议（博弈修正——原"复用 EventAlert"证据被证伪，改专属 payload）**：
1. **不塞 `inventory_snapshot`**——`InventorySnapshotV1`（`schema/inventory.rs:268-287`）是幂等全量状态镜像,reason 是一次性瞬时事件,塞进去语义别扭 + 竞态歧义。
2. **也不复用泛型 `EventAlert`**。博弈实地核验推翻原论据："EventAlert 是三条先例中两条的载体"为**假**——三条里**只有 `ProbeDenialReason` 走 EventAlert**（`freshness_probe_emit.rs:72`）；`MineralProbeDenialReason` 走**专属** `MineralProbeResultV1`（`mineral_probe_emit.rs:24`）、`CastRejectReason` 走 `CastSyncV1`（`client_request_handler.rs:9440/9456`）。**sibling 系统惯例 = 拒绝原因走专属结构化 payload**，且泛型 EventAlert 的 `message` 是裸 `String`（把 reason+required_realm+slot+cap 塞进去无编码语法、client 只能字符串猜）。
3. **决议：新增专属 `InventoryMoveRejectedV1` S2C payload**（结构化字段 `reason`/`required_realm`/`slot`/`cap`，可饱和断言），模式照抄 `MineralProbeResultV1`。server 内部 `InventoryMoveRejectReason` enum（仿 `CastRejectReason`）→ `to_wire_tag()` → payload。走 proto envelope.proto + proto_convert.rs + schema samples 双端（`reference_server_data_payload_field` 六点）。client 新增 `InventoryMoveRejectedHandler` 消费。**wire 形状安全**：reason 用 string tag（非 proto enum，避 [[project_wire_format_bridge_audit]] 枚举前缀 noOp）、cap uint32（桥层自动归一）、无坐标。

**落点**：`server/src/inventory/mod.rs:3158,4664`（收敛 Result）/ `network/client_request_handler.rs:9896-9925,10105-10119`（境界门控并入 + Err 旁 emit）/ `proto/bong/envelope.proto` + `schema/server_data.rs` + `proto_convert.rs` + `agent/packages/schema/samples/*.json`（新 payload 双端）/ `network/mineral_probe_emit.rs:24`（emit 范本）/ plan §P0 §P1。

### #3 文案与 worldview 语感对齐

**决议**：
1. **复用现成 `Realm→中文` 链路**,不新造字符串表。server `realm_to_string`（`schema/cultivation.rs:321-330`）输出英文 tag,client `RealmLabel.displayName`（`util/RealmLabel.java:9-26`）转醒灵/引气/凝脉/固元/通灵/化虚。这条链路 `inventory_snapshot.realm` 已在端到端用。
2. `RealmTooLow` 的 payload 只带 `required_realm: "Condense"` 英文 tag,client 现场转中文。**不在 server 端硬编中文文案再下发**（避免措辞漂移）。

**落点**：`schema/cultivation.rs:321-330`（realm_to_string）/ `client/.../util/RealmLabel.java:9-26`（displayName）/ plan §P1。

> **过时点勘误（收口时发现）**：原骨架头部例句 "挪非空背包弹回（`container not empty`）" **已过时**——该拒绝分支被 `plan-tarkov-backpack-v1` P0 删除（`mod.rs:4700-4706` 注释 "此处不再因 pack 容器非空返回 Err"）。本 plan 不含该 reason 变体。

---

## §10 实施工作流

scope ~3-4 PR，单 plan 内序列化（`docs/CLAUDE.md` §六）。

- **§10.1 推荐拆分点**（依赖顺序，前一个 merge 后开下一个）：
  1. **PR-1 P0**：server 拒绝原因结构化（`InventoryMoveRejectReason` enum + 收敛 Result + 72 处单测迁移 + 境界门控并入 + **新增 `InventoryMoveRejectedV1` payload 双端** + emit）。含 proto/schema/samples，独立可 merge。
  2. **PR-2 P1**：client `InventoryMoveRejectedHandler`（活链路）+ 失败 toast + 文案表（境界名走 `RealmLabel`）。依赖 PR-1 的 payload + typeString。
  3. **PR-3 P2+P3**：hover 预警（含空槽 + 槽身份通道）+ 视听规格收尾。依赖 PR-2 的文案表。
- **§10.2 撞车防护**：`fix/inventory-tab-live-refresh`（过期重复分支，功能已由 PR #775 落地）、`inventory-v1`/`atlas/inventory-v1-server-data-closure`（相对 origin/main 零独有提交，死分支）均**不构成冲突**（收口调研已核实）。每 PR 开前仍 `git fetch origin && git log origin/main` 比对 `inventory/mod.rs` / `InspectScreen.java` / `ProtoServerDataBridge.java` / `schema/server_data.rs` 是否被动过。
- **§10.3 测试要求**：P0 每个 reject 变体专属 case + 下发契约测 + proto round-trip sample 对拍 + 72 处 error.contains 迁移全绿；P1/P2 client 文案契约测（断可观察文案 + 传入 BongToast 参数，不绑渲染内部）。饱和覆盖：每个 enum 变体、境界不足特例（转中文）、worn_cap 满边界（slot+cap 数值）、空槽 hover。
- **§10.4 CR 等待**：每 PR `ScheduleWakeup` 1200s × ≤3 回合等 CodeRabbit（[[feedback_wait_coderabbit_approve]]），修完重等 re-review；**CR 限流时**（本仓并行 PR 多，额度常耗尽）按 [[feedback_consume_presubmit_debate]]：threads resolve + 博弈过 + e2e 过即 merge（CR 非 required check）。
- **§10.5 subagent 实施**：每 PR 独立 `claude` subagent（opus + `ultrathink`），主线只收 result + merge（`docs/CLAUDE.md` §6.4）。**每 PR push 前必跑对抗博弈自检**（sonnet 控方/辩方/就绪 → opus 裁决，[[feedback_consume_presubmit_debate]]），辩方干净胜出才 push/merge。
- **§10.6 单次 consume 全自动到 merge**：收口已完成（本 §8.1，含博弈复核修正），`/consume-plan` 即可，醒来看是否入 `finished_plans/`。

## 落地证据链

- 收口调研（2026-07-02，Explore agent 实地核查 + sonnet 博弈自检复核）：server 拒绝路径 `inventory/mod.rs` + 境界门控 `client_request_handler.rs:9896-9925`;拒绝下发**专属 payload 惯例**（`MineralProbeResultV1`/`CastSyncV1`，仅 ProbeDenial 走 EventAlert）;client 活链路 `network/*Handler`→ToastSpec→`BongToast.show(String,int,..)`（死代码 `BongEventAlertOverlay`/非 network `EventAlertHandler` 已排除）;境界映射 `realm_to_string`↔`RealmLabel.displayName`。
- 博弈修正记录：原收口"复用泛型 EventAlert"被控方证伪（EventAlert 仅 1/3 先例在用 + 死代码引用 + message 裸 String 无编码语法）→ 改专属结构化 payload + 活链路。
- 相关先例 plan：`docs/finished_plans/plan-tarkov-backpack-v1.md`（删了 container-not-empty 拒绝）;`plan-mineral-*`（`MineralProbeResultV1` 专属拒绝 payload 范本）;`plan-wire-format-bridge-v1`（wire 形状教训：reason 用 string tag 避枚举前缀）。

---

## Finish Evidence

**验收日期**：2026-07-03（`/consume-plan` 全自动消费：Design→Implement P0-P3→博弈对峙 Verify，7 commit + opus verdict=ready + 全绿）

### 落地清单
- **P0 server 拒绝原因结构化 + 专属 payload**：`server/src/inventory/mod.rs`（`InventoryMoveRejectReason` enum 含 `ArmorSlotUnresolvable`，`apply_inventory_move`/`validate_move_semantics`/`validate_equip_to`/`displaced_at_target`/`validate_attach_fits`/`attach_at_location` 收敛为 `Result<_, InventoryMoveRejectReason>`，`client_request_handler.rs:9896-9925` 伪皮胸槽境界门控并入 `RealmTooLow`）；`server/src/schema/server_data.rs`（`ServerDataPayloadV1::InventoryMoveRejected` variant + `InventoryMoveRejectedV1` struct + 双向 round-trip）；`proto/bong/envelope.proto`（`InventoryMoveRejected` message，oneof 字段 137）；`server/src/schema/proto_convert.rs`（From arm + pin count 125）；`agent/packages/schema/src/server-data.ts`（`ServerDataInventoryMoveRejectedV1` TypeBox + ServerDataV1 union + samples，drift gate 绿）；`server/src/network/inventory_move_rejected_emit.rs`（emit 只发触发者不广播）。
- **P1 client 失败 toast**：`client/.../network/InventoryMoveRejectedHandler.java`（活链路 → `BongToast.show(String,int,long,long)`；文案表覆盖全变体；境界 `RealmLabel.displayName` 现场转中文；500ms 去重）；`ProtoServerDataBridge.java`（`CASE_TO_TYPE` + `extractInner()` switch 两处映射）；`ServerDataRouter.java`（注册 `inventory_move_rejected`）。
- **P2 hover 预警**：`ItemTooltipPanel.java`（「约束说明」区 + 空槽 `slotType` 通道）；`InspectScreen.java`（`updateTooltipFromHover` 接约束）；`EquipSlotComponent.java`（`slotType()`）。只覆盖静态规则（`worn_cap` 常量），动态 `worn_cap_bonus` 仍走 toast。
- **P3 视听**：`ItemTooltipPanel.java`（背板 `0xE0141414` + 1px 边框 + 满足绿/不满足红 + 4-tick 淡入）；toast 走 `BongToast.WARNING_COLOR 0xFFAA55`，背景恒 `0x88000000`。

### 关键 commit（分支 `auto/plan-inventory-hint-panel-v1`）
- `af3476e44`（2026-07-03）feat(inventory): InventoryMoveRejectReason enum 收敛库存移动拒绝路径
- `80edd7151` feat(schema): InventoryMoveRejectedV1 拒绝原因结构化 S2C payload 双端
- `e778346b1` feat(network): emit InventoryMoveRejectedV1 + 伪皮境界门控并入 enum
- `deac28c3f` feat(client): 库存拒绝原因失败 toast — InventoryMoveRejectedHandler 接入 P1
- `228079671` feat(client): 装备槽 hover 约束预警 — ItemTooltipPanel「约束说明」区 P2
- `664be932f` feat(client): hover 约束面板视听规格收尾 P3
- `91e845e5d` fix(inventory): armor 无法解析槽位改用独立 ArmorSlotUnresolvable 变体，杜绝 unknown 占位符泄漏（博弈 major 硬化）

### 测试结果
- **server**：`cargo fmt --check` ✅ + `cargo clippy --all-targets -- -D warnings` ✅ + `cargo test` = **10172 passed / 0 failed / 1 ignored**
- **client**：`./gradlew test build` = **BUILD SUCCESSFUL / 3368 passed / 0 failed**（`InventoryMoveRejectedHandlerTest` 36 tests）
- **agent/schema**：`npm test`（packages/schema）738 passed，`generated-artifacts` drift gate 绿

### 跨仓库核验
- **server**：`InventoryMoveRejectReason`（+`ArmorSlotUnresolvable`）、`InventoryMoveRejectedV1`、`ServerDataPayloadV1::InventoryMoveRejected`、`inventory_move_rejected_emit`
- **agent**：`ServerDataInventoryMoveRejectedV1` TypeBox + ServerDataV1 union + samples
- **client**：`InventoryMoveRejectedHandler`、`ProtoServerDataBridge`（CASE_TO_TYPE + extractInner `INVENTORY_MOVE_REJECTED`）、`ServerDataRouter` 注册 `inventory_move_rejected`、`RealmLabel.displayName`

### 博弈自检
- promote 阶段两轮 sonnet 博弈：round-1 2 blocker（"复用 EventAlert" 证据被证伪 + 死代码引用）→ round-2 ready（改专属结构化 payload + 活链路）。
- consume Verify 博弈：opus verdict=ready、defenseWins=true，端到端链路逐点核验真通无孤岛。控方 major（armor `unknown` 泄漏）判"真实但不可达"→ 主线顺手硬化（`91e845e5d`，补 None 分支专属测试）；minor（拖拽 ghost 隐藏）opus 核实为 docked 面板 z=200 架构裁剪、非漏做，未改。

### 遗留 / 后续
- hover 面板"拖拽进行中隐藏"：`ItemTooltipPanel` 为 rightCol 固定 docked 面板（非跟随光标浮动 tooltip），拖拽 ghost 以 z=200 覆盖绘制结构上不遮挡——原 §P3 该条属范围裁剪非漏实现（opus 核实在案）。
- `worn_cap_bonus`（`mod.rs:659-661`，P5 占位恒 0）将来由境界/功法派生启用后，动态加成类拒绝仍走 P1 toast、hover 不承诺（§P2 边界已声明）。
