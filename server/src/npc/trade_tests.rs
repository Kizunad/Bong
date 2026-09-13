use super::*;

// =========================================================================
// Catalog alignment — TRADE_CATALOGUE (展示路) 与 npc_trade_catalog_entry (买路) 一致性
// =========================================================================

/// TRADE_CATALOGUE 中每个 template_id 必须能在展示后被购买（买路有对应条目）。
/// 这是"展示的 offer == 可购清单"的核心约束，防止"能看不能买"孤岛。

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

// =========================================================================
// P3 — NpcPlayerReputation
// =========================================================================

#[test]
fn catalogue_all_entries_purchasable_via_buy_path() {
    // 用遍历 archetype 检查：TRADE_CATALOGUE 里的每个 template_id
    // 必须至少被 Commoner 或 Rogue 的买路认可。
    // 注：Disciple NPC 展示 offer 但生产买路暂无 Disciple arm——已知孤岛，后续 plan 补齐；
    // 此处故意仅覆盖 Commoner/Rogue，避免误报为"已覆盖"。
    // 非交易 archetype（GuardianRelic/Daoxiang/…）不参与校验。
    use crate::network::client_request_handler::npc_trade_catalog_entry;
    let trading_archetypes = [NpcArchetype::Commoner, NpcArchetype::Rogue];
    for entry in TRADE_CATALOGUE {
        let found = trading_archetypes
            .iter()
            .any(|arch| npc_trade_catalog_entry(*arch, entry.template_id).is_some());
        assert!(
            found,
            "TRADE_CATALOGUE 条目 '{}' 在买路（npc_trade_catalog_entry）中不存在 \
             ——展示了却买不到（孤岛），期望: Commoner 或 Rogue 买路有此 template_id，\
             实际: None",
            entry.template_id
        );
    }
}

/// TRADE_CATALOGUE 中 spirit_grass 条目的价格必须与买路对齐（均为 10 骨币）。
/// 展示价格与实际结算价格不一致会欺骗玩家。
#[test]
fn catalogue_spirit_grass_price_matches_buy_path() {
    let cat = TRADE_CATALOGUE
        .iter()
        .find(|e| e.template_id == "spirit_grass")
        .expect("spirit_grass 应在 TRADE_CATALOGUE 中");
    assert_eq!(
        cat.price_bone_coins, 10,
        "TRADE_CATALOGUE spirit_grass 价格应为 10 骨币（与买路一致），\
         实际为 {}",
        cat.price_bone_coins
    );
}

/// TRADE_CATALOGUE 中 broken_artifact_scroll 条目的价格必须与买路对齐（均为 40 骨币）。
#[test]
fn catalogue_broken_artifact_scroll_price_matches_buy_path() {
    let cat = TRADE_CATALOGUE
        .iter()
        .find(|e| e.template_id == "broken_artifact_scroll")
        .expect("broken_artifact_scroll 应在 TRADE_CATALOGUE 中");
    assert_eq!(
        cat.price_bone_coins, 40,
        "TRADE_CATALOGUE broken_artifact_scroll 价格应为 40 骨币（与买路一致），\
         实际为 {}",
        cat.price_bone_coins
    );
}

/// 旧废弃 template_id（lingcao/fragment_scroll/bone_meal/rough_bandage 等）
/// 不应再出现在 TRADE_CATALOGUE——它们无 asset 支撑或与买路不对齐。
#[test]
fn catalogue_no_legacy_misaligned_ids() {
    let legacy_ids = [
        "lingcao",
        "fragment_scroll",
        "bone_meal",
        "rough_bandage",
        "qi_condensing_powder",
        "spirit_stone_shard",
        "bone_reinforcing_pill",
        "spirit_jade",
    ];
    for legacy in &legacy_ids {
        assert!(
            !TRADE_CATALOGUE.iter().any(|e| e.template_id == *legacy),
            "废弃/不对齐 template_id '{}' 不应出现在 TRADE_CATALOGUE，\
             期望: 已移除，实际: 仍存在",
            legacy
        );
    }
}

/// 所有在 TRADE_CATALOGUE 中的条目，通过 assign_npc_trade_inventory 生成的
/// TradeOffer.price_bone_coins 必须与 TRADE_CATALOGUE 条目一致。
#[test]
fn catalogue_prices_propagate_to_trade_offers() {
    for seed in 0..100u64 {
        let inv = assign_npc_trade_inventory(NpcArchetype::Rogue, Realm::Condense, seed);
        for offer in &inv.offers {
            let entry = TRADE_CATALOGUE
                .iter()
                .find(|e| e.template_id == offer.template_id)
                .expect("offer 中的 template_id 必须在 TRADE_CATALOGUE 中有对应条目");
            assert_eq!(
                offer.price_bone_coins, entry.price_bone_coins,
                "TradeOffer 价格应与 TRADE_CATALOGUE 一致：template_id={}, \
                 期望: {}, 实际: {}",
                offer.template_id, entry.price_bone_coins, offer.price_bone_coins
            );
        }
    }
}
