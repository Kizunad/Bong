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
- **durable 保存面**：`durable_block_ownership`/`DurableBlockOwnershipIndex` 是 durable physical ownership 的唯一权威；`mineral_sources`/`MineralOreNode`/`MineralOreIndex`、`forge_stations`/`WeaponForgeStation`、`ForgeSession`/`ForgeSessions` 都只是由 manifest owner 约束的 source/station 或 ECS projection。复用 `player/state.rs::{load_player_inventory_from_sqlite,persist_player_inventory_json_in_transaction}`、`inventories`、`player_skills`、`DroppedLootRegistry`/`dropped_loot`。P1B 必增 transaction-local skill/source/drop/manifest helper，禁止拼接现有独立 save API。
- **出料**：P1A 普通路径只写 process-local live commit；P1B terminal 只 CAS immutable WAL 至 `CommittedPendingInstall`，projection worker 以 receipt 安装 WAL snapshot，ack 后才发 stable-ID committed event。P2 才统一 snapshot；P4 才 wire/client。
- **共享类型/event**：复用 `ItemCategory`、`InventoryLocationV1`、`InventoryMoveOutcome`、`ContainerFreshnessBehavior`、`SkillSet`、`DroppedLootRegistry`。P1A 是 borrowed `PreparedInventoryMutation<'a>`；P1B 固定为 owned serializable `PreparedInventorySettlement`、公共 `InventorySettlementId` 与 `InventorySettlementCommittedEvent`。Entity/tick/runtime `ForgeSessionId` 绝不可作 durable key。
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
3. `PreparedInventoryMutation<'a>` 为 private、non-Clone、`#[must_use]` 的借用 process-local **单 `PlayerInventory`** 载体：prepare→`commit_prepared_inventory_mutation(self, drops: &mut DroppedLootRegistry, outcomes: &mut EventWriter<InventoryMoveOutcome>) -> CommittedInventoryEffects` 独占 `&'a mut PlayerInventory`。prepare 只持 staged inventory、route、merge/push、`Vec<PreparedInventoryDrop>`、一次 outcome 与 debug `original_revision`；commit 的调用 handler 是唯一 effect owner，显式传入仍由其持有的 drop/outcome sinks，替换 inventory 后不可失败地安装 effects。载体不得存 resource/event、跨 handler/tick/DB 或并存，revision 恰 +1；无 CAS。
4. `PreparedInventoryMutation` **只**服务普通 process-local 单 inventory move/swap/非 durable ingress。它不是 crash atomic，也绝不神奇覆盖 multi-resource：`ExternalContainerPreparedMove`（player+`ExternalContainer`）、`PreparedPlayerTransfer`（source+target `PlayerInventory`+`DroppedLootRegistry`+`DropContext`）、`PreparedOwnerLossSpill`（owner inventory+drop registry+outcome）及 `PreparedBoneCoinTransaction`（inventory+bone-coin storage+ledger+drop+event）是各自显式命名、只在调用方同时独占全部参与资源期间构造并 commit 的 carrier。每种 carrier 列出 staged candidates、唯一 commit owner、每个 player revision +1 的规则；prepare 任何失败或 commit 前退出时所有参与资源深比较不变。P1B `PreparedInventorySettlement` 另行持久化，禁止调用上述任一 process-local commit。
5. 新增 `PreparedInventorySettlement`：owned、可序列化、无 live borrow、不得直接 commit/outcome；以同一 resolver/filter/fit/drop policy 产生 P1B WAL 所需 candidate `PlayerInventory`、candidate `SkillSet`、candidate drops/result/source/station snapshots，交给 P1B 唯一 durable commit API。不得让 borrowed mutation 跨 tick/SQLite。
6. P1A 的 `InventoryItemPrepareFn` 仅原样携带 freshness 字段；不得执行 Freeze/Normal 或任何 transition。ordinary move/swap、owner-loss spill/drop、durable settlement 同样遵守；P2 才替换/扩展 hook。
7. 普通 move/swap、四 automatic ingress 与非 durable `_or_ground` ingress 仅可使用 `PreparedInventoryMutation`。`ExternalContainer` 双资源 move 固定用 `ExternalContainerPreparedMove`；owner-loss（discard/morph/backpack break）固定用 `PreparedOwnerLossSpill` 做 detach→rebuild→spill→drop→outcome；death/TSY death、broken recovery、remains、treasure deactivate、trade、full transfer、bone-coin 按第 4 项各自的 explicit multi-resource carrier。每条路径的 prepare 失败均不得改变任一 live resource，成功 revision 按各自 carrier 的规则精确推进。
8. `transfer_all_inventory_contents` 固定用 `PreparedPlayerTransfer`，live owner 交易必 `ContainerOwnerTradeForbidden`；bone coin craft 固定用 `PreparedBoneCoinTransaction`，且其 ledger transfer 继续走 `qi_physics::ledger`。NPC/world 自产不是玩家 ingress。
9. 不新增、不迁移、不锁定、不验收 snapshot sender；不要求 EventWriter/outbox/final snapshot once；保持既有 request/resync 给 P2。

### 饱和测试

普通 `PreparedInventoryMutation` 的 new/partial/full merge、move/swap 双向 filter、self-containment、owner-loss/rebuild/spill/drop、每种非 durable ingress；`ExternalContainerPreparedMove`、`PreparedPlayerTransfer`、`PreparedOwnerLossSpill`、`PreparedBoneCoinTransaction` 分别做 compile/API pin，证明只能在持有其全部命名资源时 commit、effect sink 只能消费一次。每个 carrier 覆盖成功 revision 精确 +1 和每一 resolver/category/bounds/collision/weight/drop-context/ledger 前置失败的所有参与资源深比较不变；ordinary drop/outcome 恰一次。另 pin private non-Clone `#[must_use]`、活借用期间不能二借/改 revision/layout；freshness raw 字段完全不变；不测 transition、snapshot sender/outbox/final snapshot。

## P1B — mineral/forge durable source settlement ⬜

### Stable ID、durable mineral source/WAL 与 station 状态

1. 公共 `InventorySettlementId` 为 UUID/ULID；retry/reconnect/reload/notification 全复用。禁止 Entity、tick、runtime `ForgeSessionId`。
2. 新增 SQLite `mineral_sources` 为矿物唯一 durable **source** 真源；物理 durable ownership 仍唯一取 `durable_block_ownership`。复合 identity 固定为 dimension、`BlockPos`、canonical `mineral_id`、`generation_id`；row 至少持 `extraction_seq`、remaining、`state: Active|Exhausted|Respawning`、revision、nullable respawn deadline、schema_version 与 audit timestamps。初始 anchor/旧档迁移、每次 extraction terminal、exhaustion、respawn generation +1 必须在同一 durable `BEGIN IMMEDIATE` create/CAS 推进 `mineral_sources` 与对应 `DurableBlockOwnership` manifest（terminal remove/exhaust 时同事务更新/删除 manifest），commit 后才幂等投影 `MineralOreNode`/`MineralOreIndex`/block。settlement source unique key 必须引用该 row 的 generation + extraction_seq + revision；同坐标再生绝不串账。
3. `data/minerals/exhausted.json` 和 `ExhaustedMineralsLog` 只做一次性导入：导入时按 anchor 维度赋值、分配 generation 并落 `mineral_sources` 后立即退役；之后它们最多是从 SQLite 派生的 projection/删除路径，绝不能独立决定 spawn、exhaustion 或 respawn。坏/歧义 legacy entry fail-closed，不以空 log 猜测新 source。
4. `ForgeStationId { dimension: DimensionKind, pos: BlockPos, generation: u64 }` 是唯一锻炉 durable identity，**owner 不入 ID**。SQLite `forge_stations` 是真源，ECS `WeaponForgeStation` 与 ANVIL block 只为投影。row 至少持完整 ID、tier、nullable canonical owner player identity、integrity、revision、`state: Active|Removed`、nullable `active_attempt_id`、schema_version 与 audit timestamps。
5. `ForgeStationPlacementId` 必须稳定且可重试去重（由被消费的唯一 station item instance + target `ForgeStationId`/immutable request key 派生）。placement 在同一 `BEGIN IMMEDIATE` 以内 canonical player + expected inventory revision CAS 精确 station item instance（删除或递减）、持久 candidate inventory/revision、按 `(dimension,pos)` 分配 generation、写 `forge_stations Active`、对应 `DurableBlockOwnership { owner: ForgeStation { station_id } }` manifest 及 placement audit/idempotency row；commit 后才安装**同一** inventory snapshot 并投影 ECS/ANVIL。SQL/CAS 失败零扣物/零 station/零 manifest；commit/install 间 crash 从 DB 重装；重复 request 返回既有 placement outcome，绝不二扣。
6. 新增持久 `ForgeAttemptId`：inputs accept/lock 的同一 `BEGIN IMMEDIATE` 必须以 expected inventory revision CAS 精确 input instance IDs/stack slices 与数量，持久化扣除后的 candidate inventory、immutable escrow payload、完整 `ForgeStationId`、canonical player、expected station revision、Prepared attempt row，并 CAS station `(id, expected revision, active_attempt_id=None)` 为 `active_attempt_id=attempt` 和 revision +1。runtime `ForgeSessionId`/Entity 只作投影；terminal 只能消费 attempt escrow，禁止重读 live inventory。
7. durable source/settlement WAL row（实现为版本化 `inventory_settlements`）保存 settlement ID、canonical player identity、`candidate_inventory_json`、`candidate_skill_json`、`candidate_drop_rows`、immutable mineral source-after 或 forge attempt/station after-state、完整 item/no-item payload、XP delta、drop context、forge quality/color/effects/tier/consecration、全部 expected revision、`wal_schema_version`、payload SHA-256、结果/outcome 与 audit。状态**唯一固定**为 `Prepared`、`CommittedPendingInstall`、`CommittedStored`、`CommittedDroppedToGround`、`CommittedNoItem`、`QuarantinedCorrupt`；Perfect/Good/Flawed 只能走 `CommittedStored|CommittedDroppedToGround`，Waste/Explode 只能 `CommittedNoItem`；Explode 同时持久 station integrity/wear，五档都有唯一 terminal row。`Prepared` 插入同一 `BEGIN IMMEDIATE` 内写入所有 immutable candidate snapshots、expected revisions 和 reservations；之后 terminal/recovery **只能**反序列化该 WAL，不得重算经济或从 live ECS 补 candidate。`CommittedPendingInstall` 不是最终可写状态：它声明经济 SQLite 真源已提交但 projection 尚未由 receipt 确认，必须保留该 settlement 的全部 reservation。
8. `InventorySettlementReservation` 以 settlement ID 对 canonical player inventory/skill、mineral source 或 forge attempt、forge station 四类 key 建 durable unique reservation；在同一 Prepared transaction 建立。所有相关 inventory/skill/source/station writer 先经统一 `settlement_writer_gate` 查 reservation：被占资源只允许其 reservation ID 的 terminal/recovery writer 修改，普通 move/trade/death/autosave、mineral extraction、forge start/step/break 一律 typed `SettlementReserved` 拒绝，不得推进 revision。

### 唯一 durable commit API：WAL、source/station CAS 与恢复顺序

1. **线性化与 reservation**：先纯计算 candidate `InventorySettlementId` 和全部 immutable snapshots；再以 mineral source `(identity,generation,extraction_seq,revision)` 或 forge attempt key 进入一个 `BEGIN IMMEDIATE`。transaction 同时验证全部 expected revisions、写唯一 `Prepared` WAL、写全部 `InventorySettlementReservation`；重复 source unique conflict 必须查询并返回既有 settlement ID，绝不重生成经济。Prepared commit 即线性化点；其后普通 writers 均被 gate 拒绝，故不存在 terminal 前 revision stale。
2. **terminal→install→ack（冻结）**：唯一 terminal writer 在单 connection `BEGIN IMMEDIATE` 中读取并 SHA-256/schema 校验 `Prepared` WAL、**只** CAS `Prepared→CommittedPendingInstall` 并写 audit，**不得写 candidate/live projection、不得删除任何 reservation**，再 commit。只有 `CommittedPendingInstall` 可被 projection worker 消费；worker 仅反序列化 immutable WAL，以 `(settlement_id, wal_schema_version, payload_sha256)` 建 `InventorySettlementInstallReceipt`，`INSERT … ON CONFLICT DO NOTHING` claim 为 `Installing` 后安装 WAL 中的 `PlayerInventory`、`SkillSet`、drop、source/station/manifest、index 与 ECS。receipt 对每个 target 写固定 revision/hash；已安装且同 hash 为 no-op，不同 hash 进 `QuarantinedCorrupt`，绝不覆盖。ground drop 的 stable `drop_id=hash(settlement_id,payload_ordinal)` 先写 receipt，重放先查 `DroppedLootRegistry`/`dropped_loot`，已存在绝不 spawn 第二 entity。receipt 全目标 `Installed` 后，唯一 acknowledgement `BEGIN IMMEDIATE` 复验 WAL+receipt hash，CAS `CommittedPendingInstall→CommittedStored|CommittedDroppedToGround|CommittedNoItem`、标 `Acknowledged`，并在**同一 transaction**删除该 ID 全部 reservations exactly once；仅此后发 `InventorySettlementCommittedEvent`。
3. **崩溃恢复（冻结）**：terminal 前=`Prepared`+reservations，重做 terminal CAS；terminal 后/install 前=`CommittedPendingInstall`+无 receipt，recovery claim/install；安装中=`CommittedPendingInstall`+partial `Installing` receipt，按 target hash/revision 与 stable `drop_id` 补齐；install 后/ack 前=完整 receipt，只 ack；ack 后=最终 `Committed*`+`Acknowledged` receipt+无 reservation，只读且不安装/不 drop。receipt/hash/schema/target 不一致唯一去 `QuarantinedCorrupt` 并保 reservation，禁止模糊“幂等”。所有 normal inventory/skill/source/station writers——move、trade、death、autosave、disconnect/shutdown persistence、mineral extraction、forge start/step/break——在上述所有 pending 间隙仍先过 `settlement_writer_gate` 并返回 `SettlementReserved`。
4. **escrow cancel/failure**：Prepared attempt escrow 始终归该 attempt；restart 只重建 attempt session，普通 move/trade/death/disconnect 不得移动已 escrow 材料。若产品允许取消，typed cancel 必须先生成完整 immutable settlement candidate，再走第 1–3 项相同 WAL/reservation/terminal/install/ack 流程返还 candidate inventory 或 durable drop；其它失败不改变 Prepared。
5. **rollback 边界**：Prepared transaction 在写 WAL/reservation 前任一 SQL/serialize/revision 校验失败则 rollback，DB/ECS/source/drop/XP/outcome 均不变；terminal 失败保留 `Prepared`+reservation；install 失败保留 `CommittedPendingInstall`+reservation+receipt evidence；ack 失败保留完整 receipt+reservation，绝不回滚已安装 projection。普通 writer 绝不以 live ECS 覆写 reservation 数据库。
6. **统一恢复与调度**：`durable_block_ownership` migration 后，`durable_block_ownership_hydrate_startup` 必须先加载 `DurableBlockOwnershipIndex`，再由 `InventorySettlementRecovery` 扫描 mineral 与 forge `inventory_settlements` 的 `Prepared|CommittedPendingInstall`；forge attempt escrow hydration 与 settlement recovery 严格分流。真实 `main.rs::build_server_app` 的 `inventory::register`、`mineral::register`、`forge::register`、`world::register` 共同接线：Startup manifest hydration 严格 `.after(crate::persistence::PersistenceBootstrapSet).after(crate::world::setup_world).before(InventorySettlementRecovery)`，并早于 recovery、玩家 hydration 和所有 block-break writer；首个 Update 仍由 `InventorySettlementSet::Recovery` 兜底。`world::register` 固定 `BlockBreakSet::{Authorize,Ordinary,DurablePrepare,DurableTerminal,Projection}`，以 Recovery → Authorize → Ordinary → DurablePrepare → DurableTerminal → Projection 链保证可观测顺序。矿物/forge recovery 只按 WAL+receipt 安装 source/index/ECS/block 或产物/skill/drop/station，不重算或重跑 runtime session；chunk 未加载则保留 pending receipt，延期同一 hash 的 projection。
7. **station lifecycle**：`ForgeStationId` 的 `Active|Removed` durable row 是生命周期真源；`active_attempt_id` 或任一 `InventorySettlementReservation` 存在时，专用 forge break 只返回 typed busy/reserved reject，不得写方块。仅空闲 station 的 `Removed` terminal transaction 可以删除其 durable row投影。startup 从 Active rows重建 durable index/ECS；Prepared attempt 从 escrow hydration，terminal settlement 只能由 `InventorySettlementRecovery` 消费 WAL，二者不得相互替代。
8. **canonical durable ownership manifest（冻结，唯一物理 owner 真源）**：新增 SQLite `durable_block_ownership`，其 row 类型固定为 `DurableBlockOwnership { key: DurableBlockKey { dimension: DimensionKind, pos: BlockPos }, owner: DurableBlockOwnerKind, revision, schema_version, audit timestamps }`；`DurableBlockOwnerKind` 固定两变体：`MineralSource { source_id, generation_id }` 与 `ForgeStation { station_id: ForgeStationId }`。`DurableBlockOwnershipIndex` 是该表的内存镜像，key 必须严格为 dimension+`BlockPos`，不得由 `MineralOreIndex`、`WeaponForgeStation`、ECS Entity、block state 或任何 source/station projection 反推/补写 owner。普通 terrain/worldgen 的 `IRON_ORE` 没有 manifest；durable mineral anchor/respawn 与 forge placement 均在同一 `BEGIN IMMEDIATE` 先写/更新 manifest 和 source/station row，commit 后才投影 block、`MineralOreIndex` 或 ECS。mineral exhaustion/remove、forge Removed terminal 在同一 transaction 更新或删除 manifest/source/station，commit 后才投影 AIR。`persistence::register` migration 后，`durable_block_ownership_hydrate_startup` 必须加载 `DurableBlockOwnershipIndex`；它在真实 `main.rs::build_server_app` 经 `inventory::register`、`mineral::register`、`forge::register`、`world::register` 接线，严格 `.after(crate::persistence::PersistenceBootstrapSet).after(crate::world::setup_world).before(InventorySettlementRecovery)`，并早于玩家 hydration 与全部 break writer。manifest presence 是唯一分类：miss=`Ordinary`（即使 raw state 是 `IRON_ORE`/ANVIL），hit 只按 owner kind 分派；hit 后 block state 仅用于校验该 owner 的 expected projection。manifest hit 但 source/index/ECS/block 任一 projection 缺失或不一致，发 `DurableBlockIntegrityFault`，不降级 ordinary。

   **诚实边界**：manifest 尚存时，任何 secondary projection loss（包括 source/index/ECS/block 全丢）必须可检测为 fault；但若外部同时删除/损坏 canonical manifest 与全部 durable secondary metadata，raw block state 在信息论上无法区分 ordinary `IRON_ORE` 与 FanTie，系统不得声称能由 `DiggingEvent` 猜出 durable owner。manifest 自身损坏由 migration invariant/integrity sweep/backup 或 typed manual quarantine 处理，不允许 classifier 猜测、修复或误伤普通方块。
9. **单一 raw、phase-aware dispatcher（冻结）**：仅 `world::block_break::authorize_block_break_intents` 可在 production `server/src` 声明 `EventReader<DiggingEvent>`。它为每条 raw dig 读取当前 canonical player、`CurrentDimension`、`VisibleChunkLayer`，在 Start 建立不可变 `BreakAttemptSnapshot { attempt_id, player, player_identity, dimension, position, original_block_state（完整 properties）, direction/face, digging_state, game_mode, tool_context, permission_distance_context, received_tick }`；同一玩家+dimension+position 的 Start 以单调 `epoch` 和 `sequence` 分配 `BreakAttemptId`，重复 Start 结束旧 attempt 为 `Superseded` 后新建；Stop/Abort 只匹配当前 open attempt，未匹配 terminal 生成独立 `orphan epoch` attempt（不得借用 live state），Creative Start 既是 Start 又是立即 terminal，同一 ID。dispatcher 对所有 Start/Stop/Abort 都发 `AuthorizedDiggingPhase { snapshot, phase: Start|Abort|Terminal }` 或 typed reject/fault，按 `DurableBlockOwnershipIndex` 分类 `Ordinary|Mineral|Forge`；权限、距离、原始 block snapshot 与 manifest/projection 校验**全部只在 dispatcher**完成。terminal 仅当 snapshot 的 original state 仍满足授权：ordinary 使用 expected-original-state CAS，否则 `StaleAuthorizedBreak`、零 AIR/零掉落；durable mismatch 为 integrity fault、零经济。不得以任一 reader 是否先 `read()`、`.before/.after` 或 live chunk 重读作为授权、掉落或 AIR gate。
10. **消费者相位矩阵与唯一 owner（冻结）**：`apply_default_block_break` 只消费 `AuthorizedDiggingPhase::Terminal(Ordinary)`，对 snapshot original state CAS 为 AIR 并删 `FurnitureRegistry`；`apply_block_drops` 只消费同一 terminal snapshot，以 snapshot `original_block_state`、`tool_context`、player/attempt ID 与 deterministic receipt 计算普通掉落，**绝不读取 live `ChunkLayer`**，同 attempt receipt 已存在则零掉落。mineral：Survival Start 建 gathering session、Abort 取消、Terminal 仅进入 durable settlement；Creative Start/Terminal cleanup 也为同一 attempt 且零物品。spiritwood：只消费 Start 建 session（Creative reject）。social spirit niche：只消费 Start 与 Terminal 产生 reveal/intrusion，Abort 无副作用。`world/container_block.rs` 与 `craft/workbench.rs` 各自只按其当前生产语义消费 dispatcher 的相位 envelope：container 的 terminal 才拆/落物，workbench 的 Start 选中/打开、Abort 取消、Terminal 完成/拆除；不得重新读 raw。forge：Start 做 station/permission busy preflight，Abort 取消未 terminal 的交互，Terminal 仅走 reserved/active reject 或 Removed durable settlement。所有 nonterminal 相位禁止物品、XP、source/station 终结经济；ordinary/mineral/forge terminal receipt 是其唯一经济 owner。
11. **真实 schedule、静态 pin 与对抗测试**：`world::block_break::BlockBreakSet::{Authorize,Ordinary,DurablePrepare,DurableTerminal,Projection}` 由 `world::register` 在 `Update` 以 `configure_sets(Update, (Authorize, Ordinary, DurablePrepare, DurableTerminal, Projection).chain())` 注册，并整体置于 `InventorySettlementSet::Recovery` 之后；顺序固定 Recovery → Authorize → Ordinary → DurablePrepare → DurableTerminal → Projection。静态/compile pin 断言 production `server/src` 除 dispatcher 外不存在 `EventReader<DiggingEvent>`。真实 `main::build_server_app` SQLite/World/ChunkLayer/client 对抗测试须在 terminal→install、install 中、install→ack 三间隙插入 move/death/autosave writer，断言 reservation 留存、`SettlementReserved`、无 lost update；并覆盖 crash 四点与 receipt ground-drop 不重复。dig matrix 必须覆盖 mineral Start→Abort、Start→Stop、重复 Start/Stop/Abort、无 Start terminal、Creative、spiritwood Start、social Start/Terminal、ordinary duplicate terminal、原状态变更 stale CAS，逐例断言现有玩法不丢、nonterminal 无终结经济、terminal receipt 不重复。ordinary/durable consumer 注册顺序扰动后所有 block/inventory/drop/allocator/WAL/manifest/source/station/index/ECS 结果相同。


### 生产切流与饱和测试

`mineral/break_handler.rs` 只消费 `AuthorizedDiggingPhase`：Survival Start 建 session、Abort 取消、Terminal 才由 immutable source snapshot 写 Prepared→CommittedPendingInstall；`apply_default_block_break` 与 `apply_block_drops` 只消费同一 ordinary Terminal snapshot，前者 expected-original-state CAS AIR，后者只用 snapshot block/tool context 和 attempt receipt，不得以 `MineralOreIndex` miss 或 live `ChunkLayer` 作为普通授权。projection worker **只**消费 `CommittedPendingInstall` 的 WAL+receipt；矿物 `remaining > 0` 的 receipt 保块并同步 source/manifest/index/ECS，只有 Exhausted receipt 才 AIR、移 index、despawn；ack 后所有 projection 只读。任一 transaction/CAS/serialize/recovery/quarantine/rollback 失败均保留 block，且 DB/manifest/index/ECS 不变或按同一 receipt 重放；`MineralDropEvent` 最多为含 ID wake/notification，不能作为恢复输入。旧 exhausted JSON 一次性导入/退役后断开 anchor/respawn authority。`InventorySettlementRecovery` 在 normal writers 前完成无客户端 Prepared 或 CommittedPendingInstall。forge station placement 走 `ForgeStationPlacementId` 的 inventory+station+manifest 单事务后才投影；专用 forge break 消费 phase envelope，Start preflight、Abort 取消、Terminal 才处理 reserved/active reject 或 Removed settlement。inputs lock 把精确材料转入 `ForgeAttemptId` escrow；canonical XP pure function 仅针对 escrow 产 candidate `SkillSet`，五档终局只存在于 WAL；禁止先 `SkillXpGain` 再 `ForgeOutcomeEvent`。inventory bridge/S2C/Redis/audio/source readers 改读 ack 后 committed-ID wake 并去重，P1B 不接 P2 snapshot routing，也不扩展其它 placeable。

测试必须覆盖：`mineral_sources` 初始 anchor/legacy JSON 一次导入、维度/generation/extraction_seq/revision、同坐标再生、每次 extraction/exhaustion/respawn CAS、JSON 退役后不能决定 spawn；ordinary terrain `IRON_ORE` 与 manifest durable FanTie 的相同 `BlockState::IRON_ORE` 必须分流正确；manifest hit 的 partial projection loss、source/index/ECS/block 全部 secondary loss 均 fault，manifest missing 则明确 ordinary（不得声称 raw state 自动发现损坏 manifest）。矿物及 forge 五档各自覆盖 Prepared 后立即崩溃、terminal 后 install 前、install 中、install 后 ack 前、ack 后崩溃、玩家永久离线、registry/code `wal_schema_version` 变化；recovery 只从 WAL+receipt 得出唯一 terminal/install/ack，不重算 item/quality/XP/source/station 经济。每个 gap 都以真实 `main::build_server_app` 注入 move/death/autosave writer，断言 reservation 保留、所有 inventory/skill/source/station normal writer 得 `SettlementReserved`、revision 不动且无 lost update；ack 后 reservation 恰清一次、receipt/drop ID 重放零二次实体。payload hash/schema/receipt target 破坏只到 `QuarantinedCorrupt` 且不释放给普通 writer。矿物必须覆盖 phase matrix 的 Start→Abort、Start→Stop、duplicate Start/Stop/Abort、无 Start Terminal、Creative Start/Terminal，SQL/CAS/rollback/recovery/quarantine failure 均保块，WAL/DB/manifest/source/index/node ECS 不变或按 receipt 重放；每次 non-exhausted terminal 保块，只有 Exhausted receipt 才 AIR+移 index+despawn。station 覆盖同址 generation、placement item 精确扣除/重复 placement 幂等/SQL 失败零扣物零 manifest 零投影/commit-install crash reload、reserved 与 `active_attempt_id` busy break 均 typed reject 并保 ANVIL、空闲 break 仅 Removed receipt 后恰一次 AIR+删 ECS/index、chunk unloaded deferred receipt projection；forge input exact instance/slice escrow、duplicate Start、lock 前后 crash、重连/死亡/交易/断线不能移动 escrow、Prepared attempt hydration 与 settlement recovery 分流、typed cancel 同一 WAL 流程、五档最终 ack 后释放 active lock、Perfect/Good/Flawed Stored/Dropped、Waste/Explode NoItem、Explode station wear exactly once、非 Explode 不改 integrity；满包不可drop、首次无 inventory row、duplicate source/wake/retry/reconnect/reload；每个 SQL write 失败、autosave/disconnect/shutdown 不回退；经济物/XP/source/drop/outcome row 恰一次。spiritwood Start 必建且仅建一次 session；social Start+Terminal 必发既有 reveal/intrusion、Abort 不发；ordinary duplicate terminal 与 stale original-state CAS 均零重复掉落/零错误 AIR。

**强制真实 App block-authority 集成矩阵**：不得只调用 handler helper。每例均以 `main::build_server_app` 建最小 SQLite/World/ChunkLayer/client，发送真实 `DiggingEvent` 并运行完整 Update，且同时断言 `ChunkLayer` block、player inventory、drop event、`InventoryInstanceIdAllocator`、SQLite WAL/receipt/manifest、durable projection index、相关 `MineralOreNode`/`WeaponForgeStation` ECS：① ordinary terrain `IRON_ORE` 的 Terminal snapshot 只掉普通物恰一次；② manifest durable FanTie 同 state 只走 mineral phase；③ reserved forge 的 Terminal 维持 ANVIL、station row/revision/active lock、manifest 与 ECS/index 不变且只产 typed reject；④ mineral transaction failure 维持矿块，source/WAL/receipt/manifest/index/node 均不变；⑤ mineral 非 Exhausted receipt 保块，source remaining/revision 与 manifest/node/index 同步；⑥ mineral Exhausted receipt 才 AIR，source Exhausted、manifest 终态、index 移除、node despawn、最终 ack WAL 一致；⑦ 成功 forge remove 只在 Removed receipt 后 AIR，manifest/station ECS/index 移除且 repeated terminal 不二写；⑧ manifest hit 的 source/index/ECS/block mismatch 均保留原块、发 `DurableBlockIntegrityFault`，并且 block/drop/inventory/allocator/DB/WAL/receipt/manifest/index/ECS 均零副作用；⑨ ordinary snapshot 原始 state 在 terminal 前变更为 stale，零 AIR/零掉落。另以 ordinary/durable consumer 注册顺序扰动复跑①–⑨，结果必须相同，证明安全性来自 dispatcher+manifest+snapshot/receipt，而不是 `EventReader` 读取顺序；并加 static/compile pin 保证 production `server/src` 仅 dispatcher 声明 `EventReader<DiggingEvent>`。

以真实 `main::build_server_app` 的最小 App/SQLite 集成测试断言 `durable_block_ownership` migration 先于 `durable_block_ownership_hydrate_startup`，后者先于 `InventorySettlementRecovery`、玩家 hydration 与 dispatcher；删除或反转任一 `.after/.before(...)` 依赖必须使顺序断言失败。Update Recovery 仍早于 Authorize、ordinary/durable prepare、terminal 与 projection；漏 `configure_sets`/`add_systems` 或顺序反转必须红。P1A 未 merge不得开 P1B；P1B 完整 crash matrix、manifest/dispatcher block-authority matrix 与注册顺序 gate 未过不得开 P2。

## P2 — freshness、integrity、snapshot 唯一调度 ⬜

P2 独占 `apply_container_freshness_transition`、`sealed_vial=Halve`、`spirit_seal_box=Freeze`、`moisture_guard=SpoilOnly { rate: 0.3 }` 的 4×4 mapping；P1A/P1B 全部 writer 以 post-transition identity merge。持久 `integrity_lock(reason,detected_tick)`，唯一 `reconcile_container_integrity_freshness` 管 owner corruption。固定 `InventoryReconciliationSet::{Commit,SnapshotRequestProducer,Reconcile,Sweep,CollectSnapshots,EmitSnapshots}`：所有 P1A/P1B business/reconciliation writer 只 request；唯一 reader/outbox/emitter 每 entity/tick 最终一帧。P2 不 retry settlement、不补发 XP/物品、不把 settlement retry 混入 reconciliation。P2 固定在 `inventory::register`（`server/src/inventory/mod.rs:854`）以 `app.configure_sets(Update, (...).chain())` 注册这六个 set，并把 `reconcile_container_integrity_freshness`、唯一 request reader、outbox collector、emitter 分别 `in_set` 到对应阶段；在 `main::build_server_app`（`server/src/main.rs:71`）真实建 App 的 integration test 运行 Update，断言六 set 的 reader/collector/emitter 都已加载且顺序可观察，不能以静态数量计数替代运行接线。

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

`ContainerSnapshotV1.accept_filter` 固定为带 presence 的 `optional AcceptFilterV1 accept_filter`：`AcceptFilterV1 { repeated ItemCategoryV1 categories = 1; }`。wrapper 缺失为协议错误，整份 snapshot fail-closed；wrapper present 且 `categories=[]` 表示 all；present 且 nonempty 按 category OR。`acceptance_lock` 保持 `optional`，healthy 时缺省，present 时为 `{reason: ContainerAcceptanceLockReason}`（canonical lower-snake `owner_missing`/`owner_not_found`/`owner_invalid`，与 category 同级 Rust/protobuf/TypeBox/Java/client 同步）；locked 仍携 present `accept_filter`，不把 lock 当 filter presence。`InventoryItemViewV1.category` 由 P1A resolver 产 canonical lower-snake。JSON/TypeBox/Rust serde 与 Java parser 必须拒绝 null 和 extra key；protobuf binary 不承诺拒绝普通 unknown field（proto 前向兼容），仅 unknown enum、非法 tagged variant、缺 `accept_filter` wrapper、或语义不合法的 present wrapper fail-closed。Rust `server/src/schema/inventory.rs::{ContainerSnapshotV1,InventoryItemViewV1}`、`proto/bong/envelope.proto::{ContainerSnapshot,AcceptFilterV1,ItemCategoryV1}`、`agent/packages/schema/src/inventory.ts::{ContainerSnapshotV1,AcceptFilterV1,ItemCategoryV1}`、`client/.../InventorySnapshotHandler.java::parseContainers` 同名同步。Worn/Pack/Inspect 以 `ContainerFilterRules.accepts` 预提示，预测非法/locked 仍发 `InventoryMoveIntent`，server 权威。P4 不接 snapshot routing。

**canonical `ItemCategory` exact set**：当前权威 `server/src/inventory/mod.rs:355-381` 的 `Pill`、`Herb`、`RecipeFragment`、`RecipeHint`、`Weapon`、`Armor`、`Treasure`、`BoneCoin`、`Tool`、`Scroll`、`Misc`、`Block`、`Mineral`、`Anqi`、`Liquid`、`Container`、`Food`、`Shield`，共 18 个；测试从该 exact set 穷举导出 18，禁止裸魔法数。synthetic Mineral 不是第 19 类：仅 P1A `resolve_item_category` 的 canonical mineral-id/registry/template 三重命中产生 `Mineral`，其 spoof/unknown/mismatch 为 typed invalid。bot/e2e 覆盖两通用背包仍收多类别、七专用容器过滤（含 ore_sack pass/herb reject、swap reverse、moisture_guard）、locked source/target、integrity revision、同 template 双 owner，并验证默认 loadout、`basic.grass_pouch` recipe、旧存档的 canonical template/owner/container ID 不变及无 `herb_pouch`/`herb_crate` ContainerSpec。

**饱和测试**：`AcceptFilterV1` raw protobuf wire pin 分别覆盖 wrapper 缺失（fail-closed）、wrapper present+empty（all）、present+nonempty（OR）、unknown enum/非法 tagged variant（fail-closed）及普通 unknown binary field（protobuf 兼容忽略，不作为拒绝承诺）；JSON/TypeBox/Rust serde/Java parser 分别覆盖 missing/null/extra-key reject。穷举上文 exact `ItemCategory` set 的 18 项及 synthetic Mineral 正反；`ContainerAcceptanceLockReason` 三变体各自正反 round-trip、unknown reason 整份 snapshot fail-closed；Worn/Pack/Inspect 的 category/prefix/empty/footprint/lock `VALID`/`INVALID` 与仍发送 Intent；bot/e2e 对九容器逐一真实验证。

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
**决议**：P1A 交付 process-local prepared core；borrowed mutation/ordinary commit 永不跨 DB、不作 crash atomic。P1B 交付 `PreparedInventorySettlement`、`InventorySettlementId`、`InventorySettlementInstallReceipt`、SQLite `mineral_sources`（legacy exhausted JSON 一次导入后退役）、`durable_block_ownership`/`DurableBlockOwnershipIndex`（唯一物理 durable ownership）、mineral generation/sequence/recovery、`ForgeStationId`/SQLite `forge_stations`、`ForgeStationPlacementId` 原子扣 station item、escrow `ForgeAttemptId`。唯一状态机为 Prepared→CommittedPendingInstall→ack 后 CommittedStored/CommittedDroppedToGround/CommittedNoItem：terminal 不删 reservation，receipt 对 immutable WAL snapshots 的 player/skill/drop/source/station/manifest/index/ECS 逐目标 hash 安装；stable ground `drop_id` 防重 spawn，ack transaction 才删 reservation exactly once。固定仅 `authorize_block_break_intents(DiggingEvent)` 读取 raw dig，发携 immutable original `BlockState`/dimension/pos/direction/game mode/tool context 的 `AuthorizedDiggingPhase`；Start/Abort/Terminal 以 epoch/sequence `BreakAttemptId` 关联，重复 Start supersede、无 Start terminal orphan、Creative Start 即 terminal。manifest miss 一律 Ordinary，即使 state 为 `IRON_ORE`/ANVIL，hit 后才校验 source/index/ECS/block projection。ordinary Terminal default AIR/drop 只消费同一 snapshot（AIR expected-state CAS，drop 不读 live layer）；mineral Start/Abort/Terminal、forge Start/Abort/Terminal 及 spiritwood/social/container/workbench 均按 P1B phase matrix 消费 envelope。manifest 尚存时任一 secondary projection loss 是 typed integrity fault；manifest 与全部 secondary metadata 同时丢失时 raw state 无法区分 ordinary，交 migration/integrity sweep/backup/manual quarantine，禁止 DiggingEvent 猜测。所有 pending gap 的 inventory/skill/source/station normal writer 经 gate 拒绝；现有 mineral direct consumer/forge direct grant 只为待替基线；禁止 consumer-first pending、source-event 重发、runtime session key、generic durable commit 和端到端一次通知承诺。

**落点**：`server/src/mineral/anchors.rs`、`persistence.rs`、`respawn.rs`、`break_handler.rs:169-275`（迁为 `AuthorizedDiggingPhase` 的 Start/Abort/Terminal consumer）/ `server/src/mineral/inventory_grant.rs:38-138`（direct consumer）/ `server/src/forge/station.rs:26-165`（现有 ECS placement projection）/ `server/src/forge/mod.rs:182-384`、`server/src/forge/session.rs:81-222`（runtime attempt baseline）/ `server/src/world/block_break.rs:33-58`（替为 `authorize_block_break_intents`、attempt snapshot 与 phase envelope）/ `server/src/world/block_drop.rs:160-277`（替为 Terminal snapshot consumer，移除 index-miss/live-layer authority）/ `server/src/world/container_block.rs:186`、`server/src/craft/workbench.rs:165`、`server/src/spiritwood/mod.rs:83-166`、`server/src/social/mod.rs:2130-2175`（各自 phase matrix consumer）/ `server/src/persistence/mod.rs:1083-2351`（`durable_block_ownership`、`mineral_sources`/`forge_stations`、settlement receipt migration）/ `server/src/player/state.rs:1313-1375,2342-2367`（load/transaction helper）+ 本 plan「P1A」「P1B」。

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
