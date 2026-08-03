# plan-refactor-inventory-core-v1 — Inventory 巨石拆分 + 网格/交付事务一致性（重构轨 R10）

> 所属总纲：`docs/plans-skeleton/plan-refactor-master-v1.md`。一句话：拆分 2 万行级 `server/src/inventory/mod.rs`，以统一 `InventoryTxn` 收口给予、交付、消费、拾取、堆叠与网格占格，使物品移动可原子验证、可观测且不静默丢失。

## 阶段总览

- ✅ 2026-08-03 P0 设计收口 + 吸收清单验真
- ⬜ P1 巨石拆分（行为不变）+ `InventoryTxn` 骨架
- ⬜ P2 交付/消费路径统一
- ⬜ P3 网格/堆叠一致性 + 老存档布局迁移
- ⬜ P4 bot 验收 + 被吸收 plan 收口

> **实施门禁**：按 master §4/§5，R10 实现属于 Wave 2，必须等待 R3 P1 提供稳定 persistence inventory slice/hydration 接缝。当前仅 P0 完成；P1–P4 不得提前启动。2026-08-03 核验时 R3 仅有 P0 PR #1308 OPEN，R3 P1 尚未落地。

## 现状证据（2026-08-03 复核）

- `server/src/inventory/mod.rs` 当前 20479 行；原骨架记录的 20165 行已过时。该文件同时承载 domain model、registry/loadout、实例创建、交付/堆叠、网格移动、装备与套包、掉落拾取、遗骸转移、耐久/磨损接缝及大量内联测试。
- `add_item_to_player_inventory_or_ground`（`server/src/inventory/mod.rs:1839`）只在真实容量不足时降级为地面掉落，其它结构错误原样返回；这是统一满包策略的现役生产先例。
- `stack_identity_matches`（`server/src/inventory/mod.rs:2223`）与现有 staged grant 已具备“完整动态身份才合并、验证成功后再提交”的正确基线，R10 应抽取复用而非改弱为仅比较 `template_id`。
- `force_attach_item_to_inventory`（`server/src/inventory/mod.rs:4733`）在无合法位置时直接把物品压入 `(0, 0)`；`transfer_all_inventory_contents`（`:4675`）会调用它，故占格重叠缺口仍可达。
- `pickup_dropped_loot_instance`（`server/src/inventory/mod.rs:5337`）只找空 footprint，不尝试合并已有同身份堆叠，拾取合并缺口仍存在。
- server 的 `apply_inventory_move`（`server/src/inventory/mod.rs:3828`）会正确交换旋转后的 `grid_w/grid_h`，但 `InventoryEventV1::Moved`（`server/src/schema/inventory.rs:294`）只带 from/to；client `InventoryEventHandler`（`:85`）复用旧 item view，因此 delta 路径会保留旧 footprint。
- pack move 成功分支在 `handle_inventory_move`（`server/src/network/client_request_handler.rs:15310`）执行 `worn_pack_rebuild` snapshot 后提前返回；普通 `send_moved_event`（`:15590`）未覆盖该分支，稳定成功回执缺口仍存在。
- `emit_changed_inventory_snapshots`（`server/src/network/inventory_snapshot_emit.rs:93`）会为 `Changed<PlayerInventory>` 自动推送权威快照；forge 路径已在 `production_forge_station_real_place.py:114-158` 从 `/give` 后快照取得真实 `instance_id` 并完成放砧/消耗断言，但该证据**不能代表所有生产系统已闭环**。`production_lingtian_gathering_intents.py:9-11,53-68` 仍明确使用 `hoe_instance_id: 0`，只等任意 server-data 回流；`hoe_iron` 的 producer→snapshot→equip/held→`lingtian_start_till` 深链仍未覆盖。
- `load_player_inventory_from_sqlite`（`server/src/player/state.rs:1372`）已形成 schema-version 分流、纯内存迁移和 hydration 后校验接缝；pre-#249 布局迁移应在 R3 P1 抽出的接缝上调用 R10 的纯迁移函数，不应在 persistence 巨石中再造一套 inventory 规则。

## 接入面

- **进料**：R1 session 产物交付、R4 gate 通过后的 inventory 请求、world dropped-loot registry、R3 persistence inventory slice/hydration、craft/alchemy/forge/botany 等产出域。
- **出料**：`InventoryTxn` 的交付/消费/拾取 receipt、权威 `InventoryRevision`、世界掉落记录、网格变更事实；公共 S2C 经 R6 拥有的 emit/proto 接缝发送。
- **共享类型**：保留 `PlayerInventory`、`ItemInstance`、`ContainerState`、`InventoryLocationV1`、`InventoryRevision`、`owner_instance_id` 和现有 `ItemCategory` 合法集；不建立兼容层或平行模型。
- **跨仓库契约**：server 产出 `inventory_snapshot` / `inventory_event`；client 消费权威 footprint；bot 通过同一 wire payload 验证 instance、revision、move/merge/spill。R10 只冻结所需 payload 事实，R6 拥有公共 S2C schema/emit 接缝改动。
- **worldview 锚点**：物品不凭空消失、满包产物落在争夺现场，对齐 `worldview.md` §十三末法物资稀缺与争夺语义；唯一真货币仍为骨币，R10 不改经济定义。
- **qi_physics 锚点**：普通物品归属/位置变化不是新的真元流，R10 不新增真元常数或公式。含真元物品的磨损、销毁、衰变仍调用 R5/既有 ledger 语义；spill 与堆叠不得重置或吞掉动态真元字段。

## P0 — 设计收口 + 吸收清单验真（✅ 2026-08-03）

### P0.1 职责拆分图

P1 按下表迁移 symbol；先做机械拆分和测试平移，不在同一提交中改行为。

| 目标模块 | 独占职责 | 首批迁入 symbol / 数据 |
|---|---|---|
| `inventory/model.rs` | inventory 领域模型与 revision/receipt 基础类型 | `PlayerInventory`、`ItemInstance`、`ContainerState`、`PlacedItemState`、装备/位置/修订类型 |
| `inventory/registry.rs` | template registry、TOML/loadout 解析、实例创建 | `ItemRegistry`、template lookup、instance allocator/construction |
| `inventory/grid.rs` | footprint、碰撞、fit、attach/detach、move/swap/rotate | `find_free_slot`、`find_first_fit_container_location`、`validate_attach_fits`、`attach_at_location`、`apply_inventory_move` |
| `inventory/txn.rs` | staged delivery/consume/pickup/merge 与统一 receipt | staged grant、`stack_identity_matches`、新增 `InventoryTxn` |
| `inventory/container.rs` | 穿戴套包 owner、派生容器重建、容量/重量、overflow spill | worn-pack helpers、`rebuild_containers_from_equipment`、`rebuild_and_drop_overflow` |
| `inventory/corpse.rs` | 死亡掉落、遗骸 inventory 与全量转移编排 | 保留现有 corpse 模块；将 `transfer_all_inventory_contents` 改为消费 txn/grid API，不再强塞 |
| `inventory/freshness.rs` | 仅 freshness 数据与 inventory construction 接缝 | 保留现有模块；衰变公式继续由 shelflife/R5 单一实现，不复制到 txn |
| `inventory/mod.rs` | 模块声明、稳定 re-export、Bevy plugin/system wiring | 不再承载具体算法或大块测试 |

测试跟随所属模块迁移；跨模块契约测试放 `inventory/tests` 或现有集成测试文件，不把测试继续堆回 `mod.rs`。

### P0.2 冻结 `InventoryTxn` 契约

P1/P2 必须落地以下稳定语义；具体 Rust lifetime 可在实现时按借用检查器调整，但名称、输入事实、结果事实和错误边界不得漂移：

```rust
pub struct InventoryTxn<'a> { /* staged view over one PlayerInventory */ }

impl InventoryTxn<'_> {
    pub fn deliver(
        &mut self,
        request: DeliveryRequest,
        spill: Option<&mut SpillContext<'_>>,
    ) -> Result<InventoryDeliveryReceipt, InventoryTxnError>;

    pub fn consume_checked(
        &mut self,
        request: ConsumeRequest,
    ) -> Result<InventoryConsumeReceipt, InventoryTxnError>;

    pub fn pickup_and_merge(
        &mut self,
        request: PickupRequest,
        dropped: &mut DroppedLootRegistry,
    ) -> Result<InventoryPickupReceipt, InventoryTxnError>;
}
```

- `DeliveryRequest` 必须同时表达“按 template 创建”与“交付既有 `ItemInstance`”；后者保留 instance id、耐久、freshness、attributes/NBT 等动态身份，不允许重建成默认实例。
- `InventoryDeliveryReceipt` 至少包含：最终 `revision`、`created_instance_ids`、直接放入的 instance ids、逐项 merge 的 source/target/count、逐项 spill 的 dropped id/instance id。允许“部分 merge + 剩余落地”，但总量必须守恒。交付既有 `ItemInstance` 时，receipt 中的 placed/spilled source id 必须仍是请求携带的 id，不能归入 `created_instance_ids`。
- `InventoryConsumeReceipt` 至少包含最终 `revision` 与逐 instance 扣除量；所有 template、数量、身份和持有量前置条件须在 mutation 前完成验证，失败时 inventory/revision 不变。
- `InventoryPickupReceipt` 至少包含最终 `revision`、被移除的 dropped id、merge/placement 明细，以及 `PickupAttritionBasis { target_instance_id, incoming_instance_id, incoming_stack_count, incoming_abs_qi_before }`。`target_instance_id` 指向提交后真实承载物品的实例（纯放置时等于 incoming id，merge 时为既有 stack id）；`incoming_abs_qi_before = dropped.item.spirit_quality * incoming_stack_count`，禁止调用者再从合并后整栈反推。只有 inventory commit 成功后才能从 `DroppedLootRegistry` 删除世界掉落。
- R5 的稳定 pickup attrition API 必须消费 `PickupAttritionBasis`：只从 `incoming_abs_qi_before` 计算损耗并归还 zone；merge 后目标整栈的绝对真元更新为 `preexisting_abs_qi + incoming_abs_qi_after`，不得对 pre-existing quantity 重复磨损。placement 路径同样通过 receipt 定位，不再假设 dropped id 必然可在 inventory 中二次查到。
- 堆叠资格统一复用完整 identity 规则；仅 `template_id` 相等不足以合并。freshness、耐久、attributes/NBT 或其它影响物品身份的字段不一致时必须分栈。
- 所有成功 txn 至多 bump 一次 revision；结构拒绝与容量拒绝不得 bump。receipt 是调用域和 bot 的权威观察面，不再依赖错误字符串前缀或猜测 snapshot 差异。
- `InventoryTxnError` 至少区分：unknown template、zero quantity、missing container/location、instance id conflict、identity mismatch、insufficient items、capacity exceeded、invalid grid placement、missing spill context。结构错误绝不伪装成 spill。

### P0.3 冻结满包与网格策略

1. **统一策略：机制结算点/玩家脚下地面掉落，不建个人暂存箱。** craft、alchemy、forge、give、loot 使用同一 delivery fallback；世界坐标和 dimension 由调用者提供。
2. 只有经过完整验证后确认的容量不足可以 spill。unknown template、零数量、无容器、ID 冲突、非法 footprint 等必须返回结构错误并保持源状态不变。
3. 调用者没有真实 `SpillContext` 时 fail closed，返回 `CapacityExceeded`/`MissingSpillContext`；不得用虚构坐标，也不得清 session、扣材料或删除世界掉落。
4. spill 必须携带原 `ItemInstance`；对 minted stack 可按合法最大堆叠拆分，对既有动态实例不得通过重新 mint 丢字段。
5. 所有 attach 都必须先通过 `validate_attach_fits` 并由 `attach_at_location` 提交；删除 `force_attach_item_to_inventory` 的 `(0,0)` 强塞 fallback。全量转移放不下的剩余项进入显式 spill receipt，而不是重叠占格。
6. dropped-loot registry 是有界 durable queue，不是无限容器：R10 P1 定义 `MAX_DURABLE_DROPPED_LOOT_ENTRIES` 单一常量（初值 `4096`，测试只引用该常量）以及 `DroppedLootRegistry::try_insert` / `try_insert_batch` 原子容量 API。任何生产写入都必须先通过该 API；单条写入在 mutation 前预留 1 槽，批量写入在首条 mutation 前一次性预留全部槽位，禁止调用方直接操作 `entries.insert`。生产 writer 的穷举清单是：`add_item_to_player_inventory_or_ground`、`rebuild_and_drop_overflow`、`enforce_intrinsic_gate_on_morph_release`、`apply_death_drop_on_revive` 的 TSY/主世界分支、`apply_termination_drop_on_terminate` 的 world-drop fallback 分支、`spawn_template_dropped_loot`、`discard_inventory_item_to_dropped_loot`、`tsy_loot_spawn::spawn_for_layer` 以及该文件内所有 placeable-break/TSY layer event writer；P2/P3 必须逐一迁移，测试 fixture 允许直接构造 registry 但不得代表生产接线。所有 delivery/discard/overflow/death/TSY/placeable-break 路径在 mutation 前按“本事务新增 dropped entry 数”原子预留容量。`len + required > MAX` 时返回 typed `DroppedLootCapacityExceeded { current, required, limit }`，inventory、revision、session、材料、世界掉落与 persistence 均保持不变；禁止逐项插入后才发现超限。启动 hydration 的行数检查由 R3 提供 guard 接缝、由 R10 P1 的常量/API 接入；在 R10 P1 合入前 R3 不得引用该 symbol。超限时必须进入 R3 load-failure guard/只读降级并告警，不能截断、驱逐或覆盖旧行。
7. 正常回收只允许玩家 pickup 或后续显式管理员运维；本 plan 不以 TTL/LRU/按价值驱逐静默销毁稀缺物品。每次 pickup 与 durable delete 同事务释放槽位。R6 的 `dropped_loot_sync` 必须分页/分片，每 payload 至多 `256` entries，携带 `snapshot_revision/page_index/page_count`；client 收齐同 revision 全部分片后原子替换视图，内容变化时不再对每个 client 构建/排序一份无界全量 payload。
8. rotate/move receipt 必须携带变更后的完整 item view 或等价 `grid_w/grid_h + rotated` 事实。R10 的 inventory outcome 提供所需事实；R4 在其独占 handler 中把 outcome 交给 emit API；R6 扩展公共 S2C schema/emit 并让 client 以权威新 footprint 替换旧 view。三段任一未完成均不得判 P3/P4 完成。

### P0.4 文件所有权与跨轨边界

- **R10 独占**：`server/src/inventory/**`；各产出域只改为调用冻结的 transaction API，不接管其 session/玩法状态机。
- **R1**：拥有 session 生命周期。R10 返回 delivery 成败与 receipt；R1 只能在成功存入或成功 spill 后清 session，失败必须保留可重试状态。
- **R3**：拥有 `server/src/persistence/**`、autosave 与 hydration 编排。R3 P1 先冻结 inventory slice hydration seam；R10 P1 同时提供 `MAX_DURABLE_DROPPED_LOOT_ENTRIES` 与 `DroppedLootRegistry::{try_insert,try_insert_batch}` 这一容量契约，R3 P2/P4 只能在 R10 P1 merge 后接入并引用它；在此之前 R3 以未超限 guard seam 为占位，不得复制常量。R10 P3 再在 `inventory/migration.rs` 提供纯、幂等的旧布局转换/校验；R3 P4 的 inventory migration consumer 子批次调用它、保存新 schema，并为迁移 overflow 提供真实 `SpillContext`（player identity、mechanism/world position、dimension、durable registry 与 reservation）。R3 consumer 必须先在临时内存/副本上完成 migration，成功且所有 overflow 已通过容量预留并持久化后才提交新 schema；缺上下文、容量不足或持久化失败时保留旧行并返回 retryable load failure，不得伪造坐标、清空旧行或把 overflow 留在无主纯值中。R10 不直接重构 persistence 巨石。
- **R4**：拥有 C2S gate 与 `server/src/network/client_request_handler.rs`。通过 gate 后调用 R10 txn；对于 rotate/pack move，R4 还负责把 R10 outcome 的 request identity、revision 和新 item view 传给 R6 emit API，但不定义 wire schema。
- **R5**：拥有 qi ledger、磨损/衰变物理；R10 保留动态字段并产出 `PickupAttritionBasis`，R5 P3 已登记 incoming-only pickup attrition API 与守恒断言，R4 只传 receipt 不重算整栈真元。
- **R6**：拥有公共 S2C schema/emit 与 client network consumer。R6 canonical plan 的 P0/吸收清单已登记 `rotate-footprint-sync`、`bot-inventory-pack-feedback`；R6 P1/P4 已登记 dropped-loot 分片快照与 inventory receipt contract 子批次，负责修改 `server/src/schema/inventory.rs` 及双端 samples/convert，使成功 `moved`（或等价 accepted receipt）携带请求可归因字段、结果 revision 和权威 item view，修改 inventory emit API 与 `client/.../InventoryEventHandler.java`，并补 server schema/emit、client handler、Python decoder contract tests。**在 R6 P4 子批次 merge 前，R10 P3 不得标 ✅、P4 不得归档相关 skeleton。**
- **R7**：拥有 `InspectScreen`；R10 只提供权威 inventory model/receipt，不改 UI 导航架构。

#### 跨轨执行顺序（P3 硬门）

1. **R10 P3 inventory PR**：在 `server/src/inventory/**` 产出 rotate/pack move 的 typed outcome，含 `instance_id/from/to/revision/item_view`；不触碰 wire 文件。
2. **R6 P0 + P4 wire PR**：P0 先更新 `plan-refactor-wire-s2c-v1` 的吸收清单/阶段；P4 的 inventory receipt contract 子批次再落地 schema → sample/convert → emit API → Fabric client consumer → 双端 contract tests，其 API 对 R4 可调用。
3. **R4 P2 handler PR**：在按域拆分 inventory handler 时，把 R10 outcome 交给 R6 emit，并把 `PickupAttritionBasis` 交给 R5 pickup attrition API；成功与拒绝都必须逐请求发稳定机器回执，snapshot 仅作状态修正。若 R4 P2 尚未启动，需先开该阶段的最小 inventory-handler 接线 PR，不得由 R10/R6 越权改 `client_request_handler.rs`。
4. 三个 PR 均 merge 后，R10 才运行 §P4 的跨层 bot gate，并以实际 server→wire→client/bot 链路作为吸收归档证据。

### P0.5 吸收清单验真

| plan（省略 `plan-bughunt-` / `-v1`） | 2026-08-03 裁决 | 第一性证据与 R10 落点 |
|---|---|---|
| `alchemy-takeback-full-inventory-loss` | **吸收交付部分** | 满包产物必须先 deliver/spill 成功再由 R1 清 session；session teardown 本身仍归 R1。P2 delivery matrix 锁定。 |
| `dropped-loot-pickup-stack-merge` | **吸收，仍真实** | `pickup_dropped_loot_instance` 只找空 footprint，不 merge。P2 `pickup_and_merge` + incoming-only attrition receipt。 |
| `force-attach-grid-collision` | **吸收，仍真实** | `force_attach_item_to_inventory` 可直接压入 `(0,0)`，且全量转移可达。P3 删除强塞并用合法 attach/spill。 |
| `rotate-footprint-sync` | **吸收，仍真实** | server 已旋转 footprint，`Moved` S2C 不带新 view，client 复用旧尺寸。P3 与 R6 接缝联动。 |
| `bot-inventory-pack-feedback` | **吸收，仍真实** | pack rebuild 成功路径 snapshot 后提前返回，未发普通 moved receipt。P3 统一 receipt；P4 bot 断言。 |
| `bot-production-inventory-instance-visibility` | **部分闭环，剩余契约吸收** | forge 已从 `/give` 快照取真实 anvil instance 并跑通放砧；灵田场景仍把 `hoe_instance_id` 写死为 `0`，未验证 `hoe_iron` 的 snapshot/equip/真实 till 链。P4 必须恢复该深断言后才能归档。 |
| `forge-outcome-full-inventory-loss` | **吸收，仍真实** | #1294 已于 2026-07-27 merge，但只建立 bughunt skeleton，并非修复在飞。P2 forge outcome 改走统一 deliver/spill。 |

**不吸收**：

- `craft-refund-full-inventory-loss`：已在 `docs/finished_plans/` 独立闭环，不重复实现。
- `container-filter-and-completion`：独立 feature plan，R10 只提供稳定 transaction/grid 底座。
- `nested-pack-base`：已在 `docs/finished_plans/` 以 WITHDRAWN/独立结案，不复活旧范围。

### P0.6 pre-#249 老布局迁移决议

**决议**：并入 R10 P3，一次清账，但严格等待 R3 P1。

1. R10 在 `server/src/inventory/migration.rs` 提供纯函数，例如 `migrate_legacy_inventory_layout(value, schema_version) -> MigrationOutcome`；输入/输出只涉及 inventory slice，不执行 SQL。`MigrationOutcome` 必须显式携带 `overflow: Vec<ItemInstance>`，不得把无法安放的物品丢弃或藏在不可消费的字符串中。
2. 迁移挂到 R3 冻结的 hydration seam：现有依据为 `server/src/player/state.rs:1372-1433` 的 schema-version 分流与反序列化路径；R3 P1 先冻结 loader seam，R10 P3 落纯迁移，R3 P4 inventory migration consumer 子批次再调用并保存。R3 consumer 负责把本次加载的玩家身份、真实机制结算点/世界坐标、dimension 与可写 `DroppedLootRegistry` 组装为真实 `SpillContext`，并负责持久化 overflow 与容量 reservation；R10 的纯函数不自行猜位置、不执行 SQL。
3. 迁移幂等：新布局不变；旧 5×7/main-pack 形态转换为当前容器布局；所有 item instance 与动态字段保留；无法合法放入的物品必须进入 `MigrationOutcome::overflow`，由 R3 consumer 通过 `InventoryTxn::deliver`/显式 migration spill handoff 原子落入 durable registry。缺少真实上下文、容量不足或 overflow 持久化失败时，R3 必须保留旧数据库行并返回可重试的 load failure/只读降级；不得伪造坐标、先写新 schema 后再补 spill，或把 overflow 留在无主内存。
4. 迁移成功后由 R3 保存新 schema 形状；只有“新 schema 与全部 overflow 均已持久化”才算成功。R10 不直接写 `inventories` 表、不修改 autosave 所有权。
5. P3 测试至少覆盖：旧布局 happy path、空库存、边界填满、动态字段保留、重复迁移不变、非法/损坏 JSON 拒绝、overflow 明细守恒，以及 R3 consumer 在有真实 `SpillContext` 时成功提交、缺上下文/容量不足/持久化失败时保留旧行并可重试。

**落点**：`server/src/player/state.rs:1372-1433`（当前依据）→ R3 P1 后的 inventory slice loader；本 plan §P3。

### P0.7 开放问题决议（pre-P0 收口，2026-08-03）

#### #1 满包溢出策略

**决议**：选择玩家脚下/机制结算点地面掉落，拒绝个人暂存箱。

- 复用现役 `add_item_to_player_inventory_or_ground` 与 `rebuild_and_drop_overflow` 语义，减少第二套持久化、权限、过期与 UI 生命周期。
- 地面掉落保留末法争夺风险；通过 dropped id + instance id + world position/dimension receipt 保证可追踪，并受 §P0.3 的 durable registry 固定上限、原子容量预留和 R6 分片同步约束。
- 不用 TTL/LRU 静默清理物品；队列满时 delivery fail closed 并保留源状态，玩家 pickup/管理员显式运维释放容量。
- 仅容量不足可降级，结构错误 fail closed；无真实世界上下文不得伪造 fallback。

**落点**：`server/src/inventory/mod.rs:1839`、`:5016`（依据）；本 plan §P0.2、§P0.3、§P2。

#### #2 pre-#249 老布局迁移是否并入

**决议**：并入 P3，但在 R3 P1 后实施；R10 提供纯迁移，R3 负责 load/save 编排。

**落点**：`server/src/player/state.rs:1372-1433`（依据）；本 plan §P0.6、§P3。

原开放问题全部已在本节收口。原题保留如下以备追溯，**实施时以 §P0.7 决议为准**：

1. 满包溢出策略：脚下掉落 vs 个人暂存箱。
2. pre-#249 老存档迁移是否并入本轨 P3。

## P1 — 巨石拆分 + `InventoryTxn` 骨架（⬜）

> 前置：R3 P1 已 merge；仅机械拆分，不夹带 P2/P3 行为修复。

- 按 §P0.1 建立 `model.rs`、`registry.rs`、`grid.rs`、`txn.rs`、`container.rs`、`migration.rs`，收薄 `mod.rs`；现有 public import 直接迁到新 canonical 路径并统一 re-export，不建立双实现兼容层。
- 平移现有测试并保持行为等价；新增 module ownership pin，禁止 grid/txn 算法回流 `mod.rs`。按 master §0 的重构轨测试覆盖规则，仅契约 pin 必须保留/迁移；与被删实现绑定的旧单测允许随实现删除，不以原始数量为门禁。
- 落地 §P0.2 类型骨架、staged validation/commit 基础设施与 error/receipt pin tests；生产调用暂不全量切换。
- 验收：server 完整门禁全绿；`mod.rs` 不再承担具体 inventory 算法；contract-pin 清单逐项迁移且无缺失，删除的 implementation-coupled tests 在 PR 描述中列出原测试名与删除理由。

## P2 — 交付/消费路径统一（⬜）

- give、craft、alchemy、forge、loot/pickup 全部改走 `InventoryTxn::deliver` / `consume_checked` / `pickup_and_merge`。
- 删除按错误字符串前缀判断 public fallback 的调用契约，改用 typed error/disposition。
- session 类调用严格执行“txn 成功（stored 或 spilled）后才 teardown”；失败不扣料、不删 session、不删 dropped entry。
- 饱和测试覆盖：空包、恰好放满、少一格、同身份全 merge/部分 merge、不同动态身份拒绝 merge、无 spill context、每类结构错误、revision 单次 bump、总量守恒；另设 `consume_checked` 专属 contract pin：成功消费逐 instance 扣除精确数量、receipt 逐项记录 source/count、revision 只 bump 一次；库存不足/未知 instance/zero quantity 任一失败时 inventory、revision、session 与材料均逐字节/逐字段保持不变，且不得出现部分扣除。
- 既有实例 round-trip contract pin：构造非默认 `instance_id/durability/freshness/mineral_id/charges/forge_*/alchemy/lingering_owner_qi` 的 `ItemInstance`，分别验证直接 placement 与 spill 后所有字段逐项不变、receipt 将原 id 记入 placed/spilled 而非 created；merge 只允许完整 identity 相等且 receipt 保留 source/target/count。
- dropped registry bound pin：`limit-1`、恰好 `limit`、`limit+1`/单事务多 spill 越界；越界时 inventory/revision/session/材料/世界掉落与 persistence 全不变，pickup 后容量可复用，超限 hydration 进入 guard 而不截断。

## P3 — 网格/堆叠一致性 + 迁移（⬜）

- 删除 `force_attach_item_to_inventory` 强塞路径；全量转移、遗骸与套包重建统一合法 attach，overflow 返回显式 spill receipt。
- R10 在 `server/src/inventory/**` 落地拾取合并及 rotate/pack typed outcome；不得把权威 snapshot 当作动作级 receipt。
- **跨轨阻塞交付**：严格执行 §P0.4 的 R10 → R6 → R4 顺序。R6 PR 必须完成 schema/sample/convert/emit/client handler/contract tests，R4 PR 必须完成 handler 调用；两者未 merge 时，本阶段保持 ⬜，不得归档 `rotate-footprint-sync` 或 `bot-inventory-pack-feedback`。
- 按 §P0.6 完成 pre-#249 幂等迁移并接入 R3 hydration seam。
- 饱和测试覆盖每个 footprint 边界、碰撞/swap/rotate 状态、pack rebuild overflow、迁移正反/幂等/字段守恒。

## P4 — bot 验收 + 被吸收 plan 收口（⬜）

1. `inv_full_delivery_matrix`：满包下 craft 完工、取丹、锻造出炉、give，断言 stored + spilled 总量不丢且失败不提前 teardown。
2. `inv_stack_merge`：拾取同身份掉落，断言合并既有堆叠、revision 与 receipt；receipt 的 `incoming_stack_count/incoming_abs_qi_before/target_instance_id` 驱动 pickup attrition，断言 ledger 只扣 incoming 绝对真元、旧 stack 原有绝对真元不变且玩家+zone 守恒；不同动态身份保持分栈。
3. `inv_footprint_sync`：发送 `rotated=true` 的 2×1 move 后，必须收到**该请求之后**、匹配 `instance_id/from/to` 与结果 revision 的 accepted/moved receipt，且 receipt 中权威 item view 为 1×2；Fabric handler contract test 应把本地模型替换为 1×2。拒绝、1×1、非网格目标不得误改 footprint。全量 snapshot 不能替代此动作级断言。
4. `inv_pack_feedback`：对 stow / 空 pack unequip / equip 三个成功请求，逐个等待发送时间锚之后、匹配 `instance_id/from/to` 的 accepted/moved receipt，并断言 receipt revision 与随后 snapshot 一致；对非法位置、非空 pack（若规则禁止）逐个断言 `inventory_move_rejected` 的 reason/instance/from/to，连接保持；预置同 instance 的旧 moved/rejected 事件，证明时间锚与结果 revision 不会误读 stale feedback。VFX 和 snapshot 只可附加验证，**不得作为唯一动作结果或 accepted/rejected 的替代**。
5. `inv_give_visibility_forge`：保留 production forge 的 `/give fan_iron_anvil` → snapshot 真实 instance id → `forge_station_place` → 消耗快照深链回归。
6. `inv_give_visibility_lingtian`：改造 `production_lingtian_gathering_intents.py`，禁止 `hoe_instance_id: 0` 和“任意 server-data 即成功”；`/give hoe_iron` 后必须在带时间锚的新 `inventory_snapshot` 找到 `hoe_iron` 及真实 instance/location，按生产规则把该实例移到 main-hand held（等待匹配请求的 accepted receipt 与权威 snapshot），再用同一非零 instance id 发 `lingtian_start_till`，断言真实 `lingtian_session`/明确业务拒绝回执且请求未因 instance mismatch 被拒。只有 producer→snapshot→equip→till 全链通过，才可归档 visibility skeleton。
7. Python server-data decoder 对新增/扩展 inventory receipt 与 dropped-loot 分片快照做正反样本测试；bot 场景的 request correlation 必须同时使用发送时间锚、instance id、from/to 和 revision，不得命中历史事件。
8. `inv_dropped_loot_bound`：预置到 `MAX_DURABLE_DROPPED_LOOT_ENTRIES`，再分别触发 craft/alchemy/forge/give/loot spill、player discard、death revive 的 TSY/主世界 drop、termination 的 world-drop fallback、TSY relic layer spawn、placeable-break template spawn、backpack unequip overflow、morph-release gate overflow；每个生产 writer 逐域断言通过 `try_insert/try_insert_batch` 获得 typed capacity rejection，源 inventory/revision/session/材料/世界掉落与 persistence 不变；pickup 或显式 durable delete 一件后重试成功。重启超限数据库必须进入 guard 而非截断；分片 sync 每页 ≤ `DROPPED_LOOT_SYNC_PAGE_SIZE`，同 revision 缺页不得替换 client 视图。
9. 按 §P0.5 逐项核验被吸收 skeleton 的代码、跨轨 merge SHA、client/bot 测试与证据，再依仓库三态规则分别收口；本 plan 未全部完成前不写 `Finish Evidence`、不归档。
