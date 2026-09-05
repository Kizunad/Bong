//! `client_request_handler` 的私有契约测试登记处。
//!
//! 只有确实需要父模块私有状态/纯 helper 的测试放在这里；可由公开 C2S ingress
//! 驱动的行为测试位于 `server/tests/unit/network/client_request_handler_test.rs`。

use super::*;

use crate::cultivation::components::{MeridianId, MeridianSystem};
use crate::cultivation::known_techniques::TechniqueRequiredMeridian;
use crate::cultivation::meridian::severed::{MeridianSeveredPermanent, SeveredSource};
use crate::inventory::{
    ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
};
use crate::world::dimension::{DimensionKind, DimensionLayers};
use valence::prelude::{App, BlockPos, DVec3, Entity, EntityLayerId, Update};

#[test]
fn combat_pill_buff_status_payload_preserves_hud_fields() {
    let bytes = build_pill_buff_status_payload("tie_bi_san", 1800, 1.25, 2)
        .expect("valid pill buff status payload should serialize");
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).expect("test build emits JSON server_data");
    assert_eq!(value["type"], "pill_buff_status");
    assert_eq!(value["buff_id"], "tie_bi_san");
    assert_eq!(value["remaining_ticks"], 3600);
    assert_eq!(value["effect_multiplier"], 1.25);
}

#[test]
fn combat_pill_buff_status_rejects_invalid_multiplier() {
    assert!(build_pill_buff_status_payload("tie_bi_san", 1800, f32::NAN, 1).is_none());
    assert!(build_pill_buff_status_payload("tie_bi_san", 1800, 0.0, 1).is_none());
}

#[test]
fn combat_pill_buff_status_rejects_empty_buff_id() {
    assert!(build_pill_buff_status_payload("  ", 1800, 1.25, 1).is_none());
}

#[test]
fn combat_pill_buff_status_duration_zero_uses_base_ticks() {
    let bytes = build_pill_buff_status_payload("tie_bi_san", 1800, 1.25, 0)
        .expect("zero duration multiplier uses one duration");
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["remaining_ticks"], 1800);
}

#[test]
fn combat_pill_buff_status_remaining_ticks_clamps_to_u32_max() {
    let bytes = build_pill_buff_status_payload("tie_bi_san", u64::from(u32::MAX), 1.25, 2)
        .expect("oversized duration should serialize after clamping");
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["remaining_ticks"], u64::from(u32::MAX));
}

#[test]
fn external_session_zero_timeout_is_not_expired_but_finite_deadline_is_inclusive() {
    assert!(!external_session_is_expired(0, u64::MAX));
    assert!(!external_session_is_expired(101, 100));
    assert!(external_session_is_expired(100, 100));
}

#[test]
fn requester_gate_dimension_falls_back_to_entity_layer() {
    let overworld = Entity::from_raw(101);
    let tsy = Entity::from_raw(102);
    let layers = DimensionLayers { overworld, tsy };
    assert_eq!(
        dimension_for_target_layer(None, Some(&EntityLayerId(overworld)), Some(&layers)),
        Some(DimensionKind::Overworld)
    );
    assert_eq!(
        dimension_for_target_layer(None, Some(&EntityLayerId(tsy)), Some(&layers)),
        Some(DimensionKind::Tsy)
    );
    assert_eq!(
        dimension_for_target_layer(
            None,
            Some(&EntityLayerId(Entity::from_raw(103))),
            Some(&layers),
        ),
        None
    );
}

#[test]
fn meridian_label_maps_regular_and_extraordinary_channels() {
    let cases = [
        (MeridianId::Lung, "肺经"),
        (MeridianId::LargeIntestine, "大肠经"),
        (MeridianId::Stomach, "胃经"),
        (MeridianId::Spleen, "脾经"),
        (MeridianId::Heart, "心经"),
        (MeridianId::SmallIntestine, "小肠经"),
        (MeridianId::Bladder, "膀胱经"),
        (MeridianId::Kidney, "肾经"),
        (MeridianId::Pericardium, "心包经"),
        (MeridianId::TripleEnergizer, "三焦经"),
        (MeridianId::Gallbladder, "胆经"),
        (MeridianId::Liver, "肝经"),
        (MeridianId::Ren, "任脉"),
        (MeridianId::Du, "督脉"),
        (MeridianId::Chong, "冲脉"),
        (MeridianId::Dai, "带脉"),
        (MeridianId::YinQiao, "阴跷脉"),
        (MeridianId::YangQiao, "阳跷脉"),
        (MeridianId::YinWei, "阴维脉"),
        (MeridianId::YangWei, "阳维脉"),
    ];
    for (id, expected) in cases {
        assert_eq!(
            meridian_label(&id.channel_id()),
            expected,
            "label for {id:?}"
        );
    }
}

#[test]
fn meridian_label_falls_back_for_unknown_channel_id() {
    assert_eq!(
        meridian_label(&crate::cultivation::components::MeridianChannelId::new(
            "tail_fin_channel",
        )),
        "未知经脉"
    );
}

#[test]
fn alchemy_explode_tier_three_scales_backlash_above_tier_one() {
    let tier_one = scale_alchemy_explosion_damage(40.0, 1);
    let tier_three = scale_alchemy_explosion_damage(40.0, 3);
    assert!(tier_one > 0.0);
    assert!(tier_three > tier_one);
    assert_eq!(tier_three, 80.0);
    assert!(scale_alchemy_explosion_crack(0.3, 3) > scale_alchemy_explosion_crack(0.3, 1));
}

fn lookup_item(instance_id: u64) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: "bone_whistle".to_string(),
        display_name: "测试物品".to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 0.0,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    }
}

fn lookup_inventory() -> PlayerInventory {
    PlayerInventory {
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: "main_pack".to_string(),
            name: "main_pack".to_string(),
            rows: 5,
            cols: 7,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
        triggered_treasures: Vec::new(),
    }
}

#[test]
fn inventory_instance_id_by_template_prefers_containers_hotbar_then_equipped() {
    let mut inventory = lookup_inventory();
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: lookup_item(11),
    });
    inventory.hotbar[0] = Some(lookup_item(22));
    inventory.equipped.insert(
        crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
        crate::inventory::SlotContents::held_single(lookup_item(33)),
    );
    assert_eq!(
        inventory_instance_id_by_template(&inventory, "bone_whistle"),
        Some(11)
    );
    inventory.containers[0].items.clear();
    assert_eq!(
        inventory_instance_id_by_template(&inventory, "bone_whistle"),
        Some(22)
    );
}

#[test]
fn inventory_instance_id_by_template_finds_worn_equipped_item() {
    let mut inventory = lookup_inventory();
    inventory.equipped.insert(
        crate::inventory::EQUIP_SLOT_CHEST.to_string(),
        crate::inventory::SlotContents::worn_single(lookup_item(44)),
    );
    assert_eq!(
        inventory_instance_id_by_template(&inventory, "bone_whistle"),
        Some(44)
    );
}

#[path = "client_request_handler_migrated_tests.rs"]
mod migrated_tests;

#[test]
fn inventory_instance_id_by_template_uses_stable_equipped_slot_order() {
    let mut inventory = lookup_inventory();
    inventory.equipped.insert(
        crate::inventory::EQUIP_SLOT_OFF_HAND.to_string(),
        crate::inventory::SlotContents::held_single(lookup_item(55)),
    );
    inventory.equipped.insert(
        crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
        crate::inventory::SlotContents::held_single(lookup_item(66)),
    );
    assert_eq!(
        inventory_instance_id_by_template(&inventory, "bone_whistle"),
        Some(66)
    );
}

#[test]
fn inventory_instance_id_by_template_returns_none_when_missing() {
    assert_eq!(
        inventory_instance_id_by_template(&lookup_inventory(), "bone_whistle"),
        None
    );
}

#[test]
fn qi_color_inspect_scope_requires_near_same_dimension_target() {
    assert_eq!(parse_qi_color_inspect_protocol_id("entity:42"), Some(42));
    assert_eq!(parse_qi_color_inspect_protocol_id("entity_bits:42"), None);
    assert_eq!(parse_qi_color_inspect_protocol_id("entity:bad"), None);
    let (_, radius) = crate::reach::DistanceRule::NEARBY_INTERACT
        .profile_parts()
        .expect("NearbyInteract must remain a named distance profile");
    assert!(is_qi_color_inspect_position_in_scope(
        DVec3::ZERO,
        DVec3::new(radius, 0.0, 0.0),
        true
    ));
    assert!(!is_qi_color_inspect_position_in_scope(
        DVec3::ZERO,
        DVec3::new(radius + 0.01, 0.0, 0.0),
        true
    ));
    assert!(!is_qi_color_inspect_position_in_scope(
        DVec3::ZERO,
        DVec3::new(1.0, 0.0, 0.0),
        false
    ));
}

#[test]
fn give_dan_target_state_gate_accepts_only_live_receiving_states() {
    use crate::fauna::dying_elder::{DyingElderState, DYING_ELDER_DAN_THRESHOLD};
    assert!(dying_elder_can_receive_dan(&DyingElderState::Plea));
    assert!(dying_elder_can_receive_dan(&DyingElderState::Recovering {
        dan_received: DYING_ELDER_DAN_THRESHOLD - 1,
    }));
    assert!(!dying_elder_can_receive_dan(&DyingElderState::Recovering {
        dan_received: DYING_ELDER_DAN_THRESHOLD,
    }));
    assert!(!dying_elder_can_receive_dan(&DyingElderState::Betrayal));
    assert!(!dying_elder_can_receive_dan(&DyingElderState::Dead {
        dead_by_betrayal: false,
    }));
}

#[test]
fn check_player_skill_meridian_gate_helper_unit_no_deps_passes() {
    assert!(check_player_skill_meridian_gate(
        "unknown.skill",
        &[],
        &MeridianSystem::default(),
        None,
        None,
    )
    .is_ok());
}

#[test]
fn check_player_skill_meridian_gate_helper_unit_rejects_severed_via_required() {
    let ms = MeridianSystem::default();
    let mut severed = MeridianSeveredPermanent::default();
    severed.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);
    let required = [TechniqueRequiredMeridian {
        channel: "Lung".to_string(),
        min_health: 0.5,
    }];
    assert_eq!(
        check_player_skill_meridian_gate("test.skill", &required, &ms, Some(&severed), None),
        Err(MeridianId::Lung)
    );
}

#[test]
fn check_player_skill_meridian_gate_helper_unit_rejects_low_integrity_via_required() {
    let mut ms = MeridianSystem::default();
    let lung = ms.get_mut(MeridianId::Lung);
    lung.opened = true;
    lung.integrity = 0.3;
    let required = [TechniqueRequiredMeridian {
        channel: "Lung".to_string(),
        min_health: 0.5,
    }];
    assert_eq!(
        check_player_skill_meridian_gate("test.skill", &required, &ms, None, None),
        Err(MeridianId::Lung)
    );
}

#[test]
fn check_player_skill_meridian_gate_helper_unit_rejects_via_deps_table_severed() {
    let ms = MeridianSystem::default();
    let mut severed = MeridianSeveredPermanent::default();
    severed.insert(MeridianId::Heart, SeveredSource::BackfireOverload, 5);
    let mut deps = SkillMeridianDependencies::default();
    deps.declare("test.skill", vec![MeridianId::Heart]);
    assert_eq!(
        check_player_skill_meridian_gate("test.skill", &[], &ms, Some(&severed), Some(&deps)),
        Err(MeridianId::Heart)
    );
}

#[test]
fn check_player_skill_meridian_gate_helper_unit_rejects_not_opened_via_required() {
    let mut ms = MeridianSystem::default();
    ms.get_mut(MeridianId::Lung).integrity = 1.0;
    let required = [TechniqueRequiredMeridian {
        channel: "Lung".to_string(),
        min_health: 0.5,
    }];
    assert_eq!(
        check_player_skill_meridian_gate("test.skill", &required, &ms, None, None),
        Err(MeridianId::Lung)
    );
}

#[test]
fn check_player_skill_meridian_gate_helper_unit_rejects_not_opened_via_deps_table() {
    let ms = MeridianSystem::default();
    let mut deps = SkillMeridianDependencies::default();
    deps.declare("test.skill", vec![MeridianId::Stomach]);
    assert_eq!(
        check_player_skill_meridian_gate("test.skill", &[], &ms, None, Some(&deps)),
        Err(MeridianId::Stomach)
    );
}

#[test]
fn check_player_skill_meridian_gate_helper_unit_passes_when_opened_and_integrity_satisfied() {
    let mut ms = MeridianSystem::default();
    let lung = ms.get_mut(MeridianId::Lung);
    lung.opened = true;
    lung.integrity = 0.8;
    let required = [TechniqueRequiredMeridian {
        channel: "Lung".to_string(),
        min_health: 0.5,
    }];
    assert!(check_player_skill_meridian_gate("test.skill", &required, &ms, None, None).is_ok());
}

#[test]
fn check_player_skill_meridian_gate_helper_unit_deps_table_passes_when_opened() {
    let mut ms = MeridianSystem::default();
    ms.get_mut(MeridianId::Kidney).opened = true;
    let mut deps = SkillMeridianDependencies::default();
    deps.declare("test.skill", vec![MeridianId::Kidney]);
    assert!(check_player_skill_meridian_gate("test.skill", &[], &ms, None, Some(&deps)).is_ok());
}

#[cfg(test)]
mod take_pill_tests {
    use super::*;
    use crate::inventory::{ContainerState, InventoryRevision, ItemInstance, ItemRarity};

    fn make_pill(instance_id: u64, template_id: &str, stack: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Rare,
            description: String::new(),
            stack_count: stack,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn fresh_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main".into(),
                name: "main".into(),
                rows: 4,
                cols: 4,
                items: Vec::new(),

                owner_instance_id: None,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    #[test]
    fn consume_hotbar_decrements_stack() {
        let mut inv = fresh_inventory();
        inv.hotbar[2] = Some(make_pill(1, "guyuan_pill", 3));
        assert!(consume_one_by_template(&mut inv, "guyuan_pill"));
        assert_eq!(inv.hotbar[2].as_ref().unwrap().stack_count, 2);
        assert_eq!(inv.revision.0, 1);
    }

    #[test]
    fn consume_hotbar_removes_slot_when_stack_one() {
        let mut inv = fresh_inventory();
        inv.hotbar[0] = Some(make_pill(1, "guyuan_pill", 1));
        assert!(consume_one_by_template(&mut inv, "guyuan_pill"));
        assert!(inv.hotbar[0].is_none());
    }

    #[test]
    fn consume_falls_back_to_container_when_hotbar_missing() {
        let mut inv = fresh_inventory();
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: make_pill(7, "guyuan_pill", 2),
            });
        assert!(consume_one_by_template(&mut inv, "guyuan_pill"));
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 1);
    }

    #[test]
    fn alchemy_attrition_selection_matches_consume_order() {
        let mut inv = fresh_inventory();
        inv.hotbar[0] = Some(make_pill(11, "guyuan_pill", 1));
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: make_pill(22, "guyuan_pill", 1),
            });

        assert_eq!(
            select_template_instances_for_consumption(&inv, "guyuan_pill", 1),
            vec![11],
            "投料磨损应命中 hotbar 中即将被 consume_one_by_template 消耗的实例"
        );
        assert!(consume_one_by_template(&mut inv, "guyuan_pill"));
        assert!(inv.hotbar[0].is_none());
        assert_eq!(inv.containers[0].items[0].instance.instance_id, 22);
    }

    #[test]
    fn alchemy_attrition_selection_spans_consumed_stacks_once_per_instance() {
        let mut inv = fresh_inventory();
        inv.hotbar[0] = Some(make_pill(11, "guyuan_pill", 2));
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: make_pill(22, "guyuan_pill", 3),
            });
        inv.equipped.insert(
            "off_hand".into(),
            crate::inventory::SlotContents::held_single(make_pill(33, "guyuan_pill", 1)),
        );

        assert_eq!(
            select_template_instances_for_consumption(&inv, "guyuan_pill", 5),
            vec![11, 22],
            "投料磨损应按 hotbar → containers → equipped 覆盖将被消耗的实例"
        );
    }

    #[test]
    fn alchemy_ingredient_selection_skips_wrong_mineral_and_uses_matching_instances() {
        let mut inv = fresh_inventory();
        let mut hotbar_wrong = make_pill(11, "dan_sha_aux", 2);
        hotbar_wrong.mineral_id = Some("zhu_sha".into());
        inv.hotbar[0] = Some(hotbar_wrong);

        let mut container_match = make_pill(22, "dan_sha_aux", 1);
        container_match.mineral_id = Some("dan_sha".into());
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: container_match,
            });

        let mut equipped_match = make_pill(33, "dan_sha_aux", 3);
        equipped_match.mineral_id = Some("dan_sha".into());
        inv.equipped.insert(
            "off_hand".into(),
            crate::inventory::SlotContents::held_single(equipped_match),
        );
        let ingredient = crate::alchemy::recipe::IngredientSpec {
            material: "dan_sha_aux".into(),
            count: 2,
            mineral_id: Some("dan_sha".into()),
        };

        assert_eq!(
            select_ingredient_instances_for_consumption(&inv, &ingredient, 2),
            Some(vec![(22, 1), (33, 1)]),
            "expected wrong mineral instance 11 to be skipped and matching instances to fill required count across inventory positions"
        );
    }

    #[test]
    fn alchemy_ingredient_selection_returns_none_when_matching_mineral_is_short() {
        let mut inv = fresh_inventory();
        let mut wrong_mineral = make_pill(11, "dan_sha_aux", 5);
        wrong_mineral.mineral_id = Some("zhu_sha".into());
        inv.hotbar[0] = Some(wrong_mineral);
        let mut matching_mineral = make_pill(22, "dan_sha_aux", 1);
        matching_mineral.mineral_id = Some("dan_sha".into());
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: matching_mineral,
            });
        let ingredient = crate::alchemy::recipe::IngredientSpec {
            material: "dan_sha_aux".into(),
            count: 2,
            mineral_id: Some("dan_sha".into()),
        };

        assert_eq!(
            select_ingredient_instances_for_consumption(&inv, &ingredient, 2),
            None,
            "expected selection to reject shortage when only one matching dan_sha item exists and wrong-mineral stacks cannot satisfy the ingredient"
        );
    }

    #[test]
    fn consume_returns_false_if_template_missing() {
        let mut inv = fresh_inventory();
        assert!(!consume_one_by_template(&mut inv, "ghost_pill"));
        assert_eq!(inv.revision.0, 0);
    }

    #[test]
    fn resolve_pill_consume_target_uses_exact_instance_when_provided() {
        let mut inv = fresh_inventory();
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: make_pill(7, "guyuan_pill", 1),
            });
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 1,
                instance: make_pill(8, "guyuan_pill", 1),
            });

        let item = resolve_pill_consume_target(&inv, "guyuan_pill", Some(8)).unwrap();

        assert_eq!(item.instance_id, 8);
    }

    #[test]
    fn shelflife_warn_emits_spoil_warning() {
        let profile = crate::shelflife::DecayProfile::Spoil {
            id: crate::shelflife::DecayProfileId::new("test_spoil"),
            formula: crate::shelflife::DecayFormula::Exponential {
                half_life_ticks: 100,
            },
            spoil_threshold: 60.0,
        };
        let mut profiles = DecayProfileRegistry::new();
        profiles.insert(profile.clone()).unwrap();
        let mut item = make_pill(9, "guyuan_pill", 1);
        item.freshness = Some(crate::shelflife::Freshness::new(0, 100.0, &profile));

        let (spoil, age) = shelflife_checks_for_item(&item, 100, Some(&profiles), None);

        assert!(matches!(spoil, SpoilCheckOutcome::Warn { .. }));
        assert!(matches!(age, AgePeakCheck::NotApplicable));
    }

    #[test]
    fn shelflife_critical_block_is_detected_before_consumption() {
        let profile = crate::shelflife::DecayProfile::Spoil {
            id: crate::shelflife::DecayProfileId::new("test_spoil"),
            formula: crate::shelflife::DecayFormula::Exponential {
                half_life_ticks: 100,
            },
            spoil_threshold: 60.0,
        };
        let mut profiles = DecayProfileRegistry::new();
        profiles.insert(profile.clone()).unwrap();
        let mut item = make_pill(9, "guyuan_pill", 1);
        item.freshness = Some(crate::shelflife::Freshness::new(0, 100.0, &profile));

        let (spoil, _age) = shelflife_checks_for_item(&item, 1_000, Some(&profiles), None);

        assert!(matches!(spoil, SpoilCheckOutcome::CriticalBlock { .. }));
    }

    #[test]
    fn shelflife_checks_use_forced_world_season_state() {
        let profile = crate::shelflife::DecayProfile::Spoil {
            id: crate::shelflife::DecayProfileId::new("test_spoil"),
            formula: crate::shelflife::DecayFormula::Exponential {
                half_life_ticks: 100,
            },
            spoil_threshold: 60.0,
        };
        let mut profiles = DecayProfileRegistry::new();
        profiles.insert(profile.clone()).unwrap();
        let mut item = make_pill(9, "guyuan_pill", 1);
        item.freshness = Some(crate::shelflife::Freshness::new(0, 100.0, &profile));
        let now_tick = 70;
        let mut forced = WorldSeasonState::default();
        forced.set_phase(crate::world::season::Season::Winter, now_tick);

        let (raw_spoil, _) = shelflife_checks_for_item(&item, now_tick, Some(&profiles), None);
        let (forced_spoil, _) =
            shelflife_checks_for_item(&item, now_tick, Some(&profiles), Some(&forced));

        assert!(
            matches!(raw_spoil, SpoilCheckOutcome::Warn { .. }),
            "raw tick should still be summer-fast enough to warn"
        );
        assert!(
            matches!(forced_spoil, SpoilCheckOutcome::Safe { .. }),
            "forced winter phase should slow spoil checks immediately"
        );
    }
}

#[cfg(test)]
mod named_faction_reputation_tests {
    use super::*;
    use crate::npc::faction::{FactionId, FactionRank, MissionQueue, NamedFactionId, Reputation};

    fn membership_with_loyalty(loyalty: f64) -> FactionMembership {
        FactionMembership {
            faction_id: FactionId::Neutral,
            rank: FactionRank::Disciple,
            reputation: Reputation { loyalty },
            lineage: None,
            mission_queue: MissionQueue::default(),
        }
    }

    #[test]
    fn npc_zone_faction_reputation_replaces_global_identity_renown() {
        let mut identities = PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 80;
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 60);

        let score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            Some(&faction_reputation),
            Some("qingyun_peaks"),
        );

        assert_eq!(
            score, 60,
            "青云 zone NPC 应读取 QingyunHunters per_faction 信誉，而不是全局 identity Renown"
        );
    }

    #[test]
    fn npc_zone_faction_reputation_falls_back_to_identity_for_unknown_zone() {
        let mut identities = PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 80;
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 60);

        let score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            Some(&faction_reputation),
            Some("spawn"),
        );

        assert_eq!(
            score, -80,
            "未映射到具名势力的 zone 应保持 legacy identity Renown fallback"
        );
    }

    #[test]
    fn npc_zone_faction_reputation_falls_back_when_zone_or_reputation_missing() {
        let mut identities = PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 40;
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 60);

        let missing_zone_score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            Some(&faction_reputation),
            None,
        );
        let missing_reputation_score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            None,
            Some("qingyun_peaks"),
        );
        let empty_score = reputation_to_player_score_for_npc_zone(None, None, None, None);

        assert_eq!(
            missing_zone_score, -40,
            "zone_name=None 时必须回退 legacy identity reputation，避免误读具名势力信誉"
        );
        assert_eq!(
            missing_reputation_score, -40,
            "玩家缺少 FactionReputation 组件时必须回退 legacy identity reputation"
        );
        assert_eq!(
            empty_score, 0,
            "缺少 membership/identity/faction reputation 的空输入应保持中立 0"
        );
    }

    #[test]
    fn npc_zone_faction_reputation_clamps_membership_plus_faction_score() {
        let high_membership = membership_with_loyalty(1.0);
        let low_membership = membership_with_loyalty(0.0);
        let medium_membership = membership_with_loyalty(0.245);
        let mut high_reputation = FactionReputation::default();
        high_reputation.apply_delta(NamedFactionId::QingyunHunters, 1);
        let mut low_reputation = FactionReputation::default();
        low_reputation.apply_delta(NamedFactionId::QingyunHunters, -1);
        let mut off_by_one_reputation = FactionReputation::default();
        off_by_one_reputation.apply_delta(NamedFactionId::QingyunHunters, 50);

        let upper = reputation_to_player_score_for_npc_zone(
            Some(&high_membership),
            None,
            Some(&high_reputation),
            Some("qingyun_peaks"),
        );
        let lower = reputation_to_player_score_for_npc_zone(
            Some(&low_membership),
            None,
            Some(&low_reputation),
            Some("qingyun_peaks"),
        );
        let off_by_one = reputation_to_player_score_for_npc_zone(
            Some(&medium_membership),
            None,
            Some(&off_by_one_reputation),
            Some("qingyun_peaks"),
        );

        assert_eq!(
            upper, 100,
            "membership baseline + faction score 超过上界时必须 clamp 到 100"
        );
        assert_eq!(
            lower, -100,
            "membership baseline + faction score 低于下界时必须 clamp 到 -100"
        );
        assert_eq!(
            off_by_one, -1,
            "未触及边界的 membership baseline + faction score 不应被误 clamp"
        );
    }

    #[test]
    fn wanted_tier_blocks_trade_even_when_score_would_otherwise_allow() {
        let target = NpcEngagementTarget {
            entity: Entity::PLACEHOLDER,
            archetype: NpcArchetype::Commoner,
            reputation_to_player: 100,
            faction_reputation_tier: FactionReputationTier::Wanted,
            display_name: "青云残峰散修".to_string(),
            greeting_text: String::new(),
            position: DVec3::ZERO,
            npc_player_rep: None,
        };

        assert!(
            !target.can_trade(),
            "Wanted tier 必须优先阻断交易，即使 reputation_to_player 分数本身足够高"
        );
    }
}

// ── plan-cultivation-pacing-v1 P2.2 NPC 丹药交易测试 ──

#[cfg(test)]
mod npc_flawed_pill_trade_tests {
    use super::*;
    use crate::npc::lifecycle::NpcArchetype;

    #[test]
    fn commoner_sells_flawed_ling_xi_wan_at_8_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Commoner, "ling_xi_wan_flawed");
        assert_eq!(
            result,
            Some(("ling_xi_wan_flawed", 8)),
            "Commoner 应以 8 骨币售卖次品灵息丸"
        );
    }

    #[test]
    fn commoner_sells_flawed_ju_ling_dan_at_15_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Commoner, "ju_ling_dan_flawed");
        assert_eq!(
            result,
            Some(("ju_ling_dan_flawed", 15)),
            "Commoner 应以 15 骨币售卖次品聚灵丹"
        );
    }

    #[test]
    fn rogue_sells_flawed_ling_xi_wan_at_8_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Rogue, "ling_xi_wan_flawed");
        assert_eq!(
            result,
            Some(("ling_xi_wan_flawed", 8)),
            "Rogue 也应以 8 骨币售卖次品灵息丸"
        );
    }

    #[test]
    fn rogue_sells_flawed_ju_ling_dan_at_15_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Rogue, "ju_ling_dan_flawed");
        assert_eq!(
            result,
            Some(("ju_ling_dan_flawed", 15)),
            "Rogue 也应以 15 骨币售卖次品聚灵丹"
        );
    }

    #[test]
    fn chinese_alias_also_resolves_for_commoner() {
        assert_eq!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ling_xi_wan_次品"),
            Some(("ling_xi_wan_flawed", 8)),
            "中文别名 ling_xi_wan_次品 应解析到同一物品"
        );
        assert_eq!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ju_ling_dan_次品"),
            Some(("ju_ling_dan_flawed", 15)),
            "中文别名 ju_ling_dan_次品 应解析到同一物品"
        );
    }

    #[test]
    fn beast_does_not_sell_flawed_pills() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Beast, "ling_xi_wan_flawed").is_none(),
            "Beast 不应售卖次品丹药"
        );
    }

    #[test]
    fn zombie_does_not_sell_flawed_pills() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Zombie, "ling_xi_wan_flawed").is_none(),
            "Zombie 不应售卖次品丹药"
        );
    }

    #[test]
    fn normal_pills_not_in_npc_catalog() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ling_xi_wan").is_none(),
            "正品灵息丸不应在 NPC 交易目录中"
        );
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ju_ling_dan").is_none(),
            "正品聚灵丹不应在 NPC 交易目录中"
        );
    }

    #[test]
    fn higher_pills_not_in_npc_catalog() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "tong_mai_san_flawed").is_none(),
            "通脉散以上 NPC 不售卖"
        );
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Rogue, "xi_sui_ye_flawed").is_none(),
            "洗髓液以上 NPC 不售卖"
        );
    }

    /// 买路 spirit_grass 条目价格应为 10 骨币（与 TRADE_CATALOGUE 对齐）。
    #[test]
    fn buy_path_spirit_grass_price_10() {
        let result = npc_trade_catalog_entry(NpcArchetype::Commoner, "spirit_grass");
        assert_eq!(
            result,
            Some(("spirit_grass", 10)),
            "买路 spirit_grass 应以 10 骨币售卖（与 TRADE_CATALOGUE 对齐），\
             期望: Some((\"spirit_grass\", 10))，实际: {:?}",
            result
        );
    }

    /// 买路 broken_artifact_scroll 条目价格应为 40 骨币（与 TRADE_CATALOGUE 对齐）。
    #[test]
    fn buy_path_broken_artifact_scroll_price_40() {
        let result = npc_trade_catalog_entry(NpcArchetype::Rogue, "broken_artifact_scroll");
        assert_eq!(
            result,
            Some(("broken_artifact_scroll", 40)),
            "买路 broken_artifact_scroll 应以 40 骨币售卖（与 TRADE_CATALOGUE 对齐），\
             期望: Some((\"broken_artifact_scroll\", 40))，实际: {:?}",
            result
        );
    }
}

// ── RefuseRare rarity 门控逻辑单元测试 ─────────────────────────────────────
// 验证 TradeEligibility::RefuseRare arm 对不同 ItemRarity 的判断逻辑是正确的：
// - Rare+ (Rare/Epic/Legendary/Ancient) → 拒绝
// - Common/Uncommon → 通过（1.3x markup）
//
// NOTE：这组测试直接调用生产函数 is_rarity_refused_at_low_rep，
// 确保任何变体增删/修改都会立刻让测试撞红。
#[cfg(test)]
mod refuse_rare_rarity_gate_tests {
    use crate::inventory::ItemRarity;
    use crate::network::client_request_handler::is_rarity_refused_at_low_rep;

    /// Low 信誉买 Rare 物品（broken_artifact_scroll，rarity=Rare）→ 应被拒绝。
    /// 期望：is_rarity_refused_at_low_rep(Rare) = true（触发 continue，不走到 add_item）。
    #[test]
    fn rare_rarity_is_refused_for_low_rep() {
        assert!(
            is_rarity_refused_at_low_rep(ItemRarity::Rare),
            "ItemRarity::Rare 应触发 RefuseRare 拒绝门控，\
             期望: is_rarity_refused_at_low_rep(Rare) = true，实际: false"
        );
    }

    /// Low 信誉买 Common 物品（spirit_grass，rarity=Common）→ 应通过。
    /// 期望：is_rarity_refused_at_low_rep(Common) = false（走到 1.3x 加价路径）。
    #[test]
    fn common_rarity_allowed_for_low_rep_with_markup() {
        assert!(
            !is_rarity_refused_at_low_rep(ItemRarity::Common),
            "ItemRarity::Common 不应触发 RefuseRare 门控，\
             期望: is_rarity_refused_at_low_rep(Common) = false，实际: true"
        );
    }

    /// Low 信誉买 Uncommon 物品（skill_scroll_herbalism_baicao_can，rarity=Uncommon）→ 应通过。
    /// 这是 Rare 阈值 off-by-one 边界：Uncommon 在 Rare 之下，应允许（1.3x）。
    #[test]
    fn uncommon_rarity_is_allowed_off_by_one_boundary() {
        assert!(
            !is_rarity_refused_at_low_rep(ItemRarity::Uncommon),
            "ItemRarity::Uncommon 是 Rare 阈值 off-by-one 边界（低于 Rare），\
             期望: is_rarity_refused_at_low_rep(Uncommon) = false（允许 1.3x markup），实际: true"
        );
    }

    /// Epic/Legendary/Ancient 全部应被拒绝（Rare+ 全覆盖）。
    #[test]
    fn epic_legendary_ancient_all_refused() {
        assert!(
            is_rarity_refused_at_low_rep(ItemRarity::Epic),
            "ItemRarity::Epic 应触发 RefuseRare 门控，\
             期望: true，实际: false"
        );
        assert!(
            is_rarity_refused_at_low_rep(ItemRarity::Legendary),
            "ItemRarity::Legendary 应触发 RefuseRare 门控，\
             期望: true，实际: false"
        );
        assert!(
            is_rarity_refused_at_low_rep(ItemRarity::Ancient),
            "ItemRarity::Ancient 应触发 RefuseRare 门控，\
             期望: true，实际: false"
        );
    }

    /// High/Mid 信誉不触发 RefuseRare——check_trade_eligibility 返回 Allowed，
    /// 不走 RefuseRare arm，所以 rarity 门控根本不会执行。
    /// 此测试通过验证 TradeEligibility 确认逻辑路径分叉正确。
    #[test]
    fn high_mid_rep_not_refused_by_eligibility() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        // High tier → Allowed（不走 RefuseRare arm）
        assert!(
            matches!(
                check_trade_eligibility(RepTier::High),
                TradeEligibility::Allowed { .. }
            ),
            "High 信誉不应走 RefuseRare arm，期望: Allowed，实际: 非 Allowed"
        );
        // Mid tier → Allowed（不走 RefuseRare arm）
        assert!(
            matches!(
                check_trade_eligibility(RepTier::Mid),
                TradeEligibility::Allowed { .. }
            ),
            "Mid 信誉不应走 RefuseRare arm，期望: Allowed，实际: 非 Allowed"
        );
    }

    /// Hostile 信誉触发 Refused（全拒），与 RefuseRare 是不同分支。
    #[test]
    fn hostile_rep_is_fully_refused_not_rare_gated() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        assert_eq!(
            check_trade_eligibility(RepTier::Hostile),
            TradeEligibility::Refused,
            "Hostile 信誉应触发 Refused（全拒），期望: Refused，实际: 非 Refused"
        );
    }

    /// Low 信誉对应 RefuseRare 资格——买路 broken_artifact_scroll(Rare) 在此分支下应被拒绝。
    #[test]
    fn low_rep_eligibility_is_refuse_rare() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        assert_eq!(
            check_trade_eligibility(RepTier::Low),
            TradeEligibility::RefuseRare,
            "Low 信誉应触发 RefuseRare，期望: RefuseRare，实际: 非 RefuseRare"
        );
    }

    /// 完整 RefuseRare 链路验证：Low rep + Rare 物品 → 被拒绝。
    /// 模拟 broken_artifact_scroll(Rare) 在 Low 声望下的完整判断链。
    #[test]
    fn full_refuse_rare_chain_rare_item_low_rep_refused() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        let rep_tier = RepTier::Low; // score ∈ (0.1, 0.3]
        let eligibility = check_trade_eligibility(rep_tier);
        assert_eq!(
            eligibility,
            TradeEligibility::RefuseRare,
            "Low rep 应得到 RefuseRare 资格"
        );
        // Rare 物品：应触发拒绝
        let is_rare = is_rarity_refused_at_low_rep(ItemRarity::Rare);
        assert!(
            is_rare,
            "broken_artifact_scroll(Rare) 应触发 RefuseRare 拒绝门控，\
             期望: is_rare = true，实际: false"
        );
    }

    /// 完整 RefuseRare 链路验证：Low rep + Common 物品 → 通过（1.3x markup）。
    /// 模拟 spirit_grass(Common) 在 Low 声望下的完整判断链。
    #[test]
    fn full_refuse_rare_chain_common_item_low_rep_allowed() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        let rep_tier = RepTier::Low;
        let eligibility = check_trade_eligibility(rep_tier);
        assert_eq!(
            eligibility,
            TradeEligibility::RefuseRare,
            "Low rep 应得到 RefuseRare 资格"
        );
        let is_rare = is_rarity_refused_at_low_rep(ItemRarity::Common);
        assert!(
            !is_rare,
            "spirit_grass(Common) 不应触发 RefuseRare 拒绝，\
             期望: is_rare = false（走 1.3x markup 路径），实际: true"
        );
        // 验证 1.3x 价格计算
        use crate::npc::trade::TradePricingConfig;
        let config = TradePricingConfig::default();
        let base_price = 10u64; // spirit_grass base price
        let final_price = (base_price as f64 * config.rep_low_markup as f64)
            .ceil()
            .max(1.0) as u64;
        assert_eq!(
            final_price, 13,
            "spirit_grass(10 骨币) 在 Low rep 1.3x markup 下应为 13 骨币，\
             期望: 13，实际: {}",
            final_price
        );
    }
}

// ─── plan-exploration-probe-return-v1 P1 — FreshnessProbe handler 测试 ───
#[cfg(test)]
mod skill_bar_ownership_gate_tests {
    use super::*;
    use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};

    fn make_known(entries: &[(&str, bool)]) -> KnownTechniques {
        KnownTechniques {
            entries: entries
                .iter()
                .map(|(id, active)| KnownTechnique {
                    id: (*id).to_string(),
                    proficiency: 0.5,
                    active: *active,
                })
                .collect(),
        }
    }

    /// Happy path: technique is present and active → gate passes.
    #[test]
    fn active_technique_is_known() {
        let kt = make_known(&[("sword.cleave", true)]);
        assert!(
            player_knows_technique(&kt, "sword.cleave"),
            "player_knows_technique must return true when technique is present and active; \
             entries={:?}",
            kt.entries
        );
    }

    /// Inactive technique in list → gate rejects (inactive = not in use / suspended).
    #[test]
    fn inactive_technique_is_not_known() {
        let kt = make_known(&[("sword.cleave", false)]);
        assert!(
            !player_knows_technique(&kt, "sword.cleave"),
            "player_knows_technique must return false when technique.active=false; \
             entries={:?}",
            kt.entries
        );
    }

    /// Technique not in list at all → gate rejects.
    #[test]
    fn absent_technique_is_not_known() {
        let kt = make_known(&[("sword.cleave", true)]);
        assert!(
            !player_knows_technique(&kt, "baomai.full_power_charge"),
            "player_knows_technique must return false when technique is absent from entries; \
             entries={:?}",
            kt.entries
        );
    }

    /// Empty KnownTechniques → gate rejects all techniques.
    #[test]
    fn empty_known_techniques_rejects_all() {
        let kt = KnownTechniques { entries: vec![] };
        assert!(
            !player_knows_technique(&kt, "sword.cleave"),
            "player_knows_technique must return false when KnownTechniques.entries is empty"
        );
        assert!(
            !player_knows_technique(&kt, "baomai.full_power_charge"),
            "player_knows_technique must return false for any technique when entries is empty"
        );
    }

    /// Multiple techniques, the target one active → gate passes.
    #[test]
    fn active_among_many_is_known() {
        let kt = make_known(&[
            ("sword.cleave", true),
            ("baomai.full_power_charge", true),
            ("burst_meridian.ni_mai_hu_ti", false),
        ]);
        assert!(
            player_knows_technique(&kt, "baomai.full_power_charge"),
            "player_knows_technique must return true for the active target technique \
             even when other techniques are also present; entries={:?}",
            kt.entries
        );
    }

    /// Multiple techniques, the target one inactive while others are active → gate rejects.
    #[test]
    fn inactive_among_active_siblings_is_not_known() {
        let kt = make_known(&[
            ("sword.cleave", true),
            ("baomai.full_power_charge", false),
            ("movement.dash", true),
        ]);
        assert!(
            !player_knows_technique(&kt, "baomai.full_power_charge"),
            "player_knows_technique must return false for inactive technique \
             even when other active techniques exist; entries={:?}",
            kt.entries
        );
    }

    /// The dangerous real-world case from the bug report: baomai.full_power_charge with
    /// empty required_meridians should be blocked at the ownership gate when not learned.
    #[test]
    fn baomai_full_power_charge_blocked_when_not_learned() {
        // Player has only basic sword techniques — has NOT learned baomai.
        let kt = make_known(&[("sword.cleave", true), ("sword.thrust", true)]);
        assert!(
            !player_knows_technique(&kt, "baomai.full_power_charge"),
            "An Awaken-realm player without baomai in KnownTechniques must be blocked \
             from casting baomai.full_power_charge (no meridian gate exists for this technique); \
             entries={:?}",
            kt.entries
        );
    }

    /// Gate passes for the ni_mai_hu_ti case from the bug report when the player has it.
    #[test]
    fn ni_mai_hu_ti_passes_when_learned() {
        let kt = make_known(&[("burst_meridian.ni_mai_hu_ti", true)]);
        assert!(
            player_knows_technique(&kt, "burst_meridian.ni_mai_hu_ti"),
            "player_knows_technique must return true for ni_mai_hu_ti when learned and active"
        );
    }

    /// Gate rejects ni_mai_hu_ti when not learned (original exploit path from bug report).
    #[test]
    fn ni_mai_hu_ti_blocked_when_not_learned() {
        let kt = make_known(&[("sword.cleave", true)]);
        assert!(
            !player_knows_technique(&kt, "burst_meridian.ni_mai_hu_ti"),
            "An Awaken-realm player without ni_mai_hu_ti in KnownTechniques must not be \
             able to bind or cast it, even though technique_definition lookup would succeed; \
             entries={:?}",
            kt.entries
        );
    }

    #[test]
    fn lingtian_plot_index_tracks_authoritative_positions() {
        let mut app = App::new();
        app.init_resource::<LingtianPlotIndex>();
        app.add_systems(Update, refresh_lingtian_plot_index);

        let first = BlockPos::new(-3, 64, 7);
        let second = BlockPos::new(9, 65, -11);
        let first_entity = app.world_mut().spawn(LingtianPlot::new(first, None)).id();
        app.world_mut().spawn(LingtianPlot::new(second, None));

        app.update();
        let index = app.world().resource::<LingtianPlotIndex>();
        assert!(
            index.contains(&first),
            "the refreshed index must admit the first authoritative plot position"
        );
        assert!(
            index.contains(&second),
            "the refreshed index must admit the second authoritative plot position"
        );

        app.world_mut().despawn(first_entity);
        app.update();
        assert!(
            !app.world().resource::<LingtianPlotIndex>().contains(&first),
            "despawned plots must disappear from the next ingress index snapshot"
        );
        assert!(
            app.world()
                .resource::<LingtianPlotIndex>()
                .contains(&second),
            "remaining plots must stay addressable after index refresh"
        );
    }
}
