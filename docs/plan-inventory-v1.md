# Plan: Inventory v1

状态：草案（2026-04-13，§3.1 频道部分 2026-04-13 校准为统一 `bong:server_data` 路由）
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
   - 拾取掉落实体（Phase B）。

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
    pub spirit_stones: u64,                  // 散装灵石（不占格子）
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

`InventoryModel` 新增 `spiritStones / boneCoins` 已有 → 复用。

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
- C2S：`InventoryMoveRequest`、`ApplyPillRequest`、`DropItemRequest`
- 内部：`InventoryEvent { kind: Added|Removed|Moved|StackChanged|DurabilityChanged, instance_id, ... }`
- outbound：`DroppedItemEvent`（供未来战斗 plan / world entity 消费）

---

## 3. IPC Schema

### 3.1 Channels

**S2C 统一走 `bong:server_data` CustomPayload + `type` 路由**（与 `cultivation_detail` / `ui_open` / `player_state` 一致，禁止新建独立 channel）。

需要扩的 `ServerDataType` 变体（`server/src/schema/server_data.rs`）：
- `InventorySnapshot`（全量，进服 / 重生 / 容器切换时推一次）
- `InventoryEvent`（delta，`kind: Added|Removed|Moved|StackChanged|DurabilityChanged` + `instance_id`）
- `ItemRegistrySnapshot`（进服首次推送模板库，client 缓存到 `ItemRegistryStore`）

Client 端照 `ServerDataRouter` 现有模式，新增三个 handler 注册到 dispatcher：
```java
handlers.put("inventory_snapshot", new InventorySnapshotHandler(store));
handlers.put("inventory_event",    new InventoryEventHandler(store));
handlers.put("item_registry_snapshot", new ItemRegistrySnapshotHandler(registry));
```

C2S 复用 `bong:client_request`，新增 3 个联合变体（`InventoryMoveRequestV1` / `ApplyPillRequestV1` / `DropItemRequestV1`）。

### 3.2 Schema 文件（`agent/packages/schema/src/`）

- `inventory.ts`：`ItemTemplateV1`、`ItemInstanceV1`、`ContainerV1`、`InventorySnapshotV1`、`InventoryEventV1`、`ItemEffectV1`（discriminated union）
- 扩展 `client-request.ts`：
  ```ts
  InventoryMoveRequestV1 { v, instance_id, to_container, to_row, to_col }
  ApplyPillRequestV1    { v, instance_id, target: { kind: "self" } | { kind: "meridian", meridian_id: MeridianId } }
  DropItemRequestV1     { v, instance_id }
  ```
- Round-trip 测试覆盖所有新 schema。

### 3.3 Item Template 同步

**决策**：template **不**走每帧 world_state，仅**一次性** S2C `item_registry_snapshot`（玩家进服首次推送）。Client 缓存到 `ItemRegistryStore`，按 `template_id` 查 name/icon/grid size。

---

## 4. 集成点（现有 plan 的未闭环项）

| 来源 | 集成方式 | 消除 TODO |
|---|---|---|
| cultivation §91 `BreakthroughRequest.material_bonus = 0.0 占位` | `ApplyPillRequest(kind=BreakthroughBonus)` → `Cultivation.pending_material_bonus`，下次 breakthrough 读取并清零 | ✅ |
| cultivation §89 mock 「凝脉散 on SI」 | `ApplyPillRequest(kind=MeridianHeal, target=Specific(SI))` → `MeridianSystem.applied_items` + `healing_boost` | ✅ |
| cultivation §37 `CultivationDeathTrigger` 消费者之一 | `death_drop_system` 订阅，产出 `DroppedItemEvent`（不阻塞战斗 plan） | 部分 ✅ |
| worldview §686 死亡掉落 | 同上 | 骨架 ✅ |

**不集成**：
- worldview §486 灵气税（v2）
- worldview §526/§528 交易/箱子（独立 plan）

---

## 5. 阶段化实施路线

### P1 — Server 骨架 + 只读 snapshot
- TOML loader + `ItemRegistry` Resource（含 5–10 个 sample items）
- `PlayerInventory` Component + `inventory_init_system`（从 TOML loadout 填初始物品）
- `inventory_snapshot_emit`（进服推全量）
- Client：`InventoryStateStore` + `InventorySnapshotHandler` 把 mock 换成真实 snapshot（仍只读）
- 单测：TOML 解析、snapshot round-trip

### P2 — 移动 + 装备 + 堆叠
- `InventoryMoveRequest` C2S + `inventory_move_system`（校验 footprint、stack merge）
- `InventoryEvent::Moved/StackChanged` delta S2C
- Client：`DragState` 结束时发 C2S（替换当前纯本地移动），订阅 delta 应用到 UI
- 单测：合法/越界/重叠移动、stack merge

### P3 — 丹药效果联动（关闭 cultivation TODO §89/§91）
- `ApplyPillRequest` + `apply_pill_system`
- `ItemEffect::BreakthroughBonus` / `MeridianHeal` / `ContaminationCleanse` 三个分支落地
- Client：右键 pill → context menu「服用 / 外敷（选经脉）」；外敷时接入 `BodyInspectComponent` 的经脉选中 hover
- 单测：effect magnitude clamp、breakthrough 消费清零

### P4 — 死亡掉落（骨架，待战斗 plan 激活）
- `death_drop_system` 随机抽样（seed 可测）
- 产出 `DroppedItemEvent`（暂不渲染世界 entity）
- 单测：50% 命中、随机种子稳定

### P5 — Weight penalty
- `derived_weight > max_weight` → `OverloadedMarker` + HUD 指示
- 具体移速减益等战斗/移动 plan 接管后再落

### 延后（v2 / 独立 plan）
- 灵气税（worldview §486）
- 掉落物世界实体 + 拾取
- 交易 / 箱子 / 骨币死信箱
- 持久化（DB/文件）
- 法器装备加成 / 耐久度消耗

---

## 6. 测试策略

- Rust：TOML 解析、每个 system 至少 3 测（正常 / 非法 / 边界）、schema round-trip
- TS：每个 schema artifact round-trip + discriminated union 分支覆盖
- Java：handler 解析 + store 应用 + snapshot fixture 与 mock 同构对比
- 端到端：server + client 联跑，进服见到 TOML loadout、I 键打开看到真实物品、拖拽生效后刷新

---

## 7. Client UI Refactor Checklist

UI 侧为**增量改造**，无推倒重来。按 P 阶段拆分。

### 7.1 P1 — 只读 snapshot 替换 mock

**数据模型补字段**
- [ ] `InventoryItem` 新增 4 字段：`long instanceId`、`int stackCount`、`double spiritQuality (0..1)`、`double durability (0..1)`
- [ ] `InventoryItem.create(...)` 工厂签名扩展 + 向后兼容 overload（仅旧字段）供 mock 用；所有调用点逐步切换
- [ ] 影响面：`MockMeridianData`（3 处 appliedItem）、`MockInventoryData`、`InventoryModel` 相关 builder

**新类**
- [ ] `com.bong.client.inventory.state.InventoryStateStore`：复刻 `MeridianStateStore` 模式（`CopyOnWriteArrayList<Consumer<InventoryModel>>` 监听、`replace()` / `snapshot()` / `addListener()` / `resetForTests()`）
- [ ] `com.bong.client.inventory.state.ItemRegistryStore`：缓存 `Map<String, ItemTemplate>`，进服首次 snapshot 后常驻；提供 `lookup(templateId)` 给 UI 查 name/icon/grid size
- [ ] `com.bong.client.network.InventorySnapshotHandler implements ServerDataHandler`：解析 `inventory_snapshot` 全量 → `InventoryStateStore.replace(...)`
- [ ] `com.bong.client.network.ItemRegistrySnapshotHandler implements ServerDataHandler`：解析 `item_registry_snapshot` → `ItemRegistryStore.replace(...)`
- [ ] 两个 handler 在 `ServerDataDispatch` / 分发路由处注册

**Bootstrap + Screen 接线**
- [ ] `InspectScreenBootstrap.createScreenForCurrentState()`：从 `InventoryStateStore.snapshot()` 取 model，空时 fallback `MockInventoryData`
- [ ] `InspectScreen` 打开时注册 `InventoryStateStore.addListener(...)`，close 时移除；listener 内重新刷新 grid/hotbar/equipment
- [ ] 丢弃 screen 内保留的 `model` 字段直接引用 —— 改为每次渲染前从 store 取最新

**UI 显示增强（小改）**
- [ ] `GridSlotComponent`：`stackCount > 1` 时右下角叠加数字（使用 MC 默认 font renderer）
- [ ] `GridSlotComponent`：`spiritQuality < 0.5` 时边框变灰；`< 0.2` 时再降饱和
- [ ] `ItemTooltipPanel`：追加两行「纯度 X%」「耐久 Y%」，仅当 < 1.0 显示，避免新玩家信息过载
- [ ] `InspectScreen` bottom bar：显示 `spiritStones` / `boneCoins` 双通货（现有字段 `spiritStones` 复用，`boneCoins` 需加到 `InventoryModel`）

**测试**
- [ ] `InventorySnapshotHandlerTest`：全量 payload → store 断言；字段缺失 / 长度不一致 → 不触碰 store
- [ ] `ItemRegistrySnapshotHandlerTest`：template parse + lookup
- [ ] `InventoryItemTest`：新字段 default / clamp 行为
- [ ] Fixture JSON 与 `agent/packages/schema/samples/inventory_snapshot.json` 对齐

**显式 NOT P1（延后到 P2/P3）**
- 拖拽不发 C2S，仍保留本地 `DragState` 行为 —— P1 作为「视觉只读快照」就够；真正移动待 P2 打通
- 右键菜单、ApplyPill 交互均 P3

### 7.2 P2 — 拖拽发 C2S（补表略，实施前再展开）
### 7.3 P3 — 丹药右键菜单（补表略，实施前再展开）

---

## 8. 风险 / 未决

- **instance_id 全局 uid 分配**：MVP 用 `AtomicU64`，重启归零不影响（无持久化）
- **堆叠 vs 多格**：同一 template 既 stack 又占 2×1 的情况 MVP 禁止（通过 `ItemTemplate.stackable: bool` 约束）
- **ApplyPill 的经脉 target 校验**：must 已通（`meridian.opened`）且 integrity > 0 —— 否则 `apply_pill_system` 拒绝并发 `InventoryEvent::ApplyRejected`
- **死亡掉落随机性**：用 `XorshiftRoll`（复用 cultivation plan 已有），cause_seed 进 log 供 QA 复现
