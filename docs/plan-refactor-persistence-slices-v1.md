# plan-refactor-persistence-slices-v1 — 玩家/世界状态持久化 Slice 框架 + persistence 巨石拆分（重构轨 R3）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：拆掉 16324 行的 `persistence/mod.rs` 巨石，建统一的持久化 Slice 框架——载入失败守护（绝不空表覆盖）、关服强制 flush registry、相对 tick 基准、autosave 竞态互斥——把“重启丢档/断线丢档/载入清零”整簇缺陷从根上消灭。

## 现状证据（2026-07-28 P0 复核）

- `origin/main` 已含 #1289 的 v39 `player_lifecycle`：当前生产基线为 **53 次生产 DDL、51 个独立表**，`CURRENT_USER_VERSION = 39`。原侦察的“53”现已成立；P0 普查表在玩家核心 Slice 中纳入 `player_lifecycle`。
- `agent_world_model` 仍是版本链外的 schema ensure；`tribulations_active` 与 `player_lifespan` 各有一次兼容性重复建表。
- 覆盖参差：SQLite 已承载玩家、NPC、社交、zone、heartbeat、qi runtime 等状态；ActiveEvents、TiandaoAttention、长期状态效果等仍有纯内存缺口。JSON 域的 mineral/spiritwood 已有 hydrate/节流保存，但缺关服 flush 或损坏保护，不能表述成“完全无 persistence”。
- #1289 已于 2026-07-28 合入，Lifecycle persistence 的文件避让解除；#1259 仍开放且修改 `persistence/mod.rs`、`player/state.rs`、`player/mod.rs`、`combat/lifecycle.rs`，其合入前继续避让这些精确文件。
- 绝对 runtime tick 已确认造成重启漂移：mineral `respawn_at_tick`、void action `ready_at_tick` 会把旧进程 uptime 带入新进程；heartbeat pseudo-vein 的 observed age + pending elapsed + wall anchor 是可复用的正向范式。
- 关服链路已由 `shutdown.rs` 在 `PreUpdate` 发出一次 `AppExit::Success`，同帧 `Last` 可落盘；目前 player、recipe unlock、zone runtime 等各自消费事件，没有统一顺序、错误隔离和汇总报告。

## P0 表域普查（当前 v39 基线）

| 目标域 | 当前表 | 后续拆分落点 |
|---|---|---|
| bootstrap / 运维 | `bootstrap_events` | `persistence/bootstrap.rs` |
| 玩家核心 Slice | `player_core`、`player_slow`、`inventories`、`player_ui_prefs`、`player_lifespan`、`player_lifecycle`、`player_skills`、`player_shrine`、`player_cultivation`、`player_known_techniques`、`player_craft_sessions`、`player_identities`、`dropped_loot` | `persistence/player/{core,position,inventory,ui_prefs,lifespan,lifecycle,skills,shrine,cultivation,techniques,craft_session,identity}.rs`；保留 `player/state.rs` facade 与跨表 transaction |
| 生死与公共档案 | `life_records`、`life_events`、`death_registry`、`lifespan_events`、`deceased_snapshots`、`epitaphs` | `persistence/{life,deceased_archive,epitaph}.rs` |
| NPC / 势力 / 离屏 | `npc_state`、`npc_digests`、`factions`、`reputation`、`membership`、`relationships`、`archetype_registry`、`npc_deceased_index`、`pending_dormant_relics` | `persistence/npc/{runtime,faction,archetypes,archive,dormant_relics}.rs` |
| 渡劫 / 身份 / 化虚 | `tribulations_active`、`ascension_quota`、`legacy_letterbox`、`void_action_cooldowns`、`high_renown_milestones` | `persistence/{tribulation,void_state,social_milestones}.rs`；`player_identities` 继续由现有 `identity.rs` 承载 |
| 世界 / 区域 / qi runtime | `zones_runtime`、`zone_overlays`、`zone_influence`、`heartbeat_pseudo_veins`、`qi_runtime_accounts` | `persistence/world/{zones,territory,heartbeat}.rs`、`persistence/qi_runtime.rs` |
| Agent / 天道 | `agent_eras`、`agent_decisions`、`agent_world_model` | `persistence/agent.rs`；先把 `agent_world_model` 的版本链外 ensure 单列，不在机械拆分时偷偷改迁移语义 |
| 社交 | `social_anonymity`、`social_relationships`、`social_exposures`、`social_renown`、`social_spirit_niches`、`social_faction_memberships`、`social_faction_reputations` | `persistence/social.rs` |
| 待确认消费者 | `spirit_treasure_world`、`spirit_treasure_dialogue_log` | 先标 orphan candidate；确认生产 consumer 后再迁往 `persistence/spirit_treasure.rs`，不得仅凭 schema 存在宣称已接线 |

迁移拆分只允许机械移动且保持版本、顺序、transaction 与失败行为。v13 legacy cultivation backfill、v21/v23 缺表/缺列兼容、v30 faction 数据映射、v34 qi unknown/fail-closed、v35 heartbeat 保守回填、v38 overflow 初始化，以及 v39 Lifecycle rebase 都是 load-bearing migration，禁止行为级 squash。

## 接入面

- **进料**：SQLite（`bong.db`，沿用每次操作开连接的 ownership）、JSON 状态文件、`shutdown.rs` 的 `AppExit` 链路、各域 runtime clock 与 wall-clock snapshot。
- **出料**：统一 Slice contract 供各域声明 `load_policy / autosave_policy / shutdown_flush / time_basis / write_domain`；R1 的 session 持久化钩子和各域运行态逐批接入。
- **共享类型**：`server/src/persistence/slice.rs` 的 `PersistenceSlice`、`SliceDescriptor`、`SliceLoad`、`PersistenceSliceRegistry`、`DirtyTracker` 与 tick rebase helper。
- **跨仓库契约**：零 wire 改动。
- **qi_physics 锚点**：任何带 qi 的快照持久化/恢复不得造成账面变化；`qi_runtime_accounts` 的缺行/失败继续 fail-closed，恢复兜底不得把未知余额解释为 0。

## 阶段

- ⏳ P0 设计收口 + contract pins：完成 51 表归域与吸收清单验真；在 `server/src/persistence/slice.rs` 冻结 descriptor-based Slice contract、load guard、shutdown registry、同 tick save-before-load、注入时钟、tick rebase 与 domain-bound dirty receipt；不迁移生产 slice。
- ⬜ P1 框架落地 + 巨石拆分：`persistence/` 按域拆文件（迁移链不变、行为不变）；等 #1259 合入后，将 KnownTechniques/Lifecycle 平移为首批宿主。
- ⬜ P2 载入守护推广：全部玩家 slice（core/position/inventory/SkillSet/Wounds/长期 buff/身份键等）收编；聚合 writer 按 `WriteSet` omit 被阻断 slice。
- ⬜ P3 关服 flush + tick rebase 批次：shutdown registry 逐域替换旧 `Last` hook；绝对 deadline 改相对基准；autosave/事件写入按 write authority + revision/CAS 串行化。
- ⬜ P4 遗漏运行态补持久化批次：ActiveEvents、TiandaoAttention、长期 consumable/buff、realm taint、season override、supply cooldown、灵眼等逐个补 Slice；业务生命周期修复留在各自领域。
- ⬜ P5 bot 验收 + 吸收 plan 批量归档。

## 吸收清单验真（25 项）

### R3 直接吸收

- **仍缺、作为框架/迁移宿主**：active-events-restart-loss、mineral-respawn-tick-restart-drift、realm-taint-restart-amnesia、season-override-restart、spiritwood-shutdown-flush、status-effects-consumable-persistence（仅长期 consumable/buff）、supply-coffin-cooldown-restart-rollback、tiandao-attention-persistence、zone-influence-shutdown-flush（已有 hydrate/节流，范围仅为最终 flush）、dormant-redis-dirty-ack（异步 ACK/失败重脏）、coffin-autosave-inflight-race、identity-persist-key-mismatch、mineral-exhausted-log-corrupt-revival、spirit-eye-runtime-persistence、voidaction-cooldown-runtime-tick-restart。
- **merged plan only、代码仍缺**：wounds-relog-full-heal（#1282）、player-slice-load-failure-clears（#1290）、shelflife-clock-restart-freshness（#1294）。Wounds/Lifecycle 生产接线已随 #1289 落地，但要等 #1259 解除精确文件避让后再迁入 Slice 框架。
- **代码已闭环、P5 只归档**：recipe-unlock-shutdown-flush（#1261）。KnownTechniques #1288 不是清单项，但作为 load guard 基线。

### R3 只提供 persistence adapter/原语，业务修复拆回领域 owner

- heiwushi-dormant-identity-loss → NPC virtualization；
- placeable-entity-restart-loss → world/container lifecycle；
- scatter-bead-burial-restart-loss → zhenfa + qi ledger；
- surface-stash-lifecycle-volatile → TSY/onboarding；
- coffin-offline-reclaim-respawn-dup → coffin/session lifecycle；
- stale-spirit-niche-lifecycle → social/new-character lifecycle。

这六项仍可使用 R3 的 snapshot/hydrate/flush 原语，但不得把实体重建、所有权、守恒或角色轮换语义塞进通用持久层。

## 文件所有权与边界

- 独占：`server/src/persistence/**`、`player/state.rs` + `player/mod.rs` 的 autosave/载入区段、各域持久化接线点。
- P0 冻结区：#1289 已合入；#1259 合入前继续避让 `server/src/persistence/mod.rs`、`server/src/player/state.rs`、`server/src/player/mod.rs`、`server/src/combat/lifecycle.rs` 的生产主体。P0 对 `persistence/mod.rs` 仍只允许增加模块声明。
- 不碰：session 业务逻辑（R1 经钩子接入）、qi 语义（R5）、`client_request_handler.rs`（R4）。已有 craft/inventory/cultivation/ledger/dropped-loot 跨表 transaction 不得拆成多连接写入。
- 不引入：全局 `rusqlite::Connection` Resource、`Mutex<Connection>`、异步全局 writer、所有时间统一 wall-clock、或新旧 shutdown hook 双注册。

## bot 验收场景

1. `restart_player_slices`：bot 建号→修炼/学功法/受伤→关服重启→重连→断言功法/伤势/濒死后果/buff 全部还原。
2. `restart_world_runtime`：触发矿脉枯竭/配方解锁/zone influence→SIGTERM 关服→重启→断言无回滚无复活。
3. `load_failure_guard`：注入一行损坏 slice 数据→启动→断言该玩家进入守护降级而非清零覆盖。
4. `tick_rebase`：带冷却/再生倒计时重启→断言按该 slice 的 offline policy 折算，首个 live tick 不重复计时。

## 开放问题（pre-P0 收口）

1. 载入守护的玩家体验：只读降级 vs 拒绝进服 vs 回滚到上一备份？
2. 迁移链是否借机做一次 squash？

以上问题均已在下节收口。原表保留以备追溯，**实施时以 §P0.1 决议为准**。

## §P0.1 决议（pre-P0 收口，2026-07-27）

### #1 Slice 形态：静态 descriptor + exclusive-world adapter

**决议**：
1. `PersistenceSlice` 只返回一个静态 `SliceDescriptor`；registry 保存静态 descriptor 并按 `(order, id)` 稳定排序。hook 使用无捕获函数指针 `fn(&mut World, &SliceRunContext) -> SliceRunResult`，由 adapter 在 hook 内拿强类型 Resource/Query。
2. descriptor 必须声明 `scope`、`load_policy`、`time_basis`、`write_domain`、`write_ordering`、`autosave_policy`，以及可选 hydrate/rebase/shutdown hook；重复/空 Slice ID 在注册时 fail-fast。
3. 不把泛型 `SystemParam`、`Query`、`ResMut`、关联类型或 `rusqlite::Connection` 装进 trait object。若后续确需动态 driver，再以同一对象安全签名包装，不推翻 P0 descriptor。

**落点**：`server/src/persistence/mod.rs:103-110`、`server/src/player/state.rs:278-290`（现有资源只拥有路径/元数据）；plan §阶段 P0、§接入面。

### #2 Load guard：读取结果与写资格分离

**决议**：
1. 统一三态为 `SliceLoad<T, E> = Missing | Loaded(T) | Failed(E)`。只有连接成功、查询成功且确认无行才是 `Missing`；连接/SQL/解码/校验失败一律是 `Failed`。
2. `Missing` 与 `Loaded` 可写；`Failed` 即使为维持会话创建 runtime default，也必须携带 `WriteBlocked` provenance。即时 Changed、周期 autosave、disconnect、shutdown、export 与聚合 transaction 全部只能通过 `GuardedSlice::write_permit` 取得不可直接构造的 writer permit；公开 API 不提供丢弃 load provenance 的拆解入口，新会话成功重载才恢复写资格。
3. 默认体验为**按 slice 降级**：展示/非关键 slice 可只读运行；core、inventory、craft、lifespan/Lifecycle、cultivation/qi 等高价值或跨 slice mutation 在依赖集读取失败时拒绝可变 gameplay。关键世界账本使用 `RefuseStartup`。自动回滚整库备份不进入在线加载路径，只保留人工审计恢复。

**落点**：`server/src/player/state.rs:173-182`、`server/src/player/mod.rs:221-272,470-486,629-645`（KnownTechniques 正向范式）；`server/src/player/state.rs:434-573`（待推广 fallback）；plan §阶段 P2、§bot 验收场景 #3。

### #3 Shutdown registry：复用 `AppExit → Last`，失败隔离

**决议**：
1. 不重造 signal handler。唯一触发仍是 `shutdown.rs` 在 `PreUpdate` 发出的单次 `AppExit::Success`；统一 dispatcher 位于 `Last`。
2. dispatcher 复制已排序 descriptor 列表后再逐个调用 hook，避免持有 registry 的 World borrow 时再次可变借用 World；返回汇总报告 `attempted/clean/flushed/blocked/failures`。
3. 单个 hook 失败不得中断后续 slice；失败必须保留 dirty 和旧文件/旧行。P0 只冻结和测试 dispatcher，不注册生产 slice；P3 迁移时逐域“移除旧 hook → 注册 registry”，禁止双写。

**落点**：`server/src/shutdown.rs:43-74,247-375`；`server/src/craft/unlock.rs:282-301,1209-1337`；`server/src/persistence/mod.rs:921-977`；plan §阶段 P3、§bot 验收场景 #2。

### #4 Tick rebase：每个 Slice 声明时间语义

**决议**：
1. `TimeBasis` 至少区分 `None`、`RemainingLogicalTicks`、`WallDeadline`、`ObservedAgeWithElapsed`；禁止持久化一个“跨进程永不归零”的全局 runtime clock，也禁止把所有字段粗暴改成 wall-clock。
2. deadline 持久化 `remaining_ticks + saved_at_wall + offline_policy`：online-only 重建为 `new_tick + remaining`；offline-continuous 先按 `MILLIS_PER_TICK` 扣除 wall elapsed，再重建本地 deadline。age/elapsed 使用 observed age + pending elapsed；history/audit tick 不参与新进程 deadline 比较。
3. rebase 在 hydrate 后、首个 live Update 前只执行一次。旧 raw deadline 无法精确恢复时必须写明保守迁移，不得伪造精确值。

**落点**：`server/src/time.rs:1`；`server/src/persistence/mod.rs:2826-2919`（void action 反例）；`server/src/mineral/persistence.rs:57-71,180-259`（mineral 反例）；`server/src/world/heartbeat.rs:491-584,3025-3153`（正向范式）；plan §阶段 P3、§bot 验收场景 #4。

### #5 Autosave 竞态：write authority + dirty revision/CAS

**决议**：
1. 每个 `WriteDomain` 的 mutation 递增 `DirtyRevision`；`DirtyTracker` 只能凭同一 `WriteBinding(domain + authority)` 的 write permit 产生 snapshot。写失败不产生 receipt、永不清 dirty；writer 成功后由 `PersistedRevisionFence::commit` 产生不可直接构造的 domain-bound `DurableWriteReceipt`，tracker 仅消费匹配当前 revision 的 receipt 才能 ack clean。
2. revision 只保护内存 dirty acknowledgement，不能单独阻止旧 snapshot 晚到覆盖数据库。registry 对同一 domain 强制唯一 authority 和一致 ordering；每个 domain 选择单一串行 writer，或由 `DurableWriteRequest` 把 expected persisted revision 纳入 SQL CAS/单调拒绝。
3. 字段写权威必须明确：事件拥有的字段不得被周期快照重新断言。跨 inventory/session/cultivation/ledger/dropped-loot 的原子 checkpoint 保持领域 transaction，不拆散。

**落点**：`server/src/coffin/mod.rs:656-666`、`server/src/player/mod.rs:773-805`、`server/src/player/state.rs:670-697,780-830`；plan §阶段 P3。

### #6 迁移链与 P0 范围：不 squash，只落纯契约

**决议**：
1. 不重置 `PRAGMA user_version`，不删除 v1–v39 legacy upgrade path，不把行为迁移 squash 成一份 fresh schema。未来可额外生成新库 baseline，但旧库升级链、升级前备份和 fixture 必须长期保留。
2. P0 只新增 `server/src/persistence/slice.rs` 与 contract-pin tests，并在 `persistence/mod.rs` 增加模块声明；不迁移生产 hook、不修改 schema、不拆巨石。#1289 已合入；#1259 的玩家/饱食度接线继续避让。
3. P0 pins 覆盖：registry ID/authority/ordering 校验与稳定排序；无 shutdown 请求不调用；失败隔离；load 三态、`RefuseStartup` 与不可伪造 write permit；deadline 两种 offline policy 与边界；domain-bound dirty snapshot + durable receipt；同 tick save-before-load；注入时钟。

**落点**：`server/src/persistence/mod.rs:57-62,1083-2386`；plan §P0 表域普查、§阶段 P0、§文件所有权与边界。

### #7 同 tick 断线保存 / 重连载入顺序：保存先于载入（#1289 review 继承项）

**决议**：
1. 同一持久化主体在同一 schedule tick 内出现 disconnect 与 reconnect 时，必须同步完成旧实体的 disconnect save，成功后才允许新实体 hydrate；保存失败则跳过载入并保留失败，禁止从旧 durable row 重建后继续运行。
2. P0 以 `dispatch_reconnect_handoff` 冻结该次序：registry 内同一玩家主体的所有 `SliceDescriptor::disconnect_save` 先按稳定顺序串行完成；只有全部保存成功后，才开始任何 `hydrate`。入口用稳定 `handoff_key` 绑定主体；任一保存失败都会跳过整个载入阶段。
3. P1/P2 真实玩家接线必须使用该 handoff 入口，不得依赖 Bevy 系统注册先后、deferred commands 或“通常下一 tick 才重连”的时间假设。

**落点**：`server/src/persistence/slice.rs:124-149,191-203,446-532,1342-1408` 的 `SliceRunReason::{DisconnectSave,ReconnectLoad}`、`SliceDescriptor::disconnect_save`、`dispatch_reconnect_handoff` 与 all-save-before-any-load contract pin；plan §阶段 P0/P2、§bot 验收场景 #1。

### #8 时间 / deadline 测试：只用注入时钟（#1289 review 继承项）

**决议**：
1. Slice dispatcher 不直接读取 wall clock；统一消费 `SliceClock` 注入的 `runtime_tick` 与 `wall_unix_millis`。测试使用 `FixedClock`，精确固定边界两侧的毫秒值。
2. deadline rebase helper继续接受显式时间参数；contract pins 禁止调用 `SystemTime::now()`、`Instant::now()` 或依赖测试执行恰好未跨秒的 exact assertion。
3. 生产 adapter 在调用边界采样一次时间后注入；同一 dispatch 内复用该快照，避免一次操作跨秒得到不一致字段。

**落点**：`server/src/persistence/slice.rs:133-150,407-425,468-485,1022-1049,1052-1064,1590-1631` 的 `SliceClock`、`dispatch_shutdown_flushes`、`dispatch_reconnect_handoff`、显式时间参数 rebase helper 与 `FixedClock` contract pins；plan §阶段 P0/P3、§bot 验收场景 #4。
