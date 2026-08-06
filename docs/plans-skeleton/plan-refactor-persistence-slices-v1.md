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
- **出料**：统一 Slice API：`load(guarded) / autosave(cadence) / flush_on_shutdown / tick_rebase`。R1 session persistence 只消费该接口；tick rebase 保留 suspension/retry/lease 的真实剩余时长与已消耗 age，不刷新租约。R3 durable 实现严格投影 R1 O-01..O-27：`reserve_new_terminal_obligation`（O-01/O-02）、`reuse_terminal_obligation`（O-04）、durable `CancelPending` reconciliation（O-05..O-07）、reservation→outbox atomic handoff（O-08/O-09）、claim/retry/dead-letter CAS（O-10..O-20）、receipt/disposition retention、bounded tombstone 与 watermark GC（O-21..O-24）。`TsyPresence` 与 player position/dimension 仍为独立 coupled snapshot contract；routine autosave、disconnect、shutdown 必须共用 transaction/version，crash 后只能全旧或全新。
- **共享类型**：新 `server/src/persistence/` 多文件模块（按域拆表定义 + 迁移链保持线性单入口）；`PlayerSliceRegistry`（对齐 #1290 skeleton 的方向，直接吸收它）。
- **跨仓库契约**：零 wire 改动。
- **qi_physics 锚点**：任何带 qi 的快照持久化/恢复不得造成账面变化；恢复失败的兜底路径必须走 `release_dormant_qi_to_zone` 而非丢弃（对齐守恒律红旗清单）。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：53 张表普查归域；冻结 Slice trait（载入守护语义：读失败 = 保留旧行 + 告警 + 只读降级，绝不写回空态；flush registry；tick rebase 协议）；等 #1288/#1289/#1261/#1259 merge 后定基线。
- ⬜ P1 框架落地 + 巨石拆分：保持 migration 单入口；上线 Slice guard/flush/rebase 与 inventory seam。实现 `SessionDeliveryQuota`、`SessionDeliveryReservation`、`SessionDeliveryOutbox`、receipt/disposition/history/tombstone storage 及 R1 O-01..O-27 所需 atomic CAS API。quota admission 直接消费 R1 `MAX_ACTIVE_SESSION_DELIVERY_ROWS = 4_096`、`MAX_ACTIVE_SESSION_DELIVERY_BYTES = 4_294_967_296` 与每 obligation 固定 1 MiB reservation；O-01 在同一事务/CAS 同时检查 row/bytes aggregate，任一满即 O-02，DeadLetter 在 O-19 前仍计入 active aggregate。固定 reservation 使 row/bytes 只能 lockstep 到达边界，P1 acceptance 以 `(rows, bytes)` 的 limit-1、exact-limit、limit+1 paired cases 与双连接竞争对拍，不要求不可达的独立 row-full/bytes-not-full 或 bytes-full/rows-not-full。O-05 后 cancel-mark 写失败仍由原 reservation 作为 durable owner（O-25），写成功但后续取消失败则留下 `CancelPending` owner、retry metadata 与 live reconciliation scanner（O-07）；只有 O-06 可释放 Q。O-08 同事务固定 payload bytes/SHA-256 digest、terminalize checkpoint 并转 reservation 为 outbox；commit 后 R1 执行 S-14，ack 丢失可从 durable 状态重放。P1 同时冻结 `ReconnectGuard { owner_key, session_key, generation, phase_revision, restore_token }` 与 Suspended checkpoint 的同事务 persistence seam，供 R1 S-10 reload、R6 `CraftRestoreGuard` control frame 和 R2 guarded Restore consumer 使用；该 seam 在对应 atomic cutover 前保持 declared/test-only。P1 acceptance 覆盖 O-row trace、lockstep quota boundary、`serialized_bytes <= SESSION_DELIVERY_MAX_PAYLOAD_BYTES`（含 max 与 max+1 reject）、handoff crash points、cancel failure/restart retry、payload digest、O-11 CAS loser no-write 后 reread winner `InFlight`/authoritative row、receipt/disposition atomic release。另在同阶段交付 `TsyPresence`/position/dimension routine/disconnect/shutdown coupled-snapshot crash harness；P3 仅复用，不延期首次证据。
- **P1 obligation storage projection**：API、状态、quota effect 与 acceptance 只引用 R1 O-01..O-27；R3 不另设 lifecycle table。实现可选择独立 `CancelPending` 表或 reservation row metadata，但必须保持 O-07 durable retry owner。**排期裁决**：R3 P1 的 M-04 durable provider（quota/reservation/outbox/CAS/history storage 与 coupled-snapshot implementation）属于 master Wave 0 的实现交付；master Wave 2 的 M-04 相关表述只指下游 consumer/production activation（例如 R10 P1 及其依赖接线），不回置或延迟 R3 P1。R3 P1 可在 Wave 0 合入 contract-first/provider implementation，但在对应 master atomic cutover 前，跨轨 consumer 只能使用 declared/test-only seam，不得宣称 production reachable。
- ⬜ P2 载入守护推广：全部玩家 slice（SkillSet/Wounds/状态 buff/身份键……）收编，#1290 模式全量落地；身份主键统一。session restore 在 startup 发现 persisted `Running/Paused` 来自旧 process epoch 或没有 runtime binding 时必须执行 R1 S-25：先 durable fence/increment generation，再 normalize 为 `Suspended`，禁止恢复旧 Bevy `Entity` 或直接继续 tick；随后只可经 S-10/S-11 guarded rebind。dropped-loot hydration guard 只消费 R10 M-13 提供的 metadata/capacity/纯 migration contract 与 R3 M-04 durable seam，并作为 R3 producer evidence 汇入 M-14；超限进入 load-failure guard/只读降级并告警，禁止截断、驱逐或空表覆盖。spill/pickup recoverable transaction 只在对应 master M-row provider/consumer 全部存在后接入。
- ⬜ P3 关服 flush + tick rebase 批次：shutdown flush registry 收编全部节流落盘域；绝对 tick 全部改相对基准；autosave/事件写入竞态互斥。`TsyPresence` 三者原子 autosave/disconnect/shutdown 语义已在 P1 冻结，P3 仅补全 flush registry 接线与 crash 回归。
- ⬜ P4 遗漏运行态补持久化批次：ActiveEvents、TiandaoAttention、状态效果、化虚冷却、灵眼、地表遗缴、散灵珠、可放置实体、dormant 往返身份完整性等按 Slice 框架补表。`TsyPresence` guarded relog parity 与 placed-id hydrate 分别引用 M-12/M-05；migration consumer 只按 master ledger 进入，不复制跨轨顺序。
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
7. `session_delivery_outbox_atomicity`：执行 R1 O-01..O-27 与 S-07/S-25 的 durable traces；覆盖固定 Q 下 `(rows, bytes)` lockstep quota 的 limit-1/exact-limit/limit+1 paired cases、双 connection race、busy loser 的 O-25（O-05 标记未提交）或 O-05→O-07/O-06（标记已提交）、restore O-04、O-01 固定 1 MiB reservation、实际 payload `serialized_bytes <= SESSION_DELIVERY_MAX_PAYLOAD_BYTES` 与 max+1 reject 且无 resize、S-07 从 Running/Paused 各自在 checkpoint/state commit 前失败回 exact prior state、commit 后 crash restore Suspended、旧 process epoch persisted Running/Paused 经 S-25 fence 后 Suspended且不复用 Entity、handoff crash、O-11 CAS loser no-write并读到 winner authoritative state、attempts 7/8 与 retry age 前一 tick/精确边界/后一 tick、payload digest mismatch、history quota O-26、O-16/O-19 atomic release、O-17 stale replay 不二次 release，以及 O-21→O-24 bounded retention/GC。每步断言 `quota_rows/bytes = sum(active obligations)`，DeadLetter disposition 前仍计入。

## 开放问题（pre-P0 收口）

1. 载入守护的玩家体验：只读降级 vs 拒绝进服 vs 回滚到上一备份？需人工拍板。
2. 迁移链是否借机做一次 squash（v1-v39 合并基线）？风险与老存档兼容性需评估。
3. receipt/disposition history 的 replay horizon、tombstone 上限、GC watermark 与 history quota 已由 R1 §3.1 冻结；R3 P0 只决定表结构、索引与 compaction 调度，必须执行 O-21..O-27，不能永久保留或在重启时刷新 age。
4. TsyPresence 三者 coupled snapshot 的 routine autosave/disconnect/shutdown crash-injection harness 与跨连接屏障时序是 P1 acceptance 的组成部分：P1 必须在每个逻辑写边界注入 crash 并重启，证明结果只能全旧或全新；P3 只复用该 harness 做 flush-registry 长尾回归，不得延期首次原子性证据。

## § P0 决议锚点（待 R3 P0 开工时补齐）

- **R3 P2** 在 master M-13 provider 完成后接入 dropped-loot guarded hydration，并把真实 hydration evidence 交给 M-14；它不等待自己参与生产的 M-14。R6 projection/page 只在 R3 hydration 与 R6 projection 两侧共同闭合 M-14 后启用。R3 不复制 R10 常量，失败保留旧行可重试。
- **R3 P1** 的 outbox/reservation 与 coupled snapshot 是 M-04/M-12 的 implementation surface；R1/R10 仅消费冻结接口。

## 验收与实施边界

- `cargo test --package bong-server persistence -- --nocapture` 覆盖 Slice guard、flush/tick rebase、migration consumer、dropped-loot bound 与 R1 O-01..O-27 durable traces；bot e2e 必须覆盖本 plan §bot 验收场景列出的全部七项：`restart_player_slices`、`restart_world_runtime`、`load_failure_guard`、`tick_rebase`、`tsy_presence_relog_parity`、`tsy_presence_snapshot_atomicity`、`session_delivery_outbox_atomicity`，这是 R3 P5 自身完成门。craft production activation 仍只由 master M-10 的 prerequisites/cutover evidence 放行；TSY attach/restore 仍只消费 master M-12 的 coupled-snapshot evidence，不因 R3 七项场景另建跨轨 release gate。
- R3 P1 实现 M-04 durable provider 与 M-12 coupled-snapshot implementation：交付 `SessionDeliveryQuota`、reservation/outbox、receipt/history/tombstone storage、atomic CAS 与 coupled-snapshot API，并冻结 inventory seam；不实现 R10 migration/capacity，也不修改 `server/src/inventory/**`。P2/P4 在对应 master M-row 就绪后实现 consumer，R1/R10 只消费冻结接口。
