# plan-bughunt-r3-findings-v1（骨架）

> **骨架（草案）**。一句话主题：代码库自检 bug-hunt **round3**（fresh origin/main worktree @ `341fc4461` 为 ROOT，换角度：inventory/loot 守恒 · 离屏 hydrate 复活吞真元 · 视听完整性 · persistence 竞态 · 状态机生命周期）确认的 **6 个新真 bug**——含 **1 critical（hydrate 复活逻辑死亡 NPC + 真元双计，守恒红线）**。已对 r1+r2 去重，全部 real-on-main。

> 立项动机：round3 用修正后的方法论（**fresh origin/main worktree 为 ROOT**，不再扫主仓 stale 工作目录），9 候选 → 怀疑者对抗 → opus 逐条 Read/Grep 复核裁决，6 REAL / 3 NOT_REAL（NOT_REAL：sword-path AV 已 plan defer / coffin_grade TOCTOU 同步链无并发 / tribulation_kind wildcard 无构造点，宁漏不误报已剔除）。

## 阶段总览（按主题分组，逐项独立可修）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 hydrate 复活逻辑死亡 NPC + 真元双计（守恒 critical） | plan_skeleton | ⬜ |
| P1 | inventory/loot 守恒（TSY 搜刮 loot 全压 (0,0)） | fix_pr | ⬜ |
| P2 | persistence 竞态 / 生命周期孤儿（DEFERRED race + JueBi 断线） | fix_pr + plan_skeleton | ⬜ |
| P3 | 视听完整性（渡劫成功 + VoidPath 涡流五招） | plan_skeleton + fix_pr | ⬜ |

## P0 — 🔴 hydrate 复活逻辑死亡 NPC + 真元双计（critical，守恒）

- **#2 critical（守恒，plan_skeleton）**：`server/src/npc/hydrate/mod.rs:119-130` 的 `to_hydrate` 循环对 `store.snapshots` **无 `!combat_dead_pending_release` 守卫**，而 `dormant/mod.rs:763-773` 注释明确把该标记视为逻辑死亡并 early-continue，`collect_zone_combat_pairs` 也过滤它。触发链：
  - zone 满时 `release_dormant_qi_to_zone`（`dormant/mod.rs:1583-1620`）把 overflow **同时**留在 `snapshot.cultivation.qi_current` 与 ledger `npc:{char_id}` 账户；
  - `in_player_zone`（line 126）无距离限制，玩家进入离屏战场所在 zone 即把战死者快照 `store.remove` **复活**为活体 ECS entity（`spawn_from_snapshot` 注入携 overflow 的 Cultivation）；
  - 此后 `run_pending_combat_release_retry`（1038，filter `combat_dead_pending_release`）在 store 中再也找不到该快照，`npc:{char_id}` 余额**永不清零**（全仓无 `remove_balance(npc:{char_id})`）；
  - `summarize_world_qi`（`ledger.rs:443-467`）的 `player_qi`（累加所有 Cultivation.qi_current）与 `ledger_qi`（累加所有 ledger 余额）是 `total_observed` 的**独立加项** → 复活后 overflow **双计**，违反守恒。
  - 修：hydrate 循环加 `!combat_dead_pending_release` 过滤（对齐 dormant/collect_zone_combat_pairs）；并在 release 路径对账（snapshot 残余 qi 与 ledger 余额二选一为准，不重复持有）。**守恒关键时序，report-only roadmap，consume 时定 hydrate 过滤 + ledger 对账方案。**

## P1 — inventory/loot 守恒（TSY 搜刮）

- **#1 major（fix_pr）**：`server/src/world/tsy_container_search.rs:707-711` `place_item_in_main_pack` **无条件 push `row:0,col:0`**，且 `tick_search_progress` 的 loot 循环（576-578）对每个 loot item 调用一次。`roll_loot_pool` 返回 `Vec<ItemInstance>`（`loot_pools.json` 存在 `rolls:[1,2]`，多物品容器真实存在），加 `maybe_spawn_jizhaojing` 可再 push 一件 → 多件全部落同一 `(0,0)`，完全绕过 `inventory/mod.rs:1436` `find_free_slot` 空间排布（正路 `add_item_to_player_inventory_inner:1366` 走的）。`schema/inventory.rs:2561-2571` 明确校验 footprint 不得 overflow/overlap → 多件落 (0,0) 违反 grid 完整性不变量，UI 按 row/col 渲染时第一件之后对玩家**不可达**（物品丢失）。修：`place_item_in_main_pack` 改调 `find_free_slot`。**局部明确。**

## P2 — persistence 竞态 / 生命周期孤儿

- **#7 major（fix_pr）**：`server/src/persistence/mod.rs:2768-2779` `release_ascension_quota_slot` 对 `occupied_slots` 做 read-decrement-write 却用**默认 DEFERRED** `connection.transaction()`。姊妹函数 `try_complete_tribulation_ascension`（2692-2766）对同款 read-check-write 显式用 `transaction_with_behavior(Immediate)` 并注释（2699-2705）解释 WAL 下 DEFERRED 先读后写危害；`mod.rs:12090-12130` 多线程 Barrier 压测证明这些函数**确从多线程并发调用**、每次 open 独立 Connection 无 Mutex 串行化。生产 caller 在死亡链（`death_hooks.rs:143/216`、`lifecycle.rs:1408`、`cultivation/mod.rs:804`）→ 两 Void 玩家同时死亡并发触发 release → DEFERRED 第二写 commit 时 `SQLITE_BUSY_SNAPSHOT` 失败被 warn 吞 → release 静默丢失 → §三:78 化虚名额 `occupied_slots` 永久虚高、静默阻塞后续渡劫。修：改 `transaction_with_behavior(Immediate)` 对齐姊妹函数（一行）。**局部明确。**
- **#8 major（plan_skeleton）**：`server/src/cultivation/tribulation.rs:3271` `abort_du_xu_on_client_removed`（全仓唯一触及 `TribulationState`/`delete_active_tribulation` 的 `RemovedComponents<Client>` 处理器）在 3303-3305 对 `kind != DuXu` 直接 continue，跳过 `delete_active_tribulation`/remove component/settle event。但 JueBi 确为玩家可触发：`dispatch_pending_juebi`（1160-1240）对带 `Username` 的 entity insert `TribulationState{kind:JueBi}` + `JueBiRuntimeContext` 并 `persist_active_tribulation` 写 `active_tribulations` 行（1204-1206），sources 含 `VoidQuotaExceeded`/`WoliuVortexHeart` 等玩家驱动路径（persistence 测试 8873/8905 确认 JueBi 写 SQLite）。清理仅在 `juebi_settlement_system`（1849）的 `phase==Settle` 分支 delete（1982），需玩家在线推完全部 wave。玩家在 Omen/wave 期间断线 → entity despawn → settlement 永不触发，disconnect handler 又跳过 JueBi → **SQLite 行孤儿 + 组件泄漏**，`load_active_tribulation_count` 计入孤儿行可阻塞后续渡劫资格。**需设计决策（断线 abort vs 重连 restore），report-only roadmap。**

## P3 — 视听完整性

- **#3 major（plan_skeleton）**：`server/src/network/audio_trigger.rs:270-293` `emit_tribulation_audio_triggers` 读 `TribulationAnnounce`(thunder)/`JueBiTriggered`/`WaveCleared`/`TribulationFailed`(realm_regression)，**零 `EventReader<TribulationSettled>`**；`vfx_animation_trigger.rs:150-177` 对 `TribulationFailed` 发 hurt_stagger 动画也**零 `TribulationSettled`**。`TribulationSettled` 是真实 emit 事件（`halfstep_rechallenge_emit.rs:409` 等，携 `outcome=Ascended/HalfStep`），仅被 HUD 广播/state/halfstep/NPC AI 消费，**无任何视听**。渡劫失败有音效+动画，**渡劫成功（修仙最高级 gameplay 事件）完全无反馈**——纯遗漏（无 TODO、docs 内无 defer 注记，与 sword-path 分阶段不同）。修：补 `TribulationSettled` reader + 渡劫成功的音效/粒子/动画/narration（视听规格按 `docs/CLAUDE.md` §四视听精度要求写）。**内容/AV roadmap。**
- **#4 major（fix_pr）**：`server/src/combat/.../skills.rs:1741-1771` `visual_for()` 为 `AmbientVortex`/`VoidVortex`/`SwallowingVortex`/`VortexEcho`/`VoidCore` 定义 5 个 particle_id（`bong:vortex_ambient`/`woliu_void_sphere`/`woliu_swallowing_spiral`/`woliu_echo_ripple`/`woliu_void_core_collapse`）；这 5 招经 `resolve_woliu_v2_skill`（255-291）→ `emit_cast_events`（427）→ `VortexCastEvent{visual}` → `vfx_animation_trigger.rs:194-204` 把 `event.visual.particle_id` 作为 `SpawnParticle.event_id` 发送。但 client `VfxBootstrap.java:144-149` **仅注册 VortexSpiralPlayer 的 6 个基础 id**，5 个 VoidPath id 全仓 client grep **零命中** → `BongVfxParticleBridge.java:48-52` 对未注册 id 返回 `Optional.empty()` → `orElse(false)` **静默丢弃**。与 6 基础涡流 id 已注册形成非对称（玩家施这 5 招完全无粒子）。修：client 增 VfxPlayer 注册 5 id（视听差异化按 [[feedback_skill_av_diff]]，每招粒子需差异化非套同一 player）。**局部接线 + AV。**

## §N 开放问题

1. #2 hydrate 守恒：snapshot 残余 qi 与 ledger 余额谁为准（hydrate 时清 ledger vs 清 snapshot.qi_current）；是否并入 round1 `plan-qi-conservation-leaks-v1`（同守恒主题）还是独立（离屏复活时序较特殊）。
2. #8 JueBi 断线：abort（丢渡劫）vs 重连 restore（保留进度）——后者需 JueBiRuntimeContext 持久化恢复路径。
3. #4 VoidPath 五招：5 id 各自独立 VfxPlayer（差异化）vs 复用 VortexSpiralPlayer 调色（省工但违 AV 差异化红线）。
4. #1/#7 两条 fix_pr 是否合一个机械 fix PR（与 r1 `plan-bughunt-r1-mechanical-fixes-v1` 同性质）还是各自独立。

## 审计来源

bug-hunt round3（workflow，5 角度 finder + 怀疑者对抗 + opus 裁决，9 候选）。**ROOT = fresh origin/main worktree `.worktree/bughunt-verify` @ 341fc4461**（方法论修正后首轮，杜绝扫 stale）。已对 r1+r2 去重，6 REAL 全新。**report-only**：critical hydrate 守恒优先；#1/#4/#7 局部明确可直接 fix_pr，#2/#3/#8 需守恒时序/断线设计/AV roadmap 决议。
