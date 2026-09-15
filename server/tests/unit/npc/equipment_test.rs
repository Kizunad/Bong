use bong_server::combat::components::WoundKind;
use bong_server::combat::events::{FIST_REACH, SPEAR_REACH, SWORD_REACH};
use bong_server::combat::weapon::WeaponKind;
use bong_server::cultivation::components::Realm;
use bong_server::npc::equipment::{
    armor_drop_durability, armor_item_kind_for_tier, assign_npc_equipment, merge_equipment,
    npc_weapon_damage_multiplier, reach_for_weapon_kind, roll_equipment_drops,
    weapon_drop_durability, weapon_item_kind_for_tier, wound_kind_for_weapon, NpcEquipSlot,
    NpcEquipment,
};
use bong_server::npc::faction::FactionId;
use bong_server::npc::lifecycle::NpcArchetype;
use bong_server::skin::faction_tint::visual_equipment;
use valence::prelude::ItemKind;

fn make_weapon_slot(
    template_id: &str,
    display_name: &str,
    item_kind: ItemKind,
    weapon_kind: WeaponKind,
    quality_tier: u8,
    base_attack: f32,
    durability_ratio: f32,
) -> NpcEquipSlot {
    NpcEquipSlot {
        template_id: template_id.to_string(),
        display_name: display_name.to_string(),
        item_kind,
        quality_tier,
        base_attack,
        weapon_kind: Some(weapon_kind),
        armor_profile_id: None,
        durability_ratio,
    }
}

fn make_armor_slot(
    template_id: &str,
    display_name: &str,
    item_kind: ItemKind,
    armor_profile_id: &str,
    quality_tier: u8,
    durability_ratio: f32,
) -> NpcEquipSlot {
    NpcEquipSlot {
        template_id: template_id.to_string(),
        display_name: display_name.to_string(),
        item_kind,
        quality_tier,
        base_attack: 0.0,
        weapon_kind: None,
        armor_profile_id: Some(armor_profile_id.to_string()),
        durability_ratio,
    }
}

// === P0.1 NpcEquipment + NpcEquipSlot basic ===

#[test]
fn npc_equipment_default_all_none() {
    let eq = NpcEquipment::default();
    assert!(eq.main_hand.is_none());
    assert!(eq.off_hand.is_none());
    assert!(eq.head.is_none());
    assert!(eq.chest.is_none());
    assert!(eq.legs.is_none());
    assert!(eq.feet.is_none());
}

#[test]
fn npc_equipment_iter_slots_returns_non_empty() {
    let empty = NpcEquipment::default();
    assert_eq!(
        empty.iter_slots().count(),
        0,
        "empty eq should yield 0 slots"
    );
    let eq = NpcEquipment {
        main_hand: Some(make_weapon_slot(
            "test_sword",
            "测试剑",
            ItemKind::IronSword,
            WeaponKind::Sword,
            0,
            8.0,
            1.0,
        )),
        chest: Some(make_armor_slot(
            "test_armor",
            "测试甲",
            ItemKind::LeatherChestplate,
            "test_profile",
            0,
            1.0,
        )),
        ..Default::default()
    };
    let names: Vec<&str> = eq.iter_slots().map(|(name, _)| name).collect();
    assert_eq!(names, vec!["main_hand", "chest"]);
}

#[test]
fn npc_equipment_armor_slots_only_returns_armor() {
    let eq = NpcEquipment {
        main_hand: Some(make_weapon_slot(
            "sword",
            "剑",
            ItemKind::IronSword,
            WeaponKind::Sword,
            0,
            8.0,
            1.0,
        )),
        head: Some(make_armor_slot(
            "helmet",
            "盔",
            ItemKind::LeatherHelmet,
            "h",
            0,
            1.0,
        )),
        feet: Some(make_armor_slot(
            "boots",
            "靴",
            ItemKind::LeatherBoots,
            "b",
            0,
            1.0,
        )),
        ..Default::default()
    };
    let armor_count = eq.armor_slots().count();
    assert_eq!(
        armor_count, 2,
        "armor_slots should only return head+feet, not main_hand"
    );
}

// === P0.1 npc_weapon_damage_multiplier ===

#[test]
fn damage_multiplier_tier0_full_durability() {
    let slot = make_weapon_slot(
        "s",
        "s",
        ItemKind::IronSword,
        WeaponKind::Sword,
        0,
        8.0,
        1.0,
    );
    let mul = npc_weapon_damage_multiplier(&slot);
    // attack_mul = max(1.0, 8/10) = 1.0; quality = 1.0; durability = 0.5 + 0.5*1 = 1.0
    let expected = 1.0 * 1.0 * 1.0;
    assert!(
        (mul - expected).abs() < 1e-5,
        "tier0 full dur expected {expected}, got {mul}"
    );
}

#[test]
fn damage_multiplier_tier1_half_durability() {
    let slot = make_weapon_slot(
        "s",
        "s",
        ItemKind::DiamondSword,
        WeaponKind::Sword,
        1,
        14.0,
        0.5,
    );
    let mul = npc_weapon_damage_multiplier(&slot);
    // attack_mul = max(1.0, 14/10) = 1.4; quality = 1.15; durability = 0.5 + 0.5*0.5 = 0.75
    let expected = 1.4 * 1.15 * 0.75;
    assert!(
        (mul - expected).abs() < 1e-4,
        "tier1 half dur expected {expected}, got {mul}"
    );
}

#[test]
fn damage_multiplier_tier2_zero_durability() {
    let slot = make_weapon_slot(
        "s",
        "s",
        ItemKind::GoldenSword,
        WeaponKind::Sword,
        2,
        20.0,
        0.0,
    );
    let mul = npc_weapon_damage_multiplier(&slot);
    // attack_mul = 2.0; quality = 1.35; durability = 0.5 + 0 = 0.5
    let expected = 2.0 * 1.35 * 0.5;
    assert!(
        (mul - expected).abs() < 1e-4,
        "tier2 zero dur expected {expected}, got {mul}"
    );
}

#[test]
fn damage_multiplier_low_base_attack_floors_at_1() {
    let slot = make_weapon_slot(
        "s",
        "s",
        ItemKind::IronSword,
        WeaponKind::Sword,
        0,
        3.0,
        1.0,
    );
    let mul = npc_weapon_damage_multiplier(&slot);
    // attack_mul = max(1.0, 3/10) = 1.0
    assert!(
        (mul - 1.0).abs() < 1e-5,
        "low base_attack should floor to 1.0, got {mul}"
    );
}

#[test]
fn damage_multiplier_clamped_durability_above_1() {
    let slot = make_weapon_slot(
        "s",
        "s",
        ItemKind::IronSword,
        WeaponKind::Sword,
        0,
        10.0,
        1.5,
    );
    let mul = npc_weapon_damage_multiplier(&slot);
    // durability should clamp to 1.0
    let expected = 1.0 * 1.0 * 1.0;
    assert!(
        (mul - expected).abs() < 1e-5,
        "clamped durability expected {expected}, got {mul}"
    );
}

#[test]
fn damage_multiplier_clamped_durability_below_0() {
    let slot = make_weapon_slot(
        "s",
        "s",
        ItemKind::IronSword,
        WeaponKind::Sword,
        0,
        10.0,
        -0.5,
    );
    let mul = npc_weapon_damage_multiplier(&slot);
    let expected = 1.0 * 1.0 * 0.5;
    assert!(
        (mul - expected).abs() < 1e-5,
        "negative durability should clamp to 0.0, got {mul}"
    );
}

// === P0.1 reach + wound mapping ===

#[test]
fn reach_for_all_weapon_kinds() {
    assert_eq!(reach_for_weapon_kind(WeaponKind::Sword), SWORD_REACH);
    assert_eq!(reach_for_weapon_kind(WeaponKind::Saber), SWORD_REACH);
    assert_eq!(reach_for_weapon_kind(WeaponKind::Spear), SPEAR_REACH);
    assert_eq!(reach_for_weapon_kind(WeaponKind::Staff), SPEAR_REACH);
    assert_eq!(reach_for_weapon_kind(WeaponKind::Fist), FIST_REACH);
    // Dagger has custom reach
    let dagger_reach = reach_for_weapon_kind(WeaponKind::Dagger);
    assert!((dagger_reach.base - 1.2).abs() < 1e-5);
}

#[test]
fn wound_kind_for_all_weapon_kinds() {
    assert_eq!(wound_kind_for_weapon(WeaponKind::Sword), WoundKind::Cut);
    assert_eq!(wound_kind_for_weapon(WeaponKind::Saber), WoundKind::Cut);
    assert_eq!(wound_kind_for_weapon(WeaponKind::Dagger), WoundKind::Cut);
    assert_eq!(wound_kind_for_weapon(WeaponKind::Staff), WoundKind::Blunt);
    assert_eq!(wound_kind_for_weapon(WeaponKind::Fist), WoundKind::Blunt);
    assert_eq!(wound_kind_for_weapon(WeaponKind::Spear), WoundKind::Pierce);
    assert_eq!(wound_kind_for_weapon(WeaponKind::Bow), WoundKind::Pierce);
}

// === P0.1 ItemKind mapping ===

#[test]
fn weapon_item_kind_tiers() {
    assert_eq!(weapon_item_kind_for_tier(0), ItemKind::IronSword);
    assert_eq!(weapon_item_kind_for_tier(1), ItemKind::DiamondSword);
    assert_eq!(weapon_item_kind_for_tier(2), ItemKind::GoldenSword);
    assert_eq!(weapon_item_kind_for_tier(3), ItemKind::GoldenSword);
}

#[test]
fn armor_item_kind_tiers_per_slot() {
    assert_eq!(armor_item_kind_for_tier("head", 0), ItemKind::LeatherHelmet);
    assert_eq!(
        armor_item_kind_for_tier("head", 1),
        ItemKind::ChainmailHelmet
    );
    assert_eq!(armor_item_kind_for_tier("head", 2), ItemKind::IronHelmet);
    assert_eq!(
        armor_item_kind_for_tier("chest", 0),
        ItemKind::LeatherChestplate
    );
    assert_eq!(
        armor_item_kind_for_tier("legs", 1),
        ItemKind::ChainmailLeggings
    );
    assert_eq!(armor_item_kind_for_tier("feet", 2), ItemKind::IronBoots);
}

// === P0.2 assign_npc_equipment — archetype coverage ===

#[test]
fn assign_zombie_returns_empty() {
    let eq = assign_npc_equipment(NpcArchetype::Zombie, Realm::Awaken, None, 42);
    assert_eq!(eq, NpcEquipment::default());
}

#[test]
fn assign_beast_returns_empty() {
    let eq = assign_npc_equipment(NpcArchetype::Beast, Realm::Condense, None, 42);
    assert_eq!(eq, NpcEquipment::default());
}

#[test]
fn assign_fuya_returns_empty() {
    let eq = assign_npc_equipment(NpcArchetype::Fuya, Realm::Spirit, None, 42);
    assert_eq!(eq, NpcEquipment::default());
}

#[test]
fn assign_skull_fiend_returns_empty() {
    let eq = assign_npc_equipment(NpcArchetype::SkullFiend, Realm::Void, None, 42);
    assert_eq!(eq, NpcEquipment::default());
}

#[test]
fn assign_rogue_has_weapon_or_empty_deterministic() {
    let eq1 = assign_npc_equipment(NpcArchetype::Rogue, Realm::Awaken, None, 12345);
    let eq2 = assign_npc_equipment(NpcArchetype::Rogue, Realm::Awaken, None, 12345);
    assert_eq!(eq1, eq2, "same seed should produce identical equipment");
}

#[test]
fn assign_rogue_quality_tier_never_exceeds_2() {
    for seed in 0..200u64 {
        let eq = assign_npc_equipment(NpcArchetype::Rogue, Realm::Void, None, seed * 7919);
        for (_name, slot) in eq.iter_slots() {
            assert!(
                slot.quality_tier <= 2,
                "seed {} slot quality_tier {} exceeds max 2 (法宝)",
                seed,
                slot.quality_tier
            );
        }
    }
}

#[test]
fn assign_rogue_condense_upgrades_tier() {
    // Scan seeds until we find iron_sword and wooden_staff to assert tier upgrade on both
    let mut found_sword = false;
    let mut found_staff = false;
    for seed in 0..500u64 {
        let eq = assign_npc_equipment(NpcArchetype::Rogue, Realm::Condense, None, seed);
        if let Some(main) = &eq.main_hand {
            if main.template_id == "iron_sword" && !found_sword {
                assert!(
                    main.quality_tier >= 1,
                    "Condense rogue iron_sword should have tier >= 1, got {} (seed {})",
                    main.quality_tier,
                    seed
                );
                found_sword = true;
            }
            if main.template_id == "wooden_staff" && !found_staff {
                assert!(
                    main.quality_tier >= 1,
                    "Condense rogue wooden_staff should have tier >= 1, got {} (seed {})",
                    main.quality_tier,
                    seed
                );
                found_staff = true;
            }
        }
        if found_sword && found_staff {
            break;
        }
    }
    assert!(
        found_sword,
        "failed to find iron_sword seed in 500 iterations"
    );
    assert!(
        found_staff,
        "failed to find wooden_staff seed in 500 iterations"
    );
}

#[test]
fn assign_commoner_mostly_empty_handed() {
    let mut empty_count = 0;
    for seed in 0..100u64 {
        let eq = assign_npc_equipment(NpcArchetype::Commoner, Realm::Awaken, None, seed * 31);
        if eq.main_hand.is_none() {
            empty_count += 1;
        }
    }
    // Should be roughly 80% empty-handed
    assert!(
        empty_count >= 60,
        "commoner should be ~80% empty-handed, got {empty_count}/100"
    );
}

#[test]
fn assign_disciple_always_has_weapon() {
    for seed in 0..50u64 {
        for faction in [
            Some(FactionId::Attack),
            Some(FactionId::Defend),
            Some(FactionId::Neutral),
        ] {
            let eq =
                assign_npc_equipment(NpcArchetype::Disciple, Realm::Awaken, faction, seed * 7);
            assert!(
                eq.main_hand.is_some(),
                "disciple should always have main_hand weapon (seed={seed}, faction={faction:?})"
            );
        }
    }
}

#[test]
fn assign_disciple_defend_has_staff_and_shield() {
    let eq = assign_npc_equipment(
        NpcArchetype::Disciple,
        Realm::Awaken,
        Some(FactionId::Defend),
        42,
    );
    assert!(eq.main_hand.is_some(), "defend disciple should have staff");
    assert!(eq.off_hand.is_some(), "defend disciple should have shield");
    let staff = eq.main_hand.unwrap();
    assert_eq!(
        staff.weapon_kind,
        Some(WeaponKind::Staff),
        "defend disciple main_hand should be staff"
    );
}

#[test]
fn assign_guardian_relic_ancient_sword_tier2() {
    let eq = assign_npc_equipment(NpcArchetype::GuardianRelic, Realm::Spirit, None, 42);
    let main = eq.main_hand.expect("guardian relic must have weapon");
    assert_eq!(main.template_id, "ancient_sword");
    assert_eq!(main.quality_tier, 2);
    assert_eq!(main.item_kind, ItemKind::GoldenSword);
}

#[test]
fn assign_guardian_relic_has_at_least_2_armor() {
    let eq = assign_npc_equipment(NpcArchetype::GuardianRelic, Realm::Awaken, None, 42);
    let armor_count = eq.armor_slots().count();
    assert!(
        armor_count >= 2,
        "guardian relic should have >= 2 armor pieces, got {armor_count}"
    );
}

#[test]
fn assign_daoxiang_rusty_sword_low_durability() {
    let eq = assign_npc_equipment(NpcArchetype::Daoxiang, Realm::Awaken, None, 42);
    let main = eq.main_hand.expect("daoxiang must have rusty sword");
    assert_eq!(main.template_id, "rusted_sword");
    assert!(
        (main.durability_ratio - 0.3).abs() < 1e-5,
        "daoxiang sword durability should be 0.3"
    );
}

#[test]
fn assign_zhinian_always_has_weapon_half_damaged() {
    for seed in 0..50u64 {
        let eq = assign_npc_equipment(NpcArchetype::Zhinian, Realm::Induce, None, seed * 13);
        let main = eq.main_hand.as_ref().expect("zhinian must have weapon");
        assert!(
            (main.durability_ratio - 0.5).abs() < 1e-5,
            "zhinian weapon should have durability 0.5"
        );
    }
}

// === P0.2 determinism ===

#[test]
fn assign_deterministic_all_archetypes() {
    for archetype in [
        NpcArchetype::Zombie,
        NpcArchetype::Commoner,
        NpcArchetype::Rogue,
        NpcArchetype::Beast,
        NpcArchetype::Disciple,
        NpcArchetype::GuardianRelic,
        NpcArchetype::Daoxiang,
        NpcArchetype::Zhinian,
        NpcArchetype::Fuya,
        NpcArchetype::SkullFiend,
    ] {
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let a = assign_npc_equipment(archetype, realm, None, 42);
            let b = assign_npc_equipment(archetype, realm, None, 42);
            assert_eq!(a, b, "determinism failed for {archetype:?} x {realm:?}");
        }
    }
}

// === P0.3 merge_equipment ===

#[test]
fn merge_equipment_no_npc_eq_uses_visual_profile() {
    use bong_server::skin::npc_skin_selector::select_npc_visual_profile;
    let profile =
        select_npc_visual_profile(NpcArchetype::Disciple, Realm::Awaken, None, None, 0.5);
    let merged = merge_equipment(None, &profile);
    let visual_only = visual_equipment(&profile);
    // Without NpcEquipment, should match visual profile output
    assert_eq!(merged.head().item, visual_only.head().item);
    assert_eq!(merged.chest().item, visual_only.chest().item);
}

#[test]
fn merge_equipment_npc_eq_overrides_visual_profile() {
    use bong_server::skin::npc_skin_selector::select_npc_visual_profile;
    let profile = select_npc_visual_profile(
        NpcArchetype::Disciple,
        Realm::Awaken,
        Some(FactionId::Attack),
        Some(bong_server::npc::faction::FactionRank::Disciple),
        0.5,
    );
    let npc_eq = NpcEquipment {
        chest: Some(make_armor_slot(
            "iron_chest",
            "铁甲",
            ItemKind::IronChestplate,
            "armor_iron_chestplate",
            1,
            0.9,
        )),
        ..Default::default()
    };
    let merged = merge_equipment(Some(&npc_eq), &profile);
    // NpcEquipment chest should override faction tint chest
    assert_eq!(
        merged.chest().item,
        ItemKind::ChainmailChestplate, // tier 1 maps to chainmail
        "NpcEquipment should override visual profile chest slot"
    );
}

#[test]
fn merge_equipment_empty_npc_eq_preserves_visual() {
    use bong_server::skin::npc_skin_selector::select_npc_visual_profile;
    let profile = select_npc_visual_profile(
        NpcArchetype::Disciple,
        Realm::Awaken,
        Some(FactionId::Attack),
        Some(bong_server::npc::faction::FactionRank::Disciple),
        0.5,
    );
    let npc_eq = NpcEquipment::default(); // all None
    let merged = merge_equipment(Some(&npc_eq), &profile);
    let visual_only = visual_equipment(&profile);
    // Empty NpcEquipment should not clear visual profile equipment
    assert_eq!(merged.chest().item, visual_only.chest().item);
}

#[test]
fn merge_equipment_main_hand_weapon() {
    use bong_server::skin::npc_skin_selector::select_npc_visual_profile;
    let profile =
        select_npc_visual_profile(NpcArchetype::Rogue, Realm::Awaken, None, None, 0.5);
    let npc_eq = NpcEquipment {
        main_hand: Some(make_weapon_slot(
            "iron_sword",
            "铁剑",
            ItemKind::IronSword,
            WeaponKind::Sword,
            0,
            8.0,
            1.0,
        )),
        ..Default::default()
    };
    let merged = merge_equipment(Some(&npc_eq), &profile);
    assert_eq!(merged.main_hand().item, ItemKind::IronSword);
}

// === P0.4 战斗结算（纯函数测试） ===

#[test]
fn equipped_npc_has_higher_damage_than_unarmed() {
    let armed_slot = make_weapon_slot(
        "iron_sword",
        "铁剑",
        ItemKind::IronSword,
        WeaponKind::Sword,
        1,
        12.0,
        0.9,
    );
    let armed_mul = npc_weapon_damage_multiplier(&armed_slot);
    let unarmed_mul = 1.0_f32; // 无装备走默认 1.0 乘数
    assert!(
        armed_mul > unarmed_mul,
        "armed NPC damage multiplier ({armed_mul}) should be > unarmed ({unarmed_mul})"
    );
}

#[test]
fn higher_tier_weapon_does_more_damage() {
    let tier0 = make_weapon_slot(
        "s",
        "s",
        ItemKind::IronSword,
        WeaponKind::Sword,
        0,
        10.0,
        1.0,
    );
    let tier1 = make_weapon_slot(
        "s",
        "s",
        ItemKind::DiamondSword,
        WeaponKind::Sword,
        1,
        10.0,
        1.0,
    );
    let tier2 = make_weapon_slot(
        "s",
        "s",
        ItemKind::GoldenSword,
        WeaponKind::Sword,
        2,
        10.0,
        1.0,
    );
    let m0 = npc_weapon_damage_multiplier(&tier0);
    let m1 = npc_weapon_damage_multiplier(&tier1);
    let m2 = npc_weapon_damage_multiplier(&tier2);
    assert!(m1 > m0, "tier1 ({m1}) should > tier0 ({m0})");
    assert!(m2 > m1, "tier2 ({m2}) should > tier1 ({m1})");
}

// === P0.5 death drops ===

#[test]
fn roll_equipment_drops_deterministic() {
    let eq = assign_npc_equipment(NpcArchetype::Rogue, Realm::Awaken, None, 42);
    let drops1 = roll_equipment_drops(&eq, 999);
    let drops2 = roll_equipment_drops(&eq, 999);
    assert_eq!(drops1, drops2, "same seed should produce identical drops");
}

#[test]
fn roll_equipment_drops_empty_equipment_no_drops() {
    let eq = NpcEquipment::default();
    let drops = roll_equipment_drops(&eq, 42);
    assert!(drops.is_empty(), "empty equipment should produce no drops");
}

#[test]
fn roll_equipment_drops_rate_roughly_30_percent() {
    // 用 Guardian Relic（通常 3-5 slot 有装备）大量 roll 统计命中率
    let eq = assign_npc_equipment(NpcArchetype::GuardianRelic, Realm::Spirit, None, 42);
    let slot_count = eq.iter_slots().count();
    assert!(
        slot_count >= 3,
        "guardian relic should have >= 3 slots filled"
    );

    let mut total_possible = 0usize;
    let mut total_dropped = 0usize;
    for seed in 0..1000u64 {
        let drops = roll_equipment_drops(&eq, seed * 7919);
        total_possible += slot_count;
        total_dropped += drops.len();
    }
    let rate = total_dropped as f32 / total_possible as f32;
    assert!(
        (0.25..=0.35).contains(&rate),
        "equipment drop rate should be ~30%, got {rate:.3} ({total_dropped}/{total_possible})"
    );
}

#[test]
fn weapon_drop_durability_formula() {
    let slot = make_weapon_slot(
        "s",
        "s",
        ItemKind::IronSword,
        WeaponKind::Sword,
        0,
        8.0,
        0.8,
    );
    let dur = weapon_drop_durability(&slot);
    assert!(
        (dur - 0.64).abs() < 1e-5,
        "weapon drop dur should be 0.8*0.8=0.64, got {dur}"
    );
}

#[test]
fn armor_drop_durability_formula() {
    let slot = make_armor_slot("a", "a", ItemKind::LeatherChestplate, "p", 0, 0.7);
    let dur = armor_drop_durability(&slot);
    assert!(
        (dur - 0.49).abs() < 1e-5,
        "armor drop dur should be 0.7*0.7=0.49, got {dur}"
    );
}

#[test]
fn weapon_drop_durability_clamped() {
    let slot = make_weapon_slot(
        "s",
        "s",
        ItemKind::IronSword,
        WeaponKind::Sword,
        0,
        8.0,
        1.5,
    );
    let dur = weapon_drop_durability(&slot);
    assert!(dur <= 1.0, "durability should be clamped to 1.0, got {dur}");
}

#[test]
fn armor_drop_durability_zero() {
    let slot = make_armor_slot("a", "a", ItemKind::LeatherChestplate, "p", 0, 0.0);
    let dur = armor_drop_durability(&slot);
    assert!((dur - 0.0).abs() < 1e-5, "zero durability should yield 0.0");
}

// === P0.2 每个 archetype x 每个 realm 至少 1 case ===

#[test]
fn assign_all_archetypes_all_realms_produces_valid_equipment() {
    let all_archetypes = [
        NpcArchetype::Zombie,
        NpcArchetype::Commoner,
        NpcArchetype::Rogue,
        NpcArchetype::Beast,
        NpcArchetype::Disciple,
        NpcArchetype::GuardianRelic,
        NpcArchetype::Daoxiang,
        NpcArchetype::Zhinian,
        NpcArchetype::Fuya,
        NpcArchetype::SkullFiend,
    ];
    let all_realms = [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ];
    for archetype in all_archetypes {
        for realm in all_realms {
            let eq = assign_npc_equipment(archetype, realm, None, 42);
            for (_name, slot) in eq.iter_slots() {
                assert!(
                    slot.quality_tier <= 2,
                    "{archetype:?} x {realm:?}: quality_tier {} exceeds max 2",
                    slot.quality_tier
                );
                assert!(
                    slot.durability_ratio >= 0.0 && slot.durability_ratio <= 1.5,
                    "{archetype:?} x {realm:?}: durability_ratio {} out of range",
                    slot.durability_ratio
                );
            }
        }
    }
}
