# plan-refactor-inventory-core-v1 — Inventory 核心事务契约（重构轨 R10）

> 所属总纲：`plan-refactor-master-v1.md`。P0 只冻结事实、失败边界、跨轨 owner 与审核要求的 pins；不把设计当实现。

## 阶段

- ✅ 2026-08-03 P0 完整契约面重写
- ⬜ A：inventory 拆分 + txn/capacity 骨架
- ⬜ B：全部 production writer 迁移
- ⬜ C：R3 durable seam + R6 wire/client + R4 handler
- ⬜ D：legacy consumer + bot/e2e + plan 收口

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
try_insert / try_insert_batch
```

所有 production writer 禁止直接 `entries.insert`，包括 give/craft/alchemy/forge/loot、player discard、container/pack overflow、death/revive、termination、morph release、`spawn_template_dropped_loot`、`tsy_loot_spawn::spawn_for_layer`、placeable-break 与 TSY layer/relic writers，以及实现波次枚举出的同类 producer。批量事务在 source mutation 前一次 reservation；超限返回 `{current, required, limit}`，所有状态与 DB 不变。pickup/授权 durable delete 才释放容量；不以 TTL/LRU/价值驱逐静默销毁。超限 hydration 由 R3 只读降级并告警，禁止截断或空表覆盖。

内存 reservation 不等于 durability。R3 transaction/outbox 必须把 source inventory/session/material mutation、drop insert/delete、source revision、drop/transaction id 作为一个 recoverable commit。失败/崩溃/重启不得形成“只删 source”或“只写 drop”；按 `(transaction_id, source_revision, dropped_id)` 幂等重试，不丢不重。

## 3. Recipient-specific dropped sync

R6 在编码前对每个 recipient 用 server authority 过滤：同 dimension、在授权 distance/zone observation 范围、owner/private 仅 owner 或授权管理员可见。排序、revision、page count 都针对过滤后的 projection；只有 visibility key 完全相同者可复用编码页，禁止一个 global snapshot 发给所有 client。

## 4. Pickup transaction + authorization

R4 从 ECS 构造不可由 client 覆盖的 `PickupAuthorization`：player UUID、`CurrentDimension`、authoritative position/observation range、owner/private permission、server-resolved entry、revision/anti-replay fact。txn 重新验证 entry/instance、同维、距离/zone、权限和 freshness；知道 dropped id、曾收到 sync 或跨维相同 XYZ 都不构成授权。

顺序固定：authorize + validate merge/placement/capacity → staged inventory commit → durable inventory revision 与 dropped delete 同事务 → receipt/R5 attrition。attach/capacity/auth/persistence 任一失败时 drop 仍在且可重试；placement-only 与 merge 都必须覆盖，成功前不得删除 entry。

所有 attach 必须 `validate_attach_fits` 后 `attach_at_location`，删除 `(0,0)` 强塞。move/rotate/pack accepted outcome 含 request id、revision、instance/from/to、权威 item view；rejected outcome 含 reason/instance/from/to。snapshot 仅作状态修正，不是动作级反馈。

## 5. Legacy migration

```text
migrate_legacy_inventory_layout(value, schema_version)
  -> MigrationOutcome { migrated_value, overflow: Vec<ItemInstance> }
```

R10 函数纯且幂等，保留所有实例/动态字段，不执行 SQL、不猜 world context、不隐藏 overflow。R3 在临时副本上以真实 player/dimension/position 与 capacity/durable seam 消费；全成才写新行，失败保留旧行可重试且不重复 drop。

## 6. 所有权与顺序

- **R10**：`server/src/inventory/**` model/grid/txn/capacity、writer enumeration、typed outcome、纯 migration。
- **R3**：SQL/outbox、spill/pickup recoverable commit、hydration guard、migration consumer。
- **R4**：C2S gate/handler、authoritative pickup context、调用 R10 并转交 R6 outcome。
- **R5**：incoming-only qi attrition/ledger。
- **R6**：receipt wire/client、recipient projection/page、decoder；canonical plan 登记 rotate、pack feedback、dropped sync。
- **R1**：txn stored/spilled 成功后才 teardown，失败保留 session。
- **R7**：UI 消费，不拥有事务。

顺序：R3 P1 → R10 A/B → R3 durable → R6 wire/client → R4 handler → R3 legacy → e2e。R10 不越权改 persistence、wire、handler 或 client。

## 7. 审核要求的 contract pins

仅保留下列 demanded pins；refactor 可删除 implementation-coupled 旧测试，不以数量为门：

1. `consume_checked` 成功精确扣除；insufficient/unknown/zero 失败无 mutation/revision。
2. existing instance placement/spill 逐字段保留，created 与 placed/spilled ids 分离。
3. capacity 的 limit-1/limit/limit+1/batch；逐个生产 writer 证明走统一 gate，失败全状态不变。
4. spill durable write failure、commit interruption、restart/retry：无单边状态、无重复 drop。
5. pickup 同维成功；跨维、超距/zone、owner/private 拒绝；merge、placement-only、failed attach/capacity/validation/persistence 后 entry 仍在；成功后才删。
6. incoming-only attrition receipt + R5 ledger：旧 stack absolute qi 不变且总量守恒。
7. 两 recipient 的 dimension/range/owner-private 正反 visibility；page/revision 按 projection；缺页/混 revision 不替换。
8. accepted/rejected move correlation；pack stow/equip/unequip 与拒绝必须动作级 receipt，stale event 和 snapshot-only baseline 不通过。
9. forge 深链保留；另锁 `/give hoe_iron → 新 snapshot → 真实非零 instance → held/equip → lingtian_start_till`，禁止 `instance_id=0` 或任意 server-data 冒充成功。
10. migration pure happy/empty/full/dynamic/idempotent/invalid；R3 consumer 对真实 context 成功，缺 context/capacity/persistence failure 保留旧行可重试。

## 8. Deferred-to-implementation-wave

P0 不决定 Rust lifetime、SQL/outbox 实现、锁粒度、distance/zone 数值、visibility-key 编码、client 重发、管理员运维或 UI；由 owning PR 设计并受上述 contract/pins 约束。P0 不迁移 writer、不删旧入口、不改 runtime。未跑真实 server→wire→client/bot 链前，不以 forge、snapshot 或文档声明归档 bug skeleton。

## 9. 吸收边界

吸收：`alchemy-takeback-full-inventory-loss`、`dropped-loot-pickup-stack-merge`、`force-attach-grid-collision`、`rotate-footprint-sync`、`bot-inventory-pack-feedback`、`bot-production-inventory-instance-visibility`、`forge-outcome-full-inventory-loss`、`dropped-loot-cross-dimension-pickup`。

不吸收：已闭环 `craft-refund-full-inventory-loss`；独立 feature `container-filter-and-completion`；已撤回 `nested-pack-base`。

P0 完成只表示上述完整 surface、owner、失败边界与 pins 已冻结，不表示后续实现完成。
