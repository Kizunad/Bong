//! plan-offscreen-war-v1 P1：离屏 dormant 战斗的**纯逻辑核心**（配对 + 胜负 roll）。
//!
//! 本模块只回答两个问题：「谁打谁」(`collect_zone_combat_pairs`) 与「谁赢」
//! (`roll_dormant_combat_death`)。两者皆为纯函数——只接收 `&` 入参、返回 owned 数据，
//! **零 store mutation / 零 ledger / 零真元流动 / 无 `ResMut`**。把战斗决策从结算副作用
//! 里解耦出来，让任何回归都能被饱和单测立刻撞红，且 roll 失败无残留可回滚。
//!
//! 真正的战死结算（败者 `release_dormant_qi_to_zone` 守恒还灵气 / emit `NpcDeathNotice`
//! / `store.remove` 人口回写）是 **P2** 的活，在 `dormant_global_tick_system` 里按本模块
//! 返回的 `(CharId, CharId)` 列表逐对索引结算（§10.1 #3 借用安全：返回 owned id 对而非
//! 快照引用，避免 per-char_id 单可变借用与两两对战冲突）。
//!
//! ## 战力合成（§10.1 #2）
//! `NpcDormantSnapshot` 不存 `KnownTechniques` / `DerivedAttrs` / `Wounds`（避免 Redis
//! 快照膨胀），而 `compute_combat_power` 三者必传。dormant 战力因此用**合成默认值**：
//! 满血 `Wounds`、中性 `DerivedAttrs::default()`、空 `KnownTechniques`、无装备。结果由
//! `realm_weight × condition_factor` 主导（realm-monotonic），是有意接受的离屏简化战力。
//!
//! 注意：`KnownTechniques::default()` 在 `dev-techniques` feature 下会返回**全功法**而非空，
//! 为让合成战力与 feature flag 无关、保持 realm 单调，这里显式构造 `{ entries: vec![] }`。

// plan-offscreen-war-v1 P2：本模块的配对 / 胜负 roll 已被 `dormant_global_tick_system` 的
// combat phase（`run_dormant_combat_phase`）真实消费，P1 时期的 `#![allow(dead_code)]` 豁免
// 已移除——接口先于实现锁定，饱和单测全面覆盖，接入后未改任何测试。

use super::{
    deterministic_hash, effective_group, NpcDormantSnapshot, NpcDormantStore,
    NpcVirtualizationConfig,
};
use crate::combat::components::{DerivedAttrs, Wounds};
use crate::cultivation::breakthrough::{RollSource, XorshiftRoll};
use crate::cultivation::components::Realm;
use crate::cultivation::known_techniques::KnownTechniques;
use crate::npc::combat_power::{compute_combat_power, realm_ordinal, CombatPowerScore};
use crate::npc::faction::FactionStore;
use crate::npc::lifecycle::NpcArchetype;
use crate::social::components::CharId;

/// 合成一份 dormant 快照的离屏战力（§10.1 #2）。
///
/// dormant 快照缺 `Wounds` / `DerivedAttrs` / `KnownTechniques`，这里用确定的默认值补齐：
/// - `Wounds::default()` = 满血（`health_current == health_max`），离屏单元默认无伤。
/// - `DerivedAttrs::default()` = 中性派生属性（attack/defense = 1.0）。
/// - 空 `KnownTechniques`（**显式构造**，不走 `::default()`——后者在 `dev-techniques`
///   feature 下是全功法，会破坏 realm 单调）。
/// - 无装备（`None`）。
///
/// 真元当前/上限直接读快照 `cultivation`，故 condition_factor 仍反映 qi 充盈度。结果
/// 由 realm 主导且 realm-monotonic（同条件下高境界 > 低境界）。纯只读，绝不 panic。
pub fn synthesize_dormant_power(snapshot: &NpcDormantSnapshot) -> CombatPowerScore {
    let full_health = Wounds::default();
    let default_derived = DerivedAttrs::default();
    // 显式空功法：不用 KnownTechniques::default()，因为 dev-techniques feature 下它非空。
    let no_techniques = KnownTechniques {
        entries: Vec::new(),
    };
    compute_combat_power(
        snapshot.cultivation.realm,
        &snapshot.cultivation,
        &full_health,
        &default_derived,
        &no_techniques,
        None,
    )
}

/// 「谁打谁」——遍历每个 zone，配出本轮的离屏敌对战斗对（§10.1 #1/#3）。
///
/// 对每个 zone：
/// 1. 取该 zone 的 dormant char_ids（`store.char_ids_in_zone`，非 test 访问器）。
/// 2. **先按战力 cap 候选**到前 `2 * max_combats_per_zone` 个（防 5000 dormant 在单个
///    高密度 zone 引爆 O(n²) 配对；strongest-first 留下最有"战斗资格"的单元）。
/// 3. 把候选集按 char_id 升序排序，两两扫描——**每个 NPC 一轮至多参战一次**（成对后
///    两者都跳过）。仅当双方都 `Some(faction)` 且 `is_hostile_pair(a, b)` 为真才成对。
/// 4. 每 zone 配出的对数再 cap 到 `max_combats_per_zone`。
///
/// 返回 **owned** `Vec<(CharId, CharId)>`（不是快照引用）——§10.1 #3：P2 结算阶段按 id
/// 索引取出双方，规避 `dormant_global_tick_system` per-char_id 单可变借用与双快照对战冲突。
///
/// 纯函数：只接 `&store` / `&faction_store` / `&config`，零 mutation、无 `ResMut`。
/// 配对内部对 `(a, b)` 规范化为 char_id 升序，故返回元组第一元素恒 < 第二元素，跨调用稳定。
pub fn collect_zone_combat_pairs(
    store: &NpcDormantStore,
    faction_store: &FactionStore,
    config: &NpcVirtualizationConfig,
) -> Vec<(CharId, CharId)> {
    let max_pairs = config.max_combats_per_zone as usize;
    let mut pairs = Vec::new();
    if max_pairs == 0 {
        return pairs;
    }
    // 候选 cap：每 zone 至多保留战力前 2N 个（2 * 对数上限），strongest-first。
    let candidate_cap = max_pairs.saturating_mul(2);

    // by_zone 的 key 顺序在 HashMap 里非确定，但配对内不跨 zone，且每 zone 独立 cap，
    // 故 zone 遍历顺序不影响"每 zone 配哪些对"。为输出稳定，按 zone 名排序遍历。
    let mut zone_names: Vec<&String> = store.by_zone.keys().collect();
    zone_names.sort();

    for zone_name in zone_names {
        let zone_ids = store.char_ids_in_zone(zone_name);
        if zone_ids.len() < 2 {
            continue;
        }

        // 取出本 zone 候选快照 + 战力，按战力降序截断到 candidate_cap。
        // 过滤 `combat_dead_pending_release`：已离屏战死、真元待释放的败者**不再参战**
        // （plan-offscreen-war-v1 P3 review-fix）。它仍留在 `store.snapshots`（防吞真元 +
        // 随 Redis 持久化），但若再被选中重新 roll，死亡 notice / outcome 会重复 emit、污染
        // P4 派系死亡聚合。retry release 由 `run_pending_combat_release_retry` 单独负责。
        let mut scored: Vec<(&NpcDormantSnapshot, f32)> = zone_ids
            .iter()
            .filter_map(|id| store.snapshots.get(id))
            .filter(|snap| !snap.combat_dead_pending_release)
            .map(|snap| (snap, synthesize_dormant_power(snap).0))
            .collect();
        // 战力降序；战力相等时按 char_id 升序兜底，保证 cap 边界确定（无 NaN：power finite）。
        scored.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.char_id.cmp(&right.0.char_id))
        });
        scored.truncate(candidate_cap);

        // 候选集按 char_id 升序排序后两两配对（每个 NPC 一轮至多一次）。
        let mut candidates: Vec<&NpcDormantSnapshot> =
            scored.into_iter().map(|(snap, _)| snap).collect();
        candidates.sort_by(|left, right| left.char_id.cmp(&right.char_id));

        let mut zone_pair_count = 0usize;
        let mut index = 0usize;
        while index + 1 < candidates.len() && zone_pair_count < max_pairs {
            let a = candidates[index];
            let b = candidates[index + 1];
            // plan-offscreen-war-v1 P5 reframe b：敌对判定改走涌现群体身份——双方都解析出
            // 群体（effective_group：显式优先、否则 faction 派生）且 `are_hostile`（不同群体）
            // 才成对（§十灵气零和）。取代 P1 `is_hostile_pair` 的 Attack↔Defend 2-faction 上限，
            // 天然支持 >2 群体互殴；群体 None（如旧 Neutral 散修）进不了配对、仍非战斗。
            match (
                effective_group(a, faction_store),
                effective_group(b, faction_store),
            ) {
                (Some(ga), Some(gb)) if faction_store.are_hostile(ga, gb) => {
                    // 规范化为 char_id 升序（candidates 已升序，a < b 恒成立）。
                    pairs.push((a.char_id.clone(), b.char_id.clone()));
                    zone_pair_count += 1;
                    index += 2; // 两者本轮均已参战，跳过。
                }
                _ => {
                    // 非敌对：a 本轮不与 b 战，但 b 可能与下一个敌对，故只前进 1。
                    index += 1;
                }
            }
        }
    }

    pairs
}

/// SplitMix64 finalizer：把"组合种子"雪崩混合后再喂 `XorshiftRoll`。
///
/// **为什么需要**：`XorshiftRoll` 是为"持续迭代的流"设计的 PRNG，对**单次** `roll_unit()`
/// 而言，相邻输入（`deterministic_hash(key,tick) ^ seed` 在 seed 连续递增时只改低位）
/// 单步 xorshift 后高位几乎不变——实测 4000 个连续 seed 的 roll 全部落在 ≈0.28，永不越过
/// 0.5，导致同境 50/50 完全塌成"a 永不战死"。先过一遍 SplitMix64 finalizer 把任意位差
/// 扩散到全字长，单步 xorshift 即得近均匀分布（实测 0.49–0.51）。纯整数运算、确定性。
fn avalanche(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// 「谁赢」——对一对 dormant 快照 roll 出**败者** char_id（§10.1 #2 战力 + 确定性 RNG）。
///
/// - 双方战力用 [`synthesize_dormant_power`] 合成（满血 / 中性派生 / 空功法 / 无装备）。
/// - `ratio = power_a / (power_a + power_b)` 是 a 的**存活权重**；退化 `power_a+power_b == 0`
///   回退 0.5（双方零战力 ⇒ 公平 50/50，不 panic、不除零）。
/// - `roll = XorshiftRoll(avalanche(deterministic_hash("{a_id}:{b_id}", tick) ^ seed)).roll_unit()`，
///   `roll ∈ [0,1)`。先 [`avalanche`] 雪崩混合组合种子（否则相邻 seed 单步 xorshift 不均匀，
///   见该函数文档），再单步取 unit。`roll > ratio ⇒ a 死`（a 存活权重越大越不容易触发）；
///   否则 b 死。同境 ⇒ ratio≈0.5 ⇒ ~50/50；高境一方存活权重大 ⇒ 战死概率更低。
/// - 一场战斗必致死，故正常入参恒返回 `Some(loser)`；仅 `a.char_id == b.char_id`（非法
///   自我对战）返回 `None`。
///
/// 纯函数：只读双方快照，返回一个 char_id，无 store / ledger / 真元副作用。同 `(a,b,tick,seed)`
/// 永远给同一个败者（确定性）；换 seed 或换 tick 可改变结果。
pub fn roll_dormant_combat_death(
    a: &NpcDormantSnapshot,
    b: &NpcDormantSnapshot,
    tick: u64,
    seed: u64,
) -> Option<CharId> {
    if a.char_id == b.char_id {
        // 非法：同一个 NPC 不能跟自己对战。
        return None;
    }

    let power_a = synthesize_dormant_power(a).0 as f64;
    let power_b = synthesize_dormant_power(b).0 as f64;
    let total = power_a + power_b;
    // a 的存活权重；退化零战力 ⇒ 公平 50/50（避免除零产生 NaN）。
    let ratio = if total > 0.0 { power_a / total } else { 0.5 };

    let key = format!("{}:{}", a.char_id, b.char_id);
    let combined = avalanche(deterministic_hash(&key, tick) ^ seed);
    let roll = XorshiftRoll(combined).roll_unit();

    if roll > ratio {
        // a 的存活权重被 roll 超过 ⇒ a 战死。
        Some(a.char_id.clone())
    } else {
        Some(b.char_id.clone())
    }
}

/// 固元境（`Realm::Solidify`）的 realm ordinal——遗物克制判定的境界门槛（plan-offscreen-war-v1
/// P3 §139）。提取为具名 const 而非裸字面 `3`：境界 ordinal 是 [`realm_ordinal`] 的契约，
/// 写死字面会在 enum 重排时静默漂移。`debug_assert` 在测试 / debug build 锁死该等价关系。
const RELIC_REALM_THRESHOLD: u8 = 3; // realm_ordinal(Realm::Solidify)

/// 「这名战死者值不值得在战场留下遗物」——**克制判定**（plan-offscreen-war-v1 P3 §139）。
///
/// 末法基调下遍地尸体违和，故只有"知名 / 有来历"的战死者才留遗物：
/// - `Disciple` / `GuardianRelic` archetype（有宗门 / 是护道遗灵 → 必有信物）；**或**
/// - 任意 `faction.is_some()`（结派散修，身上带派系信物）；**或**
/// - 境界 ≥ 固元（`realm_ordinal(realm) >= 3`，固元及以上是真正的高手，殒落值得记载）。
///
/// 普通无派系低境 `Rogue` / `Commoner` / `Beast` 等 ⇒ `false`，不留遗物、不淹没世界。
///
/// 纯只读判定：只看 archetype / faction / realm 三个快照字段，零副作用、绝不 panic。
/// **守恒无关**：本函数不碰真元——它只决定"要不要后续创建一个零真元遗物"，真元守恒
/// 已由调用方在判定**之前**的 `release_dormant_qi_to_zone` 完成（§10.1 #5 ④红线）。
pub fn should_leave_relic(snapshot: &NpcDormantSnapshot) -> bool {
    debug_assert_eq!(
        realm_ordinal(Realm::Solidify),
        RELIC_REALM_THRESHOLD,
        "RELIC_REALM_THRESHOLD must stay == realm_ordinal(Solidify); realm enum reordered without updating the relic threshold const"
    );
    matches!(
        snapshot.archetype,
        NpcArchetype::Disciple | NpcArchetype::GuardianRelic
    ) || snapshot.faction.is_some()
        || realm_ordinal(snapshot.cultivation.realm) >= RELIC_REALM_THRESHOLD
}

/// 战场遗物的 **deterministic loot 种子**（plan-offscreen-war-v1 P3 §141/§142）。
///
/// 遗物 loot 必须可复现：战死结算时算出该种子并持久化进 `pending_dormant_relics`，玩家
/// 靠近 hydrate 时用同一种子 `roll_loot(default_loot_for_archetype(archetype), seed)`，保证
/// "同一具战场遗骸每次掉同一批战利品"。种子由 `(char_id, tick)` 经 [`deterministic_hash`]
/// 混合后再 `^ sim_seed`——与 [`roll_dormant_combat_death`] 同源的确定性 RNG 输入构造，
/// 让 `BONG_SIM_SEED` 一并控制 loot 结果，真服 e2e 可复现。纯整数运算、确定性。
pub fn relic_loot_seed(char_id: &CharId, tick: u64, sim_seed: u64) -> u64 {
    deterministic_hash(char_id, tick) ^ sim_seed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::npc::faction::{
        FactionId, FactionMembership, FactionRank, MissionQueue, Reputation,
    };
    use crate::npc::lifecycle::NpcArchetype;

    /// 造一个最小可用的 dormant 快照：给定 char_id / zone / realm / faction。
    /// 真元拉满（qi_current == qi_max），让 condition_factor 由 hp(满)+qi(满) 主导，
    /// 使合成战力 realm-monotonic、便于断言"高境界战力更高"。
    fn snapshot(
        char_id: &str,
        zone: &str,
        realm: Realm,
        faction: Option<FactionId>,
    ) -> NpcDormantSnapshot {
        let qi_max = match realm {
            Realm::Awaken => 10.0,
            Realm::Induce => 30.0,
            Realm::Condense => 60.0,
            Realm::Solidify => 120.0,
            Realm::Spirit => 200.0,
            Realm::Void => 400.0,
        };
        let cultivation = Cultivation {
            realm,
            qi_current: qi_max,
            qi_max,
            ..Default::default()
        };
        NpcDormantSnapshot {
            char_id: char_id.to_string(),
            archetype: NpcArchetype::Rogue,
            dimension: crate::world::dimension::DimensionKind::Overworld,
            zone_name: zone.to_string(),
            position: [0.0, 64.0, 0.0],
            schedule_seed: None,
            cultivation: cultivation.clone(),
            meridian_system: Default::default(),
            meridian_severed: Default::default(),
            contamination: Default::default(),
            lifespan: crate::npc::lifecycle::NpcLifespan::new(0.0, 100_000.0),
            shared_lifespan: crate::cultivation::lifespan::LifespanComponent::for_realm(realm),
            lifespan_extension_ledger: Default::default(),
            death_registry: crate::cultivation::lifespan::DeathRegistry::new(char_id.to_string()),
            life_record: crate::cultivation::life_record::LifeRecord::new(char_id.to_string()),
            faction: faction.map(membership),
            // 默认无显式群体：走 effective_group 的 faction 派生回退（Attack→0/Defend→1/
            // Neutral→None），让既有配对测试零改动通过；显式群体测试单独 set 该字段。
            emergent_group: None,
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            tsy_hostile: None,
            intent: super::super::DormantBehaviorIntent::Cultivate {
                zone: zone.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: cultivation.qi_current,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
        }
    }

    fn membership(faction_id: FactionId) -> FactionMembership {
        FactionMembership {
            faction_id,
            rank: FactionRank::Disciple,
            reputation: Reputation::default(),
            lineage: None,
            mission_queue: MissionQueue::default(),
        }
    }

    fn store_with(snaps: Vec<NpcDormantSnapshot>) -> NpcDormantStore {
        let mut store = NpcDormantStore::default();
        for snap in snaps {
            store.snapshots.insert(snap.char_id.clone(), snap);
        }
        store.rebuild_indexes();
        store
    }

    fn config_with_max(max: u32) -> NpcVirtualizationConfig {
        NpcVirtualizationConfig {
            max_combats_per_zone: max,
            ..Default::default()
        }
    }

    // ───────────────────────── 战力合成（§10.1 #2 契约） ─────────────────────────

    #[test]
    fn compute_combat_power_uses_synthesized_defaults_for_dormant() {
        // dormant 快照没有 techniques/wounds/derived，合成默认值后不 panic，
        // 且 realm-monotonic：高境界战力严格更高。
        let realms = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        let mut last = f32::NEG_INFINITY;
        for realm in realms {
            let snap = snapshot("c", "z", realm, None);
            let power = synthesize_dormant_power(&snap).0;
            assert!(
                power.is_finite(),
                "synthesized power for {realm:?} must be finite (no NaN/inf from missing fields), got {power}"
            );
            assert!(
                power > last,
                "synthesized power must be realm-monotonic: {realm:?} power {power} should exceed previous realm's {last}"
            );
            last = power;
        }
    }

    // ───────────────────────── 配对：empty / single / no-hostile ─────────────────

    #[test]
    fn empty_zone_no_pairs() {
        let store = store_with(vec![]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(3));
        assert!(
            pairs.is_empty(),
            "an empty store must yield zero combat pairs because there is nobody to fight, got {pairs:?}"
        );
    }

    #[test]
    fn single_npc_no_pairs() {
        let store = store_with(vec![snapshot(
            "lone",
            "z",
            Realm::Induce,
            Some(FactionId::Attack),
        )]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(3));
        assert!(
            pairs.is_empty(),
            "a zone with one NPC cannot form a pair (needs two), got {pairs:?}"
        );
    }

    #[test]
    fn no_hostile_pair_yields_no_combat() {
        let factions = FactionStore::default();
        let config = config_with_max(3);

        // 同派系（Attack vs Attack）：is_hostile_pair=false ⇒ 无对。
        let same = store_with(vec![
            snapshot("a", "z", Realm::Induce, Some(FactionId::Attack)),
            snapshot("b", "z", Realm::Induce, Some(FactionId::Attack)),
        ]);
        assert!(
            collect_zone_combat_pairs(&same, &factions, &config).is_empty(),
            "same-faction NPCs (Attack vs Attack) are not hostile ⇒ no combat pair"
        );

        // Neutral 对谁都不敌对。
        let neutral = store_with(vec![
            snapshot("a", "z", Realm::Induce, Some(FactionId::Neutral)),
            snapshot("b", "z", Realm::Induce, Some(FactionId::Attack)),
        ]);
        assert!(
            collect_zone_combat_pairs(&neutral, &factions, &config).is_empty(),
            "Neutral is hostile to nobody (faction.rs is_hostile_pair) ⇒ no combat pair with Attack"
        );

        // 缺 faction（None）：不入对。
        let missing = store_with(vec![
            snapshot("a", "z", Realm::Induce, None),
            snapshot("b", "z", Realm::Induce, Some(FactionId::Defend)),
        ]);
        assert!(
            collect_zone_combat_pairs(&missing, &factions, &config).is_empty(),
            "an NPC with faction=None cannot be matched into a hostile pair ⇒ no combat"
        );
    }

    #[test]
    fn hostile_pair_in_zone_produces_one_pair() {
        let store = store_with(vec![
            snapshot("a", "z", Realm::Induce, Some(FactionId::Attack)),
            snapshot("b", "z", Realm::Induce, Some(FactionId::Defend)),
        ]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(3));
        assert_eq!(
            pairs,
            vec![("a".to_string(), "b".to_string())],
            "one Attack + one Defend in a zone must produce exactly the hostile pair (a,b) in char_id-ascending order, got {pairs:?}"
        );
    }

    #[test]
    fn pending_release_loser_excluded_from_pairs() {
        // plan-offscreen-war-v1 P3 review-fix（CodeRabbit Major）：`collect_zone_combat_pairs` 的
        // `!combat_dead_pending_release` 过滤分支的专属纯函数单测。已离屏战死、真元待释放的败者
        // （逻辑死亡）绝不再被选中参战——否则会被重新 roll、重复 emit death notice / outcome。
        //
        // 构造：升序 a(Attack, flagged) < b(Defend, alive) < c(Attack, alive)。若**不**过滤 flagged，
        // 升序两两扫会先把 a-b 配成敌对对（a<b 先命中）；过滤掉 a 后只剩 b-c，配出 (b,c)。
        // 故断言结果恰为 (b,c) 同时排除 a，精确锁住"配出 alive 的、跳过 flagged 的"。
        let mut a = snapshot("a", "z", Realm::Induce, Some(FactionId::Attack));
        a.combat_dead_pending_release = true; // 逻辑死亡，待释放
        let b = snapshot("b", "z", Realm::Induce, Some(FactionId::Defend));
        let c = snapshot("c", "z", Realm::Induce, Some(FactionId::Attack));
        let store = store_with(vec![a, b, c]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(3));
        assert_eq!(
            pairs,
            vec![("b".to_string(), "c".to_string())],
            "the flagged (pending-release) loser `a` must be skipped before pairing — without the \
             filter, ascending scan would match a-b first; with it, only the two alive NPCs (b,c) \
             pair up. got {pairs:?}"
        );
        assert!(
            !pairs.iter().any(|(x, y)| x == "a" || y == "a"),
            "the pending-release loser `a` must NOT appear in any combat pair (re-selecting it would \
             re-emit a death for an already-dead NPC); got {pairs:?}"
        );
    }

    #[test]
    fn all_candidates_flagged_yields_no_pairs() {
        // 边界：候选全部 `combat_dead_pending_release=true` → 过滤后空候选 → 零对。
        // （满 zone 下多败者积压全待释放时不会再凭空冒出新战斗。）
        let mut a = snapshot("a", "z", Realm::Induce, Some(FactionId::Attack));
        let mut b = snapshot("b", "z", Realm::Induce, Some(FactionId::Defend));
        a.combat_dead_pending_release = true;
        b.combat_dead_pending_release = true;
        let store = store_with(vec![a, b]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(3));
        assert!(
            pairs.is_empty(),
            "when every hostile candidate is flagged pending-release (all logically dead), the \
             filter must leave no candidates and produce zero pairs; got {pairs:?}"
        );
    }

    #[test]
    fn pairs_do_not_cross_zone_boundaries() {
        // 两个敌对 NPC 在不同 zone ⇒ 不成对（配对只在 zone 内）。
        let store = store_with(vec![
            snapshot("a", "zone_one", Realm::Induce, Some(FactionId::Attack)),
            snapshot("b", "zone_two", Realm::Induce, Some(FactionId::Defend)),
        ]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(3));
        assert!(
            pairs.is_empty(),
            "hostile NPCs in different zones must not fight (combat is per-zone), got {pairs:?}"
        );
    }

    #[test]
    fn each_npc_used_at_most_once_per_round() {
        // 3 个 Attack + 1 个 Defend：升序 a(Atk) b(Atk) c(Atk) d(Def)。
        // 升序两两扫：a,b 同派系跳，b,c 同派系跳，c,d 敌对成对。每个 NPC 一轮至多一次。
        // 关键断言：d 只出现在一对里（不被复用）。
        let store = store_with(vec![
            snapshot("a", "z", Realm::Induce, Some(FactionId::Attack)),
            snapshot("b", "z", Realm::Induce, Some(FactionId::Attack)),
            snapshot("c", "z", Realm::Induce, Some(FactionId::Attack)),
            snapshot("d", "z", Realm::Induce, Some(FactionId::Defend)),
        ]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(5));
        let d_appearances = pairs.iter().filter(|(x, y)| x == "d" || y == "d").count();
        assert_eq!(
            d_appearances, 1,
            "the single Defend NPC 'd' must be used in at most one pair per round, appeared in {d_appearances} pairs: {pairs:?}"
        );
        assert_eq!(
            pairs,
            vec![("c".to_string(), "d".to_string())],
            "ascending pairwise scan should match c(Attack) with d(Defend) after a,b skip as same-faction, got {pairs:?}"
        );
    }

    #[test]
    fn two_disjoint_hostile_pairs_in_zone() {
        // a(Atk) b(Def) c(Atk) d(Def) 升序：a-b 敌对成对(跳到 c)，c-d 敌对成对 ⇒ 两对。
        let store = store_with(vec![
            snapshot("a", "z", Realm::Induce, Some(FactionId::Attack)),
            snapshot("b", "z", Realm::Induce, Some(FactionId::Defend)),
            snapshot("c", "z", Realm::Induce, Some(FactionId::Attack)),
            snapshot("d", "z", Realm::Induce, Some(FactionId::Defend)),
        ]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(5));
        assert_eq!(
            pairs,
            vec![
                ("a".to_string(), "b".to_string()),
                ("c".to_string(), "d".to_string()),
            ],
            "four alternating Attack/Defend should form two disjoint pairs (a,b)+(c,d), got {pairs:?}"
        );
    }

    // ───────── plan-offscreen-war-v1 P5 reframe b：显式涌现群体（>2 群体互殴） ─────────

    use crate::npc::faction::EmergentGroupId;

    #[test]
    fn three_explicit_groups_in_zone_all_cross_group_pairs_are_hostile() {
        // 三个**显式** emergent_group（0/1/2）同 zone，验证 >2 群体互殴：跨组对全敌对。
        // 升序 a(组0) b(组1) c(组2)：升序两两扫先配 a-b（异组成对，跳到 c），c 落单 ⇒ 一对。
        // 关键：a-b 能成对**仅因为显式群体不同**——faction 全 None，若还走旧 is_hostile_pair
        // 路径则永远配不出（None faction），故此测试同时锁住"敌对判定改走 emergent_group"。
        let mut a = snapshot("a", "z", Realm::Induce, None);
        let mut b = snapshot("b", "z", Realm::Induce, None);
        let mut c = snapshot("c", "z", Realm::Induce, None);
        a.emergent_group = Some(EmergentGroupId(0));
        b.emergent_group = Some(EmergentGroupId(1));
        c.emergent_group = Some(EmergentGroupId(2));
        let store = store_with(vec![a, b, c]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(5));
        assert_eq!(
            pairs,
            vec![("a".to_string(), "b".to_string())],
            "three distinct explicit emergent groups (0/1/2) with faction=None must still pair across groups via are_hostile (ascending scan binds a-b first, leaving c). got {pairs:?}"
        );
        // 把 max 提到能配多对，再确认跨组确实两两敌对：现在加一个组1的 d 让 c 有异组对手。
        let mut a2 = snapshot("a", "z", Realm::Induce, None);
        let mut b2 = snapshot("b", "z", Realm::Induce, None);
        let mut c2 = snapshot("c", "z", Realm::Induce, None);
        let mut d2 = snapshot("d", "z", Realm::Induce, None);
        a2.emergent_group = Some(EmergentGroupId(0));
        b2.emergent_group = Some(EmergentGroupId(1));
        c2.emergent_group = Some(EmergentGroupId(2));
        d2.emergent_group = Some(EmergentGroupId(0));
        let store2 = store_with(vec![a2, b2, c2, d2]);
        let pairs2 = collect_zone_combat_pairs(&store2, &factions, &config_with_max(5));
        assert_eq!(
            pairs2,
            vec![
                ("a".to_string(), "b".to_string()),
                ("c".to_string(), "d".to_string()),
            ],
            "with groups 0,1,2,0 ascending (a,b,c,d), cross-group pairs (a∈0 vs b∈1)+(c∈2 vs d∈0) must both form — proving >2 groups all fight each other. got {pairs2:?}"
        );
    }

    #[test]
    fn explicit_group_overrides_faction_derivation() {
        // 显式 emergent_group 优先于 faction 派生：两个 NPC 都 faction=Attack（派生同为组0，
        // 旧路径 ⇒ 不敌对），但显式群体 2 vs 1（不同）⇒ are_hostile ⇒ 成对。
        // 用群体 2（不用 0）确保不与 Attack 的派生值 0 巧合相等，精确锁住"显式覆盖派生"。
        let mut a = snapshot("a", "z", Realm::Induce, Some(FactionId::Attack));
        let mut b = snapshot("b", "z", Realm::Induce, Some(FactionId::Attack));
        a.emergent_group = Some(EmergentGroupId(2));
        b.emergent_group = Some(EmergentGroupId(1));
        let store = store_with(vec![a, b]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(3));
        assert_eq!(
            pairs,
            vec![("a".to_string(), "b".to_string())],
            "both NPCs have faction=Attack (would derive to group 0 ⇒ same ⇒ no fight), but explicit emergent_group 2 vs 1 must override the derivation and make them hostile. got {pairs:?}"
        );
    }

    #[test]
    fn same_explicit_group_is_not_hostile() {
        // 同显式群体不敌对：两个 NPC 都 emergent_group=2（即便 faction 不同）⇒ 不成对。
        // 用 Attack/Defend 不同 faction 故意制造"旧路径会敌对"的反差，确认现在按群体身份判定。
        let mut a = snapshot("a", "z", Realm::Induce, Some(FactionId::Attack));
        let mut b = snapshot("b", "z", Realm::Induce, Some(FactionId::Defend));
        a.emergent_group = Some(EmergentGroupId(2));
        b.emergent_group = Some(EmergentGroupId(2));
        let store = store_with(vec![a, b]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(3));
        assert!(
            pairs.is_empty(),
            "two NPCs in the SAME explicit emergent group (both 2) must NOT be hostile even though their factions (Attack/Defend) would have been hostile under the old path; got {pairs:?}"
        );
    }

    // ───────────────────────── 上限：zone cap + candidate cap-by-power ───────────

    #[test]
    fn zone_caps_combats_to_max_per_zone() {
        // 远超 2N 的敌对候选：交替 Attack/Defend 共 20 个 ⇒ 理论可配 10 对，
        // 但 candidate cap = 2 * max，pair cap = max。max=3 ⇒ 候选截到 6 个 ⇒ 至多 3 对，
        // 再被 pair cap 卡到 3。断言配对数恰为 cap。
        let max = 3u32;
        let mut snaps = Vec::new();
        for i in 0..20 {
            let faction = if i % 2 == 0 {
                FactionId::Attack
            } else {
                FactionId::Defend
            };
            // 用零填充让 char_id 升序 == 数值升序（n00..n19）。
            snaps.push(snapshot(
                &format!("n{i:02}"),
                "z",
                Realm::Induce,
                Some(faction),
            ));
        }
        let store = store_with(snaps);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(max));
        assert_eq!(
            pairs.len(),
            max as usize,
            "with >2N alternating hostile candidates the per-zone pair count must equal the cap {max}, got {}",
            pairs.len()
        );
    }

    #[test]
    fn zero_max_combats_yields_no_pairs() {
        let store = store_with(vec![
            snapshot("a", "z", Realm::Induce, Some(FactionId::Attack)),
            snapshot("b", "z", Realm::Induce, Some(FactionId::Defend)),
        ]);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(0));
        assert!(
            pairs.is_empty(),
            "max_combats_per_zone=0 must disable all combat (boundary off-by-one guard), got {pairs:?}"
        );
    }

    #[test]
    fn candidate_cap_keeps_strongest_2n() {
        // candidate cap 取战力前 2N。构造一个低境界(Awaken) Attack + 高境界(Void) Defend +
        // 一堆中境界(Induce) 同派系填充。max=1 ⇒ candidate_cap=2 ⇒ 只留战力前 2 名。
        // 前 2 名应是 Void(最强) 与某中境界——但我们要的是验证"cap 后仍能配出强者对"。
        //
        // 更精确：让最强两名互为敌对（Void-Defend + Spirit-Attack），其余都是弱 Neutral
        // 填充。candidate_cap=2 后只剩这两名强者，且它们敌对 ⇒ 配出 1 对。
        let mut snaps = vec![
            snapshot("aaa_strong_def", "z", Realm::Void, Some(FactionId::Defend)),
            snapshot(
                "aab_strong_atk",
                "z",
                Realm::Spirit,
                Some(FactionId::Attack),
            ),
        ];
        // 大量弱填充（Awaken Neutral），char_id 排在前面也无所谓——cap 按战力不按 id。
        for i in 0..10 {
            snaps.push(snapshot(
                &format!("weak_{i:02}"),
                "z",
                Realm::Awaken,
                Some(FactionId::Neutral),
            ));
        }
        let store = store_with(snaps);
        let factions = FactionStore::default();
        let pairs = collect_zone_combat_pairs(&store, &factions, &config_with_max(1));
        assert_eq!(
            pairs,
            vec![(
                "aaa_strong_def".to_string(),
                "aab_strong_atk".to_string()
            )],
            "candidate cap (2N) must keep the two strongest NPCs by power (Void+Spirit), and since they are hostile they form the single pair; weak Neutral fillers are dropped before pairing. got {pairs:?}"
        );
    }

    // ───────────────────────── 胜负 roll：realm 倾向 + 50/50 + 退化 ──────────────

    #[test]
    fn roll_higher_realm_wins_more_often() {
        // 高境界(Void) vs 低境界(Awaken)，跑 N 个确定 seed：高境界战死率应 < 50%。
        let high = snapshot("high", "z", Realm::Void, Some(FactionId::Attack));
        let low = snapshot("low", "z", Realm::Awaken, Some(FactionId::Defend));
        let n = 2000u64;
        let mut high_deaths = 0u64;
        for seed in 0..n {
            let loser = roll_dormant_combat_death(&high, &low, 7, seed)
                .expect("distinct ids must yield a loser");
            if loser == "high" {
                high_deaths += 1;
            }
        }
        let rate = high_deaths as f64 / n as f64;
        assert!(
            rate < 0.5,
            "a Void NPC should statistically die less than 50% of the time against an Awaken NPC because its survival weight (power ratio) is far higher; observed high-realm death rate {rate:.3} over {n} seeds"
        );
        // 进一步：远高于对手 ⇒ 死亡率应明显偏低（留宽容差防 flaky）。
        assert!(
            rate < 0.2,
            "Void's power dominates Awaken so its death rate should be well below 0.2, observed {rate:.3}"
        );
    }

    #[test]
    fn same_realm_is_fifty_fifty() {
        // 同境界 ⇒ ratio≈0.5 ⇒ 战死大约五五开。跑 N seed，容差 ±0.07。
        let a = snapshot("alpha", "z", Realm::Condense, Some(FactionId::Attack));
        let b = snapshot("beta", "z", Realm::Condense, Some(FactionId::Defend));
        let n = 4000u64;
        let mut a_deaths = 0u64;
        for seed in 0..n {
            let loser = roll_dormant_combat_death(&a, &b, 11, seed)
                .expect("distinct ids must yield a loser");
            if loser == "alpha" {
                a_deaths += 1;
            }
        }
        let rate = a_deaths as f64 / n as f64;
        assert!(
            (rate - 0.5).abs() < 0.07,
            "equal-realm equal-power combat must split deaths ~50/50 (ratio≈0.5); observed 'alpha' death rate {rate:.3} over {n} seeds, expected within 0.07 of 0.5"
        );
    }

    #[test]
    fn invalid_self_combat_returns_none() {
        let a = snapshot("same", "z", Realm::Induce, Some(FactionId::Attack));
        let b = snapshot("same", "z", Realm::Induce, Some(FactionId::Defend));
        assert_eq!(
            roll_dormant_combat_death(&a, &b, 1, 1),
            None,
            "an NPC cannot fight itself (a.char_id == b.char_id); must return None instead of fabricating a self-death"
        );
    }

    #[test]
    fn fight_is_always_lethal_for_distinct_ids() {
        // 任意有效对（含双方零战力退化）都必致死 ⇒ 恒 Some(loser)。
        let a = snapshot("a", "z", Realm::Awaken, Some(FactionId::Attack));
        let b = snapshot("b", "z", Realm::Awaken, Some(FactionId::Defend));
        for seed in 0..50u64 {
            let loser = roll_dormant_combat_death(&a, &b, seed, seed);
            assert!(
                loser.is_some(),
                "a fight between distinct NPCs is always lethal ⇒ must return Some(loser), got None at seed {seed}"
            );
            let loser = loser.unwrap();
            assert!(
                loser == "a" || loser == "b",
                "the loser must be one of the two combatants, got {loser}"
            );
        }
    }

    #[test]
    fn degenerate_zero_power_falls_back_to_half() {
        // 双方真元拉到 0 ⇒ condition_factor 落到 floor(0.15)，战力非零但相等；
        // 这里更直接：用真正零战力构造（realm Awaken + qi=0）逼近退化路径。
        // 即便 total>0，相等战力 ⇒ ratio=0.5。跑 N seed 验证近五五开（与 same_realm 同源，
        // 但显式覆盖"qi 耗尽"边界，确认不 panic 且分布对称）。
        let mut a = snapshot("a", "z", Realm::Awaken, Some(FactionId::Attack));
        let mut b = snapshot("b", "z", Realm::Awaken, Some(FactionId::Defend));
        a.cultivation.qi_current = 0.0;
        b.cultivation.qi_current = 0.0;
        let n = 3000u64;
        let mut a_deaths = 0u64;
        for seed in 0..n {
            let loser = roll_dormant_combat_death(&a, &b, 3, seed)
                .expect("distinct ids must yield a loser even at zero qi");
            if loser == "a" {
                a_deaths += 1;
            }
        }
        let rate = a_deaths as f64 / n as f64;
        assert!(
            (rate - 0.5).abs() < 0.07,
            "two equal-power (qi-exhausted) NPCs must still split ~50/50 without panic/divide-by-zero; observed 'a' death rate {rate:.3}, expected within 0.07 of 0.5"
        );
    }

    // ───────────────────────── 确定性：same seed / different seed ────────────────

    #[test]
    fn same_seed_same_outcome() {
        let a = snapshot("a", "z", Realm::Induce, Some(FactionId::Attack));
        let b = snapshot("b", "z", Realm::Condense, Some(FactionId::Defend));
        let first = roll_dormant_combat_death(&a, &b, 42, 99);
        let second = roll_dormant_combat_death(&a, &b, 42, 99);
        assert_eq!(
            first, second,
            "identical (a,b,tick,seed) must produce an identical loser (deterministic RNG); got {first:?} then {second:?}"
        );
    }

    #[test]
    fn different_seed_different_outcome() {
        // 换 seed 应能改变结果——在一段 seed sweep 上至少出现一次分歧。
        let a = snapshot("a", "z", Realm::Condense, Some(FactionId::Attack));
        let b = snapshot("b", "z", Realm::Condense, Some(FactionId::Defend));
        let baseline = roll_dormant_combat_death(&a, &b, 5, 0);
        let diverged =
            (1..200u64).any(|seed| roll_dormant_combat_death(&a, &b, 5, seed) != baseline);
        assert!(
            diverged,
            "varying the seed must be able to change the loser for an equal-power match; no seed in 1..200 diverged from baseline {baseline:?} (RNG seed is not actually wired through)"
        );
    }

    #[test]
    fn tick_changes_outcome_distribution() {
        // tick 也进 hash ⇒ 同 seed 不同 tick 应能产生不同结果（确认 tick 真的进了 RNG）。
        let a = snapshot("a", "z", Realm::Condense, Some(FactionId::Attack));
        let b = snapshot("b", "z", Realm::Condense, Some(FactionId::Defend));
        let baseline = roll_dormant_combat_death(&a, &b, 0, 7);
        let diverged =
            (1..200u64).any(|tick| roll_dormant_combat_death(&a, &b, tick, 7) != baseline);
        assert!(
            diverged,
            "varying the tick must be able to change the loser (tick is mixed into deterministic_hash); no tick in 1..200 diverged from baseline {baseline:?}"
        );
    }

    // ───────────────────────── 克制式遗物判定 should_leave_relic（P3 §139） ────────

    /// 造一个指定 archetype 的快照（faction / realm 可选），用于遗物克制判定测试。
    fn relic_snapshot(
        archetype: NpcArchetype,
        realm: Realm,
        faction: Option<FactionId>,
    ) -> NpcDormantSnapshot {
        let mut snap = snapshot("relic_subject", "z", realm, faction);
        snap.archetype = archetype;
        snap
    }

    #[test]
    fn plain_rogue_leaves_no_relic() {
        // 普通无派系低境 Rogue（末法的"无名散修"）⇒ 不留遗物，避免遍地尸体违和。
        let snap = relic_snapshot(NpcArchetype::Rogue, Realm::Awaken, None);
        assert!(
            !should_leave_relic(&snap),
            "a plain factionless low-realm Rogue must NOT leave a relic (末法基调 restraint); should_leave_relic returned true"
        );
    }

    #[test]
    fn commoner_and_beast_leave_no_relic() {
        // 其它低境无派系 archetype（凡人 / 妖兽）同样不留——遗物只给"知名 / 有来历"者。
        for archetype in [NpcArchetype::Commoner, NpcArchetype::Beast] {
            let snap = relic_snapshot(archetype, Realm::Induce, None);
            assert!(
                !should_leave_relic(&snap),
                "{archetype:?} (low realm, no faction) must not leave a relic; only named/factioned/high-realm dead do"
            );
        }
    }

    #[test]
    fn disciple_archetype_leaves_relic_even_low_realm() {
        // Disciple 有宗门 → 身上必带派系信物 ⇒ 即便最低境也留遗物。
        let snap = relic_snapshot(NpcArchetype::Disciple, Realm::Awaken, None);
        assert!(
            should_leave_relic(&snap),
            "a Disciple carries a sect token by archetype, so even an Awaken-realm one must leave a relic"
        );
    }

    #[test]
    fn guardian_relic_archetype_leaves_relic_even_low_realm() {
        // GuardianRelic 是护道遗灵 → 必留遗物（engraved_plaque chance=1.0）。
        let snap = relic_snapshot(NpcArchetype::GuardianRelic, Realm::Awaken, None);
        assert!(
            should_leave_relic(&snap),
            "a GuardianRelic is a relic-guardian by nature and must leave a relic regardless of realm"
        );
    }

    #[test]
    fn factioned_low_realm_rogue_leaves_relic() {
        // 关键正例：低境 Rogue 但**有派系** ⇒ 留遗物（结派散修带信物）。
        // 单测同时锁住"faction.is_some() 这一支独立于 archetype/realm 生效"。
        let snap = relic_snapshot(NpcArchetype::Rogue, Realm::Awaken, Some(FactionId::Attack));
        assert!(
            should_leave_relic(&snap),
            "a factioned Rogue (even Awaken realm) must leave a relic because faction membership implies a sect token; faction branch failed"
        );
    }

    #[test]
    fn neutral_faction_low_realm_rogue_still_leaves_relic() {
        // faction.is_some() 不区分 faction_id：Neutral 也算"结派"⇒ 留遗物。
        // （worldview：带派系信物即可，敌对与否是战斗判定的事，不是遗物判定。）
        let snap = relic_snapshot(NpcArchetype::Rogue, Realm::Awaken, Some(FactionId::Neutral));
        assert!(
            should_leave_relic(&snap),
            "any faction membership (incl. Neutral) implies a token ⇒ relic; should_leave_relic only checks faction.is_some()"
        );
    }

    #[test]
    fn solidify_realm_rogue_leaves_relic_boundary() {
        // 境界门槛 off-by-one 边界：恰好固元（ordinal==3）⇒ 留遗物（>= 门槛）。
        let snap = relic_snapshot(NpcArchetype::Rogue, Realm::Solidify, None);
        assert!(
            should_leave_relic(&snap),
            "a Solidify-realm Rogue is exactly at the threshold (ordinal 3 >= 3) and must leave a relic; off-by-one in realm gate"
        );
    }

    #[test]
    fn condense_realm_rogue_leaves_no_relic_just_below_threshold() {
        // 境界门槛 off-by-one 边界：凝脉（ordinal==2，固元下一档）⇒ 不留遗物（< 门槛）。
        let snap = relic_snapshot(NpcArchetype::Rogue, Realm::Condense, None);
        assert!(
            !should_leave_relic(&snap),
            "a Condense-realm Rogue (ordinal 2 < 3) is just below the relic threshold and must NOT leave a relic; gate must be >= Solidify not >= Condense"
        );
    }

    #[test]
    fn high_realm_factionless_rogue_leaves_relic() {
        // 高境（化虚 ordinal==5）无派系 Rogue ⇒ realm 支撑留遗物（独立于 faction/archetype）。
        let snap = relic_snapshot(NpcArchetype::Rogue, Realm::Void, None);
        assert!(
            should_leave_relic(&snap),
            "a Void-realm factionless Rogue must leave a relic via the realm>=Solidify branch alone"
        );
    }

    #[test]
    fn relic_threshold_const_matches_solidify_ordinal() {
        // 锁死 const 与 realm_ordinal(Solidify) 的等价——enum 重排即撞红（不靠 debug_assert）。
        assert_eq!(
            RELIC_REALM_THRESHOLD,
            realm_ordinal(Realm::Solidify),
            "RELIC_REALM_THRESHOLD must equal realm_ordinal(Solidify); a realm enum reorder silently broke the relic gate"
        );
    }

    // ───────────────────────── 遗物 loot 种子 relic_loot_seed（P3 §142） ──────────

    #[test]
    fn relic_loot_seed_is_deterministic_for_same_inputs() {
        let id = "dormant:fallen:disciple".to_string();
        let first = relic_loot_seed(&id, 1234, 42);
        let second = relic_loot_seed(&id, 1234, 42);
        assert_eq!(
            first, second,
            "identical (char_id, tick, sim_seed) must yield an identical loot seed so a relic re-rolls the same loot on hydrate; got {first} then {second}"
        );
    }

    #[test]
    fn relic_loot_seed_varies_with_sim_seed() {
        // BONG_SIM_SEED 必须真的影响 loot 种子（确定性可控，不同 sim_seed 不同 loot）。
        let id = "dormant:fallen:disciple".to_string();
        let baseline = relic_loot_seed(&id, 1234, 0);
        let diverged = (1..200u64).any(|sim| relic_loot_seed(&id, 1234, sim) != baseline);
        assert!(
            diverged,
            "varying sim_seed must change the loot seed (sim_seed is XORed in); no sim_seed in 1..200 diverged from baseline {baseline}"
        );
    }

    #[test]
    fn relic_loot_seed_varies_with_char_id() {
        // 不同战死者（char_id）应得不同 loot 种子，避免全场遗物掉一样的东西。
        let tick = 1234u64;
        let sim = 42u64;
        let a = relic_loot_seed(&"dormant:a".to_string(), tick, sim);
        let b = relic_loot_seed(&"dormant:b".to_string(), tick, sim);
        assert_ne!(
            a, b,
            "different fallen NPCs (char_id) should get different loot seeds; deterministic_hash collapsed distinct ids to the same seed"
        );
    }
}
