//! plan-offscreen-war-v1 P5：散修群体消长 census（**纯逻辑核心**，reframe b）。
//!
//! 末法残土无具名宗门——离屏散修在某 zone 自发聚成匿名涌现集体（[`EmergentGroupId`]）。本模块
//! 把「每个涌现群体当前多少人、最集中在哪、谁是涌现强者、相对上轮是涨是落」从 telemetry publish
//! 里解耦出来，做成可被饱和单测完全锁住的纯函数：
//!
//! - [`compute_faction_census`]：遍历 dormant store 的**存活**快照，按 [`effective_group`] 分组，
//!   逐组算 population / dominant_zone（众数）/ region_descriptor / 涌现强者（最高境界活体）。
//! - [`assign_status`]：对比上轮人口（[`LastFactionCensus`]）判 Rising / Stable / Waning。
//!
//! **守恒红线（§10.1 #5）**：全模块只读 `&store` / `&faction_store` / `&LastFactionCensus`，零
//! mutation、零 ledger、零真元流动。强者陨落（最高境界活体被移出 store）只让 census 读到更少人口
//! + 更低强者境界 → `assign_status` 判 Waning，**绝不**在此特殊处理真元（残余真元守恒仍唯一走
//! P2 的 `release_dormant_qi_to_zone`）。涌现强者是 Q2 派生焦点，**无号令权**——不是宗主 / 掌门，
//! 只是恰好境界最高的散修，census 也只读它做盘面，不赋予它任何指挥语义。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Resource};

use super::{effective_group, NpcDormantStore};
use crate::cultivation::components::Realm;
use crate::npc::combat_power::realm_ordinal;
use crate::npc::faction::{EmergentGroupId, FactionStore, GroupStatus};
use crate::social::components::CharId;

/// 一个涌现群体某轮的人口盘面（[`compute_faction_census`] 的逐组产出）。
///
/// `strongest_*` 是该组**涌现强者**（最高 `realm_ordinal` 的活体；平局取 char_id 字典序最小，
/// 确定性）。强者无号令权（reframe b：匿名涌现集体无宗主 / 掌门），census 仅读它做盘面。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FactionCensusEntry {
    pub group_id: EmergentGroupId,
    /// `"{dominant_zone}一带散修"`——匿名区域描述符（无具名宗门）。
    pub region_descriptor: String,
    pub population: u32,
    /// 该组成员最集中的 zone（zone_name 众数；平票取字典序最小）。
    pub dominant_zone: String,
    /// 涌现强者境界（最高 realm 活体）。
    pub strongest_realm: Realm,
    /// 涌现强者 char_id（最高 realm 活体；平局取字典序最小）。
    pub strongest_char_id: CharId,
}

/// 上一轮每个涌现群体的人口快照（[`assign_status`] 据此判消长）。
///
/// 由 telemetry publish system（`network::npc_event_bridge::publish_faction_state`）每轮 census
/// 后写回。`Default` = 空 map（首轮没有历史 → 所有当前群体都判 Rising，新涌现即上升）。
#[derive(Resource, Default)]
pub struct LastFactionCensus(pub HashMap<EmergentGroupId, u32>);

/// 散修群体消长 census（纯函数，零 mutation / 零真元）。
///
/// 遍历 `store.snapshots` 的**存活**快照（跳过 `combat_dead_pending_release`——已离屏战死、真元
/// 待释放的「逻辑死亡」单元不计入人口），按 [`effective_group`] 分组（`None` 即无群体身份的散修，
/// 如中立散修，跳过不计）。每组：
/// - `population` = 该组存活成员计数；
/// - `dominant_zone` = 成员 `zone_name` 众数（计数最高；**平票取字典序最小**，保证确定性、跨
///   重启稳定，不靠 HashMap 迭代顺序）；
/// - `region_descriptor` = `"{dominant_zone}一带散修"`（匿名区域描述符，无具名宗门）；
/// - 涌现强者 = 该组 `realm_ordinal` 最高的成员（**平局取 char_id 字典序最小**）→ `strongest_realm`
///   / `strongest_char_id`。
///
/// 返回 **按 `group_id` 升序** 的 `Vec<FactionCensusEntry>`（确定性输出，便于 telemetry 稳定排序
/// 与测试断言）。空 store / 全 `None` 群体 → 空 Vec（不产 entry）。
pub(crate) fn compute_faction_census(
    store: &NpcDormantStore,
    faction_store: &FactionStore,
) -> Vec<FactionCensusEntry> {
    // 逐组累积器：population + 每 zone 计数（算众数）+ 当前涌现强者候选。
    struct GroupAccum {
        population: u32,
        zone_counts: HashMap<String, u32>,
        strongest_ordinal: u8,
        strongest_realm: Realm,
        strongest_char_id: CharId,
    }

    let mut groups: HashMap<EmergentGroupId, GroupAccum> = HashMap::new();

    for snapshot in store.snapshots.values() {
        // 已离屏战死、真元待释放的快照是逻辑死亡，不计入存活人口（与 combat phase 的
        // collect_zone_combat_pairs 对「存活」的定义一致）。
        if snapshot.combat_dead_pending_release {
            continue;
        }
        let Some(group_id) = effective_group(snapshot, faction_store) else {
            // 无群体身份（如中立散修 faction=Neutral 派生 None）——不归任何涌现群体。
            continue;
        };
        let realm = snapshot.cultivation.realm;
        let ordinal = realm_ordinal(realm);
        let char_id = snapshot.char_id.clone();
        let zone = snapshot.zone_name.clone();

        match groups.get_mut(&group_id) {
            Some(accum) => {
                accum.population += 1;
                *accum.zone_counts.entry(zone).or_insert(0) += 1;
                // 涌现强者：先比 realm_ordinal，平局取 char_id 字典序最小（确定性）。
                if ordinal > accum.strongest_ordinal
                    || (ordinal == accum.strongest_ordinal && char_id < accum.strongest_char_id)
                {
                    accum.strongest_ordinal = ordinal;
                    accum.strongest_realm = realm;
                    accum.strongest_char_id = char_id;
                }
            }
            None => {
                let mut zone_counts = HashMap::new();
                zone_counts.insert(zone, 1);
                groups.insert(
                    group_id,
                    GroupAccum {
                        population: 1,
                        zone_counts,
                        strongest_ordinal: ordinal,
                        strongest_realm: realm,
                        strongest_char_id: char_id,
                    },
                );
            }
        }
    }

    let mut entries: Vec<FactionCensusEntry> = groups
        .into_iter()
        .map(|(group_id, accum)| {
            let dominant_zone = dominant_zone_of(&accum.zone_counts);
            FactionCensusEntry {
                group_id,
                region_descriptor: region_descriptor_for(&dominant_zone),
                population: accum.population,
                dominant_zone,
                strongest_realm: accum.strongest_realm,
                strongest_char_id: accum.strongest_char_id,
            }
        })
        .collect();
    // 按 group_id 升序——确定性输出（EmergentGroupId 派生 Ord）。
    entries.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    entries
}

/// 区域描述符：`"{zone}一带散修"`（reframe b 匿名标识，无具名宗门）。
fn region_descriptor_for(dominant_zone: &str) -> String {
    format!("{dominant_zone}一带散修")
}

/// 从 zone→计数 map 取众数 zone：计数最高者；**平票取字典序最小**（确定性，不靠 HashMap 顺序）。
/// 空 map（理论上不会发生，每组至少一名成员贡献一个 zone）回退空串，绝不 panic。
fn dominant_zone_of(zone_counts: &HashMap<String, u32>) -> String {
    zone_counts
        .iter()
        .max_by(|(a_zone, a_count), (b_zone, b_count)| {
            // 先比计数（大者胜）；计数相等再比 zone 名（**小者胜** → 用反向 cmp 让 max_by 选小者）。
            a_count.cmp(b_count).then_with(|| b_zone.cmp(a_zone))
        })
        .map(|(zone, _)| zone.clone())
        .unwrap_or_default()
}

/// 判一个涌现群体相对上轮的消长状态（plan-offscreen-war-v1 P5 reframe b）。
///
/// - 上轮 census 有该组：`current > last` → Rising / `current < last` → Waning / 相等 → Stable。
/// - 上轮没有该组（**新涌现**）：→ Rising（新群体从无到有即上升）。
///
/// `last` 不含本轮新增的群体（telemetry publish 每轮 census 后才写回），故首轮所有群体都判 Rising。
pub(crate) fn assign_status(
    group_id: EmergentGroupId,
    current_pop: u32,
    last: &LastFactionCensus,
) -> GroupStatus {
    match last.0.get(&group_id) {
        Some(&last_pop) => match current_pop.cmp(&last_pop) {
            std::cmp::Ordering::Greater => GroupStatus::Rising,
            std::cmp::Ordering::Less => GroupStatus::Waning,
            std::cmp::Ordering::Equal => GroupStatus::Stable,
        },
        // 新涌现群体：上轮没有它 → 从无到有 → Rising。
        None => GroupStatus::Rising,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Cultivation;
    use crate::npc::dormant::{DormantBehaviorIntent, NpcDormantSnapshot, NpcDormantStore};
    use crate::npc::faction::{
        EmergentGroupId, FactionId, FactionMembership, FactionRank, FactionStore, GroupStatus,
        MissionQueue, Reputation,
    };
    use crate::npc::lifecycle::NpcArchetype;

    /// 造一个最小可用 dormant 快照：给定 char_id / zone / realm / 显式 emergent_group。
    /// emergent_group 显式 Some 让 census 不依赖 faction 派生回退，便于精确控制分组。
    fn snapshot(
        char_id: &str,
        zone: &str,
        realm: Realm,
        group: Option<EmergentGroupId>,
    ) -> NpcDormantSnapshot {
        let cultivation = Cultivation {
            realm,
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
            faction: None,
            emergent_group: group,
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            tsy_hostile: None,
            intent: DormantBehaviorIntent::Cultivate {
                zone: zone.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: cultivation.qi_current,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
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

    fn last_with(pairs: &[(u16, u32)]) -> LastFactionCensus {
        let mut last = LastFactionCensus::default();
        for &(id, pop) in pairs {
            last.0.insert(EmergentGroupId(id), pop);
        }
        last
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

    // ───────────────────────── compute_faction_census ─────────────────────────

    #[test]
    fn empty_store_yields_no_entries() {
        // 边界：空 store → 空 census（不产任何 entry，下游不 publish）。
        let store = NpcDormantStore::default();
        let census = compute_faction_census(&store, &FactionStore::default());
        assert!(
            census.is_empty(),
            "an empty dormant store must yield no census entries (nothing to publish); got {census:?}"
        );
    }

    #[test]
    fn group_with_only_none_membership_yields_no_entry() {
        // 边界：成员全是无群体身份（中立散修，faction=Neutral 派生 None）→ 不归任何群体 → 空 census。
        let store = store_with(vec![
            // 用 faction=Neutral + emergent_group=None，走 effective_group 的派生回退 → None。
            {
                let mut s = snapshot("dormant:rogue:0", "spawn", Realm::Awaken, None);
                s.faction = Some(membership(FactionId::Neutral));
                s
            },
            {
                let mut s = snapshot("dormant:rogue:1", "spawn", Realm::Induce, None);
                s.faction = Some(membership(FactionId::Neutral));
                s
            },
        ]);
        let census = compute_faction_census(&store, &FactionStore::default());
        assert!(
            census.is_empty(),
            "members whose effective_group is None (e.g. neutral rogues) must not form any census entry; got {census:?}"
        );
    }

    #[test]
    fn population_counts_living_members_per_group() {
        // happy：两组各计数自己的存活成员，结果按 group_id 升序。
        let store = store_with(vec![
            snapshot(
                "dormant:rogue:0",
                "spawn",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:1",
                "spawn",
                Realm::Induce,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:2",
                "rift_valley",
                Realm::Awaken,
                Some(EmergentGroupId(1)),
            ),
        ]);
        let census = compute_faction_census(&store, &FactionStore::default());
        assert_eq!(
            census.len(),
            2,
            "two distinct groups must produce two entries"
        );
        assert_eq!(
            census[0].group_id,
            EmergentGroupId(0),
            "entries must be sorted by group_id ascending: first entry should be group 0"
        );
        assert_eq!(
            census[0].population, 2,
            "group 0 has 2 living members, expected population 2, got {}",
            census[0].population
        );
        assert_eq!(census[1].group_id, EmergentGroupId(1));
        assert_eq!(
            census[1].population, 1,
            "group 1 has 1 living member, expected population 1, got {}",
            census[1].population
        );
    }

    #[test]
    fn combat_dead_pending_release_excluded_from_population() {
        // 状态转换：combat_dead_pending_release 的快照是逻辑死亡，不计入存活人口。
        let store = store_with(vec![
            snapshot(
                "dormant:rogue:0",
                "spawn",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
            {
                let mut dead = snapshot(
                    "dormant:rogue:1",
                    "spawn",
                    Realm::Void,
                    Some(EmergentGroupId(0)),
                );
                dead.combat_dead_pending_release = true;
                dead
            },
        ]);
        let census = compute_faction_census(&store, &FactionStore::default());
        assert_eq!(census.len(), 1, "still one live group entry");
        assert_eq!(
            census[0].population, 1,
            "the combat_dead_pending_release (logically dead) member must NOT be counted; \
             expected population 1 (only the live Awaken rogue), got {}",
            census[0].population
        );
        // 那个 Void 死者不该被当成涌现强者（它已逻辑死亡、被跳过）。
        assert_eq!(
            census[0].strongest_realm,
            Realm::Awaken,
            "the logically-dead Void member must not be the emergent strongest; strongest must be the live Awaken member, got {:?}",
            census[0].strongest_realm
        );
    }

    #[test]
    fn dominant_zone_is_the_mode() {
        // happy：dominant_zone 取成员 zone_name 众数（出现最多的 zone）。
        let store = store_with(vec![
            snapshot(
                "dormant:rogue:0",
                "spawn",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:1",
                "spawn",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:2",
                "rift_valley",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
        ]);
        let census = compute_faction_census(&store, &FactionStore::default());
        assert_eq!(
            census[0].dominant_zone, "spawn",
            "dominant_zone must be the mode of members' zones (spawn appears twice vs rift_valley once), got {}",
            census[0].dominant_zone
        );
    }

    #[test]
    fn dominant_zone_tie_breaks_lexicographically_smallest() {
        // 边界（确定性）：zone 计数平票时取字典序最小，不靠 HashMap 迭代顺序。
        // "alpha" 与 "omega" 各 1 票 → 取 "alpha"（字典序更小）。
        let store = store_with(vec![
            snapshot(
                "dormant:rogue:0",
                "omega",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:1",
                "alpha",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
        ]);
        let census = compute_faction_census(&store, &FactionStore::default());
        assert_eq!(
            census[0].dominant_zone, "alpha",
            "on a zone-count tie, dominant_zone must be the lexicographically smallest ('alpha' < 'omega') for determinism, got {}",
            census[0].dominant_zone
        );
    }

    #[test]
    fn region_descriptor_uses_zone_band_format() {
        // happy：region_descriptor == "{dominant_zone}一带散修"（匿名区域描述符，无具名宗门）。
        let store = store_with(vec![snapshot(
            "dormant:rogue:0",
            "rift_valley",
            Realm::Awaken,
            Some(EmergentGroupId(0)),
        )]);
        let census = compute_faction_census(&store, &FactionStore::default());
        assert_eq!(
            census[0].region_descriptor, "rift_valley一带散修",
            "region_descriptor must be the anonymous \"{{zone}}一带散修\" band descriptor (no named sect); got {}",
            census[0].region_descriptor
        );
    }

    #[test]
    fn strongest_reflects_highest_realm_member() {
        // happy：涌现强者 = 该组 realm_ordinal 最高的活体。
        let store = store_with(vec![
            snapshot(
                "dormant:rogue:0",
                "spawn",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:1",
                "spawn",
                Realm::Solidify,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:2",
                "spawn",
                Realm::Condense,
                Some(EmergentGroupId(0)),
            ),
        ]);
        let census = compute_faction_census(&store, &FactionStore::default());
        assert_eq!(
            census[0].strongest_realm,
            Realm::Solidify,
            "the emergent strongest must be the highest-realm living member (Solidify > Condense > Awaken), got {:?}",
            census[0].strongest_realm
        );
        assert_eq!(
            census[0].strongest_char_id, "dormant:rogue:1",
            "strongest_char_id must point at the Solidify member dormant:rogue:1, got {}",
            census[0].strongest_char_id
        );
    }

    #[test]
    fn strongest_tie_breaks_smallest_char_id() {
        // 边界（确定性）：同为最高境界时，涌现强者取 char_id 字典序最小。
        let store = store_with(vec![
            snapshot(
                "dormant:rogue:zzz",
                "spawn",
                Realm::Solidify,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:aaa",
                "spawn",
                Realm::Solidify,
                Some(EmergentGroupId(0)),
            ),
        ]);
        let census = compute_faction_census(&store, &FactionStore::default());
        assert_eq!(
            census[0].strongest_char_id, "dormant:rogue:aaa",
            "on a realm tie, the emergent strongest must be the lexicographically smallest char_id ('aaa' < 'zzz') for determinism, got {}",
            census[0].strongest_char_id
        );
    }

    #[test]
    fn census_uses_faction_derived_group_when_emergent_group_absent() {
        // 迁移路径：emergent_group=None 时 census 走 effective_group 的 faction 派生回退
        // （Attack→0 / Defend→1），与 combat 配对同源——旧持久化快照零迁移也能进 census。
        let store = store_with(vec![
            {
                let mut s = snapshot("dormant:rogue:0", "spawn", Realm::Awaken, None);
                s.faction = Some(membership(FactionId::Attack));
                s
            },
            {
                let mut s = snapshot("dormant:rogue:1", "spawn", Realm::Awaken, None);
                s.faction = Some(membership(FactionId::Defend));
                s
            },
        ]);
        let census = compute_faction_census(&store, &FactionStore::default());
        assert_eq!(
            census.len(),
            2,
            "Attack→group0 and Defend→group1 must form two entries via faction-derived fallback"
        );
        assert_eq!(
            census[0].group_id,
            EmergentGroupId(0),
            "Attack faction must derive to group 0"
        );
        assert_eq!(
            census[1].group_id,
            EmergentGroupId(1),
            "Defend faction must derive to group 1"
        );
    }

    // ───────────────────────── assign_status 状态转换 ─────────────────────────

    #[test]
    fn assign_status_population_increase_is_rising() {
        // 状态转换 A→Rising：人口比上轮多 → Rising。
        let last = last_with(&[(0, 3)]);
        assert_eq!(
            assign_status(EmergentGroupId(0), 5, &last),
            GroupStatus::Rising,
            "population grew 3→5, status must be Rising (a growing group is rising)"
        );
    }

    #[test]
    fn assign_status_population_decrease_is_waning() {
        // 状态转换 A→Waning：人口比上轮少 → Waning。
        let last = last_with(&[(0, 5)]);
        assert_eq!(
            assign_status(EmergentGroupId(0), 2, &last),
            GroupStatus::Waning,
            "population shrank 5→2, status must be Waning (a shrinking group is waning)"
        );
    }

    #[test]
    fn assign_status_population_unchanged_is_stable() {
        // 状态转换 A→Stable：人口与上轮相等 → Stable。
        let last = last_with(&[(0, 4)]);
        assert_eq!(
            assign_status(EmergentGroupId(0), 4, &last),
            GroupStatus::Stable,
            "population unchanged 4→4, status must be Stable"
        );
    }

    #[test]
    fn assign_status_new_group_is_rising() {
        // 状态转换 (none)→Rising：上轮没有该组（新涌现）→ Rising。
        let last = last_with(&[(0, 4)]); // 只有 group 0 的历史；group 7 是新涌现。
        assert_eq!(
            assign_status(EmergentGroupId(7), 1, &last),
            GroupStatus::Rising,
            "a newly emerged group (absent from last census) must be Rising (从无到有即上升), even at population 1"
        );
    }

    #[test]
    fn assign_status_empty_history_treats_all_as_rising() {
        // 边界：首轮（last 为空）→ 任何当前群体都判 Rising。
        let last = LastFactionCensus::default();
        assert_eq!(
            assign_status(EmergentGroupId(0), 10, &last),
            GroupStatus::Rising,
            "with an empty history (first census tick), every current group must be Rising"
        );
    }

    // ───────────── 强者陨落 → 式微（census + assign_status 联合专测） ─────────────

    #[test]
    fn strongest_falls_then_group_wanes() {
        // 核心场景（强者陨落→式微）：一组 1 个 Solidify 强者 + 3 个 Awaken。先 census 记下上轮
        // 人口（4），再移除强者（模拟离屏战死被 finalize 移出 store），重算 census：
        //   ① strongest_realm 从 Solidify 降到 Awaken（强者陨落）；
        //   ② population 从 4 减到 3（人口流失）；
        //   ③ assign_status(用上轮人口 4)→ Waning（式微）。
        let mut store = store_with(vec![
            snapshot(
                "dormant:rogue:0",
                "spawn",
                Realm::Solidify,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:1",
                "spawn",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:2",
                "spawn",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
            snapshot(
                "dormant:rogue:3",
                "spawn",
                Realm::Awaken,
                Some(EmergentGroupId(0)),
            ),
        ]);
        let faction_store = FactionStore::default();

        // 上轮 census：强者在场，population=4，strongest=Solidify。
        let before = compute_faction_census(&store, &faction_store);
        assert_eq!(before.len(), 1);
        assert_eq!(
            before[0].population, 4,
            "before: group has 4 living members"
        );
        assert_eq!(
            before[0].strongest_realm,
            Realm::Solidify,
            "before: the Solidify rogue must be the emergent strongest"
        );
        // 把上轮人口写进 LastFactionCensus（模拟 publish 后写回）。
        let mut last = LastFactionCensus::default();
        for entry in &before {
            last.0.insert(entry.group_id, entry.population);
        }

        // 强者陨落：把 Solidify 强者移出 store（守恒红线：真元释放走 P2，census 不碰 ledger）。
        store.snapshots.remove("dormant:rogue:0");
        store.rebuild_indexes();

        // 本轮 census：强者已陨落。
        let after = compute_faction_census(&store, &faction_store);
        assert_eq!(after.len(), 1, "group still exists with 3 Awaken survivors");
        assert_eq!(
            after[0].population, 3,
            "① population must drop 4→3 after the strongest falls, got {}",
            after[0].population
        );
        assert_eq!(
            after[0].strongest_realm,
            Realm::Awaken,
            "② strongest_realm must drop Solidify→Awaken after the strongest falls (remaining are all Awaken), got {:?}",
            after[0].strongest_realm
        );
        // ③ 用上轮人口判消长 → Waning。
        assert_eq!(
            assign_status(after[0].group_id, after[0].population, &last),
            GroupStatus::Waning,
            "③ with last population 4 and current 3, the group must be Waning (强者陨落 + 人口流失 → 式微)"
        );
    }
}
