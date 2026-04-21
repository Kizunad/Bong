# Plan: Inventory v1

状态：进行中（2026-04-20 校准：Inventory 主链已闭环；剩余工作以实机 QA、文档收口与少量 UI 打磨为主）
作者：Claude Code + Kiz
前置：`plan-cultivation-v1.md`（P1 客户端 UI 已落地）、`worldview.md` §486/§558/§686

---

## 0. 设计公理（不可违反）

1. **Server 权威**：client 只展示 + 发操作意图，合法性校验全部在 server。
2. **世界观一致**：
   - §79/§92 丹药辅助突破：`material_bonus` 从已持有丹药派生；
   - §686 死亡 50% 掉落：`CultivationDeathTrigger` 消费物品清单。
3. **Delta 协议**：每次变更推一条 S2C 增量事件；进服 / 重生推一次全量 snapshot。
4. **显式不做（MVP 范围外）**：
   - **灵气流失税**（§486）— 世界观要求但不是 MVP 重点，延后到 v2；
   - 持久化（重启丢失，offline 模式 OK）；
   - 多人交易 / 箱子 / 炼丹炉（独立 plan）；
   - 真正世界实体化的掉落渲染 / 自动 proximity pickup / 多人争抢（Phase B 以后）。

---

## 0.1 当前状态校准（2026-04-20）

### 已闭环完成

- 权威 inventory snapshot / delta 主链：`InventorySnapshot`、`InventoryEvent`、`InventoryStateStore`、`InventorySnapshotHandler`、`InventoryEventHandler`、`InspectScreenBootstrap` 已接通。
- 权威 inventory 操作：拖拽移动已走 C2S 意图；丹药 `ApplyPillRequest` 三个分支（突破加成 / 经脉修复 / 丹毒清理）已落地。
- dropped-loot 最小闭环已超出原 plan 骨架：
  - `death_drop_system` + `DroppedLootRegistry`；
  - `dropped_loot_sync` S2C；
  - client `DroppedItemStore`；
  - projected HUD marker（含 edge accent / 方向前缀 / corner dominant-axis / 基础抗抖）；
  - `G` 键 pickup；
  - InspectScreen 右侧垃圾桶丢弃 → server-authoritative dropped loot。
- overweight/weight penalty 的 HUD 指示已可用。

### 已实现，待实机 QA / 收口

- 丢弃 → dropped marker → `G` pickup 一轮完整场景 QA 仍需系统化记录。
- dropped marker 已可用，但视觉细节（edge readability / 进一步稳定性）仍可继续优化。
- 文档自身仍需按本节结论持续收口，避免与代码现状再次脱节。

### 近期一致性修复（2026-04-20）

- **marker / pickup 目标一致性**：`DroppedItemStore.nearestTo` 原基于 `ConcurrentHashMap.values()` 迭代顺序，两个等距 dropped loot 时 marker 渲染与 `G` 键 pickup 可能选中不同 entry。修法：引入单调 `insertionCounter` + per-entry `insertionOrder`，距离差 ≤ `DISTANCE_TIE_EPSILON_SQ`（0.01 m²）时按 insertionOrder 倒序（**最新丢入的胜出**）tie-break，与玩家"刚丢的物品最想被高亮 / 被 G 捡回"的直觉一致。replaceAll 按 list 顺序分配 order（与 server `registry.by_owner` Vec push 尾部=latest 的约定对齐）；同 id replace 保持原 order，避免 server snapshot 洗掉"最新"语义。客户端新增 4 个 tie-break 单测，不破坏原有测试。
- **optimistic discard 回滚链已存在，不属风险项**：server 端 `inventory_discard_rejection` 分支会 `resync_snapshot` 推全量 snapshot，client `InventorySnapshotHandler` 调 `InventoryStateStore.applyAuthoritativeSnapshot` 覆盖本地，无需额外回滚代码。
- **tooltip 左上角 icon**：`ItemTooltipPanel` 复用 `GridSlotComponent.drawItemTexture`，在 32×32 左上位置画物品 icon；name/meta/status 从 icon 右侧起，description 在 icon 底部之下走全宽。面板尺寸（196×72）不变，长描述仍会截断（如需完全展示留给后续 "tooltip 高度自适应"）。
- **世界空间 billboard 渲染（"甲" 轻量方案）**：新增 `DroppedItemWorldRenderer`（`client/src/main/java/com/bong/client/inventory/render/`），走 `WorldRenderEvents.AFTER_ENTITIES`，yaw-only semi-billboard（正面随相机 yaw 转动、pitch 锁竖直）+ sine 上下浮动（±0.06 m，周期 2 s）+ 悬浮 0.45 m + 距离剔除 32 m + lightmap 采样。贴图复用 `textures/gui/items/{item_id}.png`。**纯 client-only**：世界坐标来自 `DroppedItemStore`，不 spawn entity、不改 server，不违反"真正世界实体化延后"约束——这是一个**视觉 surrogate**，用户能直接看到物品在地上而不再仅靠 HUD 方向指示。
- **HUD marker 下线**：原 `DroppedItemHudPlanner` 屏幕 2D overlay 用独立的投影+`MARKER_VERTICAL_OFFSET=24px`+stabilization lerp 计算位置，和 world billboard 两套定位系统并存时文字标签相对图标**乱飘**（投影不同步 + lerp 滞后 + 屏幕 clamp）。`BongHudOrchestrator.buildCommands` 不再调用 planner；`DroppedItemHudPlanner` 类和 14 个单测保留作为规格文档，便于未来如有需求回退。`BongHudOrchestratorTest` 同步删除 `droppedItemIndicatorAppearsWhenStoreHasEntries` 测试与 `TEST_CONTEXT` 常量。

### 明确延后

- **真正世界实体化**的掉落渲染与拾取（server 端 Item entity + 多人争抢 + 物理 + 自动 proximity pickup）。当前 client billboard 只是视觉 surrogate，不替代。
- 多人争抢 / 多玩家共享可见 dropped loot。
- 持久化。
- 灵气税（§486）。
- 交易 / 箱子 / 骨币死信箱。
- 装备加成 / 耐久度消耗体系深化。

---

## 1. 数据模型

### 1.1 Item Registry（静态数据）

**存储**：TOML 文件，`server/assets/items/*.toml`，启动时扫描目录加载成 `Res<ItemRegistry>`。

**示例** `server/assets/items/pills.toml`：

```toml
[[item]]
id = "guyuan_pill"
name = "固元丹"
category = "Pill"
grid_w = 1
grid_h = 1
base_weight = 0.2
rarity = "rare"
spirit_quality_initial = 1.0
description = "温补真元，服后可加速恢复灵力"
# Pill 专用字段（其他 category 可省）
effect = { kind = "breakthrough_bonus", magnitude = 0.12 }

[[item]]
id = "ningmai_powder"
name = "凝脉散"
category = "Pill"
grid_w = 1
grid_h = 1
base_weight = 0.3
rarity = "uncommon"
spirit_quality_initial = 1.0
description = "外敷经脉，缓解走火入魔"
effect = { kind = "meridian_heal", magnitude = 0.20, target = "any_meridian" }
```

**`ItemTemplate`**：
```rust
pub struct ItemTemplate {
    pub id: String,
    pub name: String,
    pub category: ItemCategory,   // Pill / Herb / Weapon / SpiritStone / BoneCoin / Misc
    pub grid_w: u8,
    pub grid_h: u8,
    pub base_weight: f64,
    pub rarity: Rarity,           // common / uncommon / rare / epic / legendary
    pub spirit_quality_initial: f64,  // 0..1，未来灵气税使用
    pub description: String,
    pub effect: Option<ItemEffect>,
}

pub enum ItemEffect {
    BreakthroughBonus { magnitude: f64 },            // 0..0.30 clamp（由 cultivation plan §3.1 定）
    MeridianHeal { magnitude: f64, target: Target }, // Target: AnyMeridian | Specific(MeridianId)
    ContaminationCleanse { magnitude: f64 },
    // v2+：法器装备加成 / 灵石使用 / 骨币（不落 effect）
}
```

### 1.2 运行时 Component

```rust
#[derive(Component)]
pub struct PlayerInventory {
    pub containers: Vec<Container>,          // 默认 3: 主背包 5×7 / 小口袋 3×3 / 前挂包 3×4
    pub equipped: HashMap<EquipSlot, ItemInstance>,
    pub hotbar: [Option<ItemInstance>; 9],
    // v1 out-of-scope: future currency slot, not modeled here
    pub bone_coins: u64,                     // 骨币
    pub max_weight: f64,
}

pub struct Container {
    pub name: String,
    pub rows: u8,
    pub cols: u8,
    pub items: Vec<PlacedItem>,              // 每个 PlacedItem 记录左上角 (row, col)
}

pub struct PlacedItem {
    pub row: u8,
    pub col: u8,
    pub instance: ItemInstance,
}

pub struct ItemInstance {
    pub instance_id: u64,                    // server 分配的全局 uid，用于 delta 事件
    pub template_id: String,                 // 索引 ItemRegistry
    pub stack_count: u32,                    // 默认 1；可堆叠类别（SpiritStone/Herb）可 > 1
    pub spirit_quality: f64,                 // 0..1，MVP 固定 spirit_quality_initial
    pub durability: f64,                     // 0..1，MVP 固定 1.0
}
```

### 1.3 客户端模型补齐

现有 `InventoryItem` 缺字段，需补：
- `long instanceId`（配对 server delta）
- `int stackCount`
- `double spiritQuality`（MVP 仅展示，不影响逻辑）
- `double durability`（MVP 仅展示）

`InventoryModel` 仅复用骨币，不再为 v1 引入额外散装灵石字段。

---

## 2. System / Tick

| System | 触发 | 职责 |
|---|---|---|
| `inventory_init_system` | 玩家 Client 组件加入时 | 附挂 `PlayerInventory`，按出生 loadout（TOML 指定）填充 |
| `inventory_move_system` | 消费 `InventoryMoveRequest` Event | 校验 footprint/overlap，移动 `PlacedItem`，发 `InventoryEvent::Moved` |
| `apply_pill_system` | 消费 `ApplyPillRequest` Event | 根据 `ItemEffect::kind` 派发：`BreakthroughBonus` → `Cultivation.pending_material_bonus`；`MeridianHeal` → `MeridianSystem.healing_boost`；`ContaminationCleanse` → `Contamination.cleanse_queued` |
| `weight_penalty_system` | `PlayerInventory` 变更 | 重算 `derived_weight`，`> max_weight` 时加 `OverloadedMarker` |
| `death_drop_system` | 消费 `CultivationDeathTrigger` | 按 `cause` 策略随机抽 50% instance，生成 `DroppedItemEvent`（Phase A 仅记事件，不落 entity） |
| `inventory_snapshot_emit` | 玩家新连入 / 重生 | 推全量 `inventory_snapshot` S2C |

**Event 清单**：
- C2S：`InventoryMoveRequest`、`ApplyPillRequest`、`InventoryDiscardItemRequest`、`PickupDroppedItemRequest`
- 内部：`InventoryEvent { kind: Added|Removed|Moved|StackChanged|DurabilityChanged, instance_id, ... }`
- outbound：`DroppedItemEvent`（供未来战斗 plan / world entity 消费）

---

## 3. IPC Schema

### 3.1 Channels

**S2C 统一走 `bong:server_data` CustomPayload + `type` 路由**（与 `cultivation_detail` / `ui_open` / `player_state` 一致，禁止新建独立 channel）。

需要扩的 `ServerDataType` 变体（`server/src/schema/server_data.rs`）：
- `InventorySnapshot`（全量，进服 / 重生 / 容器切换时推一次）
- `InventoryEvent`（delta，`kind: Added|Removed|Moved|StackChanged|DurabilityChanged` + `instance_id`）
- `DroppedLootSync`（全量 owner-scoped dropped loot，同步给 HUD marker / pickup 流）
- 模板同步方案已移出 v1，不再作为当前实现目标。

Client 端照 `ServerDataRouter` 现有模式，新增两个 handler 注册到 dispatcher：
```java
handlers.put("inventory_snapshot", new InventorySnapshotHandler(store));
handlers.put("inventory_event",    new InventoryEventHandler(store));
```

C2S 复用 `bong:client_request`，当前 inventory 主线实际使用 4 个联合变体：
- `InventoryMoveRequestV1`
- `ApplyPillRequestV1`
- `InventoryDiscardItemRequestV1`
- `PickupDroppedItemRequestV1`

### 3.2 Schema 文件（`agent/packages/schema/src/`）

- `inventory.ts`：`ItemTemplateV1`、`ItemInstanceV1`、`ContainerV1`、`InventorySnapshotV1`、`InventoryEventV1`、`ItemEffectV1`（discriminated union）
- 扩展 `client-request.ts`：
  ```ts
  InventoryMoveRequestV1 { v, instance_id, to_container, to_row, to_col }
  ApplyPillRequestV1    { v, instance_id, target: { kind: "self" } | { kind: "meridian", meridian_id: MeridianId } }
  InventoryDiscardItemRequestV1 { v, instance_id, from: InventoryLocationV1 }
  PickupDroppedItemRequestV1    { v, instance_id }
  ```
- Round-trip 测试覆盖所有新 schema。

### 3.3 Item Template 同步

**决策**：v1 不再依赖单独的模板同步通道。库存快照本身要自包含，`InventorySnapshot` 直接携带 UI 需要的显示字段，客户端按快照数据渲染。模板同步若以后要做，另起 plan。

---

## 4. 集成点（现有 plan 的未闭环项）

| 来源 | 集成方式 | 消除 TODO |
|---|---|---|
| cultivation §91 `BreakthroughRequest.material_bonus = 0.0 占位` | `ApplyPillRequest(kind=BreakthroughBonus)` → `Cultivation.pending_material_bonus`，下次 breakthrough 读取并清零 | ✅ |
| cultivation §89 mock 「凝脉散 on SI」 | `ApplyPillRequest(kind=MeridianHeal, target=Specific(SI))` → `MeridianSystem.applied_items` + `healing_boost` | ✅ |
| cultivation §37 `CultivationDeathTrigger` 消费者之一 | `death_drop_system` 订阅，产出 `DroppedItemEvent` + `DroppedLootRegistry` + `dropped_loot_sync` | 最小闭环 ✅ |
| worldview §686 死亡掉落 | 同上；client 已能看到 marker 并按 `G` 捡回 | 最小闭环 ✅ |

**不集成**：
- worldview §486 灵气税（v2）
- worldview §526/§528 交易/箱子（独立 plan）

---

## 5. 阶段化实施路线

### P1 — Server 骨架 + 只读 snapshot（已完成）
- TOML loader + `ItemRegistry` Resource（含 5–10 个 sample items）
- `PlayerInventory` Component + `inventory_init_system`（从 TOML loadout 填初始物品）
- `inventory_snapshot_emit`（进服推全量）
- Client：`InventoryStateStore` + `InventorySnapshotHandler` 把 mock 换成真实 snapshot（仍只读）
- 单测：TOML 解析、snapshot round-trip

### P2 — 移动 + 装备 + 堆叠（已完成）
- `InventoryMoveRequest` C2S + `inventory_move_system`（校验 footprint、stack merge）
- `InventoryEvent::Moved/StackChanged` delta S2C
- Client：`DragState` 结束时发 C2S（替换当前纯本地移动），订阅 delta 应用到 UI
- 单测：合法/越界/重叠移动、stack merge

### P3 — 丹药效果联动（已完成）
- `ApplyPillRequest` + `apply_pill_system`
- `ItemEffect::BreakthroughBonus` / `MeridianHeal` / `ContaminationCleanse` 三个分支落地
- Client：右键 pill → context menu「服用 / 外敷（选经脉）」；外敷时接入 `BodyInspectComponent` 的经脉选中 hover
- 单测：effect magnitude clamp、breakthrough 消费清零

### P4 — 死亡掉落 + dropped-loot 最小闭环（已完成，且超出原骨架）
- `death_drop_system` 随机抽样（seed 可测）
- `DroppedLootRegistry` + `DroppedItemEvent` + `dropped_loot_sync`
- client `DroppedItemStore` + projected HUD marker
- `G` 键 pickup
- InspectScreen 右侧垃圾桶丢弃接入同一 dropped-loot 流
- 单测：50% 命中、随机种子稳定、discard/pickup 主链可回归

### P5 — Weight penalty（已完成）
- `derived_weight > max_weight` → `OverloadedMarker` + HUD 指示
- 具体移速减益等战斗/移动 plan 接管后再落

### 延后（v2 / 独立 plan）
- 灵气税（worldview §486）
- 掉落物世界实体 + 自动 proximity 拾取 + 多人争抢
- 交易 / 箱子 / 骨币死信箱
- 持久化（DB/文件）
- 法器装备加成 / 耐久度消耗

---

## 6. 测试策略

- Rust：TOML 解析、每个 system 至少 3 测（正常 / 非法 / 边界）、schema round-trip
- TS：每个 schema artifact round-trip + discriminated union 分支覆盖
- Java：handler 解析 + store 应用 + snapshot fixture 与 mock 同构对比
- 端到端：server + client 联跑，进服见到 TOML loadout、I 键打开看到真实物品、拖拽生效、垃圾桶丢弃后出现 marker，并可 `G` 键捡回

---

## 7. Client UI 状态校准（2026-04-20）

### 7.1 已完成

- `InventoryItem` 权威字段（`instanceId` / `stackCount` / `spiritQuality` / `durability`）已补齐并接入主线 snapshot。
- `InventoryStateStore`、`InventorySnapshotHandler`、`InventoryEventHandler`、`ServerDataRouter` 注册已落地。
- `InspectScreenBootstrap` / `InspectScreen` 已按权威 snapshot 刷新，不再以旧 mock 作为主线数据源。
- P2/P3 主线已完成：拖拽移动发 C2S、丹药右键菜单、经脉 targeting 均已落地。
- dropped-loot UI 主线已完成：`DroppedItemStore`、`DroppedLootSyncHandler`、projected marker、`G` pickup、右侧垃圾桶丢弃接 server-authoritative dropped loot。

### 7.2 待实机 QA / 文档收口

- 丢弃 → marker → `G` pickup 的一轮完整手工 QA 仍需留记录。
- dropped marker 的视觉细节已可用，但 edge readability / target switching / 进一步稳定性仍可后续优化。

### 7.3 可选 UI 尾项（不阻塞 v1 闭环）

- `GridSlotComponent`：`stackCount > 1` 的角标展示。
- `GridSlotComponent`：`spiritQuality` 的轻量边框退饱和提示。
- `ItemTooltipPanel`：纯度 / 耐久附加展示。

### 7.4 已移出 v1 主线 / 明确延后
**数据模型补字段**
- [x] `InventoryItem` 新增 4 字段：`long instanceId`、`int stackCount`、`double spiritQuality (0..1)`、`double durability (0..1)`
- [x] `InventoryItem.create(...)` 工厂签名扩展 + 向后兼容 overload（仅旧字段）供 mock 用；所有调用点逐步切换
- [x] 影响面：`MockMeridianData`（3 处 appliedItem）、`MockInventoryData`、`InventoryModel` 相关 builder

**新类**
- [x] `com.bong.client.inventory.state.InventoryStateStore`：复刻 `MeridianStateStore` 模式（`CopyOnWriteArrayList<Consumer<InventoryModel>>` 监听、`replace()` / `snapshot()` / `addListener()` / `resetForTests()`）
- [ ] `com.bong.client.inventory.state.ItemRegistryStore`：如后续需要再单独立项，当前 v1 不作为主线
- [x] `com.bong.client.network.InventorySnapshotHandler implements ServerDataHandler`：解析 `inventory_snapshot` 全量 → `InventoryStateStore.replace(...)`
- [x] `com.bong.client.network.InventoryEventHandler implements ServerDataHandler`：解析 `inventory_event` → `InventoryStateStore` 增量合并
- [x] 两个 handler 在 `ServerDataRouter` / 分发路由处注册

**Bootstrap + Screen 接线**
- [x] `InspectScreenBootstrap.createScreenForCurrentState()`：从 `InventoryStateStore.snapshot()` 取 model，权威快照未到时才 fallback `MockInventoryData`
- [x] `InspectScreen` 打开时注册 `InventoryStateStore.addListener(...)`，close 时移除；listener 内重新刷新 grid/hotbar/equipment
- [x] 丢弃 screen 内保留的 `model` 字段直接引用 —— 改为每次渲染前从 store 取最新

**UI 显示增强（小改）**
- [x] `GridSlotComponent`：`stackCount > 1` 时右下角叠加数字（使用 MC 默认 font renderer）
- [x] `GridSlotComponent`：`spiritQuality < 0.5` 时边框变灰；`< 0.2` 时再降饱和
- [x] `ItemTooltipPanel`：追加两行「纯度 X%」「耐久 Y%」，仅当 < 1.0 显示，避免新玩家信息过载
- [x] 散装灵石相关 UI 指引已 out-of-scope，不再作为 v1 目标；如未来要做双通货，再开新 plan

**测试**
- [x] `InventorySnapshotHandlerTest`：全量 payload → store 断言；字段缺失 / 长度不一致 → 不触碰 store
- [ ] `ItemRegistrySnapshotHandlerTest`：template parse + lookup
- [ ] `InventoryItemTest`：新字段 default / clamp 行为
- [x] Fixture JSON 与 `agent/packages/schema/samples/inventory_snapshot.json` 对齐

- 单独的 `ItemRegistryStore` / 模板同步主线。
- 散装灵石双通货 UI 指引。

### 7.5 Manual QA Checklist（实机验收）

自动化 gates 由 `scripts/smoke-inventory-complete.sh` 覆盖（schema + server inventory/discard/pickup/death-drop 测试 + client DroppedItem*/Inventory*/InspectScreen* 测试 + 全量 build）。UI 与渲染类改动 smoke 抓不到，按下列清单手测：

**前置**：server 在跑（`cd server && cargo run`）；client jar 已同步（`bash scripts/windows-client.sh --launch`）；进 offline 世界。

**A. 背包 UI + Tooltip**
- [ ] 按 `I` 打开背包，container grid + 装备槽 + 丹药 quickuse 正确渲染
- [ ] 悬停任何物品，tooltip **左上 32×32 icon** 显示
- [ ] 短描述 tooltip **保底 112 px 高度**（不拥挤）
- [ ] 长描述 tooltip **按内容自适应扩展**，文字完整 word-wrap，**无 `…` 截断**
- [ ] 纯度 < 100% 或耐久 < 100% 时 tooltip 显示对应状态行（颜色按阈值切换）

**B. Discard 流（发 C2S → server 权威）**
- [ ] 拖物品到右侧垃圾桶 → 物品从背包消失
- [ ] 关背包，地面原地出现 **world-space billboard**（item png）
- [ ] Billboard **悬浮 0.45 m**、**yaw-only 朝向相机**（抬头低头 quad 不歪）
- [ ] Billboard 以 2 s 周期做 **sine 上下浮动**（±0.06 m，明显可见）
- [ ] Billboard 夜晚 / 阴影处**变暗**（lightmap 采样生效）
- [ ] 距离 > 32 m 走远后 billboard **不再渲染**（保证性能，走近会重新出现）
- [ ] 屏幕上**不再出现 HUD 文字标签**（原 marker 已下线）

**C. Pickup 流（G 键）**
- [ ] 距离 < 2.5 m 内按 `G`，物品回到背包，billboard 消失
- [ ] 距离 > 2.5 m 按 `G` 无响应（server reject，日志确认）
- [ ] **tie-breaker 验证**：连续 discard 两个物品站原地不动（两物品 server 端有 0.1m×n 偏移，client 视为近乎等距），按 `G` 先捡到**最后丢的**（`DroppedItemStore.nearestTo` insertionOrder 倒序 tie-break）

**D. 丹药使用（ApplyPill）**
- [ ] 右键丹药 / 拖到 BodyInspect 区，按 effect 触发：
  - `breakthrough_bonus` → 下次突破窗口期内生效（看 cultivation HUD / 日志）
  - `meridian_heal` → 选中某条经脉施用，integrity 回升
  - `contamination_cleanse` → 丹毒 mellow/violent 值下降

**E. Overweight / Weight Penalty**
- [ ] 背包重量 > max_weight 时 HUD 顶部出现"超载"
- [ ] Discard 超重物品后"超载"消失
- [ ] 超重期间移动速度受影响（server 端 `weight_penalty_system` 生效，通过身法 HUD 观察）

**F. 死亡掉落（与 plan-cultivation-v1 联动）**
- [ ] 触发死亡 → 随机 50% 掉落物在重生点附近生成对应 dropped loot
- [ ] 重生后回到死亡点能看到 billboard + `G` 捡回
- [ ] `cause_seed` 进入日志（复现核对）

**G. 回归（避免破 snapshot / delta 主链）**
- [ ] 重新进服：推一次完整 snapshot，UI 全部恢复
- [ ] 操作一次后重连：snapshot 与最后 delta 一致，无幽灵物品

完成一轮后在 `docs/plan-inventory-v1.md` 末尾或 PR 描述里记一行 `QA pass by <人>, <日期>`。

---

## 8. 风险 / 未决

- **instance_id 全局 uid 分配**：MVP 用 `AtomicU64`，重启归零不影响（无持久化）
- **堆叠 vs 多格**：同一 template 既 stack 又占 2×1 的情况 MVP 禁止（通过 `ItemTemplate.stackable: bool` 约束）
- **ApplyPill 的经脉 target 校验**：must 已通（`meridian.opened`）且 integrity > 0 —— 否则 `apply_pill_system` 拒绝并发 `InventoryEvent::ApplyRejected`
- **死亡掉落随机性**：用 `XorshiftRoll`（复用 cultivation plan 已有），cause_seed 进 log 供 QA 复现
