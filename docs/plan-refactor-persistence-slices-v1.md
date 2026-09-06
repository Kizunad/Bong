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
| 渡劫 / 身份 / 化虚 | `tribulations_active`、`ascension_quota`、`void_action_cooldowns`、`high_renown_milestones` | `persistence/{tribulation,void_state,social_milestones}.rs`；`player_identities` 继续由现有 `identity.rs` 承载 |
| 世界 / 区域 / qi runtime | `zones_runtime`、`zone_overlays`、`zone_influence`、`heartbeat_pseudo_veins`、`qi_runtime_accounts` | `persistence/world/{zones,territory,heartbeat}.rs`、`persistence/qi_runtime.rs` |
| Agent / 天道 | `agent_eras`、`agent_decisions`、`agent_world_model` | `persistence/agent.rs`；先把 `agent_world_model` 的版本链外 ensure 单列，不在机械拆分时偷偷改迁移语义 |
| 社交 | `social_anonymity`、`social_relationships`、`social_exposures`、`social_renown`、`social_spirit_niches`、`social_faction_memberships`、`social_faction_reputations` | `persistence/social.rs` |
| 待确认消费者 | `spirit_treasure_world`、`spirit_treasure_dialogue_log` | 先标 orphan candidate；确认生产 consumer 后再迁往 `persistence/spirit_treasure.rs`，不得仅凭 schema 存在宣称已接线 |

迁移拆分只允许机械移动且保持版本、顺序、transaction 与失败行为。v13 legacy cultivation backfill、v21/v23 缺表/缺列兼容、v30 faction 数据映射、v34 qi unknown/fail-closed、v35 heartbeat 保守回填、v38 overflow 初始化，以及 v39 Lifecycle rebase 都是 load-bearing migration，禁止行为级 squash。

## 接入面

- **进料**：SQLite（`bong.db`，沿用每次操作开连接的 ownership）、JSON 状态文件、`shutdown.rs` 的 `AppExit` 链路、各域 runtime clock 与 wall-clock snapshot。
- **出料**：统一 Slice contract 供各域声明 `load_policy / autosave_policy / shutdown_flush / time_basis / write_domain`；R1 的 session 持久化钩子和各域运行态逐批接入。
- **共享类型**：`server/src/persistence/slice.rs` 的公开 `PersistenceSlice`、`SliceDescriptor`、`SliceLoad`、`GuardedSlice`、`DirtyTracker` 与 tick rebase helper；`PersistenceSliceRegistry` 及其 activation token 属于 `crate::persistence` 内部 trust boundary，不向 gameplay 暴露构造/Resource 访问能力。server、agent、client 三端均不新增、不修改且不复用任何 event/schema。
- **跨仓库契约**：零 wire 改动；server、agent、client 三端均无 event/schema 新增、变更或复用，不扩大现有 IPC/CustomPayload 边界。
- **worldview 锚点**：`docs/worldview.md §一 L17-L22` 的全服灵气总量/压强法则、`§二 L30-L55` 的正负灵域时间与区域语义、`§十三 L1209-L1268` 的固定区域身份。Slice 只保存/恢复既有状态，不创造灵气、不重解释区域，也不得绕过 qi ledger。
- **qi_physics 锚点**：任何带 qi 的快照持久化/恢复不得造成账面变化；`qi_runtime_accounts` 的缺行/失败继续 fail-closed，恢复兜底不得把未知余额解释为 0。P2/P5 以 `qi_physics::ledger::{summarize_world_qi, assert_conservation}` 对保存→重启/同 tick handoff/失败加载前后做 `era_decay = 0` 守恒验收，并锁定原 account/zone 身份不被 fallback 重解释。

## 阶段

- ✅ 2026-07-30 P0 设计收口 + contract pins：完成 51 表归域与吸收清单验真；`server/src/persistence/slice.rs` 已冻结 descriptor-based Slice contract、opaque load guard、shutdown registry、同 tick save→全量只读 preflight→无 blocked/error 返回通道的 cleanup→lease check→subject-bound capability hydrate→实际 lease 审计→rebase（hydrate/rebase fail-fast，后序失败则对所有已尝试 descriptor 逆序 cleanup 并复核 token/foreign subject lease）、注入时钟、tick rebase、稳定主体独占 activation 与 payload-bound dirty receipt；contract-pin tests 覆盖 registry 缺失 fail-closed、注册冲突、失败 provenance 不可提取、写资格、重连顺序/阻断/旧 activation 原子保留/foreign-subject 拒绝/部分 hydrate 回滚与干净重试、dirty revision/CAS 与 deadline 边界。2026-08-04 返工安装唯一 canonical registry 与 `AppExit → Last` dispatcher，将 zone-runtime 关服写入迁为首个 production descriptor，并以 KnownTechniques 接通首个真实 player reconnect/load guard/dirty snapshot/durable fence adapter；P3 负责向其余玩家与世界域推广，不重复安装这些 P0 接入点。
- ⬜ P1 框架落地 + 巨石拆分 + M-04/M-12 guard/checkpoint 持久化：`persistence/` 按域拆文件（迁移链不变、行为不变），机械保留 P0 已安装的 canonical registry、zone-runtime 与 KnownTechniques production wiring；等 #1259 合入后，将 Lifecycle 等其余首批宿主平移入对应域文件。同时落地 master §4.2 M-10 登记的 M-04/M-12 guard/checkpoint 持久化：S-07 `ReconnectGuard` 与 Suspended checkpoint 同事务持久化、`CraftRestoreGuard` control frame 持久化（R1 `docs/plan-refactor-server-session-v1.md` §5 owner contract 的持久化侧），作为 craft production atomic activation 的 R3 persistence 依赖。
- ⬜ P2 载入守护推广：全部玩家 slice（core/position/inventory/SkillSet/Wounds/长期 buff/身份键等）收编；聚合 writer 按 `WriteSet` omit 被阻断 slice。
- ⬜ P3 reconnect/关服 flush/tick rebase 推广批次：以 P0 的 KnownTechniques production adapter 为基线，将一次性 subject-bound handoff、shutdown descriptor、相对 deadline 与 write authority + revision/CAS 串行化逐域推广；每迁一域先移除其旧 hook，禁止双写。R1 session 域衔接：coupled `TsyPresence` snapshot 由 R3 提供（R1 reconnect/gate consumer，`docs/plan-refactor-server-session-v1.md` §5 登记）。
- ⬜ P4 遗漏运行态补持久化批次：ActiveEvents、TiandaoAttention、长期 consumable/buff、realm taint、season override、supply cooldown、灵眼等逐个补 Slice；业务生命周期修复留在各自领域。R1 session 域衔接：O-01..O-27 reservation/quota/outbox 的 durable producer/storage 由 R3 提供（R1 semantic consumer、R10 worker consumer，`docs/plan-refactor-server-session-v1.md` §5 登记）。
- ⬜ P5 bot 验收 + 吸收 plan 批量归档。

## 吸收清单验真（27 项）

下表以当前 production code 为准；`P0 契约`只说明本 PR 已提供框架原语，**不等于**对应生产 Slice 已接线。`—` 表示无独立 durable schema/file 或无该方向接线。

| # | 吸收项 | schema / file | save | load | relog / restart wiring | 当前代码锚点 | 剩余归属 |
|---:|---|---|---|---|---|---|---|
| 1 | `active-events-restart-loss` | —；`ActiveEventsResource` 仅内存 | — | — | restart `default()` | `server/src/world/events.rs:101-114,345-376,1625-1638` | P4 world-runtime Slice |
| 2 | `mineral-respawn-tick-restart-drift` | JSON exhausted-minerals log | 有，但 `respawn_at_tick` 为旧进程绝对 tick | 有 | startup hydrate 已接 | `server/src/mineral/persistence.rs:57-70,115-173,177-259`；`server/src/mineral/mod.rs:69-108`；`server/src/mineral/respawn.rs:29-74` | P3 time rebase |
| 3 | `realm-taint-restart-amnesia` | —；`RealmTaintState` 仅 ECS | — | — | 仅 Update event consumer | `server/src/cultivation/realm_taint.rs:16-107` | P4 player/runtime Slice |
| 4 | `season-override-restart` | —；`WorldHeartbeat.overrides` 仅内存 | — | — | register 后 runtime apply/expiry | `server/src/world/heartbeat.rs:239-255,316-379,594-657,825-868` | P4 world-runtime Slice |
| 5 | `spiritwood-shutdown-flush` | JSON harvested log | tmp+rename 节流写 | 有 | startup hydrate 已接；无 `Last`/`AppExit` 强制写 | `server/src/spiritwood/mod.rs:57-79`；`server/src/spiritwood/persistence.rs:78-205,208-287` | P3 shutdown registry/flush |
| 6 | `status-effects-consumable-persistence` | —；`StatusEffects` 仅 component | — | — | join 插 `default()` | `server/src/combat/components.rs:404-415`；`server/src/combat/mod.rs:171-176` | P4 长期 buff/player Slice |
| 7 | `supply-coffin-cooldown-restart-rollback` | —；`SupplyCoffinRegistry` 仅内存 | — | — | startup `new()` | `server/src/supply_coffin/mod.rs:111-220,252-294` | P4 supply-coffin runtime |
| 8 | `tiandao-attention-persistence` | —；`TiandaoAttention` 仅 ECS | — | — | 缺 component 时插 `default()` | `server/src/world/tiandao_hunt.rs:53-75,479-496` | P4 player/runtime Slice |
| 9 | `zone-influence-shutdown-flush` | SQLite `zone_influence` | periodic upsert 已有 | 有 | startup hydrate 已有；`Last` 只强刷 zone runtime，未强刷 influence | `server/src/persistence/mod.rs:698-712,723-847,926-977,2024-2042,3680-3820` | P3 shutdown registry/flush |
| 10 | `dormant-redis-dirty-ack` | Redis HASH `NPC_DORMANT_REDIS_KEY` | `take_dirty()` 先清、fire-and-forget send | startup `HGETALL` restore | 写失败只 warn，无 ACK/re-dirty | `server/src/npc/dormant/mod.rs:392-450,588-645`；`server/src/network/mod.rs:1320-1344`；`server/src/network/redis_bridge.rs:562-565,1793-1829` | P3 Redis write authority/ACK |
| 11 | `coffin-autosave-inflight-race` | 既有 player/coffin SQLite slices | 多条 direct save | 有 | disconnect/shutdown/autosave 已接；无 production revision/CAS handoff | `server/src/player/state.rs:652-830`；`server/src/player/mod.rs:463-506,535-688,703-884`；`server/src/coffin/mod.rs:661,716,823,973,1210-1263` | P3 write authority + revision/CAS |
| 12 | `identity-persist-key-mismatch` | SQLite `player_identities` | command/revealed/social 按 runtime `Lifecycle.character_id` 写 | join 按 `canonical_player_id(username)` 读 | 两侧 key source 均存在，但尚无全链同值不变量 pin | `server/src/persistence/identity.rs:36-102`；`server/src/identity/mod.rs:307-333`；`server/src/identity/{command,revealed}.rs:311-441,79-109`；`server/src/social/mod.rs:1514-1524,1573-1603` | P2 identity key/load contract |
| 13 | `mineral-exhausted-log-corrupt-revival` | 同 mineral JSON | 直写最终文件 | parse error 被当 empty log | startup hydrate 已接，损坏可令矿脉复活 | `server/src/mineral/persistence.rs:115-173,220-259`；`server/src/mineral/mod.rs:69-108` | P3 atomic/corrupt-safe persistence |
| 14 | `spirit-eye-runtime-persistence` | —；`SpiritEyeRegistry` 仅 runtime | — | — | zones + `startup_salt()` 重建 | `server/src/world/spirit_eye.rs:40-69,109-145,232-268,350-363,592-606` | P4 world-runtime Slice |
| 15 | `voidaction-cooldown-runtime-tick-restart` | SQLite `void_action_cooldowns` | 有 | 有 | startup hydrate 已接；`ready_at_tick` 是绝对 runtime tick | `server/src/persistence/mod.rs:723-775,1720-1729,2868-2957`；`server/src/cultivation/void/actions.rs:290-294` | P3 time rebase |
| 16 | `r1-mechanical-fixes` P6 NPC deceased archive DB-open rollback | SQLite `npc_deceased_index` + zstd archive bundle | production `persist_npc_deceased_archive` 先写 bundle，再在 `persisted` 补偿闭包内打开 DB/transaction、upsert index + 删除 hot rows；任一步失败均恢复旧 bundle | `load_npc_deceased_archive` 可读 index + 解压/解码，但当前仅测试调用 | terminated NPC 由 periodic persistence system 归档；无 production archive rehydrate/restore consumer | `server/src/persistence/mod.rs:5931-5982`（`persist_npc_deceased_archive_with_hooks`） | 2026-08-05 R3 已补 DB-open/transaction-open rollback 并以失败原子性回归测试闭环；机械缺陷 handoff 已关闭，P1 仅保持该边界，NPC archive owner 仍决定 production restore 语义 |
| 17 | `r10-findings` #1 mineral shutdown flush | 同 mineral JSON | Update interval 已有 | startup hydrate 已有 | register 无 `Last`/`AppExit` | `server/src/mineral/mod.rs:69-108`；`server/src/mineral/persistence.rs:177-218` | P3 shutdown registry/flush |
| 18 | `wounds-relog-full-heal` | **无 Wounds durable schema** | **无** | **无** | join/rebirth 均 `Wounds::default()`，重连满血缺口仍在 | `server/src/combat/components.rs:73-102`；`server/src/combat/mod.rs:171-184,197-200`；`server/src/combat/lifecycle.rs:1803-1814,1916-1918` | P2 新建 player Slice/load guard；**不得与 Lifecycle 混记** |
| 19 | `player-slice-load-failure-clears` | 既有 player SQLite tables | 有 | 部分：KnownTechniques 有 `Loaded/LoadFailed` marker，其余多处 fallback/default | failure 后仍可能默认值覆盖 durable row | `server/src/player/state.rs:158,434-573`；`server/src/player/mod.rs:228-236,269-278` | P2 load guard + aggregate `WriteSet` omit |
| 20 | `shelflife-clock-restart-freshness` | `Freshness` 随 `ItemInstance`/SQLite `inventories.inventory_json` 持久化；无独立 clock schema | `save_player_inventory_slice`/玩家聚合保存序列化 `created_at_tick` | inventory load 原样反序列化 `Freshness` | relog 可恢复物品；restart 时 `GameplayTick::default()` 归零，`effective_dt_ticks` 的 `saturating_sub` 把旧绝对 tick 钳为 0；无 clock hydrate/rebase | `server/src/shelflife/types.rs:175-205`；`server/src/shelflife/compute.rs:246-256`；`server/src/player/state.rs:486-496,792-830,1372-1438`；`server/src/player/gameplay.rs:137-167` | P3 clock/time rebase |
| 21 | `recipe-unlock-shutdown-flush` | JSON `recipe_unlocks.json` | atomic tmp+rename | 有 | `AppExit → Last` 强制 flush 已接 | `server/src/craft/unlock.rs:17-18,180-205,220-259,266-301` | 已闭环；P5 仅归档 |
| 22 | `heiwushi-dormant-identity-loss` | Redis `NPC_DORMANT_REDIS_KEY` 序列化 `NpcDormantSnapshot`；snapshot 无黑武士专属字段 | 通用 dehydrate/Redis dirty flush 会保存 snapshot，但不捕获 `HeiwushiMarker/State`、`FaunaTag/VisualKind`、thinker | Redis restore + hydrate 已有；`NpcArchetype::Beast` 固定走普通 `spawn_beast_npc_at` | dormant/Redis restart wiring 存在，但 relog/hydrate 会把黑武士洗成普通 Beast | `server/src/npc/dormant/mod.rs:299-365,392-450`；`server/src/npc/hydrate/mod.rs:388-469,721-750`；`server/src/network/redis_bridge.rs:1793-1829` | NPC virtualization owner 补专属 roundtrip；R3 仅提供 adapter/ACK 原语 |
| 23 | `placeable-entity-restart-loss` | 无 placed-entity durable schema；`WorkbenchBlock`/`ContainerBlock`/`ExternalContainer.items` 仅 ECS | 放置先扣 inventory 并 spawn entity，无 placed record/content save | 无 | startup persistence bootstrap 不 hydrate workbench/container；restart 后本体与箱内内容丢失 | `server/src/world/block_place.rs:213-275,463-500`；`server/src/craft/workbench.rs:34-43,89-112`；`server/src/world/container_block.rs:119-171`；`server/src/inventory/external_container.rs:36-58` | world/container lifecycle owner 建 schema、hydrate 与原子 move；R3 仅原语 |
| 24 | `scatter-bead-burial-restart-loss` | 无 durable schema；`ScatterBeadBurials` 为内存 Resource | 埋设只 `insert` 内存 burial/ledger source，无 durable save | 无 | zhenfa register 每次 `ScatterBeadBurials::default()`；restart 后 trigger/excretion 均无记录 | `server/src/zhenfa/mod.rs:174-229,601-606,2489-2526,2570-2714` | zhenfa + qi-ledger owner 建稳定 owner/id、hydrate/flush 与守恒补算；R3 仅原语 |
| 25 | `surface-stash-lifecycle-volatile` | 无 durable schema；`SurfaceStashPlayerLimit`、`PoiRespawnStore` 与 `LootContainer.depleted` 仅内存/ECS | 搜索完成只改 `depleted` + 内存计数 | 无 | 同进程 respawn 只 mark refreshed、不恢复容器；restart startup scatter 重建 `depleted=false` 且限频归零 | `server/src/world/tsy_container_search.rs:168-220,377-388,589-599`；`server/src/world/poi_respawn_tick.rs:45-115`；`server/src/world/poi_novice.rs:614-701`；`server/src/world/tsy_container.rs:124-158` | TSY/onboarding owner 建稳定 POI lifecycle/限频 Slice；R3 仅原语 |
| 26 | `coffin-offline-reclaim-respawn-dup` | SQLite `player_lifespan.in_coffin/coffin_grade`；`CoffinRegistry` 本体仅内存 | disconnect `save_player_slices_with_coffin` 写 `in_coffin=true`，随后 runtime `clear_player` 清占用 | join load 后 `reclaim_occupied` 可从持久态重建 registry 记录 | relog wiring 存在但持久态与 runtime cleanup 冲突；缺棺时 `reclaim_occupied` 可凭空补 registry，marker 后续重生 | `server/src/player/state.rs:681-707`；`server/src/player/mod.rs:206-305,463-525`；`server/src/coffin/mod.rs:200-232,1040-1071` | coffin/session-lifecycle owner 裁决离线占用语义；P3 authority/handoff 只提供一致性原语 |
| 27 | `stale-spirit-niche-lifecycle` | SQLite `social_spirit_niches` 已存在 | `persist_social_spirit_niche` 已 upsert | owner 单行 load + startup `load_all_social_spirit_niches` hydrate 已有 | restart 会恢复所有旧角色 niche；新角色只清 runtime `spawn_anchor`，未删旧 owner 行；旧 niche reveal 按 username 清当前 shrine anchor | `server/src/persistence/mod.rs:1608-1631`；`server/src/social/mod.rs:397-417,2189-2278,2851-2944,3205-3244`；`server/src/combat/lifecycle.rs:1803-1863` | social/new-character-lifecycle owner 修角色轮换/删除与 reveal 校验；R3 仅原语 |

Lifecycle 是 #1289 已落地的独立生产 Slice 基线：SQLite `player_lifecycle` 已具备 save/load、combat join hydrate、disconnect/`Last`/autosave wiring（`server/src/persistence/mod.rs:2350-2380`；`server/src/player/state.rs:603-632,1951-2022,2262-2281`；`server/src/combat/mod.rs:96-148`；`server/src/player/mod.rs:463-517,535-688,867-884`）。其剩余问题是 load error 仍 warn 后 default，待 P2 纳入 `Failed` provenance/`WriteBlocked`；这与第 18 项 **Wounds 完全未持久化** 是两条独立事实。

第 22–27 项可以使用 R3 的 snapshot/hydrate/flush 原语，但通用持久层不得自称已完成其 schema、实体重建、所有权、守恒或角色轮换语义；各领域 owner 必须在自己的 production 链与验收中闭环。

## 文件所有权与边界

- 独占：`server/src/persistence/**`、`player/state.rs` + `player/mod.rs` 的 autosave/载入区段、各域持久化接线点。
- P0 返工边界：#1289 已合入；为关闭 production dead-wire，已授权修改 `server/src/persistence/mod.rs`、`server/src/player/state.rs` 与 `server/src/player/mod.rs` 的 KnownTechniques 精确接入区段，交付 canonical registry、zone-runtime shutdown descriptor 与首个 player production adapter；P1 机械拆分必须原样保留这些接入点。
- 不碰：session 业务逻辑（R1 经钩子接入）、qi 语义（R5）、`client_request_handler.rs`（R4）。已有 craft/inventory/cultivation/ledger/dropped-loot 跨表 transaction 不得拆成多连接写入。
- 不引入：全局 `rusqlite::Connection` Resource、`Mutex<Connection>`、异步全局 writer、所有时间统一 wall-clock、或新旧 shutdown hook 双注册。

## bot 验收场景

1. `restart_player_slices`：bot 建号→修炼/学功法/受伤→关服重启→重连→断言功法/伤势/濒死后果/buff 全部还原。
2. `same_tick_reconnect_handoff`：真实 lifecycle 在同一 schedule tick 投递同主体 disconnect+reconnect（含同 event ID 重复投递）→断言 all saves→全量只读 preflight→无 blocked/error 返回通道的旧 activation cleanup→lease check→all hydrates→all rebases 且事件只 mint/dispatch 一次；注入任一 save/preflight 失败或 blocked→断言两个旧 `GuardedSlice + DirtyTracker + PersistedRevisionFence` activation/lease 均完整保留且零 hydrate，解除后 clean retry；注入第二 hydrate 在真实 activation 后失败或 blocked→断言两个已尝试 descriptor 逆序 abort cleanup、零残留 lease，随后 clean retry；故意让 cleanup 返回但保留任一 lease→断言 `DuplicateSubject` fail-closed。
3. `restart_world_runtime`：触发矿脉枯竭/配方解锁/zone influence→SIGTERM 关服→重启→断言无回滚无复活。
4. `load_failure_guard`：注入一行损坏 slice 数据→启动→断言该玩家进入守护降级而非清零覆盖。
5. `tick_rebase`：带冷却/再生倒计时重启→断言按该 slice 的 offline policy 折算，首个 live tick 不重复计时。
6. `restart_qi_conservation`：带 `qi_runtime_accounts` 与原 zone/account 身份保存→重启/同 tick handoff/失败加载→以 `summarize_world_qi` 前后快照和 `assert_conservation(..., era_decay = 0)` 断言不重复记账、不把 unknown 当 0、不迁错账户。

## 开放问题（pre-P0 收口）

1. 载入守护的玩家体验：只读降级 vs 拒绝进服 vs 回滚到上一备份？
2. 迁移链是否借机做一次 squash？

以上问题均已在下节收口。原表保留以备追溯，**实施时以 §P0.1 决议为准**。

## §P0.1 决议（pre-P0 收口，2026-07-27）

### #1 Slice 形态：静态 descriptor + exclusive-world adapter

**决议**：
1. `PersistenceSlice` 只返回一个静态 `SliceDescriptor`；registry 保存静态 descriptor 并按 `(order, id)` 稳定排序。会修改运行态的 hook 使用无捕获函数指针 `fn(&mut World, &SliceRunContext) -> SliceRunResult`，由 adapter 在 hook 内拿强类型 Resource/Query；重连 `reconnect_preflight` 单独使用 `fn(&World, &SliceRunContext) -> SliceRunResult`，在类型层禁止破坏旧 activation。
2. descriptor 必须按实际 `SliceDescriptor` 形状声明 `id`、`scope`、`order`、`load_failure`、`time_basis`、`write_binding`、`write_ordering`、`autosave`，以及可选的 `hydrate`、`reconnect_preflight`、`reconnect_cleanup`、`rebase`、`disconnect_save`、`shutdown_flush` hook；重复/空 Slice ID 在注册时 fail-fast。任何声明 rebase 的玩家 descriptor 必须同时声明 hydrate，使失败回滚集合覆盖所有可能建立新 activation 的 hook；禁止 rebase-only reconnect participant 绕过 abort cleanup/lease check。`write_binding` 同时固定 write domain 与 write authority，不另设可漂移的 `write_domain` 字段。
3. 不把泛型 `SystemParam`、`Query`、`ResMut`、关联类型或 `rusqlite::Connection` 装进 trait object。若后续确需动态 driver，再以同一对象安全签名包装，不推翻 P0 descriptor。

**落点**：`server/src/persistence/mod.rs:103-110`、`server/src/player/state.rs:278-290`（现有资源只拥有路径/元数据）；plan §阶段 P0、§接入面。

### #2 Load guard：读取结果与写资格分离

**决议**：
1. 统一三态由 opaque `SliceLoad<T, E>` 表达：公开 API 仅有 `missing`、`loaded`、`failed` 构造器与只读 `SliceLoadStatus` 查询；payload-bearing `Missing | Loaded(T) | Failed(E)` state 留在 persistence trust boundary 内。只有连接成功、查询成功且确认无行才是 `Missing`；连接/SQL/解码/校验失败一律是 `Failed`。
2. `Missing` 与 `Loaded` 可写；`Failed` 即使为维持会话创建 runtime default，也必须携带 `WriteBlocked` provenance。即时 Changed、周期 autosave、disconnect、shutdown、export 与聚合 transaction 全部只能通过 `GuardedSlice::write_permit` 取得不可直接构造的 writer permit；公开 API 不提供丢弃 load provenance 的拆解入口，新会话成功重载才恢复写资格。
3. 默认体验为**按 slice 降级**：展示/非关键 slice 可只读运行；core、inventory、craft、lifespan/Lifecycle、cultivation/qi 等高价值或跨 slice mutation 在依赖集读取失败时拒绝可变 gameplay。关键世界账本使用 `RefuseStartup`。自动回滚整库备份不进入在线加载路径，只保留人工审计恢复。

**落点**：`server/src/player/state.rs:173-182`、`server/src/player/mod.rs:463-501,645-683`（KnownTechniques 正向范式）；`server/src/player/state.rs:434-573`（待推广 fallback）；plan §阶段 P2、§bot 验收场景 #4。

### #3 Shutdown registry：复用 `AppExit → Last`，失败隔离

**决议**：
1. 不重造 signal handler。唯一触发仍是 `shutdown.rs` 在 `PreUpdate` 发出的单次 `AppExit::Success`；统一 dispatcher 位于 `Last`。2026-08-04 返工已将 zone-runtime 旧 `Last` hook 原位迁为首个 production descriptor，并安装 canonical registry + dispatcher；后续域继续逐个迁移。
2. dispatcher 复制已排序 descriptor 列表后再逐个调用 hook，避免持有 registry 的 World borrow 时再次可变借用 World；返回汇总报告 `attempted/clean/flushed/blocked/failures`。
3. 单个 hook 失败不得中断后续 slice；失败必须保留 dirty 和旧文件/旧行。P0 已注册 zone-runtime 与 KnownTechniques 两个 production descriptor，并移除其对应旧写入接线；P3 只对其余域逐个执行“移除旧 hook → 注册 registry”，禁止重复注册或双写。

**落点**：`server/src/shutdown.rs:43-74,247-375`；`server/src/craft/unlock.rs:282-301,1209-1337`；`server/src/persistence/mod.rs:921-977`；plan §阶段 P3、§bot 验收场景 #3。

### #4 Tick rebase：每个 Slice 声明时间语义

**决议**：
1. `TimeBasis` 至少区分 `None`、`RemainingLogicalTicks`、`WallDeadline`、`ObservedAgeWithElapsed`；禁止持久化一个“跨进程永不归零”的全局 runtime clock，也禁止把所有字段粗暴改成 wall-clock。
2. deadline 持久化 `remaining_ticks + saved_at_wall + offline_policy`：online-only 重建为 `new_tick + remaining`；offline-continuous 先按 `MILLIS_PER_TICK` 扣除 wall elapsed，再重建本地 deadline。age/elapsed 使用 observed age + pending elapsed；history/audit tick 不参与新进程 deadline 比较。
3. rebase 在 hydrate 后、首个 live Update 前只执行一次。旧 raw deadline 无法精确恢复时必须写明保守迁移，不得伪造精确值。

**落点**：`server/src/time.rs:1`；`server/src/persistence/mod.rs:773-784,2860-2959`（void action hydrate 与存取/恢复反例）；`server/src/mineral/persistence.rs:57-71,180-259`（mineral 反例）；`server/src/world/heartbeat.rs:491-584,3025-3153`（正向范式）；plan §阶段 P3、§bot 验收场景 #5。

### #5 Autosave 竞态：write authority + dirty revision/CAS

**决议**：
1. 每个 `WriteDomain` 的 mutation 递增 `DirtyRevision`；canonical registry 以 `(WriteDomain, PersistenceSubjectKey)` 签发独占 activation lease，同一 durable 主体不得重复激活出第二套 writer state；每个 active `GuardedSlice` 只能一次性联合恢复唯一一对 `DirtyTracker + PersistedRevisionFence`，初始 revision 由 persistence-private activation 注入，禁止 gameplay 指定、失败写后重铸 clean tracker或让 autosave/shutdown 各持分叉 tracker。tracker 只能凭同一 subject 的 `WriteBinding(domain + authority)` write permit 原子捕获 `payload + revision + outlet`；写失败不产生 receipt、永不清 dirty；writer 成功后由 persistence-private durable capability + `PersistedRevisionFence::commit` 产生不可直接构造的 subject-bound `DurableWriteReceipt`，tracker 仅消费匹配同一 subject 与当前 revision 的 receipt 才能 ack clean。
2. revision 只保护内存 dirty acknowledgement，不能单独阻止旧 snapshot 晚到覆盖数据库。registry 对同一 domain 强制唯一 authority 和一致 ordering；registry 构造、注册、lookup token 与 `SliceLoad::activate` 全部封闭在 `crate::persistence` trust boundary，外部 gameplay 只能声明静态 descriptor，不能构造 shadow registry 签发降级 token；`GuardedSlice` 再从 canonical lookup 固定 `write_ordering` 并原样传给唯一 fence。每个 domain 选择单一串行 writer，或由 `DurableWriteRequest` 把 expected persisted revision 纳入 SQL CAS/单调拒绝。
3. 字段写权威必须明确：事件拥有的字段不得被周期快照重新断言。跨 inventory/session/cultivation/ledger/dropped-loot 的原子 checkpoint 保持领域 transaction，不拆散。

**落点**：`server/src/persistence/slice.rs:293-412,655-697,770-792,1149-1195`（canonical registry trust boundary、load activation、唯一 persistence state 与 durable commit）；`server/src/coffin/mod.rs:656-666`、`server/src/player/mod.rs:773-805`、`server/src/player/state.rs:670-697,780-830`（P3 生产迁移锚点）；plan §阶段 P3。

### #6 迁移链与 P0 范围：不 squash，只落纯契约

**决议**：
1. 不重置 `PRAGMA user_version`，不删除 v1–v39 legacy upgrade path，不把行为迁移 squash 成一份 fresh schema。未来可额外生成新库 baseline，但旧库升级链、升级前备份和 fixture 必须长期保留。
2. P0 在 2026-08-04 返工前只新增 `server/src/persistence/slice.rs` 与 contract-pin tests，并在 `persistence/mod.rs` 增加模块声明；返工为关闭 production dead-wire，安装唯一 canonical registry 与 `AppExit → Last` dispatcher，将既有 zone-runtime 关服 hook 原位迁为首个 world production descriptor，并以 KnownTechniques 接通首个真实 player reconnect/load guard/dirty snapshot/durable fence adapter；不修改 schema、不拆巨石。#1289 已合入；#1259 的玩家/饱食度其余接线继续避让。
3. P0 pins 覆盖：registry ID/authority/ordering 校验与稳定排序；无 shutdown 请求不调用；失败隔离；load 三态、`RefuseStartup` 与不可伪造 write permit；deadline 两种 offline policy 与边界；domain-bound dirty snapshot + durable receipt；同 tick save-before-load；注入时钟。

**落点**：`server/src/persistence/mod.rs:57-62,1083-2386`；plan §P0 表域普查、§阶段 P0、§文件所有权与边界。

### #7 同 tick 断线保存 / 重连载入顺序：保存先于载入（#1289 review 继承项）

**决议**：
1. 同一持久化主体在同一 schedule tick 内出现 disconnect 与 reconnect 时，必须同步完成旧实体的 disconnect save，成功后才允许新实体 hydrate；保存失败则跳过载入并保留失败，禁止从旧 durable row 重建后继续运行。
2. P0 以 `dispatch_reconnect_handoff` 冻结该次序：registry 内同一玩家主体的所有 `SliceDescriptor::disconnect_save` 先按稳定顺序串行完成；只有全部返回 `Clean | Flushed` 后，才对所有参与玩家重连的 descriptor 执行**只读** `reconnect_preflight`。任一 preflight blocked/failed 时，旧 activation/lease 全部原样保留且零 hydrate；全量 preflight 成功后才用独立、无 blocked/error 返回通道的 `reconnect_cleanup` 提交删除旧 activation（panic 视为 adapter 契约违规并 fail-fast）。所有 hydrate 成功后才按同一时钟快照运行 rebase。入口消费 persistence-private 的一次性 `ReconnectHandoffToken`，同一 generation 不可重复执行；hydrate/rebase fail-fast，失败或 blocked 时对所有已尝试 hydrate descriptor（包括 hook 激活后才返回失败者）以 `ReconnectAbort` 逆序调用同一 cleanup，随后复核所有 subject/domain lease 均已释放；残留以 `DuplicateSubject` fail-closed，干净时才允许同一稳定主体重试。
3. P0 已以 KnownTechniques 把真实 player lifecycle 接到一次性 handoff dispatcher；P2/P3 只按各自阶段向其余玩家 Slice 推广 subject-bound activation、稳定 reconnect event ID 去重与 write authority。推广不得依赖 Bevy 系统注册先后、deferred commands 或“通常下一 tick 才重连”的时间假设；一次性 generation 只约束已经 mint 的 token。
4. 所有该主体/domain 的 disconnect save 成功后、任何 hydrate 前，adapter 必须把“能否同步释放旧 activation”的可失败检查放进只读签名 `reconnect_preflight: fn(&World, ...)`；registry 校验所有参与玩家 reconnect 的 descriptor 必须同时提供 preflight 与无 blocked/error 返回通道的 `reconnect_cleanup`。只有全量 preflight 成功后 dispatcher 才统一 cleanup；cleanup 必须同步释放对应 state、可幂等处理 `ReconnectCleanup`（旧 activation 提交）与 `ReconnectAbort`（本轮新 activation 回滚），panic 视为 adapter 契约违规。cleanup/abort 返回后若仍残留 lease，以 `DuplicateSubject` fail-closed，禁止框架强制 revoke 后并存双 writer。

**落点**：`server/src/persistence/slice.rs` 的 `SliceRunReason::{DisconnectSave,ReconnectPreflight,ReconnectCleanup,ReconnectLoad,ReconnectAbort}`、`ReconnectActivationCapability`、`SlicePreflightHook`、`SliceDescriptor::{disconnect_save,reconnect_preflight,reconnect_cleanup}`、`dispatch_reconnect_handoff` 与真实 activation contract pins；plan §阶段 P0/P2/P3、§bot 验收场景 #2。

### #8 时间 / deadline 测试：只用注入时钟（#1289 review 继承项）

**决议**：
1. Slice dispatcher 不直接读取 wall clock；统一消费 `SliceClock` 注入的 `runtime_tick` 与 `wall_unix_millis`。测试使用 `FixedClock`，精确固定边界两侧的毫秒值。
2. deadline rebase helper继续接受显式时间参数；contract pins 禁止调用 `SystemTime::now()`、`Instant::now()` 或依赖测试执行恰好未跨秒的 exact assertion。
3. 生产 adapter 在调用边界采样一次时间后注入；同一 dispatch 内复用该快照，避免一次操作跨秒得到不一致字段。
**落点**：`server/src/persistence/slice.rs` 的 `SliceClock`、`dispatch_shutdown_flushes`、`dispatch_reconnect_handoff`、显式时间参数 rebase helper、`FixedClock` 与 deadline contract pin；plan §阶段 P0/P3、§bot 验收场景 #5。

## §10 实施工作流

本 plan 继续按依赖顺序序列化 P1–P5；每个阶段独立 PR，前一阶段合入 `origin/main` 且门禁全绿后才进入下一阶段，不拆成新的 persistence 总体 plan。

1. **PR-P1 框架落地 + 机械拆分 + M-04/M-12 guard/checkpoint 持久化**：前置为 #1259 合入并解除 `persistence/mod.rs`、`player/state.rs`、`player/mod.rs`、`combat/lifecycle.rs` 避让。机械移动迁移/查询代码并保留 P0 已安装的唯一 canonical `PersistenceSliceRegistry`、zone-runtime 与 KnownTechniques production wiring，迁移链、transaction、错误行为和连接 ownership 不变；同时按 master §4.2 M-10 登记落地 M-04/M-12：S-07 `ReconnectGuard` 与 Suspended checkpoint 同事务持久化、`CraftRestoreGuard` control frame 持久化。P1 必须同时落地并跑通能证明拆分后生产路径未断的 bot/integration acceptance；没有对应绿证据不得合入。
2. **PR-P2 玩家 load guard 推广**：依赖 P1；以 P0 的 KnownTechniques adapter 为范式，按价值域逐批接 Lifecycle、Wounds、core/position/inventory/cultivation/craft 等其余真实 adapter，所有连接/SQL/解码失败保留 `Failed` provenance，聚合 transaction 按 `WriteSet` omit 被阻断 slice。P2 必须在本 PR 之前或伴随落地并跑通 `restart_player_slices` 与 `load_failure_guard`（含 Wounds/功法/buff 恢复、失败不清空且写入被阻断）；缺任一对应主验收场景或证据不得合入。
3. **PR-P3 shutdown/reconnect/time/write authority 推广**：依赖 P2；对 P0 尚未迁移的域逐个执行“移除旧 hook → 注册 registry”，推广 `AppExit → Last`、一次性 reconnect handoff、deadline rebase、payload-bound snapshot 和 serialized/CAS writer，禁止重复安装 registry、重复注册 P0 descriptor 或新旧 hook 双写。P3 必须在本 PR 之前或伴随落地并跑通 `same_tick_reconnect_handoff`、`restart_world_runtime` 与 `tick_rebase`；缺任一对应主验收场景或证据不得合入。
4. **PR-P4 遗漏运行态持久化**：依赖 P3；补 ActiveEvents、TiandaoAttention、长期 consumable/buff、realm taint、season override、supply cooldown、灵眼等真实 Slice；实体重建、所有权与守恒仍归领域 owner。P4 必须在本 PR 之前或伴随落地并跑通 `restart_world_runtime`（涵盖新增 world-runtime 域）与受影响领域的集成验收；缺主验收场景、领域 owner 证据或守恒/所有权验收不得合入。Bot 主验收是总纲要求的绑定 gate，不得推迟到 P5 首次补齐。任何新增/迁移的生产 Slice 都必须在所属阶段绑定至少一个可执行的 `scripts/bot/scenarios/` 或等价协议集成场景。其余 `restart_qi_conservation` 作为 P3/P4 受影响域的伴随验收，不能在 P5 才首次执行。
5. **PR-P5 restart Bot 验收 + 吸收归档**：依赖 P4；跑 `restart_player_slices`、`same_tick_reconnect_handoff`、`restart_world_runtime`、`load_failure_guard`、`tick_rebase`、`restart_qi_conservation`，并等待第 22–27 项的领域 owner 依赖全部满足后，才可核验 27 项吸收清单、补 `## Finish Evidence` 并归档到 `docs/finished_plans/`。第 22–27 项不是 R3 框架完成项：#22 依赖 NPC virtualization owner 保留黑武士身份的 Redis/restart round-trip；#23 依赖 world/container lifecycle owner 完成 placed entity 与容器内容的 durable schema、hydrate 和原子迁移；#24 依赖 zhenfa + qi-ledger owner 完成散落珠埋设记录 hydrate/flush 与 `summarize_world_qi`/`assert_conservation`（`era_decay = 0`）验收；#25 依赖 TSY/onboarding owner 完成 surface stash 的 depleted/respawn/限频生命周期持久化；#26 依赖 coffin/session-lifecycle owner 证明离线 reclaim 不重复占用且 ownership transition 一致；#27 依赖 social/new-character-lifecycle owner 完成 stale niche 删除、角色轮换隔离和 reveal 校验。每项都必须有对应 owner fix PR 已合入 `origin/main`，并有覆盖上述生产行为的 Bot/E2E 或等价集成验收证据；任一依赖未闭环，P5 可以保留实现 PR，但不得追加 Finish Evidence、不得把该项标为已吸收、不得执行 plan 归档，plan 继续保持 active。

每个 PR 由独立 fresh-context `claude` 实施 subagent 在本 R3 worktree 完成实现、测试、commit/push/PR；每个逻辑单元必须使用中文 atomic commit，每个 agent 生成的 commit 必须写入真实执行模型 ID 的 `Model:` trailer 与 `Co-Authored-By: Claude <noreply@anthropic.com>`，该 commit trailer 与启动配置中的 `model: "opus"` 是两项独立门禁、不得混用或省略。主线协调器只接收 200–500 token 结论并负责跨 PR 编排与 review 等待，实施 subagent 不跨调用等待 review，也不得并行实施相邻阶段。启动配置遵循 `Agent(subagent_type: "claude", model: "opus", prompt: "<本 PR 精确范围、前置依赖、门禁与禁改边界>\n\nultrathink")`；共享本 worktree，不创建 nested worktree。返工使用新的独立 subagent，从 PR 精确 HEAD 继续且不得重复 promotion/归档。

每个 PR push 前执行 `git fetch origin && git merge origin/main`，重跑受影响栈门禁与 Bot E2E，并对精确 HEAD 启动 fresh-context adversarial validator。Push 后独立评论 `/review` 并等待 `/review` 与 CodeRabbit 收敛；CodeRabbit pending 时用 `ScheduleWakeup(1200)`，最多三轮无进展才交人工，禁止 sleep/busy poll。Review 有修改意见时由新返工 subagent 修复、对新 HEAD 重跑 validator/门禁/推送并重新触发 `/review`、等待复审；前一 PR 未收敛不得启动下一阶段。不得以 P0 contract test 代替后续生产接线验收。

### 单次 consume-plan 全自动到 merge

用户发起一次 `/consume-plan plan-refactor-persistence-slices-v1` 后，consumer 依次完成当前未完成阶段的实现、locked gate、Bot E2E、精确 HEAD validator、push、PR、独立 `/review`、返工复审和 merge；每个阶段 merge 后从最新 `origin/main` 继续下一 PR。只有真实用户决策、#1259 等外部依赖未满足或基础设施持续不可用时才暂停；P5 全绿后自动补 Finish Evidence、归档 plan 并提交最终 PR。

## R3 P1 本 PR 证据（2026-09-06，拆分 + review 修复）

- 本 PR **已不再是纯机械移动**：先完成 persistence 生产码按域拆分，再在当前 PR 内修复 Kody 18 条 inline 记录归并出的 9 个既有缺陷，并为每条真实缺陷补最小回归锁定。`server/src/persistence/mod.rs` 仍只保留模块声明、跨域装配、canonical `PersistenceSliceRegistry`、`AppExit → Last` dispatcher、zone-runtime descriptor 与 KnownTechniques production wiring。
- 拆分落点与最终行数为 `mod.rs` 214 行、`models.rs` 533 行、`known_techniques.rs` 973 行、`bootstrap.rs` 407 行、`migrations.rs` 1908 行、`void_actions.rs` 121 行、`agent.rs` 442 行、`tribulation.rs` 461 行、`world.rs` 1180 行、`world_qi.rs` 125 行、`player.rs` 306 行、`npc.rs` 1307 行、`life.rs` 1058 行、`social.rs` 301 行、`helpers.rs` 444 行、`epitaph.rs` 80 行，全部小于 3000 行。
- 初始拆分的机械等价性核验：以原 `mod.rs` 与拆分后全部 persistence 源文件分别提取顶层函数名和类型/常量名对拍，均为 251/251 与 118/118 完全一致；初始阶段除模块导入、`pub(super)` 父模块可见性及必要测试字段可见性外，没有生产逻辑改写。下列行为变更均是本 PR 明确纳入的 review 修复，不能再宣称本 PR 行为不变：
  - `helpers.rs`：归档发布由可覆盖的 `fs::rename` 改为同目录 `fs::hard_link` + 临时文件清理，目标已存在时 `AlreadyExists` 且不覆盖；归档 deceased/digest 的 `char_id` 统一经过拒绝空值、`.`、`..`、分隔符、NUL 与绝对路径的组件校验；回归覆盖 no-replace、路径遍历与临时文件清理。
  - `known_techniques.rs`：重试清理改为比较 retry 键与当前仍有工作的 subject 集合，仅清除不再 pending 且没有断线保存失败待重试的 subject；回归覆盖 stale retry 被清除、pending retry 保留，以及无 reconnect handoff 的断线保存失败仍保留 retry 并使用首个退避帧。
  - `npc.rs`：deceased/digest 归档入口复用安全组件校验；归档写入/DB 失败且 rollback 再失败时以结构化聚合错误保留 primary source 与 rollback 诊断；无替换发布失败时只清理本方临时文件，不删除并发发布者已建立的 deceased 或 digest 目标；回归覆盖路径边界、双失败 source，以及两条归档路径的竞争发布目标保留。
  - `world.rs`：`persist_zone_influence_snapshot` 在既有事务内先清除旧 `zone_influence` 行，再写入当前全量快照；回归覆盖移除 zone/player 后 hydrate 不复活陈旧行。
  - `world_qi.rs` + `qi_physics/ledger.rs`：runtime qi hydrate 改走 ledger 的固定持久 owner 集合恢复接口，完整校验缺失、重复、未知、非法值与总量可表示性，不伪造 `QiTransfer`；回归覆盖 restart 守恒、无审计伪造和失败无部分 hydrate。
  - `player.rs` + `player/state.rs`：删除 durable dropped-loot row 前读取并校验 durable payload，要求接收 `PlayerInventory` 持有同一 instance（仅允许既有 Pickup attrition 降低 `spirit_quality`）；无 ownership proof 时保留 row、整笔 checkpoint 失败。拾取是同一 item 的 ground→inventory 所有权迁移，不重复释放 qi；回归覆盖无 proof 拒绝及成功拾取。
  - `void_actions.rs`：`ready_at_tick` 从 SQLite `i64` 到 runtime `u64` 改为受检转换，负值映射 `InvalidData` 并 fail-closed；回归覆盖负 tick 不被解释为 `u64::MAX`。
  - `migrations.rs`：legacy `player_core.spirit_qi` 改用 `qi_physics::finite_non_negative`；负值/非有限值拒绝，不再 clamp 或静默跳过；该字段继续恢复到玩家 ECS cultivation，不创建伪造 ledger account/transfer；回归覆盖负值迁移事务回滚。
- 以上修复未改变迁移版本链、表结构、既有跨表事务边界或连接 ownership；保留 R3 P0 已安装的 canonical registry、`AppExit → Last`、zone-runtime shutdown descriptor、KnownTechniques reconnect/load guard/dirty snapshot/durable fence adapter。zone influence 的清除发生在原有全量快照事务内，dropped-loot ownership proof 发生在原有 inventory+drop+zone checkpoint 事务内。
- 精确 HEAD 的无上下文 validator 复核初轮修复后又发现并纳入本 PR 的 4 条边界路径：KnownTechniques 断线保存失败且无 handoff 时 retry 会被 stale cleanup 误删；runtime qi hydrate 会静默忽略未知 durable account；NPC deceased hard-link 发布失败的回滚窗口可能删除并发发布者目标；NPC digest sweep 的同类 no-replace 失败路径也可能删除并发发布者目标。修复分别保留 retry 工作项、对未知 qi account fail-closed、以及禁止失败方删除非其所有的 deceased/digest 归档目标，并各自增加生产/竞争回归测试；因此本 PR 的 review 修复总数为首轮 9 项加二次复核 4 条边界路径。
- 本 PR **不含 M-04/M-12 guard/checkpoint 持久化**（`ReconnectGuard`、Suspended checkpoint、`CraftRestoreGuard` control frame）；因此 R3 P1 总体仍保持未完成，不将本证据写成阶段完成或 production 接入扩展。
- 定向与完整 server gate、最终 merge 后复验、fresh-context validator、CI/e2e 与当前 HEAD Kody 复审结果将在本 PR 最终 HEAD 确认后补记；本条目诚实记录为“拆分 + review 修复”，不粉饰为行为不变。
