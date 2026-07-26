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
7. P1A 固定内部 `ContainerAcceptanceLockReason::{OwnerMissing, OwnerNotFound, OwnerInvalid}`，判定口径即上第 2 条 resolver 的 owner allowlist/eligible surface 三态；P2 持久化 `integrity_lock.reason` 与 P4 `acceptance_lock.reason` 均取该类型，wire 编码见 P4。

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

### Stable ID、durable mineral source/WAL 与 station 状态

1. 公共 `InventorySettlementId` 为 UUID/ULID；retry/reconnect/reload/notification 全复用。禁止 Entity、tick、runtime `ForgeSessionId`。
2. 新增 SQLite `mineral_sources` 为矿物唯一 durable 真源。复合 identity 固定为 dimension、`BlockPos`、canonical `mineral_id`、`generation_id`；row 至少持 `extraction_seq`、remaining、`state: Active|Exhausted|Respawning`、revision、nullable respawn deadline、schema_version 与 audit timestamps。初始 anchor/旧档迁移、每次 extraction terminal、exhaustion、respawn generation +1 都必须在 durable transaction create/CAS 推进 row，随后才幂等投影 `MineralOreNode`/`MineralOreIndex`/block。settlement source unique key 必须引用该 row 的 generation + extraction_seq + revision；同坐标再生绝不串账。
3. `data/minerals/exhausted.json` 和 `ExhaustedMineralsLog` 只做一次性导入：导入时按 anchor 维度赋值、分配 generation 并落 `mineral_sources` 后立即退役；之后它们最多是从 SQLite 派生的 projection/删除路径，绝不能独立决定 spawn、exhaustion 或 respawn。坏/歧义 legacy entry fail-closed，不以空 log 猜测新 source。
4. `ForgeStationId { dimension: DimensionKind, pos: BlockPos, generation: u64 }` 是唯一锻炉 durable identity，**owner 不入 ID**。SQLite `forge_stations` 是真源，ECS `WeaponForgeStation` 与 ANVIL block 只为投影。row 至少持完整 ID、tier、nullable canonical owner player identity、integrity、revision、`state: Active|Removed`、nullable `active_attempt_id`、schema_version 与 audit timestamps。
5. `ForgeStationPlacementId` 必须稳定且可重试去重（由被消费的唯一 station item instance + target `ForgeStationId`/immutable request key 派生）。placement 在同一 `BEGIN IMMEDIATE` 以内 canonical player + expected inventory revision CAS 精确 station item instance（删除或递减）、持久 candidate inventory/revision、按 `(dimension,pos)` 分配 generation、写 `forge_stations Active` 及 placement audit/idempotency row；commit 后才安装**同一** inventory snapshot 并投影 ECS/ANVIL。SQL/CAS 失败零扣物/零 station；commit/install 间 crash 从 DB 重装；重复 request 返回既有 placement outcome，绝不二扣。
6. 新增持久 `ForgeAttemptId`：inputs accept/lock 的同一 `BEGIN IMMEDIATE` 必须以 expected inventory revision CAS 精确 input instance IDs/stack slices 与数量，持久化扣除后的 candidate inventory、immutable escrow payload、完整 `ForgeStationId`、canonical player、expected station revision、Prepared attempt row，并 CAS station `(id, expected revision, active_attempt_id=None)` 为 `active_attempt_id=attempt` 和 revision +1。runtime `ForgeSessionId`/Entity 只作投影；terminal 只能消费 attempt escrow，禁止重读 live inventory。
7. durable source/settlement WAL row（可等价命名但全文一致）保存 settlement ID、canonical player identity、immutable source before/after、完整 item 或 no-item payload、XP delta、drop context、forge quality/color/effects/tier/consecration、完整 station state 与 audit。状态至少 `Prepared`、`CommittedStored`、`CommittedDroppedToGround`、`CommittedNoItem`；Perfect/Good/Flawed 只能走前两种 item terminal，Waste/Explode 只能 NoItem；Explode 同时持久 station integrity/wear，五档都有唯一 terminal row。
8. **禁止路线**：consumer-first `insert_if_absent PendingInventoryIngress` 不是耐久；不得保留/重发/重读 transient source event；不得 generic commit 后立即 outcome；不得将网络通知承诺为端到端一次交付；不得留下模糊 consumer pending worker 充 source truth。

### 唯一 durable commit API：WAL、source/station CAS 与恢复顺序

1. **线性化**：先生成 candidate `InventorySettlementId`，再以 `mineral_sources` immutable row identity + generation/extraction_seq/revision 或 immutable forge attempt source key + candidate ID 插入 durable `Prepared`，该 insert 是线性化点。重复 source 的 unique conflict 必须查询并返回既有 settlement ID，绝不重生成经济；在 Prepared 前 source/inventory/XP/outcome 均零副作用。此后才可按该 ID 重试。
2. **transaction**：同 ID + expected source/inventory/skill revision 进入明确单 connection `BEGIN IMMEDIATE`（或等价）；CAS `Prepared`。矿物 terminal 必须同一 transaction CAS `mineral_sources` extraction_seq/remaining/state/revision；forge terminal 还必须 CAS attempt `Prepared` 和 station `(完整 ForgeStationId, expected revision, active_attempt_id=attempt)`，清 durable active lock 并 revision +1：Perfect/Good/Flawed 写 Stored/Dropped item terminal，Waste/Explode 写 NoItem；只有 Explode 改 integrity/wear，其余四档只清 lock。transaction 内同时写 source terminal、`inventories` candidate JSON、`player_skills` candidate JSON、可选 `dropped_loot`、station/attempt/settlement terminal 与 audit。必须新增 transaction-local skill/source/drop/station/mineral helper，禁止拼接既有独立 save API；首次无 inventory row 也在这里创建。
3. **escrow cancel/failure**：Prepared attempt 的 escrow 始终归该 attempt 所有；restart 只重建 session 并继续，普通 move/trade/death/disconnect 不得再移动已 escrow 材料。若产品允许取消，仅 typed cancel 可在同一 transaction 将 escrow 返还 candidate inventory 或 durable drop、清 station lock、写 terminal audit；其它 typed failure 保持 Prepared/retry，禁止模糊回退或双返还。
4. **失败**：任一 SQL/serialize/CAS 失败 rollback；DB/ECS/source/drop/XP/outcome 均不变，Prepared 保留可重试；stale source/inventory/skill/station revision 或 active lock 不匹配是 typed reject，绝不以 live ECS 覆写数据库。
5. **矿物恢复与 block gate**：startup `MineralSettlementRecovery` 是唯一 scanner/worker，必须在 world/player normal writers 前读取 Prepared WAL + `mineral_sources`，按 immutable candidate 确定性继续 commit（优先，避免撤销语义分叉）；Committed 只 install snapshot、不重算。Prepared terminal commit 前门控/抑制 `apply_default_block_break` 的 AIR 写入，失败保持或恢复 source block projection；仅 terminal commit 后才投影 AIR/exhausted/index/despawn。即使关服后玩家不再上线/不 retry，startup 也必须完成 Prepared 到唯一终态；不得依赖 transient `MineralDropEvent`。
6. **install**：commit 成功后同一主线程安装 transaction 返回的**同一 committed snapshots**至 `PlayerInventory`、`SkillSet`、`DroppedLootRegistry`、`mineral_sources` 对应 node/index/block 或 forge session/station ECS。不得先改 ECS，防 autosave/disconnect 用旧 ECS 覆盖 DB。
7. **station lifecycle**：专用 forge break 路径必须先于 `world::block_break::apply_default_block_break` 的 AIR 写入执行；`active_attempt_id` 非空即 typed busy reject，只有空闲 station 才在 transaction 写 Removed + revision 后删除 ECS/block 投影。startup 读取 Active rows 建 durable index/ECS；dimension chunk 未加载时保留 DB/index 状态并延期、幂等投影 ANVIL，绝不因 block 暂缺删除 row。startup/player hydration 对 Prepared attempt 以原完整 `ForgeStationId` 和 escrow 重建 runtime session；Committed 只从 DB snapshot 重装 ECS，不重算经济；同址 generation 隔离旧 attempt。
8. **event/notification**：install 后才发 keyed `InventorySettlementCommittedEvent`。经济 source/item/XP/drop/terminal row exactly-once；不在本 plan 增加 durable notification outbox/ack，因此 network/audio/Redis 仅为 keyed best-effort wake/notification，重复可按 settlement ID 去重，断线/重启依赖 authoritative DB hydration/resync，绝不声称端到端 at-least-once 或 exactly-once。

### 生产切流与饱和测试

`mineral/break_handler.rs` 先通过 `mineral_sources` 写 Prepared/transaction，成功后才投影 remaining/exhausted/index/despawn；`MineralDropEvent` 最多为含 ID wake/notification，不能作为恢复输入。旧 exhausted JSON 一次性导入/退役后断开 anchor/respawn authority。`MineralSettlementRecovery` 在 normal writers 前完成无客户端 Prepared。forge station placement 走 `ForgeStationPlacementId` 的 inventory+station 单事务后才投影；专用 break 在默认 AIR 前 CAS；inputs lock 把精确材料转入 `ForgeAttemptId` escrow；canonical XP pure function 仅针对 escrow 产 candidate `SkillSet`，五档终局同 transaction；禁止先 `SkillXpGain` 再 `ForgeOutcomeEvent`。inventory bridge/S2C/Redis/audio/source readers 改读 committed-ID wake 并去重，P1B 不接 P2 snapshot routing，也不扩展其它 placeable。

测试必须覆盖：`mineral_sources` 初始 anchor/legacy JSON 一次导入、维度/generation/extraction_seq/revision、同坐标再生、每次 extraction/exhaustion/respawn CAS、JSON 退役后不能决定 spawn；Prepared 前与 Prepared 后关服、玩家永不重连时 startup recovery 到唯一 terminal、AIR gate/失败 block 恢复/terminal 后才 despawn；station 同址 generation、placement item 精确扣除/重复 placement 幂等/SQL 失败零扣物零投影/commit-install crash reload、busy break typed reject、空闲 break Removed、chunk unloaded deferred ANVIL projection；forge input exact instance/slice escrow、duplicate Start、stale inventory/station revision、lock 前后 crash、重连/死亡/交易/断线不能移动 escrow、Prepared restart session hydration、typed cancel 同事务返还或 drop、五档全部释放 active lock、Perfect/Good/Flawed Stored/Dropped、Waste/Explode NoItem、Explode station wear exactly once、非 Explode 不改 integrity；满包不可drop、首次无 inventory row、duplicate source/wake/retry/reconnect/reload；每个 SQL write 失败、commit 后/ECS install 前、install 后/notification 前、terminal retry；autosave/disconnect/shutdown 不回退；经济物/XP/source/drop/outcome row 恰一次；best-effort notification 只按 ID dedupe，不声称 delivery guarantee。P1A 未 merge不得开 P1B；P1B 完整 crash matrix 未过不得开 P2。

## P2 — freshness、integrity、snapshot 唯一调度 ⬜

P2 独占 `apply_container_freshness_transition`、`sealed_vial=Halve`、`spirit_seal_box=Freeze`、`moisture_guard=SpoilOnly { rate: 0.3 }` 的 4×4 mapping；P1A/P1B 全部 writer 以 post-transition identity merge。持久 `integrity_lock(reason,detected_tick)`，唯一 `reconcile_container_integrity_freshness` 管 owner corruption。固定 `InventoryReconciliationSet::{Commit,SnapshotRequestProducer,Reconcile,Sweep,CollectSnapshots,EmitSnapshots}`：所有 P1A/P1B business/reconciliation writer 只 request；唯一 reader/outbox/emitter 每 entity/tick 最终一帧。P2 不 retry settlement、不补发 XP/物品、不把 settlement retry 混入 reconciliation。

- locked 期间该容器 behavior 强制 `Normal`；任何 active Freeze interval 整段 discard（`frozen_since_tick` 清空且**不**计入 `frozen_accumulated`），items 的位置/stack/layout 不变。
- 同 reason 重复检测零修改、revision 不变；reason 改变则更新 reason/detected_tick 并同样 discard active interval。
- repair 在**同一原子 commit** 清 `integrity_lock`：修复后 behavior=Freeze 则以新 `now_tick` 对每个 Freshness item `enter_container` 重开 interval，否则保持 None；首次 lock/reason-change/repair 各恰 +1，same-reason 0。
- 4×4 mapping 与上述强制 `Normal` 均锚回既有实现 `server/src/shelflife/container.rs`：`container_storage_multiplier`（:30，Halve 非 Stepwise 减半/Stepwise 退 Normal，Freeze time-based 0.0/Stepwise 必须 1.0**禁 0.0**）、`enter_container`（:82）、`exit_container`（:94，SpoilOnly{0.3} 仅 Spoil track/Decay·Age 退 Normal）。

**饱和测试**：逐格覆盖 4×4 freshness 转换及每个 P1A/P1B writer 的 raw→transition、merge identity、spill/drop；保存/加载三个 reason 各一条 integrity lock、首次/reason-change/repair revision +1 与 same-reason 0；静态 gate 断言唯一 `EventReader<InventorySnapshotRequest>` 和唯一 emitter；同 tick 多 P1A/P1B writer、多 reason、Added/Changed/join/revive、两个 entity 隔离、缺 client/serialize/send failure drain、最终 revision/content 一帧。任何 snapshot/reconcile 测试都不得重试 settlement 或重放经济副作用。

## P3 — 暗器迁移与恰好九个随身容器 ⬜

| # | canonical template | grid | capacity | slot | cost | accept | freshness |
|---|---|---|---:|---|---:|---|---|
|1|worn_grass_pouch|3×3|8.0|chest|0.008|empty/all|Normal|
|2|grass_pouch|3×3|10.0|chest|0.005|empty/all|Normal|
|3|ore_sack|3×3|10.0|chest|0.005|mineral|Normal|
|4|projectile_bag|3×4|10.0|legs|0.005|anqi|Normal|
|5|sealed_vial|2×2|8.0|chest|0.008|pill, food|Halve|
|6|spirit_seal_box|2×2|10.0|chest|0.005|treasure, pill|Freeze|
|7|moisture_guard|3×3|8.0|legs|0.008|empty/all|SpoilOnly { rate: 0.3 }|
|8|coin_box|3×3|8.0|chest|0.008|bonecoin|Normal|
|9|sealed_envelope|1×2|8.0|head|0.008|recipe_fragment, recipe_hint, scroll|Normal|

P3 的随身 `ContainerSpec` allowlist 必须与上表 **exact set equality**，恰好九个。`worn_grass_pouch` 与 `grass_pouch` 是已上线的 canonical 通用背包，分别保留既有 `default.toml` 起手 chest worn、`pack_grass_pouch → pack_<instance_id>` runtime container、`basic.grass_pouch` recipe 和旧存档 template/owner/container ID；**不** rename、映射或迁移它们，只补显式 `accept=[]`（或等价 all）和 P2 所需 `Normal` freshness metadata，保证可继续收纳多类别内容。`herb_pouch`/`herb_crate` 保持现有 misc craft outputs，P3 不将其升级为 `ContainerSpec`、不删除物品或配方、也不作 legacy 映射；`herb_crate_placed` 仍是独立 placeable twin，连同 `trade_crate`/`dead_drop_box` 均不进入九件。12 暗器迁 Anqi，`projectile_bag` 只收 Anqi；coin_box 只承诺骨币分类收纳便利。测试必须以**结构化 TOML parse 的真实 `[item.container]`**全 registry canonical ID exact-set equality 锁九件，不得被注释文本误导；六个 nonempty filter（ore/projectile/vial/seal/coin/envelope）分别有 accept/reject，三件 all（两通用背包+moisture_guard）分别有任意已知 category 的多类别正例及 malformed/unknown 负例；覆盖默认 loadout、`basic.grass_pouch` recipe、旧存档 save/load 的 canonical template/owner/container ID 不变，并断言不存在 `herb_pouch`/`herb_crate` ContainerSpec。

## P4 — filter wire、client 预提示、e2e 与归档 ⬜

`ContainerSnapshotV1.accept_filter` required array/repeated，optional presence `acceptance_lock={reason: ContainerAcceptanceLockReason}`（canonical lower-snake `owner_missing`/`owner_not_found`/`owner_invalid`，与 `category` 同级 Rust/protobuf/TypeBox/Java/client 同步）；健康 absent+`[]` 才 all，present 时 exact `[]` inert；P4 从 P2 persisted reason 投影，不下发 tick。`InventoryItemViewV1.category` 由 P1A resolver 产 canonical lower-snake；Rust/protobuf/TypeBox/Java/client 同步，非法/unknown/missing/null/extra key 整份 snapshot fail-closed。Worn/Pack/Inspect 以 `ContainerFilterRules.accepts` 预提示，预测非法/locked 仍发 `InventoryMoveIntent`，server 权威。P4 不接 snapshot routing。bot/e2e 覆盖两通用背包仍收多类别、七专用容器过滤（含 ore_sack pass/herb reject、swap reverse、moisture_guard）、locked source/target、integrity revision、同 template 双 owner，并验证默认 loadout、`basic.grass_pouch` recipe、旧存档的 canonical template/owner/container ID 不变及无 `herb_pouch`/`herb_crate` ContainerSpec。

**饱和测试**：两种 filter tagged variant、required array/repeated、healthy absent+empty 与 locked present+exact empty、locked non-empty、缺/null/unknown/extra-key 的 Rust/protobuf/TypeBox/JsonFormat/client parser fail-closed；18 个 canonical category 和 synthetic mineral 正反；`ContainerAcceptanceLockReason` 三变体各自正反 round-trip、unknown reason 整份 snapshot fail-closed；Worn/Pack/Inspect 的 category/prefix/empty/footprint/lock `VALID`/`INVALID` 与仍发送 Intent；bot/e2e 对九容器逐一真实验证。

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
**决议**：P1A 交付 process-local prepared core；borrowed mutation/ordinary commit 永不跨 DB、不作 crash atomic。P1B 交付 `PreparedInventorySettlement`、`InventorySettlementId`、SQLite `mineral_sources`（legacy exhausted JSON 一次导入后退役）、mineral generation/sequence/recovery、`ForgeStationId`/SQLite `forge_stations`、`ForgeStationPlacementId` 原子扣 station item、escrow `ForgeAttemptId`、Prepared→Committed source/WAL 和单事务 inventory/skill/drop/source/station；station placement 先 durable commit 后投影，busy break 在默认 AIR 前 typed reject，Active row/Prepared attempt 支持 deferred projection 与 hydration。矿物 Prepared 无客户端也由 startup recovery 决定性 commit；commit 后安装同 snapshots ECS、再发 committed-ID best-effort wake；现有 mineral direct consumer/forge direct grant 只为待替基线；禁止 consumer-first pending、source-event 重发、runtime session key、generic durable commit 和端到端一次通知承诺。

**落点**：`server/src/mineral/anchors.rs`、`persistence.rs`、`respawn.rs`、`break_handler.rs:170-260`（legacy projection/source 起点）/ `server/src/mineral/inventory_grant.rs:38-138`（direct consumer）/ `server/src/forge/station.rs:26-165`（现有 ECS placement projection）/ `server/src/forge/mod.rs:182-384`、`server/src/forge/session.rs:81-222`（runtime attempt baseline）/ `server/src/world/block_break.rs:34-59`（AIR 前 forge/mineral gate）/ `server/src/persistence/mod.rs:1083-2351`（`mineral_sources`/`forge_stations` migration）/ `server/src/player/state.rs:1313-1375,2342-2367`（load/transaction helper）+ 本 plan「P1A」「P1B」。

### #3 P1 raw freshness，P2 transition
**决议**：P1A/P1B hook 原样携 freshness；P2 独占 transition、integrity、reconciliation 和 six-stage snapshot。P2 观察 committed writers，不 retry settlement/补发经济。

**落点**：`server/src/shelflife/container.rs:29-98` / `server/src/spiritwood/mod.rs:611-647` / `server/src/inventory/mod.rs` + 本 plan「P1A」「P1B」「P2」。

### #4 九容器固定
**决议**：P3 仅上表恰好九个 canonical ContainerSpec、12 暗器和骨币分类文案：既有通用 `worn_grass_pouch`/`grass_pouch` 原样占两席，仅补显式 all filter 与 Normal metadata，保持 default loadout/recipe/runtime pack/旧存档 ID；其余七席为新增专用件。`herb_pouch`/`herb_crate` 保持 misc craft outputs，不升级、删除或映射为 ContainerSpec；placeable 边界不变。

**落点**：`server/assets/items/core.toml:383-419`（两种既有通用背包与显式 all/Normal）/ `server/assets/inventory/loadouts/default.toml`（起手 worn）/ `server/src/craft/mod.rs:878-885`（`basic.grass_pouch`）/ `server/src/player/state.rs:1313-1420`（既有 canonical template/owner/container load invariants）/ `server/assets/items/workbench_materials.toml`（保持 misc 的 `herb_pouch`/`herb_crate`）/ TOML item registry structured parse test + 本 plan「P3」「范围边界」。

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
2. **PR-2 / P1B**：settlement ID、SQLite `mineral_sources`/legacy JSON 一次导入退役、generation/extraction/recovery/AIR gate、`ForgeStationId`/`forge_stations`、`ForgeStationPlacementId` 原子扣物、escrow `ForgeAttemptId`、WAL、single transaction、station placement/break/hydrator、ECS install/hydration/retry/committed-ID best-effort dedupe 和 crash matrix；不接 P2 snapshot routing，不扩展其它 placeable。
3. **PR-3 / P2**：freshness transition、integrity/reconciliation、六阶段 snapshot pipeline，覆盖 P1A/P1B writers，不重算 settlement。
4. **PR-4 / P3**：九容器 TOML 的 structured-parse registry exact-set gate、既有 `worn_grass_pouch`/`grass_pouch` 的 explicit-all/Normal metadata 与 canonical ID 不变回归、七新增专用件、保持 misc 的 `herb_pouch`/`herb_crate`、12 Anqi、coin 文案与 filter tests；不含 placeable 转换。
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
