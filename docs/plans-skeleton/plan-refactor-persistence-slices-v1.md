# plan-refactor-persistence-slices-v1 — 玩家/世界状态持久化 Slice 框架 + persistence 巨石拆分（重构轨 R3）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：拆掉 16324 行的 `persistence/mod.rs` 巨石，建统一的持久化 Slice 框架——载入失败守护（绝不空表覆盖）、关服强制 flush registry、相对 tick 基准、autosave 竞态互斥——把"重启丢档/断线丢档/载入清零"整簇（20+ 份 plan）从根上消灭。

## 现状证据（2026-07-27 侦察）

- `persistence/mod.rs` 单文件 16324 行、53 处 `CREATE TABLE`、v1-v39 手写迁移链；`register(app)` 只挂 5 个系统，玩家数据另走 `player/mod.rs` 的分片 autosave（4 个 `autosave_player_*`）。
- 覆盖参差：craft/mineral/spiritwood/zone/npc 有表；alchemy/forge/gathering/lingtian session、ActiveEventsResource、TiandaoAttention、状态 buff、化虚冷却等纯内存重启即丢。
- 已确认的同构缺陷族（在飞 PR 群）：#1288 KnownTechniques 载入失败空表覆盖丢档、#1289 Lifecycle 从未持久化（重连清空濒死后果）、#1282 Wounds 重连满血、#1290 呼吁推广载入守护到所有 slice——这是"每个 slice 手写一遍、各漏各的"的直接证据。
- 绝对 tick 持久化导致重启漂移：mineral-respawn-tick-restart-drift、voidaction-cooldown-runtime-tick-restart 同构。
- 关服 flush 缺口：recipe-unlock（#1261 在修）、spiritwood、zone-influence 同构。

## 接入面

- **进料**：SQLite（bong.db，沿用）、`shutdown.rs`（#1261 之后的关服链路）、`CultivationClock`（相对 tick 基准）。
- **出料**：统一 Slice API 供各域注册：`load(guarded) / autosave(cadence) / flush_on_shutdown / tick_rebase`；R1 的 session 持久化钩子、各域运行态表全部走它。`tick_rebase` 对 R1 suspension lease 与 outbox 的 `next_retry_tick`/`created_at_tick`/`lease_until` 保持相对剩余时长/已消耗 age；R1 冻结默认 `SESSION_SUSPENSION_TTL_TICKS = 1_728_000` 与扫描 cadence `1_200` ticks，本轨不得把重启折算成无限续租或刷新 outbox 退避/7 天 age。为 terminal delivery 提供 `SessionDeliveryOutbox`：事务内只提交 outbox 插入与 session checkpoint terminalization/删除，提交成功后 runtime gameplay claim 释放必须可依据 durable terminal/outbox 状态幂等重放；禁止仅持久化 session、把 delivery id 留在内存。每个 checkpointed session 的 reservation 必须遵守 P1 addendum 的 `reserve_new_terminal_obligation` / `reuse_terminal_obligation` / `cancel_unconsumed_reservation` 三分协议；只有首次 admission 可在取得 claim 前以同一 `BEGIN IMMEDIATE` 事务 conditional-update 单例 `SessionDeliveryQuota { used_rows, used_bytes, generation }`（仅当 row/bytes 上限均有余量）并插入唯一 `SessionDeliveryReservation { session_key, reserved_bytes = SESSION_DELIVERY_MAX_PAYLOAD_BYTES }`，任一步失败整笔回滚。restore/retry 只能验证并复用既有 reservation，不能 insert 或增加 counter。等价非 SQLite 实现必须以 quota generation CAS 提供同一线性化点，禁止先读剩余额度再另事务插 reservation。terminal handoff 在 checkpoint/outbox 同事务把 reservation 转移给 outbox，不重复计量；只有 receipt `Committed` 或带完整 payload 的 audited `ResolvedDisposition` 在同一事务删除 obligation 并扣减 counter，CAS loser 不得释放 quota。payload 增长不得超过 reservation，上限不足必须在接受新 escrow/产物前 fail closed。dead-letter 从自动扫描集合移除且不占 claim，但继续占 quota。`TsyPresence` 与 player position/dimension 作为同一 coupled snapshot：routine autosave、disconnect save、shutdown flush 均须共用同一事务/版本边界，不能由独立 Slice 提交不同快照。
- **共享类型**：新 `server/src/persistence/` 多文件模块（按域拆表定义 + 迁移链保持线性单入口）；`PlayerSliceRegistry`（对齐 #1290 skeleton 的方向，直接吸收它）。
- **跨仓库契约**：零 wire 改动。
- **qi_physics 锚点**：任何带 qi 的快照持久化/恢复不得造成账面变化；恢复失败的兜底路径必须走 `release_dormant_qi_to_zone` 而非丢弃（对齐守恒律红旗清单）。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：53 张表普查归域；冻结 Slice trait（载入守护语义：读失败 = 保留旧行 + 告警 + 只读降级，绝不写回空态；flush registry；tick rebase 协议）；等 #1288/#1289/#1261/#1259 merge 后定基线。
- ⬜ P1 框架落地 + 巨石拆分：`persistence/` 按域拆文件（迁移链不变、行为不变）；Slice 框架上线，KnownTechniques/Lifecycle（在飞 PR 的成果）平移为首批宿主；冻结 inventory slice hydration seam 及 `MigrationOutcome` consumer 边界，R3 不复制 inventory 网格规则，待 R10 P1 提供纯幂等迁移函数后接入，不得提前引用 R10 常量/实现。注册 R1 所需的 `TsyPresence` auxiliary Slice，字段固定为 `family_id`、`entered_at_tick`、`entry_inventory_snapshot`、`return_to`、schema/version，并接入 guarded load、**与 player position/dimension 共用同一 snapshot/version 的原子 autosave**、`flush_on_shutdown`、`tick_rebase`；任何 autosave、disconnect save 或 shutdown flush 都不得让三者跨快照提交，crash 后只能全旧或全新。落 `SessionDeliveryOutbox`、`SessionDeliveryQuota`、`SessionDeliveryReservation` 表与 atomic handoff/reservation API，stable `delivery_id`、完整 payload、cause、attempts/next_retry/created_at/tick_epoch/state、单调 generation 与 in-flight lease 全部持久化。P1 acceptance 必须同时锁定：TsyPresence/position/dimension 在 routine autosave、disconnect save、shutdown flush 的每个逻辑写边界均由 fault-injection harness 强杀并重启，结果只能全旧或全新；reservation 的 quota conditional update + unique insert 原子性；terminal handoff 时 reservation→outbox 不重复计量；Committed/ResolvedDisposition 扣减 quota 与 obligation 删除原子性；checkpoint terminalization/deletion 与 outbox insert 同一 SQLite transaction 全成或全败；以及 outbox insert 前、terminalization 前、commit 后 ack 前的 crash-injection pins。P3 只复用 coupled-snapshot harness 做 flush registry 长尾回归并处理其余 flush/tick-rebase 批次，不得把首次原子性证据延后。
- **P1 reservation protocol addendum（覆盖接入面旧 `reserve_terminal_obligation` 合称）**：实现 R1 §2.2.2 的 `reserve_new_terminal_obligation`（仅首次 admission，`+Q`）、`reuse_terminal_obligation`（matching restore/retry，`ΔQ=0`）和 `cancel_unconsumed_reservation`（busy loser/claim 后校验失败/无 checkpoint 残留，恰一次 `-Q`）。restore missing/conflicting owner/bytes/generation fail closed，禁止 insert/+Q；handoff、retry、lease expiry与 CAS loser 均 0，receipt/disposition 原子 `-Q`。
- ⬜ P2 载入守护推广：全部玩家 slice（SkillSet/Wounds/状态 buff/身份键……）收编，#1290 模式全量落地；身份主键统一（identity-persist-key-mismatch）。仅在 R10 P1 容量契约合入后，dropped-loot hydration guard 才引用 `MAX_DURABLE_DROPPED_LOOT_ENTRIES` 与 `DroppedLootRegistry::try_insert/try_insert_batch`；超限进入 load-failure guard/只读降级并告警，禁止截断、驱逐或空表覆盖。同步实现 spill/pickup recoverable transaction seam，使 source mutation、attrited item、zone/ledger、drop insert/delete 与 transaction id 原子提交；crash/retry pins 常绿后才放行 R10 P2a Public writer path。
- ⬜ P3 关服 flush + tick rebase 批次：shutdown flush registry 收编全部"节流落盘"域；绝对 tick 全部改相对基准；autosave/事件写入竞态互斥（coffin-autosave-inflight-race 模式）；R3 的其余非耦合 slice 按 registry 收编。`TsyPresence` 三者原子 autosave/disconnect/shutdown 语义已在 P1 冻结，P3 仅补全 flush registry 接线与 crash 回归，不得把 coupled snapshot boundary 延后或缩窄为 shutdown-only。
- ⬜ P4 遗漏运行态补持久化批次：ActiveEvents、TiandaoAttention、状态效果、化虚冷却、灵眼、地表遗缴、散灵珠、可放置实体、dormant 往返身份完整性（heiwushi）等——逐个按 Slice 框架补表。加入 `TsyPresence` guarded relog parity 契约测试和 TSY 维度重启 bot 场景，断言失败加载不 attach presence、成功恢复后才重新开放 TSY 请求。另拆两个 inventory consumer 子批次：R10 P1 + R3 P2 seam/compatibility pins 后，dropped-loot hydration 调 `migrate_legacy_dropped_loot_entry` 补 `owner=None`/`visibility=Public`，且先于 R6 projection/page；R10 P3 后，inventory-layout overflow 用真实 player/position/dimension `SpillContext` 调 `migrate_legacy_inventory_layout` 并持久化。任一 migration/context/capacity/persistence 失败均保留旧行可重试，两批不得绑成同一 gate。
- **P4 placeable gate addendum**：可放置实体子批次须落实 `plan-bughunt-placeable-entity-restart-loss-v1` P0-P2：持久化 `placed_id`（非 Entity），world/layer ready 后 hydrate `WorkbenchBlock` 并建立唯一 `placed_id→runtime Entity` registry；missing/duplicate/unhydrated fail closed。它是 R4 runtime target→stable claim 与 R1 restore rebind 的 provider，常绿前不得迁移 checkpointed workbench craft。
- ⬜ P5 bot 验收 + 吸收 plan 批量归档。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：active-events-restart-loss、mineral-respawn-tick-restart-drift、realm-taint-restart-amnesia、recipe-unlock-shutdown-flush（若 #1261 已 merge 则只归档）、season-override-restart、spiritwood-shutdown-flush、status-effects-consumable-persistence、supply-coffin-cooldown-restart-rollback、tiandao-attention-persistence、zone-influence-shutdown-flush、dormant-redis-dirty-ack、heiwushi-dormant-identity-loss；round-bundle 精确吸收：r1-mechanical-fixes P6 NPC deceased archive DB-open rollback、r10-findings #1 `mineral::record_exhausted_minerals` shutdown flush。
skeleton：coffin-autosave-inflight-race、identity-persist-key-mismatch、mineral-exhausted-log-corrupt-revival、placeable-entity-restart-loss、scatter-bead-burial-restart-loss、spirit-eye-runtime-persistence、surface-stash-lifecycle-volatile、voidaction-cooldown-runtime-tick-restart、coffin-offline-reclaim-respawn-dup、stale-spirit-niche-lifecycle；在飞：wounds-relog-full-heal（#1282）、player-slice-load-failure-clears（#1290）、shelflife-clock-restart-freshness（#1294）。

## 文件所有权与边界

- 独占：`server/src/persistence/**`、`player/state.rs`+`player/mod.rs` 的 autosave/载入区段、各域的持久化接线点（新增表定义）。
- 不碰：session 业务逻辑（R1 经钩子接入）、qi 语义（R5）、`client_request_handler.rs`（R4）。
- 依赖：基线等 #1288/#1289/#1259/#1261 merge；R1 P2 依赖本轨 P1。**本轨是 Wave 0 的锚，最优先动工。**

## bot 验收场景

1. `restart_player_slices`：bot 建号→修炼/学功法/受伤→关服重启→重连→断言功法/伤势/濒死后果/buff 全部还原。
2. `restart_world_runtime`：触发矿脉枯竭/配方解锁/zone influence→SIGTERM 关服→重启→断言无回滚无复活。
3. `load_failure_guard`：基础损坏 slice 断言守护降级而非清零覆盖；仅在 R10 P1 合入后，以超过 `MAX_DURABLE_DROPPED_LOOT_ENTRIES` 的 rows 断言数据库不得截断/清空。另覆盖旧 `entry_json` 缺 owner/visibility→`None`/`Public`、malformed/migration failure 保留旧行可重试，以及 spill durable write failure、pickup attrition staged 后 commit interruption/restart；attrited item、zone/ledger 与 drop delete 不得单边提交，按 transaction id 重试不得重复应用。
4. `tick_rebase`：带冷却/再生倒计时重启→断言倒计时按真实流逝折算（对齐 #1289 的 deadline 折算先例）；另持久化一半已消耗的 `SuspensionPolicy` lease，重启/rebase 只保留原剩余 TTL，连续重复重启不刷新 lease，并覆盖剩余时长前一 tick、精确边界、后一 tick；outbox 同样覆盖 `next_retry_tick` 剩余退避、`created_at_tick` 已消耗 age、`lease_until` 剩余 lease 在单次/连续重启后的前一 tick、精确边界、后一 tick，断言不会把旧 process-local tick 直接带入新 epoch。
5. `tsy_presence_relog_parity`：进入 TSY→关服 flush→guarded load→只有 `family_id`、`entered_at_tick`、`entry_inventory_snapshot`、`return_to`、schema/version 校验通过才 attach `TsyPresence` 并开放 TSY 请求；损坏或缺失 Slice 保持未 attach 且拒绝请求；恢复后 death-drop 对原带物继续执行 50%/武器保护，对 TSY 所得执行既有 100% 规则。
6. `tsy_presence_snapshot_atomicity`：分别在 routine autosave、disconnect save、shutdown flush 中，向 presence、position、dimension 三个逻辑写入之间注入 crash；重启后断言三者只能全部保留旧 snapshot/version 或全部提交新 snapshot/version，不接受各 Slice 独立提交后碰巧通过 clean restore 对拍。shutdown 路径另断言 session registry 静止后才 flush coupled snapshot。
7. `session_delivery_outbox_atomicity`：分别在 outbox insert 前、insert 后 checkpoint terminalize 前、事务 commit 后 ack 前强杀重启；payload 边界分成两条：序列化后恰为 `SESSION_DELIVERY_MAX_PAYLOAD_BYTES` 必须成功接受并完成 handoff，`SESSION_DELIVERY_MAX_PAYLOAD_BYTES + 1` 必须在接受新增 escrow/产物前 fail closed。正常路径断言 checkpoint/outbox 恰有一个权威 owner，完整 payload 不丢且 stable `delivery_id` 不变。以两个独立 SQLite connection/线程同时竞争最后一个 row reservation、以及分别竞争不足以容纳两份 reservation 的最后 bytes，屏障同步到事务入口后并发调用 `reserve_new_terminal_obligation`，每种场景必须恰好一个 commit、一个 quota-full，最终 counter 与唯一 reservation 对拍且重启后不超配；另以两个不同 `SessionKey` 竞争同一 busy facility，证明 reservation 均可先提交但 busy loser 通过 `cancel_unconsumed_reservation` 恰好释放自身 `Q`，重复取消/CAS loser 不释放赢家额度。checkpoint restore 与并发 retry 必须以 `reuse_terminal_obligation` 命中原 reservation、counter 增量为零；missing 或 owner/bytes/generation 冲突 fail closed。再覆盖 handoff 转移不双计、Pending/InFlight/DeadLetter retry 与 lease expiry 不改 quota、Committed/ResolvedDisposition 原子释放后新 admission 才成功。

## 开放问题（pre-P0 收口）

1. 载入守护的玩家体验：只读降级 vs 拒绝进服 vs 回滚到上一备份？需人工拍板。
2. 迁移链是否借机做一次 squash（v1-v39 合并基线）？风险与老存档兼容性需评估。
3. `ResolvedDisposition` 的完整 payload 不能在释放 obligation quota 后无限增长；本轮不冻结 retention、compaction 或独立 disposition quota，避免引入新的存储策略。延期到总纲 `plan-refactor-master-v1.md §3 Wave 2` 的 R10 durable-delivery 收口窗口，由 R3/R10 共同定义有界保留与释放规则。
4. TsyPresence 三者 coupled snapshot 的 routine autosave/disconnect/shutdown crash-injection harness 与跨连接屏障时序是 P1 acceptance 的组成部分：P1 必须在每个逻辑写边界注入 crash 并重启，证明结果只能全旧或全新；P3 只复用该 harness 做 flush-registry 长尾回归，不得延期首次原子性证据。

## § P0 决议锚点（待 R3 P0 开工时补齐）

- R3 P2/P4 仅在 R10 P1 merge 后引用 `MAX_DURABLE_DROPPED_LOOT_ENTRIES`，不复制常量；dropped-loot hydration consumer 在 R10 migration helper、R3 P2 seam 与 compatibility pins 后执行，且先于 R6 projection/page。
- inventory-layout overflow consumer 在 R10 P3 后以真实 `SpillContext` 持久化；任一 consumer 失败均保留旧行可重试。

## 验收与实施边界

- `cargo test --package bong-server persistence -- --nocapture` 覆盖 Slice guard、flush/tick rebase、migration consumer、dropped-loot bound 与 R1 §2.2.2 reservation 全状态转换；bot e2e 必须覆盖本 plan §bot 验收场景列出的全部七项：`restart_player_slices`、`restart_world_runtime`、`load_failure_guard`、`tick_rebase`、`tsy_presence_relog_parity`、`tsy_presence_snapshot_atomicity`、`session_delivery_outbox_atomicity`。任一场景未合入/未常绿，R3 P5 与下游 craft/TSY 放行均不得完成。
- R3 P1 只冻结 inventory seam，不实现 R10 migration/capacity；P2/P4 等依赖合入后才实现 consumer，且不修改 `server/src/inventory/**`。
