# plan-container-filter-and-completion-v1 — owner-instance 容器筛选与九个随身容器闭环

> **主题**：在当前 `ContainerState.owner_instance_id` + `PlayerInventory.containers` 平展模型上，统一容器 owner/filter 解析、权威移动门、保鲜状态迁移、九个随身容器数据与客户端预提示。
> **状态**：Active。P0 已于 2026-06-13 合入；P1 有零散代码超前，但 `InventoryMoveIntent` 权威玩家移动链尚未闭环，因此本 plan 仍在实施中。
> **历史证据**：P0 = PR #526，merge commit `3161ccf0ba1ff25d5ab781e654667090b0e143ac`（2026-06-13）。该证据只覆盖 P0 明列的数据模型与测试，不外推 P1–P4 完成度。

## 阶段总览

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | `ItemCategory` 三变体 + `ContainerAcceptFilter` + `ContainerSpec.accept_filter` + TOML `accept` + `item_passes_filter` | ✅ | 2026-06-13 |
| P1 | `PreparedInventoryMutation` 与跨库存 staged transaction、owner/filter resolver、所有端点及自动入口/owner-loss 原子提交 | 🔄 | |
| P2 | owner freshness、`integrity_lock`，以及唯一 server 内 reconciliation/shelflife/snapshot 调度 | ⬜ | |
| P3 | 暗器 category 迁移 + 九件完整 `ContainerSpec`、coin 文案与 registry/save-load 闭环 | ⬜ | |
| P4 | `ContainerSnapshotV1.accept_filter` / persisted `acceptance_lock` 跨端同步 + Worn/Pack/Inspect 三 surface、e2e 与同 PR 归档 | ⬜ | |

> P1 的 `🔄` 仅表示 `container_accepts_runtime_grant` 已有部分 owner/filter 代码：权威玩家 move 仍未使用同一 resolver，swap 也未双向执行 filter 校验。不得据此将 P1 标成完成。

## 接入面（防孤岛）

- **进料**：
  - `server/src/inventory/mod.rs::PlayerInventory.containers`：所有运行时容器的平展集合。
  - `server/src/inventory/mod.rs::ContainerState.owner_instance_id`：随身容器到 owner 物品实例的唯一权威归属。
  - `server/src/inventory/mod.rs::{ContainerSpec.accept_filter, ContainerAcceptFilter, item_passes_filter}`：P0 已落地的筛选定义与纯判定函数。
  - `find_live_container_owner` 唯一从 worn、held、hotbar、`body_pocket` 寻找 live owner；`main_pack` / `small_pouch` / `front_satchel`、任意 `pack_*` grid 与 owner 自身容器都不属 eligible surface。
  - `server/src/schema/client_request.rs::ClientRequestV1::InventoryMoveIntent` → `server/src/network/client_request_handler.rs::handle_inventory_move` → `server/src/inventory/mod.rs::apply_inventory_move_with_race`：普通玩家移动的唯一权威链；其目标 wear、`SlotMove` attrition、owner/pack rebuild 与最终 snapshot request 必须同一 prepared mutation。
  - `server/src/inventory/mod.rs::{apply_death_drop_on_revive, apply_death_drop_to_inventory, transfer_remains_to_looter, apply_treasure_activate}`、`server/src/inventory/tsy_death_drop.rs::apply_tsy_death_drop`、`server/src/combat/resolve.rs` 与 `server/src/forge/artifact_meridian.rs`：死亡、遗骸、法宝停用和装备破损也必须通过 P1 typed prepared ingress，不得保留 first-fit/live-write 旁路。
  - `server/src/inventory/reconciliation.rs::{InventoryReconciliationSet, ContainerIntegrityFreshnessQueue, InventorySnapshotReason, InventorySnapshotRequest, InventorySnapshotOutbox, collect_inventory_snapshot_requests, emit_queued_inventory_snapshots}`：P2 唯一 server 内 reconciliation/shelflife/snapshot 调度；integrity queue 仅是 Reconcile 输入，snapshot 严格走 Event→outbox→emit 管线。
- **出料**：
  - runtime grant、move 与 swap 共用 `classify_container_ownership` / `find_live_container_owner` / owner-filter 解析；所有 `InventoryLocationV1::Container` 源、目标端点均先过结构门，源端不对移出物作 accept filter。
  - P1 的 `PreparedInventoryMutation<'a>` 是 private、non-Clone、`#[must_use]` 的独占借用事务：从 prepare 到 commit 持有 `&'a mut PlayerInventory`，不得进入 resource/event、跨 handler/tick、与第二个 prepared mutation 并存，亦不得在其存活时改 revision 或 layout。它只携带 staged copies、route、merge/push 计划、non-Clone `Vec<PreparedInventoryDrop>` 与一次性 outcome；`original_revision` 仅用于 invariant/debug assert，不采用 CAS。
  - `prepare_inventory_mutation_with` 返回该借用载体；`commit_prepared_inventory_mutation(self)` 消费 self、不可失败且唯一写边：一次替换 inventory、一次 enqueue 每个预校验 drop、一次发布 outcome，最后 revision 恰好 +1。顺序发生的下一笔 mutation 必须从新 revision 重新 prepare。
  - P2 的 `reconcile_container_integrity_freshness` 只能由 `Reconcile.after(Commit)` 消费 `ContainerIntegrityFreshnessQueue`，业务 caller 只能 enqueue entity/reason，不能在业务 prepare 前直接调用。若同一 prepared commit 已纳入 integrity/freshness repair，则由该 commit 单次 +1；否则首次 lock/reason-change/repair 作为独立 reconciliation commit +1，same reason/no-op=0。move/swap 的 typed owner reject、grant 的 retriable `InventoryGrantError::IntegrityReconciled` 与即时 corrective request 均由该序列的最终状态决定并经唯一 emitter 同 tick输出；输入不吞、不得同调用隐式 retry。
  - P2 的 reconciliation/queue 在唯一调度序内完成：同一业务 prepared commit 若已包含 integrity/freshness 修复，只由该 commit revision +1；独立首次 lock、reason-change 或 repair 才作为独立 reconciliation commit 恰好 +1；same reason/no-op 为 0。P2 只让 `emit_queued_inventory_snapshots` 在 Reconcile/Sweep 后读取最终 server 状态；P4 才把 persisted reason 投影为 optional `acceptance_lock`，供 `WornContainerPanel`、`PackContainerWindow`、`InspectScreen` 做非权威预提示，P4 不下发 `detected_tick`。
- **共享类型 / event**：复用 `ItemCategory`、`ContainerAcceptFilter`、`ContainerFreshnessBehavior`、`InventoryMoveRejectReason`、`InventoryMoveOutcome`、`InventoryLocationV1`；新增的 reconciliation queue/request 只服务 server 内唯一调度，不另造第二套移动协议或容器状态树。
- **跨仓库契约**：
  - server：`ContainerSnapshotV1` / `InventoryItemViewV1`、`inventory_snapshot_emit::build_inventory_snapshot`、`schema::proto_convert`。
  - schema：`agent/packages/schema/src/inventory.ts::{ContainerSnapshotV1, InventoryItemViewV1}` + inventory snapshot sample；agent 无玩法逻辑，但 TypeBox 仍是 wire 镜像的一部分。
  - protobuf：`proto/bong/envelope.proto::{ContainerSnapshot, InventoryItemView}`。
  - client：`InventorySnapshotHandler` → `InventoryModel.ContainerDef` / `InventoryItem` → `InventoryStateStore` → `WornContainerPanel` / `PackContainerWindow` / `InspectScreen`。
- **worldview 锚点**：保鲜容器呼应 `worldview.md §二 L157-L163` 的离体真元易挥发；`coin_box` 只做骨币品类筛选，遵守 `worldview.md §九 L844-L850` 的骨币流转约束，不增加骨币保鲜或囤积收益。
- **qi_physics 锚点**：本 plan 只选择既有 `ContainerFreshnessBehavior` 并维护既有冻结时间字段，不定义任何衰减率公式或新的物理常数。过滤与移动只改变库存归属，不生成、吞噬或重复结算真元/灵气；若实现发现必须改变真元量，必须停下并接 `qi_physics::ledger::QiTransfer` 既有守恒入口，而非在本 plan 内私设扣减或返还公式。

## 当前架构基线与范围边界

1. `ContainerState.owner_instance_id` 是 owner 真源；`pack_<instance_id>` 仅保留为稳定 UI / 协议 `container_id`。实现只能用它与 owner 字段核对，绝不得从 ID 后缀反推出或补写权威 owner。
2. `classify_container_ownership` 是唯一分类入口，静态 allowlist 固定且穷尽为 `body_pocket`、`main_pack`、`small_pouch`、`front_satchel`（锚：`server/src/inventory/mod.rs:83-107,6549-6592`）。它不接受“已知”“预期”等推断分类：
   - allowlist + `owner_instance_id=None` → `Static`，`accept_filter=None`，全收（`Normal` freshness）；
   - allowlist + `owner_instance_id=Some(_)` → `ContainerOwnerInvalid`；
   - 非 allowlist + `owner_instance_id=None`（未知 ID、伪 `pack_` 文本、placeholder 都包括）→ `ContainerOwnerMissing`；
   - 非 allowlist + `owner_instance_id=Some(owner_id)` → 只按该字段调用 `find_live_container_owner` 查实例，再查 template/`ContainerSpec`，并核对 ID 必须为 `pack_<owner_id>`；任一缺失、无效或不匹配均 typed reject。
3. `find_live_container_owner` 是唯一 owner lookup surface，供 classify/resolve、P1 rebuild、grant、move、swap、P2 freshness/reconcile/sweep、snapshot 共同复用：只允许 worn、held、hotbar、`body_pocket` 命中 owner instance；明确排除 `main_pack` / `small_pouch` / `front_satchel`、任意 `pack_*` grid 与 owner 自己的容器。eligible surface 无匹配 instance → `ContainerOwnerNotFound`；匹配实例但 registry template 缺失或无 `ContainerSpec` → `ContainerOwnerInvalid`。不得从 ID 推 owner。
4. 所有 `InventoryLocationV1::Container` 端点（from/to）必须在其它语义校验和 mutation 前走同一 classify/resolve：源端只校验 ID/owner 结构，绝不对移出的物品应用 accept filter；目标端再校验结构和 accept filter。普通 move、swap、container→hotbar 与 container→equip 全部适用。owner instance 进入自己的 `pack_<owner_id>`（普通 move 或 swap 任一方向）在 prepare 前固定为 `InventoryMoveRejectReason::ContainerOwnerSelfContainment`；它不是 integrity lock reason，拒绝时 inventory/revision/drop/outcome 全部不变。legacy 已自包含仍按 owner corruption/reconcile 处理。
5. `PreparedInventoryMutation<'a>` 是 private、non-Clone、`#[must_use]` 的唯一库存事务载体：其 prepare→commit 全程独占 `&'a mut PlayerInventory`，不得存入 resource/event、跨 handler/tick 或与第二个 prepared mutation 并存。它只持有 staged item/container/equip/hotbar、from/to route、候选 merge/push、non-Clone `Vec<PreparedInventoryDrop>`、一次性 outcome 与 `original_revision` invariant/debug assert；不实现 CAS。`prepare_inventory_mutation_with` 返回该借用载体，prepared 存活时 API 必须无法二次借用或改 revision/layout；顺序第二笔 mutation 只能从新 revision prepare。
6. `commit_prepared_inventory_mutation(self)` 消费 prepared self 且不可失败：rebuild/spill 只在 staged inventory 上完成，prepare 预校验 dimension/world position/spawn context，任何 prepare 失败均零 live/drop/outcome 副作用；commit 一次替换库存、为每个 overflow remainder（包括 merge 后余量）保留原 `ItemInstance.instance_id` 并恰好 enqueue 一次 drop、发布一次 outcome，最后 revision +1。commit 内不得有可失败操作、重试或重复 spawn。
7. P2 的 `reconcile_container_integrity_freshness` 只能经唯一 reconciliation 调度执行；它与业务 prepare 的 revision 边界为：同一 prepared commit 已纳入 integrity/freshness 修复时只 +1，绝不由 reconcile 再 bump；独立首次 lock、reason change 或 repair 才独立 +1，same reason/no-op=0。即时 corrective snapshot 仍在同 tick 输出，但只能读取 Reconcile/Sweep 后的最终 inventory，绝不从任意业务 caller 旁路 emit。
8. `water_skin` 已移出本 plan scope：不为它定义 grid、filter、`ContainerSpec`、随身包验收或任何剩余 PR 交付物。
9. `trade_crate` 与 `dead_drop_box` 属 placeable container 计划，不计入本 plan 的九个随身容器，也不在本 plan 实装方块放置、打开或持久化。`herb_crate_placed` 是 `herb_crate` 的独立 placeable twin：P3 不修改它，也不实现 portable↔placed 转换。

## 历史否决：不得恢复 nested/session 路线

旧的嵌套/session 架构已被否决；P1–P4 不得恢复其 client request、session 或子容器字段，不得新增旧 wire 的兼容分支，也不得把它们写成依赖、TODO、测试目标或验收入口。当前唯一移动入口始终是 `InventoryMoveIntent`。

## P0 — 筛选数据模型 ✅ 2026-06-13

**已完成范围仅限以下交付物**（PR #526 / `3161ccf0ba1ff25d5ab781e654667090b0e143ac`）：

- `server/src/inventory/mod.rs::ItemCategory::{Mineral, Anqi, Liquid}` 及 TOML category 解析。
- `server/src/inventory/mod.rs::ContainerAcceptFilter::{Category, TemplatePrefix}`。
- `server/src/inventory/mod.rs::ContainerSpec.accept_filter` 与 TOML `[item.container].accept` 解析。
- `server/src/inventory/mod.rs::item_passes_filter`：`None` / empty 全收；非空列表按 OR 语义匹配 category 或 template prefix。
- 对应 category、TOML、serde、默认值、单 filter、多 filter 与正反路径测试。

**不属于 P0 完成证据**：runtime grant 接线、玩家移动校验、owner 损坏拒绝、freshness、九容器数据、snapshot/client wire、UI 提示与 e2e。上述全部留在 P1–P4。

## P1 — owner/filter 统一 resolver、一次性迁移与全入口事务 🔄

### 可核验交付物

1. 在 `server/src/inventory/mod.rs` 冻结唯一 `classify_container_ownership` 和其上的 `resolve_container_acceptance`：输入 `&PlayerInventory`、`&ItemRegistry`、`container_id`，分类严格遵循「当前架构基线」的四行真值表；输出目标 `ContainerState`、`Resolved` 或 `Locked` 及归一化 filter。它只能经 `find_live_container_owner` 查询 owner，静态 allowlist 只能是 `body_pocket` / `main_pack` / `small_pouch` / `front_satchel`，所有 owner-backed 判断只读 `ContainerState.owner_instance_id`。P1 固定内部 `ContainerAcceptanceLockReason::{OwnerMissing, OwnerNotFound, OwnerInvalid}`；同 PR 将 `rebuild_containers_from_equipment` 的 live 判据替换为该 helper，owner 从 eligible surface 离开时，在同一 mutation commit 原子 spill/remove，full overflow 走既有 drop outcome，不能先留下 locked container。
2. resolver 的接受语义严格为：
   - `Static` 的 `None` filter 全收，静态容器带 owner 一律 `InventoryMoveRejectReason::ContainerOwnerInvalid`；
   - 非 allowlist 无 owner 一律 `ContainerOwnerMissing`，包括未知 ID、伪 `pack_` 文本和 placeholder；
   - 非 allowlist 有 owner 时，eligible surface 没有 owner instance 为 `ContainerOwnerNotFound`；template 缺失、无 `ContainerSpec` 或 `container_id != pack_<owner_id>` 为 `ContainerOwnerInvalid`；
   - 仅完整 `Resolved` 的**目标端**非空 filter 未命中才是 `ContainerFilterRejected`，错误携带 `container_id`、目标物 `template_id` 与 filter 摘要。
3. 替换 `server/src/player/state.rs::backfill_owner_instance_ids` 的“每次 load 按 `pack_` 前缀回填、却不写库”路径，实施一次性、版本化、可重试的 owner migration。当前真实存档锚点是 `inventories.schema_version` 与 `INVENTORY_SCHEMA_VERSION = 2`（不是无版本存档）：定义紧随 v2 的下一 inventory schema migration，并在 `load_player_inventory_from_sqlite`、`save_player_inventory_slice`、`persist_player_inventory_slice_in_sqlite`、`persist_player_inventory_json_in_transaction` 的真实读取/事务写入路径中显式传递 migration pending 状态。迁移本身不得 bump gameplay `InventoryRevision` 或 emit snapshot；只有 inventory JSON **成功保存且 SQLite transaction commit 成功**后，`inventories.schema_version` 才可升到新值。serialize/SQL/commit 失败必须保留 pending/dirty 以供下次保存重试，不能把未落盘的内存修补当已迁移。
4. owner migration 只处理旧 schema 且 `owner_instance_id == None` 的 container，且 `container.id` 必须被新的严格 canonical parser 识别为精确 `pack_<u64>`：数字文本必须等于 `u64::to_string()`，故拒绝 `pack_01`、空 suffix、符号、空白、溢出与任何杂字；不得复用当前宽松的 `worn_pack_instance_from_container_id` 作为迁移判据。仅当该 instance 在 worn / held / hotbar / `body_pocket` 四个 eligible surface 中**唯一**命中、registry template 存在且有 `ContainerSpec` 时才写 `Some(id)`。零候选、多候选、非容器模板、owner 只在 `main_pack` / 普通 grid / 任意嵌套 `pack_*` grid、或 legacy self-contained 一律不猜测、保持 `None`，交 P2 reconcile lock；已有 `Some(_)` 永不覆盖。migration 成功并 reload 后不得再依赖 prefix 回填。
5. `PreparedInventoryMutation<'a>` 必须为 private、non-Clone、`#[must_use]`，从 `prepare_inventory_mutation_with` 返回开始到 `commit_prepared_inventory_mutation(self)` 消费结束一直独占 `&'a mut PlayerInventory`；不得存入 resource/event、跨 handler/tick，不能与第二个 prepared mutation 并存。其字段只含 staged copies、from/to route、候选 stack merge/push、non-Clone `Vec<PreparedInventoryDrop>`、一次性 outcome 与 `original_revision` invariant/debug assert；不使用 CAS。prepared 存活时不得触碰 live item/container/equip/hotbar/revision；API 以借用保证不能二次借用或改 revision/layout，顺序第二笔 mutation 必须从新 revision prepare。
6. `prepare_inventory_mutation_with` 先对全部 `InventoryLocationV1::Container` from/to 做结构 resolve（源仅结构，目标结构+filter），并在普通 move 或 swap 任一方向将 owner instance 移进自己的 `pack_<owner_id>` 时返回 `InventoryMoveRejectReason::ContainerOwnerSelfContainment`；随后对每个 incoming logical item staged copy 调用不可失败的 `InventoryItemPrepareFn`，按 prepare 后完整 stack identity 重算 merge/push/placement，最后 bounds/collision/weight 校验。rebuild/spill 只在 staged inventory 完成；overflow remainder（含 merge 后余量）保留原 `ItemInstance.instance_id`，prepare 必须先验证 dimension/world position/spawn context，任何错误均为 live/drop/outcome 零副作用。
7. `commit_prepared_inventory_mutation(self)` 是唯一写边且不可失败：它消费 self，一次替换 detach/attach/merge/push/equip/hotbar/staged 字段与 rebuild/spill 后的库存，一次 enqueue 每个已预校验的 `PreparedInventoryDrop`，一次发布 outcome，最后 revision 恰好 +1。commit 内禁止可失败操作、隐式重试与重复 spawn。P1 生产调用传 identity `InventoryItemPrepareFn`；禁止在 prepare/hook 前决定 merge，禁止任何提交后补写。
8. 所有生产自动 ingress 与 owner/filter/prepared 入口都必须改走同一个 `prepare_inventory_mutation_with`（或其 typed prepared wrapper）→ structural/filter resolver → fit/bounds/collision/weight → consume-self commit 管线，禁止仅保留 `container_accepts_runtime_grant` 或“塞进第一个容器”旁路。至少覆盖 `add_item_to_player_inventory_inner`、`add_existing_item_to_player_inventory`、`pickup_dropped_loot_instance`、`force_attach_item_to_inventory` 以及其已核实生产 callers；filter/结构非法的候选应继续检查下一个候选。所有候选失败时 template grant 保留输入或走既有完整 `_or_ground` fallback，pickup 保留地面 `DroppedLootEntry`，绝不吞 item、地面物或 outcome。`force_attach_item_to_inventory` 不得再作为 infallible `(0,0)` fallback：仅其两个生产 caller——全量转移与骨币制作——改为 typed prepared placement；不得创建 16×16 假 `main_pack`，不得绕 bounds/collision/filter/owner。玩法 caller 不得复制 resolver。
9. `exchange_inventory_items` 是 social 普通交易唯一生产路径。prepare 前对 offered/requested 的每个 instance 调 `find_live_container_owner`：任一等于任一 live `ContainerState.owner_instance_id`，在 detach 前返回 typed `ContainerOwnerTradeForbidden`，双方 inventory/container/revision/drop/outcome 全部不变。其余交易必须以双方 inventory 为一个 staged 双库存事务：先完整结构/filter/fit 双向预检，才 commit；commit 对双方各至多 bump revision 一次，不能一方先写、另一方失败。
10. `transfer_all_inventory_contents` 是 tribulation/death 全量转移唯一生产路径，**允许**容器 owner。它必须以 source inventory、target inventory、`DroppedLootRegistry` 与 `DropContext` 构成单个 staged multi-resource transaction：合法目标 attach、owner/derived container rebuild、source/target orphan spill 和 target overflow 都先预演为 `PreparedInventoryDrop`。骨币必须全额可转；目标 JS-safe room 不足即整笔 typed reject，禁止 partial transfer。任一 prepare（含 registry/drop context）失败均 source/target/drop/outcome 零副作用；commit 后每个 changed inventory 恰好 +1 revision，每一 drop 恰一次。
11. `apply_bone_coin_craft_session` 唯一 storage fallback 冻结为 `BoneCoinCraftOutcome::{Stored, DroppedToGround}` 与 `DropFallbackUnavailable`（若仓库已有等价 typed 名称则复用该名）。材料消费、既有 Crafting reason 的 qi ledger transfer、产物 storage 或 ground drop、revision 与 `BoneCoinCrafted` event 必须同一 staged transaction。sealed qi 记入可追踪产物容器账户（如 `container:item:<output_id>`），seal surcharge 归当前 zone 或既有 tracked overflow；禁止直接 `qi_current -=`、多次 bump 或先消费后发现无处存放。ledger、drop context 或 registry 任一失败均全状态不变。
12. **死亡掉落唯一 staged transaction**：`apply_death_drop_on_revive` 的主世界路径与 `tsy_death_drop::apply_tsy_death_drop` 必须收敛为唯一 `prepare_death_drop(mode: DeathDropMode::{Overworld, Tsy}, ...) -> PreparedDeathDrop`（允许仅作不改变职责的 Rust 命名微调，不得另留路线）。`Overworld` 保持现有 deterministic 50% 选择，`Tsy` 保持入场物 50% + 秘境所得 100% 及既有武器保护；只共享 transaction，不得互相覆盖 selection rule。prepare 必须在 staged copy 中选择/detach、对 owner-loss 的 derived container rebuild/spill、remaining placement、所有 overflow/drop、`DroppedLootRegistry` inserts、`DroppedItemEvent`，以及 TSY corpse spawn/outcome 所需的一切不可失败数据，并预校验 `DropContext`、position、dimension、registry、corpse context。commit 才一次写 inventory/registry/events/corpse/outcome：实际 inventory 改变时 revision 恰 +1，零选中/零改变 revision=0 且无虚假 event；任一 prepare 失败则 live inventory/registry/event/corpse/outcome 全不变。两条 production caller 与「被选中 owner item」都进 grep/test gate。
13. **破损装备恢复唯一入口**：删除 `move_equipped_item_to_first_container_slot`，`combat/resolve.rs` 与 `forge/artifact_meridian.rs` 的两个 caller 都只能调用唯一 `prepare_broken_equipped_item_recovery`（或职责完全等价的一个固定名）。它在 staged copy 中将 durability 置零/终止态，resolver-aware 遍历目标 container（结构/filter/bounds/collision/weight），owner-loss 时 staged rebuild/spill；无合法收纳位置时预校验 ground-drop fallback，并把 `WeaponBroken` 或现有等价 durability outcome/event 纳入一次性 prepared outcome。Stored 或 Dropped 两路均一次 commit，整次 break revision 恰 +1、drop/event/outcome 各恰一次；任何 prepare/drop-context 失败均零副作用。不得先写 durability/revision 再回收。
14. **遗骸与法宝 deactivate 不能旁路**：`transfer_remains_to_looter` 必须以同一 `prepare_inventory_mutation_with` 或明确 typed prepared ingress 同时 stage 源 `RemainsContainer` record/骨币与 looter inventory；只有目标 owner/filter/fit 全部成功才移 source。非法候选继续找，最终无合法位置 typed reject，遗骸、玩家 inventory、revision/outcome 全不变；成功 item identity 守恒、player revision 一次、source 移除一次/outcome 一次。`apply_treasure_activate(..., activate=false)` / `handle_treasure_activate` 必须把 active/equipped source 与 player inventory 放入同一 staged transaction；目标非法继续候选，最终无 fit 时保持原 active/equipped treasure，不能先 deactivate/吞物；成功时结构/filter/fit 权威、必要 owner-loss rebuild、revision/outcome 各一次。死亡、broken recovery、remains、treasure deactivate 四组新入口纳入“全入口 grep 门”；其它 `DroppedLootRegistry.entries.insert` 的 NPC/world 自产路径不是玩家 container ingress，不扩大本 plan。
15. **普通 move 没有 commit 后 writer**：`InventoryMoveIntent` 的 `prepare_inventory_mutation_with` 必须在 prepare 内同算目标 `maybe_apply_targeted_item_wear` 等价规则、`SlotMove` attrition、owner/pack rebuild、freshness、layout/drop/outcome；wear/attrition 不适用即 staged no-op，目标/上下文缺失或计算失败即整笔 typed reject 零副作用。成功 commit 一次替换全部 staged state、revision 恰 +1，并 enqueue 一帧最终 snapshot，以及仅适用的 durability/attrition/move outcome 各恰一次。删除或收为 private 的旧 post-move live writer（包括 `maybe_apply_targeted_item_wear` → `apply_item_spiritual_wear`、handler 内直接 `apply_attrition_checked`、`rebuild_and_drop_overflow`）；生产 caller 不得旁路。
16. 所有 owner-loss 生产路径必须同一 staged transaction 完成「业务 detach / swap / remove → `rebuild_containers_from_equipment` → spill → overflow drop → outcome」：普通 move、swap 两个方向、discard、`enforce_intrinsic_gate_on_morph_release` 的 race/morph 强制驱逐、`handle_backpack_break`、死亡掉落、破损装备恢复与法宝 deactivate 全部纳入。当前 `handle_inventory_move` 仅在 `Moved` pack 分支 rebuild、`Swapped` 分支只 resync 是已知缺口，必须消除。成功只 commit 一次、revision 恰好 +1，禁止业务 mutation 与 rebuild 分别 bump；每个 overflow remainder 保留同一 instance/freshness、drop/outcome 恰一次；任一 prepare/transaction 失败 live/drop/outcome 零副作用。legacy self-contained 不走普通 spill，继续由 corruption/reconcile 处理。
17. P2 的 integrity/reconciliation 由其唯一 server 调度执行，P1 不得在 handler/caller 内直接 reconcile 或 emit snapshot。业务 prepared commit 若已含 integrity/freshness 修复仅 +1；独立首次 lock、reason change 或 repair 才独立 +1，same reason/no-op=0；即时 corrective request 同 tick 仍在 P2 的 Reconcile/Sweep 后从最终 inventory emit。

### 饱和测试

- migration：从真实 `inventories.schema_version=2` row 起测；worn、held、hotbar、`body_pocket` 四个合法携带面各一条成功持久化回填。严格 parser 接受 `pack_1` / `pack_<u64::MAX>`，拒绝 `pack_01`、空/非数字/溢出 suffix；零候选、多候选、非容器 template、普通/嵌套 grid、self-contained、已有 `Some` 均专属 pin。migration 不 bump revision、不 emit snapshot；save/transaction failure 保留 pending 并在重试成功后才升级 schema，升级后 reload 不再回填；随后首次 reconcile +1、same reason 重复=0。
- ingress、跨库存与全入口 grep 门：四个既有自动 ingress `add_item_to_player_inventory_inner`、`add_existing_item_to_player_inventory`、`pickup_dropped_loot_instance`、`force_attach_item_to_inventory` 及其所有 production caller 逐一覆盖合法收纳、filter/structural reject 后继续检查候选、full 或 `_or_ground` fallback、输入/地面物保留、零副作用 reject 与成功 revision +1；`force_attach_item_to_inventory` 仅全量转移/骨币制作两个 caller，均为 typed prepared placement，永无 `(0,0)` 或假 `main_pack` fallback。死亡门覆盖 `prepare_death_drop(Overworld)` deterministic 50%、`prepare_death_drop(Tsy)` 入场物 50%+秘境所得 100%、零选中无 revision/event、被选中 owner item 的 staged rebuild/spill、registry/drop-context/position/dimension/corpse-context 各失败零副作用，以及主世界/TSY caller grep gate。broken recovery 覆盖 combat 与 forge 两 caller、Stored/DroppedToGround、durability终止态/`WeaponBroken` exactly once、drop context失败全回滚，并静态断言 `move_equipped_item_to_first_container_slot` 无 production caller。remains 覆盖 source record/骨币与 looter 双资源成功、非法候选继续、最终无 fit typed reject 与 source/player/revision/outcome深比较不变；treasure deactivate 覆盖 active/equipped source、非法候选继续、无 fit 保留原物、owner-loss rebuild 与一次 revision/outcome。四组入口连同既有 ingress 的全入口 grep 门不得漏；NPC/world 自产 `DroppedLootRegistry.entries.insert` 明确不计入。
- `exchange_inventory_items` 覆盖 owner trade typed reject（双方完整深比较不变）及非 owner 双库存双向预检/各至多一次 revision；`transfer_all_inventory_contents` 覆盖 owner 可转、source/target/orphan/overflow prepared drops、JS-safe room 整笔 reject、registry/drop-context 任一失败零副作用、每个 changed inventory +1/drop exactly once；`apply_bone_coin_craft_session` 覆盖 Stored/DroppedToGround、DropFallbackUnavailable、ledger/registry/drop-context 全失败回滚、Crafting ledger 的产物账户/zone surcharge、一次 revision/event。
- owner-loss 与 move 叠加：普通 move、swap 两方向、discard、`enforce_intrinsic_gate_on_morph_release`、`handle_backpack_break`、death、broken recovery、treasure deactivate 分别断言同一个 staged rebuild/spill/remove transaction、成功仅 +1 revision、每个 overflow 的同 instance/freshness 在 inventory∪drops 守恒且 outcome/drop 恰一次，失败零副作用。普通 `InventoryMoveIntent` 专测 move+targeted wear、move+`SlotMove` attrition、move+pack rebuild 与三者叠加：总 revision 仅 +1、最终 snapshot 一帧、仅适用 durability/attrition/move outcome 各一次；wear/attrition no-op、缺失上下文/计算失败 typed reject 与全回滚；静态/生产路径门禁止 commit 后 `apply_item_spiritual_wear`、`apply_attrition_checked`、`rebuild_and_drop_overflow` writer。
- classification / resolver：四个静态 ID 分别 ownerless 全收；静态带 owner、未知 ownerless、伪 `pack_` ownerless、placeholder ownerless 各自 typed reject；合法 owner、eligible surface 缺失、template 缺失、无 `ContainerSpec`、ID-owner mismatch 各一条专属 case。
- unique live lookup / rebuild：worn、held、hotbar、`body_pocket` 四个合法 surface 各自命中；owner 从这四者分别移入 `main_pack`、任意 `pack_*` grid、自包含容器时，**同一** prepare/commit 原子 spill/remove，revision 恰好 +1；full overflow 的每个 stack/template/count/instance_id/freshness 在 inventory∪drops 守恒，drop 与 outcome 恰好一次，prepare spawn-context 失败则 live/drop/outcome 全零副作用。
- filter：None、empty、目标 category 命中/拒绝、目标 prefix 命中/拒绝、多 filter 首项命中/末项命中/全不命中；swap 必须逐向覆盖 moving→to 与 occupant→from 的单边 filter 拒绝及双边拒绝；源端移出任意 item 时不运行 accept filter。
- endpoints：corrupt target → container/hotbar/equip、locked 源→健康 container/hotbar/equip、一般 filter/bounds 拒绝分别覆盖；各自深比较 inventory、items、equip、hotbar、freshness、revision、drop/outcome 全部不变。
- `PreparedInventoryMutation<'a>`：compile-fail doctest 或等价 API test 必须证明 private non-Clone `#[must_use]` prepared 存活时不能二次借用 inventory 或改 revision/layout；顺序第二笔 mutation 只能在第一笔消费 commit 后从新 revision prepare。identity prepare 覆盖 new stack、partial merge、full merge，断言 hook 前不决定 merge、失败 prepare 零外部副作用、成功 commit 一次写布局并 revision +1；P1 不断言 Freeze 或其它 freshness 转换。
- self containment：普通 move 与 swap 两方向 owner→自身 `pack_<owner_id>` 均返回 `InventoryMoveRejectReason::ContainerOwnerSelfContainment`，inventory/revision/drop/outcome 均零变；legacy 已自包含单列为 owner corruption/reconcile 测试。
- 网络集成：真实 `ClientRequestV1::InventoryMoveIntent` → handler → reconcile/prepare → commit → typed `InventoryMoveRejectedV1` / corrective snapshot，覆盖 happy、reject、swap、corrupt target、locked source 与 self-containment；首次 integrity detection +1 后中止、已 locked same reason 重复请求 =0、一般 filter/bounds reject=0。

## P2 — owner-based freshness 与预提交状态迁移 ⬜

### 固定行为映射

| owner template | `ContainerFreshnessBehavior` | 语义 |
|----------------|------------------------------|------|
| `sealed_vial` | `Halve` | 非 Stepwise 的既有衰减速率减半 |
| `spirit_seal_box` | `Freeze` | 复用冻结时间记账；Stepwise 仍遵守既有 multiplier 语义 |
| `moisture_guard` | `SpoilOnly { rate: 0.3 }` | 只影响 Spoil track；Decay / Age 退 Normal |

### Freshness 4×4 转换表

| from \ to | Normal | Halve | SpoilOnly | Freeze |
|-----------|--------|-------|-----------|--------|
| Normal | `exit_container(now)` | `exit_container(now)` | `exit_container(now)` | `enter_container(now)` |
| Halve | `exit_container(now)` | `exit_container(now)` | `exit_container(now)` | `enter_container(now)` |
| SpoilOnly | `exit_container(now)` | `exit_container(now)` | `exit_container(now)` | `enter_container(now)` |
| Freeze | `exit_container(now)` | `exit_container(now)` | `exit_container(now)` | `enter_container(now)`，保留原 `frozen_since_tick`、不累计 |

任一 non-Freeze→non-Freeze 与 Freeze→non-Freeze 都调用 `exit_container(now)`（没有 active interval 时 no-op）；non-Freeze→Freeze 调用 `enter_container(now)`；Freeze→Freeze 也调用 `enter_container(now)`，但保留原 active tick、不累计。每个 incoming staged copy 均在 merge 前转换，完整 post-transition Freshness 才决定 stack identity。locked path 只由 queue→Reconcile set 处理：active Freeze interval 整段 discard、强制 Normal 并结构 reject，不再运行转换；repair 到 Freeze 从 repair tick 重新 `enter_container(now)`。

### 可核验交付物

1. 扩 `server/src/spiritwood/mod.rs::item_freshness_behavior` 的 owner-template 映射，保留既有 `ling_xia` 与 `food.container.ice_cellar` 行为不变；不复制 `container_storage_multiplier` 公式。
2. 在 `server/src/inventory/mod.rs` 固定 `resolve_container_freshness_behavior`，复用 P1 `classify_container_ownership` / `find_live_container_owner`；静态容器默认 `Normal`，损坏 owner 返回同一 typed error。
3. P2 将 `prepare_inventory_mutation_with` 的生产 `InventoryItemPrepareFn` 强制替换为 `apply_container_freshness_transition`：closure 捕获权威 `now_tick` 与 from/to behavior，严格按 P2 4×4 表在 merge 选择前对每个 incoming logical item staged copy 转换；完整 post-transition Freshness identity 才决定 merge/push/placement。该 hook 覆盖 P1 全部 owner-loss staged 路径：普通 move、swap 两方向、discard、race/morph 强制驱逐与 container break；任何 item 从 Freeze owner 离开时，对每个 incoming/spilled/dropped staged item 都先 `exit_container(now_tick)`，再 merge/drop，实现 Freeze→Normal。move、swap、runtime grant 成功时只由同一 `commit_prepared_inventory_mutation(self)` 写入 staged 字段、merge/push/equip/hotbar 并 revision +1；不得提交后补写。locked entity 由 queue→`Reconcile.after(Commit)` 处理：若该业务 prepared 已含 repair，则不作独立 reconcile bump；否则独立 lock/repair commit 后通过唯一 emitter 返回 typed reject/corrective state。无变化仍 locked 一律结构 typed reject、整段 discard active Freeze interval，不运行普通 exit。
4. runtime grant 的 staged 顺序固定：先以目标 behavior 调用 `apply_container_freshness_transition`，再按完整 post-transition Freshness identity 决定 merge/push/placement，最后才做 bounds/collision/weight 校验与 commit。Freeze 仅可与 post-transition identity 完全相同的 stack 合并；非 freshness item 安全 no-op。此规则覆盖新 stack placement、部分 merge + new stack、全量 merge 既有 stack（`new_stacks` 为空但 stack_count 改变并 revision +1），不可绑定具体 `items.push`。
5. P2 在 `ContainerState` 与 `PlayerInventory` server persistence serde/default 新增 `integrity_lock: Option<ContainerIntegrityLock>`，`ContainerIntegrityLock` 字段**恰为** `reason: ContainerAcceptanceLockReason` 与 `detected_tick: u64`，legacy/default=None；P1 固定内部 `ContainerAcceptanceLockReason::{OwnerMissing, OwnerNotFound, OwnerInvalid}`。必须覆盖三 reason + tick 的 save/load round-trip 与 legacy None。P2 禁止新增 snapshot/protobuf/TypeBox/client 的 `acceptance_lock`，但持久化 `integrity_lock.reason` 与 `detected_tick` 是 P2 server 边界；P4 只映射 reason、不下发 tick。
6. 新建 `server/src/inventory/reconciliation.rs`，冻结唯一 server 内调度类型：`InventoryReconciliationSet::{Commit, SnapshotRequestProducer, Reconcile, Sweep, CollectSnapshots, EmitSnapshots}`、供 Reconcile 专用的 `ContainerIntegrityFreshnessQueue(BTreeSet<Entity>)`、server-only stable-`Ord` `InventorySnapshotReason`、`InventorySnapshotRequest { entity, reason: InventorySnapshotReason }` 与 `InventorySnapshotOutbox(BTreeMap<Entity, BTreeSet<InventorySnapshotReason>>)`；以及唯一 mutable reconcile 函数 `reconcile_container_integrity_freshness`。系统序严格固定为 `Commit -> SnapshotRequestProducer -> Reconcile -> Sweep -> CollectSnapshots -> EmitSnapshots`：所有 inventory business/reconciliation writer（含大 `handle_client_request_payloads`）在 Commit，写后只 `EventWriter<InventorySnapshotRequest>` enqueue；join inventory/player-state attach 在 SnapshotRequestProducer 前；只读 resync producer 在 `SnapshotRequestProducer.before(Reconcile).before(CollectSnapshots)`；mutable reconcile 在 Reconcile，`sweep_shelflife_variants` 在 Sweep.after(Reconcile)，`collect_inventory_snapshot_requests` 在 CollectSnapshots.after(Sweep)，唯一 `emit_queued_inventory_snapshots` 在 EmitSnapshots.after(CollectSnapshots)。`build_inventory_snapshot` 保持纯读。
7. producer 只能持有 `EventWriter<InventorySnapshotRequest>`，不得直接 serialize/send；`collect_inventory_snapshot_requests` 是全 app 唯一 `EventReader<InventorySnapshotRequest>`，在 Sweep 后同时观察 `Added/Changed<PlayerInventory>` 与 revive/join 所需状态（若 revive 已由 producer event 进入，必须明确该 event 是唯一来源，禁止双计），按 entity 合并进 outbox。reason 只作 server diagnostics：按 enum 稳定排序后以 `+` join 记录，绝不进入 P4 wire。所有当前 `send_inventory_snapshot_to_client` / inline `resync_snapshot` direct sender——Added/Changed/revive、botany、alchemy/coffin/workbench/forge/lingtian/mineral/cast/social/block、NPC/scroll helpers、supply coffin/world container/read-only resync——都必须改为 EventWriter；botany 删除自建 reader/serialize。Commit 内的 corrective request 必在 Collect 前，最后一个 read-only producer由 explicit `SnapshotRequestProducer.before(Reconcile/CollectSnapshots)` 保证同 tick。
8. `emit_queued_inventory_snapshots` 是 outbox 唯一 drain/send owner：每 tick `mem::take` 整体 drain，一 entity 只 build/send 一帧最终 snapshot。缺 `Client` / `Username` / `PlayerState` / `Cultivation`、序列化失败或 client send 失败均丢弃本 tick entry并记录 diagnostics，不跨 tick保留陈旧快照、不 retry 旧 revision。Reconcile/Sweep 允许按已定义顺序 enqueue；其 mutation 由 Sweep 后 `Changed<PlayerInventory>` 观察与其 request 合并，避免一 tick迟滞。
9. `reconcile_container_integrity_freshness` 的状态机及 revision 边界固定：同一业务 prepared commit 已包含 integrity/freshness 修复时仅该 commit +1，reconcile 不得再 bump；独立首次 lock、reason change 或 repair 才作为 reconciliation commit 恰好 +1，same reason/no-op=0。首次 Healthy/None→Locked(reason) 写入 `{ reason, detected_tick: now_tick }`，清每个 active `frozen_since_tick` 而不增 `frozen_accumulated`；reason change 同样替换 reason/tick 并 discard active interval；Locked→Resolved repair 清 lock，Freeze item 从 repair tick `enter_container`，否则保持 None。独立 mutation 后会由 queue 输出 corrective snapshot；任何变更不得和一次业务 commit 重复 bump。
10. commit→即时 corrective request 同 tick必须由最终 queued emitter 输出 reconciled revision/content；sweep 仅实际 variant 切换时额外 +1。lock 存在时 behavior 强制 `Normal`。P2 只实现 server 内 lock/reconcile/queue/ordering，绝不提前加 proto/schema/client 的 `acceptance_lock`。

### 饱和测试

- 三个固定映射逐一 pin，未知 owner 为 typed reject、静态容器为 `Normal`；既有 `ling_xia` / ice cellar 回归不变。
- 4×4 转换表全部 16 格逐一 pin：freshness/non-freshness、active/no-active interval、`exit_container` no-op、Freeze→Freeze 保留 `frozen_since_tick` 不累计；每格覆盖 new、partial merge、full merge，必须由 post-transition identity 决定 merge，任何失败零副作用。
- `apply_container_freshness_transition` 作为 `prepare_inventory_mutation_with` 的强制 prepare fn，覆盖 move/swap/grant 及普通 move、swap 两方向、discard、morph 强制驱逐、container break 的全部 owner-loss 成功形态；所有 Freeze→Normal 的 incoming/spilled/dropped staged item 在 merge/drop 前各调用一次 `exit_container(now_tick)`，active/no-active 均测，不得重复累计。hook 在 merge 前更改 staged copy，成功 `commit_prepared_inventory_mutation(self)` 一次写 staged freshness/layout 并 revision +1，inventory∪drops 守恒，绝不提交后补写。
- runtime grant 覆盖 new stack placement、partial merge + new stack、full merge existing stack（`new_stacks` 为空但 stack_count 变化）三种成功形态；Freeze/非 Freeze、非 freshness no-op、post-transition Freshness identity 不同不得 merge、filter/full skip、owner corruption `Err` 与最终失败零副作用各自 pin。
- integrity persistence：`PlayerInventory` server persistence serde/default 对三 reason + `detected_tick` save/load round-trip，legacy load 默认 lock=None；合法 Freeze 在 tick100、tick200 首次 lock、tick250 同 reason 重复、tick300 repair 的序列断言 active 100 起 interval 整段 discard（accumulated 不增）、重复 no-op、repair 从300重新 enter。reason change、无 Freshness、batch 多 item、snapshot/sweep 对拍均专属 pin。
- reconciliation 调度与 outbox：静态/系统注册门断言唯一 `EventReader<InventorySnapshotRequest>` 是 `collect_inventory_snapshot_requests`、唯一 outbox drain/send owner 是 `emit_queued_inventory_snapshots`；全部 writer 在 Commit，read-only producer 在 `SnapshotRequestProducer`，顺序严格 `Commit -> SnapshotRequestProducer -> Reconcile -> Sweep -> CollectSnapshots -> EmitSnapshots`，join/player-state attach 在 producer 前。Added/Changed/join/revive、botany 与每一类 direct resync 只能 EventWriter，botany 无自建 reader/serialize。单 entity 多 producer/multi-reason 按 stable `Ord` 合并成一帧，两个 entity 隔离，最后一个 read-only producer、Commit corrective、Reconcile 与 Sweep Changed 都在同 tick输出最终 revision/content；空 outbox 不 send。缺依赖、serialize 或 send 失败后 drain 无残留；下一 tick 新 revision 请求不被旧 request 覆盖；reason 不进 wire。
- revision/中止语义：同一业务 prepared commit 含 integrity/freshness repair 时总计仅 +1；独立首次 reconcile、reason change、repair 各恰好 +1，same reason=0；sweep 仅真实 variant 切换额外 +1。commit 后的即时 corrective request 同 tick输出最终 reconciled revision/content；一般 filter/bounds reject=0。grant 的 `InventoryGrantError::IntegrityReconciled` 保留输入 item，调用方显式重排或 `_or_ground` fallback，禁止同调用 retry/吞物。
- sweep/snapshot：唯一 queued emitter 只在 reconciliation/sweep 后运行；persisted lock 强制 Normal，任何 active Freeze interval 已被 discard，corrective snapshot 保留真实 layout/items/revision。

## P3 — 暗器迁移与恰好九个随身容器 ⬜

### 九容器完整 `ContainerSpec` 固定矩阵

> 以下字段是 P3 唯一 TOML 值，全部采用现有 `worn_grass_pouch` / `grass_pouch` 的 canonical profile（`8.0` / `0.008` 或 `10.0` / `0.005`），不留“实现时选择”空间；`equip_slot` 均为 parser 当前合法身体槽，`attrition_exempt=false`、`quick_access=false` 均须显式写入。

| # | template id | rows×cols | weight_capacity | equip_slot | durability_cost_per_op | attrition_exempt | quick_access | `accept`（OR） | freshness |
|---|-------------|-----------|-----------------|------------|------------------------|------------------|--------------|----------------|-----------|
| 1 | `herb_pouch` | 3×3 | 8.0 | `chest` | 0.008 | false | false | `herb`, `food` | Normal |
| 2 | `ore_sack` | 3×3 | 10.0 | `chest` | 0.005 | false | false | `mineral` | Normal |
| 3 | `projectile_bag` | 3×4 | 10.0 | `legs` | 0.005 | false | false | `anqi` | Normal |
| 4 | `herb_crate` | 4×4 | 10.0 | `chest` | 0.005 | false | false | `herb` | Normal |
| 5 | `sealed_vial` | 2×2 | 8.0 | `chest` | 0.008 | false | false | `pill`, `food` | Halve |
| 6 | `spirit_seal_box` | 2×2 | 10.0 | `chest` | 0.005 | false | false | `treasure`, `pill` | Freeze |
| 7 | `moisture_guard` | 3×3 | 8.0 | `legs` | 0.008 | false | false | empty（全收） | `SpoilOnly { rate: 0.3 }` |
| 8 | `coin_box` | 3×3 | 8.0 | `chest` | 0.008 | false | false | `bonecoin` | Normal |
| 9 | `sealed_envelope` | 1×2 | 8.0 | `head` | 0.008 | false | false | `recipe_fragment`, `recipe_hint`, `scroll` | Normal |

### 可核验交付物

1. 在现有 item TOML 中将上述恰好九个 template 迁为 `category = "container"`，并逐项按 P3 完整矩阵写齐 `[item.container]` 的 rows、cols、weight_capacity、equip_slot、durability_cost_per_op、attrition_exempt、quick_access、accept；不得省略默认字段或改为待调参数。它们接入 PR-1 已落地的 owner-instance / live rebuild 语义，不新增第二种容器状态。
2. `server/assets/items/anqi.toml` 中十二个暗器实物从 `misc` 迁到 `anqi`：`anqi_bone_chip`、`anqi_yibian_shougu`、`anqi_lingmu_arrow`、`anqi_dyed_bone`、`anqi_fenglinghe_bone`、`anqi_shanggu_bone` 及各自 `_charged` 变体。既有 `max_stack_count` 保持权威。
3. `projectile_bag` 的 category filter 必须对十二个迁移项全收、对非暗器拒绝；不得用 template-prefix 偷代 category 迁移。
4. `coin_box` 将现有“丹砂隔灵，骨币少漏”文案迁为 `骨币分类收纳匣，便于归类携带。`，只表达骨币分类/收纳便利；不得暗示隔灵、少漏、保鲜、保值或延缓半衰。`herb_crate_placed` 保持独立 placeable twin，P3 不修改它，也不实现 portable↔placed 转换。

### 饱和测试

- 九个 template registry-load 全绿；逐个断言 category、完整 `ContainerSpec` 八字段、filter、freshness 映射与矩阵一致。
- 每个 non-empty 容器各有一条 filter-invalid item 拒绝；`moisture_guard` 因 empty 全收不得有 filter-invalid item，负例只覆盖 bounds、collision、full 或 owner corruption，并另 pin empty 全收。
- 十二个暗器逐项断言 `ItemCategory::Anqi`，并逐项通过 `projectile_bag`；普通矿石、草药、骨币分别拒绝。
- save/load：九类容器按 PR-1 已落地的 eligible surface 语义恢复 owner、容量、内容与 filter 解析；普通/嵌套 grid 不产生 live container 的行为由 P1 回归锁住，P3 不重复实现 rebuild/spill。
- `coin_box` 文案测试 pin 仅骨币分类/收纳便利，明确不含“隔灵”“少漏”“保鲜”“保值”“半衰”等承诺。

## P4 — snapshot filter、客户端预提示与端到端验收 ⬜

### Wire 契约

1. 新增 `ContainerAcceptFilterV1`，JSON 采用稳定 tagged 形状：category = `{ "kind": "category", "value": "mineral" }`，prefix = `{ "kind": "template_prefix", "value": "anqi_" }`；protobuf 用等价 `oneof`。`accept_filter` 是 required array/repeated：仅健康且 `acceptance_lock` **缺省**时 `[]` 表示全收；lock **present** 时数组必须恰为 `[]` 且完全 inert/ignored，绝不将 locked 下的 `[]` 解释为接受授权。
2. `ContainerSnapshotV1.acceptance_lock` 固定为 Rust `Option<ContainerAcceptanceLockV1>`；protobuf 是有 presence 的 singular `ContainerAcceptanceLock acceptance_lock` message，message 内为 `string reason`；JSON / TypeBox / Java 统一为可选对象 `{ "reason": "owner_missing" | "owner_not_found" | "owner_invalid" }`。健康快照字段**缺省**，禁止 null、空对象和顶层 sentinel enum。wire lock 仅由持久化 `ContainerState.integrity_lock.reason` 映射，P4 不下发 `detected_tick`，不得从 snapshot projection 猜测。
3. `inventory_snapshot_emit::build_inventory_snapshot` 必须保持纯读；P4 不调用 reconciliation。P2 的唯一 emitter 已保证其读取 Reconcile/Sweep 后最终 `ContainerState.owner_instance_id`、真实 revision/layout/items。`integrity_lock` present 时 projection **直接** emit exact `accept_filter=[]` + lock reason，绝不从坏 owner 推 filter；仅 lock absent 才经同一 `find_live_container_owner` + resolver 构建 filter。`pack_<instance_id>` 只作为 container ID，不再作为 owner 推导源。三种 owner 损坏路由必须形成可序列化 corrective snapshot；lock 存在时 projection 只消费已持久化的强制 `Normal` behavior，首次腐损 request 的 corrective snapshot 带新 revision+reason。
4. `InventoryItemViewV1.category` 是 server registry 派生的 required lower-snake canonical 字符串，恰为 `pill, herb, recipe_fragment, recipe_hint, weapon, armor, treasure, bone_coin, tool, scroll, misc, block, mineral, anqi, liquid, container, food, shield`；client 禁止维护 template→category 映射。TOML accept 的 `bonecoin` 与 wire `bone_coin` 是两个不同且已冻结的语法。
5. snapshot parser 必须 fail-closed：`accept_filter` 缺失/null、lock present 却 non-empty filter、present 的非法/unknown lock reason、unknown filter kind/value、缺/null/unknown category 或额外键时，整份 snapshot no-op，保留旧 revision/state。客户端以 presence / `hasAcceptanceLock()` 判断 lock；present lock 必显示 `INVALID` 并令 filter inert，绝不接受以 null、空对象、`NONE` 或 empty filter 掩盖 corrupt owner。
6. 同步修改：
   - Rust serde structs、`server/src/schema/proto_convert.rs` 与正反测试；
   - `proto/bong/envelope.proto::{ContainerSnapshot, InventoryItemView}`；
   - `agent/packages/schema/src/inventory.ts`、samples，并先构建 `@bong/schema` dist 再跑 schema/agent 测试；
   - client `InventorySnapshotHandler`、`InventoryModel.ContainerDef`、`InventoryItem`、`InventoryStateStore`。

### Client 交互

1. `WornContainerPanel`、`PackContainerWindow` 与 `InspectScreen` 三个 surface 都从 `InventoryStateStore` 当前 `ContainerDef.acceptFilter` / optional `acceptanceLock` 读取；拖拽高亮统一调用固定 client 纯函数 `ContainerFilterRules.accepts`。作为源端时三 surface 不以 filter 阻断移出，但 present lock source 仍显示锁态并在松手时发出原有 Intent。
2. `acceptanceLock` presence / `hasAcceptanceLock()` 为 true 时任意 drag **进入**该目标一律使用既有 `GridSlotComponent.HighlightState.INVALID`（`0x33CC2222`，即时显示、无 fade），filter 不再参与目标接受语义；无论 locked source 或 locked target，松手仍发送原有 `InventoryMoveIntent`，server 在源/目标结构门返回同类 typed owner reject。lock 字段缺省时，dragged item 不通过目标 filter 同样显示 `INVALID`；通过 filter 后仍需叠加 bounds / collision 判定，只有全部通过才显示 `VALID`。
3. 预提示绝不代替 server 权威：即使 client 预测为非法，松手仍发送原有 `InventoryMoveIntent`；最终接受 / 拒绝、revision 与 corrective snapshot 全以 server 为准。parser 对非法 lock reason、filter 或 category，以及 required array 缺失/null、lock present+non-empty array 的整份 snapshot fail-closed no-op，保留旧 revision/state；corrective snapshot 的 lock 必传且优先，带 exact `[]`，不得丢弃或把空 filter 解释为 corrupt owner 的接受授权。
4. 本阶段不新增粒子、动画、音频或 narration 资产；可感知反馈仅复用现有 grid `INVALID` / `VALID` tint，既有背包操作 A/V 不变。

### 饱和测试与验收

- Rust / TypeBox / protobuf：两种 filter 变体正反、unknown kind/value、缺 value、empty、OR 列表、owner 派生、静态全收、`accept_filter` required array/repeated、serde/proto round-trip 与 sample 对拍；lock absent+`[]` 仅健康全收，lock present+exact `[]` 仅表示 inert array，locked+non-empty、array 缺失/null、非法/unknown lock/filter/category、extra key 都使整份 snapshot fail-closed。`ContainerAcceptanceLockV1` 只接受 present `{reason}` 的三种 reason，健康字段缺省，null/空对象/额外键/未知 reason 一律 fail-closed。三种损坏 owner 路由的 corrective snapshot 可序列化，带真实 layout/items、新 revision 与 persisted lock reason；snapshot/sweep 只读 reconciliation 后强制 `Normal` 状态。
- category wire：18 个 canonical 值 `pill, herb, recipe_fragment, recipe_hint, weapon, armor, treasure, bone_coin, tool, scroll, misc, block, mineral, anqi, liquid, container, food, shield` 逐一覆盖 serde/proto/TypeBox/JsonFormat/sample/client parser 的正反测试；缺/null/unknown category 或 extra key 均整份 snapshot no-op 并保留旧 revision/state，server registry 是唯一派生源，TOML `bonecoin` 不可替换 wire `bone_coin`。
- client parser：字段完整、空 filter、两 filter 变体、unknown kind/value、非法/未知 category、present 三 reason、lock 缺省、null/空对象/额外键/unknown reason 全部覆盖；任何非法 snapshot fail-closed 保留旧 revision/state。snapshot 替换后 UI 只读取新 filter/lock；修复 snapshot 清除字段、读取新 filter 和新的 Freeze tick。
- client UI：`WornContainerPanel` 与 `PackContainerWindow` 各覆盖合法 category、非法 category、合法 prefix、非法 prefix、empty、filter 合法但 footprint 冲突六路；非法均断言 `INVALID` tint。`InspectScreen` 专属接线测试覆盖合法 category→`VALID`、合法 prefix→`VALID`、category/prefix reject、unlocked empty、footprint conflict、locked corruption、revision 替换后读取新 filter/lock；三 surface 均以 presence / `hasAcceptanceLock()` 使用同一规则并仍发 Intent。
- locked source：Worn/Pack/Inspect 均覆盖 locked source→健康 container/hotbar/equip，UI 仍发送 Intent，server 返回同类 typed owner reject，inventory/freshness 深比较与 revision 均不变。
- 权威链与 e2e：client 预判非法、locked target 或 locked source 后仍发 `InventoryMoveIntent`；首次腐损请求只做 reconcile +1 并中止，corrective snapshot 带新 revision+reason；同 reason 已 locked 重试 revision=0；一般 filter/bounds reject=0。九容器 e2e 逐一打开对应 owner 容器，八个非 empty 容器合法物通过、filter-invalid 物拒绝；`moisture_guard` empty 全收，负例仅 bounds/collision/full/owner corruption；另覆盖两个同模板 owner 同时存在时 UI/server 不串 filter。
- bot 场景：新增容器 filter move 场景，至少覆盖 `ore_sack` 矿石通过、草药拒绝、swap 反向拒绝、`moisture_guard` 非 filter 负例、locked source typed reject、首次/重复 integrity revision 差异，拒绝后 layout 不变。

## §8 开放问题（历史表，全部已收口）

| # | 历史问题 | 状态 |
|---|----------|------|
| 1 | 容器归属继续走旧嵌套/session，还是改绑当前平展模型？ | 已在 §8.1 #1 收口 |
| 2 | owner/filter 是否允许多套 resolver、缺 owner 是否 fail-open？ | 已在 §8.1 #2 收口 |
| 3 | `moisture_guard` 精确行为与 rate？ | 已在 §8.1 #3 收口 |
| 4 | 本 plan 到底交付哪些随身容器，容量/filter 如何固定？ | 已在 §8.1 #4 收口 |
| 5 | client 从哪里取得 filter 与 dragged item category，预判是否可替代 server？ | 已在 §8.1 #5 收口 |
| 6 | 剩余实施如何拆 PR？ | 已在 §8.1 #6 收口 |

> 原表仅保留作历史追溯；不存在待拍板项，实施一律以 §8.1 决议为准。

## §8.1 决议（pre-P1 收口，2026-07-24）

### #1 owner-instance 平展架构是唯一执行路线

**决议**：`PlayerInventory.containers` + `ContainerState.owner_instance_id` 为唯一状态与归属模型；稳定 `pack_<instance_id>` 只作 ID，不恢复历史方案，也没有前置 plan blocker。

**落点**：`server/src/inventory/mod.rs:469-482`（`ContainerState`）/ `server/src/inventory/mod.rs:767-772`（`PlayerInventory`）+ 本 plan「当前架构基线与范围边界」「历史否决」。

### #2 一份 resolver、typed corruption、filter OR

**决议**：`classify_container_ownership` 冻结四静态 ID allowlist（`body_pocket` / `main_pack` / `small_pouch` / `front_satchel`）及四行分类真值表；`find_live_container_owner` 是唯一 owner lookup，只查 worn、held、hotbar、`body_pocket`，排除普通静态 grid、任意 `pack_*` grid 与 owner 自身容器。P1 同 PR 以它替换 `rebuild_containers_from_equipment` live 判据，并在 owner 离开 eligible surface 时同一 prepare/commit 原子 spill/remove。runtime grant、move、swap 共用其上的 `resolve_container_acceptance`。静态带 owner=invalid，非 allowlist ownerless（未知/伪pack/placeholder）=missing，非 allowlist ownerful 只按字段查 eligible owner/template/`ContainerSpec` 并校验 `pack_<owner_id>`；None/empty 全收，非空列表 OR；任何损坏 typed reject。每个 Container from/to 端点先过结构门，源端不套 filter、目标端才套；swap 在写状态前双向校验，拒绝保持 revision/layout 原样。owner 移入自己的 `pack_<owner_id>`（move/swap 任一方向）固定 `InventoryMoveRejectReason::ContainerOwnerSelfContainment`，它不进入 integrity lock reason；legacy 已自包含继续走 owner corruption/reconcile。

`PreparedInventoryMutation<'a>` 固定为 private、non-Clone、`#[must_use]`，持有 prepare→`commit_prepared_inventory_mutation(self)` 的独占 `&'a mut PlayerInventory`，不得进入 resource/event、跨 handler/tick 或与第二笔 prepared 并存；`original_revision` 仅 invariant/debug assert，不采用 CAS。它带 non-Clone `Vec<PreparedInventoryDrop>` 与一次性 outcome；rebuild/spill 在 staged inventory 完成，prepare 预校验 spawn context，commit 不可失败地一次替换库存、一次 enqueue drops、一次发布 outcome、最后 revision +1，绝不 retry/repeat spawn。P1 同时以此管线取代 load-time prefix backfill：从当前 inventory schema v2 版本化迁移，只在 SQLite 保存成功后升版；四个自动 ingress、普通 move/swap/discard/morph/break owner-loss、social `exchange_inventory_items` 与 tribulation/death `transfer_all_inventory_contents` 都不得另走 first-fit 或双 bump 旁路。两条死亡掉落 production caller 固定共用 `prepare_death_drop(DeathDropMode::{Overworld,Tsy}, ...) -> PreparedDeathDrop`：保持 Overworld deterministic 50% 与 Tsy 入场物50%+秘境所得100%的各自选择规则，同时 stage selected detach、owner rebuild/spill、drop registry/event 与 TSY corpse/outcome，prepare 失败绝不改 live inventory/registry/event/corpse/outcome，零选择不 bump/event。删除 `move_equipped_item_to_first_container_slot`，combat/forge 仅走 `prepare_broken_equipped_item_recovery`，在同一 staged copy 完成 durability终止、resolver-aware placement或预校验 ground drop、rebuild/spill 与 `WeaponBroken` outcome，任何失败零副作用。`transfer_remains_to_looter` 同时 stage 源遗骸与 looter，`apply_treasure_activate(..., false)` 同时 stage triggered/equipped source 与玩家库存；均要求非法候选继续、最终无 fit 保留 source、成功 only-once revision/outcome。四类入口与既有 ingress 全部进 grep 门，NPC/world 自产 registry insert 不在范围。普通 `InventoryMoveIntent` 在 prepare 内同算 wear、`SlotMove` attrition、owner/pack rebuild、freshness/layout/drop/outcome，绝无 commit 后 live writer；适用 outcome 和最终 snapshot 均 once，整笔 revision 仅 +1。交易遇任一 offered/requested instance 等于任一 live owner 必 typed `ContainerOwnerTradeForbidden` 并零副作用；非 owner 交易只能 staged 双库存双向预检、双方各至多一次 revision。全量转移允许 owner，但 source+target+`DroppedLootRegistry`/`DropContext` 必为单一 staged multi-resource transaction：所有 attach/rebuild/orphan/overflow 均预演 drop，骨币 JS-safe room 不足整笔 typed reject、绝无 partial transfer。`force_attach_item_to_inventory` 仅全量转移/骨币制作两 caller 可用，且二者均 typed prepared placement，禁 `(0,0)`/16×16 假包/越 filter-owner；骨币制作材料、Crafting ledger（sealed qi 到 `container:item:<output_id>`，surcharge 到当前 zone/既有 tracked overflow）、Stored/DroppedToGround 与 `DropFallbackUnavailable`、revision 及 `BoneCoinCrafted` event 同一 staged transaction，任一 ledger/registry/drop-context 失败全回滚。

**落点**：`server/src/inventory/mod.rs:83-107,6549-6592`（静态 ID 锚）/ `server/src/inventory/mod.rs:2117-2130`（现有局部 grant helper）/ `server/src/inventory/mod.rs:3828-3938`（原子 move/swap）/ `server/src/inventory/mod.rs:5572-5647`（attach 校验）+ 本 plan P1。

### #3 freshness 映射与提交边界固定

**决议**：`sealed_vial=Halve`、`spirit_seal_box=Freeze`、`moisture_guard=SpoilOnly { rate: 0.3 }`；只复用既有公式。P1 对 `prepare_inventory_mutation_with` 传 identity `InventoryItemPrepareFn`；P2 将该 prepare fn 强制替换为 `apply_container_freshness_transition(now_tick, from/to behavior)`，按 4×4 表在 merge 前操作每个 incoming staged copy，最终仅由 `commit_prepared_inventory_mutation(self)` 一次写入 staged freshness、布局与 revision。non-Freeze→non-Freeze 调 `exit_container`，non-Freeze→Freeze 调 `enter_container`，Freeze→non-Freeze 调 `exit_container`，Freeze→Freeze 保留 active tick 而不累计。此 hook 同时覆盖普通 move、swap 两方向、discard、race/morph 强制驱逐和 container break 的 staged spill/drop；每件离开 Freeze owner 的 incoming/spilled/dropped item 在 merge/drop 前只 exit 一次，locked corruption 仍走 reconcile 的整段 discard。

P2 通过 `PlayerInventory` server persistence serde/default 保存 `integrity_lock(reason, detected_tick)`；新建 `server/src/inventory/reconciliation.rs` 作为唯一 server 内调度，固定 `InventoryReconciliationSet::{Commit, SnapshotRequestProducer, Reconcile, Sweep, CollectSnapshots, EmitSnapshots}`、供 Reconcile 专用的 `ContainerIntegrityFreshnessQueue(BTreeSet<Entity>)`、stable-`Ord` server-only `InventorySnapshotReason`、`InventorySnapshotRequest { entity, reason }`、`InventorySnapshotOutbox(BTreeMap<Entity, BTreeSet<InventorySnapshotReason>>)`与 `reconcile_container_integrity_freshness`。严格序为 Commit → SnapshotRequestProducer → Reconcile → Sweep → Collect → Emit：所有 writer（含大 handler）在 Commit 后 EventWriter enqueue，join/player-state attach 在 producer 前，只读 resync producer显式在 SnapshotRequestProducer 且 Collect前，Reconcile/Sweep 可按序 enqueue；唯一 EventReader collect 在 Sweep 后合并 request 与 Added/Changed/revive/join，唯一 emitter `mem::take` drain outbox 后每 entity 仅发最终一帧。reason 仅 diagnostics 稳定排序 join、绝不进 wire；缺 Client/Username/PlayerState/Cultivation、serialize/send失败均记录并丢弃本 tick entry、不 retry陈旧 revision。所有旧 snapshot sender 只 enqueue，botany 删除自建 reader/serialize；business/reconcile/sweep 的同 tick最终状态均合一帧。若业务 prepared 已含 repair 仅 +1；独立首次 lock/reason change/repair 才 +1，same reason=0，sweep 只有实际 variant 切换才额外 +1。即时 corrective request 同 tick仍在 Reconcile/Sweep 后输出最终内容。P2 不新增 snapshot/protobuf/TypeBox/client lock；P4 只将 persisted reason 映射为 presence optional lock object，不下发 tick。

**落点**：`server/src/shelflife/container.rs:29-98`（multiplier / enter / exit）/ `server/src/spiritwood/mod.rs:611-647`（既有 behavior resolver）/ `server/src/network/client_request_handler.rs:14984-15278`（move 成功/拒绝分支）+ 本 plan P2。

### #4 恰好九个随身容器，矩阵不再可选

**决议**：交付集合与完整 `ContainerSpec`（rows、cols、weight_capacity、equip_slot、durability_cost_per_op、attrition_exempt、quick_access、accept）以 P3 九行矩阵为唯一答案；`moisture_guard` 取 empty 全收；暗器十二项全部迁 `Anqi`。owner 容器只在 worn/held/hotbar/body_pocket 位置 live，移入普通或嵌套 grid 的 rebuild 原子 spill/remove 已由 P1 交付，P3 只验证其与新数据的 save/load 接线。`coin_box` 只承诺骨币分类/收纳便利，不承诺隔灵、少漏、保鲜、保值或半衰变化；`herb_crate_placed` 为 scope-out 的独立 placeable twin。本 plan 不再使用“12 容器”口径。

**落点**：`server/assets/items/workbench_materials.toml` / `server/assets/items/anqi.toml` + 本 plan P3 与「范围边界」。

### #5 snapshot 派生 filter，client 只预提示

**决议**：P4 独占 `acceptance_lock` 的跨端契约：filter 与 optional presence `acceptance_lock` 同挂 `ContainerSnapshotV1`，item required lower-snake category 挂 `InventoryItemViewV1`，Rust / TypeBox / protobuf / Java `JsonFormat` / client 同步。`accept_filter` 始终为 required array/repeated：健康且 lock 缺省时 `[]` 才表示全收；lock present 时数组必须 exact `[]` 且 inert/ignored，presence 优先，客户端 `INVALID`、server structural reject，绝不将该空数组解释为授权。P4 将 P2 已持久化的 `integrity_lock.reason` 映射为缺省或 `{reason: owner_missing|owner_not_found|owner_invalid}`；禁止 null、空对象、sentinel enum 与下发 tick。snapshot 构造遇 lock present 直接 emit exact `[]` + reason，不得从坏 owner 推 filter；lock absent 才 resolve filter。

Worn/Pack/Inspect 以 presence / `hasAcceptanceLock()` 与 `ContainerFilterRules.accepts` 做预提示并使用 `INVALID`，locked source 或 target 均仍发 `InventoryMoveIntent`，server 先查 from/to Container 结构并 typed reject；Inspect 同时 pin 合法 category/prefix 的 `VALID` 正路径。client 永远不能吞掉请求，server 永远权威。非法/未知 reason、filter kind/value、category、required array 缺失/null、locked+non-empty array 或 extra key 令整份 snapshot fail-closed no-op、保留旧 revision/state；server registry 是 category 唯一派生源，TOML `bonecoin` 与 wire `bone_coin` 分离冻结。

**落点**：`server/src/schema/inventory.rs:241-287` / `proto/bong/envelope.proto:688-695` / `agent/packages/schema/src/inventory.ts:322-363` / `client/src/main/java/com/bong/client/network/InventorySnapshotHandler.java:139-168` / `client/src/main/java/com/bong/client/inventory/component/GridSlotComponent.java:36-77` + 本 plan P4。

### #6 P1–P4 严格四 PR 串行

**决议**：P0 已是历史 merge；剩余恰好四个 PR，分别只消费 P1、P2、P3、P4。每个 PR 必须先完成仓库 review / CI / merge gate，下一 PR 才从最新 `origin/main` 开始，禁止并行跨阶段实现。P4 在代码/e2e 初步收敛并完成首轮 CI/review 后，于同一 PR 追加阶段状态、真实 `Finish Evidence` 与归档 commit；该新 HEAD 必须完整重验才可 merge，绝不另开第五 PR。

**落点**：本 plan §10。

## §10 实施工作流

本 plan 剩余 scope = P1–P4 四个严格串行 PR。纯逻辑、数据与既有 UI tint 接线，不产出 NBT、layout、bbmodel 或新贴图，因此三轮视觉资产打磨与 `<PROMISE>` 不适用。

### §10.1 串行不变量

1. PR-N 开始前 `git fetch origin`，核验上一 PR 已 merge，并以最新 `origin/main` 建独立 branch/worktree。
2. 一个 PR 只改本 plan 当前阶段，不提前夹带下一阶段，也不修改其它 plan；唯一例外是 P4 在代码/e2e 初步收敛后，于**同一 PR**追加本 plan 的状态、真实 `Finish Evidence` 与 `git mv` 归档 commit，绝不创建第五 PR 或直推 main。
3. 每个 PR 依次完成：饱和测试 → 受影响栈完整本地 gate → 合并最新 `origin/main` 后复验 → `/review` + CodeRabbit → e2e / required CI 收敛 → merge。
4. P4 的归档 commit 改变 HEAD 后，必须在该 exact HEAD 重新跑 validator、审核 Workflow、全部受影响本地门禁、CI、`/review` 与 CodeRabbit；全绿后才 merge。前一 PR 未通过 review/merge gate，后一 PR 不开工；review 返工导致 HEAD 变化同样必须重跑相应验证并重新触发 `/review`。
5. P4 merge 即代表已归档；不得在 merge 后产生归档提交、第五 PR 或直接推送 main。

### §10.2 剩余 PR 拆分

1. **PR-1（P1）migration、全入口/跨库存事务、预提交 owner/filter 与 live rebuild**：从真实 `inventories.schema_version=2` 的一次性 persisted owner migration（严格 canonical `pack_<u64>`、仅 eligible unique owner、save-success 才升版、失败重试）、private non-Clone `#[must_use]` `PreparedInventoryMutation<'a>` 独占 `&mut PlayerInventory`、`prepare_inventory_mutation_with` / consume-self `commit_prepared_inventory_mutation`、identity `InventoryItemPrepareFn`、non-Clone prepared drops/outcome、静态 allowlist/唯一 `find_live_container_owner`、所有 Container from/to 结构门、双向 swap filter 与 self-containment reject。四个自动 ingress `add_item_to_player_inventory_inner` / `add_existing_item_to_player_inventory` / `pickup_dropped_loot_instance` / `force_attach_item_to_inventory` 的全 caller grep 门；后者仅全量转移/骨币制作两个 typed prepared placement caller，禁止 `(0,0)`/假包 bypass。主世界与 TSY 死亡掉落共用唯一 `prepare_death_drop(DeathDropMode::{Overworld,Tsy}) -> PreparedDeathDrop`，保留各自 50%/TSY所得选择策略，在一笔 transaction stage detach、owner rebuild/spill、remaining placement、registry/drop/event 与 TSY corpse/outcome，并预校验 DropContext/position/dimension/registry/corpse context；零选中无 revision/event、任一失败全资源不变。删除 `move_equipped_item_to_first_container_slot`，combat/forge 两 caller只走 `prepare_broken_equipped_item_recovery`，同笔完成 durability终止、resolver-aware收纳或预校验地面掉落、rebuild/spill 和 `WeaponBroken` outcome，revision/drop/event/outcome all once。`transfer_remains_to_looter` stage 源遗骸/骨币与 looter，`apply_treasure_activate(..., false)` stage active/equipped source 与 player inventory；非法候选继续、无 fit source 不变、成功 only-once。四组入口和既有 ingress全入口 grep 门，明确豁免 NPC/world自产 registry insert。普通 move 同一 prepare 计算 targeted wear、`SlotMove` attrition、pack rebuild、freshness/layout/drop/outcome，删/收所有 commit 后 live writer，组合操作总 revision+1、最终 snapshot/outcome exactly once。`exchange_inventory_items` owner 禁交易 typed reject 或非 owner staged 双库存；`transfer_all_inventory_contents` 允许 owner 的 source+target+drop registry/context 多资源原子全量转移、骨币 JS-safe room 全额或整笔 reject；`apply_bone_coin_craft_session` 的 materials+Crafting ledger+Stored/DroppedToGround+event 单事务。仅 server 栈；完整 gate 为 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
2. **PR-2（P2）freshness prepare、唯一 server reconciliation/snapshot outbox 调度与 persistence**：4×4 `apply_container_freshness_transition`、post-transition staged identity/merge、所有 owner-loss staged spill/drop 的 Freeze exit-once、`ContainerState.integrity_lock` 的 `PlayerInventory` serde/default save/load；新增 `server/src/inventory/reconciliation.rs` 的六阶段 `InventoryReconciliationSet::{Commit, SnapshotRequestProducer, Reconcile, Sweep, CollectSnapshots, EmitSnapshots}`、Reconcile 专用 `ContainerIntegrityFreshnessQueue(BTreeSet<Entity>)`、server-only stable-`Ord` `InventorySnapshotReason`、request、`BTreeMap<Entity, BTreeSet<...>>` outbox、唯一 EventReader collect、唯一 drain/send emitter与 `reconcile_container_integrity_freshness`。所有 writer（含大 handler）Commit 后 EventWriter enqueue，join/player attach 在 producer 前，read-only resync producer显式在 Collect前；Added/Changed/revive、botany、生产站、NPC/scroll/world-container/read-only resync 全改 EventWriter，botany删自建 reader/serialize。每 tick `mem::take`、每 entity最终一帧；多 reason稳定合并，缺依赖/serialize/send失败丢弃本 tick entry、不重试旧 revision；Commit corrective、Reconcile 和 Sweep Changed 均同 tick最终输出。覆盖 prepared repair 不双 bump、独立首次/reason-change/repair +1、same reason 0、sweep 实际 variant 才 +1；不新增 snapshot/protobuf/TypeBox/client 的 `acceptance_lock`，但持久化 reason+tick。依赖 PR-1 merge；仅 server 栈并跑完整 server gate.
3. **PR-3（P3）九容器与暗器数据**：九容器完整 TOML `ContainerSpec`、十二暗器 category、coin_box 收纳便利文案、基于已落地 P1 live 语义的 registry/save-load/filter 矩阵测试；八个 non-empty 容器有 filter-invalid 测试，`moisture_guard` 只测 empty 全收和非 filter 负例，`herb_crate_placed` 不改。依赖 PR-2 merge；仅 server 资产与 server 测试，跑完整 server gate.
4. **PR-4（P4）required-array/presence-lock wire、client/e2e 与归档**：P2 persisted integrity reason→optional `{reason}` `ContainerAcceptanceLockV1` / snapshot `acceptance_lock` 映射；健康 lock absent+`[]` 全收、locked present+exact `[]` inert、locked+non-empty/array缺失或null/非法内容整份 fail-closed；18 category wire、Rust/TypeBox/protobuf/Java JsonFormat/client 全链、带新 revision/reason 的 corrective snapshot、Worn/Pack/Inspect 三 surface 的 presence locked source/target 与 VALID/INVALID 预提示、bot 与九容器 e2e。代码/e2e 初步收敛后，在同一 PR 追加阶段✅、真实 `Finish Evidence` 与归档 commit；新 HEAD 重跑 exact-HEAD validator、审核 Workflow、所有受影响门禁、CI、`/review`/CodeRabbit 后才 merge。依赖 PR-3 merge；跑 schema build/test、server 完整 gate、client `./gradlew test build` 与仓库 e2e.

### §10.3 每 PR 独立实施上下文

每个 PR 使用独立实现 agent；prompt 必须带当前阶段范围、上一 PR merge SHA、owner-instance 红线、饱和测试清单与“不得恢复历史路线”。实现 agent 只回报 PR URL、HEAD SHA、测试与 validator 结论；review 返工使用同一远端 PR 分支，禁止重复 promotion 或提前归档。

### §10.4 本 plan 的实施 agent 模型治理（2026-07-24 用户覆盖）

本节按用户 2026-07-24 的最新硬约束，覆盖 `docs/CLAUDE.md §6.4` 对本 plan 的通用模板：实施、编辑、测试、Git 与 GitHub 操作一律使用 `subagent_type: claude`、`model: sonnet`，prompt 末尾必须带 `ultrathink`。GPT/Opus 只能用于 read-only validator、对抗审核或 final judge，绝不得实施。实现 agent 只负责实施、测试、提交、push、提 PR，不等待 review 或 merge；主流程负责 ScheduleWakeup、返工调度、merge 与清理。

### §10.5 完成条件

只有 P1–P4 全部 merge、九容器 e2e 通过、跨端 symbols 对拍且 P4 同 PR 内的 `Finish Evidence` 五栏填实、归档 commit 及其新 HEAD 所有门禁/CI/review 重验全绿后，才把本 plan 视为完成。P0 的历史 merge 不能替代剩余四阶段验收。


## Finish Evidence

### 落地清单

### 关键 commit

### 测试结果

### 跨仓库核验

### 遗留 / 后续
