# plan-refactor-inventory-core-v1 — Inventory 核心事务契约（重构轨 R10）

> 所属总纲：`plan-refactor-master-v1.md`。P0 只冻结事实、失败边界、跨轨 owner 与审核要求的 pins；不把设计当实现。

## 阶段

- ⏳ P0 完整契约面重写 + absorption audit
- ⬜ P1：inventory 拆分 + txn/capacity 骨架 + inventory-layout/dropped-loot 纯 migration helpers；对应 master M-06/M-13 的 transaction/provider surface，R3 seam 只按 M-04 提供。
- ⬜ P2：production writer 迁移分为 metadata/provider、Public/OwnerOnly writer 与 terminal worker；terminal worker 对应 M-06/O-10..O-27，dropped-loot hydration/projection 与 pickup consumer 只按 master M-13/M-14/M-15 启用。
- ⬜ P3：pickup/merge txn；R4/R5/R6 consumer 与 receipt/attrition 接缝只引用 master M-14/M-15。
- ⬜ P4：联合 bot/e2e + plan 收口；完成 evidence 需覆盖对应 M-row 与 canonical O-row。

所有跨轨 start/order/cutover 只引用 `plan-refactor-master-v1.md §3/§4.1` 与 PR 1902；本阶段表不复制箭头。

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

### 1.2 Terminal-delivery production consumer（P2c）

R10 P2c 交付常驻 `SessionDeliveryWorker`，作为 R3 outbox 的唯一 production consumer。它只实现 R1 canonical obligation reducer 的 O-10..O-21/O-26/O-27，不拥有 gameplay session 状态或 teardown：

```rust
SessionDeliveryWorker::claim_next(now, worker_id)
    -> Result<Option<ClaimedDelivery {
        delivery_id, lease_id, generation, payload, payload_digest
    }>, DeliveryWorkerError>
SessionDeliveryWorker::commit_claimed(claimed, Option<&mut SpillContext>)
    -> Result<DeliveryCommitReceipt, DeliveryWorkerError>
SessionDeliveryWorker::fail(claimed, reason, now)
    -> Result<DeliveryRetryState, DeliveryWorkerError>
```

`commit_claimed` 必须先预留 bounded history capacity（不足走 O-26），再校验 digest 并从 `claimed.payload` decode/validate 唯一 `DeliveryRequest`；caller 无权另传 request。payload 大小、attempt/age 边界只消费 R1 `TerminalDeliveryPolicy`（1 MiB、8 attempts、24 小时），不得在 R10 另定阈值。O-16 transaction 对拍 canonical bytes/digest、semantic item set 与 inventory receipt，并原子提交 inventory/spill、receipt、obligation delete、quota `-Q`。已有 receipt 时返回既有结果，不再次 deliver 或释放 quota。malformed/digest mismatch 命中 O-14，禁止调用 `deliver`；`retry_dead_letter` 与 `resolve_dead_letter` 的 production dispatch path 是 `server/src/cmd/gameplay/session_delivery.rs::SessionDeliveryOperatorCmd`，由 `server/src/cmd/gameplay/mod.rs::register` 注册并由 `handle_session_delivery_operator` 调用；handler 将命令 executor 映射为当前 `MaintenancePrincipal`，再把 canonical delivery id/generation/disposition 交给 worker。它不是 dev-only 命令、维护脚本或测试 helper；无 principal、错误 executor scope、stale generation/lease/disposition replay 均在 worker CAS 前拒绝。retry/lease/dead-letter 只走 O-12/O-13/O-18，resolve 只走 O-19/O-20，绝不恢复 R1 session/claim。

P2c pins 直接执行 R1 O-10..O-21/O-26/O-27：并发唯一 claim、claim 后 crash/lease expiry、payload A/request B 拒绝、history quota fail-before-mutation、deliver 前后 crash、receipt replay、retry/dead-letter/operator CAS、满包 spill。R1 `session_delivery_crash_atomicity` 与 R3 `session_delivery_outbox_atomicity` 必须走真实 worker，不得由 domain 直接调用 `InventoryTxn::deliver` 冒充 consumer。

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

R6 在编码前对每个 recipient 用 server authority 过滤：同 dimension、在授权 distance/zone observation 范围、`OwnerOnly` 仅 owner 或授权管理员可见。排序、revision、page count 都针对过滤后的 projection；只有 visibility key + projection digest 完全相同者可复用编码页，禁止一个 global snapshot 发给所有 client。registry mutation、join，以及 recipient 的 dimension/range observation bucket/owner/admin permission key 变化都递增该 recipient projection revision；context key 变化即使 registry 未变也须先发 `DroppedLootProjectionReset`，使 client fail-closed 清旧视图，再按 R6 的 dirty-recipient queue 重建。empty projection 必须发单个 empty page，不能以零页或沉默表示。

R6 projection 实现必须消费 master M-14 冻结的 aggregate bounds：每 tick 最多重建 64 个 recipient projection、最多发送 4 MiB dropped-loot sync bytes；以 dimension/range spatial index + dirty-recipient queue 避免 recipient×4096 全表扫描，超额保留最新 dirty revision并合并中间版本，reset/revocation 优先于新增可见页。R10 提供 metadata/capacity，不复制这些 wire 调度常量。

## 4. Pickup transaction + authorization

R4 从 ECS 构造不可由 client 覆盖的 `PickupAuthorization`：player UUID、`CurrentDimension`、authoritative position/observation range、owner/private permission、server-resolved entry、revision/anti-replay fact。txn 重新验证 entry/instance、同维、距离/zone、权限和 freshness；知道 dropped id、曾收到 sync 或跨维相同 XYZ 都不构成授权。

顺序固定：authorize + validate merge/placement/capacity → staged attach/merge 与 incoming-only R5 attrition transfer（item qi → authoritative zone + ledger）→ 同一 durable transaction 原子提交 attrited item、inventory revision、zone balance/ledger 与 dropped delete → receipt。任一步失败或崩溃恢复都不得只应用其中一侧；drop 保留且可按 transaction id 重试。placement-only 与 merge 都必须覆盖。

所有 attach 必须 `validate_attach_fits` 后 `attach_at_location`，删除 `(0,0)` 强塞。move/rotate/pack accepted outcome 含 request id、revision、instance/from/to、权威 item view；rejected outcome 含 reason/instance/from/to。snapshot 仅作状态修正，不是动作级反馈。

## 5. Legacy migration

```text
migrate_legacy_inventory_layout(value, schema_version)
  -> MigrationOutcome { migrated_value, overflow: Vec<ItemInstance> }

migrate_legacy_dropped_loot_entry(value, schema_version)
  -> Result<serde_json::Value, DroppedLootMigrationError>
  // owning phase: R10 P1 pure migration helper; consumed by R3 P4 hydration
```

R10 的 inventory-layout 与 dropped-loot migration 函数均纯且幂等，保留所有实例/动态字段，不执行 SQL、不猜 world context、不隐藏 overflow；其中 dropped-loot 迁移把旧 `entry_json` 缺失的 `owner`/`visibility` 明确补为 `owner = None`、`visibility = Public`。R3 hydration consumer 必须先按 persisted schema version 解码/迁移旧 dropped-loot JSON，再构造 `DroppedLootEntry`；inventory-layout migration 则在临时副本上以真实 player/dimension/position 与 capacity/durable seam 消费。两类 migration 都须全成才写新行，失败保留旧行可重试且不重复 drop。

## 6. 所有权与顺序

- **R10** 独占 `server/src/inventory/**`、纯 migration helpers 与 `SessionDeliveryWorker`；terminal worker 只投影 R1 O-10..O-21/O-26/O-27，inventory/spill transaction 不定义 session teardown。
- **R3** 独占 SQL/outbox/CAS/reconciliation、hydration guard 与 migration consumer；R10 通过冻结接口 claim/commit/fail。
- **R4/R5/R6/R7** 分别拥有 C2S authorization、qi attrition ledger、receipt/projection wire 与 UI consumer；inventory accepted/rejected receipt 的 TypeBox/sample/generated/dist 由 master M-16 的 Agent owner 生产，R6 消费冻结 SHA 实现 proto/converter/client，R10 不修改其文件。
- 所有跨轨 start/order/cutover 仅引用 master §3/§4.1 与 PR 1902，不在本 plan 复制箭头。接口可 contract-first 合入；真实 worker activation 必须等 master 列出的 R3 outbox 与 R10 transaction artifacts 存在，且不得用 mock 宣称 production closure。

## 7. 审核要求的 contract pins

仅保留下列 demanded pins；refactor 可删除 implementation-coupled 旧测试，不以数量为门：

1. `consume_checked` 成功精确扣除；insufficient/unknown/zero 失败无 mutation/revision。
2. `deliver` 对 same-template/different-identity、duplicate id、illegal footprint/placement、容量不足但缺 `SpillContext` 逐项 typed reject，且无 mutation/revision；existing instance placement/spill 逐字段保留，created 与 placed/spilled ids 分离。
3. capacity 的 limit-1/limit/limit+1/batch；逐个生产 writer 证明走统一 gate，失败全状态不变；owner-only player discard 覆盖单 `PlayerId` 的 quota-1/quota/quota+1 与 system-reserved boundary，证明一个 owner 不能耗尽 global capacity，系统 writer 在 discard 洪峰下仍可 admission。
4. spill durable write failure、commit interruption、restart/retry：无单边状态、无重复 drop。
5. pickup 同维成功；跨维、超距/zone、owner/private 拒绝；merge、placement-only、failed attach/capacity/validation/persistence 后 entry 仍在；成功后才删。
6. incoming-only attrition receipt + R5 ledger：旧 stack absolute qi 不变；注入 attrition 后、durable commit 中断与 restart/retry，断言 attrited item + zone/ledger + drop delete 原子且总量守恒。
7. visibility matrix：同维/范围内 `Public` 对非 owner 可见，`OwnerOnly` 对 owner 可见、对普通非 owner 不可见、对 server-authorized admin 可见；另测跨维/超距拒绝。page/revision 按每个 recipient projection；缺页/混 revision 不替换。
8. terminal-delivery worker：执行 R1 O-10..O-21/O-26/O-27，覆盖空队列、并发唯一 claim、lease expiry、payload/digest binding、history quota fail-before-mutation、commit crash、receipt replay、retry/dead-letter/operator CAS；quota effect 逐 row 对拍。O-18 retry 与 O-19 discard/requeue 必须分别覆盖 authorized operator 成功、无 operator 身份、错误 role/scope、stale generation/lease/disposition replay 的拒绝矩阵；所有 unauthorized/replay 分支 obligation、quota、inventory、history 均不变。
9. accepted/rejected move correlation；pack stow/equip/unequip 与拒绝必须动作级 receipt，stale event 和 snapshot-only baseline 不通过。
10. forge 深链保留；另锁 `/give hoe_iron → 新 snapshot → 真实非零 instance → held/equip → lingtian_start_till`，禁止 `instance_id=0` 或任意 server-data 冒充成功。
11. inventory-layout migration pure happy/empty/full/dynamic/idempotent/invalid；dropped-loot migration 覆盖旧 `entry_json` 缺 owner/visibility → `None`/`Public`、已有字段原样保留、malformed/幂等；R3 consumer 对真实 context 成功，缺 context/capacity/persistence/migration failure 保留旧行可重试。

## 8. Named bot acceptance（P4）

以下名称即 `scripts/bot/scenarios/<name>.py` 的稳定身份：

1. `inv_full_delivery_matrix`：craft/alchemy/forge/give 满包时 `stored + spilled == requested`；checkpointed 三域经真实 `SessionDeliveryOutbox`→P2c O-10..O-16→receipt 链。handoff 已在 R1 S-14 teardown，worker failure 只留 Pending/DeadLetter obligation，不恢复 session。
2. `inv_stack_merge`：同 identity merge、异 identity 分栈；placement-only 与拒绝路径保留 drop；attrition durable 中断/restart 仍原子守恒。
3. `inv_footprint_sync`：2×1 rotate 后以 request/instance/from/to/revision 锚定 1×2 authoritative receipt；snapshot 不代替回执。
4. `inv_pack_feedback`：stow/equip/unequip 的 accepted/rejected 均按时间锚与 correlation 匹配，stale event 不通过。
5. `inv_give_visibility_forge`：`/give fan_iron_anvil` → 新 snapshot 真实 id → `forge_station_place`。
6. `inv_give_visibility_lingtian`：`/give hoe_iron` → 新 snapshot 非零 id → held/equip receipt → `lingtian_start_till`。
7. `inv_dropped_loot_bound`：全 writer 容量拒绝、delete 后重试、超限 hydration guard、recipient 分页/可见性。

## 9. 吸收边界与 P0 验真

| plan | 2026-08-03 验真结论 | 证据/落点 |
|---|---|---|
| `alchemy-takeback-full-inventory-loss` | 部分吸收 | terminal handoff 后由 obligation worker deliver/spill；失败走 O-13，不 reopen。 |
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

本轮新增 deferred decisions（不扩 P0 实现范围）：

1. **Pickup freshness / anti-replay**：`PickupAuthorization` 的 revision/anti-replay fact 如何生成、绑定和失效，留待 R10 P3 pickup txn 设计时决定；理由是必须与真实 durable transaction/idempotency 语义共同冻结，避免 P0 先拍一个不可验证的 token 形状；交叉引用 §6 顺序与总纲 §3 Wave 2。
2. **Receipt correlation / C2S request ID**：accepted move/rotate/pack receipt 必须贯通稳定 `request_id`，并携 result revision、instance、from/to 与 post-operation authoritative item view；rejected receipt 保留同一 request correlation、reason、instance、from/to。若现行 `inventory_move_intent` 缺 request identity，R6 P4 必须连同 Rust/proto/TypeBox/Java breaking contract 一次补齐并执行其全链 acceptance；这不是可省略的 deferred shape decision。
3. **Recipient-context revocation（已冻结）**：registry mutation、join、dimension/range observation bucket/owner/admin permission key 变化均递增 recipient projection revision；context 变化先发 reset 撤销旧视图，empty projection 仍发单 empty page。实现与 client monotonic assembly 只按 R6/M-14。
4. **Snapshot multiplicativity bound（已冻结）**：R6/M-14 每 tick最多 64 次 recipient rebuild、4 MiB sync bytes；使用 spatial index + dirty queue，不允许 recipient×4096 全表扫描；预算耗尽保留最新 revision 并优先 reset。R10 只提供 registry/index 输入，不另设 wire 常量。

P0 完成只表示上述 surface、owner、失败边界、pins 与逐项 absorption audit 已冻结，不表示后续实现完成。
