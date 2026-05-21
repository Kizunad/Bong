//! NPC 交易库存（plan-npc-combat-gear-v1 §8.1 #3）。
//!
//! `NpcTradeInventory` component 在 spawn 时由 `assign_npc_trade_inventory()` 一次性生成。
//! 交易物品与装备独立——散修可能卖灵草但自己用剑战斗，两者不矛盾。
//!
//! 定价走骨币枚数（整数），server 端按 shelflife 模块的骨币真元含量做折算——
//! GUI 层不暴露半衰期细节。

use valence::prelude::{bevy_ecs, Component};

use crate::cultivation::components::Realm;
use crate::cultivation::technique_scroll::realm_rank;
use crate::npc::lifecycle::NpcArchetype;

// ─── splitmix64 helpers (deterministic RNG) ──────────────────────────────────

fn splitmix64(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn splitmix64_range(seed: u64, n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let bits = ((splitmix64(seed) >> 40) & 0x00FF_FFFF) as u32;
    let unit = bits as f32 / (1u32 << 24) as f32;
    (unit * n as f32) as u32 % n
}

// ─── TradeOffer ─────────────────────────────────────────────────────────────

/// 单条交易报价。
#[derive(Clone, Debug, PartialEq)]
pub struct TradeOffer {
    /// 物品模板 ID（对应 `ItemTemplateRegistry` 的 key）。
    pub template_id: String,
    /// 展示用中文名称（下发到 client HUD）。
    pub display_name: String,
    /// 可供购买的数量。
    pub count: u32,
    /// 骨币定价（整数枚）。
    pub price_bone_coins: u32,
}

// ─── NpcTradeInventory ──────────────────────────────────────────────────────

/// NPC 可售卖物品列表（spawn 时一次性生成，§8.1 #3 方案 B）。
///
/// 通过 `bong:npc_metadata` S2C packet 下发给 client。
#[derive(Clone, Debug, Default, Component)]
pub struct NpcTradeInventory {
    pub offers: Vec<TradeOffer>,
}

// ─── Static trade catalogue ─────────────────────────────────────────────────

/// 候选交易物品条目（静态定义，按 realm_min 过滤）。
struct CatalogueEntry {
    template_id: &'static str,
    display_name: &'static str,
    realm_min: u8,
    price_bone_coins: u32,
    count_min: u32,
    count_max: u32,
}

/// 交易物品候选库——覆盖灵草/丹药/残卷/工具/材料等类目。
/// 各 realm tier 逐步解锁高阶物品。
const TRADE_CATALOGUE: &[CatalogueEntry] = &[
    // Awaken tier
    CatalogueEntry {
        template_id: "lingcao",
        display_name: "灵草",
        realm_min: 0,
        price_bone_coins: 12,
        count_min: 1,
        count_max: 5,
    },
    CatalogueEntry {
        template_id: "bone_meal",
        display_name: "骨粉",
        realm_min: 0,
        price_bone_coins: 5,
        count_min: 2,
        count_max: 8,
    },
    CatalogueEntry {
        template_id: "rough_bandage",
        display_name: "粗布绷带",
        realm_min: 0,
        price_bone_coins: 8,
        count_min: 1,
        count_max: 3,
    },
    // Induce tier
    CatalogueEntry {
        template_id: "fragment_scroll",
        display_name: "残卷",
        realm_min: 1,
        price_bone_coins: 45,
        count_min: 1,
        count_max: 2,
    },
    CatalogueEntry {
        template_id: "qi_condensing_powder",
        display_name: "凝气散",
        realm_min: 1,
        price_bone_coins: 30,
        count_min: 1,
        count_max: 3,
    },
    // Condense tier
    CatalogueEntry {
        template_id: "meridian_salve",
        display_name: "通脉膏",
        realm_min: 2,
        price_bone_coins: 60,
        count_min: 1,
        count_max: 2,
    },
    CatalogueEntry {
        template_id: "spirit_stone_shard",
        display_name: "灵石碎片",
        realm_min: 2,
        price_bone_coins: 80,
        count_min: 1,
        count_max: 3,
    },
    // Solidify tier
    CatalogueEntry {
        template_id: "bone_reinforcing_pill",
        display_name: "固骨丹",
        realm_min: 3,
        price_bone_coins: 120,
        count_min: 1,
        count_max: 2,
    },
    // Spirit tier
    CatalogueEntry {
        template_id: "spirit_jade",
        display_name: "灵玉",
        realm_min: 4,
        price_bone_coins: 200,
        count_min: 1,
        count_max: 1,
    },
];

// ─── assign_npc_trade_inventory ─────────────────────────────────────────────

/// 根据 archetype / realm 生成 NPC 交易库存。
///
/// 分配规则（§8.1 #3）：
/// - `Commoner`: 1-2 件 Awaken 物品
/// - `Rogue`: 1-3 件，按 realm 解锁
/// - `Disciple`: 2-4 件，按 realm 解锁
/// - `GuardianRelic` / `Daoxiang` / `Zhinian` / `Beast` / `SkullFiend` / `Fuya` / `Zombie`: 不可交易
pub fn assign_npc_trade_inventory(
    archetype: NpcArchetype,
    realm: Realm,
    entity_seed: u64,
) -> NpcTradeInventory {
    let npc_rank = realm_rank(realm);

    let (offer_min, offer_max) = match archetype {
        NpcArchetype::Commoner => (1, 2),
        NpcArchetype::Rogue => (1, 3),
        NpcArchetype::Disciple => (2, 4),
        // 非交易 archetype
        NpcArchetype::GuardianRelic
        | NpcArchetype::Daoxiang
        | NpcArchetype::Zhinian
        | NpcArchetype::Beast
        | NpcArchetype::SkullFiend
        | NpcArchetype::Fuya
        | NpcArchetype::Zombie => {
            return NpcTradeInventory { offers: Vec::new() };
        }
    };

    // 过滤可用物品
    let available: Vec<&CatalogueEntry> = TRADE_CATALOGUE
        .iter()
        .filter(|entry| entry.realm_min <= npc_rank)
        .collect();

    if available.is_empty() {
        return NpcTradeInventory { offers: Vec::new() };
    }

    // 决定物品数量
    let range = (offer_max - offer_min + 1) as u32;
    let count = offer_min + splitmix64_range(entity_seed, range) as usize;
    let count = count.min(available.len());

    // Fisher-Yates shuffle 选前 count 个
    let mut indices: Vec<usize> = (0..available.len()).collect();
    for i in (1..indices.len()).rev() {
        let j = splitmix64_range(
            entity_seed.wrapping_add(i as u64 * 0x9E37_79B9),
            (i + 1) as u32,
        ) as usize;
        indices.swap(i, j);
    }

    let offers: Vec<TradeOffer> = indices
        .iter()
        .take(count)
        .enumerate()
        .map(|(idx, &orig_idx)| {
            let entry = available[orig_idx];
            let count_range = entry.count_max - entry.count_min + 1;
            let count_seed = entity_seed.wrapping_add(idx as u64 * 0xBF58_476D);
            let item_count = entry.count_min + splitmix64_range(count_seed, count_range);
            TradeOffer {
                template_id: entry.template_id.to_string(),
                display_name: entry.display_name.to_string(),
                count: item_count,
                price_bone_coins: entry.price_bone_coins,
            }
        })
        .collect();

    NpcTradeInventory { offers }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // === Non-trading archetypes return empty ===

    #[test]
    fn guardian_relic_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::GuardianRelic, Realm::Spirit, 42);
        assert!(
            inv.offers.is_empty(),
            "GuardianRelic should have no trade inventory"
        );
    }

    #[test]
    fn daoxiang_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::Daoxiang, Realm::Condense, 42);
        assert!(
            inv.offers.is_empty(),
            "Daoxiang should have no trade inventory"
        );
    }

    #[test]
    fn zhinian_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::Zhinian, Realm::Condense, 42);
        assert!(
            inv.offers.is_empty(),
            "Zhinian should have no trade inventory"
        );
    }

    #[test]
    fn beast_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::Beast, Realm::Awaken, 42);
        assert!(
            inv.offers.is_empty(),
            "Beast should have no trade inventory"
        );
    }

    #[test]
    fn skull_fiend_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::SkullFiend, Realm::Void, 42);
        assert!(
            inv.offers.is_empty(),
            "SkullFiend should have no trade inventory"
        );
    }

    #[test]
    fn fuya_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::Fuya, Realm::Spirit, 42);
        assert!(inv.offers.is_empty(), "Fuya should have no trade inventory");
    }

    #[test]
    fn zombie_returns_empty() {
        let inv = assign_npc_trade_inventory(NpcArchetype::Zombie, Realm::Awaken, 42);
        assert!(
            inv.offers.is_empty(),
            "Zombie should have no trade inventory"
        );
    }

    // === Trading archetypes produce correct counts ===

    #[test]
    fn commoner_returns_1_to_2() {
        for seed in 0..50u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Commoner, Realm::Awaken, seed);
            assert!(
                !inv.offers.is_empty() && inv.offers.len() <= 2,
                "commoner should have 1-2 offers, got {} (seed={})",
                inv.offers.len(),
                seed
            );
            for offer in &inv.offers {
                assert!(offer.price_bone_coins > 0, "price should be positive");
                assert!(offer.count >= 1, "count should be at least 1");
                assert!(
                    !offer.template_id.is_empty(),
                    "template_id should not be empty"
                );
                assert!(
                    !offer.display_name.is_empty(),
                    "display_name should not be empty"
                );
            }
        }
    }

    #[test]
    fn rogue_returns_1_to_3() {
        for seed in 0..50u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Rogue, Realm::Condense, seed);
            assert!(
                !inv.offers.is_empty() && inv.offers.len() <= 3,
                "rogue should have 1-3 offers, got {} (seed={})",
                inv.offers.len(),
                seed
            );
        }
    }

    #[test]
    fn disciple_returns_2_to_4() {
        for seed in 0..50u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Disciple, Realm::Condense, seed);
            assert!(
                inv.offers.len() >= 2 && inv.offers.len() <= 4,
                "disciple should have 2-4 offers, got {} (seed={})",
                inv.offers.len(),
                seed
            );
        }
    }

    // === Realm gating ===

    #[test]
    fn awaken_only_gets_awaken_tier_items() {
        for seed in 0..100u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Rogue, Realm::Awaken, seed);
            for offer in &inv.offers {
                let entry = TRADE_CATALOGUE
                    .iter()
                    .find(|e| e.template_id == offer.template_id)
                    .expect("offer should reference a catalogue entry");
                assert!(
                    entry.realm_min == 0,
                    "Awaken NPC should only get realm_min=0 items, got {} with realm_min={}",
                    offer.template_id,
                    entry.realm_min
                );
            }
        }
    }

    #[test]
    fn higher_realm_unlocks_more_items() {
        // Spirit realm should have access to more items than Awaken
        let awaken_available: Vec<_> = TRADE_CATALOGUE
            .iter()
            .filter(|e| e.realm_min == 0)
            .collect();
        let spirit_available: Vec<_> = TRADE_CATALOGUE
            .iter()
            .filter(|e| e.realm_min <= 4)
            .collect();
        assert!(
            spirit_available.len() > awaken_available.len(),
            "Spirit realm should unlock more items than Awaken ({} vs {})",
            spirit_available.len(),
            awaken_available.len()
        );
    }

    // === Determinism ===

    #[test]
    fn assign_trade_inventory_deterministic() {
        for archetype in [
            NpcArchetype::Commoner,
            NpcArchetype::Rogue,
            NpcArchetype::Disciple,
        ] {
            let a = assign_npc_trade_inventory(archetype, Realm::Condense, 12345);
            let b = assign_npc_trade_inventory(archetype, Realm::Condense, 12345);
            assert_eq!(
                a.offers.len(),
                b.offers.len(),
                "same seed should produce same count for {:?}",
                archetype
            );
            for (oa, ob) in a.offers.iter().zip(b.offers.iter()) {
                assert_eq!(
                    oa.template_id, ob.template_id,
                    "same seed should produce same items for {:?}",
                    archetype
                );
                assert_eq!(
                    oa.count, ob.count,
                    "same seed should produce same counts for {:?}",
                    archetype
                );
            }
        }
    }

    // === Count bounds within catalogue spec ===

    #[test]
    fn item_counts_within_catalogue_bounds() {
        for seed in 0..200u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Rogue, Realm::Spirit, seed);
            for offer in &inv.offers {
                let entry = TRADE_CATALOGUE
                    .iter()
                    .find(|e| e.template_id == offer.template_id)
                    .expect("offer should reference a catalogue entry");
                assert!(
                    offer.count >= entry.count_min && offer.count <= entry.count_max,
                    "count {} for {} should be in [{}, {}]",
                    offer.count,
                    offer.template_id,
                    entry.count_min,
                    entry.count_max
                );
            }
        }
    }

    // === No duplicates ===

    #[test]
    fn no_duplicate_offers() {
        for seed in 0..100u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Disciple, Realm::Spirit, seed);
            let mut seen = std::collections::HashSet::new();
            for offer in &inv.offers {
                assert!(
                    seen.insert(&offer.template_id),
                    "duplicate template_id {} in trade inventory (seed={})",
                    offer.template_id,
                    seed
                );
            }
        }
    }

    // === Price correctness ===

    #[test]
    fn prices_match_catalogue() {
        for seed in 0..100u64 {
            let inv = assign_npc_trade_inventory(NpcArchetype::Rogue, Realm::Spirit, seed);
            for offer in &inv.offers {
                let entry = TRADE_CATALOGUE
                    .iter()
                    .find(|e| e.template_id == offer.template_id)
                    .expect("offer should reference a catalogue entry");
                assert_eq!(
                    offer.price_bone_coins, entry.price_bone_coins,
                    "price for {} should match catalogue ({} vs {})",
                    offer.template_id, offer.price_bone_coins, entry.price_bone_coins
                );
            }
        }
    }
}
