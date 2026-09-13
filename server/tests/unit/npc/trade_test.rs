#![allow(dead_code, unused_imports)]
use bong_server::cultivation::components::Realm;
use bong_server::npc::lifecycle::NpcArchetype;
use bong_server::npc::trade::*;
use valence::prelude::Entity;

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
fn reputation_new_npc_default_mid() {
    let rep = NpcPlayerReputation::default();
    assert!(
        (rep.get("unknown") - 0.5).abs() < f32::EPSILON,
        "unknown player should get default 0.5, got {}",
        rep.get("unknown")
    );
    assert_eq!(
        rep.tier("unknown"),
        RepTier::Mid,
        "default 0.5 should map to Mid tier"
    );
}

#[test]
fn reputation_trade_increases() {
    let mut rep = NpcPlayerReputation::default();
    rep.adjust("player:a", 0.05);
    let expected = 0.55;
    assert!(
        (rep.get("player:a") - expected).abs() < f32::EPSILON,
        "after +0.05 trade adjustment from 0.5, expected {}, got {}",
        expected,
        rep.get("player:a")
    );
}

#[test]
fn reputation_attack_decreases() {
    let mut rep = NpcPlayerReputation::default();
    rep.adjust("player:b", -0.3);
    let expected = 0.2;
    assert!(
        (rep.get("player:b") - expected).abs() < f32::EPSILON,
        "after -0.3 attack adjustment from 0.5, expected {}, got {}",
        expected,
        rep.get("player:b")
    );
}

#[test]
fn reputation_clamp_bounds() {
    let mut rep = NpcPlayerReputation::default();

    // Clamp upper bound
    rep.adjust("player:up", 1.0);
    assert!(
        (rep.get("player:up") - 1.0).abs() < f32::EPSILON,
        "reputation should clamp at 1.0, got {}",
        rep.get("player:up")
    );

    // Further increase should stay at 1.0
    rep.adjust("player:up", 0.5);
    assert!(
        (rep.get("player:up") - 1.0).abs() < f32::EPSILON,
        "reputation should stay at 1.0 after further increase, got {}",
        rep.get("player:up")
    );

    // Clamp lower bound
    rep.adjust("player:down", -1.0);
    assert!(
        (rep.get("player:down") - 0.0).abs() < f32::EPSILON,
        "reputation should clamp at 0.0, got {}",
        rep.get("player:down")
    );

    // Further decrease should stay at 0.0
    rep.adjust("player:down", -0.5);
    assert!(
        (rep.get("player:down") - 0.0).abs() < f32::EPSILON,
        "reputation should stay at 0.0 after further decrease, got {}",
        rep.get("player:down")
    );
}

#[test]
fn reputation_tier_boundaries() {
    // 0.71 → High
    assert_eq!(
        RepTier::from_score(0.71),
        RepTier::High,
        "0.71 should be High tier (boundary: > 0.7)"
    );
    // 0.70 → Mid (boundary exact)
    assert_eq!(
        RepTier::from_score(0.70),
        RepTier::Mid,
        "0.70 should be Mid tier (not > 0.7)"
    );
    // 0.31 → Mid
    assert_eq!(
        RepTier::from_score(0.31),
        RepTier::Mid,
        "0.31 should be Mid tier (boundary: > 0.3)"
    );
    // 0.30 → Low (boundary exact)
    assert_eq!(
        RepTier::from_score(0.30),
        RepTier::Low,
        "0.30 should be Low tier (not > 0.3)"
    );
    // 0.11 → Low
    assert_eq!(
        RepTier::from_score(0.11),
        RepTier::Low,
        "0.11 should be Low tier (boundary: > 0.1)"
    );
    // 0.10 → Hostile (boundary exact)
    assert_eq!(
        RepTier::from_score(0.10),
        RepTier::Hostile,
        "0.10 should be Hostile tier (not > 0.1)"
    );
    // 0.09 → Hostile
    assert_eq!(
        RepTier::from_score(0.09),
        RepTier::Hostile,
        "0.09 should be Hostile tier"
    );
    // 0.0 → Hostile
    assert_eq!(
        RepTier::from_score(0.0),
        RepTier::Hostile,
        "0.0 should be Hostile tier"
    );
    // 1.0 → High
    assert_eq!(
        RepTier::from_score(1.0),
        RepTier::High,
        "1.0 should be High tier"
    );
}

#[test]
fn reputation_multiple_players_independent() {
    let mut rep = NpcPlayerReputation::default();
    rep.adjust("player:alice", 0.2);
    rep.adjust("player:bob", -0.3);

    assert!(
        (rep.get("player:alice") - 0.7).abs() < f32::EPSILON,
        "alice should be 0.7, got {}",
        rep.get("player:alice")
    );
    assert!(
        (rep.get("player:bob") - 0.2).abs() < f32::EPSILON,
        "bob should be 0.2, got {}",
        rep.get("player:bob")
    );
    // Third player unaffected
    assert!(
        (rep.get("player:charlie") - 0.5).abs() < f32::EPSILON,
        "charlie should remain 0.5, got {}",
        rep.get("player:charlie")
    );
}

#[test]
fn reputation_cumulative_adjustments() {
    let mut rep = NpcPlayerReputation::default();
    // 0.5 + 0.05 + 0.05 + 0.1 = 0.7
    rep.adjust("player:a", 0.05);
    rep.adjust("player:a", 0.05);
    rep.adjust("player:a", 0.1);
    assert!(
        (rep.get("player:a") - 0.7).abs() < 1e-6,
        "cumulative adjustments should sum correctly, expected 0.7, got {}",
        rep.get("player:a")
    );
}

// =========================================================================
// P3 — DynamicPricing
// =========================================================================

#[test]
fn pricing_zone_qi_low_increases_price() {
    let config = TradePricingConfig::default();
    let price = DynamicPricing::compute_price(100, 0.2, 0.5, 0.5, 0.0, &config);
    // zone_mod=1.5, rep_mod=1.0, scarcity=1.0, urgency=1.0 → 100*1.5=150
    assert_eq!(
        price, 150,
        "low zone_qi (0.2) should multiply price by 1.5: expected 150, got {}",
        price
    );
}

#[test]
fn pricing_zone_qi_high_decreases_price() {
    let config = TradePricingConfig::default();
    let price = DynamicPricing::compute_price(100, 0.8, 0.5, 0.5, 0.0, &config);
    // zone_mod=0.8, rep_mod=1.0, scarcity=1.0, urgency=1.0 → 100*0.8=80
    assert_eq!(
        price, 80,
        "high zone_qi (0.8) should multiply price by 0.8: expected 80, got {}",
        price
    );
}

#[test]
fn pricing_zone_qi_mid_no_effect() {
    let config = TradePricingConfig::default();
    let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.5, 0.0, &config);
    assert_eq!(
        price, 100,
        "mid zone_qi (0.5) should have no effect: expected 100, got {}",
        price
    );
}

#[test]
fn pricing_high_reputation_discount() {
    let config = TradePricingConfig::default();
    let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.9, 0.0, &config);
    // zone_mod=1.0, rep_mod=0.85, scarcity=1.0, urgency=1.0 → 100*0.85=85
    assert_eq!(
        price, 85,
        "high reputation (0.9) should give 15% discount: expected 85, got {}",
        price
    );
}

#[test]
fn pricing_low_reputation_markup() {
    let config = TradePricingConfig::default();
    let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.2, 0.0, &config);
    // zone_mod=1.0, rep_mod=1.3, scarcity=1.0, urgency=1.0 → 100*1.3=130
    assert_eq!(
        price, 130,
        "low reputation (0.2) should markup 30%: expected 130, got {}",
        price
    );
}

#[test]
fn pricing_npc_urgent_sells_cheap() {
    let config = TradePricingConfig::default();
    let price = DynamicPricing::compute_price(100, 0.5, 0.1, 0.5, 0.0, &config);
    // zone_mod=1.0, rep_mod=1.0, scarcity=1.0, urgency=0.7 → 100*0.7=70
    assert_eq!(
        price, 70,
        "urgent NPC (qi_need=0.1) should sell at 70%: expected 70, got {}",
        price
    );
}

#[test]
fn pricing_npc_not_urgent_no_discount() {
    let config = TradePricingConfig::default();
    let price = DynamicPricing::compute_price(100, 0.5, 0.3, 0.5, 0.0, &config);
    // urgency_mod = 1.0 because npc_qi >= 0.2
    assert_eq!(
        price, 100,
        "non-urgent NPC (qi_need=0.3) should have no urgency discount: expected 100, got {}",
        price
    );
}

#[test]
fn pricing_price_floor_enforced() {
    let config = TradePricingConfig::default();
    // High zone_qi + High rep + urgent → 0.8 * 0.85 * 0.7 = 0.476 → 47.6
    // But floor = 100 * 0.3 = 30, so result should be max(30, 47.6) = 48 (rounded)
    let price = DynamicPricing::compute_price(100, 0.8, 0.1, 0.9, 0.0, &config);
    let floor = (100.0 * config.floor_ratio).round() as u32;
    assert!(
        price >= floor,
        "price {} should be >= floor {}: base=100, zone_qi=0.8, npc_qi=0.1, rep=0.9",
        price,
        floor
    );
}

#[test]
fn pricing_price_floor_boundary() {
    let config = TradePricingConfig::default();
    // base=10, floor = 10 * 0.3 = 3
    // Even with maximum discounts, price should be >= 3
    let price = DynamicPricing::compute_price(10, 0.8, 0.1, 0.9, 0.0, &config);
    assert!(
        price >= 3,
        "price for base=10 should be >= floor 3, got {}",
        price
    );
}

#[test]
fn pricing_minimum_one() {
    let config = TradePricingConfig {
        floor_ratio: 0.0, // disable floor to test min=1 boundary
        ..TradePricingConfig::default()
    };
    let price = DynamicPricing::compute_price(1, 0.8, 0.1, 0.9, 0.0, &config);
    assert!(
        price >= 1,
        "minimum price should always be 1, got {}",
        price
    );
}

#[test]
fn pricing_scarce_item_no_discount() {
    let config = TradePricingConfig::default();
    // scarcity=1.0 → scarcity_mod = max(1.0, 1.0 + 1.0 * 0.5) = 1.5
    let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.5, 1.0, &config);
    assert!(
        price >= 100,
        "scarce item (scarcity=1.0) should only markup, never below base: expected >=100, got {}",
        price
    );
    assert_eq!(
        price, 150,
        "scarcity=1.0 should add 50% markup: expected 150, got {}",
        price
    );
}

#[test]
fn pricing_scarcity_moderate() {
    let config = TradePricingConfig::default();
    // scarcity=0.5 → scarcity_mod = max(1.0, 1.0 + 0.5 * 0.5) = 1.25
    let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.5, 0.5, &config);
    assert_eq!(
        price, 125,
        "scarcity=0.5 should add 25% markup: expected 125, got {}",
        price
    );
}

#[test]
fn pricing_zero_scarcity_no_markup() {
    let config = TradePricingConfig::default();
    let price = DynamicPricing::compute_price(100, 0.5, 0.5, 0.5, 0.0, &config);
    assert_eq!(
        price, 100,
        "zero scarcity should have no markup: expected 100, got {}",
        price
    );
}

#[test]
fn pricing_default_config_values() {
    let config = TradePricingConfig::default();
    assert!(
        (config.zone_qi_low_threshold - 0.3).abs() < f32::EPSILON,
        "default zone_qi_low_threshold should be 0.3"
    );
    assert!(
        (config.zone_qi_high_threshold - 0.6).abs() < f32::EPSILON,
        "default zone_qi_high_threshold should be 0.6"
    );
    assert!(
        (config.zone_qi_low_multiplier - 1.5).abs() < f32::EPSILON,
        "default zone_qi_low_multiplier should be 1.5"
    );
    assert!(
        (config.zone_qi_high_multiplier - 0.8).abs() < f32::EPSILON,
        "default zone_qi_high_multiplier should be 0.8"
    );
    assert!(
        (config.rep_high_discount - 0.85).abs() < f32::EPSILON,
        "default rep_high_discount should be 0.85"
    );
    assert!(
        (config.rep_low_markup - 1.3).abs() < f32::EPSILON,
        "default rep_low_markup should be 1.3"
    );
    assert!(
        (config.floor_ratio - 0.3).abs() < f32::EPSILON,
        "default floor_ratio should be 0.3"
    );
}

#[test]
fn pricing_combined_modifiers() {
    let config = TradePricingConfig::default();
    // Low zone qi + low rep + scarce → 1.5 * 1.3 * 1.5 * 1.0 = 2.925 → 293
    let price = DynamicPricing::compute_price(100, 0.2, 0.5, 0.2, 1.0, &config);
    let expected = (100.0_f32 * 1.5 * 1.3 * 1.5 * 1.0).round() as u32;
    assert_eq!(
        price, expected,
        "combined low_qi + low_rep + scarce: expected {}, got {}",
        expected, price
    );
}

// =========================================================================
// P3 — TradeEligibility
// =========================================================================

#[test]
fn eligibility_high_tier_discount() {
    let e = check_trade_eligibility(RepTier::High);
    assert_eq!(
        e,
        TradeEligibility::Allowed {
            price_modifier: 0.85
        },
        "High tier should allow trade with 0.85 discount"
    );
}

#[test]
fn eligibility_mid_tier_normal() {
    let e = check_trade_eligibility(RepTier::Mid);
    assert_eq!(
        e,
        TradeEligibility::Allowed {
            price_modifier: 1.0
        },
        "Mid tier should allow trade with 1.0 modifier"
    );
}

#[test]
fn eligibility_low_tier_refuse_rare() {
    let e = check_trade_eligibility(RepTier::Low);
    assert_eq!(
        e,
        TradeEligibility::RefuseRare,
        "Low tier should refuse rare items"
    );
}

#[test]
fn eligibility_hostile_refused() {
    let e = check_trade_eligibility(RepTier::Hostile);
    assert_eq!(
        e,
        TradeEligibility::Refused,
        "Hostile tier should refuse all trade"
    );
}

#[test]
fn eligibility_extreme_low_refuses_all() {
    let mut rep = NpcPlayerReputation::default();
    rep.adjust("player:hostile", -0.5); // 0.5 - 0.5 = 0.0 → Hostile
    let tier = rep.tier("player:hostile");
    assert_eq!(tier, RepTier::Hostile, "score 0.0 should be Hostile");
    let e = check_trade_eligibility(tier);
    assert_eq!(
        e,
        TradeEligibility::Refused,
        "Hostile tier from extreme low reputation should refuse all trade"
    );
}

// =========================================================================
// P3 — InformationOffer
// =========================================================================

#[test]
fn info_accuracy_correlates_reputation_high() {
    // High tier: accuracy in [0.8, 1.0]
    for seed in 0..50u64 {
        let offers = generate_info_offers("test_zone", RepTier::High, seed);
        for offer in &offers {
            assert!(
                offer.accuracy >= 0.8 && offer.accuracy <= 1.0,
                "High tier accuracy should be in [0.8, 1.0], got {} (seed={})",
                offer.accuracy,
                seed
            );
        }
    }
}

#[test]
fn info_accuracy_correlates_reputation_mid() {
    // Mid tier: accuracy in [0.5, 0.8]
    for seed in 0..50u64 {
        let offers = generate_info_offers("test_zone", RepTier::Mid, seed);
        for offer in &offers {
            assert!(
                offer.accuracy >= 0.5 && offer.accuracy <= 0.8,
                "Mid tier accuracy should be in [0.5, 0.8], got {} (seed={})",
                offer.accuracy,
                seed
            );
        }
    }
}

#[test]
fn info_accuracy_correlates_reputation_low() {
    // Low tier: accuracy in [0.3, 0.6]
    for seed in 0..50u64 {
        let offers = generate_info_offers("test_zone", RepTier::Low, seed);
        for offer in &offers {
            assert!(
                offer.accuracy >= 0.3 && offer.accuracy <= 0.6,
                "Low tier accuracy should be in [0.3, 0.6], got {} (seed={})",
                offer.accuracy,
                seed
            );
        }
    }
}

#[test]
fn info_kind_all_variants() {
    // Verify all 4 InfoKind variants are constructible
    let zone_qi = InfoKind::ZoneQiLevel {
        zone_name: "test".to_string(),
        qi_value: 0.5,
    };
    let danger = InfoKind::DangerWarning {
        zone_name: "test".to_string(),
        threat_desc: "danger".to_string(),
    };
    let resource = InfoKind::ResourceLocation {
        zone_name: "test".to_string(),
        resource: "ore".to_string(),
    };
    let sighting = InfoKind::NpcSighting {
        target_desc: "rogue".to_string(),
        last_zone: "test".to_string(),
    };

    // Verify they are distinct
    assert_ne!(
        zone_qi, danger,
        "ZoneQiLevel and DangerWarning should differ"
    );
    assert_ne!(
        resource, sighting,
        "ResourceLocation and NpcSighting should differ"
    );
    assert_ne!(
        zone_qi, resource,
        "ZoneQiLevel and ResourceLocation should differ"
    );
}

#[test]
fn info_generate_produces_bounded() {
    let mut any_zero = false;
    let mut any_nonzero = false;
    for seed in 0..200u64 {
        let offers = generate_info_offers("test_zone", RepTier::Mid, seed);
        assert!(
            offers.len() <= 2,
            "generate_info_offers should produce 0-2 items, got {} (seed={})",
            offers.len(),
            seed
        );
        if offers.is_empty() {
            any_zero = true;
        } else {
            any_nonzero = true;
        }
        for offer in &offers {
            assert!(
                offer.price_bone_coins >= 5 && offer.price_bone_coins <= 20,
                "price should be in [5, 20], got {} (seed={})",
                offer.price_bone_coins,
                seed
            );
            assert!(
                offer.expiry_ticks == 24_000,
                "expiry should be 24000, got {} (seed={})",
                offer.expiry_ticks,
                seed
            );
        }
    }
    assert!(
        any_zero,
        "over 200 seeds, should see at least one empty result"
    );
    assert!(
        any_nonzero,
        "over 200 seeds, should see at least one non-empty result"
    );
}

#[test]
fn info_generate_deterministic() {
    let a = generate_info_offers("zone_a", RepTier::High, 12345);
    let b = generate_info_offers("zone_a", RepTier::High, 12345);
    assert_eq!(a.len(), b.len(), "same seed should produce same count");
    for (oa, ob) in a.iter().zip(b.iter()) {
        assert_eq!(
            oa.info_kind, ob.info_kind,
            "same seed should produce same info_kind"
        );
        assert_eq!(
            oa.price_bone_coins, ob.price_bone_coins,
            "same seed should produce same price"
        );
        assert!(
            (oa.accuracy - ob.accuracy).abs() < f32::EPSILON,
            "same seed should produce same accuracy"
        );
    }
}

// =========================================================================
// P3 — TradeAbortedByNpc
// =========================================================================

#[test]
fn ambush_abort_event_fields() {
    let npc = Entity::from_raw(42);
    let player = Entity::from_raw(99);
    let event = TradeAbortedByNpc {
        npc,
        player,
        reason: TradeAbortReason::Ambush,
    };
    assert_eq!(
        event.reason,
        TradeAbortReason::Ambush,
        "TradeAbortedByNpc should carry Ambush reason"
    );
    assert_eq!(
        event.npc,
        Entity::from_raw(42),
        "npc entity should be preserved"
    );
    assert_eq!(
        event.player,
        Entity::from_raw(99),
        "player entity should be preserved"
    );
}

#[test]
fn hostile_reputation_abort_event_fields() {
    let event = TradeAbortedByNpc {
        npc: Entity::from_raw(1),
        player: Entity::from_raw(2),
        reason: TradeAbortReason::HostileReputation,
    };
    assert_eq!(
        event.reason,
        TradeAbortReason::HostileReputation,
        "TradeAbortedByNpc should carry HostileReputation reason"
    );
}

// =========================================================================
// P3 — ReputationGossipEvent
// =========================================================================

#[test]
fn gossip_event_construction() {
    let event = ReputationGossipEvent {
        source_npc: Entity::from_raw(10),
        target_player_id: "offline:Azure".to_string(),
        delta: -0.05,
        hops_remaining: 3,
    };
    assert_eq!(event.hops_remaining, 3, "initial hops should be 3");
    assert!(
        (event.delta - (-0.05)).abs() < f32::EPSILON,
        "delta should be -0.05"
    );
}

#[test]
fn pending_gossip_countdown() {
    let mut pending = PendingGossip::default();
    pending.entries.push(PendingGossipEntry {
        target_player_id: "player:test".to_string(),
        delta: -0.05,
        hops_remaining: 2,
        source_npc: Entity::from_raw(1),
        remaining_ticks: 3,
    });

    // Tick down manually
    pending.entries[0].remaining_ticks -= 1;
    assert_eq!(
        pending.entries[0].remaining_ticks, 2,
        "after 1 tick, remaining should be 2"
    );

    pending.entries[0].remaining_ticks -= 1;
    assert_eq!(
        pending.entries[0].remaining_ticks, 1,
        "after 2 ticks, remaining should be 1"
    );

    pending.entries[0].remaining_ticks -= 1;
    assert_eq!(
        pending.entries[0].remaining_ticks, 0,
        "after 3 ticks, remaining should be 0 (ready to emit)"
    );
}

#[test]
fn gossip_decay_formula() {
    // Verify the decay formula: delta * 0.5^(GOSSIP_INITIAL_HOPS - hops_remaining)
    let base_delta: f32 = -0.1;

    // Hop 3 (first emission): 0.5^(3-3) = 0.5^0 = 1.0
    let eff_0 = base_delta * 0.5_f32.powf((GOSSIP_INITIAL_HOPS - 3) as f32);
    assert!(
        (eff_0 - base_delta).abs() < f32::EPSILON,
        "at hop 3, effective delta should equal base delta: expected {}, got {}",
        base_delta,
        eff_0
    );

    // Hop 2: 0.5^(3-2) = 0.5^1 = 0.5
    let eff_1 = base_delta * 0.5_f32.powf((GOSSIP_INITIAL_HOPS - 2) as f32);
    assert!(
        (eff_1 - base_delta * 0.5).abs() < f32::EPSILON,
        "at hop 2, effective delta should be half: expected {}, got {}",
        base_delta * 0.5,
        eff_1
    );

    // Hop 1: 0.5^(3-1) = 0.5^2 = 0.25
    let eff_2 = base_delta * 0.5_f32.powf((GOSSIP_INITIAL_HOPS - 1) as f32);
    assert!(
        (eff_2 - base_delta * 0.25).abs() < f32::EPSILON,
        "at hop 1, effective delta should be quarter: expected {}, got {}",
        base_delta * 0.25,
        eff_2
    );

    // Hop 0: 0.5^(3-0) = 0.5^3 = 0.125
    let eff_3 = base_delta * 0.5_f32.powf(GOSSIP_INITIAL_HOPS as f32);
    assert!(
        (eff_3 - base_delta * 0.125).abs() < f32::EPSILON,
        "at hop 0, effective delta should be 1/8: expected {}, got {}",
        base_delta * 0.125,
        eff_3
    );
}

#[test]
fn gossip_constants() {
    assert_eq!(
        GOSSIP_PROPAGATION_RADIUS, 48.0,
        "gossip propagation radius should be 48 blocks"
    );
    assert_eq!(GOSSIP_DELAY_TICKS, 120, "gossip delay should be 120 ticks");
    assert_eq!(GOSSIP_INITIAL_HOPS, 3, "gossip initial hops should be 3");
}

// =========================================================================
// P3 — RepTier enum completeness
// =========================================================================

#[test]
fn rep_tier_all_variants_reachable() {
    // Ensure all 4 variants are reachable from from_score
    let variants = [
        (1.0, RepTier::High),
        (0.5, RepTier::Mid),
        (0.2, RepTier::Low),
        (0.0, RepTier::Hostile),
    ];
    for (score, expected) in variants {
        assert_eq!(
            RepTier::from_score(score),
            expected,
            "score {} should map to {:?}",
            score,
            expected
        );
    }
}

// =========================================================================
// P3 — NpcPlayerReputation + DynamicPricing integration
// =========================================================================

#[test]
fn reputation_to_pricing_integration() {
    let mut rep = NpcPlayerReputation::default();
    let config = TradePricingConfig::default();

    // Default (Mid) → normal price
    let price_mid = DynamicPricing::compute_price(100, 0.5, 0.5, rep.get("player:a"), 0.0, &config);
    assert_eq!(price_mid, 100, "mid reputation should give normal price");

    // High reputation → discount
    rep.adjust("player:a", 0.25); // 0.75 → High
    let price_high =
        DynamicPricing::compute_price(100, 0.5, 0.5, rep.get("player:a"), 0.0, &config);
    assert_eq!(price_high, 85, "high reputation should give 15% discount");

    // Low reputation → markup
    rep.adjust("player:b", -0.25); // 0.25 → Low
    let price_low = DynamicPricing::compute_price(100, 0.5, 0.5, rep.get("player:b"), 0.0, &config);
    assert_eq!(price_low, 130, "low reputation should give 30% markup");
}
