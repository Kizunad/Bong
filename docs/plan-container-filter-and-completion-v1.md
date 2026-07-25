# plan-container-filter-and-completion-v1 — owner-instance 容器筛选、durable settlement 与九容器闭环

> **主题**：在 `ContainerState.owner_instance_id` + `PlayerInventory.containers` 平展模型上统一 owner/filter、普通 prepared mutation；以 durable source settlement 原子结算矿物/锻造的源、物品、经验和掉落；最终交付九容器与 client 预提示。
> **状态**：Active。P0 已于 2026-06-13 合入；P1A 有少量 resolver/grant 代码超前但未闭环；P1B durable source settlement 未开始；P2/P3/P4 未开始。
> **历史证据**：P0 = PR #526，merge commit `3161ccf0ba1ff25d5ab781e654667090b0e143ac`（2026-06-13），仅证明 P0 数据模型和测试，绝不外推 P1A–P4。

## 阶段总览

| 阶段 | 内容 | 状态 | 验收日期 |
|---|---|---|---|
| P0 | `ItemCategory` 三变体、`ContainerAcceptFilter`、`ContainerSpec.accept_filter`、TOML `accept`、`item_passes_filter` | ✅ | 2026-06-13 |
| P1A | process-local prepared core：owner/category/filter/fit、普通 move/swap、非 durable ingress 与 owner-loss staged transaction | 🔄 | |
| P1B | mineral/forge durable source settlement：stable ID、source/WAL、单事务 inventory/skill/drop/source/station 与恢复 | ⬜ | |
| P2 | freshness transition、integrity/reconciliation、唯一 snapshot producer→reader/outbox/emitter | ⬜ | |
| P3 | 暗器 category 与恰好九个随身 `ContainerSpec` | ⬜ | |
| P4 | filter/`acceptance_lock` wire、client 预提示、e2e、真实 Finish Evidence 与归档 | ⬜ | |

> P1A 的 `🔄` 只表示既有 `container_accepts_runtime_grant` 等局部代码可复用；权威 move/swap、全入口、owner-loss 未收敛。P1B 不存在 durable 实现；现有 `MineralDropEvent` consumer 和 runtime `ForgeSessionId` 不得作为完成证据。

## 接入面（防孤岛）

- **进料**：`server/src/inventory/mod.rs::{PlayerInventory,ContainerState,ContainerAcceptFilter,item_passes_filter}`；`InventoryMoveIntent → handle_inventory_move → apply_inventory_move_with_race`；death/broken/remains/treasure/trade/full-transfer/bone-coin/`ExternalContainer`；mineral `break_handler.rs` 和 forge `session.rs`/`station.rs` source state。
- **durable 保存面**：`MineralOreNode`/`MineralOreIndex`、`ForgeSession`/`ForgeSessions`、`player/state.rs::{load_player_inventory_from_sqlite,persist_player_inventory_json_in_transaction}`、`inventories`、`player_skills`、`DroppedLootRegistry`/`dropped_loot`。P1B 必增 transaction-local skill/source/drop helper，禁止拼接现有独立 save API。
- **出料**：P1A 普通路径只写 process-local live commit；P1B 只由 durable transaction 返回 committed snapshots 再安装 ECS，install 后才发 stable-ID committed event。P2 才统一 snapshot；P4 才 wire/client。
- **共享类型/event**：复用 `ItemCategory`、`InventoryLocationV1`、`InventoryMoveOutcome`、`ContainerFreshnessBehavior`、`SkillSet`、`DroppedLootRegistry`。P1A 是 borrowed `PreparedInventoryMutation<'a>`；P1B 是 owned serializable `PreparedInventorySettlement`、公共 `InventorySettlementId` 与 `InventorySettlementCommittedEvent`（允许等价名但全文一致）。Entity/tick/runtime `ForgeSessionId` 绝不可作 durable key。
- **跨仓库契约**：P1A/P1B 不新增、迁移、锁定或验收 inventory snapshot sender、EventWriter/outbox、final snapshot once；既有 request/resync 只由 P2 替换。P4 才动 `ContainerSnapshotV1`/`InventoryItemViewV1`、protobuf、TypeBox 和 client。
- **worldview / qi**：`coin_box` 只做骨币分类；本 plan 不新增 qi 公式，锻造既有真元流仍走 ledger；freshness 复用既有 shelflife 行为。

## 当前架构基线与范围边界

1. `owner_instance_id` 为唯一 owner 真源；`pack_<instance_id>` 只为 UI/protocol ID，绝不从字符串反推、补写或授权 owner。
2. `classify_container_ownership`/`find_live_container_owner` 是唯一 resolver。静态 allowlist 仅 `body_pocket`、`main_pack`、`small_pouch`、`front_satchel`；live owner 仅 worn、held、hotbar、`body_pocket`。其它 grid、任意 `pack_*` 与 owner 自身容器不可作为 live surface。
3. 每个 `InventoryLocationV1::Container` from/to 先过同一结构门；from 不做 filter，to 才做 filter；owner 进入自身 `pack_<owner_id>`（move/swap 两方向）为 typed `ContainerOwnerSelfContainment`，零副作用。
4. `resolve_item_category(item,item_registry,mineral_registry)` 是唯一 category authority：registered template 取 canonical category；未注册仅 canonical `mineral_id`、registry hit、精确 `mineral_<canonical>` template 三者齐备才 Mineral；spoof/unknown/mismatch typed invalid，绝不 fallback `Misc`。
5. `water_skin` 已移出 scope，仅保留“历史移交 satiety/hydration”的说明；不得出现于容器数量、grid/filter、`ContainerSpec`、阶段交付、测试或 PR scope。`trade_crate`/`dead_drop_box` 是 placeable 边界，`herb_crate_placed` 为独立 placed twin，P3 不实现转换。
6. nested/session 容器路线已否决，唯一玩家移动入口仍是 `InventoryMoveIntent`；forge runtime session 只投影 durable `ForgeAttemptId`。

## P0 — 筛选数据模型 ✅ 2026-06-13

完成 `ItemCategory::{Mineral,Anqi,Liquid}`、`ContainerAcceptFilter::{Category,TemplatePrefix}`、`ContainerSpec.accept_filter`、TOML accept 和 `item_passes_filter` 的 None/empty/OR 及正反测试。runtime grant、move、freshness、durable source、九容器、wire/client 均不属于 P0。

## P1A — process-local owner/filter prepared core 🔄

### 可核验交付物

1. 在 `server/src/inventory/mod.rs` 固定 `classify_container_ownership`、`find_live_container_owner`、`resolve_container_acceptance`、`resolve_item_category`，落实上述 allowlist、eligible surface、typed corruption、目标 filter OR；`rebuild_containers_from_equipment` 复用它。
2. 版本化 owner migration 替换 load-time prefix backfill：从 `inventories.schema_version` v2 起，严格 parser 只接受 canonical `pack_<u64>`，拒 `pack_01`/空白/符号/溢出；仅唯一 eligible live owner 且有效 `ContainerSpec` 可写 `owner_instance_id`。inventory JSON SQLite commit 成功后才升版；失败保留 pending；不 bump revision、不发 snapshot。
3. `PreparedInventoryMutation<'a>` 为 private、non-Clone、`#[must_use]` 的借用 process-local 载体：prepare→`commit_prepared_inventory_mutation(self)` 独占 `&'a mut PlayerInventory`，不得存 resource/event、跨 handler/tick/DB 或并存。仅含 staged inventory、route、merge/push、`Vec<PreparedInventoryDrop>`、一次 outcome 与 debug `original_revision`，无 CAS。
4. `commit_prepared_inventory_mutation(self)` **仅**普通 process-local 路径使用：prepare 先验 bounds/collision/weight/drop context，commit 不可失败地一次替换 live inventory、一次 enqueue drops、一次发普通 outcome，revision 恰 +1。它不是 crash atomic；mineral/forge durable source 禁止调用，generic commit 不得冒充 durable settlement。
5. 新增 `PreparedInventorySettlement`：owned、可序列化、无 live borrow、不得直接 commit/outcome；以同一 resolver/filter/fit/drop policy 产生 candidate `PlayerInventory`、candidate drops/result snapshot，交给 P1B 唯一 durable commit API。不得让 borrowed mutation 跨 tick/SQLite。
6. P1A 的 `InventoryItemPrepareFn` 仅原样携带 freshness 字段；不得执行 Freeze/Normal 或任何 transition。ordinary move/swap、owner-loss spill/drop、durable settlement 同样遵守；P2 才替换/扩展 hook。
7. 普通 move/swap、四 automatic ingress、`ExternalContainer` 双资源 move、owner-loss（discard/morph/backpack break）、death/TSY death、broken recovery、remains、treasure deactivate、trade、full transfer、bone-coin 与非 durable `_or_ground` ingress 全走同一 staged contract。ExternalContainer 成功 atomically replaces both resources，player +1；失败两边深比较不变。owner-loss 在同一 transaction 做 detach→rebuild→spill→drop→outcome。
8. `transfer_all_inventory_contents` 可转 owner，但 source/target/`DroppedLootRegistry`/`DropContext` 必为 staged multi-resource transaction；live owner 交易必 `ContainerOwnerTradeForbidden`；bone coin craft 的 ledger/storage/drop/event 同 transaction。NPC/world 自产不是玩家 ingress。
9. 不新增、不迁移、不锁定、不验收 snapshot sender；不要求 EventWriter/outbox/final snapshot once；保持既有 request/resync 给 P2。

### 饱和测试

普通 prepared 的 new/partial/full merge、move/swap 双向 filter、self-containment、owner-loss/rebuild/spill/drop、ExternalContainer 双向及所有 non-durable ingress；成功 revision 恰 +1、任一 resolver/category/bounds/collision/weight/drop-context 失败全回滚；ordinary drop/outcome 恰一次。compile/API pin 证明 private non-Clone `#[must_use]`、活借用期间不能二借/改 revision/layout；freshness raw 字段完全不变；不测 transition、snapshot sender/outbox/final snapshot。

## P1B — mineral/forge durable source settlement ⬜

### Stable ID、source/WAL 状态

1. 公共 `InventorySettlementId` 为 UUID/ULID；retry/reconnect/reload/notification 全复用。禁止 Entity、tick、runtime `ForgeSessionId`。
2. mineral source key 至少含 dimension、`BlockPos`、canonical `mineral_id`、持久 `generation_id`、单调 `extraction_seq`/source revision；同坐标再生必为新 generation，绝不串账。
3. 新增持久 `ForgeAttemptId`：inputs accept/lock 时创建并写 source record；runtime `ForgeSession` 只带投影。Perfect/Good/Flawed/Waste/Explode 五档 terminal 只 CAS 一次。
4. durable source/settlement WAL row（可等价命名但全文一致）保存 stable ID、canonical player identity、immutable source before/after、完整 item 或 no-item payload、XP delta、drop context、forge quality/color/effects/tier/consecration/station state、audit。状态至少 `Prepared`、`CommittedStored`、`CommittedDroppedToGround`、`CommittedNoItem`；Perfect/Good/Flawed 前两种 item terminal，Waste/Explode 仅 NoItem，Explode 持久 station integrity/wear；五档均唯一 terminal row。
5. **禁止路线**：consumer-first `insert_if_absent PendingInventoryIngress` 不是耐久；不得保留/重发/重读 transient source event；不得 generic commit 后立即 outcome；不得将网络通知承诺为端到端一次交付；不得留下模糊 consumer pending worker 充 source truth。

### 唯一 durable commit API：WAL 顺序

1. **线性化**：mineral break/forge attempt 先 durable 写 `Prepared`，再取得 stable ID。此前不得扣 mineral remaining、despawn、将 forge session 标 Done、发 XP 或 outcome。重复 source 只拿同 ID。
2. **transaction**：同 ID + expected source/inventory/skill revision 进入明确单 connection `BEGIN IMMEDIATE`（或等价）；CAS `Prepared`。同一 transaction 写 source terminal、`inventories` candidate JSON、`player_skills` candidate JSON、可选 `dropped_loot`、settlement terminal/audit；新增 transaction-local skill/source/drop helpers，禁止拼既有独立 save API；首次无 inventory row 也在这里创建。
3. **失败**：任一 SQL/serialize/CAS 失败 rollback；DB/ECS/source/drop/XP/outcome 均不变，Prepared 留存重试。
4. **install**：commit 成功后同一主线程安装 transaction 返回的**同一 committed snapshots** 至 `PlayerInventory`、`SkillSet`、`DroppedLootRegistry`、mineral node/index/despawn 或 forge session/station ECS。不得先改 ECS，防 autosave/disconnect 用旧 ECS 覆盖 DB。
5. **event**：install 后才发 keyed `InventorySettlementCommittedEvent`。经济 source/item/XP/drop/terminal row exactly-once；network/audio/Redis 为 keyed-ID at-least-once + consumer dedupe。若用 durable notification outbox，必须写状态/ack；否则明确 commit 后通知可重放但不能重复经济副作用。
6. **恢复**：startup/player hydration 的 Prepared 同 ID retry；Committed 从 DB snapshot 重装 ECS，不重算经济；覆盖 commit 后/ECS install 前 crash，autosave/disconnect/shutdown 不能回退 DB。

### 生产切流与测试

`mineral/break_handler.rs` 先 source Prepared/transaction，成功后才更新 remaining/exhausted/index/despawn；`MineralDropEvent` 最多为含 ID wake/notification。forge 在 inputs lock 写 `ForgeAttemptId` source，canonical XP pure function 产 candidate `SkillSet`，五档终局同 transaction；禁止先 `SkillXpGain` 再 `ForgeOutcomeEvent`。inventory bridge/S2C/Redis/audio/source readers 改读 committed-ID event/dedupe，P1B 不接 P2 snapshot routing。

测试必须覆盖：mineral generation/sequence、同坐标再生、forge Perfect/Good/Flawed 的 Stored 与 DroppedToGround、Waste/Explode 的 NoItem（Explode station integrity/wear）、满包不可drop、首次无 inventory row、duplicate source/wakeup/retry/reconnect/reload；Prepared 前、Prepared 后/txn 前、每个 SQL write 失败、commit 后/ECS install 前、install 后/notification 前、stale source/inventory/skill revision；autosave/disconnect/shutdown 不回退；经济物/XP/source/drop/outcome row 恰一次；notification 按 ID dedupe 且不声称网络 exactly-once。P1A 未 merge不得开 P1B；P1B 完整 crash matrix 未过不得开 P2。

## P2 — freshness、integrity、snapshot 唯一调度 ⬜

P2 独占 `apply_container_freshness_transition`、`sealed_vial=Halve`、`spirit_seal_box=Freeze`、`moisture_guard=SpoilOnly { rate: 0.3 }` 的 4×4 mapping；P1A/P1B 全部 writer 以 post-transition identity merge。持久 `integrity_lock(reason,detected_tick)`，唯一 `reconcile_container_integrity_freshness` 管 owner corruption。固定 `InventoryReconciliationSet::{Commit,SnapshotRequestProducer,Reconcile,Sweep,CollectSnapshots,EmitSnapshots}`：所有 P1A/P1B business/reconciliation writer 只 request；唯一 reader/outbox/emitter 每 entity/tick 最终一帧。P2 不 retry settlement、不补发 XP/物品、不把 settlement retry 混入 reconciliation。

**饱和测试**：逐格覆盖 4×4 freshness 转换及每个 P1A/P1B writer 的 raw→transition、merge identity、spill/drop；保存/加载每种 integrity lock、首次/reason-change/repair revision +1 与 same-reason 0；静态 gate 断言唯一 `EventReader<InventorySnapshotRequest>` 和唯一 emitter；同 tick 多 P1A/P1B writer、多 reason、Added/Changed/join/revive、两个 entity 隔离、缺 client/serialize/send failure drain、最终 revision/content 一帧。任何 snapshot/reconcile 测试都不得重试 settlement 或重放经济副作用。

## P3 — 暗器迁移与恰好九个随身容器 ⬜

| # | template | grid | capacity | slot | cost | accept | freshness |
|---|---|---|---:|---|---:|---|---|
|1|herb_pouch|3×3|8.0|chest|0.008|herb, food|Normal|
|2|ore_sack|3×3|10.0|chest|0.005|mineral|Normal|
|3|projectile_bag|3×4|10.0|legs|0.005|anqi|Normal|
|4|herb_crate|4×4|10.0|chest|0.005|herb|Normal|
|5|sealed_vial|2×2|8.0|chest|0.008|pill, food|Halve|
|6|spirit_seal_box|2×2|10.0|chest|0.005|treasure, pill|Freeze|
|7|moisture_guard|3×3|8.0|legs|0.008|empty/all|SpoilOnly { rate: 0.3 }|
|8|coin_box|3×3|8.0|chest|0.008|bonecoin|Normal|
|9|sealed_envelope|1×2|8.0|head|0.008|recipe_fragment, recipe_hint, scroll|Normal|

全部显式 `attrition_exempt=false`、`quick_access=false`，迁为 container；12 暗器迁 Anqi，`projectile_bag` 只收 Anqi；coin_box 只承诺骨币分类收纳便利。测试九 spec、八个 nonempty filter reject、moisture_guard empty-all/非filter负例、12 暗器、save/load 与文案禁保鲜保值。

## P4 — filter wire、client 预提示、e2e 与归档 ⬜

`ContainerSnapshotV1.accept_filter` required array/repeated，optional presence `acceptance_lock={reason}`；健康 absent+`[]` 才 all，present 时 exact `[]` inert；P4 从 P2 persisted reason 投影，不下发 tick。`InventoryItemViewV1.category` 由 P1A resolver 产 canonical lower-snake；Rust/protobuf/TypeBox/Java/client 同步，非法/unknown/missing/null/extra key 整份 snapshot fail-closed。Worn/Pack/Inspect 以 `ContainerFilterRules.accepts` 预提示，预测非法/locked 仍发 `InventoryMoveIntent`，server 权威。P4 不接 snapshot routing。bot/e2e 覆盖九容器、ore_sack pass/herb reject、swap reverse、moisture_guard、locked source/target、integrity revision、同 template 双 owner。

**饱和测试**：两种 filter tagged variant、required array/repeated、healthy absent+empty 与 locked present+exact empty、locked non-empty、缺/null/unknown/extra-key 的 Rust/protobuf/TypeBox/JsonFormat/client parser fail-closed；18 个 canonical category 和 synthetic mineral 正反；Worn/Pack/Inspect 的 category/prefix/empty/footprint/lock `VALID`/`INVALID` 与仍发送 Intent；bot/e2e 对九容器逐一真实验证。

## §8 开放问题（历史表，全部已收口）

| # | 历史问题 | 状态 |
|---|---|---|
|1|平展 owner-instance 或 nested/session|§8.1 #1|
|2|resolver、mineral/forge source durability|§8.1 #2|
|3|P1 raw freshness 与 P2 transition|§8.1 #3|
|4|容器集合/矩阵|§8.1 #4|
|5|filter/lock client 预提示|§8.1 #5|
|6|剩余 PR 拆分|§8.1 #6|

> 原表仅供历史追溯；实施以 §8.1 为准。

## §8.1 决议（pre-P1 收口，2026-07-24）

### #1 owner-instance 平展模型唯一
**决议**：`PlayerInventory.containers` + `owner_instance_id`、strict resolver/migration 和 `InventoryMoveIntent` 为唯一架构，不恢复 nested/session。

**落点**：`server/src/inventory/mod.rs:469-482`（`ContainerState`）/ `server/src/inventory/mod.rs:6549-6592`（静态分类）+ 本 plan「基线」「P1A」。

### #2 resolver + durable source/WAL
**决议**：P1A 交付 process-local prepared core；borrowed mutation/ordinary commit 永不跨 DB、不作 crash atomic。P1B 交付 `PreparedInventorySettlement`、`InventorySettlementId`、mineral generation/sequence、`ForgeAttemptId`、Prepared→Committed source/WAL 和单事务 inventory/skill/drop/source/station，commit 后安装同 snapshots ECS、再发 committed-ID event。现有 mineral direct consumer/forge direct grant 只为待替基线；禁止 consumer-first pending、source-event 重发、runtime session key、generic durable commit 和端到端一次通知承诺。

**落点**：`server/src/mineral/break_handler.rs:170-260`（source 起点）/ `server/src/mineral/inventory_grant.rs:38-138`（direct consumer）/ `server/src/forge/session.rs:81-160`（runtime session）/ `server/src/forge/inventory_bridge.rs:12-145`（direct grant）/ `server/src/player/state.rs:1313-1375,2342-2367`（load/transaction helper）+ 本 plan「P1A」「P1B」。

### #3 P1 raw freshness，P2 transition
**决议**：P1A/P1B hook 原样携 freshness；P2 独占 transition、integrity、reconciliation 和 six-stage snapshot。P2 观察 committed writers，不 retry settlement/补发经济。

**落点**：`server/src/shelflife/container.rs:29-98` / `server/src/spiritwood/mod.rs:611-647` / `server/src/inventory/mod.rs` + 本 plan「P1A」「P1B」「P2」。

### #4 九容器固定
**决议**：P3 仅九容器表、12 暗器和骨币分类文案；placeable 边界不变。

**落点**：`server/assets/items/workbench_materials.toml` / `server/assets/items/anqi.toml` + 本 plan「P3」「范围边界」。

### #5 P4 仅 wire/client
**决议**：P4 投影 P2 lock、filter/category，并在三 surface 预提示；不新增 snapshot routing。

**落点**：`server/src/schema/inventory.rs:241-287` / `proto/bong/envelope.proto:688-695` / `agent/packages/schema/src/inventory.ts:322-363` / `client/src/main/java/com/bong/client/network/InventorySnapshotHandler.java:139-168` + 本 plan「P4」。

### #6 P1A→P1B→P2→P3→P4 五 PR 严格串行
**决议**：剩余恰好五 PR：PR-1/P1A、PR-2/P1B、PR-3/P2、PR-4/P3、PR-5/P4+e2e+真实 Finish Evidence+归档。严格前序 merge 后才开下一 PR；P1B crash matrix 是 P2 前置；P4 仅在 PR-5 全部满足后归档，不能提前完成。

**落点**：本 plan §10。

## §10 实施工作流

### §10.1 串行不变量
1. 每 PR 前 `git fetch origin`，核验上一 PR merge，以最新 `origin/main` 建独立 branch/worktree。
2. 一 PR 只做当前阶段，不夹带下一阶段或其它 plan；每次 HEAD 变化重跑受影响 validator/gate/review。
3. P1A merge 前不开 P1B；P1B WAL/crash/recovery/exactly-once economics matrix 通过前不开 P2；P4 e2e 后才写真实 Evidence 和归档，并在归档新 HEAD 完整复验。

### §10.2 五个严格串行 PR
1. **PR-1 / P1A**：owner migration/resolver/category、process-local `PreparedInventoryMutation`、owned settlement preparation、ordinary/non-durable staged writers 与 identity freshness；不动 snapshot sender，不做 durable source commit。
2. **PR-2 / P1B**：settlement ID、mineral generation/sequence、ForgeAttemptId、WAL、single transaction、ECS install/hydration/retry/committed-ID dedupe 和 crash matrix；不接 P2 snapshot routing。
3. **PR-3 / P2**：freshness transition、integrity/reconciliation、六阶段 snapshot pipeline，覆盖 P1A/P1B writers，不重算 settlement。
4. **PR-4 / P3**：九容器 TOML、12 Anqi、coin 文案、registry/save-load/filter tests；不含 placeable 转换。
5. **PR-5 / P4**：filter/lock/category wire、Worn/Pack/Inspect、bot/九容器 e2e；全部验收后填写真实 Finish Evidence、阶段状态与归档。归档 commit 后重跑 exact-HEAD validator、门禁、CI、`/review`、CodeRabbit。

### §10.3 完成条件
仅当五 PR 严格按序 merge，P1B crash matrix、P2 reconciliation/snapshot、P3 九容器、P4 cross-stack e2e 全实证通过，且 PR-5 填满以下 Evidence 并在归档新 HEAD 复验后，本 plan 才完成。

## Finish Evidence

> 待未来 PR-5 实际消费填写；不得预填测试、日期、commit 或完成声明。

### 落地清单
- 待填写：各阶段真实文件与 symbols。

### 关键 commit
- 待填写：hash、日期、阶段与理由。

### 测试结果
- 待填写：实际命令、数量、P1B crash matrix 与 P4 e2e。

### 跨仓库核验
- 待填写：server / schema / protobuf / client 实际命中 symbols。

### 遗留 / 后续
- 待填写：本 plan 外明确依赖与未覆盖项。
