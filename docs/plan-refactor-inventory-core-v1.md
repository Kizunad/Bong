# plan-refactor-inventory-core-v1 — Inventory 核心事务契约（重构轨 R10）

> 所属总纲：`plan-refactor-master-v1.md`。P0 只冻结事实、失败边界、跨轨 owner 与审核要求的 pins；不把设计当实现。

## 阶段

- ✅ 2026-08-03 P0 完整契约面重写 + absorption audit
- ⬜ P1：inventory 拆分 + txn/capacity 骨架
- ⬜ P2：全部 production writer 迁移
- ⬜ P3：纯 migration + R3 durable seam + R6 wire/client + R4 handler
- ⬜ P4：R3 legacy consumer + bot/e2e + plan 收口

实现属 Wave 2；跨轨工作须登记 owning plan。

## 1. 完整 contract surface

| 面 | P0 冻结的事实 |
|---|---|
| `InventoryTxn` | staged `deliver` / `consume_checked` / `pickup_and_merge`；成功最多一次 revision bump，所有验证先于 mutation |
| spill/overflow | 原实例守恒、全 writer 统一 capacity API、source 与 durable drop 可恢复原子提交 |
| dropped sync | server 按 recipient 授权投影后分页；无全局 snapshot 广播/复用 |
| pickup | authoritative dimension/range/ownership 授权；inventory commit 后才删 drop |
| legacy migration | R10 纯转换，R3 用真实 world context 消费 overflow；全成才写新 schema |

### 1.1 `InventoryTxn`

```rust
InventoryTxn::deliver(DeliveryRequest, Option<&mut SpillContext>)
    -> Result<InventoryDeliveryReceipt, InventoryTxnError>
InventoryTxn::consume_checked(ConsumeRequest)
    -> Result<InventoryConsumeReceipt, InventoryTxnError>
InventoryTxn::pickup_and_merge(PickupRequest, PickupAuthorization, &mut DroppedLootRegistry)
    -> Result<InventoryPickupReceipt, InventoryTxnError>
```

共同规则：验证在 staged view 完成；失败时 inventory/revision/session/material/registry 不变；成功最多 bump 一次 revision。堆叠必须完整 identity 相同，不能只比 `template_id`。错误至少区分 unknown/zero/insufficient/identity mismatch/invalid placement/capacity/missing spill context/unauthorized/wrong dimension/persistence unavailable。

`deliver` 同时支持 minted template 与既有 `ItemInstance`。后者必须保留 id、durability、freshness、attributes/NBT、charges、forge/alchemy 与 owner-qi 等动态字段，禁止重建默认实例。receipt 含 request id、revision、created ids、placed existing ids、merge source/target/count、spill dropped/source/count/location，且 `stored + spilled == requested`；既有 id 不得记作 created。

`consume_checked` receipt 逐 instance 记录扣除量/剩余量；insufficient、unknown、zero 任一失败不得部分扣除。

`pickup` receipt 含 request id、revision、removed drop、merge/placement、`target_instance_id`、`incoming_instance_id/count/abs_qi_before`。placement 的 target 等于 incoming；merge 的 target 是提交后既有 stack。R5 只按 incoming absolute qi 做 attrition：`target_after = preexisting_abs_qi + incoming_after`，不得磨损旧数量或由合并后整栈反推。

## 2. Spill / overflow 守恒

只允许容量不足 spill 到真实玩家脚下/机制结算点；结构错误、无真实 dimension/position、ID 冲突、非法 footprint 均 fail closed。`SpillContext` 必须含 source identity/revision、真实 dimension/position、registry、durable seam 与 transaction id；不得猜位置或留下无主 handoff。

`DroppedLootRegistry` 是有界 durable queue：

```text
MAX_DURABLE_DROPPED_LOOT_ENTRIES = 4096
MAX_OWNER_ONLY_DISCARD_ENTRIES_PER_PLAYER = 256
SYSTEM_RESERVED_DURABLE_DROPPED_LOOT_ENTRIES = 512
try_insert / try_insert_batch
```

所有 production writer 禁止直接 `entries.insert`，包括 give/craft/alchemy/forge/loot、player discard、container/pack overflow、death/revive、termination、morph release、`spawn_template_dropped_loot`、`tsy_loot_spawn::spawn_for_layer`、placeable-break 与 TSY layer/relic writers，以及实现波次枚举出的同类 producer。批量事务在 source mutation 前一次 reservation；超限返回 `{current, required, limit}`，所有状态与 DB 不变。`OwnerOnly` 的 player discard 同时受每个 `PlayerId` 的 `MAX_OWNER_ONLY_DISCARD_ENTRIES_PER_PLAYER` 配额和 `SYSTEM_RESERVED_DURABLE_DROPPED_LOOT_ENTRIES` 系统保留容量约束，不得消耗系统保留区；craft/alchemy/forge/loot/death/termination 等 production/system writer 才能预留保留区。单一 owner 因此不能填满全局 queue，其他玩家的 production spill 在 discard 洪峰下仍保有 bounded admission。pickup/授权 durable delete 才释放容量；不以 TTL/LRU/价值驱逐静默销毁。超限 hydration 由 R3 只读降级并告警，禁止截断或空表覆盖。

内存 reservation 不等于 durability。R3 transaction/outbox 必须把 source inventory/session/material mutation、drop insert/delete、source revision、drop/transaction id 作为一个 recoverable commit。失败/崩溃/重启不得形成“只删 source”或“只写 drop”；按 `(transaction_id, source_revision, dropped_id)` 幂等重试，不丢不重。

## 3. Recipient-specific dropped sync

`DroppedLootEntry` 必须持久化 `owner: Option<PlayerId>` 与 `visibility: Public | OwnerOnly`；producer 从机制权威 source 写入，普通 world loot 为 `Public`，私人 spill/drop 为 `OwnerOnly`。管理员授权来自 server permission，不写入 client payload；R3 migration/hydration 原样保留这些字段，缺失旧数据仅按明确 migration 规则补 `Public`，不得从请求猜 owner。

R6 在编码前对每个 recipient 用 server authority 过滤：同 dimension、在授权 distance/zone observation 范围、`OwnerOnly` 仅 owner 或授权管理员可见。排序、revision、page count 都针对过滤后的 projection；只有 visibility key 完全相同者可复用编码页，禁止一个 global snapshot 发给所有 client。

## 4. Pickup transaction + authorization

R4 从 ECS 构造不可由 client 覆盖的 `PickupAuthorization`：player UUID、`CurrentDimension`、authoritative position/observation range、owner/private permission、server-resolved entry、revision/anti-replay fact。txn 重新验证 entry/instance、同维、距离/zone、权限和 freshness；知道 dropped id、曾收到 sync 或跨维相同 XYZ 都不构成授权。

顺序固定：authorize + validate merge/placement/capacity → staged attach/merge 与 incoming-only R5 attrition transfer（item qi → authoritative zone + ledger）→ 同一 durable transaction 原子提交 attrited item、inventory revision、zone balance/ledger 与 dropped delete → receipt。任一步失败或崩溃恢复都不得只应用其中一侧；drop 保留且可按 transaction id 重试。placement-only 与 merge 都必须覆盖。

所有 attach 必须 `validate_attach_fits` 后 `attach_at_location`，删除 `(0,0)` 强塞。move/rotate/pack accepted outcome 含 request id、revision、instance/from/to、权威 item view；rejected outcome 含 reason/instance/from/to。snapshot 仅作状态修正，不是动作级反馈。

## 5. Legacy migration

```text
migrate_legacy_inventory_layout(value, schema_version)
  -> MigrationOutcome { migrated_value, overflow: Vec<ItemInstance> }
```

R10 函数纯且幂等，保留所有实例/动态字段，不执行 SQL、不猜 world context、不隐藏 overflow。R3 在临时副本上以真实 player/dimension/position 与 capacity/durable seam 消费；全成才写新行，失败保留旧行可重试且不重复 drop。

## 6. 所有权与顺序

- **R10**：`server/src/inventory/**` model/grid/txn/capacity、writer enumeration、typed outcome、纯 migration。P1 仅在 **R3 P1** 的 inventory/overflow seam 冻结后实现 txn/capacity 骨架；P2 production writers 只有在 **R3 P3** durable spill/pickup recoverable-commit seam 已合入后才可迁移并宣称完成；P3 pickup/attrition consumer 只有在 **R5 P3** incoming-only attrition/ledger API 与 **R6 P4** receipt wire/client API 已合入后才可接通。
- **R3**：SQL/outbox、spill/pickup recoverable commit、hydration guard、migration consumer；R10 只消费 R3 P1/P3/P4 已冻结的接口。
- **R4**：C2S gate/handler、authoritative pickup context、调用 R10 并转交 R6 outcome；R4 handler/consumer phase 必须等待 **R6 P4** receipt API 与 **R5 P3** attrition API，不得以 R10 mock 或仅 R6 P1 schema 代替。
- **R5**：incoming-only qi attrition/ledger；provider phase 为 R5 P3。
- **R6**：receipt wire/client、recipient projection/page、decoder；canonical plan 登记 rotate、pack feedback、dropped sync；receipt provider phase 为 R6 P4。
- **R1**：txn stored/spilled 成功后才 teardown，失败保留 session。
- **R7**：UI 消费，不拥有事务。

顺序：**R3 P1 → R10 P1 → R3 P3 → R10 P2 production writers → R5 P3 + R6 P4 → R4 handler/pickup consumer → R10 P3 migration/consumer → R3 P4 legacy consumer → e2e**。R10 P2 在 R3 P3 durable seam 未合入前只能保留 skeleton，R4 pickup consumer 在 R5 P3/R6 P4 provider 未合入前不得宣称完成；R10 不越权改 persistence、wire、handler 或 client。

## 7. 审核要求的 contract pins

仅保留下列 demanded pins；refactor 可删除 implementation-coupled 旧测试，不以数量为门：

1. `consume_checked` 成功精确扣除；insufficient/unknown/zero 失败无 mutation/revision。
2. `deliver` 对 same-template/different-identity、duplicate id、illegal footprint/placement、容量不足但缺 `SpillContext` 逐项 typed reject，且无 mutation/revision；existing instance placement/spill 逐字段保留，created 与 placed/spilled ids 分离。
3. capacity 的 limit-1/limit/limit+1/batch；逐个生产 writer 证明走统一 gate，失败全状态不变；owner-only player discard 覆盖单 `PlayerId` 的 quota-1/quota/quota+1 与 system-reserved boundary，证明一个 owner 不能耗尽 global capacity，系统 writer 在 discard 洪峰下仍可 admission。
4. spill durable write failure、commit interruption、restart/retry：无单边状态、无重复 drop。
5. pickup 同维成功；跨维、超距/zone、owner/private 拒绝；merge、placement-only、failed attach/capacity/validation/persistence 后 entry 仍在；成功后才删。
6. incoming-only attrition receipt + R5 ledger：旧 stack absolute qi 不变；注入 attrition 后、durable commit 中断与 restart/retry，断言 attrited item + zone/ledger + drop delete 原子且总量守恒。
7. 两 recipient 的 dimension/range/owner-private 正反 visibility；page/revision 按 projection；缺页/混 revision 不替换。
8. accepted/rejected move correlation；pack stow/equip/unequip 与拒绝必须动作级 receipt，stale event 和 snapshot-only baseline 不通过。
9. forge 深链保留；另锁 `/give hoe_iron → 新 snapshot → 真实非零 instance → held/equip → lingtian_start_till`，禁止 `instance_id=0` 或任意 server-data 冒充成功。
10. migration pure happy/empty/full/dynamic/idempotent/invalid；R3 consumer 对真实 context 成功，缺 context/capacity/persistence failure 保留旧行可重试。

## 8. Named bot acceptance（P4）

以下名称即 `scripts/bot/scenarios/<name>.py` 的稳定身份：

1. `inv_full_delivery_matrix`：craft/alchemy/forge/give 满包时 `stored + spilled == requested`，失败不 teardown。
2. `inv_stack_merge`：同 identity merge、异 identity 分栈；placement-only 与拒绝路径保留 drop；attrition durable 中断/restart 仍原子守恒。
3. `inv_footprint_sync`：2×1 rotate 后以 request/instance/from/to/revision 锚定 1×2 authoritative receipt；snapshot 不代替回执。
4. `inv_pack_feedback`：stow/equip/unequip 的 accepted/rejected 均按时间锚与 correlation 匹配，stale event 不通过。
5. `inv_give_visibility_forge`：`/give fan_iron_anvil` → 新 snapshot 真实 id → `forge_station_place`。
6. `inv_give_visibility_lingtian`：`/give hoe_iron` → 新 snapshot 非零 id → held/equip receipt → `lingtian_start_till`。
7. `inv_dropped_loot_bound`：全 writer 容量拒绝、delete 后重试、超限 hydration guard、recipient 分页/可见性。

## 9. 吸收边界与 P0 验真

| plan | 2026-08-03 验真结论 | 证据/落点 |
|---|---|---|
| `alchemy-takeback-full-inventory-loss` | 部分吸收 | 满包 deliver/spill 后才由 R1 teardown。 |
| `dropped-loot-pickup-stack-merge` | 仍真实，吸收 | 当前 pickup 只找空 footprint；落 `pickup_and_merge`。 |
| `force-attach-grid-collision` | 仍真实，吸收 | `(0,0)` 强塞仍可达；改合法 attach/spill。 |
| `rotate-footprint-sync` | 仍真实，吸收 | `Moved` 缺权威新 footprint；R6 receipt 补齐。 |
| `bot-inventory-pack-feedback` | 仍真实，吸收 | pack 成功路径可只有 snapshot；动作级 receipt 补齐。 |
| `bot-production-inventory-instance-visibility` | 部分闭环，吸收剩余 | forge 已用真实 id；lingtian 仍有 id=0 baseline，场景 6 锁定。 |
| `forge-outcome-full-inventory-loss` | 仍真实，吸收 | #1294 只建立 skeleton；outcome 改走统一 deliver/spill。 |
| `dropped-loot-cross-dimension-pickup` | 仍真实，吸收 | 当前 entry 有 dimension 但 pickup 未获 `CurrentDimension`；R4 authorization 补齐。 |

不吸收：已闭环 `craft-refund-full-inventory-loss`；独立 feature `container-filter-and-completion`；已撤回 `nested-pack-base`。P0 仅在逐项复读代码/plan 并记录上述 live/fixed/invalid 结论后完成；P4 仍须按表逐项核验 merge SHA 与 bot/client 证据后才能归档。

## 10. Deferred-to-implementation-wave

P0 不决定 Rust lifetime、SQL/outbox 实现、锁粒度、distance/zone 数值、visibility-key 编码、client 重发、管理员运维或 UI；由 owning PR 设计并受上述 contract/pins 约束。P0 不迁移 writer、不删旧入口、不改 runtime。未跑真实 server→wire→client/bot 链前，不以 forge、snapshot 或文档声明归档 bug skeleton。

P0 完成只表示上述 surface、owner、失败边界、pins 与逐项 absorption audit 已冻结，不表示后续实现完成。
