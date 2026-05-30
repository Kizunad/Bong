# Bong · plan-offscreen-war-v1

**"看不见的远方也在死人"——离屏世界闭环大 plan。** 在已落地的 dormant 二态底盘（`plan-npc-virtualize-v1` ✅）上，把 NPC 虚拟化的三份骨架（`plan-npc-virtualize-v2` Drowsy 可见性 / `plan-npc-virtualize-v3` dormant 派系互殴 / `plan-faction-wars-v1` 玩家参战 / `plan-faction-expansion-v1` 具名势力）合并为一条端到端流程：**离屏 dormant NPC 之间真实开战 → 战死 → 守恒地还灵气给 zone → 人口回写 → 克制式战场遗物 → 天道派系消长叙事**，再叠加具名势力、玩家可参与战争、Drowsy 远视野可见。

> **本 plan 的中心纪律 = 真服 headless 自动观测测试**。离屏世界"玩家不在场也在死人"这件事，单元测试锁不住——必须能在**真实运行的服务器上、无需 client、纯 redis 断言**地观测人口消长 / 战死事件 / 灵气守恒。所有 server 侧机制阶段的验收都以 `scripts/e2e-offscreen-war.sh` 真服 e2e 为准（见 §11 测试纪律），单测只是补充不是替代。

## 阶段总览

| 阶段 | 主题 | PR | headless 可测 | 状态 |
|------|------|----|----|------|
| P0 | 可观测性 & 确定性底盘 + 派系数据化 bootstrap | PR-1 | ✅ 本阶段就是为可测性铺路 | ⬜ |
| P1 | dormant 配对 + 胜负 roll（纯逻辑，饱和单测） | PR-2 | ✅ 纯单测 | ⬜ |
| P2 | 战死闭环：release qi + emit death + 人口回写 + 真服 e2e | PR-3 | ✅ 全 redis 可观测 | ⬜ |
| P3 | 克制式战场遗物（deferred-on-hydrate） | PR-4 | ✅ 持久层可测 / hydrate 触发 | ⬜ |
| P4 | 天道派系消长叙事（agent 消费 death event） | PR-5 | ✅ bong:agent_narrate 可观测 | ⬜ |
| P5 | 具名势力扩展（NamedFaction + 关系矩阵 + 领袖） | PR-6~7 | ✅ headless | ⬜ |
| P6 | 玩家可参与派系战争（宣战/投靠/HUD） | PR-8~9 | ⚠️ 离屏层 headless；玩家层需 bot/client | ⬜ |
| P7 | Drowsy 中间态（远视野可见 + 边界平滑） | PR-10~11 | ⚠️ 渲染需 client；TPS 可 headless | ⬜ |

> 验收日期待填（`YYYY-MM-DD`）。P0-P4 是脊柱（离屏死亡闭环），P5-P7 是用户选定的全范围扩展。

**前置依赖（均已 ✅，本 plan 直接 build-on，不重造）**：
- `plan-npc-virtualize-v1` ✅ — dormant SoA + `NpcDormantStore` + `release_dormant_qi_to_zone` 守恒底盘（`server/src/npc/dormant/mod.rs`）
- `plan-npc-ai-v1` ✅ — `FactionStore` / `FactionMembership` / `is_hostile_pair` / `assign_hostile_encounters`（`server/src/npc/faction.rs:144/268/344/534`）
- `plan-qi-physics-v1` ✅ — `qi_release_to_zone` / `ledger::QiTransfer` / `WorldQiAccount::transfer`（`server/src/qi_physics/`）
- `plan-npc-perf-v1` ✅ — spatial index + LOD 降频，5000 dormant 规模化 TPS 前提（`server/src/npc/perf.rs` / `spatial.rs`）
- `plan-npc-combat-ai-v1` ✅ — `abstract_combat_resolve` 纯函数（ratio 模型，本 plan 复用，**注意其 `abstract_combat_system` 当前未接入 schedule**）

**反向被取代 / 归档（本 plan 合并后这些骨架应作废）**：`plan-npc-virtualize-v2`、`plan-npc-virtualize-v3`、`plan-faction-wars-v1`、`plan-faction-expansion-v1`（四份 skeleton 内容全部折叠入本 plan，归档时 `git rm` 之）。

---

## 接入面 Checklist

- **进料**：
  - `NpcDormantStore`（`dormant/mod.rs:240`，`snapshots: HashMap<CharId,NpcDormantSnapshot>` + `by_zone` / `by_archetype` 索引）——离屏战斗候选集
  - `NpcDormantSnapshot`（`dormant/mod.rs:185`，含 `cultivation` / `meridian_system` / `lifespan` / `faction: Option<FactionMembership>` / `loot_table`）——可结算单元
  - `FactionStore::is_hostile_pair`（`faction.rs:268`，当前仅 Attack↔Defend）/ `FactionMembership`（`faction.rs:344`）——敌对判定
  - `compute_combat_power`（`combat_power.rs:22`）+ `abstract_combat_resolve`（`abstract_combat.rs:37`）——战力 / 胜负 ratio 模型（**注意缺字段，见 §10.1 #2**）
  - `XorshiftRoll` + `roll_unit`（`cultivation/breakthrough.rs:368`）+ `deterministic_hash`（`dormant/mod.rs:742`）——确定性 RNG
  - `ZoneRegistry`（zone spirit_qi）+ `WorldQiAccount`（ledger）
- **出料**：
  - `release_dormant_qi_to_zone`（`dormant/mod.rs:898`）→ `qi_release_to_zone`（`release.rs:12`）→ `ledger.transfer(ReleaseToZone)`——败者真元守恒回灌
  - `NpcDeathNotice`（`lifecycle.rs:503`，reason=`Combat` + **新增** `from_dormant_combat` / `pos`）→ `publish_npc_death_events`（`npc_event_bridge.rs:36`）→ `bong:npc/death`（`NpcDeathV1`）
  - **新增** `DormantCombatOutcome` 内部 event + telemetry channel `bong:npc/combat`（`DormantCombatOutcomeV1`）
  - `NpcDormantStore::remove` + `rebuild_indexes`（`dormant/mod.rs:272/306`）——人口回写
  - **新增** `PendingDormantRelic` 持久层 → deferred-on-hydrate loot marker（复用 `spawn_daoxiang_from_corpse` 范式 `tsy_lifecycle.rs:736` / `pending_daoxiang_spawns` `events.rs:350`）
- **共享类型 / event**：复用 `FactionStore` / `QiTransfer{ReleaseToZone}` / `NpcDeathNotice` / `NpcDeathReason::Combat`（`lifecycle.rs:482` 已有）/ `bong:npc/death` channel（`channels.rs:65`）；**新增** `DormantCombatOutcome` / `bong:npc/combat` / `bong:qi/ledger`（守恒 telemetry）/ P5 `NamedFactionId` / P6 `FactionWarEvent` / P7 `NpcLodTier::Mid`。每个新建均在对应阶段说明"为何不复用"。
- **跨仓库契约**：
  - **server**：`bong:npc/death`（加 `from_dormant_combat:bool` + `pos`）、新 `bong:npc/combat`、新 `bong:qi/ledger`、P5 `bong:faction_state`、P6 `bong:faction_war`
  - **agent**：`agent/packages/schema/src/npc.ts`（`NpcDeathV1` TypeBox，`additionalProperties:false` → **必须双端同步加字段**，见 §10.1 #4）；`redis-ipc.ts` 消费 `from_dormant_combat` 聚合派系叙事
  - **client**：P6 `FactionWarHudLayer` / P7 `npc_lod` 远视野渲染（仅这两层涉及 client）
- **worldview 锚点**：§二 守恒律（`SPIRIT_QI_TOTAL`）/ §三 NPC 与玩家平等（死亡同规则）/ §十一 散修江湖派系势力消长。**注意 §十一 行号可能漂移，写死锚点前需人工重定位，见 §10.1 #6**。
- **qi_physics 锚点**：唯一真灵气流动点 = 败者 `release_dormant_qi_to_zone` → `qi_release_to_zone` → `ledger.transfer(QiTransferReason::ReleaseToZone)`（**不新增 QiTransferReason 变体**，复用既有）。胜者真元不变（dormant 简化，仍守恒）。遗物默认 `spirit_quality=0`（零真元生成）。详见 §10.1 #5 守恒自检。

---

## P0 — 可观测性 & 确定性底盘 + 派系数据化 bootstrap ⬜

> **为什么是第一个 PR**：① 离屏世界现在**无法被真服自动观测**（`/time advance` 只动 `CultivationClock` 不动 `GameTick`，无战斗 telemetry，qi 账本纯内存）；② `is_hostile_pair` 只认 Attack↔Defend 且 seeded dormant rogue 的 `faction=None`（`dormant/mod.rs:667`）→ **不先赋派系，后续所有阶段空转**。这两件事是所有后续阶段的硬前置，合并为 PR-1。

**交付物**：

1. **确定性步进 env**（`server/src/npc/dormant/mod.rs` + `config`）：
   - `BONG_DORMANT_TICK_INTERVAL`（覆盖 `DORMANT_LIFECYCLE_TICK_INTERVAL=1200`，`dormant/mod.rs:47`）——测试可设小值快进离屏 tick，不再 sleep 真实 60s。
   - `BONG_SIM_SEED`（注入 `AbstractCombatSeed` `abstract_combat_system.rs:25` + dormant 战斗 RNG seed）——让战争结果可复现。
   - 两个 env 仅影响节流 / 随机种子，**不绕过 worldview 修炼规则或 qi 守恒**（对齐 dev-only 纪律）。
2. **守恒 telemetry**：新增周期性 redis key `bong:qi/ledger`（HASH 或 snapshot），落 `WorldQiAccount` 各 zone/npc 账户余额 + `total_observed`，让外部脚本能做**精确**守恒断言（当前只能从 `world_state.zone.spirit_qi` 间接推断方向）。发布点挂在 `network/mod.rs` world_state 发布周期旁。
3. **战死可区分 schema**（双端 breaking，单独 commit）：
   - `NpcDeathNotice`（`lifecycle.rs:503`）加 `from_dormant_combat: bool` + `pos: Option<[f64;3]>`；所有构造点（`lifecycle.rs:773` / `dormant/mod.rs:946`）回填。
   - `NpcDeathV1`（server `schema/npc.rs:16`）+ **agent TypeBox `schema/src/npc.ts:81`（`additionalProperties:false`，必须同步）** 加同名字段；`agent/packages/schema/samples/` 加 `npc_death` v2 正反 sample 对拍。`NpcDeathCauseV1` 已含 `combat`（`npc.ts:29`），无需改 cause enum。
4. **派系数据化 bootstrap**：`seed_initial_dormant_population_on_startup`（`dormant/mod.rs:515`）给 seeded dormant rogue 赋 `FactionMembership`（最小版：按 char_id 哈希分配 `FactionId::Attack` / `FactionId::Defend`，保证有敌对对）。`FactionMembership` 已是 `Serialize+Component` 且已是 snapshot 字段（`dormant/mod.rs:204`），结构上 trivial。**具名势力留 P5**。
5. **e2e 脚手架**：`scripts/e2e-offscreen-war.sh`（fork `scripts/e2e-redis.sh` 的 redis+server+ioredis subscriber+`wait_for_pattern` 套路）+ `scripts/smoke-offscreen-war.sh`。setup 入口用**路径 B**：起服前 `redis-cli HSET bong:npc/dormant <char_id> '<snapshot JSON>'` 精确种受控派系（`load_dormant_store_from_redis_system` `dormant/mod.rs:359` 还原，store 非空跳过默认 seed）。

**测试声明**：
- 单测：`bong_dormant_tick_interval_env_overrides_default`、`bong_sim_seed_makes_combat_deterministic`、`npc_death_v1_roundtrip_with_from_dormant_combat`（server serde）+ agent `npc_death_v2_sample_pin`（TypeBox 正反对拍）。
- **真服 e2e**（`e2e-offscreen-war.sh` 第一项，验证外部驱动通路本身）：`agent_command_spawn_then_death_roundtrip`——`redis-cli publish bong:agent_command`（**注意是 `bong:agent_command` 不是 CLAUDE.md 里写错的 `bong:agent_cmd`**，`schema/channels.rs:4`）注入 spawn → `SUB bong:npc/spawn` 断言收到 + zone 匹配；`HGETALL bong:npc/dormant` 确认 seed 起效。
- 守恒可观测回归：`bong:qi/ledger` 起服后非空且 `total_observed ≈ SPIRIT_QI_TOTAL`（`constants.rs:60` DEFAULT=100.0，断言取 const 引用不写字面 100）。

**验收**：`bash scripts/e2e-offscreen-war.sh` 跑通外部注入→观测回路；`BONG_DORMANT_TICK_INTERVAL` 能把一轮 dormant tick 从 60s 压到秒级；`HSET` 种入的敌对 dormant 派系在 `HGETALL` 里 `faction` 字段非 None；`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 全绿；agent `npm test` 全绿。

---

## P1 — dormant 配对 + 胜负 roll（纯逻辑，饱和单测） ⬜

**目标**：实现"谁打谁、谁赢"的纯函数核心，不触碰真元 / store mutation（解耦：roll 失败可回滚无残留）。这一阶段**全程纯单测可锁**，不需真服。

**交付物**（新建 `server/src/npc/dormant/combat.rs`）：

1. **配对**：`collect_zone_combat_pairs(store, config) -> Vec<(CharId, CharId)>`——遍历 `store.by_zone`（`dormant/mod.rs:244`，**需加非 test 访问器**，现 `ids_by_zone` 是 `#[cfg(test)]` `dormant/mod.rs:292`），同 zone 内按 char_id 升序两两扫，`is_hostile_pair(a.faction, b.faction)` 为真才成对。**先 cap 候选**到每 zone 战力前 `2N` 个（防 5000 dormant O(n²)）。
2. **胜负**：`roll_dormant_combat_death(a: &NpcDormantSnapshot, b: &NpcDormantSnapshot, seed: u64) -> Option<CharId>`——`power = compute_combat_power(realm, cult, 合成的满血 Wounds, KnownTechniques::default, 默认 DerivedAttrs)`（**dormant 快照无 techniques/wounds/derived，只能合成默认值，见 §10.1 #2**）；`ratio = power_a/(power_a+power_b)`；`roll = XorshiftRoll(deterministic_hash(format!("{a}:{b}"), tick) ^ seed).roll_unit()`；`roll > ratio → a 死`。同境 ratio≈0.5 自然 50/50。
3. **上限**：`max_combats_per_zone`（默认 3）写进 `NpcVirtualizationConfig`（`dormant/mod.rs:50`）。
4. **借用安全结构**（**见 §10.1 #3**）：配对返回 `(CharId,CharId)` 列表而非快照引用，结算阶段（P2）再按 id 索引取出——避免 `dormant_global_tick_system` 的 per-char_id 单可变借用与两两对战冲突。

**测试声明**（`dormant/combat.rs` `#[cfg(test)]`，饱和）：
- happy：`roll_higher_realm_wins_more_often`（境界差 → 高境界统计胜率 >50%，跑 N 次确定 seed 统计）
- 边界：`same_realm_is_fifty_fifty`、`no_hostile_pair_yields_no_combat`（同派系 / Neutral → 空）、`empty_zone_no_pairs`、`single_npc_no_pairs`
- cap：`zone_caps_combats_to_max_per_zone`（>2N 候选 → 配对数 == cap）
- 确定性：`same_seed_same_outcome` / `different_seed_different_outcome`
- 契约：`compute_combat_power_uses_synthesized_defaults_for_dormant`（断言缺字段时用 realm_weight×condition，不 panic）

**验收**：`cargo test dormant::combat` 全绿且覆盖到"想不出还能加什么 case"；配对/胜负是纯函数，零 store/ledger 副作用（用 clippy 确认无 `ResMut` 参数）。

---

## P2 — 战死闭环：release qi + emit death + 人口回写 + 真服 e2e ⬜

> **本阶段是脊柱核心 + 真服自动观测的标杆**。把 P1 的纯逻辑接进 `dormant_global_tick_system`，让"远方在死人"真实发生、且可被 redis 无人值守断言。

**交付物**：

1. **combat phase 注入**：在 `dormant_global_tick_system`（`dormant/mod.rs:448`，复用同一 `interval` + `ResMut<NpcDormantStore>` + `ResMut<WorldQiAccount>` + `EventWriter<NpcDeathNotice>`）的 per-zone 处理里，先 `collect_zone_combat_pairs`（P1），逐对 `roll_dormant_combat_death`。**绝不另起第二个 timer**（会抢 store/ledger 可变借用）。
2. **败者结算（守恒）**：败者 → `release_dormant_qi_to_zone(snapshot, zones, ledger)`（`dormant/mod.rs:898`，直接复用，已验证守恒）→ 多败者同 zone **sequential** 逐个 release（先 release 的 zone_qi 回升，后者读更高基线，天然不溢出，与 `release.rs:121` order-independent 同源）→ **retain-until-released**（若 `overflow>0` 本轮不 remove，下轮重试，复用 `dormant/mod.rs:489-503` 防吞真元模式）→ `emit NpcDeathNotice(reason=Combat, from_dormant_combat=true, pos=snapshot.position)`（改 `dormant_death_notice` `dormant/mod.rs:937` 的硬编码 `NaturalAging` 为按死因分支）→ `store.remove(char_id) + rebuild_indexes`。
3. **胜者**：真元不变（dormant 简化，仍守恒——未流动即未失衡；与 hydrated `abstract_combat` 扣双方 qi 的差异见 §10.1 #5 / 开放问题已收口）。可选给胜者加 `combat_cooldown_until_tick`（新增 snapshot 字段，防单 tick 反复参战）。
4. **战果 telemetry**：`DormantCombatOutcome { winner, loser, zone, qi_released }` 内部 event → 发 `bong:npc/combat`（`DormantCombatOutcomeV1`，新 channel `channels.rs`）。**注意不要学 `abstract_combat_system` 只 `emit QiTransfer` 不消费**（`abstract_combat_system.rs:261` 的 `AbstractCombat` transfer 全仓无人 apply → 真元蒸发，§10.1 #5 红线）。

**测试声明**：
- 单测：`combat_death_releases_all_qi_to_zone`、`combat_death_emits_notice_with_combat_reason_and_pos`、`combat_death_removes_loser_from_store`、`zone_full_retains_loser_until_released`（防吞真元）、`winner_qi_unchanged`、`sequential_release_no_overflow`
- **守恒集成测试**：`offscreen_war_conserves_total_qi`——seed 500 dormant + 敌对 faction，跑多轮 combat，`assert_conservation`（`ledger.rs:326`）断言 `summarize_world_qi(before).total_observed() == after`（本场景无 EraDecay → 严格相等）。
- **真服 e2e**（`e2e-offscreen-war.sh` 核心项，全 headless）：
  - setup：`flushall` → `HSET bong:npc/dormant` 种两个敌对派系 dormant（`faction` 分属 Attack/Defend，`is_hostile_pair=true`）→ `BONG_SIM_SEED=<固定>` `BONG_DORMANT_TICK_INTERVAL=<小值>` 起服。
  - action：等数轮快进 tick。
  - observation：`SUB bong:npc/death`（收 cause/from_dormant_combat）+ `HLEN bong:npc/dormant`（人口）+ `SUB bong:world_state`（zone.spirit_qi）+ `SUB bong:npc/combat`（outcome）+ `bong:qi/ledger`（守恒）。
  - assertion：① 出现 `cause=combat` & `from_dormant_combat=true` 的死亡（不再全是 natural_aging）；② **人口减少量 == 观测到的 combat 死亡数**（种群守恒）；③ 战死方 zone 的 `spirit_qi` 上升（还灵气）；④ `bong:qi/ledger` 的 `total_observed` 全程 == `SPIRIT_QI_TOTAL`（精确守恒）；⑤ `bong:npc/combat` 的 outcome 与 death 一致。失败信息带 t0/t1 人口 + 死亡计数 + qi total 三个数。

**验收**：`bash scripts/e2e-offscreen-war.sh` 全绿；server pane 无 panic / 无 `assert_conservation` 失败；`cargo test` + clippy 全绿。**这一步过了 = claim #1 的"离屏战争闭环"从 0 到 1 成立且被真服锁住**。

---

## P3 — 克制式战场遗物（deferred-on-hydrate） ⬜

**目标**：让知名 NPC 战死在战场留下可探索遗物，但**克制**（末法基调，遍地尸体违和）。

**交付物**：

1. **克制判定**：`should_leave_relic(snapshot)` = `matches!(archetype, Disciple|GuardianRelic) || faction.is_some() || realm_ordinal(cult.realm) >= Solidify(固元)`（`combat_power.rs:11` `realm_ordinal`）。普通 rogue 不留。
2. **持久层**（**不走 `world/terrain/structures.rs`**——那是 worldgen 静态布局，战场 chunk 未加载）：新增 `PendingDormantRelic { char_id, zone, position, archetype, loot_seed, created_tick }`，存 **persistence sqlite 表 + TTL sweep**（仿 `npc_digests` 表 `persistence/mod.rs:949` + `sweep_npc_digest_retention_system` `persistence/mod.rs:541`，避免无人到访的战场遗物永久堆积）。**见 §10.1 开放风险（Resource vs sqlite 已选 sqlite）**。
3. **deferred-on-hydrate 生成**：玩家靠近 zone hydrate 时（`hydrate/mod.rs:368` 消费 dormant snapshot 入口）消费 pending relic → `roll_loot(default_loot_for_archetype(archetype), loot_seed)`（`loot.rs:147`，deterministic）→ 落地为 ground loot marker（复用 `spawn_daoxiang_from_corpse` `tsy_lifecycle.rs:736` + `pending_daoxiang_spawns` `events.rs:350` 的死亡→持久实体 deferred 范式，避免无 chunk 时穿地丢失，对齐 memory `project_infinite_fall_chunk`）。
4. **零真元生成**：遗物 loot 物化成 `ItemInstance` 时**显式 `spirit_quality=0`**（`roll_loot` 产出 `RolledLoot` 不带 spirit_quality，§10.1 #5 verify 修正），骨堆/残卷无真元，仅作叙事/材料。

**视听规格**（遗物在 hydrate 时被玩家发现，可感知）：
- **粒子**：`BongGroundDecalParticle` 战场遗骸地表贴花（骨堆=灰白 `#B8AFA0` / 残卷=暗黄 `#7A6A3C`），radial 静态 1 处，lifetime 持续（直到拾取），spawn 模式 burst-once-on-hydrate，复用现有 corpse decal 贴图。`bong:vfx_event` ID `offscreen_relic_reveal`。
- **音效**：`audio_recipe` 发现遗物时一层低沉 `entity.player.levelup` pitch=0.6 volume=0.4 delay_ticks=0 + 一层 `block.bone_block.break` pitch=0.8 volume=0.5 delay_ticks=2（骨堆质感）。
- **HUD**：无常驻；遗物作为可交互 marker，沿用现有 loot 拾取提示，不加新 HudLayer。
- **narration**（zone scope, perception style，2 条）：「此地残留厮杀气息，骨片间散落一枚断裂的宗门信物。」/「不知名的散修曾在此陨落，唯余一捧灰烬与半卷残经。」

**测试声明**：
- 单测：`only_named_npc_leaves_relic`（普通 rogue → None）、`relic_loot_is_deterministic_by_seed`、`relic_item_spirit_quality_is_zero`、`pending_relic_persisted_to_sqlite`、`relic_ttl_swept_after_expiry`
- **真服 e2e**：seed combat（含 relic-eligible Disciple）→ 快进 → 断言 sqlite `pending_dormant_relic` 表有记录（或新 `bong:` 观测 key）；模拟玩家靠近（path：spawn 玩家或 hydrate 触发）→ 断言战场坐标出现 loot marker。

**验收**：`cargo test` 全绿；遗物 e2e 验证"知名战死→可探索遗物"且普通 rogue 不淹没世界；零真元生成经 `assert_conservation` 复核。

---

## P4 — 天道派系消长叙事（agent 消费 death event） ⬜

**目标**：让天道"感知远方战事"，把离屏战死聚合成派系消长 narration。**不新建 telemetry channel**（v3 骨架已明确 agent 无需额外通道，新 channel 是孤岛红旗）——复用 `bong:npc/death`（带 `from_dormant_combat` + `faction_id` + `pos`）。

**交付物**（agent 侧为主）：

1. **消费校验**：`agent/packages/tiandao/src/redis-ipc.ts:126` `NpcRuntimeEventV1` 已含 `NpcDeathV1`；`validateNpcDeathV1Contract`（实体在 `schema/src/npc.ts:104`）+ TypeBox（P0 已加 `from_dormant_combat`/`pos`）同步。
2. **聚合**：Context Assembler 在一个推演窗口内 `group_by(zone, faction_id)` 过滤 `from_dormant_combat==true` 的 death，合成派系战报喂三 Agent（变化时代 / 演绎时代）。
3. **DormantCombatOutcome** 作为 server 内部 event（不出 redis，仅本地日志/测试断言用）；对 agent 的契约面只有 `bong:npc/death`。

**narration 模板**（broadcast scope，perception/narrative style，3 条）：
- 「据传，玄岭与断魂二宗于残灰谷争脉，三名散修横尸当场，灵气复归此地。」
- 「北荒方向隐有杀伐余韵，某派系折损精锐，那一带灵脉竟比往日丰盈了几分。」
- 「又是无名之辈的厮杀——天道不记其名，只记下那点回流的真元。」

**测试声明**：
- agent 单测：`aggregates_dormant_combat_deaths_by_faction`、`ignores_natural_aging_in_war_narration`、`narration_emitted_only_when_combat_deaths_present`（mock redis 喂 `NpcDeathV1` 序列）。
- **真服 e2e**：seed 敌对派系 → 快进触发 combat death → `SUB bong:agent_narrate` 断言收到含派系消长语义的 broadcast narration（mock LLM 模式下断言模板被触发 / 真 LLM 模式断言 narration 非空且 scope=broadcast）。

**验收**：agent `npm test` 全绿；真服联跑（`scripts/start.sh` + e2e）天道能因离屏战死产出 narration，不再因 dormant 静默而无远方战事叙事。

---

## P5 — 具名势力扩展（NamedFaction + 关系矩阵 + 领袖） ⬜

> 折叠 `plan-faction-expansion-v1`。把 P0 的 3 档匿名 faction 升级为具名散修势力，让"某宗与某宗交战"的叙事落到真实势力名。**仍全程 headless 可测**。

**交付物**（PR-6 数据底盘 / PR-7 领袖与 zone 控制）：
- PR-6：`NamedFactionId` enum（青云猎盟 / 沧渊商会 / 北荒漂流者…）+ `NamedFactionRegistry` + `FactionStatus` 三态 + `FactionId` 兼容层（保留 Attack/Defend/Neutral 映射）+ `FactionRelationMatrix`（Hostile/Neutral/Pact）+ **`are_hostile(a,b)` 取代 P1 硬编码 `is_hostile_pair`**（`faction.rs:268` 只认 Attack↔Defend，§10.1 #1 硬阻塞的最终解）+ `FactionStateV1` schema 双端 + dormant snapshot faction 字段 migration + ≥8 单测。
- PR-7：`NamedFactionLeader`（领袖 NPC，realm 更高）+ 领袖 spawn + `FactionZoneClaim`（势力控制 zone）+ 领袖行为树 + `bong:faction_state` telemetry + 领袖陨落→势力消亡 event + ≥10 单测。

**测试声明 / 验收**：
- 单测：`are_hostile_matrix_pairs`、`named_faction_serde_roundtrip`、`leader_death_collapses_faction`、`faction_zone_claim_updates_on_population`
- **真服 e2e**：种具名敌对势力 dormant → 快进 → `SUB bong:npc/death` 断言 death 携带具名 `faction_id`；`SUB bong:faction_state` 断言势力人口/控制 zone 随战争消长；agent narration 点名具体宗门（"玄岭"等实名而非"某派系"）。

---

## P6 — 玩家可参与派系战争（宣战/投靠/HUD） ⬜

> 折叠 `plan-faction-wars-v1`。在 P2 dormant 批量战斗 + P5 具名势力上加战争生命周期与玩家参与。**离屏层 headless 可测；玩家交互层自动测试需 client/bot（见测试声明的张力处理）**。

**交付物**（PR-8 战争生命周期 + 玩家参战 / PR-9 zone 控制 + HUD）：
- PR-8：`FactionWarEvent` + `WarPhase` 四态（宣战→野战→结算→余波）+ `PlayerFactionRole` + `WarRole` 四态（投靠/佣兵/截胡/旁观）+ `FactionWarOutcome` + 触发阈值（订阅 `DormantCombatOutcome` 累积）+ `/faction join|mercenary|intercept` 命令 + `bong:faction_war` pub + ≥15 单测。
- PR-9：`settle_system` + `ZoneSpiritBonus`（`regen_from_zone` 修正 ±10%/±5%，**走 qi_physics 不自定衰减**）+ Renown 联动 + client `FactionWarHudLayer` + agent outcome narration + ≥8 单测。

**视听规格**（玩家可感知，full spec）：
- **HUD**：`FactionWarHudLayer`（`HudRenderLayer`）双色血条——左红 `#C0392B`（攻方势力存活比）/ 右蓝 `#2980B9`（守方），顶部势力名 + WarPhase 文字；战争 active 时常驻于屏幕上方中央，fade in 10 tick / fade out 20 tick。
- **屏幕效果**：玩家所在 zone 进入"野战"phase 时 vignette tint `#5A1E1E` opacity 0.15 持续整个 phase；玩家投靠一方后该方血条高亮描边 `#F1C40F`。
- **粒子/音效**：宣战时 zone 内 `BongRibbonParticle` 战旗升起（攻/守色），audio_recipe 一层 `entity.wither.spawn` pitch=1.2 volume=0.5 + 一层 `ui.button.click` pitch=0.7 delay_ticks=4（宣战定音）。
- **narration**（broadcast，dialogue/narrative，2 条）：「玄岭宗主祭出战旗，向断魂宗宣战——残灰谷今夜无眠。」/「散修们闻风而动，有人投玄岭，有人待价而沽。」

**测试声明 / 验收**：
- 单测：`war_phase_transitions`（宣战→野战→结算→余波四态 + 各转换）、`player_role_join_mercenary_intercept_spectate`、`zone_spirit_bonus_via_qi_physics`（断言走 `regen_from_zone` 不自定常数）
- **离屏层真服 e2e**（headless）：触发 war → `SUB bong:faction_war` 断言 phase 推进 + outcome；zone 控制权随结算转移（`bong:faction_state`）。
- **玩家层测试（张力处理）**：`/faction join` 等命令需已连接 client（无 RCON/bot，§10.1 开放风险）。**两条路任选其一在 §10.1 收口**：(a) 本 plan 内引入一个最小 headless MC bot（新 harness，作为 PR-8 的子项）打 `/faction` 命令并断言 `bong:faction_war` role 变化；(b) 玩家命令降级为 `bong:agent_command` 可注入的等价入口 + 轻断言 + 人工 client checklist。HUD 渲染（PR-9）走人工 `./gradlew runClient` 截图 checklist，不强求自动化。

---

## P7 — Drowsy 中间态（远视野可见 + 边界平滑） ⬜

> 折叠 `plan-npc-virtualize-v2`。**注意：设计判定 Drowsy 与"死人"闭环正交**（它解决穿越边界撕裂 + 远视野稀薄的**可见性/体验**问题，不是死亡机制）。用户选择全范围纳入，故单列为最后阶段，**死亡闭环不依赖它**。

**交付物**（PR-10 三态机 + tick / PR-11 远视野渲染）：
- PR-10：`NpcLodTier::Mid`（`lod.rs:26` 现仅 Near/Far/Dormant）+ `NpcDrowsyState` + `NpcLodConfig{near,mid,far}` + 6 边转换矩阵 + `drowsy_tick_system`（1Hz）+ ≥15 守恒单测（Drowsy↔Dormant↔Far 进出不丢真元）。
- PR-11：远视野低 LOD packet + client `npc_lod` 渲染 + LOD gate + 三态 e2e（100 Near + 500 Drowsy + 1000 Dormant ≥18 TPS）+ 边界穿越压测。

**视听规格**：远视野（64-256 格）NPC 以低多边形/简化皮肤渲染，无动画细节；进入 64 格 hydrate 为完整实体。

**测试声明 / 验收**：
- 单测：`lod_tier_six_edge_transitions`、`drowsy_tick_conserves_qi`、`boundary_crossing_no_qi_leak`
- **真服 e2e（部分 headless）**：TPS 门禁可 headless（grep server pane TPS 行）；远视野渲染需人工 `./gradlew runClient` 目视 checklist。

---

## §10 实施工作流（consume-plan 按此执行）

### 10.1 开放问题 pre-P0 收口（2026-05-30，已由本 plan 调研 workflow + 守恒对抗复核产出）

> 下列决议每条已落"文件:行号 + 阶段"双锚点，**实施时以本节为准**。consume-plan 不得带开放问题进 P0。

**#1 派系敌对数据化（pre-P0 硬阻塞）** — 决议：P0 先给 seeded dormant rogue 赋 `FactionMembership`（按 char_id 哈希分 Attack/Defend），保证 `is_hostile_pair`（`faction.rs:268`，**实测只认 Attack↔Defend 一个敌对对**，Neutral 对谁都不敌对 `faction.rs:779`）能配出对；具名多宗（"某宗与某宗"）留 P5 的 `are_hostile` 关系矩阵。落点：`dormant/mod.rs:515/667`（seed 赋 faction）、P5（具名升级）。

**#2 `compute_combat_power` 缺字段** — 决议：`NpcDormantSnapshot`（`dormant/mod.rs:186-218`）**不存** `KnownTechniques`/`DerivedAttrs`/`Wounds`，而 `compute_combat_power`（`combat_power.rs:22`）三者必传。dormant 战力用 `realm_weight × condition_factor` 合成默认值（`KnownTechniques::default` / 满血 `Wounds` / 默认 `DerivedAttrs`），**接受简化战力**，不给 snapshot 补这三个字段（避免 Redis 快照膨胀）。落点：P1 `roll_dormant_combat_death`。

**#3 借用冲突** — 决议：`dormant_global_tick_system` 是 per-char_id 单可变借用循环（`dormant/mod.rs:465`），两两对战需双快照。实现为：先 `collect_zone_combat_pairs` 返回 `Vec<(CharId,CharId)>`（不可变只读），再按 id 顺序索引取出结算（跟现有 expired `Vec` 收尾模式一致）。**不是"插一段"，是 collect-then-index 重构**。落点：P1/P2。

**#4 schema 双端同步（breaking）** — 决议：agent `NpcDeathV1` TypeBox 是 `additionalProperties:false`（`schema/src/npc.ts:81`，validator `:104`）。server 加 `from_dormant_combat`/`pos` **必须同步改 TypeBox + sample 对拍**，否则 agent 拒收。`cause='combat'` 已在 `NpcDeathCauseV1`（`npc.ts:29`）可过校验。落点：P0 交付物 3。

**#5 守恒律红线自检（5 个流动点）** — 决议：①配对/roll = 只读 `qi_current` 算战力，零流动 ✅；②败者 release = `release_dormant_qi_to_zone`→`ledger.transfer(ReleaseToZone)`，clamp `QI_ZONE_UNIT_CAPACITY=50.0`（`constants.rs:72`），overflow 留 npc 账户不丢 ✅；③胜者真元不变（dormant 简化，未流动即未失衡）✅；④遗物 `spirit_quality=0` 不调 ledger ✅；⑤agent 叙事只读 death event ✅。**不新增 `QiTransferReason` 变体**（复用 `ReleaseToZone`）。**红线警示**：`abstract_combat_system` 的 `AbstractCombat` transfer 全仓无人 apply（真元蒸发，`abstract_combat_system.rs:261`），离屏战争**绝不能照抄 emit-only**，必须直接调 `release_dormant_qi_to_zone`。`WorldQiAccount::transfer` 真正记账入口在 `ledger.rs:197`（不是 `:145` 的构造器）。落点：P2 守恒集成测试 `assert_conservation`（`ledger.rs:326`）。

**#6 worldview 锚点 + 胜者真元对称性 + 遗物持久层** —
- worldview §十一 "派系势力消长" 行号可能已漂移（实读 947-970 是别的段）→ **写死锚点前人工重定位**；worldview 修改必须单独人工 PR，**consume-plan 不得自动改 worldview.md / CLAUDE.md / docs/CLAUDE.md**。"dormant 战斗吞真元红旗"想加进 `docs/CLAUDE.md §四` 也须人工 PR，本 plan 仅在此登记需求。
- 胜者真元不扣（vs hydrated abstract_combat 扣双方）的观感不一致 → **决议：dormant 简化不扣胜者**（少一次 ledger 操作 + 不写胜者快照），观感差异可接受（离屏本就抽象）；若后续要对齐，再走增量。
- `PendingDormantRelic` 持久层 → **决议：persistence sqlite 表 + TTL sweep**（仿 `npc_digests` `persistence/mod.rs:949` + sweep `:541`），不用 Resource（避免 relic 与已 remove 的死者生命周期耦合成孤儿）。
- 玩家命令测试入口（P6）→ **决议：优先 (a) 引入最小 headless MC bot 作为 PR-8 子项**；若 bot 引入成本过高，退 (b) `bong:agent_command` 等价注入 + 人工 client checklist。consume 实施时据 bot 引入难度二选一并在 PR 描述说明。

### 10.2 多 PR 序列化拆分（前一个 merge 后开下一个）

PR-1(P0 底盘+bootstrap) → PR-2(P1 纯逻辑) → PR-3(P2 战死闭环+真服e2e) → PR-4(P3 遗物) → PR-5(P4 叙事) → PR-6/7(P5 具名势力) → PR-8/9(P6 玩家战争) → PR-10/11(P7 Drowsy)。**脊柱 PR-1~5 是离屏死亡闭环，是本 plan 的核心价值，必须先全绿落地**；P5-P7 为扩展层，可在脊柱稳定后逐个推进。

### 10.3 视觉资产多轮（P3 遗物 / P6 HUD / P7 远视野渲染）

涉及 client 渲染 / VFX 贴图的交付（P3 遗物 decal、P6 `FactionWarHudLayer`、P7 LOD 渲染）走 3 轮自我打磨：round 1 first cut → round 2 截图/structure dump 自检 → round 3 spec 一致性 + 视觉叙事检查，终轮 commit 末尾写 `<PROMISE>` 担保块（注意拼写）。纯 server 逻辑 PR（PR-1~3、PR-5、PR-8 server 侧）不适用，按 atomic commit + 测试全绿。

### 10.4 PR 实施用独立 subagent（context 隔离）

每个 PR 起独立 `subagent_type: "claude"` + `model: "opus"` + prompt 末尾 `ultrathink`，共享主 worktree（不 nested worktree）。主线只接收 result，不亲自跑实施。

### 10.5 CodeRabbit / Pi agent ScheduleWakeup 等待协议

每 PR `gh pr checks` → `pending` 则 `ScheduleWakeup delaySeconds=1200`，最多 3 回合（60 min）卡死才交人工。修完 review 意见**必须重新等 CR + Pi agent re-review**，两 bot 确认无阻塞（Pi agent 写 `✅ Approve`）再 merge（对齐 memory `feedback_wait_coderabbit_approve`）。多 PR 各自走完整等待，前一个未收敛不开下一个。

### 10.6 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan` 后即可下班；醒来看本 plan 是否在 `docs/finished_plans/`。脊柱（PR-1~5）优先；扩展层若 scope 过大可分次 consume。

---

## §11 测试纪律：真服 headless 自动观测（本 plan 中心约束）

> 这一节是本 plan 区别于普通 plan 的重点。离屏世界"玩家不在场也在死人"必须能在**真实运行的 server 上、无 client、纯脚本断言**地被观测，否则等于没测。

**外部驱动真服的三条入口**（按可脚本化排序）：
- **路径 A — `redis-cli publish bong:agent_command`**（注意正确 channel 名是 `bong:agent_command` `schema/channels.rs:4`，**CLAUDE.md 里的 `bong:agent_cmd` 是过时错误命名**）：注入 `SpawnNpc`/`FactionEvent`/`ModifyZone` 等 7 种 `CommandType`（`schema/common.rs:18`）。局限：`spawn_npc` 不带 faction 参数。
- **路径 B — 起服前 `HSET bong:npc/dormant`（推荐做确定性 setup）**：`load_dormant_store_from_redis_system`（`dormant/mod.rs:359`）HGETALL 还原，store 非空跳过默认 seed → 精确种受控派系/属性的 dormant，最干净，无需 client。
- **路径 C — 真 MC client 打 dev 命令**：唯一能触发 `/time` `/zone_qi` 等 brigadier 命令的方式（命令要求 `event.executor` 是已连接 Client，`time.rs:44`）。无 RCON/bot → P6 玩家命令测试的 harness gap（§10.1 #6 决议）。

**可脚本读取的观测面（全部 `redis-cli` 可读）**：`HLEN/HGETALL bong:npc/dormant`（人口/快照）、`SUB bong:npc/death`（战死，含新 `from_dormant_combat`）、`SUB bong:npc/spawn`、`SUB bong:world_state`（zone.spirit_qi）、`SUB bong:npc/combat`（新，战果）、`bong:qi/ledger`（新，精确守恒）、`SUB bong:faction_state`（P5）、`SUB bong:faction_war`（P6）、tmux pane log grep。

**脚手架**：`scripts/e2e-offscreen-war.sh`（fork `e2e-redis.sh`：redis 三级 fallback + `cargo run --release` 起服 + ioredis subscriber + `wait_for_pattern`）；`scripts/smoke-offscreen-war.sh`（轻量子项）。**确定性前提**：`BONG_SIM_SEED`（RNG 可复现）+ `BONG_DORMANT_TICK_INTERVAL`（快进离屏 tick，免 sleep 60s）+ terrain raster manifest（ZoneRegistry 非空才 seed，`dormant/mod.rs:538`；或用路径 B 绕过）。

**每个 server 机制阶段的验收 = 对应真服 e2e 子项全绿**（P2 人口守恒 + qi 精确守恒 + 战死可区分；P3 遗物持久+hydrate 生成；P4 叙事；P5 具名势力盘面；P6 离屏层战争）。单测锁纯逻辑与守恒，e2e 锁端到端真服行为，二者都要，不可互相替代。

---

## Finish Evidence

> 全部 P ✅ + 本节填完后，由 `/consume-plan` 或人工 `git mv` 入 `docs/finished_plans/`。

**落地清单**：（每阶段对应真实模块/文件路径，待填）

**关键 commit**：（hash + 日期 + 一句话，待填）

**测试结果**：（`cargo test` / agent `npm test` / `e2e-offscreen-war.sh` 数量，待填）

**跨仓库核验**：（server `bong:npc/combat` / `from_dormant_combat` / `DormantCombatOutcome` 命中；agent `NpcDeathV1` 双端 sample 对拍 + 派系叙事聚合；client `FactionWarHudLayer` / `npc_lod` 渲染，待填）

**遗留 / 后续**：
- `git rm docs/plan-npc-overhaul-v1.md` + `docs/plan-npc-combat-gear-v1.md`（两个 untracked 孤儿副本，真本已在 finished_plans/，#323/#300 已完成）——本 plan 范围外的清理项，可顺手或单独处理。
- worldview §十一 派系消长行号重定位 + `docs/CLAUDE.md §四` 加"dormant 战斗吞真元"红旗——**人工 PR**，consume 不自动改。
- `git rm` 四份被本 plan 取代的 skeleton（virtualize-v2/v3、faction-wars、faction-expansion）——归档本 plan 时一并处理。
