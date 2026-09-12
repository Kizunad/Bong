//! `client_request_handler` 的私有契约测试登记处。
//!
//! 所有 CRH 契约测试均在此登记处运行；公开 C2S ingress 测试也保留在父模块
//! 的子模块中，避免为迁移新增仅供测试调用的生产可见性 seam。

use super::*;

use crate::combat::components::{WoundKind, Wounds};
use crate::combat::events::RevivalActionIntent;
use crate::cultivation::components::{MeridianId, MeridianSystem};
use crate::cultivation::known_techniques::TechniqueRequiredMeridian;
use crate::cultivation::meridian::severed::{MeridianSeveredPermanent, SeveredSource};
use crate::inventory::{
    ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
};
use crate::world::dimension::{DimensionKind, DimensionLayers};
use valence::custom_payload::CustomPayloadEvent;
use valence::prelude::{
    ident, App, BlockPos, DVec3, Entity, EntityLayerId, IntoSystemConfigs, Update,
};
use valence::testing::create_mock_client;

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

fn explosion_inventory_item(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: crate::inventory::ItemRarity::Common,
        description: String::new(),
        stack_count,
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

fn explosion_inventory_with_stack(template_id: &str, count: u32) -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: crate::inventory::InventoryRevision(0),
        containers: vec![crate::inventory::ContainerState {
            quick_access: false,
            id: "main_pack".into(),
            name: "main_pack".into(),
            rows: 5,
            cols: 7,
            items: vec![crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: explosion_inventory_item(9001, template_id, count),
            }],
            owner_instance_id: None,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
    }
}

fn load_test_technique_registry() -> TechniqueRegistry {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/cultivation/techniques.toml");
    TechniqueRegistry::load_from_path(path, &crate::body_plan::RaceRegistry::default())
        .expect("checked-in technique catalog must load")
}

fn register_explosion_test_resources(app: &mut App) {
    app.insert_resource(CombatClock::default());
    app.init_resource::<ClientRequestBudget>();
    app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
    app.insert_resource(crate::cultivation::skill_registry::init_registry());
    app.insert_resource(load_test_technique_registry());
    app.insert_resource(SkillMeridianDependencies::default());
    app.insert_resource(GameplayActionQueue::default());
    app.insert_resource(AlchemyMockState::default());
    app.insert_resource(DroppedLootRegistry::default());
    app.add_event::<crate::inventory::RemainsLootIntent>();
    app.insert_resource(ItemRegistry::default());
    app.insert_resource(RecipeRegistry::default());
    app.insert_resource(ZoneRegistry::fallback());
    app.init_resource::<SkillConfigStore>();
    app.insert_resource(SkillConfigSchemas::default());
    app.add_event::<CustomPayloadEvent>();
    app.add_event::<crate::combat::events::AttackIntent>();
    app.add_event::<crate::cultivation::burst_meridian::BurstMeridianEvent>();
    app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
    app.add_event::<crate::network::audio_event_emit::PlaySoundRecipeRequest>();
    app.add_event::<BreakthroughRequest>();
    app.add_event::<ForgeRequest>();
    app.add_event::<InsightChosen>();
    app.add_event::<DefenseIntent>();
    app.add_event::<RevivalActionIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<FalseSkinForgeRequest>();
    app.add_event::<PlaceFurnaceRequest>();
    app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_event::<SpiritNicheRepairRequest>();
    app.add_event::<SpiritNicheCoordinateRevealRequest>();
    app.add_event::<CoffinOpenRequest>();
    app.add_event::<crate::coffin::CoffinBreakRequest>();
    app.add_event::<crate::coffin::CoffinMenuReclaimRequest>();
    app.add_event::<crate::craft::CraftStartIntent>();
    app.add_event::<crate::fauna::dying_elder::GiveDanToElderIntent>();
    app.add_event::<crate::craft::WorkbenchOpenRequest>();
    app.add_event::<crate::world::container_open::ContainerOpenRequest>();
    app.add_event::<crate::lingtian::events::StartTillRequest>();
    app.add_event::<crate::lingtian::events::StartRenewRequest>();
    app.add_event::<crate::lingtian::events::StartPlantingRequest>();
    app.add_event::<crate::lingtian::events::StartHarvestRequest>();
    app.add_event::<crate::lingtian::events::StartReplenishRequest>();
    app.add_event::<crate::lingtian::events::StartDrainQiRequest>();
    app.add_event::<StartExtractRequestEvent>();
    app.add_event::<CancelExtractRequestEvent>();
    app.add_event::<QiColorInspectRequest>();
    app.add_event::<MineralProbeIntent>();
    app.add_event::<FreshnessProbeIntent>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SkillScrollUsed>();
    app.add_event::<BlockPlaceRequest>();
    app.add_event::<crate::zhenfa::ZhenfaPlaceRequest>();
    app.add_event::<crate::zhenfa::ZhenfaTriggerRequest>();
    app.add_event::<crate::zhenfa::ZhenfaDisarmRequest>();
    app.add_event::<crate::zhenfa::ScatterBeadUseRequest>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_event::<crate::alchemy::AlchemyOutcomeEvent>();
    app.add_event::<crate::combat::events::CombatEvent>();
    app.add_event::<crate::combat::events::DeathEvent>();
    app.add_event::<crate::combat::zhenmai_v2::LocalNeutralizeEvent>();
    app.add_event::<crate::combat::zhenmai_v2::MultiPointBackfireEvent>();
    app.add_event::<crate::combat::zhenmai_v2::MeridianHardenEvent>();
    app.add_event::<crate::combat::zhenmai_v2::MeridianSeveredVoluntaryEvent>();
    app.add_event::<crate::cultivation::meridian::severed::MeridianSeveredEvent>();
    app.add_event::<crate::cultivation::overload::MeridianOverloadEvent>();
    app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
    app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
    app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
    app.add_event::<crate::cmd::dev::block_picker::BlockPickerGiveIntent>();
}

fn register_explosion_test_systems(app: &mut App) {
    crate::network::register_lingtian_ingress_wiring(app);
    app.add_systems(
        Update,
        crate::network::inventory_event_emit::emit_durability_changed_inventory_events
            .after(crate::lingtian::LingtianRequestIngressSet),
    );
    app.add_systems(
        Update,
        crate::lingtian::systems::validate_and_dispatch_lingtian_requests
            .after(crate::lingtian::LingtianRequestIngressSet),
    );
    app.add_systems(
        Update,
        crate::alchemy::apply_alchemy_explode_outcomes.after(handle_client_request_payloads),
    );
}

#[test]
fn alchemy_explode_take_back_applies_damage_and_meridian_crack() {
    let mut app = App::new();
    register_explosion_test_resources(&mut app);
    register_explosion_test_systems(&mut app);
    app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
    app.insert_resource(crate::inventory::load_item_registry().unwrap());
    app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());

    let (client_bundle, _helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    let mut meridians = crate::cultivation::components::MeridianSystem::default();
    meridians
        .get_mut(crate::cultivation::components::MeridianId::Lung)
        .opened = true;
    app.world_mut().entity_mut(entity).insert((
        Wounds {
            health_current: 100.0,
            health_max: 100.0,
            entries: Vec::new(),
        },
        meridians,
        crate::cultivation::components::Cultivation::default(),
        PlayerState::default(),
        explosion_inventory_with_stack("ci_she_hao", 3),
    ));

    let mut furnace = crate::alchemy::AlchemyFurnace::placed(BlockPos::new(2, 64, 3), 1);
    furnace.owner = Some("offline:Azure".into());
    app.world_mut().spawn(furnace);
    for data in [
        br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[2,64,3],"recipe_id":"kai_mai_pill_v0"}"#.as_slice(),
        br#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[2,64,3],"slot_idx":0,"material":"ci_she_hao","count":3}"#.as_slice(),
        br#"{"type":"alchemy_intervention","v":1,"furnace_pos":[2,64,3],"intervention":{"kind":"adjust_temp","temp":1.0}}"#.as_slice(),
        br#"{"type":"alchemy_take_back","v":1,"furnace_pos":[2,64,3],"slot_idx":0}"#.as_slice(),
    ] {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: data.to_vec().into_boxed_slice(),
            });
    }

    app.update();

    let wounds = app.world().get::<Wounds>(entity).unwrap();
    assert_eq!(wounds.health_current, 80.0);
    assert!(wounds.entries.iter().any(|wound| {
        wound.kind == WoundKind::Burn && (wound.severity - 20.0).abs() < f32::EPSILON
    }));
    let overload_events = app
        .world()
        .resource::<valence::prelude::Events<crate::cultivation::overload::MeridianOverloadEvent>>(
        );
    let mut reader = overload_events.get_reader();
    let events: Vec<_> = reader.read(overload_events).collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].entity, entity);
    assert!((events[0].severity - 0.15).abs() < 1e-9);
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
        inv.hotbar[1] = Some(make_pill(1, "guyuan_pill", 3));
        assert!(consume_one_by_template(&mut inv, "guyuan_pill"));
        assert_eq!(inv.hotbar[1].as_ref().unwrap().stack_count, 2);
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

// 公开 ingress 契约测试回归登记在父模块内，避免为测试新增生产可见性 seam。
#[cfg(test)]
#[allow(dead_code, unused_imports)]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
mod external_ingress_tests {
    use super::*;
    use crate::schema::proto_gen::bong;
    use prost::Message;
    use std::collections::{HashMap, HashSet};

    fn load_test_technique_registry() -> TechniqueRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/cultivation/techniques.toml");
        TechniqueRegistry::load_from_path(path, &crate::body_plan::RaceRegistry::default())
            .expect("checked-in technique catalog must load")
    }

    use valence::custom_payload::CustomPayloadEvent;
    use valence::prelude::{
        bevy_ecs, BlockPos, ChunkLayer, Client, Commands, DVec3, Entity, EntityLayerId,
        EntityManager, EventReader, EventWriter, Events, Position, Query, RemovedComponents, Res,
        ResMut, Resource, UniqueId, Username, With, Without,
    };

    use crate::alchemy::residue::{residue_alchemy_data, residue_kind_for_recyclable_outcome};
    use crate::alchemy::{
        learned::LearnResult, AlchemyFurnace, AlchemySession, Intervention, LearnedRecipes,
        PlaceFurnaceRequest, RecipeRegistry, MIN_ZONE_QI_TO_ALCHEMY,
    };
    use crate::botany::components::HarvestSessionStore;
    use crate::coffin::{CoffinEnterRequest, CoffinLeaveRequest, CoffinPlaceRequest};
    use crate::combat::anqi_v2::{cycle_container_slot, switch_container_slot};
    use crate::combat::carrier::{CarrierSlot, ChargeCarrierIntent, ThrowCarrierIntent};
    use crate::combat::components::{
        CastSource, Casting, Lifecycle, LifecycleState, QuickSlotBindings, SkillBarBindings,
        SkillSlot, Stamina, Wounds,
    };
    #[cfg(test)]
    use crate::combat::events::RevivalActionIntent;
    use crate::combat::events::{ApplyStatusEffectIntent, DefenseIntent, StatusEffectKind};
    use crate::combat::foreign_qi_resistance::foreign_qi_resistance_for_use;
    use crate::combat::needle::IntentSource;
    use crate::combat::tuike::{
        can_equip_false_skin, false_skin_kind_for_item, FalseSkinForgeRequest,
    };
    use crate::combat::CombatClock;
    use crate::craft::workbench::workbench_block_pos;
    use crate::craft::WorkbenchBlock;
    use crate::cultivation::breakthrough::BreakthroughRequest;
    use crate::cultivation::components::{
        recover_current_qi, Cultivation, MeridianChannelId, MeridianId,
    };
    use crate::cultivation::dugu::SelfAntidoteIntent;
    use crate::cultivation::forging::ForgeRequest;
    use crate::cultivation::insight::{InsightChosen, InsightRequest};
    use crate::cultivation::known_techniques::{KnownTechniques, TechniqueRegistry};
    use crate::cultivation::lifespan::LifespanExtensionIntent;
    use crate::cultivation::meridian::severed::{
        MeridianSeveredPermanent, SkillMeridianDependencies,
    };
    use crate::cultivation::meridian_open::MeridianTarget;
    use crate::cultivation::poison_trait::{ConsumePoisonPillIntent, PoisonPillKind};
    use crate::cultivation::possession::{DuoSheRequestEvent, UseLifeCoreEvent};
    use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
    use crate::cultivation::technique_scroll::{TechniqueLearnedEvent, TechniqueScrollReadEvent};
    use crate::cultivation::tribulation::{HeartDemonChoiceSubmitted, StartDuXuRequest};
    use crate::cultivation::void::actions::VoidActionIntent;
    use crate::fauna::dying_elder::DyingElderState;
    use crate::forge::blueprint::BlueprintRegistry;
    #[cfg(test)]
    use crate::forge::blueprint::TemperBeat;
    use crate::forge::events::{
        ConsecrationInject, InscriptionScrollSubmit, StartForgeRequest, StepAdvance, TemperingHit,
    };
    use crate::forge::learned::LearnedBlueprints;
    use crate::forge::session::{ForgeSessionId, ForgeSessions, ForgeStep};
    use crate::forge::station::{PlaceForgeStationRequest, WeaponForgeStation};
    #[cfg(test)]
    use crate::inventory::add_item_to_player_inventory;
    use crate::inventory::{
        add_item_to_player_inventory_with_alchemy, apply_inventory_move_with_race,
        apply_item_spiritual_wear, consume_item_instance_once,
        discard_inventory_item_to_dropped_loot, fully_repair_weapon_instance,
        inventory_item_by_instance_borrow, pickup_dropped_loot_instance, DroppedLootRegistry,
        InventoryDurabilityChangedEvent, InventoryInstanceIdAllocator, InventoryMoveOutcome,
        InventoryMoveRejectReason, ItemInstance, PlayerInventory,
    };
    use crate::inventory::{
        AlchemyItemData, ItemCategory, ItemEffect, ItemRegistry,
        DEFAULT_CAST_DURATION_MS as TEMPLATE_DEFAULT_CAST_MS,
        DEFAULT_COOLDOWN_MS as TEMPLATE_DEFAULT_COOLDOWN_MS,
    };
    use crate::lingtian::session::{ReplenishSource, SessionMode};
    use crate::lingtian::LingtianPlot;
    use crate::mineral::probe::is_probe_target_in_range;
    use crate::mineral::MineralProbeIntent;
    use crate::movement::{MovementAction, MovementActionIntent};
    use crate::network::agent_bridge::{
        payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
    };
    use crate::network::alchemy_snapshot_emit;
    use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
    use crate::network::cast_emit::{
        current_unix_millis, push_cast_sync, CAST_INTERRUPT_COOLDOWN_TICKS,
    };
    use crate::network::gate::budget::BudgetStore;
    use crate::network::gate::{GateContext, GateDenialReason};
    use crate::shelflife::probe::FreshnessProbeIntent;
    // dropped_loot_sync is emitted by dropped_loot_sync_emit.
    #[cfg(test)]
    use crate::identity::PlayerIdentities;
    use crate::network::inventory_move_rejected_emit::emit_inventory_move_rejected;
    use crate::network::qi_attrition_emit::{
        emit_attrition_applied_if_lost, item_abs_qi_for_attrition, AttritionAppliedEvent,
    };
    use crate::network::qi_color_observed_emit::QiColorInspectRequest;
    use crate::network::{
        gameplay_vfx, redis_bridge::RedisOutbound, vfx_event_emit::VfxEventRequest,
        RedisBridgeResource,
    };
    #[cfg(test)]
    use crate::npc::faction::FactionMembership;
    use crate::npc::lifecycle::NpcArchetype;
    use crate::npc::spawn::NpcMarker;
    #[cfg(test)]
    use crate::npc::trade::NpcPlayerReputation;
    use crate::persistence::ZoneRuntimeRecord;
    use crate::player::gameplay::GameplayActionQueue;
    use crate::player::state::{
        canonical_player_id, save_player_inventory_and_delete_dropped_loot, PlayerState,
        PlayerStatePersistence,
    };
    use crate::qi_physics::attrition::{apply_attrition_checked, is_attrition_exempt};
    use crate::qi_physics::constants::QI_TARGETED_ITEM_WEAR_WEIGHT_THRESHOLD;
    use crate::qi_physics::ledger::AttritionOpKind;
    use crate::qi_physics::qi_targeted_item_wear_fraction;
    use crate::qi_physics::AnqiContainerKind;
    use crate::schema::alchemy::{AlchemyInterventionResultV1, AlchemySessionStartV1};
    use crate::schema::client_request::{ClientRequestV1, SkillBarBindingV1};
    use crate::schema::combat_hud::{CastOutcomeV1, CastPhaseV1, CastSyncV1};
    use crate::schema::common::EventKind;
    use crate::schema::inventory::{
        ContainerIdV1, EquipSlotV1, EquipStateV1, InventoryEventV1, InventoryLocationV1,
    };
    use crate::schema::server_data::{PillBuffStatusV1, ServerDataPayloadV1, ServerDataV1};
    use crate::schema::social::GuardianKindV1;
    use crate::shelflife::{
        age_peak_check_with_season, container_storage_multiplier, spoil_check_with_season,
        AgeBonusRoll, AgePeakCheck, ContainerFreshnessBehavior, DecayProfileRegistry,
        SpoilCheckOutcome, SpoilConsumeWarning, SpoilSeverity,
    };
    use crate::skill::components::SkillSet;
    use crate::skill::config::{
        handle_config_intent, skill_config_snapshot_for_cast, validate_skill_config,
        SkillConfigRejectReason, SkillConfigSchemas, SkillConfigSnapshot, SkillConfigStore,
    };
    use crate::skill::events::{SkillScrollUsed, SkillXpGain};
    #[cfg(test)]
    use crate::social::components::{FactionReputation, FactionReputationTier};
    use crate::social::events::{
        SpiritNicheActivateGuardianRequest, SpiritNicheCoordinateRevealRequest,
        SpiritNichePlaceRequest, SpiritNicheRepairRequest, SpiritNicheRevealSource,
    };
    use crate::world::block_place::BlockPlaceRequest;
    use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
    use crate::world::events::EVENT_REALM_COLLAPSE;
    use crate::world::extract_system::{
        CancelExtractRequest as CancelExtractRequestEvent,
        StartExtractRequest as StartExtractRequestEvent,
    };
    use crate::world::karma::KarmaWeightStore;
    use crate::world::season::{query_season, WorldSeasonState};
    use crate::world::spawn_tutorial::CoffinOpenRequest;
    use crate::world::tsy_container_search::{
        CancelSearchRequest as CancelSearchRequestEvent,
        StartSearchRequest as StartSearchRequestEvent,
    };
    use crate::world::tsy_lifecycle::TsyZoneStateRegistry;
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::alchemy::recipe::{
            FireProfile, IngredientSpec, Outcomes, Recipe, RecipeStage, ToleranceSpec,
        };
        use crate::botany::components::{
            BotanyHarvestMode, BotanyPhase, HarvestSession, HarvestSessionStore,
        };
        use crate::botany::registry::BotanyPlantId;
        use crate::combat::components::{Lifecycle, UnlockedStyles, WoundKind, Wounds};
        use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, Realm};
        use crate::cultivation::known_techniques::KnownTechniques;
        use crate::cultivation::tribulation::TribulationState;
        use crate::forge::session::{ForgeSession, StepState};
        use crate::inventory::{
            BlueprintScrollSpec, ContainerState, InscriptionScrollSpec, InventoryRevision,
            ItemCategory, ItemEffect, ItemInstance, ItemRarity, ItemTemplate, PlacedItemState,
        };
        use crate::lingtian::events::{
            StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest, StartRenewRequest,
            StartReplenishRequest, StartTillRequest,
        };
        use crate::npc::faction::{
            FactionId, FactionRank, MissionQueue, NamedFactionId, Reputation,
        };
        use crate::skill::components::{ScrollId, SkillId, SkillSet};
        use crate::zhenfa::trap_content::TrapTargetFace;
        use crate::zhenfa::{
            ScatterBeadUseRequest, ZhenfaDisarmRequest, ZhenfaPlaceRequest, ZhenfaTriggerRequest,
        };
        use valence::entity::{EntityId, EntityPlugin};
        use valence::prelude::{
            ident, App, BlockPos, BlockState, DVec3, Entity, EntityKind, EntityLayer, EventReader,
            IntoSystemConfigs, OldPosition, Position, ResMut, UnloadedChunk, Update,
        };
        use valence::protocol::packets::play::{CustomPayloadS2c, GameMessageS2c};
        use valence::testing::{create_mock_client, MockClientHelper, ScenarioSingleClient};

        fn mark_test_layer_as_overworld(app: &mut App) {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, (With<ChunkLayer>, With<EntityLayer>)>();
            let layer = query
                .iter(world)
                .next()
                .expect("test scenario should have spawned a layer entity");
            world
                .entity_mut(layer)
                .insert(crate::world::dimension::OverworldLayer);
        }

        #[derive(Default)]
        struct CapturedBreakthroughRequests(Vec<BreakthroughRequest>);

        impl valence::prelude::Resource for CapturedBreakthroughRequests {}

        #[derive(Default)]
        struct CapturedForgeRequests(Vec<ForgeRequest>);

        impl valence::prelude::Resource for CapturedForgeRequests {}

        #[derive(Default)]
        struct CapturedStartDuXuRequests(Vec<StartDuXuRequest>);

        impl valence::prelude::Resource for CapturedStartDuXuRequests {}

        #[derive(Default)]
        struct CapturedInsightChoices(Vec<InsightChosen>);

        impl valence::prelude::Resource for CapturedInsightChoices {}

        #[derive(Default)]
        struct CapturedMineralProbes(Vec<MineralProbeIntent>);

        impl valence::prelude::Resource for CapturedMineralProbes {}

        #[derive(Default)]
        struct CapturedSpiritNichePlaces(Vec<SpiritNichePlaceRequest>);

        impl valence::prelude::Resource for CapturedSpiritNichePlaces {}

        #[derive(Default)]
        struct CapturedSpiritNicheRepairs(Vec<SpiritNicheRepairRequest>);

        impl valence::prelude::Resource for CapturedSpiritNicheRepairs {}

        #[derive(Default)]
        struct CapturedSpiritNicheCoordinateReveals(Vec<SpiritNicheCoordinateRevealRequest>);

        impl valence::prelude::Resource for CapturedSpiritNicheCoordinateReveals {}

        #[derive(Default)]
        struct CapturedCoffinOpenRequests(Vec<CoffinOpenRequest>);

        impl valence::prelude::Resource for CapturedCoffinOpenRequests {}

        #[derive(Default)]
        struct CapturedCoffinBreakRequests(Vec<crate::coffin::CoffinBreakRequest>);

        impl valence::prelude::Resource for CapturedCoffinBreakRequests {}

        #[derive(Default)]
        struct CapturedCoffinMenuReclaimRequests(Vec<crate::coffin::CoffinMenuReclaimRequest>);

        impl valence::prelude::Resource for CapturedCoffinMenuReclaimRequests {}

        #[derive(Default)]
        struct CapturedInscriptionScrolls(Vec<InscriptionScrollSubmit>);

        impl valence::prelude::Resource for CapturedInscriptionScrolls {}

        #[derive(Default)]
        struct CapturedTemperingHits(Vec<TemperingHit>);

        impl valence::prelude::Resource for CapturedTemperingHits {}

        #[derive(Default)]
        struct CapturedConsecrationInjects(Vec<ConsecrationInject>);

        impl valence::prelude::Resource for CapturedConsecrationInjects {}

        #[derive(Default)]
        struct CapturedStepAdvances(Vec<StepAdvance>);

        impl valence::prelude::Resource for CapturedStepAdvances {}

        #[derive(Default)]
        struct CapturedQiColorInspectRequests(Vec<QiColorInspectRequest>);

        impl valence::prelude::Resource for CapturedQiColorInspectRequests {}

        #[derive(Default)]
        struct CapturedFreshnessProbes(Vec<FreshnessProbeIntent>);

        impl valence::prelude::Resource for CapturedFreshnessProbes {}

        #[derive(Default)]
        struct CapturedRaiseShieldIntents(Vec<crate::combat::shield_block::RaiseShieldIntent>);

        impl valence::prelude::Resource for CapturedRaiseShieldIntents {}

        #[derive(Default)]
        struct CapturedLowerShieldIntents(Vec<crate::combat::shield_block::LowerShieldIntent>);

        impl valence::prelude::Resource for CapturedLowerShieldIntents {}

        fn capture_breakthrough_requests(
            mut events: EventReader<BreakthroughRequest>,
            mut captured: ResMut<CapturedBreakthroughRequests>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_forge_requests(
            mut events: EventReader<ForgeRequest>,
            mut captured: ResMut<CapturedForgeRequests>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_start_du_xu_requests(
            mut events: EventReader<StartDuXuRequest>,
            mut captured: ResMut<CapturedStartDuXuRequests>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_insight_choices(
            mut events: EventReader<InsightChosen>,
            mut captured: ResMut<CapturedInsightChoices>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_mineral_probes(
            mut events: EventReader<MineralProbeIntent>,
            mut captured: ResMut<CapturedMineralProbes>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_spirit_niche_places(
            mut events: EventReader<SpiritNichePlaceRequest>,
            mut captured: ResMut<CapturedSpiritNichePlaces>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_spirit_niche_repairs(
            mut events: EventReader<SpiritNicheRepairRequest>,
            mut captured: ResMut<CapturedSpiritNicheRepairs>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_spirit_niche_coordinate_reveals(
            mut events: EventReader<SpiritNicheCoordinateRevealRequest>,
            mut captured: ResMut<CapturedSpiritNicheCoordinateReveals>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_coffin_open_requests(
            mut events: EventReader<CoffinOpenRequest>,
            mut captured: ResMut<CapturedCoffinOpenRequests>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_coffin_break_requests(
            mut events: EventReader<crate::coffin::CoffinBreakRequest>,
            mut captured: ResMut<CapturedCoffinBreakRequests>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_coffin_menu_reclaim_requests(
            mut events: EventReader<crate::coffin::CoffinMenuReclaimRequest>,
            mut captured: ResMut<CapturedCoffinMenuReclaimRequests>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_inscription_scrolls(
            mut events: EventReader<InscriptionScrollSubmit>,
            mut captured: ResMut<CapturedInscriptionScrolls>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_tempering_hits(
            mut events: EventReader<TemperingHit>,
            mut captured: ResMut<CapturedTemperingHits>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_consecration_injects(
            mut events: EventReader<ConsecrationInject>,
            mut captured: ResMut<CapturedConsecrationInjects>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_step_advances(
            mut events: EventReader<StepAdvance>,
            mut captured: ResMut<CapturedStepAdvances>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_qi_color_inspect_requests(
            mut events: EventReader<QiColorInspectRequest>,
            mut captured: ResMut<CapturedQiColorInspectRequests>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn skill_scroll_item(instance_id: u64, template_id: &str) -> ItemInstance {
            ItemInstance {
                instance_id,
                template_id: template_id.to_string(),
                display_name: template_id.to_string(),
                grid_w: 1,
                grid_h: 2,
                weight: 0.05,
                rarity: ItemRarity::Uncommon,
                description: String::new(),
                stack_count: 1,
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

        fn test_forge_template_registry() -> ItemRegistry {
            ItemRegistry::from_map(HashMap::from([
                (
                    "blueprint_scroll_ling_feng".to_string(),
                    ItemTemplate {
                        id: "blueprint_scroll_ling_feng".to_string(),
                        display_name: "灵锋图谱残卷".to_string(),
                        category: ItemCategory::Misc,
                        placeable: None,
                        max_stack_count: 1,
                        grid_w: 1,
                        grid_h: 1,
                        base_weight: 0.05,
                        rarity: ItemRarity::Rare,
                        spirit_quality_initial: 0.9,
                        description: String::new(),
                        effect: None,
                        cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                        cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                        weapon_spec: None,
                        forge_station_spec: None,
                        blueprint_scroll_spec: Some(BlueprintScrollSpec {
                            blueprint_id: "ling_feng_v0".to_string(),
                        }),
                        inscription_scroll_spec: None,
                        technique_scroll_spec: None,
                        readable_scroll_spec: None,
                        recipe_fragment_spec: None,
                        container_spec: None,
                        shelflife_profile: None,
                        shield_spec: None,
                        shelflife_track: None,
                        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                    },
                ),
                (
                    "inscription_scroll_sharp_v0".to_string(),
                    ItemTemplate {
                        id: "inscription_scroll_sharp_v0".to_string(),
                        display_name: "锐意铭文残卷".to_string(),
                        category: ItemCategory::Misc,
                        placeable: None,
                        max_stack_count: 1,
                        grid_w: 1,
                        grid_h: 1,
                        base_weight: 0.03,
                        rarity: ItemRarity::Uncommon,
                        spirit_quality_initial: 0.8,
                        description: String::new(),
                        effect: None,
                        cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                        cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                        weapon_spec: None,
                        forge_station_spec: None,
                        blueprint_scroll_spec: None,
                        inscription_scroll_spec: Some(InscriptionScrollSpec {
                            inscription_id: "sharp_v0".to_string(),
                        }),
                        technique_scroll_spec: None,
                        readable_scroll_spec: None,
                        recipe_fragment_spec: None,
                        container_spec: None,
                        shelflife_profile: None,
                        shield_spec: None,
                        shelflife_track: None,
                        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                    },
                ),
            ]))
        }

        fn inventory_with_skill_scroll(item: ItemInstance) -> PlayerInventory {
            PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(0),
                containers: vec![ContainerState {
                    quick_access: false,
                    id: "main_pack".into(),
                    name: "main_pack".into(),
                    rows: 5,
                    cols: 7,
                    items: vec![PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: item,
                    }],

                    owner_instance_id: None,
                }],
                equipped: Default::default(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            }
        }

        fn inventory_with_stack(template_id: &str, count: u32) -> PlayerInventory {
            PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(0),
                containers: vec![ContainerState {
                    quick_access: false,
                    id: "main_pack".into(),
                    name: "main_pack".into(),
                    rows: 5,
                    cols: 7,
                    items: vec![PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: inventory_test_item(9001, template_id, count),
                    }],

                    owner_instance_id: None,
                }],
                equipped: Default::default(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            }
        }

        fn inventory_test_item(
            instance_id: u64,
            template_id: &str,
            stack_count: u32,
        ) -> ItemInstance {
            ItemInstance {
                instance_id,
                template_id: template_id.to_string(),
                display_name: template_id.to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.1,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count,
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

        fn empty_inventory() -> PlayerInventory {
            PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(0),
                containers: vec![ContainerState {
                    quick_access: false,
                    id: "main_pack".into(),
                    name: "main_pack".into(),
                    rows: 5,
                    cols: 7,
                    items: Vec::new(),

                    owner_instance_id: None,
                }],
                equipped: Default::default(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            }
        }

        fn decode_proto_server_data_payload(
            bytes: &[u8],
        ) -> Option<crate::schema::proto_gen::bong::server_data_envelope::Payload> {
            bong::ServerDataEnvelope::decode(bytes).ok()?.payload
        }

        /// The integration target links `bong_server` as a normal dependency, so the
        /// library's `cfg(test)` JSON serializer is intentionally not active here.
        /// Decode the production protobuf wire instead of changing the wire contract
        /// just to preserve an inline-test implementation detail.
        fn server_data_payload_type(bytes: &[u8]) -> Option<String> {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) {
                return value
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
            }
            use crate::schema::proto_gen::bong::server_data_envelope::Payload;
            match decode_proto_server_data_payload(bytes)? {
                Payload::InventorySnapshot(_) => Some("inventory_snapshot".to_owned()),
                Payload::AlchemyFurnace(_) => Some("alchemy_furnace".to_owned()),
                Payload::AlchemySession(_) => Some("alchemy_session".to_owned()),
                Payload::ForgeBlueprintBook(_) => Some("forge_blueprint_book".to_owned()),
                Payload::CastSync(_) => Some("cast_sync".to_owned()),
                Payload::QuickSlotConfig(_) => Some("quickslot_config".to_owned()),
                Payload::SkillConfigSnapshot(_) => Some("skill_config_snapshot".to_owned()),
                Payload::EventAlert(_) => Some("event_alert".to_owned()),
                Payload::InventoryEvent(_) => Some("inventory_event".to_owned()),
                Payload::LootContainerOpen(_) => Some("loot_container_open".to_owned()),
                Payload::LootContainerUpdate(_) => Some("loot_container_update".to_owned()),
                Payload::InventoryMoveRejected(_) => Some("inventory_move_rejected".to_owned()),
                _ => None,
            }
        }

        fn decode_alchemy_session_payload(
            bytes: &[u8],
        ) -> Option<crate::schema::alchemy::AlchemySessionDataV1> {
            if let Ok(payload) = serde_json::from_slice::<ServerDataV1>(bytes) {
                if let ServerDataPayloadV1::AlchemySession(snapshot) = payload.payload {
                    return Some(*snapshot);
                }
            }
            let crate::schema::proto_gen::bong::server_data_envelope::Payload::AlchemySession(data) =
                decode_proto_server_data_payload(bytes)?
            else {
                return None;
            };
            Some(crate::schema::alchemy::AlchemySessionDataV1 {
                recipe_id: data.recipe_id,
                active: data.active,
                elapsed_ticks: data.elapsed_ticks,
                target_ticks: data.target_ticks,
                temp_current: data.temp_current,
                temp_target: data.temp_target,
                temp_band: data.temp_band,
                qi_injected: data.qi_injected,
                qi_target: data.qi_target,
                status_label: data.status_label,
                stages: data
                    .stages
                    .into_iter()
                    .map(|stage| crate::schema::alchemy::AlchemyStageHintV1 {
                        at_tick: stage.at_tick,
                        window: stage.window,
                        summary: stage.summary,
                        completed: stage.completed,
                        missed: stage.missed,
                    })
                    .collect(),
                interventions_recent: data.interventions_recent,
            })
        }

        fn decode_alchemy_furnace_payload(
            bytes: &[u8],
        ) -> Option<crate::schema::alchemy::AlchemyFurnaceDataV1> {
            if let Ok(payload) = serde_json::from_slice::<ServerDataV1>(bytes) {
                if let ServerDataPayloadV1::AlchemyFurnace(data) = payload.payload {
                    return Some(*data);
                }
            }
            let crate::schema::proto_gen::bong::server_data_envelope::Payload::AlchemyFurnace(data) =
                decode_proto_server_data_payload(bytes)?
            else {
                return None;
            };
            let pos = match (data.pos_x, data.pos_y, data.pos_z) {
                (Some(x), Some(y), Some(z)) => Some((x, y, z)),
                (None, None, None) => None,
                _ => return None,
            };
            Some(crate::schema::alchemy::AlchemyFurnaceDataV1 {
                pos,
                tier: u8::try_from(data.tier).ok()?,
                integrity: data.integrity,
                integrity_max: data.integrity_max,
                owner_name: data.owner_name,
                has_session: data.has_session,
            })
        }

        fn decode_skill_config_snapshot_payload(
            bytes: &[u8],
        ) -> Option<crate::skill::config::SkillConfigSnapshot> {
            if let Ok(payload) = serde_json::from_slice::<ServerDataV1>(bytes) {
                if let ServerDataPayloadV1::SkillConfigSnapshot(snapshot) = payload.payload {
                    return Some(snapshot);
                }
            }
            let crate::schema::proto_gen::bong::server_data_envelope::Payload::SkillConfigSnapshot(
                data,
            ) = decode_proto_server_data_payload(bytes)?
            else {
                return None;
            };
            let configs = data
                .configs
                .into_iter()
                .map(|entry| {
                    let fields = serde_json::from_str::<
                        std::collections::BTreeMap<String, serde_json::Value>,
                    >(&entry.json_config)
                    .ok()?;
                    Some((
                        entry.skill_id,
                        crate::skill::config::SkillConfig::new(fields),
                    ))
                })
                .collect::<Option<std::collections::BTreeMap<_, _>>>()?;
            Some(crate::skill::config::SkillConfigSnapshot { configs })
        }

        fn decode_quickslot_config_payload(
            bytes: &[u8],
        ) -> Option<crate::schema::combat_hud::QuickSlotConfigV1> {
            if let Ok(payload) = serde_json::from_slice::<ServerDataV1>(bytes) {
                if let ServerDataPayloadV1::QuickSlotConfig(config) = payload.payload {
                    return Some(config);
                }
            }
            let crate::schema::proto_gen::bong::server_data_envelope::Payload::QuickSlotConfig(
                data,
            ) = decode_proto_server_data_payload(bytes)?
            else {
                return None;
            };
            Some(crate::schema::combat_hud::QuickSlotConfigV1 {
                slots: data
                    .slots
                    .into_iter()
                    .map(|slot| {
                        slot.entry
                            .map(|entry| crate::schema::combat_hud::QuickSlotEntryV1 {
                                item_id: entry.item_id,
                                display_name: entry.display_name,
                                cast_duration_ms: entry.cast_duration_ms,
                                cooldown_ms: entry.cooldown_ms,
                                icon_texture: entry.icon_texture,
                            })
                    })
                    .collect(),
                cooldown_until_ms: data.cooldown_until_ms,
                ack_request_id: data.ack_request_id,
                bind_accepted: data.bind_accepted,
            })
        }

        fn decode_forge_blueprint_book_payload(
            bytes: &[u8],
        ) -> Option<crate::schema::forge::ForgeBlueprintBookDataV1> {
            if let Ok(payload) = serde_json::from_slice::<ServerDataV1>(bytes) {
                if let ServerDataPayloadV1::ForgeBlueprintBook(data) = payload.payload {
                    return Some(*data);
                }
            }
            let crate::schema::proto_gen::bong::server_data_envelope::Payload::ForgeBlueprintBook(
                data,
            ) = decode_proto_server_data_payload(bytes)?
            else {
                return None;
            };
            Some(crate::schema::forge::ForgeBlueprintBookDataV1 {
                learned: data
                    .learned
                    .into_iter()
                    .map(|entry| {
                        Some(crate::schema::forge::ForgeBlueprintEntryV1 {
                            id: entry.id,
                            display_name: entry.display_name,
                            tier_cap: u8::try_from(entry.tier_cap).ok()?,
                            step_count: entry.step_count,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
                current_index: data.current_index,
            })
        }

        fn decode_cast_sync_payload(bytes: &[u8]) -> Option<crate::schema::combat_hud::CastSyncV1> {
            if let Ok(payload) = serde_json::from_slice::<ServerDataV1>(bytes) {
                if let ServerDataPayloadV1::CastSync(sync) = payload.payload {
                    return Some(sync);
                }
            }
            let crate::schema::proto_gen::bong::server_data_envelope::Payload::CastSync(data) =
                decode_proto_server_data_payload(bytes)?
            else {
                return None;
            };
            let phase = match data.phase {
                1 => crate::schema::combat_hud::CastPhaseV1::Idle,
                2 => crate::schema::combat_hud::CastPhaseV1::Casting,
                3 => crate::schema::combat_hud::CastPhaseV1::Complete,
                4 => crate::schema::combat_hud::CastPhaseV1::Interrupt,
                _ => return None,
            };
            let outcome = match data.outcome {
                1 => crate::schema::combat_hud::CastOutcomeV1::None,
                2 => crate::schema::combat_hud::CastOutcomeV1::Completed,
                3 => crate::schema::combat_hud::CastOutcomeV1::InterruptMovement,
                4 => crate::schema::combat_hud::CastOutcomeV1::InterruptContam,
                5 => crate::schema::combat_hud::CastOutcomeV1::InterruptControl,
                6 => crate::schema::combat_hud::CastOutcomeV1::UserCancel,
                7 => crate::schema::combat_hud::CastOutcomeV1::Death,
                8 => crate::schema::combat_hud::CastOutcomeV1::MeridianGated,
                9 => crate::schema::combat_hud::CastOutcomeV1::RejectQiInsufficient,
                10 => crate::schema::combat_hud::CastOutcomeV1::RejectOnCooldown,
                11 => crate::schema::combat_hud::CastOutcomeV1::RejectInvalidTarget,
                12 => crate::schema::combat_hud::CastOutcomeV1::RejectInRecovery,
                13 => crate::schema::combat_hud::CastOutcomeV1::RejectRealmTooLow,
                14 => crate::schema::combat_hud::CastOutcomeV1::RejectNoWeapon,
                15 => crate::schema::combat_hud::CastOutcomeV1::RejectTechniqueInactive,
                16 => crate::schema::combat_hud::CastOutcomeV1::RejectRaceMismatch,
                _ => return None,
            };
            Some(crate::schema::combat_hud::CastSyncV1 {
                phase,
                slot: u8::try_from(data.slot).ok()?,
                duration_ms: data.duration_ms,
                started_at_ms: data.started_at_ms,
                outcome,
            })
        }

        fn inventory_event_is_durability_changed(bytes: &[u8], instance_id: u64) -> bool {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) {
                return value.get("type").and_then(serde_json::Value::as_str)
                    == Some("inventory_event")
                    && value.get("kind").and_then(serde_json::Value::as_str)
                        == Some("durability_changed")
                    && value.get("instance_id").and_then(serde_json::Value::as_u64)
                        == Some(instance_id);
            }
            let Some(
                crate::schema::proto_gen::bong::server_data_envelope::Payload::InventoryEvent(data),
            ) = decode_proto_server_data_payload(bytes)
            else {
                return false;
            };
            matches!(
                data.event,
                Some(crate::schema::proto_gen::bong::inventory_event::Event::DurabilityChanged(
                    event
                )) if event.instance_id == instance_id
            )
        }

        fn decode_inventory_move_rejected_payload(
            bytes: &[u8],
        ) -> Option<crate::schema::server_data::InventoryMoveRejectedV1> {
            if let Ok(payload) = serde_json::from_slice::<ServerDataV1>(bytes) {
                if let ServerDataPayloadV1::InventoryMoveRejected(data) = payload.payload {
                    return Some(data);
                }
            }
            let crate::schema::proto_gen::bong::server_data_envelope::Payload::InventoryMoveRejected(
            data,
        ) = decode_proto_server_data_payload(bytes)?
        else {
            return None;
        };
            Some(crate::schema::server_data::InventoryMoveRejectedV1 {
                reason: data.reason,
                required_realm: data.required_realm,
                slot: data.slot,
                cap: data.cap,
            })
        }

        fn collect_server_data_payload_types(helper: &mut MockClientHelper) -> Vec<String> {
            helper
                .collect_received()
                .0
                .into_iter()
                .filter_map(|frame| {
                    let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                    if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                        return None;
                    }
                    server_data_payload_type(packet.data.0 .0)
                })
                .collect()
        }

        fn run_supply_coffin_open_payload_case(
            player_pos: DVec3,
        ) -> (App, Entity, u64, Vec<String>) {
            use crate::inventory::external_container::{
                ExternalContainer, ExternalContainerRegistry,
            };
            use crate::supply_coffin::interact::{
                handle_supply_coffin_interact, SupplyCoffinOpenRequest, SupplyCoffinOpened,
            };
            use crate::supply_coffin::{SupplyCoffinGrade, SupplyCoffinRegistry};

            let mut app = App::new();
            app.add_plugins(EntityPlugin);
            register_request_app(&mut app);
            app.add_event::<SupplyCoffinOpenRequest>();
            app.add_event::<SupplyCoffinOpened>();
            app.insert_resource(ExternalContainerRegistry::default());
            app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());
            app.insert_resource(crate::inventory::load_item_registry().expect("item registry"));
            app.add_systems(
                Update,
                handle_supply_coffin_interact.after(handle_client_request_payloads),
            );

            let target = app
                .world_mut()
                .spawn((
                    crate::world::entity_model::COFFIN_COMMON_ENTITY_KIND,
                    EntityId::default(),
                    Position::new(DVec3::new(0.0, 64.0, 0.0)),
                    OldPosition::new(DVec3::new(0.0, 64.0, 0.0)),
                ))
                .id();
            let mut registry = SupplyCoffinRegistry::new(
                (DVec3::ZERO, DVec3::new(100.0, 100.0, 100.0)),
                65.0,
                0x2468,
            );
            registry.insert_active(
                target,
                SupplyCoffinGrade::Common,
                DVec3::new(0.0, 64.0, 0.0),
                crate::supply_coffin::current_wall_clock_secs(),
            );
            let rng_before = registry.rng_state;
            app.insert_resource(registry);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let player = app
                .world_mut()
                .spawn((
                    client_bundle,
                    empty_inventory(),
                    Cultivation::default(),
                    PlayerState::default(),
                    CurrentDimension(DimensionKind::Overworld),
                ))
                .id();
            app.world_mut()
                .entity_mut(player)
                .insert(Position::new(player_pos));

            app.update();
            let entity_id = app
                .world()
                .get::<EntityId>(target)
                .expect("EntityPlugin must assign the supply-coffin protocol id")
                .get();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: player,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SupplyCoffinOpen {
                        v: 1,
                        entity_id,
                    })
                    .expect("supply_coffin_open request should serialize")
                    .into_boxed_slice(),
                });

            app.update();
            flush_all_client_packets(&mut app);
            let payload_types = collect_server_data_payload_types(&mut helper);
            let opened = app.world().get::<ExternalContainer>(target).is_some();
            if opened {
                assert!(
                    app.world()
                        .resource::<ExternalContainerRegistry>()
                        .sessions
                        .values()
                        .any(|entity| *entity == target),
                    "successful C2S open must register the target session"
                );
            }
            (app, target, rng_before, payload_types)
        }

        #[test]
        fn supply_coffin_open_payload_wiring_accepts_finite_and_rejects_non_finite_coordinates() {
            let (finite, target, _rng_before, payload_types) =
                run_supply_coffin_open_payload_case(DVec3::new(0.0, 64.0, 0.0));
            assert!(
                finite
                    .world()
                    .get::<crate::inventory::external_container::ExternalContainer>(target)
                    .is_some(),
                "real supply_coffin_open C2S payload must reach the interact consumer"
            );
            assert!(
                payload_types.iter().any(|ty| ty == "loot_container_open"),
                "successful C2S open must emit loot_container_open S2C; payloads={payload_types:?}"
            );

            for (label, x) in [
                ("nan", f64::NAN),
                ("positive_infinity", f64::INFINITY),
                ("negative_infinity", f64::NEG_INFINITY),
            ] {
                let (app, target, rng_before, payload_types) =
                    run_supply_coffin_open_payload_case(DVec3::new(x, 64.0, 0.0));
                assert!(
                    app.world()
                        .get::<crate::inventory::external_container::ExternalContainer>(target)
                        .is_none(),
                    "{label} C2S open must not create a session container"
                );
                assert!(
                app.world()
                    .resource::<crate::inventory::external_container::ExternalContainerRegistry>()
                    .sessions
                    .is_empty(),
                "{label} C2S open must not allocate a session"
            );
                assert_eq!(
                    app.world()
                        .resource::<crate::supply_coffin::SupplyCoffinRegistry>()
                        .rng_state,
                    rng_before,
                    "{label} C2S open must reject before RNG advances"
                );
                assert!(
                payload_types.iter().all(|ty| ty != "loot_container_open"),
                "{label} C2S open must not emit loot_container_open; payloads={payload_types:?}"
            );
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn run_external_container_move_case(
            player_dimension: Option<DimensionKind>,
            player_pos: DVec3,
            source_kind: crate::inventory::external_container::ExternalContainerKind,
            source_active: bool,
            session_registered: bool,
            owner_is_player: bool,
        ) -> (App, Entity, Entity, Vec<String>) {
            run_external_container_move_case_with_source(
                player_dimension,
                player_pos,
                source_kind,
                source_active,
                session_registered,
                owner_is_player,
                0,
                0,
                1,
            )
        }

        #[allow(clippy::too_many_arguments)]
        fn run_external_container_move_case_with_source(
            player_dimension: Option<DimensionKind>,
            player_pos: DVec3,
            source_kind: crate::inventory::external_container::ExternalContainerKind,
            source_active: bool,
            session_registered: bool,
            owner_is_player: bool,
            source_row: u64,
            source_col: u64,
            denial_count: usize,
        ) -> (App, Entity, Entity, Vec<String>) {
            use crate::inventory::external_container::{
                ExternalContainer, ExternalContainerRegistry,
            };
            use crate::supply_coffin::{SupplyCoffinGrade, SupplyCoffinRegistry};

            const SESSION_ID: u64 = 77;
            const INSTANCE_ID: u64 = 7001;
            const COFFIN_POS: DVec3 = DVec3::new(0.0, 64.0, 0.0);

            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let player = app
                .world_mut()
                .spawn((
                    client_bundle,
                    empty_inventory(),
                    Cultivation::default(),
                    PlayerState::default(),
                    Lifecycle::default(),
                ))
                .id();
            app.world_mut()
                .entity_mut(player)
                .insert(Position::new(player_pos));
            if let Some(dimension) = player_dimension {
                app.world_mut()
                    .entity_mut(player)
                    .insert(CurrentDimension(dimension));
            }

            let owner = if owner_is_player {
                player
            } else {
                app.world_mut().spawn_empty().id()
            };
            let coffin = app
                .world_mut()
                .spawn((
                    ExternalContainer {
                        session_id: SESSION_ID,
                        container: ContainerState {
                            id: ExternalContainer::container_id(SESSION_ID),
                            name: "external_test".to_string(),
                            rows: 3,
                            cols: 4,
                            items: vec![PlacedItemState {
                                row: 0,
                                col: 0,
                                instance: inventory_test_item(INSTANCE_ID, "spiritual_ore", 1),
                            }],
                            owner_instance_id: None,
                            quick_access: false,
                        },
                        opened_by: Some(owner),
                        timeout_wall_secs: u64::MAX,
                        source_kind,
                    },
                    Position::new(COFFIN_POS),
                    CurrentDimension(DimensionKind::Overworld),
                ))
                .id();

            let mut ext_registry = ExternalContainerRegistry {
                next_session_id: SESSION_ID + 1,
                ..Default::default()
            };
            if session_registered {
                ext_registry.sessions.insert(SESSION_ID, coffin);
            }
            app.insert_resource(ext_registry);

            let mut coffin_registry = SupplyCoffinRegistry::new(
                (DVec3::ZERO, DVec3::new(100.0, 100.0, 100.0)),
                65.0,
                0x9876,
            );
            if source_active {
                coffin_registry.insert_active(
                    coffin,
                    SupplyCoffinGrade::Common,
                    COFFIN_POS,
                    crate::supply_coffin::current_wall_clock_secs(),
                );
            }
            app.insert_resource(coffin_registry);

            for _ in 0..denial_count {
                app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: player,
                    channel: ident!("bong:client_request").into(),
                    data: format!(
                        r#"{{"type":"external_container_move","v":1,"session_id":{SESSION_ID},"instance_id":{INSTANCE_ID},"from":{{"kind":"container","container_id":"ext_{SESSION_ID}","row":{source_row},"col":{source_col}}},"to":{{"kind":"container","container_id":"main_pack","row":0,"col":0}}}}"#
                    )
                    .into_bytes()
                    .into_boxed_slice(),
                });
            }

            app.update();
            flush_all_client_packets(&mut app);
            let payload_types = collect_server_data_payload_types(&mut helper);
            (app, player, coffin, payload_types)
        }

        fn assert_external_move_rejected_without_mutation(
            app: &App,
            player: Entity,
            coffin: Entity,
        ) {
            let ext = app
                .world()
                .get::<crate::inventory::external_container::ExternalContainer>(coffin)
                .expect("external container must remain attached after rejection");
            assert!(
            ext.container
                .items
                .iter()
                .any(|item| item.instance.instance_id == 7001),
            "rejected move must keep instance 7001 in the external container; actual items={:?}",
            ext.container
                .items
                .iter()
                .map(|item| item.instance.instance_id)
                .collect::<Vec<_>>()
        );
            let inventory = app
                .world()
                .get::<PlayerInventory>(player)
                .expect("test player keeps inventory component");
            assert!(
                inventory.containers.iter().all(|container| container
                    .items
                    .iter()
                    .all(|item| item.instance.instance_id != 7001)),
                "rejected move must not copy instance 7001 into player inventory"
            );
            assert_eq!(
                inventory.revision,
                InventoryRevision(0),
                "rejected move must not advance inventory revision"
            );
        }

        #[test]
        fn supply_coffin_external_move_real_c2s_rejects_cross_dimension_same_xyz_while_session_is_valid_and_resyncs(
        ) {
            let (app, player, coffin, payload_types) = run_external_container_move_case(
                Some(DimensionKind::Tsy),
                DVec3::new(0.0, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                    grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                },
                true,
                true,
                true,
            );

            assert_external_move_rejected_without_mutation(&app, player, coffin);
            assert_eq!(
                app.world()
                    .resource::<crate::inventory::external_container::ExternalContainerRegistry>()
                    .sessions
                    .get(&77),
                Some(&coffin),
                "real C2S move must reach dimension authority while session mapping is still valid"
            );
            assert_eq!(
                app.world()
                    .get::<crate::inventory::external_container::ExternalContainer>(coffin)
                    .expect("supply coffin session must remain attached")
                    .opened_by,
                Some(player),
                "real C2S move must be rejected while opened_by still proves requester ownership"
            );
            assert!(
            app.world()
                .resource::<crate::supply_coffin::SupplyCoffinRegistry>()
                .active
                .contains_key(&coffin),
            "real C2S move must be rejected while authoritative supply-coffin source is still active"
        );
            assert!(
            payload_types.iter().any(|ty| ty == "event_alert"),
            "live gate rejection must use the bounded feedback path; payloads={payload_types:?}"
        );
            assert!(
            payload_types.iter().any(|ty| ty == "loot_container_update")
                && payload_types.iter().any(|ty| ty == "inventory_snapshot"),
            "a gate rejection must keep the existing read-only external/inventory resync contract without entering the mutation handler; payloads={payload_types:?}"
        );
        }

        #[test]
        fn supply_coffin_external_move_rejects_missing_dimension() {
            let (app, player, coffin, _payload_types) = run_external_container_move_case(
                None,
                DVec3::new(0.0, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                    grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                },
                true,
                true,
                true,
            );

            assert_external_move_rejected_without_mutation(&app, player, coffin);
        }

        #[test]
        fn supply_coffin_external_move_rejects_non_finite_coordinates_and_resyncs() {
            for (label, x) in [
                ("nan", f64::NAN),
                ("positive_infinity", f64::INFINITY),
                ("negative_infinity", f64::NEG_INFINITY),
            ] {
                let (app, player, coffin, payload_types) = run_external_container_move_case(
                    Some(DimensionKind::Overworld),
                    DVec3::new(x, 64.0, 0.0),
                    crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                        grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                    },
                    true,
                    true,
                    true,
                );

                assert_external_move_rejected_without_mutation(&app, player, coffin);
                assert!(
                payload_types.iter().any(|ty| ty == "event_alert"),
                "{label} live gate rejection must emit bounded feedback; payloads={payload_types:?}"
            );
            }
        }

        #[test]
        fn supply_coffin_external_move_rejects_out_of_lifecycle_range() {
            let (app, player, coffin, _payload_types) = run_external_container_move_case(
                Some(DimensionKind::Overworld),
                DVec3::new(6.501, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                    grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                },
                true,
                true,
                true,
            );

            assert_external_move_rejected_without_mutation(&app, player, coffin);
        }

        #[test]
        fn supply_coffin_external_move_rejects_when_active_source_disappears() {
            let (app, player, coffin, _payload_types) = run_external_container_move_case(
                Some(DimensionKind::Overworld),
                DVec3::new(0.0, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                    grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                },
                false,
                true,
                true,
            );

            assert_external_move_rejected_without_mutation(&app, player, coffin);
        }

        #[test]
        fn external_move_owner_mismatch_keeps_items_and_resyncs_requester_inventory() {
            let (app, player, coffin, payload_types) = run_external_container_move_case(
                Some(DimensionKind::Overworld),
                DVec3::new(0.0, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                    grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                },
                true,
                true,
                false,
            );

            assert_external_move_rejected_without_mutation(&app, player, coffin);
            assert!(
            payload_types.iter().any(|ty| ty == "event_alert"),
            "non-owner live gate rejection must emit bounded feedback; payloads={payload_types:?}"
        );
            assert!(
            payload_types.iter().any(|ty| ty == "inventory_snapshot")
                && payload_types.iter().all(|ty| ty != "loot_container_update"),
            "non-owner rejection may resync only the requester's inventory and must not expose external contents; payloads={payload_types:?}"
        );
        }

        #[test]
        fn external_move_non_owner_cross_dimension_does_not_disclose_container() {
            let (app, player, coffin, payload_types) = run_external_container_move_case(
                Some(DimensionKind::Tsy),
                DVec3::new(0.0, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                    grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                },
                true,
                true,
                false,
            );

            assert_external_move_rejected_without_mutation(&app, player, coffin);
            assert!(
            payload_types.iter().any(|ty| ty == "event_alert"),
            "non-owner cross-dimension rejection must emit bounded feedback; payloads={payload_types:?}"
        );
            assert!(
            payload_types.iter().any(|ty| ty == "inventory_snapshot")
                && payload_types.iter().all(|ty| ty != "loot_container_update"),
            "non-owner rejection must not disclose external contents even when dimension gate rejects first; payloads={payload_types:?}"
        );
        }

        #[test]
        fn external_move_stale_session_resyncs_inventory_without_mutation() {
            let (app, player, coffin, payload_types) = run_external_container_move_case(
                Some(DimensionKind::Overworld),
                DVec3::new(0.0, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                    grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                },
                true,
                false,
                true,
            );

            assert_external_move_rejected_without_mutation(&app, player, coffin);
            assert!(
                payload_types.iter().any(|ty| ty == "event_alert"),
                "unknown/stale session must use bounded gate feedback; payloads={payload_types:?}"
            );
        }

        #[test]
        fn external_move_stale_session_resyncs_even_when_feedback_budget_suppresses_alert() {
            let (app, player, coffin, payload_types) = run_external_container_move_case_with_source(
                Some(DimensionKind::Overworld),
                DVec3::new(0.0, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                    grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                },
                true,
                false,
                true,
                0,
                0,
                2,
            );

            assert_external_move_rejected_without_mutation(&app, player, coffin);
            assert_eq!(
            payload_types
                .iter()
                .filter(|payload_type| payload_type.as_str() == "event_alert")
                .count(),
            1,
            "feedback budget must suppress the second duplicate alert while preserving the first"
        );
            assert_eq!(
            payload_types
                .iter()
                .filter(|payload_type| payload_type.as_str() == "inventory_snapshot")
                .count(),
            2,
            "each stale-session rejection must still resync the authoritative player inventory even when alert feedback is suppressed"
        );
        }

        #[test]
        fn supply_coffin_external_move_accepts_exact_lifecycle_boundary() {
            let (app, player, coffin, payload_types) = run_external_container_move_case(
                Some(DimensionKind::Overworld),
                DVec3::new(6.5, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                    grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                },
                true,
                true,
                true,
            );

            let ext = app
                .world()
                .get::<crate::inventory::external_container::ExternalContainer>(coffin)
                .expect("coffin remains after successful move");
            assert!(
                ext.container.items.is_empty(),
                "authorized boundary move must remove the item from the external container"
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert!(
                inventory.containers.iter().any(|container| container
                    .items
                    .iter()
                    .any(|item| item.instance.instance_id == 7001)),
                "authorized boundary move must place instance 7001 into player inventory"
            );
            assert!(
            payload_types.iter().any(|ty| ty == "loot_container_update")
                && payload_types.iter().any(|ty| ty == "inventory_snapshot"),
            "successful move must keep existing update + inventory snapshot contract; payloads={payload_types:?}"
        );
        }

        #[test]
        fn external_move_rejects_forged_external_source_coordinates_without_mutation() {
            let (app, player, coffin, payload_types) = run_external_container_move_case_with_source(
                Some(DimensionKind::Overworld),
                DVec3::new(0.0, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::StorageCrate {
                    is_herb: false,
                },
                false,
                true,
                true,
                0,
                1,
                1,
            );

            assert_external_move_rejected_without_mutation(&app, player, coffin);
            assert!(
            payload_types.iter().any(|ty| ty == "loot_container_update"),
            "authorized owner with forged source coordinates must receive authoritative external resync; payloads={payload_types:?}"
        );
            assert!(
                payload_types.iter().any(|ty| ty == "inventory_snapshot"),
                "forged source rejection must resync player inventory; payloads={payload_types:?}"
            );
        }

        #[test]
        fn external_move_rejects_forged_player_source_container_and_coordinates_without_mutation() {
            for (label, source_container_id, source_row, source_col) in [
                ("container", "body_pocket", 0, 0),
                ("row", "main_pack", 1, 0),
                ("column", "main_pack", 0, 1),
            ] {
                let (app, player, coffin, payload_types) =
                    run_player_to_external_move_case_with_source(
                        source_container_id,
                        source_row,
                        source_col,
                    );
                let inventory = app
                    .world()
                    .get::<PlayerInventory>(player)
                    .expect("test player keeps inventory component");
                assert_eq!(
                    inventory.revision,
                    InventoryRevision(0),
                    "forged player {label} source must not advance inventory revision"
                );
                assert!(
                inventory.containers.iter().any(|container| {
                    container.id == "main_pack"
                        && container.items.iter().any(|item| {
                            item.instance.instance_id == 7001 && item.row == 0 && item.col == 0
                        })
                }),
                "forged player {label} source must keep instance 7001 at its authoritative slot"
            );
                let ext = app
                    .world()
                    .get::<crate::inventory::external_container::ExternalContainer>(coffin)
                    .expect("external container must remain attached after rejection");
                assert!(
                ext.container.items.is_empty(),
                "forged player {label} source must not move instance 7001 into external storage"
            );
                assert!(
                payload_types.iter().any(|ty| ty == "loot_container_update"),
                "forged player {label} source must resync external state; payloads={payload_types:?}"
            );
                assert!(
                payload_types.iter().any(|ty| ty == "inventory_snapshot"),
                "forged player {label} source must resync player state; payloads={payload_types:?}"
            );
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn run_player_to_external_move_case_with_source(
            source_container_id: &str,
            source_row: u64,
            source_col: u64,
        ) -> (App, Entity, Entity, Vec<String>) {
            use crate::inventory::external_container::{
                ExternalContainer, ExternalContainerRegistry,
            };

            const SESSION_ID: u64 = 77;
            const INSTANCE_ID: u64 = 7001;

            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut inventory = empty_inventory();
            inventory.containers[0].items.push(PlacedItemState {
                row: 0,
                col: 0,
                instance: inventory_test_item(INSTANCE_ID, "spiritual_ore", 1),
            });
            let player = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory,
                    Cultivation::default(),
                    PlayerState::default(),
                    Lifecycle::default(),
                    CurrentDimension(DimensionKind::Overworld),
                ))
                .id();
            app.world_mut()
                .entity_mut(player)
                .insert(Position::new(DVec3::ZERO));
            let coffin = app
            .world_mut()
            .spawn((
                ExternalContainer {
                    session_id: SESSION_ID,
                    container: ContainerState {
                        id: ExternalContainer::container_id(SESSION_ID),
                        name: "external_test".to_string(),
                        rows: 3,
                        cols: 4,
                        items: Vec::new(),
                        owner_instance_id: None,
                        quick_access: false,
                    },
                    opened_by: Some(player),
                    timeout_wall_secs: u64::MAX,
                    source_kind:
                        crate::inventory::external_container::ExternalContainerKind::StorageCrate {
                            is_herb: false,
                        },
                },
                Position::new(DVec3::ZERO),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
            app.insert_resource(ExternalContainerRegistry {
                next_session_id: SESSION_ID + 1,
                sessions: [(SESSION_ID, coffin)].into_iter().collect(),
            });
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: player,
                channel: ident!("bong:client_request").into(),
                data: format!(
                    r#"{{"type":"external_container_move","v":1,"session_id":{SESSION_ID},"instance_id":{INSTANCE_ID},"from":{{"kind":"container","container_id":"{source_container_id}","row":{source_row},"col":{source_col}}},"to":{{"kind":"container","container_id":"ext_{SESSION_ID}","row":0,"col":0}}}}"#
                )
                .into_bytes()
                .into_boxed_slice(),
            });
            app.update();
            flush_all_client_packets(&mut app);
            let payload_types = collect_server_data_payload_types(&mut helper);
            (app, player, coffin, payload_types)
        }

        #[test]
        fn non_supply_external_container_move_keeps_existing_contract_after_live_gate() {
            let (app, player, coffin, _payload_types) = run_external_container_move_case(
                Some(DimensionKind::Overworld),
                DVec3::new(0.0, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::StorageCrate {
                    is_herb: false,
                },
                false,
                true,
                true,
            );

            let ext = app
                .world()
                .get::<crate::inventory::external_container::ExternalContainer>(coffin)
                .expect("storage crate remains after move");
            assert!(
                ext.container.items.is_empty(),
                "supply-coffin authority rules must not spill into storage-crate move handling"
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert!(
                inventory.containers.iter().any(|container| container
                    .items
                    .iter()
                    .any(|item| item.instance.instance_id == 7001)),
                "storage-crate move contract must remain unchanged"
            );
        }

        fn inventory_with_item(item: ItemInstance) -> PlayerInventory {
            PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(0),
                containers: vec![ContainerState {
                    quick_access: false,
                    id: "main_pack".into(),
                    name: "main_pack".into(),
                    rows: 5,
                    cols: 7,
                    items: vec![PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: item,
                    }],

                    owner_instance_id: None,
                }],
                equipped: Default::default(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            }
        }

        fn flush_all_client_packets(app: &mut App) {
            let world = app.world_mut();
            let mut query = world.query::<&mut Client>();
            for mut client in query.iter_mut(world) {
                client
                    .flush_packets()
                    .expect("mock client packets should flush successfully");
            }
        }

        fn has_inventory_snapshot_payload(helper: &mut MockClientHelper) -> bool {
            for frame in helper.collect_received().0 {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    continue;
                };
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    continue;
                }
                if server_data_payload_type(packet.data.0 .0)
                    == Some("inventory_snapshot".to_owned())
                {
                    return true;
                }
            }
            false
        }

        fn collect_alchemy_session_snapshots(
            helper: &mut MockClientHelper,
        ) -> Vec<crate::schema::alchemy::AlchemySessionDataV1> {
            helper
                .collect_received()
                .0
                .into_iter()
                .filter_map(|frame| {
                    let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                    if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                        return None;
                    }
                    decode_alchemy_session_payload(packet.data.0 .0)
                })
                .collect()
        }

        const ALCHEMY_SNAPSHOT_RECIPE_ID: &str = "handler_snapshot_contract";
        const ALCHEMY_SNAPSHOT_FURNACE_POS: (i32, i32, i32) = (2, 64, 3);
        const ALCHEMY_SNAPSHOT_MATERIAL: &str = "handler_snapshot_herb";

        fn alchemy_snapshot_recipe_registry() -> RecipeRegistry {
            let mut registry = RecipeRegistry::new();
            registry
                .insert(Recipe {
                    id: ALCHEMY_SNAPSHOT_RECIPE_ID.into(),
                    name: "handler snapshot contract".into(),
                    furnace_tier_min: 1,
                    stages: vec![
                        RecipeStage {
                            at_tick: 0,
                            required: vec![IngredientSpec {
                                material: ALCHEMY_SNAPSHOT_MATERIAL.into(),
                                count: 2,
                                mineral_id: None,
                            }],
                            window: 0,
                        },
                        RecipeStage {
                            at_tick: 12,
                            required: vec![],
                            window: 3,
                        },
                    ],
                    fire_profile: FireProfile {
                        target_temp: 0.67,
                        target_duration_ticks: 48,
                        qi_cost: 9.75,
                        tolerance: ToleranceSpec {
                            temp_band: 0.07,
                            duration_band: 5,
                        },
                    },
                    outcomes: Outcomes {
                        perfect: None,
                        good: None,
                        flawed: None,
                        waste: None,
                        explode: None,
                    },
                    flawed_fallback: None,
                })
                .expect("handler snapshot recipe fixture must have a unique id");
            registry
        }

        fn alchemy_snapshot_active_session(player_id: &str) -> AlchemySession {
            let mut session =
                AlchemySession::new(ALCHEMY_SNAPSHOT_RECIPE_ID.into(), player_id.to_string());
            session.temp_current = 0.61;
            session.qi_injected = 4.25;
            session
        }

        fn spawn_owned_alchemy_snapshot_furnace(
            app: &mut App,
            player_id: &str,
            session: Option<AlchemySession>,
        ) -> Entity {
            let mut furnace = AlchemyFurnace::placed(
                valence::prelude::BlockPos::new(
                    ALCHEMY_SNAPSHOT_FURNACE_POS.0,
                    ALCHEMY_SNAPSHOT_FURNACE_POS.1,
                    ALCHEMY_SNAPSHOT_FURNACE_POS.2,
                ),
                1,
            );
            furnace.owner = Some(player_id.to_string());
            furnace.session = session;
            app.world_mut().spawn(furnace).id()
        }

        fn send_alchemy_snapshot_request(app: &mut App, client: Entity, body: serde_json::Value) {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: body.to_string().into_bytes().into_boxed_slice(),
                });
        }

        fn run_alchemy_snapshot_request(
            app: &mut App,
            client: Entity,
            helper: &mut MockClientHelper,
            body: serde_json::Value,
        ) -> Vec<crate::schema::alchemy::AlchemySessionDataV1> {
            send_alchemy_snapshot_request(app, client, body);
            app.update();
            flush_all_client_packets(app);
            collect_alchemy_session_snapshots(helper)
        }

        fn assert_authoritative_alchemy_guidance(
            snapshot: &crate::schema::alchemy::AlchemySessionDataV1,
            stage_states: [(bool, bool); 2],
        ) {
            assert_eq!(
                snapshot.recipe_id.as_deref(),
                Some(ALCHEMY_SNAPSHOT_RECIPE_ID),
                "handler payload must identify the same recipe fixture that supplies its targets"
            );
            assert_eq!(
                snapshot.target_ticks, 48,
                "target duration must come from the authoritative RecipeRegistry fixture"
            );
            assert_eq!(
                snapshot.temp_target, 0.67,
                "target temperature must come from the authoritative RecipeRegistry fixture"
            );
            assert_eq!(
                snapshot.temp_band, 0.07,
                "temperature band must come from the authoritative RecipeRegistry fixture"
            );
            assert_eq!(
                snapshot.qi_target, 9.75,
                "qi target must come from the authoritative RecipeRegistry fixture"
            );
            assert_eq!(
            snapshot.stages,
            vec![
                crate::schema::alchemy::AlchemyStageHintV1 {
                    at_tick: 0,
                    window: 0,
                    summary: format!("{ALCHEMY_SNAPSHOT_MATERIAL}×2"),
                    completed: stage_states[0].0,
                    missed: stage_states[0].1,
                },
                crate::schema::alchemy::AlchemyStageHintV1 {
                    at_tick: 12,
                    window: 3,
                    summary: String::new(),
                    completed: stage_states[1].0,
                    missed: stage_states[1].1,
                },
            ],
            "handler payload must preserve declared stage order and an exact empty summary for required=[]"
        );
        }

        #[test]
        fn alchemy_open_furnace_repushes_authoritative_recipe_snapshot_over_wire() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(alchemy_snapshot_recipe_registry());
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let client = app.world_mut().spawn(client_bundle).id();
            spawn_owned_alchemy_snapshot_furnace(
                &mut app,
                "offline:Azure",
                Some(alchemy_snapshot_active_session("offline:Azure")),
            );

            let snapshots = run_alchemy_snapshot_request(
                &mut app,
                client,
                &mut helper,
                serde_json::json!({
                    "type": "alchemy_open_furnace",
                    "v": 1,
                    "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                }),
            );

            assert_eq!(snapshots.len(), 1, "open must emit one session snapshot");
            assert!(
                snapshots[0].active,
                "open must expose the active furnace session"
            );
            assert_authoritative_alchemy_guidance(&snapshots[0], [(false, false), (false, false)]);
        }

        #[test]
        fn alchemy_ignite_repushes_authoritative_recipe_snapshot_over_wire() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(alchemy_snapshot_recipe_registry());
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let client = app.world_mut().spawn(client_bundle).id();
            spawn_owned_alchemy_snapshot_furnace(&mut app, "offline:Azure", None);

            let snapshots = run_alchemy_snapshot_request(
                &mut app,
                client,
                &mut helper,
                serde_json::json!({
                    "type": "alchemy_ignite",
                    "v": 1,
                    "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                    "recipe_id": ALCHEMY_SNAPSHOT_RECIPE_ID,
                }),
            );

            assert_eq!(snapshots.len(), 1, "ignite must emit one session snapshot");
            assert!(
                snapshots[0].active,
                "ignite must expose its newly active session"
            );
            assert_eq!(snapshots[0].elapsed_ticks, 0);
            assert_authoritative_alchemy_guidance(&snapshots[0], [(false, false), (false, false)]);
        }

        #[test]
        fn alchemy_intervention_repushes_authoritative_recipe_snapshot_over_wire() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(alchemy_snapshot_recipe_registry());
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let client = app.world_mut().spawn(client_bundle).id();
            spawn_owned_alchemy_snapshot_furnace(
                &mut app,
                "offline:Azure",
                Some(alchemy_snapshot_active_session("offline:Azure")),
            );

            let snapshots = run_alchemy_snapshot_request(
                &mut app,
                client,
                &mut helper,
                serde_json::json!({
                    "type": "alchemy_intervention",
                    "v": 1,
                    "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                    "intervention": {"kind": "adjust_temp", "temp": 0.73},
                }),
            );

            assert_eq!(
                snapshots.len(),
                1,
                "intervention must emit one session snapshot"
            );
            assert_eq!(snapshots[0].temp_current, 0.73);
            assert_eq!(
                snapshots[0].interventions_recent,
                vec!["§7AdjustTemp(0.73)"],
                "wire snapshot must expose the intervention applied by the production handler"
            );
            assert_authoritative_alchemy_guidance(&snapshots[0], [(false, false), (false, false)]);
        }

        #[test]
        fn alchemy_feed_repushes_completed_stage_with_authoritative_recipe_snapshot_over_wire() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(alchemy_snapshot_recipe_registry());
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let client = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(client).insert((
                Cultivation::default(),
                PlayerState::default(),
                inventory_with_stack(ALCHEMY_SNAPSHOT_MATERIAL, 2),
            ));
            spawn_owned_alchemy_snapshot_furnace(
                &mut app,
                "offline:Azure",
                Some(alchemy_snapshot_active_session("offline:Azure")),
            );

            let snapshots = run_alchemy_snapshot_request(
                &mut app,
                client,
                &mut helper,
                serde_json::json!({
                    "type": "alchemy_feed_slot",
                    "v": 1,
                    "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                    "slot_idx": 0,
                    "material": ALCHEMY_SNAPSHOT_MATERIAL,
                    "count": 2,
                }),
            );

            assert_eq!(
                snapshots.len(),
                1,
                "successful feed must emit one session snapshot"
            );
            assert_authoritative_alchemy_guidance(&snapshots[0], [(true, false), (false, false)]);
        }

        #[test]
        fn alchemy_take_back_repushes_finished_guidance_after_furnace_session_is_removed() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(alchemy_snapshot_recipe_registry());
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                crate::alchemy::residue::FAILED_PILL_RESIDUE_TEMPLATE_ID.into(),
                ItemTemplate::minimal_for_test(
                    crate::alchemy::residue::FAILED_PILL_RESIDUE_TEMPLATE_ID,
                ),
            )])));
            app.insert_resource(InventoryInstanceIdAllocator::default());
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let client = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(client).insert((
                Cultivation::default(),
                PlayerState::default(),
                empty_inventory(),
            ));
            let mut session = alchemy_snapshot_active_session("offline:Azure");
            session.staged.completed_stages = vec![0, 1];
            session
                .staged
                .materials
                .insert(ALCHEMY_SNAPSHOT_MATERIAL.into(), 2);
            let furnace =
                spawn_owned_alchemy_snapshot_furnace(&mut app, "offline:Azure", Some(session));

            let snapshots = run_alchemy_snapshot_request(
                &mut app,
                client,
                &mut helper,
                serde_json::json!({
                    "type": "alchemy_take_back",
                    "v": 1,
                    "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                    "slot_idx": 0,
                }),
            );

            assert_eq!(
            snapshots.len(),
            1,
            "successful take-back must emit one finished session snapshot rather than an empty-furnace snapshot"
        );
            assert!(!snapshots[0].active, "finished snapshot must be inactive");
            assert_eq!(snapshots[0].status_label, "已结束");
            assert_authoritative_alchemy_guidance(&snapshots[0], [(true, false), (true, false)]);
            assert!(
            app.world()
                .get::<AlchemyFurnace>(furnace)
                .is_some_and(|furnace| furnace.session.is_none()),
            "take-back must keep the furnace empty after sending guidance from the completed session"
        );
        }

        #[test]
        fn alchemy_take_back_missing_allocator_still_pushes_finished_session() {
            let mut app = App::new();
            register_request_resources_without_lingtian(&mut app);
            register_request_systems(&mut app);
            // The shared fixture initializes the allocator for ordinary alchemy
            // paths; this contract specifically exercises the missing-resource
            // rejection branch.
            app.world_mut()
                .remove_resource::<InventoryInstanceIdAllocator>();
            app.insert_resource(alchemy_snapshot_recipe_registry());
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                crate::alchemy::residue::FAILED_PILL_RESIDUE_TEMPLATE_ID.into(),
                ItemTemplate::minimal_for_test(
                    crate::alchemy::residue::FAILED_PILL_RESIDUE_TEMPLATE_ID,
                ),
            )])));
            // 故意不插入 InventoryInstanceIdAllocator：覆盖 non-explode 缺编号器分支。
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let client = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(client).insert((
                Cultivation::default(),
                PlayerState::default(),
                empty_inventory(),
            ));
            let mut session = alchemy_snapshot_active_session("offline:Azure");
            session.staged.completed_stages = vec![0, 1];
            session
                .staged
                .materials
                .insert(ALCHEMY_SNAPSHOT_MATERIAL.into(), 2);
            let furnace =
                spawn_owned_alchemy_snapshot_furnace(&mut app, "offline:Azure", Some(session));

            send_alchemy_snapshot_request(
                &mut app,
                client,
                serde_json::json!({
                    "type": "alchemy_take_back",
                    "v": 1,
                    "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                    "slot_idx": 0,
                }),
            );
            app.update();
            flush_all_client_packets(&mut app);

            let frames = helper.collect_received().0;
            let messages: Vec<String> = frames
                .iter()
                .filter_map(|frame| {
                    frame
                        .decode::<GameMessageS2c>()
                        .ok()
                        .map(|packet| packet.chat.to_legacy_lossy())
                })
                .collect();
            let snapshots: Vec<crate::schema::alchemy::AlchemySessionDataV1> = frames
                .iter()
                .filter_map(|frame| {
                    let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                    if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                        return None;
                    }
                    decode_alchemy_session_payload(packet.data.0 .0)
                })
                .collect();
            let furnace_payloads = frames
                .iter()
                .filter_map(|frame| {
                    let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                    if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                        return None;
                    }
                    decode_alchemy_furnace_payload(packet.data.0 .0)
                })
                .collect::<Vec<_>>();

            assert!(
                messages
                    .iter()
                    .any(|message| message.contains("实例编号器未就绪")),
                "missing allocator must surface alchemy error chat, messages={messages:?}"
            );
            assert_eq!(
            snapshots.len(),
            1,
            "allocator missing must still emit exactly one finished session snapshot so HUD clears"
        );
            assert!(!snapshots[0].active, "finished snapshot must be inactive");
            assert_eq!(snapshots[0].status_label, "已结束");
            assert_authoritative_alchemy_guidance(&snapshots[0], [(true, false), (true, false)]);
            assert_eq!(
                furnace_payloads.len(),
                1,
                "allocator missing must still push empty-furnace authority once"
            );
            assert!(
                !furnace_payloads[0].has_session,
                "empty furnace payload must report has_session=false"
            );
            assert!(
                app.world()
                    .get::<AlchemyFurnace>(furnace)
                    .is_some_and(|furnace| furnace.session.is_none()),
                "session must remain ended even when reward grant is skipped"
            );
            assert!(
                app.world()
                    .get::<PlayerInventory>(client)
                    .is_some_and(|inventory| inventory
                        .containers
                        .iter()
                        .all(|container| container.items.is_empty())),
                "missing allocator must not invent reward items"
            );
            let outcome_events = app
                .world()
                .resource::<valence::prelude::Events<crate::alchemy::AlchemyOutcomeEvent>>();
            let mut reader = outcome_events.get_reader();
            assert!(
                reader.read(outcome_events).next().is_none(),
                "failed grant path must not emit AlchemyOutcomeEvent"
            );
        }

        #[test]
        fn alchemy_take_back_grant_failure_still_pushes_finished_session() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(alchemy_snapshot_recipe_registry());
            // 编号器就绪，但 registry 故意缺少 failed-pill 模板，强制 grant 失败。
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(InventoryInstanceIdAllocator::default());
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let client = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(client).insert((
                Cultivation::default(),
                PlayerState::default(),
                empty_inventory(),
            ));
            let mut session = alchemy_snapshot_active_session("offline:Azure");
            session.staged.completed_stages = vec![0, 1];
            session
                .staged
                .materials
                .insert(ALCHEMY_SNAPSHOT_MATERIAL.into(), 2);
            let furnace =
                spawn_owned_alchemy_snapshot_furnace(&mut app, "offline:Azure", Some(session));

            send_alchemy_snapshot_request(
                &mut app,
                client,
                serde_json::json!({
                    "type": "alchemy_take_back",
                    "v": 1,
                    "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                    "slot_idx": 0,
                }),
            );
            app.update();
            flush_all_client_packets(&mut app);

            let frames = helper.collect_received().0;
            let messages: Vec<String> = frames
                .iter()
                .filter_map(|frame| {
                    frame
                        .decode::<GameMessageS2c>()
                        .ok()
                        .map(|packet| packet.chat.to_legacy_lossy())
                })
                .collect();
            let snapshots: Vec<crate::schema::alchemy::AlchemySessionDataV1> = frames
                .iter()
                .filter_map(|frame| {
                    let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                    if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                        return None;
                    }
                    decode_alchemy_session_payload(packet.data.0 .0)
                })
                .collect();
            let furnace_payloads = frames
                .iter()
                .filter_map(|frame| {
                    let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                    if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                        return None;
                    }
                    decode_alchemy_furnace_payload(packet.data.0 .0)
                })
                .collect::<Vec<_>>();

            assert!(
                messages.iter().any(|message| {
                    message.contains("炼丹产物入袋失败")
                        && message
                            .contains(crate::alchemy::residue::FAILED_PILL_RESIDUE_TEMPLATE_ID)
                }),
                "grant failure must surface alchemy error chat, messages={messages:?}"
            );
            assert_eq!(
                snapshots.len(),
                1,
                "grant failure must still emit exactly one finished session snapshot"
            );
            assert!(!snapshots[0].active, "finished snapshot must be inactive");
            assert_eq!(snapshots[0].status_label, "已结束");
            assert_authoritative_alchemy_guidance(&snapshots[0], [(true, false), (true, false)]);
            assert_eq!(
                furnace_payloads.len(),
                1,
                "grant failure must still push empty-furnace authority once"
            );
            assert!(
                !furnace_payloads[0].has_session,
                "empty furnace payload must report has_session=false"
            );
            assert!(
                app.world()
                    .get::<AlchemyFurnace>(furnace)
                    .is_some_and(|furnace| furnace.session.is_none()),
                "grant failure must not resurrect the ended furnace session"
            );
            assert!(
                app.world()
                    .get::<PlayerInventory>(client)
                    .is_some_and(|inventory| inventory
                        .containers
                        .iter()
                        .all(|container| container.items.is_empty())),
                "failed grant must not leave partial reward items"
            );
            let outcome_events = app
                .world()
                .resource::<valence::prelude::Events<crate::alchemy::AlchemyOutcomeEvent>>();
            let mut reader = outcome_events.get_reader();
            assert!(
                reader.read(outcome_events).next().is_none(),
                "failed grant path must not emit AlchemyOutcomeEvent"
            );
        }

        fn collect_skill_config_snapshots(
            helper: &mut MockClientHelper,
        ) -> Vec<crate::skill::config::SkillConfigSnapshot> {
            helper
                .collect_received()
                .0
                .into_iter()
                .filter_map(|frame| {
                    let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                    if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                        return None;
                    }
                    decode_skill_config_snapshot_payload(packet.data.0 .0)
                })
                .collect()
        }

        fn collect_quickslot_configs(
            helper: &mut MockClientHelper,
        ) -> Vec<crate::schema::combat_hud::QuickSlotConfigV1> {
            helper
                .collect_received()
                .0
                .into_iter()
                .filter_map(|frame| {
                    let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                    if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                        return None;
                    }
                    decode_quickslot_config_payload(packet.data.0 .0)
                })
                .collect()
        }

        fn send_quick_slot_bind_request(
            app: &mut App,
            entity: Entity,
            slot: u8,
            item_id: Option<&str>,
            request_id: &str,
        ) {
            let body = serde_json::json!({
                "type": "quick_slot_bind",
                "v": 1,
                "slot": slot,
                "item_id": item_id,
                "request_id": request_id,
            });
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: body.to_string().into_bytes().into_boxed_slice(),
                });
        }

        fn collect_game_messages(helper: &mut MockClientHelper) -> Vec<String> {
            helper
                .collect_received()
                .0
                .into_iter()
                .filter_map(|frame| {
                    frame
                        .decode::<GameMessageS2c>()
                        .ok()
                        .map(|packet| packet.chat.to_legacy_lossy())
                })
                .collect()
        }

        fn has_inventory_durability_payload(
            helper: &mut MockClientHelper,
            instance_id: u64,
        ) -> bool {
            for frame in helper.collect_received().0 {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    continue;
                };
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    continue;
                }
                if inventory_event_is_durability_changed(packet.data.0 .0, instance_id) {
                    return true;
                }
            }
            false
        }

        fn insert_test_forge_session(
            app: &mut App,
            session_id: u64,
            caster: Entity,
            step: ForgeStep,
        ) {
            let station = app.world_mut().spawn_empty().id();
            let mut sessions = ForgeSessions::new();
            let mut session = ForgeSession::new(
                ForgeSessionId(session_id),
                "qing_feng_v0".to_string(),
                station,
                caster,
            );
            session.current_step = step;
            session.step_state = match step {
                ForgeStep::Inscription => StepState::Inscription(Default::default()),
                ForgeStep::Tempering => StepState::Tempering(Default::default()),
                ForgeStep::Consecration => StepState::Consecration(Default::default()),
                ForgeStep::Billet => StepState::Billet(Default::default()),
                ForgeStep::Done => StepState::None,
            };
            sessions.insert(session);
            app.insert_resource(sessions);
        }

        /// C2S lingtian 测试的完整 payload 捕获：不只是 kind/pos，还要锁住
        /// actor 与 action 专属字段（hoe_instance_id / mode / plant_id / source），
        /// 让 validator→queue→handler 契约的任何字段丢失都撞红（fix-spec §9.4）。
        #[derive(Debug, PartialEq)]
        struct LingtianDispatchCapture {
            kind: &'static str,
            pos: BlockPos,
            player: Entity,
            hoe_instance_id: Option<u64>,
            mode: Option<SessionMode>,
            plant_id: Option<String>,
            source: Option<ReplenishSource>,
        }

        fn drain_lingtian_request_captures(app: &mut App) -> Vec<LingtianDispatchCapture> {
            let world = app.world_mut();
            let mut captured = Vec::new();
            captured.extend(
                world
                    .resource_mut::<Events<StartTillRequest>>()
                    .drain()
                    .map(|event| LingtianDispatchCapture {
                        kind: "till",
                        pos: event.pos,
                        player: event.player,
                        hoe_instance_id: Some(event.hoe_instance_id),
                        mode: Some(event.mode),
                        plant_id: None,
                        source: None,
                    }),
            );
            captured.extend(
                world
                    .resource_mut::<Events<StartRenewRequest>>()
                    .drain()
                    .map(|event| LingtianDispatchCapture {
                        kind: "renew",
                        pos: event.pos,
                        player: event.player,
                        hoe_instance_id: Some(event.hoe_instance_id),
                        mode: None,
                        plant_id: None,
                        source: None,
                    }),
            );
            captured.extend(
                world
                    .resource_mut::<Events<StartPlantingRequest>>()
                    .drain()
                    .map(|event| LingtianDispatchCapture {
                        kind: "planting",
                        pos: event.pos,
                        player: event.player,
                        hoe_instance_id: None,
                        mode: None,
                        plant_id: Some(event.plant_id),
                        source: None,
                    }),
            );
            captured.extend(
                world
                    .resource_mut::<Events<StartHarvestRequest>>()
                    .drain()
                    .map(|event| LingtianDispatchCapture {
                        kind: "harvest",
                        pos: event.pos,
                        player: event.player,
                        hoe_instance_id: None,
                        mode: Some(event.mode),
                        plant_id: None,
                        source: None,
                    }),
            );
            captured.extend(
                world
                    .resource_mut::<Events<StartReplenishRequest>>()
                    .drain()
                    .map(|event| LingtianDispatchCapture {
                        kind: "replenish",
                        pos: event.pos,
                        player: event.player,
                        hoe_instance_id: None,
                        mode: None,
                        plant_id: None,
                        source: Some(event.source),
                    }),
            );
            captured.extend(
                world
                    .resource_mut::<Events<StartDrainQiRequest>>()
                    .drain()
                    .map(|event| LingtianDispatchCapture {
                        kind: "drain_qi",
                        pos: event.pos,
                        player: event.player,
                        hoe_instance_id: None,
                        mode: None,
                        plant_id: None,
                        source: None,
                    }),
            );
            captured
        }

        fn run_lingtian_dispatch_case(
            payload: serde_json::Value,
            position: Option<DVec3>,
            dimension: Option<DimensionKind>,
        ) -> (Entity, Vec<LingtianDispatchCapture>) {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("LingtianDispatch");
            let client = app
                .world_mut()
                .spawn((client_bundle, Lifecycle::default()))
                .id();
            if let Some(position) = position {
                app.world_mut()
                    .entity_mut(client)
                    .insert(Position::new(position));
            }
            if let Some(dimension) = dimension {
                app.world_mut()
                    .entity_mut(client)
                    .insert(CurrentDimension(dimension));
            }
            // `LingtianStartTill` resolves its target from the authoritative plot
            // store before entering the pending queue.  Keep the shared matrix
            // helper's canonical target present so its boundary cases exercise
            // the reach/dimension checks rather than the missing-target branch.
            app.world_mut()
                .spawn(LingtianPlot::new(BlockPos::new(0, 64, 0), None));
            app.world_mut()
                .resource_mut::<Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: payload.to_string().into_bytes().into_boxed_slice(),
                });
            app.update();
            (client, drain_lingtian_request_captures(&mut app))
        }

        #[test]
        fn lingtian_c2s_dispatch_applies_shared_position_and_dimension_gate_to_all_actions() {
            let target = BlockPos::new(0, 64, 0);
            let boundary = DVec3::new(5.0, 64.5, 0.5);
            let just_beyond = DVec3::new(5.000_001, 64.5, 0.5);
            let cases = [
                (
                    "till",
                    serde_json::json!({
                        "type": "lingtian_start_till", "v": 1, "x": 0, "y": 64, "z": 0,
                        "hoe_instance_id": 7, "mode": "manual"
                    }),
                ),
                (
                    "renew",
                    serde_json::json!({
                        "type": "lingtian_start_renew", "v": 1, "x": 0, "y": 64, "z": 0,
                        "hoe_instance_id": 7
                    }),
                ),
                (
                    "planting",
                    serde_json::json!({
                        "type": "lingtian_start_planting", "v": 1, "x": 0, "y": 64, "z": 0,
                        "plant_id": "ci_she_hao"
                    }),
                ),
                (
                    "harvest",
                    serde_json::json!({
                        "type": "lingtian_start_harvest", "v": 1, "x": 0, "y": 64, "z": 0,
                        "mode": "manual"
                    }),
                ),
                (
                    "replenish",
                    serde_json::json!({
                        "type": "lingtian_start_replenish", "v": 1, "x": 0, "y": 64, "z": 0,
                        "source": "bone_coin"
                    }),
                ),
                (
                    "drain_qi",
                    serde_json::json!({
                        "type": "lingtian_start_drain_qi", "v": 1, "x": 0, "y": 64, "z": 0
                    }),
                ),
            ];

            for (kind, payload) in cases {
                let (client, captures) = run_lingtian_dispatch_case(
                    payload.clone(),
                    Some(boundary),
                    Some(DimensionKind::Overworld),
                );
                let expected = LingtianDispatchCapture {
                    kind,
                    pos: target,
                    player: client,
                    hoe_instance_id: (kind == "till" || kind == "renew").then_some(7),
                    mode: (kind == "till" || kind == "harvest").then_some(SessionMode::Manual),
                    plant_id: (kind == "planting").then(|| "ci_she_hao".to_string()),
                    source: (kind == "replenish").then_some(ReplenishSource::BoneCoin),
                };
                assert_eq!(
                    captures,
                    vec![expected],
                    "boundary Overworld {kind} request must preserve the full wire payload \
                 (actor, BlockPos, and action-specific fields) and dispatch exactly once"
                );
                for (label, position, dimension) in [
                    (
                        "just beyond boundary",
                        Some(just_beyond),
                        Some(DimensionKind::Overworld),
                    ),
                    ("wrong dimension", Some(boundary), Some(DimensionKind::Tsy)),
                    ("missing position", None, Some(DimensionKind::Overworld)),
                    ("missing dimension", Some(boundary), None),
                ] {
                    assert!(
                        run_lingtian_dispatch_case(payload.clone(), position, dimension)
                            .1
                            .is_empty(),
                        "{label} {kind} request must be rejected before ECS dispatch"
                    );
                }
            }

            assert!(
                run_lingtian_dispatch_case(
                    serde_json::json!({
                        "type": "lingtian_start_replenish", "v": 1,
                        "x": 0, "y": 64, "z": 0, "source": "unknown_source"
                    }),
                    Some(boundary),
                    Some(DimensionKind::Overworld),
                )
                .1
                .is_empty(),
                "unknown replenish source must preserve its existing parse rejection"
            );
        }

        #[test]
        fn lingtian_start_till_missing_plot_is_rejected_before_pending_queue() {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("LingtianMissingTarget");
            let client = app
                .world_mut()
                .spawn((
                    client_bundle,
                    Lifecycle::default(),
                    CurrentDimension(DimensionKind::Overworld),
                ))
                .id();
            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));

            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({
                    "type": "lingtian_start_till",
                    "v": 1,
                    "x": 99,
                    "y": 64,
                    "z": 99,
                    "hoe_instance_id": 7,
                    "mode": "manual"
                }),
            );
            app.update();

            assert!(
                app.world_mut()
                    .resource_mut::<Events<StartTillRequest>>()
                    .drain()
                    .next()
                    .is_none(),
                "a missing plot must not dispatch StartTillRequest"
            );
            assert!(
                app.world_mut()
                    .resource_mut::<Events<StartTillRequest>>()
                    .drain()
                    .next()
                    .is_none(),
                "a rejected missing-plot request must not leave a pending StartTillRequest"
            );
        }

        #[test]
        fn lingtian_start_till_accepts_authoritative_chunk_without_existing_plot() {
            let scenario = ScenarioSingleClient::new();
            let valence::testing::ScenarioSingleClient {
                mut app,
                client,
                layer,
                ..
            } = scenario;
            mark_test_layer_as_overworld(&mut app);
            register_request_app(&mut app);

            let target = BlockPos::new(0, 64, 0);
            let mut chunk_layer = app
                .world_mut()
                .get_mut::<ChunkLayer>(layer)
                .expect("ScenarioSingleClient must provide the authoritative overworld layer");
            chunk_layer.insert_chunk([0, 0], UnloadedChunk::new());
            chunk_layer.set_block(target, BlockState::DIRT);

            app.world_mut().entity_mut(client).insert((
                Lifecycle::default(),
                CurrentDimension(DimensionKind::Overworld),
                Position::new(DVec3::new(0.5, 64.5, 0.5)),
            ));
            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({
                    "type": "lingtian_start_till",
                    "v": 1,
                    "x": target.x,
                    "y": target.y,
                    "z": target.z,
                    "hoe_instance_id": 7,
                    "mode": "manual"
                }),
            );
            app.update();

            assert_eq!(
            drain_lingtian_request_captures(&mut app),
            vec![LingtianDispatchCapture {
                kind: "till",
                pos: target,
                player: client,
                hoe_instance_id: Some(7),
                mode: Some(SessionMode::Manual),
                plant_id: None,
                source: None,
            }],
            "a loaded authoritative world block must admit till ingress even before a LingtianPlot exists"
        );
            assert!(
                drain_lingtian_request_captures(&mut app).is_empty(),
                "an admitted till request must be consumed exactly once by the validator"
            );
        }

        /// #13 — network ingress 集成契约：真实 producer → 真实 queue → 真实
        /// validator 的多请求 wire FIFO。同 actor 一批三请求只 dispatch 第一条，
        /// 其余保序回到队列；逐 tick 推进后按 wire 顺序逐条 dispatch。
        #[test]
        fn lingtian_c2s_ingress_queue_preserves_wire_fifo_order() {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("LingtianFifo");
            let client = app
                .world_mut()
                .spawn((client_bundle, Lifecycle::default()))
                .id();
            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(DVec3::new(5.0, 64.5, 0.5)));
            app.world_mut()
                .entity_mut(client)
                .insert(CurrentDimension(DimensionKind::Overworld));
            app.world_mut()
                .spawn(LingtianPlot::new(BlockPos::new(1, 64, 0), None));

            let send = |app: &mut App, payload: serde_json::Value| {
                app.world_mut()
                    .resource_mut::<Events<CustomPayloadEvent>>()
                    .send(CustomPayloadEvent {
                        client,
                        channel: ident!("bong:client_request").into(),
                        data: payload.to_string().into_bytes().into_boxed_slice(),
                    });
            };
            send(
                &mut app,
                serde_json::json!({
                    "type": "lingtian_start_till", "v": 1, "x": 1, "y": 64, "z": 0,
                    "hoe_instance_id": 7, "mode": "manual"
                }),
            );
            send(
                &mut app,
                serde_json::json!({
                    "type": "lingtian_start_harvest", "v": 1, "x": 2, "y": 64, "z": 0,
                    "mode": "manual"
                }),
            );
            send(
                &mut app,
                serde_json::json!({
                    "type": "lingtian_start_planting", "v": 1, "x": 3, "y": 64, "z": 0,
                    "plant_id": "ci_she_hao"
                }),
            );

            app.update();
            assert_eq!(
                drain_lingtian_request_captures(&mut app),
                vec![LingtianDispatchCapture {
                    kind: "till",
                    pos: BlockPos::new(1, 64, 0),
                    player: client,
                    hoe_instance_id: Some(7),
                    mode: Some(SessionMode::Manual),
                    plant_id: None,
                    source: None,
                }],
                "first wire request dispatches first"
            );
            assert!(
                drain_lingtian_request_captures(&mut app).is_empty(),
                "the first validator pass must dispatch only the first wire request"
            );

            app.update();
            assert_eq!(
                drain_lingtian_request_captures(&mut app)
                    .iter()
                    .map(|capture| capture.kind)
                    .collect::<Vec<_>>(),
                vec!["harvest"],
                "second wire request dispatches second"
            );
            assert!(
                drain_lingtian_request_captures(&mut app).is_empty(),
                "the second validator pass must dispatch only the second wire request"
            );

            app.update();
            assert_eq!(
                drain_lingtian_request_captures(&mut app)
                    .iter()
                    .map(|capture| capture.kind)
                    .collect::<Vec<_>>(),
                vec!["planting"],
                "third wire request dispatches last"
            );
            assert!(
                drain_lingtian_request_captures(&mut app).is_empty(),
                "the third validator pass must drain the final wire request exactly once"
            );
        }

        /// #16 — 生产装配回归：`LingtianRequestIngressSet` 的排序边是 producer 先于
        /// validator 的唯一机制。validator 先注册、producer 后注册（反插入序）时，
        /// 删除 `network/mod.rs` 里 producer 的 `.in_set(...)` 会让本测试撞红
        /// （请求停留在持久队列、本 tick 无 dispatch）。
        #[test]
        fn production_ingress_wiring_orders_producer_before_validator() {
            let mut app = App::new();
            register_request_resources(&mut app);
            app.init_resource::<LingtianPlotIndex>();
            app.add_systems(
                Update,
                refresh_lingtian_plot_index.before(handle_client_request_payloads),
            );
            app.add_systems(
                Update,
                cleanup_client_request_budget.before(handle_client_request_payloads),
            );
            app.add_systems(
                Update,
                handle_client_request_payloads.in_set(crate::lingtian::LingtianRequestIngressSet),
            );
            app.add_systems(
                Update,
                crate::lingtian::systems::validate_and_dispatch_lingtian_requests
                    .after(crate::lingtian::LingtianRequestIngressSet),
            );

            let (client_bundle, _helper) = create_mock_client("IngressWiring");
            let client = app
                .world_mut()
                .spawn((client_bundle, Lifecycle::default()))
                .id();
            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));
            app.world_mut()
                .entity_mut(client)
                .insert(CurrentDimension(DimensionKind::Overworld));
            app.world_mut()
                .spawn(LingtianPlot::new(BlockPos::new(0, 64, 0), None));
            app.world_mut()
                .resource_mut::<Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::json!({
                        "type": "lingtian_start_till", "v": 1, "x": 0, "y": 64, "z": 0,
                        "hoe_instance_id": 7, "mode": "manual"
                    })
                    .to_string()
                    .into_bytes()
                    .into_boxed_slice(),
                });

            app.update();

            assert_eq!(
                drain_lingtian_request_captures(&mut app),
                vec![LingtianDispatchCapture {
                    kind: "till",
                    pos: BlockPos::new(0, 64, 0),
                    player: client,
                    hoe_instance_id: Some(7),
                    mode: Some(SessionMode::Manual),
                    plant_id: None,
                    source: None,
                }],
                "production ingress wiring must dispatch the wire request in the same tick"
            );
            assert!(
                drain_lingtian_request_captures(&mut app).is_empty(),
                "production ingress wiring must not leave a duplicate pending dispatch"
            );
        }

        fn send_gate_test_payload(app: &mut App, client: Entity, payload: serde_json::Value) {
            app.world_mut()
                .resource_mut::<Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: payload.to_string().into_bytes().into_boxed_slice(),
                });
        }

        #[test]
        fn c2s_ingress_budget_drops_the_thirty_third_payload_before_decode() {
            let mut app = App::new();
            // This contract exercises only the ingress budget; omit the full
            // lingtian emitter so its idle HUD snapshot cannot pollute the wire assertion.
            register_request_resources_without_lingtian(&mut app);
            register_request_systems(&mut app);
            let (client_bundle, mut helper) = create_mock_client("BudgetIngress");
            let client = app.world_mut().spawn(client_bundle).id();

            for _ in 0..33 {
                app.world_mut()
                    .resource_mut::<Events<CustomPayloadEvent>>()
                    .send(CustomPayloadEvent {
                        client,
                        channel: ident!("bong:client_request").into(),
                        data: vec![0xff].into_boxed_slice(),
                    });
            }

            app.update();

            assert_eq!(
                app.world()
                    .resource::<ClientRequestBudget>()
                    .store
                    .tokens_for(&client),
                Some(0),
                "the 33rd same-tick payload must be refused by ingress after 32 admissions"
            );
            flush_all_client_packets(&mut app);
            let payload_types = collect_server_data_payload_types(&mut helper);
            assert_eq!(
            payload_types,
            vec!["event_alert"],
            "only the budgeted rate-limit feedback may be emitted; malformed payload #33 must not be decoded"
        );
        }

        #[test]
        fn craft_start_live_gate_rejects_missing_inventory_without_emitting_intent() {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("CraftGate");
            let client = app
                .world_mut()
                .spawn((
                    client_bundle,
                    empty_inventory(),
                    Lifecycle::default(),
                    CurrentDimension(DimensionKind::Overworld),
                ))
                .id();
            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));

            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({
                    "type": "craft_start",
                    "v": 1,
                    "recipe_id": "craft.example.herb_knife.iron",
                    "quantity": 1
                }),
            );
            app.update();
            let accepted = app
                .world_mut()
                .resource_mut::<Events<crate::craft::CraftStartIntent>>()
                .drain()
                .collect::<Vec<_>>();
            assert_eq!(
                accepted.len(),
                1,
                "valid craft ingress must emit exactly one intent"
            );

            app.world_mut()
                .entity_mut(client)
                .remove::<PlayerInventory>();
            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({
                    "type": "craft_start",
                    "v": 1,
                    "recipe_id": "craft.example.herb_knife.iron",
                    "quantity": 1
                }),
            );
            app.update();
            assert!(
            app.world_mut()
                .resource_mut::<Events<crate::craft::CraftStartIntent>>()
                .drain()
                .next()
                .is_none(),
            "missing inventory must be rejected before CraftStartIntent and therefore before mutation"
        );
        }

        #[test]
        fn workbench_open_live_gate_dispatches_only_a_resolved_nearby_workbench() {
            let mut app = App::new();
            app.add_plugins(EntityPlugin);
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("WorkbenchGate");
            let client = app
                .world_mut()
                .spawn((
                    client_bundle,
                    Lifecycle::default(),
                    CurrentDimension(DimensionKind::Overworld),
                ))
                .id();
            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));
            let workbench = app
                .world_mut()
                .spawn((
                    crate::world::entity_model::WORKBENCH_ENTITY_KIND,
                    EntityId::default(),
                    Position::new(DVec3::new(0.5, 64.0, 0.5)),
                    OldPosition::new(DVec3::new(0.5, 64.0, 0.5)),
                    CurrentDimension(DimensionKind::Overworld),
                    WorkbenchBlock {
                        placed_by: client,
                        placed_at_tick: 0,
                    },
                ))
                .id();
            app.update();
            let entity_id = app
                .world()
                .get::<EntityId>(workbench)
                .expect("EntityPlugin must assign the workbench protocol id")
                .get();

            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({ "type": "workbench_open", "v": 1, "entity_id": entity_id }),
            );
            app.update();
            let requests = app
                .world_mut()
                .resource_mut::<Events<crate::craft::WorkbenchOpenRequest>>()
                .drain()
                .collect::<Vec<_>>();
            assert_eq!(
                requests.len(),
                1,
                "nearby workbench must reach its open consumer"
            );
            assert_eq!(requests[0].workbench, workbench);

            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(DVec3::new(4.000_001, 64.5, 0.5)));
            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({ "type": "workbench_open", "v": 1, "entity_id": entity_id }),
            );
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Events<crate::craft::WorkbenchOpenRequest>>()
                    .drain()
                    .next()
                    .is_none(),
                "out-of-reach workbench must be rejected before the open event"
            );

            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));
            app.world_mut()
                .entity_mut(workbench)
                .insert(CurrentDimension(DimensionKind::Tsy));
            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({ "type": "workbench_open", "v": 1, "entity_id": entity_id }),
            );
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Events<crate::craft::WorkbenchOpenRequest>>()
                    .drain()
                    .next()
                    .is_none(),
                "cross-dimension workbench must be rejected before the open event"
            );

            app.world_mut()
                .entity_mut(workbench)
                .insert(CurrentDimension(DimensionKind::Overworld));
            app.world_mut()
                .entity_mut(workbench)
                .remove::<WorkbenchBlock>();
            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({ "type": "workbench_open", "v": 1, "entity_id": entity_id }),
            );
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Events<crate::craft::WorkbenchOpenRequest>>()
                    .drain()
                    .next()
                    .is_none(),
                "a target without WorkbenchBlock must be rejected before the open event"
            );

            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({ "type": "workbench_open", "v": 1, "entity_id": entity_id + 999 }),
            );
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Events<crate::craft::WorkbenchOpenRequest>>()
                    .drain()
                    .next()
                    .is_none(),
                "an unresolved workbench entity id must be rejected before the open event"
            );
        }

        #[test]
        fn give_dan_live_gate_preserves_inventory_when_elder_state_is_invalid() {
            let mut app = App::new();
            app.add_plugins(EntityPlugin);
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("ElderGate");
            let client = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_stack("huiyuan_pill", 1),
                    Lifecycle::default(),
                    CurrentDimension(DimensionKind::Overworld),
                ))
                .id();
            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));
            let elder = app
                .world_mut()
                .spawn((
                    EntityKind::new(164),
                    EntityId::default(),
                    crate::npc::lifecycle::NpcArchetype::DyingElder,
                    crate::npc::spawn::NpcMarker,
                    Position::new(DVec3::new(0.5, 64.0, 0.5)),
                    OldPosition::new(DVec3::new(0.5, 64.0, 0.5)),
                    CurrentDimension(DimensionKind::Overworld),
                    DyingElderState::Plea,
                ))
                .id();
            app.update();
            let elder_id = app
                .world()
                .get::<EntityId>(elder)
                .expect("EntityPlugin must assign the elder protocol id")
                .get();

            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({
                    "type": "give_dan_to_elder",
                    "v": 1,
                    "pill_instance_id": 9001,
                    "elder_entity_id": elder_id
                }),
            );
            app.update();
            let accepted = app
                .world_mut()
                .resource_mut::<Events<crate::fauna::dying_elder::GiveDanToElderIntent>>()
                .drain()
                .collect::<Vec<_>>();
            assert_eq!(
                accepted.len(),
                1,
                "Plea elder must accept a live give-dan intent"
            );
            let revision_before = app.world().get::<PlayerInventory>(client).unwrap().revision;

            app.world_mut()
                .entity_mut(elder)
                .insert(DyingElderState::Dead {
                    dead_by_betrayal: false,
                });
            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({
                    "type": "give_dan_to_elder",
                    "v": 1,
                    "pill_instance_id": 9001,
                    "elder_entity_id": elder_id
                }),
            );
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Events<crate::fauna::dying_elder::GiveDanToElderIntent>>()
                    .drain()
                    .next()
                    .is_none(),
                "dead elder must be rejected before the give-dan mutation path"
            );
            assert_eq!(
                app.world().get::<PlayerInventory>(client).unwrap().revision,
                revision_before,
                "gate rejection must leave the pill inventory revision unchanged"
            );

            app.world_mut()
                .entity_mut(elder)
                .insert(DyingElderState::Plea);
            app.world_mut()
                .entity_mut(elder)
                .insert(CurrentDimension(DimensionKind::Tsy));
            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({
                    "type": "give_dan_to_elder",
                    "v": 1,
                    "pill_instance_id": 9001,
                    "elder_entity_id": elder_id
                }),
            );
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Events<crate::fauna::dying_elder::GiveDanToElderIntent>>()
                    .drain()
                    .next()
                    .is_none(),
                "cross-dimension elder must be rejected before the give-dan intent"
            );

            app.world_mut()
                .entity_mut(elder)
                .insert(CurrentDimension(DimensionKind::Overworld))
                .insert(Position::new(DVec3::new(100.0, 64.0, 100.0)));
            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({
                    "type": "give_dan_to_elder",
                    "v": 1,
                    "pill_instance_id": 9001,
                    "elder_entity_id": elder_id
                }),
            );
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Events<crate::fauna::dying_elder::GiveDanToElderIntent>>()
                    .drain()
                    .next()
                    .is_none(),
                "out-of-reach elder must be rejected before the give-dan intent"
            );

            app.world_mut()
                .entity_mut(elder)
                .insert(Position::new(DVec3::new(0.5, 64.0, 0.5)));
            send_gate_test_payload(
                &mut app,
                client,
                serde_json::json!({
                    "type": "give_dan_to_elder",
                    "v": 1,
                    "pill_instance_id": 9001,
                    "elder_entity_id": elder_id + 999
                }),
            );
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Events<crate::fauna::dying_elder::GiveDanToElderIntent>>()
                    .drain()
                    .next()
                    .is_none(),
                "an unresolved elder entity id must be rejected before the give-dan intent"
            );
            assert_eq!(
                app.world().get::<PlayerInventory>(client).unwrap().revision,
                revision_before,
                "all live gate denials must preserve the pill inventory revision"
            );
        }

        fn register_request_resources(app: &mut App) {
            // Use the existing public lingtian assembly so the handler tests exercise
            // the real validator/start-handler wiring without opening a test-only seam.
            app.insert_resource(crate::botany::PlantKindRegistry::default());
            crate::lingtian::register(app);
            register_request_handler_resources(app);
        }

        /// Minimal handler-only fixture for contracts that intentionally exercise a
        /// missing optional resource. The full lingtian assembly contains systems
        /// with a mandatory allocator parameter, so it cannot be used for that
        /// negative branch without turning the fixture panic into the assertion.
        fn register_request_resources_without_lingtian(app: &mut App) {
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            register_request_handler_resources(app);
        }

        fn register_request_handler_resources(app: &mut App) {
            app.insert_resource(CombatClock::default());
            app.init_resource::<crate::inventory::InventoryInstanceIdAllocator>();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(crate::cultivation::skill_registry::init_registry());
            app.insert_resource(load_test_technique_registry());
            // plan-bug-qc-p1 §skill-cast P0：经脉依赖表（测试场景 default 空，各测可再声明）
            app.insert_resource(SkillMeridianDependencies::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.insert_resource(ZoneRegistry::fallback());
            app.init_resource::<SkillConfigStore>();
            app.insert_resource(SkillConfigSchemas::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<crate::combat::events::AttackIntent>();
            app.add_event::<crate::qi_physics::ledger::QiTransfer>();
            app.add_event::<crate::cultivation::burst_meridian::BurstMeridianEvent>();
            app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
            app.add_event::<crate::network::audio_event_emit::PlaySoundRecipeRequest>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<RevivalActionIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<FalseSkinForgeRequest>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<SpiritNichePlaceRequest>();
            app.add_event::<SpiritNicheRepairRequest>();
            app.add_event::<SpiritNicheCoordinateRevealRequest>();
            app.add_event::<CoffinOpenRequest>();
            app.add_event::<crate::coffin::CoffinBreakRequest>();
            app.add_event::<crate::coffin::CoffinMenuReclaimRequest>();
            app.add_event::<crate::craft::CraftStartIntent>();
            app.add_event::<crate::fauna::dying_elder::GiveDanToElderIntent>();
            app.add_event::<crate::craft::WorkbenchOpenRequest>();
            app.add_event::<crate::world::container_open::ContainerOpenRequest>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<QiColorInspectRequest>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<FreshnessProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            app.add_event::<BlockPlaceRequest>();
            app.add_event::<ZhenfaPlaceRequest>();
            app.add_event::<ZhenfaTriggerRequest>();
            app.add_event::<ZhenfaDisarmRequest>();
            app.add_event::<ScatterBeadUseRequest>();
            app.add_event::<InventoryDurabilityChangedEvent>();
            app.add_event::<crate::alchemy::AlchemyOutcomeEvent>();
            app.add_event::<crate::combat::events::CombatEvent>();
            app.add_event::<crate::combat::events::DeathEvent>();
            app.add_event::<crate::combat::zhenmai_v2::LocalNeutralizeEvent>();
            app.add_event::<crate::combat::zhenmai_v2::MultiPointBackfireEvent>();
            app.add_event::<crate::combat::zhenmai_v2::MeridianHardenEvent>();
            app.add_event::<crate::combat::zhenmai_v2::MeridianSeveredVoluntaryEvent>();
            app.add_event::<crate::combat::zhenmai_v2::BackfireAmplificationActiveEvent>();
            app.add_event::<crate::cultivation::meridian::severed::MeridianSeveredEvent>();
            app.add_event::<crate::cultivation::overload::MeridianOverloadEvent>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            // plan-agent-ui-data-v1 P0 — 天道 UI 响应 event（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            // plan-worldgen-v4 P5 §8.1#5 — dev give-block intent（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::cmd::dev::block_picker::BlockPickerGiveIntent>();
        }

        /// 生产装配：producer 经 `LingtianRequestIngressSet`（与
        /// `network::register_app_wiring` 同路径），validator 排在该 set 之后
        /// （与 `lingtian::register` 的 chain 同合同）。测试删掉 set membership
        /// 会直接破坏这里的排序边（见 `production_ingress_wiring_orders_*`）。
        fn register_request_systems(app: &mut App) {
            app.init_resource::<ClientRequestBudget>();
            app.init_resource::<LingtianPlotIndex>();
            app.add_systems(
                Update,
                refresh_lingtian_plot_index.before(handle_client_request_payloads),
            );
            app.add_systems(
                Update,
                cleanup_client_request_budget.before(handle_client_request_payloads),
            );
            app.add_systems(
                Update,
                handle_client_request_payloads.in_set(crate::lingtian::LingtianRequestIngressSet),
            );
            app.add_systems(
                Update,
                crate::network::inventory_event_emit::emit_durability_changed_inventory_events
                    // 原 test 装配对 producer 与 emitter 用了 `.chain()`：inventory move
                    // 的 durability payload 必须同帧发出（`inventory_move_applies_*` 单
                    // update + flush 断言）。拆生产装配后 chain 没了，改挂 set 后置边保
                    // 持同帧语义——生产路径不依赖此边（每帧全扫，晚一帧无害）。
                    .after(crate::lingtian::LingtianRequestIngressSet),
            );
        }

        fn register_request_app(app: &mut App) {
            register_request_resources(app);
            register_request_systems(app);
        }

        fn upsert_test_harvest_session(
            app: &mut App,
            player_id: &str,
            client_entity: Entity,
            mode: BotanyHarvestMode,
            started_at_tick: u64,
            last_progress: f32,
        ) -> Entity {
            let plant = app.world_mut().spawn_empty().id();
            app.world_mut()
                .resource_mut::<HarvestSessionStore>()
                .try_insert_session(HarvestSession {
                    player_id: player_id.to_string(),
                    client_entity,
                    target_entity: Some(plant),
                    target_plant: BotanyPlantId::CiSheHao,
                    mode,
                    started_at_tick,
                    // 这些 fixtures 都从既有的手动采集 session 开始；合法 mode 请求会
                    // 通过公开 handler 更新为对应的自动/手动时长。
                    duration_ticks: 40,
                    phase: BotanyPhase::InProgress,
                    last_progress,
                    origin_position: [1.0, 64.0, 1.0],
                })
                .expect(
                    "test fixture must not silently overwrite another player's target reservation",
                );
            plant
        }

        fn send_botany_harvest_request(
            app: &mut App,
            client: Entity,
            session_id: &str,
            mode: &str,
        ) {
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: format!(
                    r#"{{"type":"botany_harvest_request","v":1,"session_id":"{session_id}","mode":"{mode}"}}"#
                )
                .into_bytes()
                .into_boxed_slice(),
            });
        }

        fn neutral_faction_membership() -> FactionMembership {
            FactionMembership {
                faction_id: FactionId::Neutral,
                rank: FactionRank::Disciple,
                reputation: Reputation::default(),
                lineage: None,
                mission_queue: MissionQueue::default(),
            }
        }

        // ══════════════════════════════════════════════════════════════════════════
        // plan-race-system-v1 P4 opus verifier MINOR —— 装备门 `form_race_id` pin
        // 测试。镜像已锁的 emit 路径测试
        // （`cultivation_detail_emit::morph_state_present_overrides_form_race_id_away_from_intrinsic_race`）：
        // `handle_inventory_move` 内 `form_race_id` 的推导（§13303 附近）此前只有 emit
        // 侧的回归 pin，装备门（`InventoryMoveIntent` → `handle_inventory_move` →
        // `apply_inventory_move_with_race`）这条真正决定"能不能穿"的路径完全没有端到端
        // 测试锁住"用 Form 身份而不是本体 intrinsic 身份"这条契约。走真实
        // `ClientRequestV1::InventoryMoveIntent` C2S 事件 → `handle_client_request_payloads`
        // 全链路。
        // ══════════════════════════════════════════════════════════════════════════

        fn make_armor_straw_chestplate_registry(allowed_race: &str) -> ItemRegistry {
            ItemRegistry::from_map(HashMap::from([(
                "armor_straw_chestplate".to_string(),
                ItemTemplate {
                    id: "armor_straw_chestplate".to_string(),
                    display_name: "species-gated chestplate".to_string(),
                    category: ItemCategory::Armor,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 1.0,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 0.0,
                    description: "test".to_string(),
                    effect: None,
                    cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                    cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shield_spec: None,
                    shelflife_profile: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::Species {
                        species: vec![crate::body_plan::RaceId::new(allowed_race)],
                    },
                },
            )]))
        }

        fn spawn_player_with_armor_straw_chestplate_in_pack(
            app: &mut App,
            username: &str,
            intrinsic_race: &str,
        ) -> Entity {
            let (client_bundle, _helper) = create_mock_client(username);
            let item = ItemInstance {
                instance_id: 1,
                template_id: "armor_straw_chestplate".to_string(),
                display_name: "species-gated chestplate".to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 1.0,
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
            };
            let inventory = PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(0),
                containers: vec![ContainerState {
                    quick_access: false,
                    id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows: 4,
                    cols: 4,
                    items: vec![PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: item,
                    }],
                    owner_instance_id: None,
                }],
                equipped: HashMap::new(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 99.0,
            };
            app.world_mut()
                .spawn((
                    client_bundle,
                    inventory,
                    Cultivation {
                        race: crate::body_plan::RaceId::new(intrinsic_race),
                        ..Cultivation::default()
                    },
                    PlayerState {
                        karma: 0.0,
                        inventory_score: 0.0,
                    },
                ))
                .id()
        }

        fn send_species_gated_equip_intent(app: &mut App, client: Entity) {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::InventoryMoveIntent {
                        v: 1,
                        instance_id: 1,
                        from: InventoryLocationV1::Container {
                            container_id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                            row: 0,
                            col: 0,
                        },
                        to: InventoryLocationV1::Equip {
                            slot: EquipSlotV1::Chest,
                            state: EquipStateV1::Worn,
                        },
                        rotated: false,
                    })
                    .expect("InventoryMoveIntent must serialize")
                    .into_boxed_slice(),
                });
            app.update();
        }

        #[test]
        fn morphed_player_equip_gate_uses_form_race_not_intrinsic_race() {
            // 本体（intrinsic）种族是 whale，但已易形为 human——胸甲只认 human，装备门
            // 判定必须用 MorphState.form="human"（放行），而不是冒用本体 Cultivation.race
            // ="whale"（会误拒）。
            let mut app = App::new();
            app.add_plugins(EntityPlugin);
            register_request_app(&mut app);
            app.insert_resource(make_armor_straw_chestplate_registry(
                crate::body_plan::HUMAN_RACE_ID,
            ));

            let client =
                spawn_player_with_armor_straw_chestplate_in_pack(&mut app, "Morpher", "whale");
            app.world_mut()
                .entity_mut(client)
                .insert(crate::body_plan::MorphState::new(
                    crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                    0,
                    0,
                ));

            send_species_gated_equip_intent(&mut app, client);

            let inventory = app.world().entity(client).get::<PlayerInventory>().unwrap();
            let equipped_chest = inventory
                .equipped
                .get(crate::inventory::EQUIP_SLOT_CHEST)
                .and_then(|contents| contents.worn.first());
            assert_eq!(
                equipped_chest.map(|item| item.instance_id),
                Some(1),
                "已易形为 human 的 whale 本体应能穿上 Species([human]) 门的胸甲——装备门必须\
             用 MorphState.form 而不是继续冒用本体 intrinsic race，实测装备槽：{:?}",
                inventory.equipped.get(crate::inventory::EQUIP_SLOT_CHEST)
            );
        }

        #[test]
        fn unmorphed_whale_intrinsic_is_rejected_by_same_species_gate() {
            // 对照组：同一件甲、同一本体，缺 MorphState（未易形）时装备门应回落到本体
            // intrinsic race="whale"，被 Species([human]) 门拒绝——证明上一条测试确实
            // 是因为 MorphState 生效才放行，不是这件甲本来就对谁都放行。
            let mut app = App::new();
            app.add_plugins(EntityPlugin);
            register_request_app(&mut app);
            app.insert_resource(make_armor_straw_chestplate_registry(
                crate::body_plan::HUMAN_RACE_ID,
            ));

            let client =
                spawn_player_with_armor_straw_chestplate_in_pack(&mut app, "Morpher", "whale");

            send_species_gated_equip_intent(&mut app, client);

            let inventory = app.world().entity(client).get::<PlayerInventory>().unwrap();
            let equipped_chest = inventory
                .equipped
                .get(crate::inventory::EQUIP_SLOT_CHEST)
                .and_then(|contents| contents.worn.first());
            assert!(
                equipped_chest.is_none(),
                "未易形的 whale 本体应被 Species([human]) 门拒绝穿戴，实测装备槽：{:?}",
                inventory.equipped.get(crate::inventory::EQUIP_SLOT_CHEST)
            );
            // 应仍留在原背包容器里，而不是被静默吞掉。
            let still_in_pack = inventory.containers[0]
                .items
                .iter()
                .any(|placed| placed.instance.instance_id == 1);
            assert!(still_in_pack, "拒绝装备后物品应留在原容器，不能凭空消失");
        }

        #[test]
        fn npc_trade_request_rejects_wanted_player_through_engagement_wiring() {
            let mut app = App::new();
            app.add_plugins(EntityPlugin);
            register_request_app(&mut app);
            app.insert_resource(ZoneRegistry::load_from_path(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"),
            ));

            let qingyun_pos = DVec3::new(-3000.0, 120.0, -2000.0);
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut faction_reputation = FactionReputation::default();
            faction_reputation.apply_delta(NamedFactionId::QingyunHunters, -51);
            assert_eq!(
            faction_reputation.score(NamedFactionId::QingyunHunters),
            -51,
            "wanted-player fixture must carry the intended Qingyun faction score before ingress"
        );
            let mut npc_membership = neutral_faction_membership();
            npc_membership.reputation = Reputation { loyalty: 0.8 };
            let player = app
                .world_mut()
                .spawn((
                    client_bundle,
                    PlayerIdentities::with_default("Azure", 0),
                    faction_reputation,
                ))
                .id();
            app.world_mut()
                .entity_mut(player)
                .insert(Position::new(qingyun_pos));
            let npc = app
                .world_mut()
                .spawn((
                    NpcMarker,
                    EntityKind::VILLAGER,
                    EntityId::default(),
                    Position::new(qingyun_pos + DVec3::new(1.0, 0.0, 0.0)),
                    OldPosition::new(qingyun_pos + DVec3::new(1.0, 0.0, 0.0)),
                    NpcArchetype::Commoner,
                    npc_membership,
                ))
                .id();

            app.update();
            let npc_entity_id = app
                .world()
                .get::<EntityId>(npc)
                .expect("EntityPlugin must assign protocol id to NPC")
                .get();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: player,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::NpcTradeRequest {
                        v: 1,
                        npc_entity_id,
                        offered_items: Vec::new(),
                        requested_item_id: "spirit_grass".to_string(),
                    })
                    .expect("npc trade request should serialize")
                    .into_boxed_slice(),
                });

            app.update();
            flush_all_client_packets(&mut app);

            let messages = collect_game_messages(&mut helper);
            assert!(
            messages.iter().any(|message| message.contains("不做买卖")),
            "Wanted player should be refused by NpcTradeRequest via resolve_npc_engagement_target/can_trade wiring, messages={messages:?}"
        );
            assert!(
                app.world().get::<PlayerInventory>(player).is_none(),
                "Wanted rejection happens before trade side effects or inventory mutation"
            );
        }

        fn setup_npc_request_app(
            player_position: DVec3,
            npc_position: DVec3,
            player_dimension: Option<DimensionKind>,
            npc_dimension: Option<DimensionKind>,
            archetype: NpcArchetype,
        ) -> (App, Entity, Entity, i32, MockClientHelper) {
            let mut app = App::new();
            app.add_plugins(EntityPlugin);
            register_request_app(&mut app);

            let (client_bundle, helper) = create_mock_client("NpcRoute");
            let player = app
                .world_mut()
                .spawn((client_bundle, empty_inventory()))
                .id();
            app.world_mut()
                .entity_mut(player)
                .insert(Position::new(player_position));
            if let Some(dimension) = player_dimension {
                app.world_mut()
                    .entity_mut(player)
                    .insert(CurrentDimension(dimension));
            }

            let npc = app
                .world_mut()
                .spawn((
                    NpcMarker,
                    EntityKind::VILLAGER,
                    EntityId::default(),
                    Position::new(npc_position),
                    OldPosition::new(npc_position),
                    archetype,
                ))
                .id();
            if let Some(dimension) = npc_dimension {
                app.world_mut()
                    .entity_mut(npc)
                    .insert(CurrentDimension(dimension));
            }

            app.update();
            let npc_entity_id = app
                .world()
                .get::<EntityId>(npc)
                .expect("EntityPlugin must assign protocol id to NPC")
                .get();
            (app, player, npc, npc_entity_id, helper)
        }

        fn send_npc_request(app: &mut App, client: Entity, request: ClientRequestV1) {
            app.world_mut()
                .resource_mut::<Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&request)
                        .expect("NPC request should serialize")
                        .into_boxed_slice(),
                });
        }

        #[test]
        fn npc_inspect_request_preserves_feedback_and_rejects_invalid_targets() {
            let (mut app, player, _npc, npc_entity_id, mut helper) = setup_npc_request_app(
                DVec3::new(0.0, 64.0, 0.0),
                DVec3::new(1.0, 64.0, 0.0),
                None,
                None,
                NpcArchetype::Commoner,
            );
            let revision_before = app.world().get::<PlayerInventory>(player).unwrap().revision;

            send_npc_request(
                &mut app,
                player,
                ClientRequestV1::NpcInspectRequest {
                    v: 1,
                    npc_entity_id,
                },
            );
            app.update();
            flush_all_client_packets(&mut app);
            let messages = collect_game_messages(&mut helper);
            assert_eq!(
                messages.len(),
                1,
                "a nearby inspect must emit exactly one chat line"
            );
            assert!(
                messages[0].starts_with("§7[NPC] "),
                "inspect must preserve the existing NPC greeting feedback, messages={messages:?}"
            );
            assert_eq!(
                app.world().get::<PlayerInventory>(player).unwrap().revision,
                revision_before,
                "inspect must not mutate the player inventory"
            );

            send_npc_request(
                &mut app,
                player,
                ClientRequestV1::NpcInspectRequest {
                    v: 1,
                    npc_entity_id: npc_entity_id.saturating_add(9999),
                },
            );
            app.update();
            flush_all_client_packets(&mut app);
            let messages = collect_game_messages(&mut helper);
            assert_eq!(
                messages,
                vec!["[NPC] 目标已不在附近，无法查看。"],
                "an unresolved NPC id must use the existing inspect rejection feedback"
            );

            app.world_mut()
                .entity_mut(player)
                .insert(Position::new(DVec3::new(0.0, 64.0, 0.0)));
            app.world_mut()
                .entity_mut(_npc)
                .insert(Position::new(DVec3::new(6.000_001, 64.0, 0.0)));
            send_npc_request(
                &mut app,
                player,
                ClientRequestV1::NpcInspectRequest {
                    v: 1,
                    npc_entity_id,
                },
            );
            app.update();
            flush_all_client_packets(&mut app);
            let messages = collect_game_messages(&mut helper);
            assert_eq!(
                messages,
                vec!["[NPC] 目标已不在附近，无法查看。"],
                "an NPC beyond the six-block interaction boundary must be rejected"
            );

            app.world_mut()
                .entity_mut(_npc)
                .insert(Position::new(DVec3::new(1.0, 64.0, 0.0)));
            app.world_mut()
                .entity_mut(player)
                .insert(CurrentDimension(DimensionKind::Overworld));
            app.world_mut()
                .entity_mut(_npc)
                .insert(CurrentDimension(DimensionKind::Tsy));
            send_npc_request(
                &mut app,
                player,
                ClientRequestV1::NpcInspectRequest {
                    v: 1,
                    npc_entity_id,
                },
            );
            app.update();
            flush_all_client_packets(&mut app);
            let messages = collect_game_messages(&mut helper);
            assert_eq!(
                messages,
                vec!["[NPC] 目标已不在附近，无法查看。"],
                "an NPC in another dimension must be rejected before feedback lookup"
            );
        }

        #[test]
        fn npc_dialogue_request_preserves_choices_and_refusal_audio() {
            let (mut app, player, _npc, npc_entity_id, mut helper) = setup_npc_request_app(
                DVec3::new(0.0, 64.0, 0.0),
                DVec3::new(1.0, 64.0, 0.0),
                None,
                None,
                NpcArchetype::Commoner,
            );
            let revision_before = app.world().get::<PlayerInventory>(player).unwrap().revision;

            for (option_id, expected_message) in
                [("inspect", "端详了一眼"), ("trade", "摊开了随身货物")]
            {
                send_npc_request(
                    &mut app,
                    player,
                    ClientRequestV1::NpcDialogueChoice {
                        v: 1,
                        npc_entity_id,
                        option_id: option_id.to_string(),
                    },
                );
                app.update();
                flush_all_client_packets(&mut app);
                let messages = collect_game_messages(&mut helper);
                assert_eq!(
                    messages.len(),
                    1,
                    "dialogue option {option_id} must emit one reply"
                );
                assert!(
                messages[0].contains(expected_message),
                "dialogue option {option_id} must preserve its existing reply, messages={messages:?}"
            );
                assert!(
                    app.world_mut()
                        .resource_mut::<Events<PlaySoundRecipeRequest>>()
                        .drain()
                        .next()
                        .is_none(),
                    "accepted dialogue option {option_id} must not emit refusal audio"
                );
            }

            send_npc_request(
                &mut app,
                player,
                ClientRequestV1::NpcDialogueChoice {
                    v: 1,
                    npc_entity_id,
                    option_id: "leave".to_string(),
                },
            );
            app.update();
            flush_all_client_packets(&mut app);
            assert!(
                collect_game_messages(&mut helper).is_empty(),
                "leave must preserve the existing silent dialogue behavior"
            );
            assert!(
                app.world_mut()
                    .resource_mut::<Events<PlaySoundRecipeRequest>>()
                    .drain()
                    .next()
                    .is_none(),
                "leave must not emit refusal audio"
            );

            send_npc_request(
                &mut app,
                player,
                ClientRequestV1::NpcDialogueChoice {
                    v: 1,
                    npc_entity_id,
                    option_id: "not-a-dialogue-option".to_string(),
                },
            );
            app.update();
            flush_all_client_packets(&mut app);
            let messages = collect_game_messages(&mut helper);
            assert_eq!(
                messages.len(),
                1,
                "an invalid dialogue option must emit one refusal"
            );
            assert!(
                messages[0].contains("不愿回应这个选择"),
                "invalid dialogue option must preserve the refusal feedback, messages={messages:?}"
            );
            let refusal_audio = app
                .world_mut()
                .resource_mut::<Events<PlaySoundRecipeRequest>>()
                .drain()
                .collect::<Vec<_>>();
            assert_eq!(
                refusal_audio.len(),
                1,
                "invalid dialogue option must emit one refusal sound"
            );
            assert_eq!(refusal_audio[0].recipe_id, "npc_refuse");
            assert_eq!(
                app.world().get::<PlayerInventory>(player).unwrap().revision,
                revision_before,
                "dialogue choices must not mutate the player inventory revision"
            );
        }

        fn run_npc_trade_request(
            player_inventory: PlayerInventory,
            trade_inventory: Option<crate::npc::trade::NpcTradeInventory>,
            requested_item_id: &str,
        ) -> (App, Entity, MockClientHelper) {
            run_npc_trade_request_with_reputation(
                player_inventory,
                trade_inventory,
                requested_item_id,
                None,
            )
        }

        fn run_npc_trade_request_with_reputation(
            player_inventory: PlayerInventory,
            trade_inventory: Option<crate::npc::trade::NpcTradeInventory>,
            requested_item_id: &str,
            npc_player_reputation: Option<NpcPlayerReputation>,
        ) -> (App, Entity, MockClientHelper) {
            run_npc_trade_request_with_context(
                player_inventory,
                trade_inventory,
                requested_item_id,
                npc_player_reputation,
                None,
                DVec3::new(0.0, 64.0, 0.0),
                None,
            )
        }

        fn run_npc_trade_request_with_context(
            player_inventory: PlayerInventory,
            trade_inventory: Option<crate::npc::trade::NpcTradeInventory>,
            requested_item_id: &str,
            npc_player_reputation: Option<NpcPlayerReputation>,
            player_faction_reputation: Option<FactionReputation>,
            position: DVec3,
            npc_membership: Option<FactionMembership>,
        ) -> (App, Entity, MockClientHelper) {
            let mut app = App::new();
            app.add_plugins(EntityPlugin);
            register_request_app(&mut app);
            app.insert_resource(crate::inventory::load_item_registry().unwrap());
            app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());
            if player_faction_reputation.is_some() {
                app.insert_resource(ZoneRegistry::load_from_path(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"),
                ));
            }

            let (client_bundle, helper) = create_mock_client("Azure");
            let player = app
                .world_mut()
                .spawn((client_bundle, player_inventory))
                .id();
            app.world_mut()
                .entity_mut(player)
                .insert(Position::new(position));
            if let Some(player_faction_reputation) = player_faction_reputation {
                app.world_mut()
                    .entity_mut(player)
                    .insert(player_faction_reputation);
            }
            let npc = app
                .world_mut()
                .spawn((
                    NpcMarker,
                    EntityKind::VILLAGER,
                    EntityId::default(),
                    Position::new(position + DVec3::new(1.0, 0.0, 0.0)),
                    OldPosition::new(position + DVec3::new(1.0, 0.0, 0.0)),
                    NpcArchetype::Commoner,
                ))
                .id();
            if let Some(trade_inventory) = trade_inventory {
                app.world_mut().entity_mut(npc).insert(trade_inventory);
            }
            if let Some(npc_player_reputation) = npc_player_reputation {
                app.world_mut()
                    .entity_mut(npc)
                    .insert(npc_player_reputation);
            }
            if let Some(npc_membership) = npc_membership {
                app.world_mut().entity_mut(npc).insert(npc_membership);
            }

            app.update();
            let npc_entity_id = app
                .world()
                .get::<EntityId>(npc)
                .expect("EntityPlugin must assign protocol id to NPC")
                .get();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: player,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::NpcTradeRequest {
                        v: 1,
                        npc_entity_id,
                        offered_items: Vec::new(),
                        requested_item_id: requested_item_id.to_string(),
                    })
                    .expect("npc trade request should serialize")
                    .into_boxed_slice(),
                });

            app.update();
            flush_all_client_packets(&mut app);
            (app, player, helper)
        }

        fn live_trade_offer(
            template_id: &str,
            display_name: &str,
            count: u32,
            price_bone_coins: u32,
        ) -> crate::npc::trade::TradeOffer {
            crate::npc::trade::TradeOffer {
                template_id: template_id.to_string(),
                display_name: display_name.to_string(),
                count,
                price_bone_coins,
            }
        }

        fn inventory_item_count(inventory: &PlayerInventory, template_id: &str) -> u32 {
            inventory
                .containers
                .iter()
                .flat_map(|container| container.items.iter())
                .filter(|placed| placed.instance.template_id == template_id)
                .map(|placed| placed.instance.stack_count)
                .sum()
        }

        #[test]
        fn npc_trade_request_grants_full_bundle_count() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 100;
            let (app, player, _helper) = run_npc_trade_request(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 12)],
                }),
                "spirit_grass",
            );

            let inventory = app
                .world()
                .get::<PlayerInventory>(player)
                .expect("trade should keep player inventory attached");
            assert_eq!(
                inventory_item_count(inventory, "spirit_grass"),
                3,
                "one accepted bundle offer must grant its full live count"
            );
            assert_eq!(
                inventory.bone_coins, 88,
                "one accepted bundle offer must deduct its live total price exactly once"
            );
        }

        #[test]
        fn npc_trade_request_single_count_offer_still_grants_one() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 100;
            let (app, player, _helper) = run_npc_trade_request(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer("spirit_grass", "灵草", 1, 12)],
                }),
                "spirit_grass",
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(inventory_item_count(inventory, "spirit_grass"), 1);
            assert_eq!(inventory.bone_coins, 88);
        }

        #[test]
        fn npc_trade_request_rejects_offer_not_present_in_live_inventory() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 100;
            let original_revision = inventory.revision;
            let (app, player, mut helper) = run_npc_trade_request(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer(
                        "ling_xi_wan_flawed",
                        "灵息丸（次品）",
                        2,
                        8,
                    )],
                }),
                "spirit_grass",
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(inventory_item_count(inventory, "spirit_grass"), 0);
            assert_eq!(inventory.bone_coins, 100);
            assert_eq!(inventory.revision, original_revision);
            let messages = collect_game_messages(&mut helper);
            assert!(
                messages
                    .iter()
                    .any(|message| message.contains("当前没有这件货")),
                "live subset rejection must be visible to the player, messages={messages:?}"
            );
            assert!(
                messages.iter().all(|message| !message.contains("买下")),
                "live subset rejection must not emit success feedback, messages={messages:?}"
            );
        }

        #[test]
        fn npc_trade_request_rejects_empty_live_inventory_without_side_effects() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 100;
            inventory.revision = InventoryRevision(8);
            let (app, player, mut helper) = run_npc_trade_request(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory { offers: vec![] }),
                "spirit_grass",
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(inventory_item_count(inventory, "spirit_grass"), 0);
            assert_eq!(inventory.bone_coins, 100);
            assert_eq!(inventory.revision, InventoryRevision(8));
            let messages = collect_game_messages(&mut helper);
            assert!(
            messages
                .iter()
                .any(|message| message.contains("当前没有这件货")),
            "empty live inventory must use the visible missing-offer rejection, messages={messages:?}"
        );
            assert!(
                messages.iter().all(|message| !message.contains("买下")),
                "empty live inventory must not emit success feedback, messages={messages:?}"
            );
        }

        #[test]
        fn npc_trade_request_uses_live_offer_count_not_catalogue_default() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 100;
            let (app, player, _helper) = run_npc_trade_request(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer("spirit_grass", "灵草", 5, 10)],
                }),
                "lingcao",
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(inventory_item_count(inventory, "spirit_grass"), 5);
            assert_eq!(inventory.bone_coins, 90);
        }

        #[test]
        fn npc_trade_request_success_message_includes_bundle_count() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 100;
            let (_app, _player, mut helper) = run_npc_trade_request(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 12)],
                }),
                "spirit_grass",
            );
            let messages = collect_game_messages(&mut helper);
            assert!(
                messages.iter().any(|message| message.contains("灵草 x3")),
                "success feedback must expose the granted bundle count, messages={messages:?}"
            );
        }

        #[test]
        fn npc_trade_request_uses_live_offer_total_price() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 100;
            let (app, player, _helper) = run_npc_trade_request(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 17)],
                }),
                "spirit_grass",
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(inventory_item_count(inventory, "spirit_grass"), 3);
            assert_eq!(
                inventory.bone_coins, 83,
                "live offer price is the bundle total and must override catalogue price"
            );
        }

        #[test]
        fn npc_trade_request_applies_non_neutral_reputation_to_live_bundle_total() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 18;
            let mut reputation = NpcPlayerReputation::default();
            reputation.adjust("offline:Azure", 0.3);
            let (app, player, _helper) = run_npc_trade_request_with_reputation(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 20)],
                }),
                "spirit_grass",
                Some(reputation),
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(inventory_item_count(inventory, "spirit_grass"), 3);
            assert_eq!(
            inventory.bone_coins, 0,
            "high reputation discount must apply to live total 20 (current ceil result 18), not catalogue 10"
        );
        }

        #[test]
        fn npc_trade_request_applies_faction_reputation_to_live_bundle_total() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 18;
            let mut faction_reputation = FactionReputation::default();
            faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 51);
            let (app, player, _helper) = run_npc_trade_request_with_context(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 20)],
                }),
                "spirit_grass",
                None,
                Some(faction_reputation),
                DVec3::new(-3000.0, 120.0, -2000.0),
                Some(neutral_faction_membership()),
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(inventory_item_count(inventory, "spirit_grass"), 3);
            assert_eq!(
                inventory.bone_coins, 0,
                "Qingyun high faction reputation must discount live total 20, not catalogue 10"
            );
        }

        #[test]
        fn npc_trade_request_rejects_when_only_catalogue_price_is_affordable() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 11;
            inventory.revision = InventoryRevision(13);
            let (app, player, mut helper) = run_npc_trade_request(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 12)],
                }),
                "spirit_grass",
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(
                inventory_item_count(inventory, "spirit_grass"),
                0,
                "affording the catalogue price must not grant any part of a dearer live bundle"
            );
            assert_eq!(inventory.bone_coins, 11);
            assert_eq!(inventory.revision, InventoryRevision(13));
            let messages = collect_game_messages(&mut helper);
            assert!(
                messages
                    .iter()
                    .any(|message| message.contains("骨币不足，需要 12 枚")),
                "rejection must expose the live bundle total, messages={messages:?}"
            );
            assert!(
                messages.iter().all(|message| !message.contains("买下")),
                "an unaffordable live bundle must not emit success feedback, messages={messages:?}"
            );
        }

        #[test]
        fn npc_trade_request_rejects_missing_trade_inventory_without_side_effects() {
            let mut inventory = empty_inventory();
            inventory.bone_coins = 100;
            inventory.revision = InventoryRevision(7);
            let (app, player, mut helper) = run_npc_trade_request(inventory, None, "spirit_grass");
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(inventory_item_count(inventory, "spirit_grass"), 0);
            assert_eq!(inventory.bone_coins, 100);
            assert_eq!(inventory.revision, InventoryRevision(7));
            let messages = collect_game_messages(&mut helper);
            assert!(
                messages
                    .iter()
                    .any(|message| message.contains("当前没有可成交的货物")),
                "missing trade component rejection must be visible, messages={messages:?}"
            );
            assert!(
                messages.iter().all(|message| !message.contains("买下")),
                "missing trade component must not emit success feedback, messages={messages:?}"
            );
        }

        #[test]
        fn npc_trade_request_inventory_failure_keeps_coins_and_revision() {
            let inventory = PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(11),
                containers: Vec::new(),
                equipped: Default::default(),
                hotbar: Default::default(),
                bone_coins: 100,
                max_weight: 50.0,
            };
            let (app, player, _helper) = run_npc_trade_request(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 12)],
                }),
                "spirit_grass",
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(inventory.bone_coins, 100);
            assert_eq!(inventory.revision, InventoryRevision(11));
        }

        #[test]
        fn npc_trade_request_partial_bundle_capacity_fails_atomically() {
            let registry = crate::inventory::load_item_registry().unwrap();
            let mut allocator = crate::inventory::InventoryInstanceIdAllocator::default();
            let mut inventory = empty_inventory();
            add_item_to_player_inventory(
                &mut inventory,
                &registry,
                &mut allocator,
                "spirit_grass",
                63,
                0,
            )
            .expect("test precondition: one compatible spirit grass stack must fit");
            inventory.containers[0].rows = 1;
            inventory.containers[0].cols = 1;
            inventory.bone_coins = 100;
            inventory.revision = InventoryRevision(17);

            let (app, player, mut helper) = run_npc_trade_request(
                inventory,
                Some(crate::npc::trade::NpcTradeInventory {
                    offers: vec![live_trade_offer("spirit_grass", "灵草", 2, 12)],
                }),
                "spirit_grass",
            );
            let inventory = app.world().get::<PlayerInventory>(player).unwrap();
            assert_eq!(
                inventory_item_count(inventory, "spirit_grass"),
                63,
                "a bundle that can merge only one of two items must not partially mutate the stack"
            );
            assert_eq!(inventory.containers[0].items.len(), 1);
            assert_eq!(inventory.bone_coins, 100);
            assert_eq!(inventory.revision, InventoryRevision(17));
            let messages = collect_game_messages(&mut helper);
            assert!(
                messages.iter().any(|message| message.contains("交易失败")),
                "partial-capacity rejection must remain player-visible, messages={messages:?}"
            );
            assert!(messages.iter().all(|message| !message.contains("买下")));
        }

        #[test]
        fn set_meridian_target_sends_generic_meridian_chat_echo() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SetMeridianTarget {
                        v: 1,
                        meridian: MeridianId::Du.channel_id(),
                    })
                    .expect("set meridian target request should serialize")
                    .into_boxed_slice(),
                });

            app.update();
            flush_all_client_packets(&mut app);

            let actual_target = app
                .world()
                .get::<MeridianTarget>(entity)
                .map(|target| target.0.clone());
            assert_eq!(
                actual_target,
                Some(MeridianId::Du.channel_id()),
                "expected SetMeridianTarget to insert selected meridian target, actual={:?}",
                actual_target
            );
            let messages = collect_game_messages(&mut helper);
            assert!(
            messages
                .iter()
                .any(|message| message.contains("[修炼] 已收到经脉目标：督脉。")),
            "expected generic meridian target chat echo because request is not limited to Chong, actual messages={messages:?}"
        );
        }

        /// plan-race-system-v1 P1 对抗审查 M4：`SetMeridianTarget` 消费边界收到未知
        /// channel id（伪造串 / 旧 PascalCase `MeridianId::Lung` 字面量 "Lung"）时必须
        /// 安全处理（回执标注"未知经脉"，`MeridianTarget` component 允许被设置但下游
        /// `meridian_open_tick` 会安全跳过，见该 system 的 debug 分支）——绝不 panic，
        /// 也不能把未知串误当合法经脉给出中文标签回执。
        #[test]
        fn set_meridian_target_with_unknown_channel_id_is_handled_safely_not_panicking() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SetMeridianTarget {
                        v: 1,
                        meridian: crate::cultivation::components::MeridianChannelId::new(
                            "totally_made_up_channel",
                        ),
                    })
                    .expect("set meridian target request should serialize")
                    .into_boxed_slice(),
                });

            // 必须不 panic —— 这是本用例的核心断言。
            app.update();
            flush_all_client_packets(&mut app);

            let messages = collect_game_messages(&mut helper);
            assert!(
                messages.iter().any(|message| message.contains("未知经脉")),
                "unknown channel id 必须回执'未知经脉'占位而不是伪造某个真实经脉名，\
             actual messages={messages:?}"
            );
        }

        /// 旧 PascalCase 字面量 "Lung"（`MeridianId::Lung` 的 `Debug`/枚举名拼写，非合法
        /// wire channel id）同样必须走"未知经脉"安全分支，不能被误认成合法的 lung 经脉。
        #[test]
        fn set_meridian_target_with_legacy_pascal_case_lung_string_is_rejected_as_unknown() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SetMeridianTarget {
                        v: 1,
                        meridian: crate::cultivation::components::MeridianChannelId::new("Lung"),
                    })
                    .expect("set meridian target request should serialize")
                    .into_boxed_slice(),
                });

            app.update();
            flush_all_client_packets(&mut app);

            let messages = collect_game_messages(&mut helper);
            assert!(
                messages.iter().any(|message| message.contains("未知经脉")),
                "旧 PascalCase 'Lung' 不是合法 snake_case wire channel id ('lung')，必须走\
             未知经脉分支而非被误认作肺经，actual messages={messages:?}"
            );
        }

        #[test]
        fn qi_scatter_bead_use_dispatches_zhenfa_event() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.world_mut().resource_mut::<CombatClock>().tick = 33;

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::QiScatterBeadUse {
                        v: 1,
                        item_instance_id: 7001,
                        x: None,
                        y: None,
                        z: None,
                    })
                    .expect("qi scatter bead request should serialize")
                    .into_boxed_slice(),
                });

            app.update();

            let mut events = app
                .world()
                .resource::<Events<ScatterBeadUseRequest>>()
                .iter_current_update_events();
            let event = events
                .next()
                .expect("qi_scatter_bead_use must dispatch ScatterBeadUseRequest");
            assert_eq!(event.player, entity);
            assert_eq!(event.item_instance_id, 7001);
            assert_eq!(event.bury_pos, None);
            assert_eq!(event.requested_at_tick, 33);
            assert!(
                events.next().is_none(),
                "qi_scatter_bead_use should emit exactly one request event"
            );
        }

        #[test]
        fn qi_scatter_bead_use_with_coords_dispatches_burial_pos() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"qi_scatter_bead_use","v":1,"item_instance_id":7002,"x":1,"y":64,"z":-2}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let event = app
                .world()
                .resource::<Events<ScatterBeadUseRequest>>()
                .iter_current_update_events()
                .next()
                .expect("qi_scatter_bead_use with coords must dispatch burial request");
            assert_eq!(event.player, entity);
            assert_eq!(event.item_instance_id, 7002);
            assert_eq!(event.bury_pos, Some([1, 64, -2]));
        }

        #[test]
        fn block_place_payload_dispatches_runtime_request_event() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"block_place","v":1,"x":8,"y":64,"z":8,"item_instance_id":4242,"target_face":"north"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let events = app
                .world()
                .resource::<valence::prelude::Events<BlockPlaceRequest>>();
            let requests = events.iter_current_update_events().collect::<Vec<_>>();
            assert_eq!(
                requests.len(),
                1,
                "expected exactly one BlockPlaceRequest from one valid block_place payload"
            );
            let request = requests[0];
            assert_eq!(request.client, entity);
            assert_eq!((request.x, request.y, request.z), (8, 64, 8));
            assert_eq!(request.item_instance_id, 4242);
            assert_eq!(request.target_face, TrapTargetFace::North);
        }

        // ─── plan-worldgen-v4 P5 §8.1#5 — block_picker_give 路由测试矩阵 ───

        /// 把一段 wire JSON 喂给 handler，返回这一轮 emit 的 BlockPickerGiveIntent 列表。
        fn dispatch_block_picker_give(
            json: &[u8],
        ) -> (
            App,
            valence::prelude::Entity,
            Vec<crate::cmd::dev::block_picker::BlockPickerGiveIntent>,
        ) {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: json.to_vec().into_boxed_slice(),
                });
            app.update();
            let events =
            app.world().resource::<valence::prelude::Events<
                crate::cmd::dev::block_picker::BlockPickerGiveIntent,
            >>();
            let collected = events
                .iter_current_update_events()
                .cloned()
                .collect::<Vec<_>>();
            (app, entity, collected)
        }

        /// happy path + 链路：合法 block_picker_give payload → 恰好 1 次 BlockPickerGiveIntent，
        /// 且 block_id / count / player 字段一路透传。
        #[test]
        fn block_picker_give_payload_dispatches_intent_with_fields() {
            let (_app, entity, intents) = dispatch_block_picker_give(
                br#"{"type":"block_picker_give","v":1,"block_id":"stone_bricks","count":16}"#,
            );
            assert_eq!(
            intents.len(),
            1,
            "一条合法 block_picker_give payload 应 emit 恰好 1 次 BlockPickerGiveIntent，实为 {}",
            intents.len()
        );
            assert_eq!(intents[0].player, entity, "intent 必须带回发起玩家 entity");
            assert_eq!(
                intents[0].block_id, "stone_bricks",
                "block_id 必须透传，实为 {}",
                intents[0].block_id
            );
            assert_eq!(
                intents[0].count, 16,
                "count 必须透传，实为 {}",
                intents[0].count
            );
        }

        fn dispatch_remains_loot(
            json: &[u8],
        ) -> (
            valence::prelude::Entity,
            Vec<crate::inventory::RemainsLootIntent>,
        ) {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: json.to_vec().into_boxed_slice(),
                });
            app.update();
            let events = app
                .world()
                .resource::<valence::prelude::Events<crate::inventory::RemainsLootIntent>>();
            let collected = events
                .iter_current_update_events()
                .cloned()
                .collect::<Vec<_>>();
            (entity, collected)
        }

        #[test]
        fn remains_loot_request_dispatches_intent_with_fields() {
            let (entity, intents) = dispatch_remains_loot(
            br#"{"type":"remains_loot","v":1,"remains_id":"3fa85f64-5717-4562-b3fc-2c963f66afa6"}"#,
        );

            assert_eq!(
                intents.len(),
                1,
                "合法 remains_loot payload 应 emit 恰好 1 次 RemainsLootIntent，实为 {}",
                intents.len()
            );
            assert_eq!(intents[0].entity, entity, "intent 必须带回发起玩家 entity");
            assert_eq!(
                intents[0].remains_id, "3fa85f64-5717-4562-b3fc-2c963f66afa6",
                "remains_id 必须从 wire payload 原样透传"
            );
        }

        #[test]
        fn remains_loot_request_with_blank_id_is_dropped() {
            let (_entity, intents) =
                dispatch_remains_loot(br#"{"type":"remains_loot","v":1,"remains_id":"   "}"#);

            assert!(
                intents.is_empty(),
                "空白 remains_id 应被 handler 拦截，不应 emit RemainsLootIntent；实际 {} 条",
                intents.len()
            );
        }

        /// 边界透传：count=1（下界）与 count=64（上界）合法值都能派发并保值。
        #[test]
        fn block_picker_give_boundary_counts_dispatch() {
            for count in [1u32, 64u32] {
                let json = format!(
                    r#"{{"type":"block_picker_give","v":1,"block_id":"stone","count":{count}}}"#
                );
                let (_app, _entity, intents) = dispatch_block_picker_give(json.as_bytes());
                assert_eq!(
                    intents.len(),
                    1,
                    "count={count} 是合法边界，应 emit 1 次 intent，实为 {}",
                    intents.len()
                );
                assert_eq!(
                    intents[0].count, count,
                    "count={count} 应透传保值，实为 {}",
                    intents[0].count
                );
            }
        }

        /// 错误分支：count=0 / count=65 越界 payload 在 wire serde 层即被拒，handler 不 emit 任何 intent
        /// （schema serde deserialize_block_picker_count 守门，未通过 → 整个 payload 反序列化失败 → drop）。
        #[test]
        fn block_picker_give_out_of_range_count_is_dropped_before_dispatch() {
            for bad in [
                &br#"{"type":"block_picker_give","v":1,"block_id":"stone","count":0}"#[..],
                &br#"{"type":"block_picker_give","v":1,"block_id":"stone","count":65}"#[..],
            ] {
                let (_app, _entity, intents) = dispatch_block_picker_give(bad);
                assert!(
                    intents.is_empty(),
                    "越界 count payload 必须在 serde 层被拒、handler 不派发任何 intent，实为 {} 条",
                    intents.len()
                );
            }
        }

        /// 错误分支：malformed JSON / 未知字段 payload 不得 emit intent（坏包安静丢弃）。
        #[test]
        fn block_picker_give_malformed_payload_is_dropped() {
            for bad in [
            &b"{not json at all"[..],
            // 多了未知字段 surprise，deny_unknown_fields 拒绝。
            &br#"{"type":"block_picker_give","v":1,"block_id":"stone","count":4,"surprise":true}"#
                [..],
            // 缺 block_id 必填字段。
            &br#"{"type":"block_picker_give","v":1,"count":4}"#[..],
        ] {
            let (_app, _entity, intents) = dispatch_block_picker_give(bad);
            assert!(
                intents.is_empty(),
                "malformed / 非法 block_picker_give payload 不得派发 intent，实为 {} 条",
                intents.len()
            );
        }
        }

        #[test]
        fn workbench_open_payload_requires_entity_manager_before_dispatch() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let client = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"workbench_open","v":1,"entity_id":42}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let events = app
                .world()
                .resource::<valence::prelude::Events<crate::craft::WorkbenchOpenRequest>>();
            assert_eq!(
                events.iter_current_update_events().count(),
                0,
                "workbench_open must not fabricate an ECS entity when EntityManager is unavailable"
            );
        }

        #[test]
        fn container_open_payload_requires_entity_manager_before_dispatch() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let client = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"container_open","v":1,"entity_id":42}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let events = app
            .world()
            .resource::<valence::prelude::Events<crate::world::container_open::ContainerOpenRequest>>();
            assert_eq!(
                events.iter_current_update_events().count(),
                0,
                "container_open must not fabricate an ECS entity when EntityManager is unavailable"
            );
        }

        fn assert_movement_action_yaw_forwarded(yaw_degrees: f32) {
            let mut app = App::new();
            register_request_app(&mut app);
            app.add_event::<MovementActionIntent>();

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let payload = format!(
                r#"{{"type":"movement_action","v":1,"action":"dash","yaw_degrees":{yaw_degrees}}}"#
            );
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: payload.into_bytes().into_boxed_slice(),
                });

            app.update();

            let events = app
                .world()
                .resource::<valence::prelude::Events<MovementActionIntent>>();
            let intents: Vec<_> = events.iter_current_update_events().collect();
            assert_eq!(
            intents.len(),
            1,
            "expected one MovementActionIntent because movement_action yaw payload was valid, actual: {}",
            intents.len()
        );
            assert_eq!(
            intents[0].entity, entity,
            "expected MovementActionIntent entity to match sending client for yaw_degrees={yaw_degrees}"
        );
            assert_eq!(
            intents[0].action,
            MovementAction::Dashing,
            "expected movement_action dash payload to map to MovementAction::Dashing for yaw_degrees={yaw_degrees}"
        );
            assert_eq!(
                intents[0].yaw_degrees,
                Some(yaw_degrees),
                "expected server to forward numeric yaw_degrees unchanged, actual intent: {:?}",
                intents[0]
            );
        }

        #[test]
        fn alchemy_inject_qi_ignored_for_furnace_in_collapsed_zone() {
            let mut app = App::new();
            register_request_app(&mut app);
            let mut zones = ZoneRegistry::fallback();
            zones
                .find_zone_mut("spawn")
                .unwrap()
                .active_events
                .push(EVENT_REALM_COLLAPSE.to_string());
            app.insert_resource(zones);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(8, 66, 8), 1);
            furnace.owner = Some("offline:Azure".into());
            furnace.session = Some(AlchemySession::new(
                "kai_mai_pill_v0".into(),
                "offline:Azure".into(),
            ));
            let furnace_entity = app.world_mut().spawn(furnace).id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"alchemy_intervention","v":1,"furnace_pos":[8,66,8],"intervention":{"kind":"inject_qi","qi":5.0}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let furnace = app.world().get::<AlchemyFurnace>(furnace_entity).unwrap();
            assert_eq!(furnace.session.as_ref().unwrap().qi_injected, 0.0);
        }

        #[test]
        fn alchemy_flawed_take_back_grants_flawed_pill_residue() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
            app.insert_resource(crate::inventory::load_item_registry().unwrap());
            app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(entity).insert((
                crate::cultivation::components::Cultivation::default(),
                PlayerState::default(),
                inventory_with_stack("ci_she_hao", 3),
            ));

            let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(3, 64, 4), 1);
            furnace.owner = Some("offline:Azure".into());
            app.world_mut().spawn(furnace);
            for data in [
            br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[3,64,4],"recipe_id":"kai_mai_pill_v0"}"#.as_slice(),
            br#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[3,64,4],"slot_idx":0,"material":"ci_she_hao","count":3}"#.as_slice(),
            br#"{"type":"alchemy_intervention","v":1,"furnace_pos":[3,64,4],"intervention":{"kind":"inject_qi","qi":15.0}}"#.as_slice(),
            br#"{"type":"alchemy_intervention","v":1,"furnace_pos":[3,64,4],"intervention":{"kind":"adjust_temp","temp":0.60}}"#.as_slice(),
            br#"{"type":"alchemy_take_back","v":1,"furnace_pos":[3,64,4],"slot_idx":0}"#.as_slice(),
        ] {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: data.to_vec().into_boxed_slice(),
                });
        }

            app.update();

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            let item_summary: Vec<_> = inventory
                .containers
                .iter()
                .flat_map(|container| container.items.iter())
                .map(|placed| {
                    format!(
                        "{}:{:?}",
                        placed.instance.template_id, placed.instance.alchemy
                    )
                })
                .collect();
            assert!(
                inventory.containers.iter().any(|container| {
                    container.items.iter().any(|placed| {
                        placed.instance.template_id
                            == crate::alchemy::residue::FLAWED_PILL_RESIDUE_TEMPLATE_ID
                            && matches!(
                                placed.instance.alchemy,
                                Some(AlchemyItemData::PillResidue {
                                    residue_kind:
                                        crate::alchemy::residue::PillResidueKind::FlawedPill,
                                    ..
                                })
                            )
                    })
                }),
                "expected flawed pill residue in inventory, got {item_summary:?}"
            );
        }

        #[test]
        fn alchemy_feed_slot_rejects_wrong_mineral_instance_on_live_request_path() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut wrong_mineral = inventory_test_item(9002, "dan_sha_aux", 1);
            wrong_mineral.display_name = "假丹砂辅料".to_string();
            wrong_mineral.mineral_id = Some("zhu_sha".to_string());
            app.world_mut().entity_mut(entity).insert((
                crate::cultivation::components::Cultivation::default(),
                PlayerState::default(),
                PlayerInventory {
                    revision: InventoryRevision(0),
                    containers: vec![ContainerState {
                        quick_access: false,
                        id: "main_pack".into(),
                        name: "main_pack".into(),
                        rows: 5,
                        cols: 7,
                        items: vec![
                            PlacedItemState {
                                row: 0,
                                col: 0,
                                instance: inventory_test_item(9001, "ci_she_hao", 2),
                            },
                            PlacedItemState {
                                row: 0,
                                col: 1,
                                instance: wrong_mineral,
                            },
                        ],

                        owner_instance_id: None,
                    }],
                    equipped: Default::default(),
                    hotbar: Default::default(),
                    triggered_treasures: Vec::new(),
                    bone_coins: 0,
                    max_weight: 50.0,
                },
            ));

            let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(5, 64, 6), 1);
            furnace.owner = Some("offline:Azure".into());
            let furnace_entity = app.world_mut().spawn(furnace).id();
            for data in [
            br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[5,64,6],"recipe_id":"jie_du_dan_v1"}"#.as_slice(),
            br#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[5,64,6],"slot_idx":0,"material":"ci_she_hao","count":2}"#.as_slice(),
            br#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[5,64,6],"slot_idx":0,"material":"dan_sha_aux","count":1}"#.as_slice(),
        ] {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: data.to_vec().into_boxed_slice(),
                });
        }

            app.update();
            flush_all_client_packets(&mut app);

            let furnace = app.world().get::<AlchemyFurnace>(furnace_entity).unwrap();
            let staged = &furnace.session.as_ref().unwrap().staged.materials;
            let staged_ci_she_hao = staged.get("ci_she_hao").copied();
            assert_eq!(
            staged_ci_she_hao,
            Some(2),
            "expected ci_she_hao×2 to stay staged because the first feed request succeeded before wrong mineral rejection, actual staged={staged:?}"
        );
            assert!(
                !staged.contains_key("dan_sha_aux"),
                "wrong mineral_id must not satisfy dan_sha_aux ingredient: {staged:?}"
            );
            let messages = collect_game_messages(&mut helper);
            assert!(
            messages
                .iter()
                .any(|message| message.contains("材料不足或矿物不符")),
            "expected wrong-mineral live request to send alchemy rejection chat, actual messages={messages:?}"
        );
            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            assert!(
                inventory.containers.iter().any(|container| {
                    container.items.iter().any(|placed| {
                        placed.instance.instance_id == 9002 && placed.instance.stack_count == 1
                    })
                }),
                "rejected wrong-mineral item must remain in inventory"
            );
        }

        #[test]
        fn alchemy_ignite_rejects_low_zone_qi_on_live_request_path() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
            app.insert_resource(crate::inventory::load_item_registry().unwrap());
            app.insert_resource(crate::world::zone::ZoneRegistry {
                spatial_revision: 0,
                zones: vec![crate::world::zone::Zone {
                    name: "spawn".to_string(),
                    dimension: DimensionKind::Overworld,
                    bounds: (
                        valence::prelude::DVec3::new(0.0, 0.0, 0.0),
                        valence::prelude::DVec3::new(10.0, 100.0, 10.0),
                    ),
                    spirit_qi: 0.0,
                    danger_level: 0,
                    active_events: Vec::new(),
                    patrol_anchors: Vec::new(),
                    blocked_tiles: Vec::new(),
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
                }],
            });

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(2, 64, 3), 1);
            furnace.owner = Some("offline:Azure".into());
            let furnace_entity = app.world_mut().spawn(furnace).id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[2,64,3],"recipe_id":"kai_mai_pill_v0"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let furnace = app.world().get::<AlchemyFurnace>(furnace_entity).unwrap();
            assert!(furnace.session.is_none());
        }

        #[test]
        fn brew_emits_vapor() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(2, 64, 3), 1);
            furnace.owner = Some("offline:Azure".into());
            let furnace_entity = app.world_mut().spawn(furnace).id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[2,64,3],"recipe_id":"kai_mai_pill_v0"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            assert!(app
                .world()
                .get::<AlchemyFurnace>(furnace_entity)
                .unwrap()
                .session
                .is_some());
            let events = app
                .world()
                .resource::<valence::prelude::Events<VfxEventRequest>>();
            let emitted = events
                .iter_current_update_events()
                .next()
                .expect("alchemy ignite should emit vapor vfx");
            match &emitted.payload {
                crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                    assert_eq!(event_id, gameplay_vfx::ALCHEMY_BREW_VAPOR);
                }
                other => panic!("expected SpawnParticle, got {other:?}"),
            }
        }

        // ── plan-skill-av-relink-v1 P3 —— alchemy_stir 内联 emit pin ─────────────────

        fn drain_alchemy_stir_anims(app: &mut App) -> Vec<(String, u16)> {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<VfxEventRequest>>()
                .drain()
                .filter_map(|request| match request.payload {
                    crate::schema::vfx_event::VfxEventPayloadV1::PlayAnim {
                        target_player,
                        anim_id,
                        priority,
                        ..
                    } if anim_id == "bong:alchemy_stir" => Some((target_player, priority)),
                    _ => None,
                })
                .collect()
        }

        fn spawn_azure_furnace_with_session(
            app: &mut App,
            owner: &str,
        ) -> valence::prelude::Entity {
            let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(8, 66, 8), 1);
            furnace.owner = Some(owner.into());
            furnace.session = Some(AlchemySession::new("kai_mai_pill_v0".into(), owner.into()));
            app.world_mut().spawn(furnace).id()
        }

        fn send_alchemy_intervention_payload(
            app: &mut App,
            client: valence::prelude::Entity,
            intervention_json: &str,
        ) {
            let data = format!(
                r#"{{"type":"alchemy_intervention","v":1,"furnace_pos":[8,66,8],"intervention":{intervention_json}}}"#
            );
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: data.into_bytes().into_boxed_slice(),
                });
        }

        /// happy path：炉主对起炉中的丹炉干预生效 → 恰发一条 alchemy_stir 搅拌动画，
        /// target = 干预者本人 uuid、优先级战斗动作档。
        #[test]
        fn alchemy_intervention_emits_stir_animation_for_owner() {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let player_uuid = app
                .world()
                .get::<UniqueId>(entity)
                .expect("mock client should carry UniqueId")
                .0
                .to_string();
            spawn_azure_furnace_with_session(&mut app, "offline:Azure");

            send_alchemy_intervention_payload(
                &mut app,
                entity,
                r#"{"kind":"adjust_temp","temp":0.5}"#,
            );
            app.update();

            let stirs = drain_alchemy_stir_anims(&mut app);
            assert_eq!(
                stirs.len(),
                1,
                "干预生效应恰发一条 alchemy_stir 搅拌动画，实际 {stirs:?}"
            );
            assert_eq!(
                stirs[0].0, player_uuid,
                "alchemy_stir 应发给干预者本人（target_player = 干预者 uuid）"
            );
            assert_eq!(stirs[0].1, 1000, "alchemy_stir 优先级应为战斗动作档");
        }

        /// 重复触发语义：每次干预生效各配一次搅拌动画（两次干预两动画，1:1 无去重）。
        #[test]
        fn each_alchemy_intervention_emits_its_own_stir() {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            spawn_azure_furnace_with_session(&mut app, "offline:Azure");

            send_alchemy_intervention_payload(
                &mut app,
                entity,
                r#"{"kind":"adjust_temp","temp":0.5}"#,
            );
            send_alchemy_intervention_payload(&mut app, entity, r#"{"kind":"inject_qi","qi":2.0}"#);
            app.update();

            assert_eq!(
                drain_alchemy_stir_anims(&mut app).len(),
                2,
                "每次干预生效各配一次 alchemy_stir（1:1）"
            );
        }

        /// enum 变体饱和：AutoProfile 是保留 no-op（`apply_intervention` 不改任何
        /// 状态、无真实搅拌动作），不发 alchemy_stir 动画。
        #[test]
        fn auto_profile_intervention_emits_no_stir_animation() {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            spawn_azure_furnace_with_session(&mut app, "offline:Azure");

            send_alchemy_intervention_payload(
                &mut app,
                entity,
                r#"{"kind":"auto_profile","profile_id":"gentle"}"#,
            );
            app.update();

            assert!(
                drain_alchemy_stir_anims(&mut app).is_empty(),
                "AutoProfile 是保留 no-op 干预（不改炉温/真元），不应发 alchemy_stir 搅拌动画"
            );
        }

        /// 错误分支：尚未起炉（furnace 无 session）→ 干预被拒不发搅拌动画。
        #[test]
        fn alchemy_intervention_without_session_does_not_emit_stir() {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(8, 66, 8), 1);
            furnace.owner = Some("offline:Azure".into());
            app.world_mut().spawn(furnace);

            send_alchemy_intervention_payload(
                &mut app,
                entity,
                r#"{"kind":"adjust_temp","temp":0.5}"#,
            );
            app.update();

            assert!(
                drain_alchemy_stir_anims(&mut app).is_empty(),
                "未起炉的干预被拒时不应发 alchemy_stir"
            );
        }

        /// 错误分支：非炉主干预他人丹炉 → 路由拒绝不发搅拌动画。
        #[test]
        fn alchemy_intervention_on_foreign_furnace_does_not_emit_stir() {
            let mut app = App::new();
            register_request_app(&mut app);
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            spawn_azure_furnace_with_session(&mut app, "offline:Bob");

            send_alchemy_intervention_payload(
                &mut app,
                entity,
                r#"{"kind":"adjust_temp","temp":0.5}"#,
            );
            app.update();

            assert!(
                drain_alchemy_stir_anims(&mut app).is_empty(),
                "非炉主的干预被拒时不应发 alchemy_stir"
            );
        }

        /// 状态前置分支：坍缩 zone 内 inject_qi 被忽略（干预未生效）→ 不发搅拌动画。
        #[test]
        fn alchemy_inject_qi_in_collapsed_zone_does_not_emit_stir() {
            let mut app = App::new();
            register_request_app(&mut app);
            let mut zones = ZoneRegistry::fallback();
            zones
                .find_zone_mut("spawn")
                .unwrap()
                .active_events
                .push(EVENT_REALM_COLLAPSE.to_string());
            app.insert_resource(zones);
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            spawn_azure_furnace_with_session(&mut app, "offline:Azure");

            send_alchemy_intervention_payload(&mut app, entity, r#"{"kind":"inject_qi","qi":5.0}"#);
            app.update();

            assert!(
                drain_alchemy_stir_anims(&mut app).is_empty(),
                "坍缩 zone 内被忽略的 inject_qi 不应发 alchemy_stir（干预未生效）"
            );
        }

        #[test]
        fn alchemy_explode_backlash_without_components_does_not_crash() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
            app.insert_resource(crate::inventory::load_item_registry().unwrap());
            app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());

            let (client_bundle, _helper) = create_mock_client("NpcLike");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .entity_mut(entity)
                .insert(inventory_with_stack("ci_she_hao", 3));
            let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(4, 64, 5), 1);
            furnace.owner = Some("offline:NpcLike".into());
            app.world_mut().spawn(furnace);
            for data in [
            br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[4,64,5],"recipe_id":"kai_mai_pill_v0"}"#.as_slice(),
            br#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[4,64,5],"slot_idx":0,"material":"ci_she_hao","count":3}"#.as_slice(),
            br#"{"type":"alchemy_intervention","v":1,"furnace_pos":[4,64,5],"intervention":{"kind":"adjust_temp","temp":1.0}}"#.as_slice(),
            br#"{"type":"alchemy_take_back","v":1,"furnace_pos":[4,64,5],"slot_idx":0}"#.as_slice(),
        ] {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: data.to_vec().into_boxed_slice(),
                });
        }

            app.update();

            assert!(app.world().get::<Wounds>(entity).is_none());
        }

        /// P3 端到端：高阶炉（tier=4）炼 tui_gu_dan_v1 → AlchemyTakeBack → bucket=Perfect；
        /// 低阶对照（tier=2，满足最低炉阶但无催化加成）→ bucket=Good。
        ///
        /// 覆盖边界：handler 中 `furnace.tier` 被透传至 `resolve_with_meta_and_furnace`，
        /// 确保"改 furnace tier → resolver 接收到正确 tier → 分桶变化"这条 wiring 不被悄悄断掉。
        /// （若改为传 0，tier-4 结果仍等于 tier-2 的 Good，测试立即红。）
        #[test]
        fn p3_take_back_high_tier_furnace_upgrades_bucket_vs_tier0_control() {
            // tui_gu_dan_v1: target_temp=0.70, temp_band=0.08, qi_cost=25.0, duration=200
            // temp=0.87 → over=0.17, score=(0.17/0.08 - 1.0)=1.125 → Good（无加成）
            // tier=4 催化炉加成 → score 下降到 ≤1.0 → Perfect
            use crate::alchemy::outcome::OutcomeBucket;

            fn build_tui_gu_dan_app() -> (App, valence::prelude::Entity, valence::prelude::Entity) {
                let mut app = App::new();
                register_request_app(&mut app);
                app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
                app.insert_resource(crate::inventory::load_item_registry().unwrap());
                app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());

                let (client_bundle, _helper) = create_mock_client("Alchemist");
                let entity = app.world_mut().spawn(client_bundle).id();
                app.world_mut().entity_mut(entity).insert((
                    crate::cultivation::components::Cultivation::default(),
                    PlayerState::default(),
                    // tui_gu_dan 需要 tui_gu_teng×2 + fauna.mutated_bone×1
                    PlayerInventory {
                        triggered_treasures: Vec::new(),
                        revision: InventoryRevision(0),
                        containers: vec![ContainerState {
                            quick_access: false,
                            id: "main_pack".into(),
                            name: "main_pack".into(),
                            rows: 5,
                            cols: 7,
                            items: vec![
                                PlacedItemState {
                                    row: 0,
                                    col: 0,
                                    instance: ItemInstance {
                                        instance_id: 9001,
                                        template_id: "tui_gu_teng".to_string(),
                                        display_name: "tui_gu_teng".to_string(),
                                        grid_w: 1,
                                        grid_h: 1,
                                        weight: 0.1,
                                        rarity: ItemRarity::Common,
                                        description: String::new(),
                                        stack_count: 2,
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
                                    },
                                },
                                PlacedItemState {
                                    row: 0,
                                    col: 1,
                                    instance: ItemInstance {
                                        instance_id: 9002,
                                        template_id: "fauna.mutated_bone".to_string(),
                                        display_name: "fauna.mutated_bone".to_string(),
                                        grid_w: 1,
                                        grid_h: 1,
                                        weight: 0.1,
                                        rarity: ItemRarity::Common,
                                        description: String::new(),
                                        stack_count: 1,
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
                                    },
                                },
                            ],

                            owner_instance_id: None,
                        }],
                        equipped: Default::default(),
                        hotbar: Default::default(),
                        bone_coins: 0,
                        max_weight: 50.0,
                    },
                ));
                let furnace_entity = app.world_mut().spawn_empty().id();
                (app, entity, furnace_entity)
            }

            fn run_tui_gu_dan_brew(
                app: &mut App,
                entity: valence::prelude::Entity,
                furnace_pos: [i32; 3],
                furnace_tier: u8,
            ) -> OutcomeBucket {
                // 注册炉体
                let mut furnace = AlchemyFurnace::placed(
                    valence::prelude::BlockPos::new(furnace_pos[0], furnace_pos[1], furnace_pos[2]),
                    furnace_tier,
                );
                furnace.owner = Some("offline:Alchemist".into());
                app.world_mut().spawn(furnace);

                let pos_json =
                    format!("[{},{},{}]", furnace_pos[0], furnace_pos[1], furnace_pos[2]);
                let requests: Vec<String> = vec![
                    format!(
                        r#"{{"type":"alchemy_ignite","v":1,"furnace_pos":{pos_json},"recipe_id":"tui_gu_dan_v1"}}"#
                    ),
                    // stage 0: tui_gu_teng×2
                    format!(
                        r#"{{"type":"alchemy_feed_slot","v":1,"furnace_pos":{pos_json},"slot_idx":0,"material":"tui_gu_teng","count":2}}"#
                    ),
                    // stage 0: fauna.mutated_bone×1
                    format!(
                        r#"{{"type":"alchemy_feed_slot","v":1,"furnace_pos":{pos_json},"slot_idx":0,"material":"fauna.mutated_bone","count":1}}"#
                    ),
                    // temp=0.87 → score=1.125 (Good without bonus; Perfect with tier-4 bonus)
                    format!(
                        r#"{{"type":"alchemy_intervention","v":1,"furnace_pos":{pos_json},"intervention":{{"kind":"adjust_temp","temp":0.87}}}}"#
                    ),
                    // qi_cost=25.0 → inject full amount
                    format!(
                        r#"{{"type":"alchemy_intervention","v":1,"furnace_pos":{pos_json},"intervention":{{"kind":"inject_qi","qi":25.0}}}}"#
                    ),
                    format!(
                        r#"{{"type":"alchemy_take_back","v":1,"furnace_pos":{pos_json},"slot_idx":0}}"#
                    ),
                ];
                for req in &requests {
                    app.world_mut()
                        .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                        .send(CustomPayloadEvent {
                            client: entity,
                            channel: ident!("bong:client_request").into(),
                            data: req.as_bytes().to_vec().into_boxed_slice(),
                        });
                }
                app.update();

                // 读取 AlchemyOutcomeEvent 中的 bucket
                let events = app
                    .world()
                    .resource::<valence::prelude::Events<crate::alchemy::AlchemyOutcomeEvent>>();
                let mut reader = events.get_reader();
                let evts: Vec<_> = reader.read(events).collect();
                assert!(
                !evts.is_empty(),
                "furnace_tier={furnace_tier}: AlchemyTakeBack 应产生 AlchemyOutcomeEvent，但未收到任何事件"
            );
                evts.last().unwrap().bucket
            }

            // --- tier=2 对照组（无加成 → Good） ---
            // tui_gu_dan_v1 的 furnace_tier_min=2，tier=2 满足最低炉阶要求但不触发催化加成。
            // catalyst_furnace_bonus 仅在 tier >= CATALYST_FURNACE_TIER(4) 时返回正值，
            // 故 tier=2 等价于"无加成"基线。
            let (mut app2, entity2, _) = build_tui_gu_dan_app();
            let bucket_tier2 = run_tui_gu_dan_brew(&mut app2, entity2, [10, 64, 10], 2);
            assert_eq!(
                bucket_tier2,
                OutcomeBucket::Good,
                "tier=2 炉 + tui_gu_dan_v1(temp=0.87) 应为 Good（无催化加成），实际 {:?}。\
             若非 Good，说明 session 参数或配方数据发生变化，需更新测试基线。",
                bucket_tier2
            );

            // --- tier=4 高阶炉（催化加成 → Perfect） ---
            let (mut app4, entity4, _) = build_tui_gu_dan_app();
            let bucket_tier4 = run_tui_gu_dan_brew(&mut app4, entity4, [20, 64, 20], 4);
            assert_eq!(
            bucket_tier4,
            OutcomeBucket::Perfect,
            "tier=4 炉 + 变异丹 tui_gu_dan_v1(temp=0.87) 应升格到 Perfect，实际 {:?}。\
             若仍是 Good，说明 handle_alchemy_take_back 未将 furnace.tier 透传给 resolver（wiring 断裂）。",
            bucket_tier4
        );

            // 核心断言：高阶炉结果优于低阶炉，证明 furnace.tier wiring 有效
            assert_ne!(
                bucket_tier4, bucket_tier2,
                "tier=4 与 tier=2 的结果应不同（前者 Perfect，后者 Good），\
             若相同说明 furnace.tier 没有被传入 resolver"
            );
        }

        #[test]
        fn unsupported_client_request_version_is_ignored_without_side_effects() {
            let mut app = App::new();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedBreakthroughRequests::default());
            app.insert_resource(CapturedForgeRequests::default());
            app.insert_resource(CapturedInsightChoices::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<RevivalActionIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_systems(
                Update,
                (
                    handle_client_request_payloads,
                    capture_breakthrough_requests,
                    capture_forge_requests,
                    capture_insight_choices,
                )
                    .chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"breakthrough_request","v":99}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            assert!(
                app.world().get::<MeridianTarget>(entity).is_none(),
                "unsupported request version should not attach MeridianTarget"
            );
            assert!(
                app.world()
                    .resource::<CapturedBreakthroughRequests>()
                    .0
                    .is_empty(),
                "unsupported request version should not emit BreakthroughRequest"
            );
            assert!(
                app.world().resource::<CapturedForgeRequests>().0.is_empty(),
                "unsupported request version should not emit ForgeRequest"
            );
            assert!(
                app.world()
                    .resource::<CapturedInsightChoices>()
                    .0
                    .is_empty(),
                "unsupported request version should not emit InsightChosen"
            );
        }

        #[test]
        fn ingress_budget_rejects_33rd_same_tick_before_decode_or_dispatch() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(CapturedBreakthroughRequests::default());
            app.add_systems(
                Update,
                capture_breakthrough_requests.after(handle_client_request_payloads),
            );

            let (client_bundle, _helper) = create_mock_client("BudgetIngress");
            let client = app.world_mut().spawn(client_bundle).id();
            for _ in 0..33 {
                app.world_mut()
                    .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                    .send(CustomPayloadEvent {
                        client,
                        channel: ident!("bong:client_request").into(),
                        data: format!(
                            "{}{{\"type\":\"breakthrough_request\",\"v\":1}}",
                            "\n".repeat(128)
                        )
                        .into_bytes()
                        .into_boxed_slice(),
                    });
            }

            app.update();

            assert_eq!(
                app.world()
                    .resource::<CapturedBreakthroughRequests>()
                    .0
                    .len(),
                32,
                "the 33rd same-tick payload must not dispatch a handler event"
            );
            assert_eq!(
            app.world()
                .resource::<CapturedBreakthroughRequests>()
                .0
                .len(),
            32,
            "the pre-decode ingress budget must reject payload #33 before it can enter dispatch"
        );
        }

        #[test]
        fn ingress_budget_clears_bucket_when_character_role_changes() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.add_systems(Update, cleanup_client_request_budget);

            let (client_bundle, _helper) = create_mock_client("RoleSwitch");
            let client = app
                .world_mut()
                .spawn((
                    client_bundle,
                    Lifecycle {
                        character_id: "character-a".to_string(),
                        ..Lifecycle::default()
                    },
                ))
                .id();
            app.update();

            {
                let mut budget = app.world_mut().resource_mut::<ClientRequestBudget>();
                for _ in 0..32 {
                    assert!(budget.store.admit_ingress(client, 0).admitted);
                }
                assert_eq!(budget.store.tokens_for(&client), Some(0));
            }

            app.world_mut()
                .get_mut::<Lifecycle>(client)
                .expect("connected client must retain lifecycle")
                .character_id = "character-b".to_string();
            app.update();

            let budget = app.world().resource::<ClientRequestBudget>();
            assert_eq!(
                app.world()
                    .get::<Lifecycle>(client)
                    .expect("switched client must retain lifecycle")
                    .character_id,
                "character-b",
                "role switch must be applied before the next ingress bucket is admitted"
            );
            assert_eq!(
                budget.store.tokens_for(&client),
                None,
                "role switch must discard the old entity bucket before the next ingress"
            );
            assert!(
                app.world_mut()
                    .resource_mut::<ClientRequestBudget>()
                    .store
                    .admit_ingress(client, 0)
                    .admitted,
                "a switched role must receive a clean 32-token bucket"
            );
        }

        #[test]
        fn ingress_budget_clears_bucket_when_client_disconnects() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.add_systems(Update, cleanup_client_request_budget);

            let (client_bundle, _helper) = create_mock_client("Disconnect");
            let client = app
                .world_mut()
                .spawn((
                    client_bundle,
                    Lifecycle {
                        character_id: "character-a".to_string(),
                        ..Lifecycle::default()
                    },
                ))
                .id();
            app.update();
            app.world_mut()
                .resource_mut::<ClientRequestBudget>()
                .store
                .admit_ingress(client, 0);
            assert!(app
                .world()
                .resource::<ClientRequestBudget>()
                .store
                .contains_client(&client));

            app.world_mut().despawn(client);
            app.update();

            let budget = app.world().resource::<ClientRequestBudget>();
            assert!(!budget.store.contains_client(&client));
            assert_eq!(
                budget.store.tokens_for(&client),
                None,
                "disconnect must release the client ingress token bucket"
            );
        }

        #[test]
        fn botany_harvest_request_updates_existing_session_without_gather_enqueue() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(HarvestSessionStore::default());

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            upsert_test_harvest_session(
                &mut app,
                "offline:Azure",
                entity,
                BotanyHarvestMode::Manual,
                10,
                0.5,
            );
            send_botany_harvest_request(&mut app, entity, "offline:Azure", "auto");

            app.update();

            let store = app.world().resource::<HarvestSessionStore>();
            let session = store.session_for("offline:Azure").unwrap();
            assert_eq!(session.mode, BotanyHarvestMode::Auto);
            assert_eq!(
                session.duration_ticks, 120,
                "auto harvest contract is a 120-tick session after a valid mode request"
            );
            assert_eq!(session.started_at_tick, 0);
            assert_eq!(session.last_progress, 0.0);
            assert_eq!(session.phase, BotanyPhase::InProgress);
            assert_eq!(
                session.client_entity, entity,
                "valid botany mode updates must preserve the existing session owner"
            );
            assert!(
                session.target_entity.is_some(),
                "valid botany mode updates must preserve the existing target reservation"
            );
        }

        #[test]
        fn botany_harvest_request_rejects_missing_session_without_gather_enqueue() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(HarvestSessionStore::default());

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();

            send_botany_harvest_request(&mut app, entity, "expired-session-token", "auto");

            app.update();

            assert!(
                app.world()
                    .resource::<HarvestSessionStore>()
                    .session_for("expired-session-token")
                    .is_none(),
                "invalid botany session_id must not create a harvest session"
            );
            assert!(
                app.world()
                    .resource::<HarvestSessionStore>()
                    .session_for("expired-session-token")
                    .is_none(),
                "invalid botany session_id must remain absent after the rejection path"
            );
        }

        #[test]
        fn botany_harvest_request_rejects_different_client_session_without_mutation() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(HarvestSessionStore::default());

            let (azure_bundle, _azure_helper) = create_mock_client("Azure");
            let azure = app.world_mut().spawn(azure_bundle).id();
            let (crimson_bundle, _crimson_helper) = create_mock_client("Crimson");
            let crimson = app.world_mut().spawn(crimson_bundle).id();
            upsert_test_harvest_session(
                &mut app,
                "offline:Azure",
                azure,
                BotanyHarvestMode::Manual,
                10,
                0.5,
            );

            send_botany_harvest_request(&mut app, crimson, "offline:Azure", "auto");

            app.update();

            let store = app.world().resource::<HarvestSessionStore>();
            let session = store.session_for("offline:Azure").unwrap();
            assert_eq!(
                session.mode,
                BotanyHarvestMode::Manual,
                "cross-client mode request must not mutate another player's session"
            );
            assert_eq!(session.started_at_tick, 10);
            assert_eq!(
            session.duration_ticks, 40,
            "manual harvest contract is a 40-tick session and a rejected request must preserve it"
        );
            assert_eq!(session.last_progress, 0.5);
            assert!(
                session.client_entity == azure,
                "rejected cross-client request must preserve the original session owner"
            );
        }

        #[test]
        fn botany_harvest_request_invalid_session_does_not_grant_gather_rewards() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(HarvestSessionStore::default());
            app.insert_resource(crate::player::gameplay::PendingGameplayNarrations::default());
            app.insert_resource(crate::qi_physics::WorldQiAccount::default());

            let initial_state = PlayerState {
                karma: 0.12,
                inventory_score: 0.34,
            };
            let initial_qi = 20.0;
            let (mut client_bundle, _helper) = create_mock_client("Azure");
            client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    initial_state.clone(),
                    Cultivation {
                        qi_current: initial_qi,
                        qi_max: 100.0,
                        ..Cultivation::default()
                    },
                ))
                .id();
            let zone_qi_before = app
                .world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .expect("fallback spawn zone should exist")
                .spirit_qi;

            send_botany_harvest_request(&mut app, entity, "expired-session-token", "auto");

            app.update();

            let player_state = app
                .world()
                .entity(entity)
                .get::<PlayerState>()
                .expect("player state should remain attached");
            assert_eq!(
                player_state, &initial_state,
                "invalid mode request must not mutate karma or inventory_score via Gather"
            );
            let cultivation = app
                .world()
                .entity(entity)
                .get::<Cultivation>()
                .expect("cultivation should remain attached");
            assert_eq!(
                cultivation.qi_current, initial_qi,
                "invalid mode request must not drain zone qi into the player"
            );
            let zone_qi_after = app
                .world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .expect("fallback spawn zone should still exist")
                .spirit_qi;
            assert_eq!(
                zone_qi_after, zone_qi_before,
                "invalid mode request must not mutate zone spirit_qi"
            );
            assert!(
                app.world()
                    .resource::<crate::qi_physics::WorldQiAccount>()
                    .transfers()
                    .is_empty(),
                "invalid mode request must not append gather qi audit transfers"
            );
            let narrations = app
                .world_mut()
                .resource_mut::<crate::player::gameplay::PendingGameplayNarrations>()
                .drain();
            assert!(
                narrations.is_empty(),
                "invalid mode request must not emit legacy gather narration"
            );
        }

        #[test]
        fn abort_tribulation_request_is_ignored_after_start_confirmation() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(CapturedStartDuXuRequests::default());
            app.add_event::<StartDuXuRequest>();
            app.add_systems(
                Update,
                capture_start_du_xu_requests.after(handle_client_request_payloads),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"start_du_xu","v":1}"#.to_vec().into_boxed_slice(),
                });

            app.update();

            assert_eq!(
                app.world().resource::<CapturedStartDuXuRequests>().0.len(),
                1,
                "control start_du_xu request should emit StartDuXuRequest"
            );

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"abort_tribulation","v":1}"#.to_vec().into_boxed_slice(),
                });

            app.update();

            assert_eq!(
            app.world().resource::<CapturedStartDuXuRequests>().0.len(),
            1,
            "abort_tribulation must not emit another StartDuXuRequest or cancellation side effect"
        );
        }

        #[test]
        fn movement_action_request_emits_intent_when_event_resource_exists() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.add_event::<MovementActionIntent>();

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"movement_action","v":1,"action":"dash"}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let events = app
                .world()
                .resource::<valence::prelude::Events<MovementActionIntent>>();
            let intents: Vec<_> = events.iter_current_update_events().collect();
            assert_eq!(
            intents.len(),
            1,
            "expected one MovementActionIntent because one valid movement_action payload was sent, actual: {}",
            intents.len()
        );
            assert_eq!(
                intents[0].entity, entity,
                "expected MovementActionIntent entity to match the sending client"
            );
            assert_eq!(
                intents[0].action,
                MovementAction::Dashing,
                "expected movement_action dash payload to map to MovementAction::Dashing"
            );
            assert_eq!(
                intents[0].yaw_degrees, None,
                "expected missing yaw_degrees to stay None for legacy movement_action payloads"
            );
        }

        #[test]
        fn movement_action_request_emits_client_yaw_when_present() {
            assert_movement_action_yaw_forwarded(90.5);
        }

        #[test]
        fn movement_action_request_accepts_yaw_boundaries() {
            for yaw_degrees in [0.0, 360.0, -45.0, 359.999] {
                assert_movement_action_yaw_forwarded(yaw_degrees);
            }
        }

        #[test]
        fn movement_action_request_rejects_non_numeric_yaw_degrees() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.add_event::<MovementActionIntent>();

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data:
                        br#"{"type":"movement_action","v":1,"action":"dash","yaw_degrees":"east"}"#
                            .to_vec()
                            .into_boxed_slice(),
                });

            app.update();

            let events = app
                .world()
                .resource::<valence::prelude::Events<MovementActionIntent>>();
            let intents: Vec<_> = events.iter_current_update_events().collect();
            assert!(
            intents.is_empty(),
            "expected no MovementActionIntent because yaw_degrees had invalid JSON type, actual: {}",
            intents.len()
        );
        }

        #[test]
        fn movement_action_request_without_event_resource_is_dropped() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"movement_action","v":1,"action":"dash"}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            assert!(app
                .world()
                .get_resource::<valence::prelude::Events<MovementActionIntent>>()
                .is_none());
        }

        #[test]
        fn use_quick_slot_reads_template_from_equipped_instance() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "bone_whistle".to_string(),
                ItemTemplate {
                    id: "bone_whistle".to_string(),
                    display_name: "骨哨".to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 0.1,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: 250,
                    cooldown_ms: 450,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                },
            )])));

            let mut inventory = empty_inventory();
            inventory.equipped.insert(
                crate::inventory::EQUIP_SLOT_OFF_HAND.to_string(),
                crate::inventory::SlotContents::held_single(inventory_test_item(
                    77,
                    "bone_whistle",
                    1,
                )),
            );
            let mut quick_slots = QuickSlotBindings::default();
            assert!(quick_slots.set(0, Some(77)));

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((client_bundle, quick_slots, inventory))
                .id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"use_quick_slot","v":1,"slot":0}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let casting = app
                .world()
                .get::<Casting>(entity)
                .expect("equipped quick slot item should start casting");
            assert_eq!(casting.bound_instance_id, Some(77));
            assert_eq!(casting.duration_ms, 250);
            assert_eq!(casting.duration_ticks, 5);
            assert_eq!(casting.complete_cooldown_ticks, 9);
        }

        #[test]
        fn use_quick_slot_unbound_slot_preserves_active_cross_slot_cast() {
            // central-review 2012 #1 回归：未绑定槽 use 必须静默忽略且**不得打断**
            // 进行中的异槽 cast。旧实现先走 cast 闸门（异槽 → cancel_previous_cast 发
            // cast_sync{Interrupt, UserCancel} 并 remove Casting），再发现槽 1 无绑定
            // 才返回——活动 cast 被无谓取消。契约（network_quickslot_config.py docstring：
            // 无绑定 → 静默忽略）下无绑定请求是无副作用的 no-op。
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "guyuan_pill".to_string(),
                ItemTemplate {
                    id: "guyuan_pill".to_string(),
                    display_name: "guyuan_pill".to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 64,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 0.1,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: 1500,
                    cooldown_ms: 1500,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                },
            )])));
            let mut inventory = empty_inventory();
            inventory.equipped.insert(
                crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                crate::inventory::SlotContents::held_single(inventory_test_item(
                    77,
                    "guyuan_pill",
                    1,
                )),
            );
            let mut quick_slots = QuickSlotBindings::default();
            assert!(quick_slots.set(0, Some(77)));
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((client_bundle, quick_slots, inventory))
                .id();

            // 请求 1：启动 slot 0 cast。
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"use_quick_slot","v":1,"slot":0}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();
            assert!(
                app.world().get::<Casting>(entity).is_some(),
                "前置：slot 0 应处于 casting 状态"
            );

            // 请求 2：slot 0 仍在 cast 时使用未绑定槽 1。
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"use_quick_slot","v":1,"slot":1}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();
            flush_all_client_packets(&mut app);

            // 活动 cast 必须原样保留（未被打断）。
            let casting = app
                .world()
                .get::<Casting>(entity)
                .expect("未绑定槽 use 不得取消进行中的 slot 0 cast");
            assert_eq!(casting.slot, 0);
            // 且不得下发 slot 0 的 Interrupt（UserCancel）cast_sync。
            let syncs = collect_cast_syncs(&mut helper);
            assert!(
                !syncs.iter().any(|s| s.phase == CastPhaseV1::Interrupt),
                "未绑定槽 use 不得产生任何 interrupt cast_sync，实际 {syncs:?}"
            );
        }

        #[test]
        fn unavailable_quick_slot_rejects_use_and_bind_without_disturbing_existing_state() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(
                crate::inventory::load_item_registry().expect("item registry loads"),
            );
            let locked_slot = QuickSlotBindings::SLOT_COUNT as u8;
            let mut quick = QuickSlotBindings::default();
            quick.set(0, Some(88));
            let mut inventory = empty_inventory();
            inventory.equipped.insert(
                crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                crate::inventory::SlotContents::held_single(inventory_test_item(
                    88,
                    "guyuan_pill",
                    1,
                )),
            );
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((client_bundle, quick, SkillBarBindings::default(), inventory))
                .id();
            for slot in [0, locked_slot] {
                app.world_mut()
                    .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                    .send(CustomPayloadEvent {
                        client: entity,
                        channel: ident!("bong:client_request").into(),
                        data: serde_json::to_vec(&serde_json::json!({
                            "type": "use_quick_slot", "v": 1, "slot": slot
                        }))
                        .unwrap()
                        .into_boxed_slice(),
                    });
                app.update();
                assert_eq!(
                    app.world()
                        .get::<Casting>(entity)
                        .expect("开放槽可开始施法")
                        .slot,
                    0,
                    "未开放槽不得替换或取消当前施法"
                );
            }
            send_quick_slot_bind_request(&mut app, entity, locked_slot, None, "locked-slot");
            app.update();
            flush_all_client_packets(&mut app);
            assert_eq!(
                app.world().get::<QuickSlotBindings>(entity).unwrap().get(0),
                Some(88),
                "越界请求不能改写开放槽的绑定"
            );
            assert!(
                !collect_quickslot_configs(&mut helper)
                    .iter()
                    .any(|config| config.ack_request_id.as_deref() == Some("locked-slot")),
                "schema 拒绝越界请求，不应产生绑定回执"
            );
        }

        #[test]
        fn use_quick_slot_on_cooldown_slot_preserves_active_cross_slot_cast() {
            // central-review 2012 #4 回归：handler 把「冷却未到期」早返回移到 cast 闸门
            // 之前——旧顺序下用冷却中的异槽会先 cancel_previous_cast（发
            // cast_sync{Interrupt, UserCancel} 并 remove Casting）再返回，活动 cast 被
            // 无谓打断。此前只有未绑定分支有测试，冷却分支完全没保护。本测试在 slot 0
            // 进行 cast 时 use 冷却中的 slot 1，断言 slot 0 cast 原样保留、无 interrupt。
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "guyuan_pill".to_string(),
                ItemTemplate {
                    id: "guyuan_pill".to_string(),
                    display_name: "guyuan_pill".to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 64,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 0.1,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: 1500,
                    cooldown_ms: 1500,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                },
            )])));
            let mut inventory = empty_inventory();
            inventory.equipped.insert(
                crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                crate::inventory::SlotContents::held_single(inventory_test_item(
                    77,
                    "guyuan_pill",
                    1,
                )),
            );
            let mut quick_slots = QuickSlotBindings::default();
            assert!(quick_slots.set(0, Some(77)));
            // slot 1 绑定同实例但处于冷却中（until_tick 设到远离默认 tick 0 的 u64::MAX）。
            assert!(quick_slots.set(1, Some(77)));
            quick_slots.set_cooldown(1, u64::MAX);
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((client_bundle, quick_slots, inventory))
                .id();

            // 请求 1：启动 slot 0 cast。
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"use_quick_slot","v":1,"slot":0}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();
            assert!(
                app.world().get::<Casting>(entity).is_some(),
                "前置：slot 0 应处于 casting 状态"
            );

            // 请求 2：slot 0 仍在 cast 时使用冷却中的 slot 1。
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"use_quick_slot","v":1,"slot":1}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();
            flush_all_client_packets(&mut app);

            let casting = app
                .world()
                .get::<Casting>(entity)
                .expect("冷却中的异槽 use 不得取消进行中的 slot 0 cast");
            assert_eq!(casting.slot, 0);
            let syncs = collect_cast_syncs(&mut helper);
            assert!(
                !syncs.iter().any(|s| s.phase == CastPhaseV1::Interrupt),
                "冷却中的异槽 use 不得产生任何 interrupt cast_sync，实际 {syncs:?}"
            );
        }

        #[test]
        fn use_quick_slot_stale_binding_missing_instance_preserves_active_cross_slot_cast() {
            // central-review 2012 #4 回归：绑定实例已不在背包（player 拖出去了）时 use
            // 必须静默忽略且不得打断进行中的异槽 cast。此前没有任何测试构造陈旧绑定
            // 覆盖 missing-instance 早返回分支——旧顺序把它放回 cast 闸门之后，用失效
            // 绑定的异槽会在活动 cast 期间先 cancel_previous_cast 再返回。本测试在
            // slot 0 进行 cast 时 use 绑定已失效实例（999，不在背包）的 slot 1，断言
            // slot 0 cast 原样保留、无 interrupt。
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "guyuan_pill".to_string(),
                ItemTemplate {
                    id: "guyuan_pill".to_string(),
                    display_name: "guyuan_pill".to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 64,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 0.1,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: 1500,
                    cooldown_ms: 1500,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                },
            )])));
            let mut inventory = empty_inventory();
            inventory.equipped.insert(
                crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                crate::inventory::SlotContents::held_single(inventory_test_item(
                    77,
                    "guyuan_pill",
                    1,
                )),
            );
            let mut quick_slots = QuickSlotBindings::default();
            assert!(quick_slots.set(0, Some(77)));
            // slot 1 绑定陈旧实例 999（不在背包），且不在冷却——恰好命中 missing-instance
            // 早返回分支（越过 cooldown 与 unbound 两个更靠前的检查）。
            assert!(quick_slots.set(1, Some(999)));
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((client_bundle, quick_slots, inventory))
                .id();

            // 请求 1：启动 slot 0 cast。
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"use_quick_slot","v":1,"slot":0}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();
            assert!(
                app.world().get::<Casting>(entity).is_some(),
                "前置：slot 0 应处于 casting 状态"
            );

            // 请求 2：slot 0 仍在 cast 时使用绑定失效实例的 slot 1。
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"use_quick_slot","v":1,"slot":1}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();
            flush_all_client_packets(&mut app);

            let casting = app
                .world()
                .get::<Casting>(entity)
                .expect("陈旧绑定（实例不在背包）use 不得取消进行中的 slot 0 cast");
            assert_eq!(casting.slot, 0);
            let syncs = collect_cast_syncs(&mut helper);
            assert!(
                !syncs.iter().any(|s| s.phase == CastPhaseV1::Interrupt),
                "陈旧绑定 use 不得产生任何 interrupt cast_sync，实际 {syncs:?}"
            );
        }

        #[test]
        fn quick_slot_bind_resolves_equipped_template_instance() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(
                crate::inventory::load_item_registry().expect("item registry loads"),
            );

            let mut inventory = empty_inventory();
            inventory.equipped.insert(
                crate::inventory::EQUIP_SLOT_OFF_HAND.to_string(),
                crate::inventory::SlotContents::held_single(inventory_test_item(
                    77,
                    "earth_crumb",
                    1,
                )),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    QuickSlotBindings::default(),
                    SkillBarBindings::default(),
                    inventory,
                ))
                .id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"quick_slot_bind","v":1,"slot":0,"item_id":"earth_crumb","request_id":"bind-equipped"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let bindings = app
                .world()
                .get::<QuickSlotBindings>(entity)
                .expect("player should keep quick slot bindings");
            assert_eq!(
                bindings.get(0),
                Some(77),
                "quick_slot_bind must resolve template ids from equipped held/worn items"
            );
        }

        #[test]
        fn quick_slot_bind_atomically_mirrors_block_item_into_skill_bar() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(
                crate::inventory::load_item_registry().expect("item registry loads"),
            );

            let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    QuickSlotBindings::default(),
                    SkillBarBindings::default(),
                    inventory,
                ))
                .id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"quick_slot_bind","v":1,"slot":1,"item_id":"earth_crumb","request_id":"bind-block"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let quick = app
                .world()
                .get::<QuickSlotBindings>(entity)
                .expect("player should keep quick slot bindings");
            assert_eq!(
                quick.get(1),
                Some(88),
                "expected block quick-slot intent to bind instance 88, actual {:?}",
                quick.get(1)
            );
            let skillbar = app
                .world()
                .get::<SkillBarBindings>(entity)
                .expect("player should keep skill bar bindings");
            assert_eq!(
                skillbar.get(1),
                Some(&SkillSlot::Item { instance_id: 88 }),
                "expected the same server intent to atomically mirror the block into skill bar"
            );
        }

        #[test]
        fn quick_slot_bind_rejects_unheld_item_without_mutating_or_persisting() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(
                crate::inventory::load_item_registry().expect("item registry loads"),
            );
            let mut quick = QuickSlotBindings::default();
            let _ = quick.set(1, Some(77));
            let mut skillbar = SkillBarBindings::default();
            let _ = skillbar.set(1, SkillSlot::Item { instance_id: 77 });
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((client_bundle, quick, skillbar, empty_inventory()))
                .id();

            send_quick_slot_bind_request(&mut app, entity, 1, Some("earth_crumb"), "reject-unheld");
            app.update();
            flush_all_client_packets(&mut app);

            assert_eq!(
                app.world().get::<QuickSlotBindings>(entity).unwrap().get(1),
                Some(77)
            );
            assert_eq!(
                app.world().get::<SkillBarBindings>(entity).unwrap().get(1),
                Some(&SkillSlot::Item { instance_id: 77 })
            );
            let configs = collect_quickslot_configs(&mut helper);
            assert!(configs.iter().any(|config| {
                config.ack_request_id.as_deref() == Some("reject-unheld")
                    && config.bind_accepted == Some(false)
            }));
        }

        #[test]
        fn quick_slot_bind_missing_skillbar_rejects_before_quick_slot_mutation() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(
                crate::inventory::load_item_registry().expect("item registry loads"),
            );
            let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((client_bundle, QuickSlotBindings::default(), inventory))
                .id();

            send_quick_slot_bind_request(
                &mut app,
                entity,
                1,
                Some("earth_crumb"),
                "reject-missing-skillbar",
            );
            app.update();
            flush_all_client_packets(&mut app);

            assert_eq!(
                app.world().get::<QuickSlotBindings>(entity).unwrap().get(1),
                None
            );
            let configs = collect_quickslot_configs(&mut helper);
            assert!(configs.iter().any(|config| {
                config.ack_request_id.as_deref() == Some("reject-missing-skillbar")
                    && config.bind_accepted == Some(false)
            }));
        }

        #[test]
        fn quick_slot_bind_clears_only_the_old_auto_mirrored_item() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(
                crate::inventory::load_item_registry().expect("item registry loads"),
            );
            let mut inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
            inventory.hotbar[0] = Some(inventory_test_item(89, "guyuan_pill", 1));
            let mut quick = QuickSlotBindings::default();
            let _ = quick.set(1, Some(88));
            let mut skillbar = SkillBarBindings::default();
            let _ = skillbar.set(1, SkillSlot::Item { instance_id: 88 });
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((client_bundle, quick, skillbar, inventory))
                .id();

            send_quick_slot_bind_request(&mut app, entity, 1, Some("guyuan_pill"), "block-to-pill");
            app.update();

            assert_eq!(
                app.world().get::<QuickSlotBindings>(entity).unwrap().get(1),
                Some(89)
            );
            assert_eq!(
                app.world().get::<SkillBarBindings>(entity).unwrap().get(1),
                Some(&SkillSlot::Empty),
                "expected block→non-block to clear only the stale automatic item mirror"
            );

            {
                let mut quick = app
                    .world_mut()
                    .get_mut::<QuickSlotBindings>(entity)
                    .unwrap();
                let _ = quick.set(1, Some(88));
            }
            {
                let mut skillbar = app.world_mut().get_mut::<SkillBarBindings>(entity).unwrap();
                let _ = skillbar.set(1, SkillSlot::Item { instance_id: 88 });
            }
            send_quick_slot_bind_request(&mut app, entity, 1, None, "block-to-clear");
            app.update();
            assert_eq!(
                app.world().get::<QuickSlotBindings>(entity).unwrap().get(1),
                None
            );
            assert_eq!(
                app.world().get::<SkillBarBindings>(entity).unwrap().get(1),
                Some(&SkillSlot::Empty),
                "expected block→clear to remove the matching automatic item mirror"
            );

            {
                let mut quick = app
                    .world_mut()
                    .get_mut::<QuickSlotBindings>(entity)
                    .unwrap();
                let _ = quick.set(1, Some(88));
            }
            {
                let mut skillbar = app.world_mut().get_mut::<SkillBarBindings>(entity).unwrap();
                let _ = skillbar.set(
                    1,
                    SkillSlot::Skill {
                        skill_id: "sword.cleave".to_string(),
                    },
                );
            }
            send_quick_slot_bind_request(&mut app, entity, 1, None, "protect-independent-skill");
            app.update();

            assert_eq!(
                app.world().get::<QuickSlotBindings>(entity).unwrap().get(1),
                None
            );
            assert_eq!(
                app.world().get::<SkillBarBindings>(entity).unwrap().get(1),
                Some(&SkillSlot::Skill {
                    skill_id: "sword.cleave".to_string()
                }),
                "expected clearing quick slot not to overwrite a later independent skill binding"
            );
        }

        #[test]
        fn quick_slot_bind_persistence_failure_leaves_both_components_unchanged() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(
                crate::inventory::load_item_registry().expect("item registry loads"),
            );
            let invalid_db_path = std::env::temp_dir();
            app.insert_resource(PlayerStatePersistence::with_db_path(
                std::env::temp_dir(),
                invalid_db_path,
            ));
            let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    QuickSlotBindings::default(),
                    SkillBarBindings::default(),
                    inventory,
                ))
                .id();

            send_quick_slot_bind_request(
                &mut app,
                entity,
                1,
                Some("earth_crumb"),
                "reject-persistence",
            );
            app.update();
            flush_all_client_packets(&mut app);

            assert_eq!(
                app.world().get::<QuickSlotBindings>(entity).unwrap().get(1),
                None
            );
            assert_eq!(
                app.world().get::<SkillBarBindings>(entity).unwrap().get(1),
                Some(&SkillSlot::Empty)
            );
            assert!(collect_quickslot_configs(&mut helper).iter().any(|config| {
                config.ack_request_id.as_deref() == Some("reject-persistence")
                    && config.bind_accepted == Some(false)
            }));
        }

        #[test]
        fn quick_slot_bind_persists_atomic_block_mirror_for_reload() {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("bong-quick-bind-{unique}"));
            let db_path = root.join("bong.db");
            crate::persistence::bootstrap_sqlite(&db_path, "quick-bind-test")
                .expect("test sqlite should bootstrap");
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(
                crate::inventory::load_item_registry().expect("item registry loads"),
            );
            app.insert_resource(PlayerStatePersistence::with_db_path(&root, &db_path));
            let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    QuickSlotBindings::default(),
                    SkillBarBindings::default(),
                    inventory,
                ))
                .id();

            send_quick_slot_bind_request(&mut app, entity, 1, Some("earth_crumb"), "persist-block");
            app.update();

            let connection = rusqlite::Connection::open(&db_path).expect("test sqlite should open");
            let prefs_json: String = connection
                .query_row(
                    "SELECT prefs_json FROM player_ui_prefs WHERE username = 'Azure'",
                    [],
                    |row| row.get(0),
                )
                .expect("accepted bind should persist UI prefs");
            let prefs: serde_json::Value =
                serde_json::from_str(&prefs_json).expect("persisted prefs should be valid JSON");
            assert_eq!(prefs["quick_slots"][1], "earth_crumb");
            assert_eq!(prefs["skill_bar"][1]["kind"], "item");
            assert_eq!(prefs["skill_bar"][1]["template_id"], "earth_crumb");
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn quick_slot_bind_accepts_128_cjk_request_id_and_rejects_129() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(
                crate::inventory::load_item_registry().expect("item registry loads"),
            );

            let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    QuickSlotBindings::default(),
                    SkillBarBindings::default(),
                    inventory,
                ))
                .id();

            // 128 个 '界' 字符（每个 3 字节，共 384 字节）必须被视为合法长度并接受
            let rid128 = "界".repeat(128);
            send_quick_slot_bind_request(&mut app, entity, 1, Some("earth_crumb"), &rid128);
            app.update();
            flush_all_client_packets(&mut app);

            assert_eq!(
                app.world().get::<QuickSlotBindings>(entity).unwrap().get(1),
                Some(88)
            );
            let configs = collect_quickslot_configs(&mut helper);
            assert!(configs.iter().any(|c| {
                c.ack_request_id.as_deref() == Some(&rid128) && c.bind_accepted == Some(true)
            }));

            // 129 个 '界' 字符必须被静默拒绝且不产生状态变异
            let rid129 = "界".repeat(129);
            send_quick_slot_bind_request(&mut app, entity, 0, Some("earth_crumb"), &rid129);
            app.update();
            flush_all_client_packets(&mut app);

            assert_eq!(
                app.world().get::<QuickSlotBindings>(entity).unwrap().get(0),
                None
            );
        }

        #[test]
        fn quick_slot_bind_rejects_empty_string_item_id_without_unbinding() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(
                crate::inventory::load_item_registry().expect("item registry loads"),
            );

            let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
            let mut quick_slots = QuickSlotBindings::default();
            assert!(quick_slots.set(1, Some(88)));
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    quick_slots,
                    SkillBarBindings::default(),
                    inventory,
                ))
                .id();

            // 发送 raw JSON item_id=""（非 null），必须被拒绝（bind_accepted=false）且已有绑定保持原样
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"quick_slot_bind","v":1,"slot":1,"item_id":"","request_id":"empty-item-id"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
            app.update();
            flush_all_client_packets(&mut app);

            // 槽 1 上的已有绑定 88 必须保持，不得被清空
            assert_eq!(
                app.world().get::<QuickSlotBindings>(entity).unwrap().get(1),
                Some(88),
                "item_id=\"\" 畸形请求不得清空既有绑定"
            );
            let configs = collect_quickslot_configs(&mut helper);
            assert!(
                configs.iter().any(|c| {
                    c.ack_request_id.as_deref() == Some("empty-item-id")
                        && c.bind_accepted == Some(false)
                }),
                "item_id=\"\" 请求应下发 bind_accepted=false 的 quickslot_config 回执"
            );
        }

        #[test]
        fn inventory_move_applies_hidden_targeted_wear_to_spiritual_item() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "spiritual_ore".to_string(),
                ItemTemplate {
                    id: "spiritual_ore".to_string(),
                    display_name: "灵矿".to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 1.0,
                    rarity: ItemRarity::Rare,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                    cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                },
            )])));
            let mut karma = KarmaWeightStore::default();
            karma.mark_player(
                "Azure",
                Some("spawn".to_string()),
                valence::prelude::BlockPos::new(8, 66, 8),
                1.0,
                1,
            );
            app.insert_resource(karma);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_item(ItemInstance {
                        instance_id: 77,
                        template_id: "spiritual_ore".to_string(),
                        display_name: "灵矿".to_string(),
                        grid_w: 1,
                        grid_h: 1,
                        weight: 1.0,
                        rarity: ItemRarity::Rare,
                        description: String::new(),
                        stack_count: 1,
                        spirit_quality: 1.0,
                        durability: 1.0,
                        freshness: None,
                        mineral_id: Some("ling_shi_zhong".to_string()),
                        charges: None,
                        forge_quality: None,
                        forge_color: None,
                        forge_side_effects: Vec::new(),
                        forge_achieved_tier: None,
                        alchemy: None,
                        lingering_owner_qi: None,
                    }),
                    Cultivation::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"inventory_move_intent","v":1,"instance_id":77,"from":{"kind":"container","container_id":"main_pack","row":0,"col":0},"to":{"kind":"container","container_id":"main_pack","row":0,"col":1}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();
            flush_all_client_packets(&mut app);

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            let moved =
                inventory_item_by_instance_borrow(inventory, 77).expect("item should remain");
            assert!(moved.durability < 1.0);
            assert!(moved.durability >= 0.95);
            assert_eq!(moved.durability, moved.spirit_quality);
            assert!(
                has_inventory_durability_payload(&mut helper, 77),
                "targeted wear should reuse durability incremental payload"
            );
        }

        /// plan-rotate-v1 e2e — 客户端 JSON wire 带 rotated:true 的 inventory_move_intent
        /// 走完整 handler 链路后，instance 的 grid_w/grid_h 在 PlayerInventory 中互换。
        #[test]
        fn inventory_move_intent_with_rotated_true_swaps_dims_end_to_end() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "long_rod".to_string(),
                ItemTemplate {
                    id: "long_rod".to_string(),
                    display_name: "长杆".to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 2,
                    grid_h: 1,
                    base_weight: 1.0,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                    cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                },
            )])));

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_item(ItemInstance {
                        instance_id: 77,
                        template_id: "long_rod".to_string(),
                        display_name: "长杆".to_string(),
                        grid_w: 2,
                        grid_h: 1,
                        weight: 1.0,
                        rarity: ItemRarity::Common,
                        description: String::new(),
                        stack_count: 1,
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
                    }),
                    Cultivation::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"inventory_move_intent","v":1,"instance_id":77,"rotated":true,"from":{"kind":"container","container_id":"main_pack","row":0,"col":0},"to":{"kind":"container","container_id":"main_pack","row":2,"col":3}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();
            flush_all_client_packets(&mut app);

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            let placed = inventory.containers[0]
                .items
                .iter()
                .find(|p| p.instance.instance_id == 77)
                .expect("item should remain in main_pack");
            assert_eq!(
                (placed.row, placed.col),
                (2, 3),
                "rotated move 应落到目标格 (2,3)"
            );
            assert_eq!(
                (placed.instance.grid_w, placed.instance.grid_h),
                (1, 2),
                "e2e：rotated:true 落位后 grid_w/grid_h 应互换为 1x2，实际 {}x{}",
                placed.instance.grid_w,
                placed.instance.grid_h
            );
        }

        /// plan-rotate-v1 e2e — rotated 落位越界（2x1 转 1x2 撞底）被拒后，
        /// 原物品位置与朝向均未变（无脏状态），且不 panic。
        #[test]
        fn inventory_move_intent_rotated_rejection_leaves_inventory_clean_end_to_end() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "long_rod".to_string(),
                ItemTemplate {
                    id: "long_rod".to_string(),
                    display_name: "长杆".to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 2,
                    grid_h: 1,
                    base_weight: 1.0,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                    cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                },
            )])));

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_item(ItemInstance {
                        instance_id: 77,
                        template_id: "long_rod".to_string(),
                        display_name: "长杆".to_string(),
                        grid_w: 2,
                        grid_h: 1,
                        weight: 1.0,
                        rarity: ItemRarity::Common,
                        description: String::new(),
                        stack_count: 1,
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
                    }),
                    Cultivation::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();
            // 目标 (4,0)：不旋转时 2x1 在最底行放得下；旋转成 1x2 后行溢出 → 拒绝。
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"inventory_move_intent","v":1,"instance_id":77,"rotated":true,"from":{"kind":"container","container_id":"main_pack","row":0,"col":0},"to":{"kind":"container","container_id":"main_pack","row":4,"col":0}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();
            flush_all_client_packets(&mut app);

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            let placed = inventory.containers[0]
                .items
                .iter()
                .find(|p| p.instance.instance_id == 77)
                .expect("item should remain in main_pack");
            assert_eq!(
                (placed.row, placed.col),
                (0, 0),
                "旋转越界拒绝后物品必须留在原位"
            );
            assert_eq!(
                (placed.instance.grid_w, placed.instance.grid_h),
                (2, 1),
                "旋转越界拒绝后必须保持原朝向 2x1（无脏状态）"
            );
        }

        #[test]
        fn apply_pill_during_tribulation_recovers_current_qi_only() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "huiyuan_pill".to_string(),
                ItemTemplate {
                    id: "huiyuan_pill".to_string(),
                    display_name: "回元丹".to_string(),
                    category: ItemCategory::Pill,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 0.1,
                    rarity: ItemRarity::Rare,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: Some(ItemEffect::QiRecovery { amount: 90.0 }),
                    cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                    cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                },
            )])));

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_item(ItemInstance {
                        instance_id: 77,
                        template_id: "huiyuan_pill".to_string(),
                        display_name: "回元丹".to_string(),
                        grid_w: 1,
                        grid_h: 1,
                        weight: 0.1,
                        rarity: ItemRarity::Rare,
                        description: String::new(),
                        stack_count: 1,
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
                    }),
                    Cultivation {
                        realm: Realm::Spirit,
                        qi_current: 20.0,
                        qi_max: 100.0,
                        qi_max_frozen: Some(30.0),
                        ..Cultivation::default()
                    },
                    PlayerState::default(),
                    TribulationState::restored(2, 5, 10),
                ))
                .id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data:
                        br#"{"type":"apply_pill","v":1,"instance_id":77,"target":{"kind":"self"}}"#
                            .to_vec()
                            .into_boxed_slice(),
                });

            app.update();

            let cultivation = app.world().get::<Cultivation>(entity).unwrap();
            assert_eq!(cultivation.qi_current, 70.0);
            assert_eq!(cultivation.qi_max, 100.0);
            assert_eq!(cultivation.qi_max_frozen, Some(30.0));
            assert!(app.world().get::<TribulationState>(entity).is_some());

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            assert!(inventory.containers[0].items.is_empty());
            assert_eq!(inventory.revision.0, 1);
        }

        #[test]
        fn mineral_probe_request_emits_probe_intent() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedMineralProbes::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_mineral_probes).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .entity_mut(entity)
                .insert(Position(DVec3::new(8.5, 32.0, 8.5)));
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"mineral_probe","v":1,"x":8,"y":32,"z":8}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let captured = app.world().resource::<CapturedMineralProbes>();
            assert_eq!(captured.0.len(), 1);
            assert_eq!(captured.0[0].player, entity);
            assert_eq!(
                captured.0[0].position,
                valence::prelude::BlockPos::new(8, 32, 8)
            );
            assert_eq!(captured.0[0].dimension, DimensionKind::Overworld);
        }

        #[test]
        fn spirit_niche_place_request_emits_place_intent() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedSpiritNichePlaces::default());
            app.insert_resource(CombatClock { tick: 88 });
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<SpiritNichePlaceRequest>();
            app.add_event::<SpiritNicheCoordinateRevealRequest>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_spirit_niche_places).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"spirit_niche_place","v":1,"x":11,"y":64,"z":10,"item_instance_id":4242}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let captured = app.world().resource::<CapturedSpiritNichePlaces>();
            assert_eq!(captured.0.len(), 1);
            assert_eq!(captured.0[0].player, entity);
            assert_eq!(captured.0[0].pos, [11, 64, 10]);
            assert_eq!(captured.0[0].item_instance_id, Some(4242));
            assert_eq!(captured.0[0].tick, 88);
        }

        #[test]
        fn spirit_niche_repair_request_emits_repair_intent() {
            let mut app = App::new();
            app.insert_resource(CapturedSpiritNicheRepairs::default());
            register_request_app(&mut app);
            app.insert_resource(CombatClock { tick: 90 });
            app.add_systems(
                Update,
                capture_spirit_niche_repairs.after(handle_client_request_payloads),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"spirit_niche_repair","v":1,"x":11,"y":64,"z":10,"item_instance_id":4242}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let captured = app.world().resource::<CapturedSpiritNicheRepairs>();
            assert_eq!(captured.0.len(), 1);
            assert_eq!(captured.0[0].player, entity);
            assert_eq!(captured.0[0].pos, [11, 64, 10]);
            assert_eq!(captured.0[0].item_instance_id, Some(4242));
            assert_eq!(captured.0[0].tick, 90);
        }

        #[test]
        fn coffin_open_request_emits_spawn_tutorial_intent() {
            let mut app = App::new();
            app.insert_resource(CapturedCoffinOpenRequests::default());
            register_request_app(&mut app);
            app.insert_resource(CombatClock { tick: 91 });
            app.add_systems(
                Update,
                capture_coffin_open_requests.after(handle_client_request_payloads),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"coffin_open","v":1,"x":0,"y":69,"z":0}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let captured = app.world().resource::<CapturedCoffinOpenRequests>();
            assert_eq!(captured.0.len(), 1);
            assert_eq!(captured.0[0].player, entity);
            assert_eq!(captured.0[0].pos, [0, 69, 0]);
            assert_eq!(captured.0[0].tick, 91);
        }

        // ─── plan-coffin-tiers-v1 P3：CoffinBreak/CoffinMenuReclaim decode→emit tests ───
        // CodeRabbit major A: 补 C2S 协议分支 decode→emit 测试。

        #[test]
        fn coffin_break_request_emits_event_with_correct_player_and_pos() {
            let mut app = App::new();
            app.insert_resource(CapturedCoffinBreakRequests::default());
            register_request_app(&mut app);
            app.insert_resource(CombatClock { tick: 77 });
            app.add_systems(
                Update,
                capture_coffin_break_requests.after(handle_client_request_payloads),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"coffin_break","v":1,"x":10,"y":64,"z":-5}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let captured = app.world().resource::<CapturedCoffinBreakRequests>();
            assert_eq!(
                captured.0.len(),
                1,
                "coffin_break 请求应 emit 恰好 1 个 CoffinBreakRequest event；实得 {}",
                captured.0.len()
            );
            assert_eq!(
                captured.0[0].player, entity,
                "CoffinBreakRequest.player 应等于发送玩家实体；期望 {entity:?}，实得 {:?}",
                captured.0[0].player
            );
            assert_eq!(
            captured.0[0].pos,
            valence::prelude::BlockPos::new(10, 64, -5),
            "CoffinBreakRequest.pos 应精确等于请求坐标 [10,64,-5]；期望 BlockPos(10,64,-5)，实得 {:?}",
            captured.0[0].pos
        );
            assert_eq!(
                captured.0[0].tick, 77,
                "CoffinBreakRequest.tick 应等于 CombatClock.tick；期望 77，实得 {}",
                captured.0[0].tick
            );
        }

        #[test]
        fn coffin_menu_reclaim_request_emits_event_with_correct_player_and_pos() {
            let mut app = App::new();
            app.insert_resource(CapturedCoffinMenuReclaimRequests::default());
            register_request_app(&mut app);
            app.insert_resource(CombatClock { tick: 88 });
            app.add_systems(
                Update,
                capture_coffin_menu_reclaim_requests.after(handle_client_request_payloads),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"coffin_menu_reclaim","v":1,"x":-8,"y":65,"z":3}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let captured = app.world().resource::<CapturedCoffinMenuReclaimRequests>();
            assert_eq!(
                captured.0.len(),
                1,
                "coffin_menu_reclaim 请求应 emit 恰好 1 个 CoffinMenuReclaimRequest event；实得 {}",
                captured.0.len()
            );
            assert_eq!(
                captured.0[0].player, entity,
                "CoffinMenuReclaimRequest.player 应等于发送玩家实体；期望 {entity:?}，实得 {:?}",
                captured.0[0].player
            );
            assert_eq!(
            captured.0[0].pos,
            valence::prelude::BlockPos::new(-8, 65, 3),
            "CoffinMenuReclaimRequest.pos 应精确等于请求坐标 [-8,65,3]；期望 BlockPos(-8,65,3)，实得 {:?}",
            captured.0[0].pos
        );
            assert_eq!(
                captured.0[0].tick, 88,
                "CoffinMenuReclaimRequest.tick 应等于 CombatClock.tick；期望 88，实得 {}",
                captured.0[0].tick
            );
        }

        #[test]
        fn spirit_niche_coordinate_requests_emit_reveal_intents() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedSpiritNicheCoordinateReveals::default());
            app.insert_resource(CombatClock { tick: 89 });
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<SpiritNichePlaceRequest>();
            app.add_event::<SpiritNicheCoordinateRevealRequest>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_systems(
                Update,
                (
                    handle_client_request_payloads,
                    capture_spirit_niche_coordinate_reveals,
                )
                    .chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut custom_payloads = app
                .world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>();
            custom_payloads.send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"spirit_niche_gaze","v":1,"x":11,"y":64,"z":10}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
            custom_payloads.send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"spirit_niche_mark_coordinate","v":1,"x":12,"y":65,"z":11}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let captured = app
                .world()
                .resource::<CapturedSpiritNicheCoordinateReveals>();
            assert_eq!(captured.0.len(), 2);
            assert_eq!(captured.0[0].observer, entity);
            assert_eq!(captured.0[0].pos, [11, 64, 10]);
            assert_eq!(captured.0[0].source, SpiritNicheRevealSource::Gaze);
            assert_eq!(captured.0[0].tick, 89);
            assert_eq!(captured.0[1].observer, entity);
            assert_eq!(captured.0[1].pos, [12, 65, 11]);
            assert_eq!(
                captured.0[1].source,
                SpiritNicheRevealSource::MarkCoordinate
            );
            assert_eq!(captured.0[1].tick, 89);
        }

        #[test]
        fn mineral_probe_request_out_of_range_is_rejected() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedMineralProbes::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_mineral_probes).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .entity_mut(entity)
                .insert(Position(DVec3::ZERO));
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"mineral_probe","v":1,"x":128,"y":64,"z":128}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let captured = app.world().resource::<CapturedMineralProbes>();
            assert!(captured.0.is_empty());
        }

        #[test]
        fn mineral_probe_request_uses_player_dimension() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedMineralProbes::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_mineral_probes).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(entity).insert((
                Position(DVec3::new(8.5, 32.0, 8.5)),
                CurrentDimension(DimensionKind::Tsy),
            ));
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"mineral_probe","v":1,"x":8,"y":32,"z":8}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let captured = app.world().resource::<CapturedMineralProbes>();
            assert_eq!(captured.0.len(), 1);
            assert_eq!(captured.0[0].dimension, DimensionKind::Tsy);
        }

        #[test]
        fn qi_color_inspect_rejects_entity_bits_target() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(CapturedQiColorInspectRequests::default());
            app.add_systems(
                Update,
                capture_qi_color_inspect_requests.after(handle_client_request_payloads),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let observer = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .entity_mut(observer)
                .insert(Position(DVec3::ZERO));
            let observed = app
                .world_mut()
                .spawn(Position(DVec3::new(1.0, 0.0, 0.0)))
                .id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: observer,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::QiColorInspect {
                        v: 1,
                        observed: format!("entity_bits:{}", observed.to_bits()),
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });

            app.update();

            assert!(app
                .world()
                .resource::<CapturedQiColorInspectRequests>()
                .0
                .is_empty());
        }

        fn qi_color_inspect_test_app() -> App {
            let mut app = App::new();
            app.add_plugins(EntityPlugin);
            register_request_app(&mut app);
            app.insert_resource(CapturedQiColorInspectRequests::default());
            app.add_systems(
                Update,
                capture_qi_color_inspect_requests.after(handle_client_request_payloads),
            );
            app
        }

        fn send_qi_color_inspect_payload(app: &mut App, observer: Entity, observed: &str) {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: observer,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::QiColorInspect {
                        v: 1,
                        observed: observed.to_string(),
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
        }

        #[test]
        fn qi_color_inspect_rejects_self_cross_dimension_and_malformed_targets_without_side_effects(
        ) {
            let mut app = qi_color_inspect_test_app();
            let (client_bundle, _helper) = create_mock_client("QiColorObserver");
            let observer = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(observer).insert((
                Position(DVec3::ZERO),
                CurrentDimension(DimensionKind::Overworld),
            ));
            let observed = app
                .world_mut()
                .spawn((
                    EntityKind::VILLAGER,
                    EntityId::default(),
                    Position(DVec3::new(1.0, 0.0, 0.0)),
                    OldPosition::new(DVec3::new(1.0, 0.0, 0.0)),
                    CurrentDimension(DimensionKind::Tsy),
                ))
                .id();

            // EntityPlugin assigns the protocol ids that the C2S resolver is allowed to consume.
            app.update();
            let observer_id = app
                .world()
                .get::<EntityId>(observer)
                .expect("the observer must have an authoritative protocol entity id")
                .get();
            let observed_id = app
                .world()
                .get::<EntityId>(observed)
                .expect("the observed entity must have an authoritative protocol entity id")
                .get();
            let entity_count_before = app.world().entities().len();

            send_qi_color_inspect_payload(&mut app, observer, &format!("entity:{observer_id}"));
            send_qi_color_inspect_payload(&mut app, observer, &format!("entity:{observed_id}"));
            send_qi_color_inspect_payload(&mut app, observer, "entity:not-a-number");

            app.update();

            assert!(
            app.world()
                .resource::<CapturedQiColorInspectRequests>()
                .0
                .is_empty(),
            "self-target, cross-dimension, and malformed entity id denials must emit no QiColorInspectRequest"
        );
            assert_eq!(
                app.world().entities().len(),
                entity_count_before,
                "QiColorInspect denials must not spawn or despawn ECS entities"
            );
            assert_eq!(
                app.world()
                    .get::<Position>(observer)
                    .expect("observer position must remain present")
                    .get(),
                DVec3::ZERO,
                "QiColorInspect denials must not mutate the observer position"
            );
            assert_eq!(
                app.world()
                    .get::<CurrentDimension>(observed)
                    .expect("observed dimension must remain present")
                    .0,
                DimensionKind::Tsy,
                "QiColorInspect denials must not mutate the observed dimension"
            );
        }

        fn production_scroll_request_app() -> App {
            let mut app = App::new();
            register_request_app(&mut app);
            let item_registry = crate::inventory::load_item_registry()
                .expect("production item registry must load for scroll routing tests");
            let mut craft_registry = crate::craft::CraftRegistry::new();
            crate::craft::load_default_craft_recipes(&mut craft_registry, &item_registry)
                .expect("production craft registry must load for scroll routing tests");
            app.insert_resource(item_registry);
            app.insert_resource(craft_registry);
            app.insert_resource(crate::craft::RecipeUnlockState::new());
            app.add_event::<crate::craft::CraftUnlockIntent>();
            app.add_event::<crate::craft::RecipeUnlockedEvent>();
            app.add_systems(
                Update,
                crate::network::craft_emit::apply_unlock_intents
                    .after(handle_client_request_payloads),
            );
            app
        }

        fn send_scroll_use(
            app: &mut App,
            entity: Entity,
            instance_id: u64,
            request: fn(u64) -> ClientRequestV1,
        ) {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&request(instance_id))
                        .unwrap()
                        .into_boxed_slice(),
                });
        }

        fn send_technique_scroll_use(app: &mut App, entity: Entity, instance_id: u64) {
            send_scroll_use(app, entity, instance_id, |instance_id| {
                ClientRequestV1::TechniqueScrollUse { v: 1, instance_id }
            });
        }

        fn send_skill_scroll_use(app: &mut App, entity: Entity, instance_id: u64) {
            send_scroll_use(app, entity, instance_id, |instance_id| {
                ClientRequestV1::LearnSkillScroll { v: 1, instance_id }
            });
        }

        #[test]
        fn production_technique_scroll_falls_through_craft_routing() {
            let mut app = production_scroll_request_app();
            let (client_bundle, _helper) = create_mock_client("Azure");
            let mut meridians = MeridianSystem::default();
            let lung = meridians.get_mut(MeridianId::Lung);
            lung.opened = true;
            lung.integrity = 1.0;
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(42, "scroll_woliu_vortex")),
                    KnownTechniques {
                        entries: Vec::new(),
                    },
                    Cultivation {
                        realm: Realm::Condense,
                        ..Default::default()
                    },
                    meridians,
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();

            send_technique_scroll_use(&mut app, entity, 42);
            app.update();

            let known = app.world().get::<KnownTechniques>(entity).unwrap();
            assert!(
            known.entries.iter().any(|entry| entry.id == "woliu.vortex"),
            "production technique scroll must reach the existing technique learner when no craft recipe names it"
        );
            assert!(
                app.world()
                    .get::<PlayerInventory>(entity)
                    .unwrap()
                    .containers[0]
                    .items
                    .is_empty(),
                "successful technique learning must consume exactly one production scroll"
            );
            assert!(
                app.world_mut()
                    .resource_mut::<valence::prelude::Events<crate::craft::RecipeUnlockedEvent>>()
                    .drain()
                    .next()
                    .is_none(),
                "a technique-only scroll must not unlock a craft recipe"
            );
        }

        #[test]
        fn starter_dash_scroll_can_be_identified_and_learned_from_inventory() {
            let mut app = production_scroll_request_app();
            let (client_bundle, _helper) = create_mock_client("DashReader");
            let item = skill_scroll_item(42, "scroll_technique_movement_dash");
            let view = crate::network::inventory_snapshot_emit::item_view_from_instance(&item);
            assert_eq!(
                view.scroll_kind.as_deref(),
                Some("combat_technique"),
                "背包必须标明功法卷轴，否则客户端不会显示研读入口"
            );
            assert_eq!(view.scroll_skill_id.as_deref(), Some("movement.dash"));
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(item),
                    KnownTechniques::default(),
                    Cultivation::default(),
                    MeridianSystem::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();
            send_technique_scroll_use(&mut app, entity, view.instance_id);
            app.update();
            assert!(
                app.world()
                    .get::<KnownTechniques>(entity)
                    .unwrap()
                    .entries
                    .iter()
                    .any(|entry| entry.id == "movement.dash"),
                "初生玩家应能研读初始残页"
            );
            assert!(
                app.world()
                    .get::<PlayerInventory>(entity)
                    .unwrap()
                    .containers[0]
                    .items
                    .is_empty(),
                "研读成功后残页应被消耗"
            );
        }

        #[test]
        fn technique_scroll_realm_too_low_emits_structured_rejection() {
            // central-review 2012 #3 回归：fresh Awaken 用 sword.infuse（required
            // realm=Induce）→ RealmTooLow 拒绝，必须下发 InventoryMoveRejectedV1
            // {reason:"realm_too_low", required_realm:"Induce"}——只回推不变快照时
            // client 无法区分「境界拒绝」与「静默忽略/错误原因拒绝」。
            let mut app = production_scroll_request_app();
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(
                        42,
                        "scroll_technique_sword_infuse",
                    )),
                    KnownTechniques {
                        entries: Vec::new(),
                    },
                    Cultivation {
                        realm: Realm::Awaken,
                        ..Default::default()
                    },
                    MeridianSystem::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();

            send_technique_scroll_use(&mut app, entity, 42);
            app.update();
            flush_all_client_packets(&mut app);

            let rejected = collect_inventory_move_rejected(&mut helper);
            assert_eq!(
                rejected,
                vec![crate::schema::server_data::InventoryMoveRejectedV1 {
                    reason: "realm_too_low".to_string(),
                    required_realm: Some("Induce".to_string()),
                    slot: None,
                    cap: None,
                }],
                "Awaken 用 sword.infuse 应下发恰好一条 realm_too_low 拒绝回执"
            );
        }

        #[test]
        fn technique_scroll_race_mismatch_emits_structured_rejection() {
            // central-review 2012 #3 回归：RaceMismatch 拒绝必须同样下发结构化
            // InventoryMoveRejectedV1 {reason:"race_mismatch"}——非人形本体
            // （is_humanoid=false）用 sword.infuse（RaceGate::Humanoid）时 realm 已到
            // Induce 满足境界门（否则被 RealmTooLow 掩盖），race gate 是唯一拒因。
            let mut app = production_scroll_request_app();
            let (body_plans, races) = non_humanoid_race_fixture();
            app.insert_resource(body_plans);
            app.insert_resource(races);
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(
                        42,
                        "scroll_technique_sword_infuse",
                    )),
                    KnownTechniques {
                        entries: Vec::new(),
                    },
                    Cultivation {
                        realm: Realm::Induce,
                        // central-review 2012 #9：玩家 race 必须指向真实的非人形资产
                        // `whale`，生产 `resolve_body_plan` 才选到 is_humanoid=false 本体；
                        // 若停在 HUMAN_RACE_ID 则人形 gate 通过、不会触发 race_mismatch。
                        race: crate::body_plan::RaceId::new("whale"),
                        ..Default::default()
                    },
                    MeridianSystem::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();

            send_technique_scroll_use(&mut app, entity, 42);
            app.update();
            flush_all_client_packets(&mut app);

            let rejected = collect_inventory_move_rejected(&mut helper);
            assert_eq!(
                rejected,
                vec![crate::schema::server_data::InventoryMoveRejectedV1 {
                    reason: "race_mismatch".to_string(),
                    required_realm: None,
                    slot: None,
                    cap: None,
                }],
                "非人形本体用 sword.infuse 应下发恰好一条 race_mismatch 拒绝回执"
            );
        }

        #[test]
        fn production_skill_scroll_falls_through_craft_routing() {
            let mut app = production_scroll_request_app();
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(
                        42,
                        "skill_scroll_herbalism_baicao_can",
                    )),
                    SkillSet::default(),
                    Cultivation::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();

            send_skill_scroll_use(&mut app, entity, 42);
            app.update();

            let skill_set = app.world().get::<SkillSet>(entity).unwrap();
            assert!(
            skill_set
                .consumed_scrolls
                .contains(&ScrollId::new("skill_scroll_herbalism_baicao_can")),
            "production skill scroll must reach the existing skill learner when no craft recipe names it"
        );
            assert!(
                app.world()
                    .get::<PlayerInventory>(entity)
                    .unwrap()
                    .containers[0]
                    .items
                    .is_empty(),
                "successful skill learning must consume exactly one production scroll"
            );
        }

        #[test]
        fn learn_skill_scroll_routes_positive_craft_recipe_unlock() {
            let mut app = production_scroll_request_app();
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(42, "scroll_workbench_lantern")),
                ))
                .id();

            send_skill_scroll_use(&mut app, entity, 42);
            app.update();

            let player_id = canonical_player_id("Azure");
            assert!(
                app.world()
                    .resource::<crate::craft::RecipeUnlockState>()
                    .is_unlocked(
                        &player_id,
                        &crate::craft::RecipeId::new("workbench.shelter.lantern")
                    ),
                "LearnSkillScroll must route craft recipe scrolls to the craft unlock consumer"
            );
            assert!(
                app.world()
                    .get::<PlayerInventory>(entity)
                    .unwrap()
                    .containers[0]
                    .items
                    .is_empty(),
                "successful craft recipe scroll use must consume exactly one scroll"
            );
        }

        #[test]
        fn craft_scroll_unlock_uses_stable_player_id_when_caster_entity_is_gone() {
            // verdict-1906-r2 major #3 回归：残卷 reservation 与 unlock 以
            // `intent.player_id`（canonical 稳定身份）为准，而非 caster entity。
            // 历史上 consumer 反查 caster 的 Username，查不到时 fallback
            // `entity:{bits}` —— reservation 以 canonical_player_id 落账却永不
            // 释放，同玩家再拿一张同卷会被 reserve 永久拒绝。这里直接构造
            // "caster 实体已不存在"的 intent（换线重连后旧 entity id 失效），
            // 走 production 全链路（request → reserve → consume → intent →
            // apply_unlock_intents），断言 unlock 仍提交。
            let mut app = production_scroll_request_app();
            let player_id = canonical_player_id("Azure");
            let recipe_id = crate::craft::RecipeId::new("workbench.shelter.lantern");

            // 正常路径先 unlock 一次（走真实请求链路，锁住 production bridge）。
            {
                let (client_bundle, _helper) = create_mock_client("Azure");
                let entity = app
                    .world_mut()
                    .spawn((
                        client_bundle,
                        inventory_with_skill_scroll(skill_scroll_item(
                            42,
                            "scroll_workbench_lantern",
                        )),
                    ))
                    .id();
                send_skill_scroll_use(&mut app, entity, 42);
                app.update();
                assert!(
                    app.world()
                        .resource::<crate::craft::RecipeUnlockState>()
                        .is_unlocked(&player_id, &recipe_id),
                    "first unlock via real request must commit"
                );
                app.world_mut().despawn(entity);
            }

            // 已解锁 → reserve 返回 false（防止再扣第二张卷）。该行为不变。
            {
                let (client_bundle, _helper) = create_mock_client("Azure");
                let second = app
                    .world_mut()
                    .spawn((
                        client_bundle,
                        inventory_with_skill_scroll(skill_scroll_item(
                            43,
                            "scroll_workbench_lantern",
                        )),
                    ))
                    .id();
                let re_reserved = app
                    .world_mut()
                    .resource_mut::<crate::craft::RecipeUnlockState>()
                    .reserve_scroll_unlock(&player_id, &recipe_id);
                assert!(
                    !re_reserved,
                    "already-unlocked recipe must not reserve again (no double consume)"
                );
                app.world_mut().despawn(second);
            }
        }

        #[test]
        fn craft_scroll_unlock_with_dead_caster_entity_still_commits_via_player_id() {
            // verdict-1906-r2 major #3 的第二面：intent 携带的 caster 实体在消费帧
            // 已不存在（队列跨帧 + 实体换线/死亡清场）时，apply_unlock_intents 必须
            // 用 intent.player_id 完成解锁 + 释放 reservation，而不是因反查 caster
            // 失败而丢弃（旧实现 fallback entity:{bits} 导致 canonical reservation
            // 永久残留）。
            let mut app = production_scroll_request_app();
            let player_id = canonical_player_id("Azure");
            let recipe_id = crate::craft::RecipeId::new("workbench.shelter.lantern");

            // 先 reserve（模拟请求帧已扣物品、reservation 落账）。
            assert!(
                app.world_mut()
                    .resource_mut::<crate::craft::RecipeUnlockState>()
                    .reserve_scroll_unlock(&player_id, &recipe_id),
                "reservation must succeed before intent processing"
            );
            // spawn 后立即 despawn：caster 实体在消费帧不存在。
            let dead_caster = app.world_mut().spawn_empty().id();
            app.world_mut().despawn(dead_caster);

            app.world_mut().send_event(crate::craft::CraftUnlockIntent {
                caster: dead_caster,
                player_id: player_id.clone(),
                recipe_id: recipe_id.clone(),
                source: crate::craft::UnlockEventSource::Scroll {
                    item_template: "scroll_workbench_lantern".to_string(),
                },
            });
            app.update();

            let unlock_state = app.world().resource::<crate::craft::RecipeUnlockState>();
            assert!(
            unlock_state.is_unlocked(&player_id, &recipe_id),
            "unlock must commit via intent.player_id even when caster entity is already despawned"
        );
            assert_eq!(
                app.world_mut()
                    .resource_mut::<valence::prelude::Events<crate::craft::RecipeUnlockedEvent>>()
                    .drain()
                    .count(),
                1,
                "dead-caster intent must still emit one observable unlock"
            );
            // 旧实现会遗留 `entity:{bits}` 错位 reservation —— 解锁后再次请求同一
            // 卷，若残留锁未清，reserve 会返回 false 且第二张卷被吞。断言解锁后
            // reservation 被释放（未解锁配方可以重新 reserve）。
            let another_recipe = crate::craft::RecipeId::new("workbench.shelter.torch");
            assert!(
                app.world_mut()
                    .resource_mut::<crate::craft::RecipeUnlockState>()
                    .reserve_scroll_unlock(&player_id, &another_recipe),
                "reservation bookkeeping must stay consistent after dead-caster intent"
            );
        }

        #[test]
        fn queued_duplicate_craft_scroll_requests_consume_one_from_stack() {
            let mut app = production_scroll_request_app();
            let (client_bundle, _helper) = create_mock_client("Azure");
            let mut inventory =
                inventory_with_skill_scroll(skill_scroll_item(42, "scroll_workbench_lantern"));
            inventory.containers[0].items[0].instance.stack_count = 2;
            let entity = app.world_mut().spawn((client_bundle, inventory)).id();

            send_technique_scroll_use(&mut app, entity, 42);
            send_technique_scroll_use(&mut app, entity, 42);
            app.update();

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            assert_eq!(inventory.containers[0].items[0].instance.stack_count, 1);
            let player_id = canonical_player_id("Azure");
            assert!(
                app.world()
                    .resource::<crate::craft::RecipeUnlockState>()
                    .is_unlocked(
                        &player_id,
                        &crate::craft::RecipeId::new("workbench.shelter.lantern")
                    ),
                "the single accepted intent must commit the recipe unlock"
            );
            assert_eq!(
                app.world_mut()
                    .resource_mut::<valence::prelude::Events<crate::craft::RecipeUnlockedEvent>>()
                    .drain()
                    .count(),
                1,
                "queued duplicates must produce one observable unlock"
            );
        }

        #[test]
        fn queued_duplicate_craft_scroll_instances_consume_only_first_copy() {
            let mut app = production_scroll_request_app();
            let (client_bundle, _helper) = create_mock_client("Azure");
            let mut inventory =
                inventory_with_skill_scroll(skill_scroll_item(42, "scroll_workbench_lantern"));
            inventory.containers[0].items.push(PlacedItemState {
                row: 0,
                col: 1,
                instance: skill_scroll_item(43, "scroll_workbench_lantern"),
            });
            let entity = app.world_mut().spawn((client_bundle, inventory)).id();

            send_technique_scroll_use(&mut app, entity, 42);
            send_technique_scroll_use(&mut app, entity, 43);
            app.update();

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            assert_eq!(inventory.containers[0].items.len(), 1);
            assert_eq!(inventory.containers[0].items[0].instance.instance_id, 43);
            assert_eq!(
                app.world_mut()
                    .resource_mut::<valence::prelude::Events<crate::craft::RecipeUnlockedEvent>>()
                    .drain()
                    .count(),
                1,
                "two instance ids in one frame must commit one observable unlock"
            );
        }

        #[test]
        fn learn_skill_scroll_consumes_first_time_and_marks_consumed() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_systems(Update, handle_client_request_payloads);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(
                        42,
                        "skill_scroll_herbalism_baicao_can",
                    )),
                    SkillSet::default(),
                    Cultivation::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"learn_skill_scroll","v":1,"instance_id":42}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            assert!(inventory.containers[0].items.is_empty());
            let skill_set = app.world().get::<SkillSet>(entity).unwrap();
            assert!(skill_set
                .consumed_scrolls
                .contains(&ScrollId::new("skill_scroll_herbalism_baicao_can")));

            let xp_events: Vec<_> = app
                .world_mut()
                .resource_mut::<valence::prelude::Events<SkillXpGain>>()
                .drain()
                .collect();
            assert_eq!(xp_events.len(), 1);
            assert_eq!(xp_events[0].skill, SkillId::Herbalism);
            assert_eq!(xp_events[0].amount, 500);
            let used_events: Vec<_> = app
                .world_mut()
                .resource_mut::<valence::prelude::Events<SkillScrollUsed>>()
                .drain()
                .collect();
            assert_eq!(used_events.len(), 1);
            assert!(!used_events[0].was_duplicate);
            assert_eq!(used_events[0].xp_granted, 500);
        }

        #[test]
        fn learn_skill_scroll_duplicate_does_not_consume_item() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_systems(Update, handle_client_request_payloads);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut skill_set = SkillSet::default();
            skill_set
                .consumed_scrolls
                .insert(ScrollId::new("skill_scroll_herbalism_baicao_can"));
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(
                        42,
                        "skill_scroll_herbalism_baicao_can",
                    )),
                    skill_set,
                    Cultivation::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"learn_skill_scroll","v":1,"instance_id":42}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();
            flush_all_client_packets(&mut app);

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            assert_eq!(inventory.containers[0].items.len(), 1);
            assert!(
                has_inventory_snapshot_payload(&mut helper),
                "duplicate rejection must resync inventory after optimistic client drop"
            );
            let xp_events: Vec<_> = app
                .world_mut()
                .resource_mut::<valence::prelude::Events<SkillXpGain>>()
                .drain()
                .collect();
            assert!(xp_events.is_empty());
            let used_events: Vec<_> = app
                .world_mut()
                .resource_mut::<valence::prelude::Events<SkillScrollUsed>>()
                .drain()
                .collect();
            assert_eq!(used_events.len(), 1);
            assert!(used_events[0].was_duplicate);
            assert_eq!(used_events[0].xp_granted, 0);
        }

        #[test]
        fn learn_blueprint_consumes_scroll_item() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.insert_resource(test_forge_template_registry());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_event::<InscriptionScrollSubmit>();
            app.add_systems(Update, handle_client_request_payloads);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(
                        42,
                        "blueprint_scroll_ling_feng",
                    )),
                    Cultivation::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data:
                        br#"{"type":"forge_learn_blueprint","v":1,"blueprint_id":"ling_feng_v0"}"#
                            .to_vec()
                            .into_boxed_slice(),
                });

            app.update();
            app.update();

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            assert!(inventory.containers[0].items.is_empty());
            let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
            assert!(learned.knows("ling_feng_v0"));
        }

        // ══════════ plan-forge-session-entry-wiring-v1 §4.1#2/#3 — 分发层饱和测试 ══════════

        fn send_forge_start_session(
            app: &mut App,
            client: Entity,
            station_pos: (i32, i32, i32),
            blueprint_id: &str,
            materials: &[(&str, u32)],
        ) {
            let materials_json: Vec<String> = materials
                .iter()
                .map(|(m, c)| format!("[\"{m}\",{c}]"))
                .collect();
            let body = format!(
            "{{\"type\":\"forge_start_session\",\"v\":1,\"station_pos\":[{},{},{}],\"blueprint_id\":\"{blueprint_id}\",\"materials\":[{}]}}",
            station_pos.0,
            station_pos.1,
            station_pos.2,
            materials_json.join(",")
        );
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: body.into_bytes().into_boxed_slice(),
                });
        }

        fn send_forge_turn_page(app: &mut App, client: Entity, delta: i32) {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: format!(
                        r#"{{"type":"forge_blueprint_turn_page","v":1,"delta":{delta}}}"#
                    )
                    .into_bytes()
                    .into_boxed_slice(),
                });
        }

        fn collect_forge_blueprint_books(
            helper: &mut MockClientHelper,
        ) -> Vec<crate::schema::forge::ForgeBlueprintBookDataV1> {
            helper
                .collect_received()
                .0
                .into_iter()
                .filter_map(|frame| {
                    let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                    if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                        return None;
                    }
                    decode_forge_blueprint_book_payload(packet.data.0 .0)
                })
                .collect()
        }

        #[test]
        fn forge_start_session_dispatches_start_forge_request_for_owned_station() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.add_event::<StartForgeRequest>();

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let station = app
                .world_mut()
                .spawn(WeaponForgeStation::placed(
                    valence::prelude::BlockPos::new(8, 66, 8),
                    1,
                    entity,
                ))
                .id();

            send_forge_start_session(
                &mut app,
                entity,
                (8, 66, 8),
                "iron_sword_v0",
                &[("fan_tie", 3)],
            );
            app.update();

            let events = app
                .world()
                .resource::<valence::prelude::Events<StartForgeRequest>>();
            let sent: Vec<_> = events.iter_current_update_events().collect();
            assert_eq!(
                sent.len(),
                1,
                "本人拥有的砧 + 合法 pos 应恰好分发 1 条 StartForgeRequest"
            );
            assert_eq!(sent[0].station, station);
            assert_eq!(sent[0].caster, entity);
            assert_eq!(sent[0].blueprint, "iron_sword_v0");
            assert_eq!(sent[0].materials, vec![("fan_tie".to_string(), 3)]);
            flush_all_client_packets(&mut app);
            assert!(
                collect_game_messages(&mut helper)
                    .iter()
                    .all(|m| !m.contains("炼器")),
                "受理路径不应发出炼器错误 chat"
            );
        }

        #[test]
        fn forge_start_session_dispatches_for_unclaimed_station_with_no_owner() {
            // owner=None 的砧（系统/公用砧）应放行任何玩家起炉。
            let mut app = App::new();
            register_request_app(&mut app);
            app.add_event::<StartForgeRequest>();

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut().spawn(WeaponForgeStation {
                tier: 1,
                owner: None,
                session: None,
                integrity: 1.0,
                pos: Some((8, 66, 8)),
            });

            send_forge_start_session(
                &mut app,
                entity,
                (8, 66, 8),
                "iron_sword_v0",
                &[("fan_tie", 3)],
            );
            app.update();

            let events = app
                .world()
                .resource::<valence::prelude::Events<StartForgeRequest>>();
            assert_eq!(events.iter_current_update_events().count(), 1);
        }

        #[test]
        fn forge_start_session_rejects_missing_station_with_chat_error_and_no_dispatch() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.add_event::<StartForgeRequest>();

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            // 故意不 spawn 任何 WeaponForgeStation。

            send_forge_start_session(
                &mut app,
                entity,
                (8, 66, 8),
                "iron_sword_v0",
                &[("fan_tie", 3)],
            );
            app.update();

            let events = app
                .world()
                .resource::<valence::prelude::Events<StartForgeRequest>>();
            assert_eq!(
                events.iter_current_update_events().count(),
                0,
                "砧不存在时不应分发 StartForgeRequest"
            );
            flush_all_client_packets(&mut app);
            let messages = collect_game_messages(&mut helper);
            assert!(
                messages.iter().any(|m| m.contains("锻炉不存在")),
                "应回执锻炉不存在，实际收到：{messages:?}"
            );
        }

        #[test]
        fn forge_start_session_rejects_station_owned_by_someone_else() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.add_event::<StartForgeRequest>();

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let (other_bundle, _other_helper) = create_mock_client("Bob");
            let other_owner = app.world_mut().spawn(other_bundle).id();
            app.world_mut().spawn(WeaponForgeStation::placed(
                valence::prelude::BlockPos::new(8, 66, 8),
                1,
                other_owner,
            ));

            send_forge_start_session(
                &mut app,
                entity,
                (8, 66, 8),
                "iron_sword_v0",
                &[("fan_tie", 3)],
            );
            app.update();

            let events = app
                .world()
                .resource::<valence::prelude::Events<StartForgeRequest>>();
            assert_eq!(
                events.iter_current_update_events().count(),
                0,
                "非本人的砧不应分发 StartForgeRequest"
            );
            flush_all_client_packets(&mut app);
            let messages = collect_game_messages(&mut helper);
            assert!(
                messages.iter().any(|m| m.contains("不是你的")),
                "应回执所有权错误，实际收到：{messages:?}"
            );
        }

        fn forge_blueprint_registry_for_tests() -> BlueprintRegistry {
            BlueprintRegistry::load_dir_with_minerals(
                crate::forge::blueprint::DEFAULT_BLUEPRINTS_DIR,
                None,
            )
            .expect("default forge blueprints should load for dispatch tests")
        }

        #[test]
        fn forge_blueprint_turn_page_positive_delta_advances_and_echoes_s2c() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(forge_blueprint_registry_for_tests());

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    LearnedBlueprints {
                        ids: vec![
                            "iron_sword_v0".to_string(),
                            "qing_feng_v0".to_string(),
                            "ling_feng_v0".to_string(),
                        ],
                        current_index: 0,
                    },
                ))
                .id();

            send_forge_turn_page(&mut app, entity, 1);
            app.update();

            let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
            assert_eq!(learned.current_index, 1, "delta=1 应恰好前进 1 页");

            flush_all_client_packets(&mut app);
            let books = collect_forge_blueprint_books(&mut helper);
            assert_eq!(books.len(), 1, "翻页应恰好回推 1 条 forge_blueprint_book");
            assert_eq!(books[0].current_index, 1);
        }

        #[test]
        fn forge_blueprint_turn_page_negative_delta_wraps_to_last_page() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(forge_blueprint_registry_for_tests());

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    LearnedBlueprints {
                        ids: vec![
                            "iron_sword_v0".to_string(),
                            "qing_feng_v0".to_string(),
                            "ling_feng_v0".to_string(),
                        ],
                        current_index: 0,
                    },
                ))
                .id();

            send_forge_turn_page(&mut app, entity, -1);
            app.update();

            let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
            assert_eq!(
                learned.current_index, 2,
                "从第 0 页向前翻应 wrap 到最后一页（索引 2）"
            );
            flush_all_client_packets(&mut app);
            assert_eq!(collect_forge_blueprint_books(&mut helper).len(), 1);
        }

        #[test]
        fn forge_blueprint_turn_page_multi_step_delta_advances_that_many_pages() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(forge_blueprint_registry_for_tests());

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    LearnedBlueprints {
                        ids: vec![
                            "iron_sword_v0".to_string(),
                            "qing_feng_v0".to_string(),
                            "ling_feng_v0".to_string(),
                        ],
                        current_index: 0,
                    },
                ))
                .id();

            send_forge_turn_page(&mut app, entity, 2);
            app.update();

            let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
            assert_eq!(learned.current_index, 2, "delta=2 应前进恰好 2 页");
            flush_all_client_packets(&mut app);
            assert_eq!(collect_forge_blueprint_books(&mut helper).len(), 1);
        }

        #[test]
        fn forge_blueprint_turn_page_extreme_delta_is_bounded_by_len_modulo() {
            // 修复轮 major——恶意单包 delta=i32::MIN（unsigned_abs=2.1B）曾按次循环，
            // 一个包冻结整个 ECS tick 数秒（DoS）。守卫后按 |delta| % len 步进：
            // 2_147_483_648 % 3 = 2，负方向 prev 2 页，0 → 2 → 1，落点必须与逐步等价。
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(forge_blueprint_registry_for_tests());

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    LearnedBlueprints {
                        ids: vec![
                            "iron_sword_v0".to_string(),
                            "qing_feng_v0".to_string(),
                            "ling_feng_v0".to_string(),
                        ],
                        current_index: 0,
                    },
                ))
                .id();

            send_forge_turn_page(&mut app, entity, i32::MIN);
            app.update();

            let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
            assert_eq!(
                learned.current_index, 1,
                "i32::MIN 应按 2.1B % 3 = 2 步 prev 处理（0→2→1），且不冻结 tick"
            );
            flush_all_client_packets(&mut app);
            assert_eq!(
                collect_forge_blueprint_books(&mut helper).len(),
                1,
                "极端 delta 仍应回推一次 S2C（server 权威页码）"
            );
        }

        #[test]
        fn forge_blueprint_turn_page_delta_multiple_of_len_is_identity_but_echoes() {
            // 边界：|delta| 恰为 len 的整数倍 → %len 后 0 步，页码不动；但请求本身
            // 合法，仍回推 S2C（与 delta=0 的静默 noop 区分——那是无意义输入）。
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(forge_blueprint_registry_for_tests());

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    LearnedBlueprints {
                        ids: vec![
                            "iron_sword_v0".to_string(),
                            "qing_feng_v0".to_string(),
                            "ling_feng_v0".to_string(),
                        ],
                        current_index: 1,
                    },
                ))
                .id();

            send_forge_turn_page(&mut app, entity, 3);
            app.update();

            let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
            assert_eq!(learned.current_index, 1, "delta=len(3) 环回原页");
            flush_all_client_packets(&mut app);
            assert_eq!(collect_forge_blueprint_books(&mut helper).len(), 1);
        }

        #[test]
        fn forge_blueprint_turn_page_delta_zero_is_noop_no_s2c() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(forge_blueprint_registry_for_tests());

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    LearnedBlueprints {
                        ids: vec!["iron_sword_v0".to_string()],
                        current_index: 0,
                    },
                ))
                .id();

            send_forge_turn_page(&mut app, entity, 0);
            app.update();

            let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
            assert_eq!(learned.current_index, 0, "delta=0 不应改变页码");
            flush_all_client_packets(&mut app);
            assert!(
                collect_forge_blueprint_books(&mut helper).is_empty(),
                "delta=0 不应回推 S2C"
            );
        }

        #[test]
        fn forge_blueprint_turn_page_noop_when_never_learned_any_blueprint() {
            // LearnedBlueprints 组件懒插入：从未学过图谱的玩家没有这个组件，无书可翻。
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(forge_blueprint_registry_for_tests());

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();

            send_forge_turn_page(&mut app, entity, 1);
            app.update();

            assert!(
                app.world().get::<LearnedBlueprints>(entity).is_none(),
                "不应凭空创建 LearnedBlueprints 组件"
            );
            flush_all_client_packets(&mut app);
            assert!(collect_forge_blueprint_books(&mut helper).is_empty());
        }

        #[test]
        fn forge_blueprint_turn_page_noop_when_learned_list_empty() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(forge_blueprint_registry_for_tests());

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    LearnedBlueprints {
                        ids: vec![],
                        current_index: 0,
                    },
                ))
                .id();

            send_forge_turn_page(&mut app, entity, 1);
            app.update();

            let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
            assert_eq!(learned.current_index, 0, "空图谱列表翻页应无操作");
            flush_all_client_packets(&mut app);
            assert!(collect_forge_blueprint_books(&mut helper).is_empty());
        }

        #[test]
        fn forge_inscription_scroll_defers_consumption_and_emits_exact_item_event() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedInscriptionScrolls::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(test_forge_template_registry());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_event::<InscriptionScrollSubmit>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_inscription_scrolls).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(
                        43,
                        "inscription_scroll_sharp_v0",
                    )),
                    Cultivation::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();
            insert_test_forge_session(&mut app, 9, entity, ForgeStep::Inscription);
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_inscription_scroll","v":1,"session_id":9,"inscription_id":"sharp_v0"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            assert_eq!(
                inventory.containers[0].items.len(),
                1,
                "C2S 网关只能预检残卷，实际消费必须留给确认进入 Inscription 的 forge 系统"
            );
            let captured = app.world().resource::<CapturedInscriptionScrolls>();
            assert_eq!(captured.0.len(), 1);
            assert_eq!(captured.0[0].session, ForgeSessionId(9));
            assert_eq!(captured.0[0].caster, entity);
            assert_eq!(captured.0[0].item_instance_id, 43);
            assert_eq!(captured.0[0].inscription_id, "sharp_v0");
        }

        #[test]
        fn forge_inscription_scroll_rejects_invalid_session_before_consuming_item() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedInscriptionScrolls::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(test_forge_template_registry());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_event::<InscriptionScrollSubmit>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_inscription_scrolls).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(
                        43,
                        "inscription_scroll_sharp_v0",
                    )),
                    Cultivation::default(),
                    PlayerState::default(),
                    QuickSlotBindings::default(),
                    UnlockedStyles::default(),
                ))
                .id();
            insert_test_forge_session(&mut app, 9, entity, ForgeStep::Tempering);
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_inscription_scroll","v":1,"session_id":9,"inscription_id":"sharp_v0"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            assert_eq!(inventory.containers[0].items.len(), 1);
            let captured = app.world().resource::<CapturedInscriptionScrolls>();
            assert!(captured.0.is_empty());
        }

        #[test]
        fn forge_tempering_hit_emits_event() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedTemperingHits::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_event::<TemperingHit>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_tempering_hits).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            insert_test_forge_session(&mut app, 9, entity, ForgeStep::Tempering);
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_tempering_hit","v":1,"session_id":9,"beat":"H","ticks_remaining":4}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let captured = app.world().resource::<CapturedTemperingHits>();
            assert_eq!(captured.0.len(), 1);
            assert_eq!(captured.0[0].session, ForgeSessionId(9));
            assert_eq!(captured.0[0].beat, TemperBeat::Heavy);
            assert_eq!(captured.0[0].ticks_remaining, 4);
        }

        #[test]
        fn forge_tempering_hit_rejects_unknown_beat() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedTemperingHits::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_event::<TemperingHit>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_tempering_hits).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_tempering_hit","v":1,"session_id":9,"beat":"X","ticks_remaining":4}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let captured = app.world().resource::<CapturedTemperingHits>();
            assert!(captured.0.is_empty());
        }

        #[test]
        fn forge_consecration_inject_emits_event() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedConsecrationInjects::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_event::<ConsecrationInject>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_consecration_injects).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            insert_test_forge_session(&mut app, 11, entity, ForgeStep::Consecration);
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data:
                    br#"{"type":"forge_consecration_inject","v":1,"session_id":11,"qi_amount":2.5}"#
                        .to_vec()
                        .into_boxed_slice(),
            });

            app.update();

            let captured = app.world().resource::<CapturedConsecrationInjects>();
            assert_eq!(captured.0.len(), 1);
            assert_eq!(captured.0[0].session, ForgeSessionId(11));
            assert_eq!(captured.0[0].qi_amount, 2.5);
        }

        #[test]
        fn forge_consecration_inject_rejects_negative_qi() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedConsecrationInjects::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_event::<ConsecrationInject>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_consecration_injects).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_consecration_inject","v":1,"session_id":11,"qi_amount":-0.5}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let captured = app.world().resource::<CapturedConsecrationInjects>();
            assert!(captured.0.is_empty());
        }

        #[test]
        fn forge_step_advance_emits_event() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedStepAdvances::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_event::<StepAdvance>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_step_advances).chain(),
            );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            insert_test_forge_session(&mut app, 12, entity, ForgeStep::Tempering);
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"forge_step_advance","v":1,"session_id":12}"#
                        .to_vec()
                        .into_boxed_slice(),
                });

            app.update();

            let captured = app.world().resource::<CapturedStepAdvances>();
            assert_eq!(captured.0.len(), 1);
            assert_eq!(captured.0[0].session, ForgeSessionId(12));
            assert_eq!(captured.0[0].from_step, ForgeStep::Tempering);
        }

        #[test]
        fn forge_session_inputs_reject_wrong_caster() {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(load_test_technique_registry());
            app.insert_resource(CapturedTemperingHits::default());
            app.insert_resource(CapturedConsecrationInjects::default());
            app.insert_resource(CapturedStepAdvances::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_event::<TemperingHit>();
            app.add_event::<ConsecrationInject>();
            app.add_event::<StepAdvance>();
            app.add_systems(
                Update,
                (
                    handle_client_request_payloads,
                    capture_tempering_hits,
                    capture_consecration_injects,
                    capture_step_advances,
                )
                    .chain(),
            );

            let (owner_bundle, _owner_helper) = create_mock_client("Owner");
            let owner = app.world_mut().spawn(owner_bundle).id();
            let (attacker_bundle, _attacker_helper) = create_mock_client("Attacker");
            let attacker = app.world_mut().spawn(attacker_bundle).id();

            insert_test_forge_session(&mut app, 21, owner, ForgeStep::Tempering);
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: attacker,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_tempering_hit","v":1,"session_id":21,"beat":"H","ticks_remaining":4}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
            app.update();
            assert!(app.world().resource::<CapturedTemperingHits>().0.is_empty());

            insert_test_forge_session(&mut app, 22, owner, ForgeStep::Consecration);
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: attacker,
                channel: ident!("bong:client_request").into(),
                data:
                    br#"{"type":"forge_consecration_inject","v":1,"session_id":22,"qi_amount":2.5}"#
                        .to_vec()
                        .into_boxed_slice(),
            });
            app.update();
            assert!(app
                .world()
                .resource::<CapturedConsecrationInjects>()
                .0
                .is_empty());

            insert_test_forge_session(&mut app, 23, owner, ForgeStep::Tempering);
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: attacker,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"forge_step_advance","v":1,"session_id":23}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();
            assert!(app.world().resource::<CapturedStepAdvances>().0.is_empty());
        }

        #[test]
        fn skill_bar_bind_skill_then_cast_starts_skillbar_cast() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let target = app.world_mut().spawn(Position::new([1.0, 0.0, 0.0])).id();
            let entity = app.world_mut().spawn(client_bundle).id();
            // beng_quan 需要 LargeIntestine/SmallIntestine/TripleEnergizer opened=true + integrity ≥ 0.01
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            for id in [
                crate::cultivation::components::MeridianId::LargeIntestine,
                crate::cultivation::components::MeridianId::SmallIntestine,
                crate::cultivation::components::MeridianId::TripleEnergizer,
            ] {
                let m = ms.get_mut(id);
                m.opened = true;
                m.integrity = 1.0;
            }
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Induce,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                ms,
                SkillBarBindings::default(),
                QuickSlotBindings::default(),
                empty_inventory(),
                known(&["burst_meridian.beng_quan"]),
            ));
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 0,
                        target: Some(format!("entity_bits:{}", target.to_bits())),
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });

            app.update();

            let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
            assert!(matches!(
                &bindings.slots[0],
                SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.beng_quan"
            ));
            let casting = app.world().get::<Casting>(entity).unwrap();
            assert_eq!(casting.source, CastSource::SkillBar);
            assert_eq!(casting.slot, 0);
            assert_eq!(casting.bound_instance_id, None);
            assert_eq!(casting.duration_ticks, 8);
            assert_eq!(casting.complete_cooldown_ticks, 60);
        }

        #[test]
        fn runtime_only_direct_generic_can_be_learned_bound_and_cast() {
            const TECHNIQUE_ID: &str = "test.runtime_only_direct";
            const SCROLL_TEMPLATE_ID: &str = "test_runtime_only_direct_scroll";
            const SCROLL_INSTANCE_ID: u64 = 91_001;

            let registry = load_runtime_only_direct_generic_registry(TECHNIQUE_ID);

            let mut scroll_template = ItemTemplate::minimal_for_test(SCROLL_TEMPLATE_ID);
            scroll_template.category = ItemCategory::Scroll;
            scroll_template.technique_scroll_spec = Some(crate::inventory::TechniqueScrollSpec {
                kind: "technique".to_string(),
                skill_id: TECHNIQUE_ID.to_string(),
            });
            let item_registry = ItemRegistry::from_map(HashMap::from([(
                SCROLL_TEMPLATE_ID.to_string(),
                scroll_template,
            )]));

            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(registry);
            app.insert_resource(item_registry);
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    crate::cultivation::components::Cultivation::default(),
                    crate::cultivation::components::MeridianSystem::default(),
                    SkillBarBindings::default(),
                    QuickSlotBindings::default(),
                    inventory_with_skill_scroll(skill_scroll_item(
                        SCROLL_INSTANCE_ID,
                        SCROLL_TEMPLATE_ID,
                    )),
                    KnownTechniques::default(),
                ))
                .id();

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::TechniqueScrollUse {
                        v: 1,
                        instance_id: SCROLL_INSTANCE_ID,
                    })
                    .expect("technique-scroll request should serialize")
                    .into_boxed_slice(),
                });
            app.update();

            let known = app.world().get::<KnownTechniques>(entity).unwrap();
            assert!(
                known
                    .entries
                    .iter()
                    .any(|entry| entry.id == TECHNIQUE_ID && entry.active),
                "request-level scroll use must learn and activate a runtime-only technique"
            );
            let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
            assert!(
                inventory_item_by_instance_borrow(inventory, SCROLL_INSTANCE_ID).is_none(),
                "successful request-level learning must consume the exact scroll instance"
            );

            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":{"kind":"skill","skill_id":"test.runtime_only_direct"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 0,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });

            app.update();

            assert!(matches!(
                app.world()
                    .get::<SkillBarBindings>(entity)
                    .unwrap()
                    .get(0),
                Some(SkillSlot::Skill { skill_id }) if skill_id == "test.runtime_only_direct"
            ));
            let casting = app
                .world()
                .get::<Casting>(entity)
                .expect("runtime-only direct-generic cast must start");
            assert_eq!(
                casting.skill_id.as_deref(),
                Some("test.runtime_only_direct")
            );
            assert_eq!(casting.duration_ticks, 17);
            assert_eq!(casting.complete_cooldown_ticks, 83);
        }

        /// 通过公开文件 loader 加载一个仅存在于本测试 catalog 的 direct-generic 技法。
        /// 这样仍保留原来 17/83 的 fixture 语义，但不依赖 integration test 无法访问的
        /// `#[cfg(test)] TechniqueRegistry::load_for_tests_with_definition`。
        fn load_runtime_only_direct_generic_registry(id: &str) -> TechniqueRegistry {
            let catalog_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/cultivation/techniques.toml");
            let mut source = std::fs::read_to_string(&catalog_path)
                .expect("checked-in technique catalog must be readable");
            source.push_str(&format!(
                r#"

[[techniques]]
id = "{id}"
display_name = "运行时直施"
grade = "common"
description = "仅用于验证 direct-generic 技法走通用施法路径。"
required_realm = "Awaken"
required_meridians = []
required_race = {{ kind = "any" }}
qi_cost = 0.0
stamina_cost = 15.0
cast_ticks = 17
cooldown_ticks = 83
range = 2.8
icon_texture = "bong-client:textures/gui/items/test_runtime_only_direct.png"
category = "attack"
dispatch = "direct_generic"
"#
            ));
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock must be after unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "bong-client-request-handler-technique-{}-{unique}.toml",
                std::process::id()
            ));
            std::fs::write(&path, source).expect("temporary technique catalog must be writable");
            let registry = TechniqueRegistry::load_from_path(
                &path,
                &crate::body_plan::RaceRegistry::default(),
            )
            .expect("public loader must accept the direct-generic test fixture");
            let _ = std::fs::remove_file(path);
            registry
        }

        /// 槽位 1 绑定崩拳——「主动切槽取消」用例里那条**通过全部门禁**的新 cast
        /// （空槽位/未学会都会在 cancel 判定之前早退，测不到取消路径）。
        fn slot1_bound_to_beng_quan() -> SkillBarBindings {
            let mut bindings = SkillBarBindings::default();
            bindings.slots[1] = SkillSlot::Skill {
                skill_id: "burst_meridian.beng_quan".to_string(),
            };
            bindings
        }

        /// 崩拳的经脉前置（大肠/小肠/三焦 opened + integrity 足量）。
        fn beng_quan_ready_meridians() -> crate::cultivation::components::MeridianSystem {
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            for id in [
                crate::cultivation::components::MeridianId::LargeIntestine,
                crate::cultivation::components::MeridianId::SmallIntestine,
                crate::cultivation::components::MeridianId::TripleEnergizer,
            ] {
                let m = ms.get_mut(id);
                m.opened = true;
                m.integrity = 1.0;
            }
            ms
        }

        /// 引导中的施法快照（槽位 0），供「主动切槽取消」两用例复用。
        fn yidao_charge_casting(skill_id: &str) -> Casting {
            Casting {
                source: CastSource::SkillBar,
                slot: 0,
                started_at_tick: 0,
                duration_ticks: 1200,
                started_at_ms: 0,
                duration_ms: 60_000,
                bound_instance_id: None,
                start_position: DVec3::new(0.0, 64.0, 0.0),
                complete_cooldown_ticks: 60,
                skill_id: Some(skill_id.to_string()),
                skill_config: None,
            }
        }

        /// plan-skill-anim-fidelity-v1 P4（review r1 补）——**用户主动切槽取消**是
        /// `tick_casts_or_interrupt` 三打断分支之外的第四条退出路径：`Casting` 在
        /// `cancel_previous_cast` 里被提前 remove，那边再也看不到它。若此处不补发
        /// StopAnim，`bong:yidao_*_loop` 这类 isLoop 蓄力段会永卡客户端（yidao 引导
        /// 窗长达 60s，命中概率远高于 sword.infuse）。
        #[test]
        fn user_cancel_by_slot_switch_stops_looping_charge_anim() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            let unique_id = UniqueId::default();
            let expected_target = unique_id.0.to_string();
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 64.0, 0.0]),
                unique_id,
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Induce,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                beng_quan_ready_meridians(),
                slot1_bound_to_beng_quan(),
                QuickSlotBindings::default(),
                empty_inventory(),
                known(&["burst_meridian.beng_quan"]),
                // 引导中的接经术（循环蓄力段已在客户端播放）。
                yidao_charge_casting(crate::combat::yidao::MERIDIAN_REPAIR_SKILL_ID),
            ));

            // 切到另一个槽位施法 → 走 cancel_previous_cast（UserCancel）。
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 1,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });

            app.update();

            let stop_anims: Vec<(String, Option<u8>)> = app
                .world_mut()
                .resource_mut::<valence::prelude::Events<VfxEventRequest>>()
                .drain()
                .filter_map(|request| match request.payload {
                    VfxEventPayloadV1::StopAnim {
                        target_player,
                        anim_id,
                        fade_out_ticks,
                    } => {
                        assert_eq!(
                            target_player, expected_target,
                            "StopAnim 必须寻址到取消施法的玩家本人"
                        );
                        Some((anim_id, fade_out_ticks))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                stop_anims,
                vec![(
                    crate::combat::yidao::ANIM_YIDAO_MERIDIAN_REPAIR_LOOP.to_string(),
                    Some(3),
                )],
                "主动切槽取消必须恰停一次被取消招的循环蓄力段（否则动画永卡客户端）"
            );
        }

        /// 负向：被取消的招式**没有**登记循环蓄力段时，取消路径不得发多余 StopAnim
        /// （查表 miss = 该招本就没有需要停的循环动画）。
        #[test]
        fn user_cancel_of_non_looping_cast_emits_no_stop_anim() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 64.0, 0.0]),
                UniqueId::default(),
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Induce,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                beng_quan_ready_meridians(),
                slot1_bound_to_beng_quan(),
                QuickSlotBindings::default(),
                empty_inventory(),
                known(&["burst_meridian.beng_quan"]),
                // 被取消的招式未登记循环蓄力段（崩拳是瞬发三段式）。
                yidao_charge_casting("burst_meridian.beng_quan"),
            ));

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 1,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });

            app.update();

            let stop_anim_count = app
                .world_mut()
                .resource_mut::<valence::prelude::Events<VfxEventRequest>>()
                .drain()
                .filter(|request| matches!(request.payload, VfxEventPayloadV1::StopAnim { .. }))
                .count();
            assert_eq!(
                stop_anim_count, 0,
                "非循环段招式被取消时不得发 StopAnim（查表 miss 即无循环动画需要停）"
            );
        }

        #[test]
        fn skill_bar_cast_defined_skill_without_resolver_uses_generic_cast_path() {
            // body.guangbo_ticao 是仍未实装 resolver 的 skeleton 招（不在 SkillRegistry 内，
            // 无 required_meridians、无 SkillMeridianDependencies）→ 走通用施法路径，
            // 通用路径无条件插入 Casting 并把 SkillConfigStore 里的配置带入 Casting.skill_config。
            let mut app = App::new();
            register_request_app(&mut app);
            app.world_mut()
                .resource_mut::<SkillConfigStore>()
                .set_config(
                    "offline:Azure",
                    "body.guangbo_ticao",
                    crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([(
                        "stance".to_string(),
                        serde_json::json!("short"),
                    )])),
                );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            assert!(skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "body.guangbo_ticao".to_string(),
                },
            ));
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                known(&["body.guangbo_ticao"]),
            ));
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 0,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });

            app.update();

            let casting = app.world().get::<Casting>(entity).unwrap();
            assert_eq!(casting.source, CastSource::SkillBar);
            assert_eq!(casting.slot, 0);
            // cast/cd 来自 known_techniques.body.guangbo_ticao（cast 60 / cooldown 200）。
            assert_eq!(casting.duration_ticks, 60);
            assert_eq!(casting.complete_cooldown_ticks, 200);
            assert_eq!(casting.skill_id.as_deref(), Some("body.guangbo_ticao"));
            assert_eq!(
                casting
                    .skill_config
                    .as_ref()
                    .and_then(|config| config.fields.get("stance")),
                Some(&serde_json::json!("short"))
            );
        }

        #[test]
        fn skill_bar_cast_requires_config_for_schema_fixture() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            assert!(skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "zhenmai.sever_chain".to_string(),
                },
            ));
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                Cultivation {
                    realm: Realm::Void,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                known(&["zhenmai.sever_chain"]),
            ));

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 0,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
            app.update();
            assert!(app.world().get::<Casting>(entity).is_none());

            app.world_mut()
                .resource_mut::<SkillConfigStore>()
                .set_config(
                    "offline:Azure",
                    "zhenmai.sever_chain",
                    crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([
                        ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                        ("backfire_kind".to_string(), serde_json::json!("array")),
                    ])),
                );
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 0,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
            app.update();

            let casting = app.world().get::<Casting>(entity).unwrap();
            assert_eq!(casting.skill_id.as_deref(), Some("zhenmai.sever_chain"));
            assert_eq!(
                casting
                    .skill_config
                    .as_ref()
                    .and_then(|config| config.fields.get("backfire_kind")),
                Some(&serde_json::json!("array"))
            );

            app.world_mut()
                .resource_mut::<SkillConfigStore>()
                .set_config(
                    "offline:Azure",
                    "zhenmai.sever_chain",
                    crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([
                        ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                        (
                            "backfire_kind".to_string(),
                            serde_json::json!("tainted_yuan"),
                        ),
                    ])),
                );
            let casting = app.world().get::<Casting>(entity).unwrap();
            assert_eq!(
                casting
                    .skill_config
                    .as_ref()
                    .and_then(|config| config.fields.get("backfire_kind")),
                Some(&serde_json::json!("array"))
            );

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillConfigIntent {
                        v: 1,
                        skill_id: "zhenmai.sever_chain".to_string(),
                        config: std::collections::BTreeMap::from([(
                            "backfire_kind".to_string(),
                            serde_json::json!("invalid"),
                        )]),
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
            app.update();
            flush_all_client_packets(&mut app);
            let snapshots = collect_skill_config_snapshots(&mut helper);
            assert_eq!(snapshots.len(), 1);
            assert_eq!(
                snapshots[0]
                    .configs
                    .get("zhenmai.sever_chain")
                    .and_then(|config| config.fields.get("backfire_kind")),
                Some(&serde_json::json!("tainted_yuan"))
            );
        }

        #[test]
        fn valid_skill_config_intent_replies_with_authoritative_snapshot() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillConfigIntent {
                        v: 1,
                        skill_id: "zhenmai.sever_chain".to_string(),
                        config: std::collections::BTreeMap::from([
                            ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                            ("backfire_kind".to_string(), serde_json::json!("array")),
                        ]),
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });

            app.update();
            flush_all_client_packets(&mut app);
            let snapshots = collect_skill_config_snapshots(&mut helper);

            assert_eq!(snapshots.len(), 1);
            assert_eq!(
                snapshots[0]
                    .configs
                    .get("zhenmai.sever_chain")
                    .and_then(|config| config.fields.get("backfire_kind")),
                Some(&serde_json::json!("array"))
            );
        }

        #[test]
        fn skill_bar_cast_rejects_when_skill_config_schemas_missing() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.world_mut().remove_resource::<SkillConfigSchemas>();

            let (client_bundle, _helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            assert!(skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "zhenmai.sever_chain".to_string(),
                },
            ));
            let entity = app.world_mut().spawn(client_bundle).id();
            // Grant the technique so the ownership gate passes; the rejection is caused by the
            // missing SkillConfigSchemas resource, not by lack of ownership.
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                known(&["zhenmai.sever_chain"]),
            ));
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 0,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });

            app.update();

            assert!(app.world().get::<Casting>(entity).is_none());
        }

        // ─────────────────────────────────────────────────────────────────────────
        // plan-bug-qc-p1 §skill-cast P0：经脉门控单元 + 集成测试 (11 tests)
        // ─────────────────────────────────────────────────────────────────────────

        /// 测试辅助：从 MockClientHelper 中提取第一个 CastSync payload。
        fn collect_cast_syncs(helper: &mut MockClientHelper) -> Vec<CastSyncV1> {
            helper
                .collect_received()
                .0
                .into_iter()
                .filter_map(|frame| {
                    let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                    if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                        return None;
                    }
                    decode_cast_sync_payload(packet.data.0 .0)
                })
                .collect()
        }

        /// Build a minimal KnownTechniques component with exactly the listed technique ids
        /// (active=true, proficiency=0.5). Use in skill_bar tests to grant only the
        /// technique under test so the ownership gate passes without granting everything.
        fn known(ids: &[&str]) -> KnownTechniques {
            use crate::cultivation::known_techniques::KnownTechnique;
            KnownTechniques {
                entries: ids
                    .iter()
                    .map(|id| KnownTechnique {
                        id: (*id).to_string(),
                        proficiency: 0.5,
                        active: true,
                    })
                    .collect(),
            }
        }

        /// 发送一个 skill_bar_cast 消息（slot 0）给 entity，并驱动一次 app.update()。
        fn send_skill_bar_cast(app: &mut App, entity: Entity) {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 0,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
            app.update();
        }

        /// 同 `send_skill_bar_cast`，但带 `entity_bits:` 目标（resolver 招式需要 target）。
        fn send_skill_bar_cast_with_target(app: &mut App, entity: Entity, target: Entity) {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 0,
                        target: Some(format!("entity_bits:{}", target.to_bits())),
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
            app.update();
        }

        // ── 1. happy path：经脉门通过 → resolver 施放成功 ─────────────────────────

        #[test]
        fn skill_bar_cast_meridian_gate_passes_when_all_deps_satisfied_resolver_path() {
            // burst_meridian.tie_shan_kao 现已实装 resolver，required_meridians 要 Stomach
            // opened=true + integrity ≥ 0.5。把经脉门和 resolver 自身的前置（target / realm
            // Condense / qi ≥ 35 / Stomach 可用）全补齐 → 经脉门放行后 resolver 真正施放 →
            // Casting 由 resolver 插入（cast 10 / cd 70，来自 known_techniques.tie_shan_kao）。
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            // 近身目标（TIE_SHAN_KAO reach max = 1.0，距离 1.0 命中）。
            let target = app.world_mut().spawn(Position::new([1.0, 0.0, 0.0])).id();
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.tie_shan_kao".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            // Stomach opened=true + integrity=1.0 ≥ min_health(0.5)：经脉门 + resolver 均放行。
            {
                let stomach = ms.get_mut(crate::cultivation::components::MeridianId::Stomach);
                stomach.opened = true;
                stomach.integrity = 1.0;
            }
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
                // resolver 前置：realm Condense + qi ≥ 35。
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Condense,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                known(&["burst_meridian.tie_shan_kao"]),
            ));

            send_skill_bar_cast_with_target(&mut app, entity, target);

            let casting = app.world().get::<Casting>(entity).expect(
                "Stomach opened=true + integrity=1.0 ≥ min_health=0.5 + realm/qi/target 满足时，\
             经脉门应放行 → resolver 施放成功；期望 Casting 存在；实际 Casting=None，\
             说明经脉门错误拦截了满足条件的 cast",
            );
            // resolver 路径插入的 Casting：cast/cd 来自 known_techniques.tie_shan_kao（10 / 70）。
            assert_eq!(casting.source, CastSource::SkillBar);
            assert_eq!(casting.duration_ticks, 10, "tie_shan_kao cast_ticks");
            assert_eq!(casting.complete_cooldown_ticks, 70, "tie_shan_kao cooldown");
        }

        // ── 2. 门控：required_meridians integrity 不足 → 拒绝（generic 路径）────

        #[test]
        fn skill_bar_cast_meridian_gate_rejects_when_required_meridian_integrity_too_low() {
            // burst_meridian.beng_quan 需要 LargeIntestine/SmallIntestine/TripleEnergizer integrity >= 0.01
            // 把 LargeIntestine 降到 0.0 → gate 应拒绝
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            // LargeIntestine integrity = 0.0 < min_health(0.01) → 应触发 gate
            ms.get_mut(crate::cultivation::components::MeridianId::LargeIntestine)
                .integrity = 0.0;
            ms.get_mut(crate::cultivation::components::MeridianId::SmallIntestine)
                .integrity = 0.5;
            ms.get_mut(crate::cultivation::components::MeridianId::TripleEnergizer)
                .integrity = 0.5;
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
                // Grant ownership so the rejection is caused by the meridian gate, not by missing KnownTechniques.
                known(&["burst_meridian.beng_quan"]),
            ));

            send_skill_bar_cast(&mut app, entity);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            assert!(
            app.world().get::<Casting>(entity).is_none(),
            "LargeIntestine integrity=0.0 < min_health=0.01 时 cast 应被拒绝（无 Casting component）；\
             期望无 Casting 因为经脉 integrity 不足；实际 Casting 存在，说明 gate 未生效"
        );
            assert!(
                syncs
                    .iter()
                    .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
                "gate 拒绝时应推送 CastSyncV1{{outcome=MeridianGated}} 反馈；\
             期望至少一条 MeridianGated sync 因为经脉 integrity 不足；\
             实际 syncs={syncs:?}"
            );
        }

        // ── 3. 门控：SEVERED 经脉 → 拒绝（generic 路径）──────────────────────────

        #[test]
        fn skill_bar_cast_meridian_gate_rejects_when_required_meridian_severed() {
            // burst_meridian.beng_quan 需要 LargeIntestine；SEVERED → gate 拒绝
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            ms.get_mut(crate::cultivation::components::MeridianId::LargeIntestine)
                .integrity = 0.5;
            ms.get_mut(crate::cultivation::components::MeridianId::SmallIntestine)
                .integrity = 0.5;
            ms.get_mut(crate::cultivation::components::MeridianId::TripleEnergizer)
                .integrity = 0.5;
            let mut severed =
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default();
            severed.insert(
                crate::cultivation::components::MeridianId::LargeIntestine,
                crate::cultivation::meridian::severed::SeveredSource::CombatWound,
                1,
            );
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                severed,
                // Grant ownership so the rejection is caused by the meridian gate, not by missing KnownTechniques.
                known(&["burst_meridian.beng_quan"]),
            ));

            send_skill_bar_cast(&mut app, entity);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            assert!(
                app.world().get::<Casting>(entity).is_none(),
                "LargeIntestine SEVERED 时 burst_meridian.beng_quan cast 应被拒绝；\
             期望无 Casting 因为 SEVERED 经脉在 required_meridians 中；实际 Casting 存在"
            );
            assert!(
                syncs
                    .iter()
                    .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
                "SEVERED 拒绝时应推送 MeridianGated sync；期望 MeridianGated；实际 syncs={syncs:?}"
            );
        }

        // ── 4. SkillMeridianDependencies 表控：声明依赖但未打通 → 拒绝（generic 路径）

        #[test]
        fn skill_bar_cast_meridian_gate_rejects_via_deps_table_when_severed() {
            // 在 SkillMeridianDependencies 表中声明 "sword.cleave"（无内置 required_meridians）
            // 依赖 LargeIntestine，把它 SEVERED → gate 应拒绝
            let mut app = App::new();
            register_request_app(&mut app);
            // 声明依赖
            app.world_mut()
                .resource_mut::<SkillMeridianDependencies>()
                .declare(
                    "sword.cleave",
                    vec![crate::cultivation::components::MeridianId::LargeIntestine],
                );

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "sword.cleave".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let ms = crate::cultivation::components::MeridianSystem::default(); // LargeIntestine integrity 默认 1.0
            let mut severed =
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default();
            severed.insert(
                crate::cultivation::components::MeridianId::LargeIntestine,
                crate::cultivation::meridian::severed::SeveredSource::TribulationFail,
                100,
            );
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                severed,
                // Grant ownership so the rejection is caused by the meridian deps_table gate, not by missing KnownTechniques.
                known(&["sword.cleave"]),
            ));

            send_skill_bar_cast(&mut app, entity);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            assert!(
            app.world().get::<Casting>(entity).is_none(),
            "SkillMeridianDependencies 中声明 LargeIntestine 依赖且该经脉 SEVERED 时应拒绝 cast；\
             期望无 Casting；实际 Casting 存在，说明 deps_table 路径未被 gate 覆盖"
        );
            assert!(
                syncs
                    .iter()
                    .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
                "deps_table 拒绝应推送 MeridianGated；实际 syncs={syncs:?}"
            );
        }

        // ── 5. 无 deps 的招 → 放行──────────────────────────────────────────────────

        #[test]
        fn skill_bar_cast_meridian_gate_passes_for_skill_with_no_deps() {
            // sword.cleave 无内置 required_meridians，且 deps_table 未声明依赖 → gate 不拦
            // 有非依赖经脉 SEVERED（Gallbladder）—— 验证 gate 不误伤无关经脉
            let mut app = App::new();
            register_request_app(&mut app);
            // 不声明任何 SkillMeridianDependencies

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "sword.cleave".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let ms = crate::cultivation::components::MeridianSystem::default();
            // 设置一条无关经脉 SEVERED，验证不会误伤
            let mut severed =
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default();
            severed.insert(
                crate::cultivation::components::MeridianId::Gallbladder, // 非依赖
                crate::cultivation::meridian::severed::SeveredSource::CombatWound,
                1,
            );
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                severed,
                // Grant ownership so the cast can reach the meridian gate (and pass it), making the
                // "no MeridianGated" assertion test the gate rather than the ownership gate.
                known(&["sword.cleave"]),
            ));

            send_skill_bar_cast(&mut app, entity);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            // 核心不变量：无 deps 招式 gate 不误拦，不发 MeridianGated
            // sword.cleave 走 resolver 路径，resolver 可能因 Weapon/Qi 不足拒绝（非 gate 原因）
            // 我们只锁住"gate 未因无关 SEVERED 经脉误触 MeridianGated"
            assert!(
                !syncs
                    .iter()
                    .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
                "无 deps 招式（sword.cleave）不应被经脉门拦截，不应出现 MeridianGated sync；\
             期望 syncs 中无 MeridianGated（因为该招无经脉依赖，Gallbladder SEVERED 是无关经脉）；\
             实际 syncs={syncs:?}"
            );
        }

        // ── 6. resolver 路径也受门控（以 SkillMeridianDependencies 为例）────────────

        #[test]
        fn skill_bar_cast_meridian_gate_covers_resolver_path_via_deps_table() {
            // sword.cleave 有 resolver；在 deps_table 里声明 LargeIntestine 依赖，SEVERED → gate 拒绝
            // 验证 gate 在 resolver 路径也生效（gate 在 resolver 分支之前检查）
            let mut app = App::new();
            register_request_app(&mut app);
            app.world_mut()
                .resource_mut::<SkillMeridianDependencies>()
                .declare(
                    "sword.cleave",
                    vec![crate::cultivation::components::MeridianId::LargeIntestine],
                );

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "sword.cleave".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let ms = crate::cultivation::components::MeridianSystem::default();
            let mut severed =
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default();
            severed.insert(
                crate::cultivation::components::MeridianId::LargeIntestine,
                crate::cultivation::meridian::severed::SeveredSource::BackfireOverload,
                200,
            );
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                severed,
                // Grant ownership so the rejection is caused by the meridian deps_table gate, not by missing KnownTechniques.
                known(&["sword.cleave"]),
            ));

            send_skill_bar_cast(&mut app, entity);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            // resolver 路径被 gate 拦截时：gate 在 commands.add() 之前 return
            // → resolver 的 World closure 根本不会运行（没有 commands.add 被提交）
            // → entity 无 Casting component（resolver 未运行）
            assert!(
            app.world().get::<Casting>(entity).is_none(),
            "gate 在 commands.add() 之前 return，resolver 闭包不运行 → 不应插入 Casting；\
             期望 Casting=None；实际 Casting 存在，说明 resolver 路径未被门控（gate return 没阻止 commands.add）"
        );
            assert!(
                syncs
                    .iter()
                    .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
                "resolver 路径下 SEVERED deps_table 依赖应触发 MeridianGated；\
             期望 MeridianGated sync；实际 syncs={syncs:?}"
            );
        }

        // ── 7. 边界：integrity 刚好等于阈值（off-by-one）────────────────────────────

        #[test]
        fn skill_bar_cast_meridian_gate_passes_when_integrity_exactly_at_min_health() {
            // 经脉门边界：burst_meridian.tie_shan_kao 需要 Stomach integrity >= 0.5。
            // integrity 恰好 = 0.5（off-by-one 边界）应放行（>= 成立）；resolver 其余前置补齐
            // → 经脉门放行后 resolver 真正插入 Casting。
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let target = app.world_mut().spawn(Position::new([1.0, 0.0, 0.0])).id();
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.tie_shan_kao".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            // Stomach opened=true + integrity=0.5 恰好等于 min_health：应放行
            {
                let stomach = ms.get_mut(crate::cultivation::components::MeridianId::Stomach);
                stomach.opened = true;
                stomach.integrity = 0.5;
            }
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Condense,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                known(&["burst_meridian.tie_shan_kao"]),
            ));

            send_skill_bar_cast_with_target(&mut app, entity, target);

            assert!(
            app.world().get::<Casting>(entity).is_some(),
            "Stomach opened=true + integrity=0.5 恰好等于 min_health=0.5 时经脉门应放行（>= 成立）\
             → resolver 施放；期望 Casting 存在；实际无 Casting，说明经脉门边界判断为 < 而非 >=（off-by-one）"
        );
        }

        #[test]
        fn skill_bar_cast_meridian_gate_rejects_when_integrity_just_below_min_health() {
            // burst_meridian.tie_shan_kao 需要 Stomach integrity >= 0.5
            // 设置 integrity = 0.499（低于 min_health）→ 应拒绝
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.tie_shan_kao".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            ms.get_mut(crate::cultivation::components::MeridianId::Stomach)
                .integrity = 0.499; // 低于阈值
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
                // Grant ownership so the rejection is caused by the meridian gate (integrity too low), not by missing KnownTechniques.
                known(&["burst_meridian.tie_shan_kao"]),
            ));

            send_skill_bar_cast(&mut app, entity);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            assert!(
                app.world().get::<Casting>(entity).is_none(),
                "Stomach integrity=0.499 低于 min_health=0.5 时应拒绝 cast；\
             期望无 Casting；实际 Casting 存在，说明 integrity 检查 off-by-one（应为 < 而非 <=）"
            );
            assert!(
                syncs
                    .iter()
                    .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
                "integrity 不足应推送 MeridianGated；实际 syncs={syncs:?}"
            );
        }

        // ── 7b. 未打通经脉（integrity 满足但 opened=false）→ 拒绝，锁住核心不变量 ────

        #[test]
        fn skill_bar_cast_meridian_gate_rejects_when_required_meridian_not_opened() {
            // burst_meridian.tie_shan_kao 需要 Stomach integrity >= 0.5
            // 设置 Stomach integrity=1.0（满足阈值）但 opened=false（未打通）→ gate 应拒绝
            // 这是核心正典约束：「经脉没通就放不出招」，opened 先于 integrity 决定能否施放
            let mut app = App::new();
            register_request_app(&mut app);
            app.world_mut()
                .resource_mut::<SkillConfigStore>()
                .set_config(
                    "offline:Azure",
                    "burst_meridian.tie_shan_kao",
                    crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([(
                        "stance".to_string(),
                        serde_json::json!("short"),
                    )])),
                );

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.tie_shan_kao".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            // integrity 满足但经脉未打通：应拒绝（opened=false 默认）
            ms.get_mut(crate::cultivation::components::MeridianId::Stomach)
                .integrity = 1.0; // ≥ min_health=0.5，但 opened 仍为 false（默认）
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
                // Grant ownership so the rejection is caused by the meridian gate (not opened), not by missing KnownTechniques.
                known(&["burst_meridian.tie_shan_kao"]),
            ));

            send_skill_bar_cast(&mut app, entity);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            assert!(
                app.world().get::<Casting>(entity).is_none(),
                "Stomach opened=false 时 cast 应被拒绝，即使 integrity=1.0 满足阈值；\
             期望无 Casting 因为经脉未打通（正典：经脉没通就放不出招）；\
             实际 Casting 存在，说明 opened 检查未生效"
            );
            assert!(
                syncs
                    .iter()
                    .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
                "未打通经脉拒绝时应推送 MeridianGated sync；\
             期望 MeridianGated 因为 opened=false；实际 syncs={syncs:?}"
            );
        }

        // ── 8. 多经脉部分满足 → 拒绝（generic 路径）───────────────────────────────

        #[test]
        fn skill_bar_cast_meridian_gate_rejects_when_only_partial_deps_satisfied() {
            // burst_meridian.beng_quan 需要 LargeIntestine + SmallIntestine + TripleEnergizer
            // 满足前两个，第三个 integrity=0.0 → 应拒绝
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            ms.get_mut(crate::cultivation::components::MeridianId::LargeIntestine)
                .integrity = 0.5; // 满足
            ms.get_mut(crate::cultivation::components::MeridianId::SmallIntestine)
                .integrity = 0.5; // 满足
            ms.get_mut(crate::cultivation::components::MeridianId::TripleEnergizer)
                .integrity = 0.0; // 不满足
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
                // Grant ownership so the rejection is caused by the meridian gate (partial deps), not by missing KnownTechniques.
                known(&["burst_meridian.beng_quan"]),
            ));

            send_skill_bar_cast(&mut app, entity);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            assert!(
                app.world().get::<Casting>(entity).is_none(),
                "多经脉依赖中 TripleEnergizer integrity=0.0 < min_health=0.01 时应拒绝；\
             期望无 Casting；实际 Casting 存在，说明 gate 未检查全部依赖"
            );
            assert!(
                syncs
                    .iter()
                    .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
                "部分满足多依赖时应推送 MeridianGated；实际 syncs={syncs:?}"
            );
        }

        // ── 9. entity 无 MeridianSystem → 放行（pre-init 玩家兼容）───────────────

        #[test]
        fn skill_bar_cast_meridian_gate_passes_when_no_meridian_system_component() {
            // entity 无 MeridianSystem component（pre-init 玩家）→ 经脉门应 skip 放行。
            // 用 body.guangbo_ticao（仍是 skeleton：无 resolver、无 required_meridians、无 deps）
            // 作载体：经脉门放行后走通用路径，无条件插入 Casting，纯粹锁住「无 MeridianSystem 放行」语义。
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "body.guangbo_ticao".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                // 故意不插入 MeridianSystem
                known(&["body.guangbo_ticao"]),
            ));

            send_skill_bar_cast(&mut app, entity);

            // 无 MeridianSystem → gate skip → cast 放行（generic 路径）→ Casting 存在
            assert!(
                app.world().get::<Casting>(entity).is_some(),
                "entity 无 MeridianSystem 时 gate 应 skip 放行（pre-init 兼容）；\
             期望 Casting 存在；实际无 Casting，说明 gate 在无 MeridianSystem 时错误拒绝了"
            );
        }

        // ── 10. 回归：既有 skill_bar_cast_defined_skill_without_resolver 不破 ────────

        #[test]
        fn skill_bar_cast_meridian_gate_regression_no_deps_generic_path_still_works() {
            // body.guangbo_ticao 是无 resolver / 无 required_meridians / 无 deps 的 skeleton 招，
            // entity 有 MeridianSystem → 经脉门无依赖可查直接放行 → 走通用路径成功施放。
            // 这是对 "skill_bar_cast_defined_skill_without_resolver_uses_generic_cast_path" 的回归验证：
            // 引入经脉门后，无依赖招的通用路径行为不变。
            let mut app = App::new();
            register_request_app(&mut app);
            app.world_mut()
                .resource_mut::<SkillConfigStore>()
                .set_config(
                    "offline:Azure",
                    "body.guangbo_ticao",
                    crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([(
                        "stance".to_string(),
                        serde_json::json!("short"),
                    )])),
                );

            let (client_bundle, _helper) = create_mock_client("Azure");
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "body.guangbo_ticao".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            // 带一条 SEVERED 的无关经脉，验证经脉门不误伤无依赖招。
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            {
                let stomach = ms.get_mut(crate::cultivation::components::MeridianId::Stomach);
                stomach.opened = true;
                stomach.integrity = 1.0;
            }
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
                known(&["body.guangbo_ticao"]),
            ));

            send_skill_bar_cast(&mut app, entity);

            let casting = app.world().get::<Casting>(entity).expect(
            "回归：body.guangbo_ticao（无依赖 skeleton 招）有 MeridianSystem 时应成功施放（与引入 gate 前行为一致）",
        );
            assert_eq!(casting.source, CastSource::SkillBar);
            assert_eq!(
                casting.skill_id.as_deref(),
                Some("body.guangbo_ticao"),
                "skill_id 应与绑定技能一致"
            );
        }

        // ── 10b. 通用技能警示 HUD：resolver-path 拒绝把原因推回 client ───────────────
        //
        // plan-skill-warn-hud：以前 resolver 路径的 CastResult::Rejected 只 tracing::debug
        // 默默 return，client 完全收不到 → 玩家"按了键没反应"。现在每个 resolver 拒绝都推
        // 一条 CastSyncV1{phase: Idle, outcome: Reject*}，通用警示 HUD 据此弹中文提示。

        #[test]
        fn skill_bar_cast_resolver_reject_pushes_cast_sync_with_reason() {
            // 经脉门放行（Stomach opened+integrity 满足）+ 提供近身目标，但 realm 默认 Awaken
            // < tie_shan_kao 要求的 Condense → resolver 在 check_realm_gate 处拒绝 RealmTooLow。
            // 期望：① 无 Casting（被 resolver 拒绝）② 推送 CastSyncV1{outcome=RejectRealmTooLow}。
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Azure");
            let target = app.world_mut().spawn(Position::new([1.0, 0.0, 0.0])).id();
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.tie_shan_kao".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            let mut ms = crate::cultivation::components::MeridianSystem::default();
            {
                // 经脉门放行：opened=true + integrity=1.0 ≥ min_health=0.5。
                let stomach = ms.get_mut(crate::cultivation::components::MeridianId::Stomach);
                stomach.opened = true;
                stomach.integrity = 1.0;
            }
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                ms,
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
                // realm = Awaken（默认）< Condense → resolver check_realm_gate 拒绝 RealmTooLow。
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Awaken,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                // Grant ownership so the rejection is caused by the resolver (RealmTooLow), not by missing KnownTechniques.
                known(&["burst_meridian.tie_shan_kao"]),
            ));

            send_skill_bar_cast_with_target(&mut app, entity, target);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            assert!(
                app.world().get::<Casting>(entity).is_none(),
                "realm Awaken < Condense 时 resolver 应拒绝；期望无 Casting；实际 Casting 存在",
            );
            assert!(
            syncs.iter().any(|s| s.outcome
                == crate::schema::combat_hud::CastOutcomeV1::RejectRealmTooLow
                && s.phase == CastPhaseV1::Idle),
            "resolver 拒绝 RealmTooLow 时应推 CastSyncV1{{phase=Idle, outcome=RejectRealmTooLow}} \
             让通用警示 HUD 显示「境界不足」；期望命中该 sync；实际 syncs={syncs:?}",
        );
        }

        // ── 10c. F1（P3 opus verify 发现）：施放门行为测试 —— 通用 skill_bar 路径 ─────
        //
        // 修复前只有 known_techniques.rs 的 `required_race.allows(...)` 真值表 pin，从不
        // 触达 `handle_skill_bar_cast` 里真实的 race gate 判定代码（line ~12013-12036）
        // ——回归删掉那段 `if !definition.required_race.allows(...) { RejectRaceMismatch }`
        // 整块不会撞红。本节直接驱动真实 cast 入口（`send_skill_bar_cast`）锁死该行为。

        /// 装载仓库中真实的非人形 `whale` 资产，避免在 integration test 中复刻
        /// `BodyPlanRegistry`/`RaceRegistry` 的 `#[cfg(test)]` 构造器或生产数据。
        fn non_humanoid_race_fixture() -> (
            crate::body_plan::BodyPlanRegistry,
            crate::body_plan::RaceRegistry,
        ) {
            let assets_root = crate::body_plan::resolve_assets_root();
            let body_plans = crate::body_plan::BodyPlanRegistry::load_dir(
                assets_root.join(crate::body_plan::registry::DEFAULT_BODY_PLANS_DIR),
            )
            .expect("checked-in body-plan assets must load");
            let races = crate::body_plan::RaceRegistry::load_file(
                assets_root.join(crate::body_plan::race_registry::DEFAULT_RACES_PATH),
                &body_plans,
            )
            .expect("checked-in race assets must load");
            (body_plans, races)
        }

        /// 装配一个持剑、已习得 sword.cleave 的 caster；`race` 为 `None` 时不插入
        /// `RaceRegistry`/`BodyPlanRegistry`（退化到 humanoid 单例，人形本体基线）；
        /// 为 `Some(race_id)` 时插入 checked-in registry 并把 Cultivation.race 设为该 id
        /// （非人形本体；当前调用使用真实的 `whale` race）。
        fn setup_sword_cleave_caster(
            app: &mut App,
            username: &str,
            race: Option<&str>,
        ) -> (Entity, MockClientHelper) {
            if race.is_some() {
                let (body_plans, races) = non_humanoid_race_fixture();
                app.insert_resource(body_plans);
                app.insert_resource(races);
            }
            let (client_bundle, helper) = create_mock_client(username);
            let mut skill_bar = SkillBarBindings::default();
            skill_bar.set(
                0,
                SkillSlot::Skill {
                    skill_id: "sword.cleave".to_string(),
                },
            );
            let entity = app.world_mut().spawn(client_bundle).id();
            // `Some(race_id)` 时 `Cultivation.race` 设为该真实 race id（fixture 以它为
            // 键注册了 is_humanoid=false 构型，生产 `resolve_body_plan` 即解析出非人形
            // 本体）；`None` 时不插 fixture，退化到 humanoid 单例（HUMAN_RACE_ID）。
            // 是否人形由「race 是否落在非人形 fixture」决定，不看 id 字符串字面意义。
            let cultivation = crate::cultivation::components::Cultivation {
                realm: Realm::Induce,
                qi_current: 42.0,
                qi_max: 100.0,
                race: match race {
                    Some(race_id) => crate::body_plan::RaceId::new(race_id),
                    None => crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                },
                ..Default::default()
            };
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                crate::combat::weapon::Weapon {
                    slot: crate::combat::weapon::EquipSlot::MainHand,
                    instance_id: 1,
                    template_id: "test_sword".to_string(),
                    weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                    base_attack: 10.0,
                    quality_tier: 0,
                    durability: 100.0,
                    durability_max: 100.0,
                },
                cultivation,
                known(&["sword.cleave"]),
            ));
            (entity, helper)
        }

        #[test]
        fn skill_bar_cast_race_gate_rejects_non_humanoid_caster_before_resolver_qi_untouched() {
            // sword.cleave 全数据表标 RaceGate::Humanoid（§8.1 #6）。非人形本体
            // （race="test_whale" + BodyPlan.is_humanoid=false）施放必须在到达 resolver
            // （cast_sword_cleave）之前被通用路径的 race gate 拒绝：① 推
            // CastSyncV1{outcome=RejectRaceMismatch} ② resolver 从未运行——用零
            // AttackIntent 事件锁死（resolver 只要跑起来必发一条 AttackIntent，见
            // `combat::sword_basics::cast_sword_attack`）③ qi_current 分毫不动（守恒律：
            // race gate 拒绝不该扣任何真元）。
            let mut app = App::new();
            register_request_app(&mut app);
            let (entity, mut helper) = setup_sword_cleave_caster(&mut app, "Whale", Some("whale"));

            send_skill_bar_cast(&mut app, entity);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            assert!(
                app.world().get::<Casting>(entity).is_none(),
                "非人形本体施放人形专属剑技必须被 race gate 拒绝在 resolver 之前；\
             期望无 Casting；实际 Casting 存在"
            );
            assert!(
                syncs.iter().any(|s| s.outcome
                    == crate::schema::combat_hud::CastOutcomeV1::RejectRaceMismatch
                    && s.phase == CastPhaseV1::Idle),
                "race gate 拒绝应推 CastSyncV1{{phase=Idle, outcome=RejectRaceMismatch}}；\
             实际 syncs={syncs:?}"
            );
            let attack_intents = app
                .world()
                .resource::<valence::prelude::Events<crate::combat::events::AttackIntent>>();
            assert!(
                attack_intents.is_empty(),
                "race gate 应在 resolver 之前拦截，resolver 从未运行；\
             期望零 AttackIntent；实际存在事件（说明 resolver 被误放行了）"
            );
            let qi_current = app
                .world()
                .get::<crate::cultivation::components::Cultivation>(entity)
                .expect("Cultivation must still exist")
                .qi_current;
            assert!(
                (qi_current - 42.0).abs() < f64::EPSILON,
                "race gate 拒绝不应扣真元（守恒律，见 CLAUDE.md 真元守恒律）；\
             期望 qi_current=42.0 不变，实际 {qi_current}"
            );
        }

        #[test]
        fn skill_bar_cast_race_gate_passes_for_humanoid_caster_reaches_resolver() {
            // 反向 happy：人形本体（race=human 默认，未插入 RaceRegistry/BodyPlanRegistry
            // → `resolve_body_plan_for_target` 退化到 humanoid 单例）施放同一招
            // sword.cleave 不应被 race gate 拦下——必须真正走到 resolver 并挥出
            // （非零 AttackIntent，且不应出现 RejectRaceMismatch）。与上一测试对照，
            // 证明 race gate 只挡非人形、不误伤人形本体。
            let mut app = App::new();
            register_request_app(&mut app);
            let (entity, mut helper) = setup_sword_cleave_caster(&mut app, "Human", None);

            send_skill_bar_cast(&mut app, entity);
            flush_all_client_packets(&mut app);
            let syncs = collect_cast_syncs(&mut helper);

            assert!(
                !syncs
                    .iter()
                    .any(|s| s.outcome
                        == crate::schema::combat_hud::CastOutcomeV1::RejectRaceMismatch),
                "人形本体不应被 race gate 拒绝；实际 syncs={syncs:?}"
            );
            let attack_intents = app
                .world()
                .resource::<valence::prelude::Events<crate::combat::events::AttackIntent>>();
            assert!(
                !attack_intents.is_empty(),
                "人形本体施放 sword.cleave 应真正走到 resolver 并挥出（发 AttackIntent）；\
             期望非空事件，实际为空——说明 race gate 误挡了人形本体"
            );
        }

        // ── 11. helper 单元：check_player_skill_meridian_gate 直接单元测试 ───────────

        #[test]
        fn skill_config_intent_resource_failures_reply_with_authoritative_snapshot() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.world_mut()
                .resource_mut::<SkillConfigStore>()
                .set_config(
                    "offline:Azure",
                    "zhenmai.sever_chain",
                    crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([
                        ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                        (
                            "backfire_kind".to_string(),
                            serde_json::json!("tainted_yuan"),
                        ),
                    ])),
                );
            app.world_mut().remove_resource::<SkillConfigSchemas>();
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillConfigIntent {
                        v: 1,
                        skill_id: "zhenmai.sever_chain".to_string(),
                        config: std::collections::BTreeMap::from([
                            ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                            ("backfire_kind".to_string(), serde_json::json!("array")),
                        ]),
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
            app.update();
            flush_all_client_packets(&mut app);
            let snapshots = collect_skill_config_snapshots(&mut helper);
            assert_eq!(snapshots.len(), 1);
            assert_eq!(
                snapshots[0]
                    .configs
                    .get("zhenmai.sever_chain")
                    .and_then(|config| config.fields.get("backfire_kind")),
                Some(&serde_json::json!("tainted_yuan"))
            );

            let mut app = App::new();
            register_request_app(&mut app);
            app.world_mut().remove_resource::<SkillConfigStore>();
            let (client_bundle, mut helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillConfigIntent {
                        v: 1,
                        skill_id: "zhenmai.sever_chain".to_string(),
                        config: std::collections::BTreeMap::from([
                            ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                            ("backfire_kind".to_string(), serde_json::json!("array")),
                        ]),
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
            app.update();
            flush_all_client_packets(&mut app);
            let snapshots = collect_skill_config_snapshots(&mut helper);
            assert_eq!(snapshots.len(), 1);
            assert!(snapshots[0].configs.is_empty());
        }

        #[test]
        fn skill_bar_cast_protocol_entity_id_does_not_fallback_to_entity_bits() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let target = app.world_mut().spawn(Position::new([1.0, 0.0, 0.0])).id();
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Induce,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                crate::cultivation::components::MeridianSystem::default(),
                SkillBarBindings::default(),
                QuickSlotBindings::default(),
                empty_inventory(),
            ));
            app.world_mut()
                .get_mut::<SkillBarBindings>(entity)
                .unwrap()
                .set(
                    0,
                    SkillSlot::Skill {
                        skill_id: "burst_meridian.beng_quan".to_string(),
                    },
                );
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 0,
                        target: Some(format!("entity:{}", target.to_bits())),
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });

            app.update();

            assert!(app.world().get::<Casting>(entity).is_none());
            assert_eq!(
                app.world()
                    .resource::<valence::prelude::Events<crate::combat::events::AttackIntent>>()
                    .len(),
                0
            );
        }

        #[test]
        fn skill_bar_cast_empty_item_or_cooldown_does_not_start_cast() {
            for binding in [
                SkillSlot::Empty,
                SkillSlot::Item { instance_id: 7 },
                SkillSlot::Skill {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                },
            ] {
                let mut app = App::new();
                register_request_app(&mut app);
                let mut skill_bar = SkillBarBindings::default();
                assert!(skill_bar.set(0, binding));
                skill_bar.set_cooldown("burst_meridian.beng_quan", 100);
                let entity = spawn_beng_quan_capable_entity(&mut app, skill_bar);
                send_skill_bar_cast(&mut app, entity);
                assert!(
                    app.world().get::<Casting>(entity).is_none(),
                    "空槽、物品绑定或冷却中的技能均不能启动施法"
                );
            }
        }

        #[test]
        fn skill_bar_bind_rejects_unknown_skill() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    SkillBarBindings::default(),
                    QuickSlotBindings::default(),
                    empty_inventory(),
                ))
                .id();
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":{"kind":"skill","skill_id":"unknown.skill"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
            assert!(matches!(bindings.slots[0], SkillSlot::Empty));
        }

        /// bughunt skillbar-rebind-cooldown-reset 返工共用：一个只要不在冷却中就一定能
        /// 把 `burst_meridian.beng_quan` 放出去的实体（Cultivation Induce+100 真元 +
        /// RIGHT_ARM_MERIDIANS opened + Position + 已学会两条技能），镜像
        /// `skill_bar_bind_skill_then_cast_starts_skillbar_cast`（本文件上方）验证过的
        /// 配方——用来让「冷却中不产生 Casting」这类断言不再因为缺前置组件而空洞：
        /// 必须证明"同一实体、不在冷却时确实能拿到 Casting"，才能说清"没拿到 Casting"
        /// 是冷却门挡的，不是别的前置缺失挡的。
        fn spawn_beng_quan_capable_entity(
            app: &mut App,
            skill_bar: SkillBarBindings,
        ) -> valence::prelude::Entity {
            let (client_bundle, _helper) = create_mock_client("Azure");
            // `ClientBundle` 自带 `Position`，与其它组件放进同一个 spawn 元组会因重复
            // component 类型 panic（Bevy bundle 校验）——必须先单独 spawn client_bundle，
            // 再用 `insert` 覆盖/追加其余组件（`insert` 允许覆盖既有 component，`spawn`
            // 的 bundle 元组不允许同类型出现两次）。
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(entity).insert((
                Position::new([0.0, 0.0, 0.0]),
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Induce,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                beng_quan_ready_meridians(),
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                known(&["burst_meridian.beng_quan", "burst_meridian.tie_shan_kao"]),
            ));
            entity
        }

        /// bughunt skillbar-rebind-cooldown-reset — 重新绑定同一槽位为**相同**技能绝不能清零冷
        /// 却，否则玩家可通过「施放高冷却大招 → 立刻把同一招式重新拖回原槽位 → 立刻再次施放」
        /// 无限绕过任何走 `SkillBarBindings` 冷却的招式（含化虚终极技）。实体带全套前置组件
        /// （见 `spawn_beng_quan_capable_entity`），与下方
        /// `skill_bar_bind_same_skill_when_off_cooldown_produces_casting` 正向对照——
        /// 唯一差异是冷却状态，从而排除"没 Casting 是因为缺组件"的空洞断言。
        #[test]
        fn skill_bar_bind_same_skill_does_not_reset_cooldown() {
            let mut app = App::new();
            register_request_app(&mut app);

            let mut skill_bar = SkillBarBindings::default();
            assert!(skill_bar.set(
                1,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                },
            ));
            // 冷却设在 clock.tick(=0) 之后很远，模拟刚放完一记高冷却招式。
            skill_bar.set_cooldown("burst_meridian.beng_quan", 1_000);
            let entity = spawn_beng_quan_capable_entity(&mut app, skill_bar);

            // 玩家把同一招式重新拖回同一槽位——绑定内容完全没变。
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":1,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

            app.update();

            let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
            assert!(matches!(
                &bindings.slots[1],
                SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.beng_quan"
            ));
            assert_eq!(
                bindings.cooldowns.get("burst_meridian.beng_quan").copied(),
                Some(1_000),
                "重绑内容与原绑定相同——冷却必须原样保留，否则重复拖拽同一招式=无限缩短冷却"
            );
            assert!(
                bindings.is_on_cooldown("burst_meridian.beng_quan", 0),
                "冷却状态应保持——重绑同值不是重置冷却的合法手段"
            );

            // 冷却仍未清空 → 再次尝试施放应仍被拒绝（不产生 Casting）。此断言之所以不空洞，
            // 是因为同一套实体前置组件在下方正向对照用例里已证明"不在冷却时确实会产生 Casting"。
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 1,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
            app.update();
            assert!(
                app.world().get::<Casting>(entity).is_none(),
                "冷却未被清零，重复施放不应产生 Casting（否则等于绕过了冷却）"
            );
        }

        /// 正向对照（opus verify 指出的空洞断言修复）：与上一条用例**完全相同**的实体前置
        /// （Cultivation+MeridianSystem+Position+已学会），唯一差异是不在冷却中——必须
        /// 产生 Casting。这条用例存在的意义就是证明上一条的"无 Casting"确实是冷却门挡的，
        /// 而不是随便一个缺前置的实体本来就永远拿不到 Casting。
        #[test]
        fn skill_bar_bind_same_skill_when_off_cooldown_produces_casting() {
            let mut app = App::new();
            register_request_app(&mut app);

            let mut skill_bar = SkillBarBindings::default();
            assert!(skill_bar.set(
                1,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                },
            ));
            // 故意不设冷却（默认 cooldowns map 为空）——同值重绑不应把"从未 cast 过"
            // 变成"被清零过"以外的任何状态，就绪态应保持就绪。
            let entity = spawn_beng_quan_capable_entity(&mut app, skill_bar);

            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":1,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
            app.update();
            assert!(
                !app.world()
                    .get::<SkillBarBindings>(entity)
                    .unwrap()
                    .is_on_cooldown("burst_meridian.beng_quan", 0),
                "同值重绑前本就未 cast 过，不应凭空产生冷却"
            );

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 1,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
            app.update();

            assert!(
                app.world().get::<Casting>(entity).is_some(),
                "正向对照：同一套前置组件、不在冷却中时必须产生 Casting——否则说明上面\
             「无 Casting」的断言是被缺前置组件挡住的空洞断言，而不是被冷却门挡住的"
            );
        }

        /// bughunt skillbar-rebind-cooldown-reset 阻塞问题 A（往返换绑路径）——换绑到
        /// **不同**技能，绝不能清零任何技能的冷却（旧行为"内容变化即清零"正是 A→B→A
        /// 换绑能清空原技能冷却的入口）。冷却按 skill_id 归属后，换绑动作本身完全不再
        /// 触碰任何 cooldowns entry；随后再换绑回原技能，原技能的冷却必须依然健在。
        #[test]
        fn skill_bar_bind_different_skill_never_touches_either_skills_cooldown() {
            let mut app = App::new();
            register_request_app(&mut app);

            let mut skill_bar = SkillBarBindings::default();
            assert!(skill_bar.set(
                1,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                },
            ));
            skill_bar.set_cooldown("burst_meridian.beng_quan", 1_000);
            let entity = spawn_beng_quan_capable_entity(&mut app, skill_bar);

            // 换绑到另一招式——绑定内容确实变了，但这不再是清零任何冷却的手段。
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":1,"binding":{"kind":"skill","skill_id":"burst_meridian.tie_shan_kao"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
            app.update();

            {
                let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
                assert!(matches!(
                    &bindings.slots[1],
                    SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.tie_shan_kao"
                ));
                assert!(
                    bindings.is_on_cooldown("burst_meridian.beng_quan", 0),
                    "换绑到不同技能不得清零 beng_quan 的冷却——这正是 A→B→A 换绑能清空原技能\
                 冷却的攻击面，冷却按 skill_id 归属后必须与槽位内容变化完全解耦"
                );
                assert!(
                    !bindings.is_on_cooldown("burst_meridian.tie_shan_kao", 0),
                    "tie_shan_kao 从未被 cast 过，不应凭空产生冷却"
                );
            }

            // A→B→A 收尾：再换绑回 beng_quan——冷却必须依然健在（往返换绑不是清零手段）。
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":1,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
            app.update();

            let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
            assert!(matches!(
                &bindings.slots[1],
                SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.beng_quan"
            ));
            assert!(
                bindings.is_on_cooldown("burst_meridian.beng_quan", 0),
                "A→B→A 往返换绑收尾后，beng_quan 的冷却必须全程原样保留"
            );
        }

        /// bughunt skillbar-rebind-cooldown-reset 阻塞问题 A 的另一半（清空→重绑路径）：
        /// 客户端「右键清空 / 拖出槽外」发送 `binding: null`（`SkillBarBind{binding: None}`
        /// → `SkillSlot::Empty`），随后把同一招式重新拖回——这条链路在 opus verify 里被
        /// 明确点名为"净效果=两次点击、零代价绕过冷却"的等价路径，必须同样锁死。
        #[test]
        fn skill_bar_bind_clear_then_rebind_same_skill_does_not_reset_cooldown() {
            let mut app = App::new();
            register_request_app(&mut app);

            let mut skill_bar = SkillBarBindings::default();
            assert!(skill_bar.set(
                1,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                },
            ));
            skill_bar.set_cooldown("burst_meridian.beng_quan", 1_000);
            let entity = spawn_beng_quan_capable_entity(&mut app, skill_bar);

            // 右键清空 / 拖出槽外：binding=null → SkillSlot::Empty。
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"skill_bar_bind","v":1,"slot":1,"binding":null}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();
            {
                let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
                assert!(matches!(bindings.slots[1], SkillSlot::Empty));
                assert!(
                    bindings.is_on_cooldown("burst_meridian.beng_quan", 0),
                    "清空槽位不得清零 beng_quan 的冷却——否则「清空→重绑」两次点击即可绕过冷却"
                );
            }

            // 把同一招式重新拖回原槽位。
            app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":1,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
            app.update();

            let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
            assert!(
                bindings.is_on_cooldown("burst_meridian.beng_quan", 0),
                "清空→重绑完整走一遍后，beng_quan 的冷却仍必须原样保留"
            );

            // 冷却仍未清空 → 尝试施放应仍被拒绝。
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot: 1,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
            app.update();
            assert!(
                app.world().get::<Casting>(entity).is_none(),
                "清空→重绑不是绕过冷却的合法手段，冷却仍在时不应产生 Casting"
            );
        }

        // ─────────────────────────────────────────────────────────────────
        // plan-inventory-hint-panel-v1 P0 — 伪皮胸槽境界门控并入 InventoryMoveRejectReason::
        // RealmTooLow：拒绝走 enum（走 emit_inventory_move_rejected 下发结构化 payload），
        // 而不是原独立硬编码分支的 warn-only（连 Result 都不走）。
        // ─────────────────────────────────────────────────────────────────

        /// 从 `MockClientHelper` 收到的包里解出所有 `InventoryMoveRejected` payload
        /// （测试构建走 JSON 序列化，见 `serialize_server_data_payload` 的 `#[cfg(test)]` 分支）。
        fn collect_inventory_move_rejected(
            helper: &mut MockClientHelper,
        ) -> Vec<crate::schema::server_data::InventoryMoveRejectedV1> {
            let mut payloads = Vec::new();
            for frame in helper.collect_received().0 {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    continue;
                };
                if packet.channel.as_str() != crate::network::agent_bridge::SERVER_DATA_CHANNEL {
                    continue;
                }
                if let Some(data) = decode_inventory_move_rejected_payload(packet.data.0 .0) {
                    payloads.push(data);
                }
            }
            payloads
        }

        /// 境界不足时装备伪皮（fake_spirit_hide → SpiderSilk，min_realm=Induce）：
        /// realm=Awaken（< Induce）应被拒绝，走 `InventoryMoveRejectReason::RealmTooLow`
        /// → 下发 `InventoryMoveRejectedV1{reason:"realm_too_low", required_realm:"Induce"}`
        /// → 不修改 inventory（伪皮件仍在原容器格，未落进 chest worn）。
        #[test]
        fn equip_false_skin_realm_too_low_emits_structured_rejection() {
            use crate::combat::tuike::FAKE_SPIRIT_HIDE_ITEM_ID;
            use crate::cultivation::components::{Cultivation, Realm};

            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, mut helper) = create_mock_client("Kiz");
            let item = inventory_test_item(9101, FAKE_SPIRIT_HIDE_ITEM_ID, 1);
            let player = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_item(item),
                    Cultivation {
                        realm: Realm::Awaken,
                        ..Default::default()
                    },
                ))
                .id();

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: player,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::EquipFalseSkin {
                        v: 1,
                        slot: crate::schema::inventory::EquipSlotV1::Chest,
                        item_instance_id: 9101,
                    })
                    .expect("equip_false_skin request should serialize")
                    .into_boxed_slice(),
                });

            app.update();
            flush_all_client_packets(&mut app);

            let rejections = collect_inventory_move_rejected(&mut helper);
            assert_eq!(
                rejections.len(),
                1,
                "境界不足的伪皮装备应下发恰好 1 条 InventoryMoveRejected"
            );
            let rejection = &rejections[0];
            assert_eq!(rejection.reason, "realm_too_low");
            assert_eq!(
                rejection.required_realm.as_deref(),
                Some("Induce"),
                "SpiderSilk 型伪皮 min_realm=Induce，应下发英文 tag 供 client RealmLabel 转中文"
            );
            assert!(rejection.slot.is_none(), "realm_too_low 不带 slot/cap");
            assert!(rejection.cap.is_none());

            // 拒绝后伪皮件仍留在原格，未被写入 chest worn（走 enum 拒绝而非静默放行）。
            let inventory = app
                .world()
                .get::<PlayerInventory>(player)
                .expect("player inventory should still exist");
            assert!(
                inventory
                    .equipped
                    .get(crate::inventory::EQUIP_SLOT_CHEST)
                    .map(|c| c.worn.is_empty())
                    .unwrap_or(true),
                "境界不足时伪皮不应落进 chest worn"
            );
        }

        // ─── plan-scroll-reading-v1 P2 — ScrollReadRequest/ScrollReadClosed 循环动画 e2e ───
        // 覆盖「开卷插 marker + 发 PlayAnim」→「关屏发 StopAnim + 移除 marker」全链路，以及
        // anim_id=None（无动画残卷）/ 未开卷即关屏（no-op）/ 重复关屏（幂等）三个边界。
        use crate::network::scroll_open_emit::ScrollReading;
        use crate::schema::vfx_event::VfxEventPayloadV1;

        fn readable_scroll_template(id: &str, anim_id: Option<&str>) -> ItemTemplate {
            ItemTemplate {
                id: id.to_string(),
                display_name: "《测试残卷》".to_string(),
                category: ItemCategory::Scroll,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 2,
                base_weight: 0.05,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 0.3,
                description: "test".to_string(),
                effect: None,
                cast_duration_ms: 1500,
                cooldown_ms: 1500,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: Some(crate::inventory::ReadableScrollSpec {
                    title: "《测试残卷》".to_string(),
                    body_pages: vec!["第一页".to_string()],
                    anim_id: anim_id.map(|s| s.to_string()),
                }),
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            }
        }

        fn inventory_with_scroll(instance_id: u64, template_id: &str) -> PlayerInventory {
            let mut inv = empty_inventory();
            inv.containers[0].items.push(PlacedItemState {
                row: 0,
                col: 0,
                instance: inventory_test_item(instance_id, template_id, 1),
            });
            inv
        }

        fn scroll_anim_drain_vfx(
            app: &mut App,
        ) -> Vec<crate::network::vfx_event_emit::VfxEventRequest> {
            app.world_mut()
            .resource_mut::<valence::prelude::Events<crate::network::vfx_event_emit::VfxEventRequest>>()
            .drain()
            .collect()
        }

        fn scroll_anim_find_play<'a>(
            reqs: &'a [crate::network::vfx_event_emit::VfxEventRequest],
            anim_id: &str,
        ) -> Option<&'a crate::network::vfx_event_emit::VfxEventRequest> {
            reqs.iter().find(|r| {
            matches!(&r.payload, VfxEventPayloadV1::PlayAnim { anim_id: id, .. } if id == anim_id)
        })
        }

        fn scroll_anim_find_stop<'a>(
            reqs: &'a [crate::network::vfx_event_emit::VfxEventRequest],
            anim_id: &str,
        ) -> Option<&'a crate::network::vfx_event_emit::VfxEventRequest> {
            reqs.iter().find(|r| {
            matches!(&r.payload, VfxEventPayloadV1::StopAnim { anim_id: id, .. } if id == anim_id)
        })
        }

        fn scroll_anim_find_spawn_particle<'a>(
            reqs: &'a [crate::network::vfx_event_emit::VfxEventRequest],
            event_id: &str,
        ) -> Option<&'a crate::network::vfx_event_emit::VfxEventRequest> {
            reqs.iter().find(|r| {
            matches!(&r.payload, VfxEventPayloadV1::SpawnParticle { event_id: id, .. } if id == event_id)
        })
        }

        fn send_scroll_read_request(app: &mut App, entity: Entity, instance_id: u64) {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::ScrollReadRequest {
                        v: 1,
                        instance_id,
                    })
                    .expect("scroll_read_request should serialize")
                    .into_boxed_slice(),
                });
        }

        fn send_scroll_read_closed(app: &mut App, entity: Entity) {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::ScrollReadClosed { v: 1 })
                        .expect("scroll_read_closed should serialize")
                        .into_boxed_slice(),
                });
        }

        // ── happy path: 开卷插 marker + 发 PlayAnim ─────────────────────────
        #[test]
        fn scroll_read_request_inserts_marker_and_emits_play_anim() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "scroll_meridian_primer".to_string(),
                readable_scroll_template("scroll_meridian_primer", Some("bong:read_scroll")),
            )])));

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_scroll(42, "scroll_meridian_primer"),
                ))
                .id();

            send_scroll_read_request(&mut app, entity, 42);
            app.update();

            assert!(
                app.world().get::<ScrollReading>(entity).is_some(),
                "ScrollReadRequest with anim_id must insert ScrollReading marker \
             (真相源 for ScrollReadClosed / death cleanup to find later)"
            );
            let emitted = scroll_anim_drain_vfx(&mut app);
            assert!(
                scroll_anim_find_play(&emitted, "bong:read_scroll").is_some(),
                "ScrollReadRequest must emit PlayAnim{{anim_id==\"bong:read_scroll\"}} \
             when spec has anim_id, got {emitted:?}"
            );
        }

        // ── 边界: spec.anim_id=None → 不插 marker、不发 PlayAnim ──────────────
        #[test]
        fn scroll_read_request_without_anim_id_does_not_insert_marker() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "scroll_no_anim".to_string(),
                readable_scroll_template("scroll_no_anim", None),
            )])));

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((client_bundle, inventory_with_scroll(7, "scroll_no_anim")))
                .id();

            send_scroll_read_request(&mut app, entity, 7);
            app.update();

            assert!(
                app.world().get::<ScrollReading>(entity).is_none(),
                "spec.anim_id=None must not insert a ScrollReading marker — there is no \
             loop animation to stop later"
            );
            let emitted = scroll_anim_drain_vfx(&mut app);
            assert!(
                scroll_anim_find_play(&emitted, "bong:read_scroll").is_none(),
                "no anim_id means no PlayAnim should be emitted, got {emitted:?}"
            );
            assert!(
                scroll_anim_find_spawn_particle(&emitted, "bong:scroll_open_glow").is_some(),
                "展开微光与 anim_id 是否存在无关——即便残卷没有阅读动画，开卷仍应有 \
             SpawnParticle{{event_id==\"bong:scroll_open_glow\"}}，got {emitted:?}"
            );
        }

        // ── happy path: 开卷发展开微光 SpawnParticle（与 anim_id 是否存在无关）──────
        #[test]
        fn scroll_read_request_emits_scroll_open_glow_particle() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "scroll_meridian_primer".to_string(),
                readable_scroll_template("scroll_meridian_primer", Some("bong:read_scroll")),
            )])));

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_scroll(42, "scroll_meridian_primer"),
                ))
                .id();

            send_scroll_read_request(&mut app, entity, 42);
            app.update();

            let emitted = scroll_anim_drain_vfx(&mut app);
            let glow = scroll_anim_find_spawn_particle(&emitted, "bong:scroll_open_glow").expect(
                "ScrollReadRequest must emit SpawnParticle{event_id==\"bong:scroll_open_glow\"}",
            );
            match &glow.payload {
                VfxEventPayloadV1::SpawnParticle {
                    color,
                    count,
                    strength,
                    duration_ticks,
                    ..
                } => {
                    assert_eq!(
                        color.as_deref(),
                        Some("#E8D9A0"),
                        "scroll_open_glow must use the pinned pale-gold color #E8D9A0"
                    );
                    assert_eq!(*count, Some(12), "burst count must be pinned to 12");
                    assert_eq!(*strength, Some(0.85));
                    assert_eq!(*duration_ticks, Some(20));
                }
                other => panic!("expected SpawnParticle, got {other:?}"),
            }
        }

        // ── happy path: 关屏发 StopAnim + 移除 marker ───────────────────────
        #[test]
        fn scroll_read_closed_emits_stop_anim_and_removes_marker() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "scroll_meridian_primer".to_string(),
                readable_scroll_template("scroll_meridian_primer", Some("bong:read_scroll")),
            )])));

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_scroll(42, "scroll_meridian_primer"),
                ))
                .id();

            send_scroll_read_request(&mut app, entity, 42);
            app.update();
            let _ = scroll_anim_drain_vfx(&mut app); // discard open events, focus on close

            send_scroll_read_closed(&mut app, entity);
            app.update();

            let emitted = scroll_anim_drain_vfx(&mut app);
            assert!(
                scroll_anim_find_stop(&emitted, "bong:read_scroll").is_some(),
                "ScrollReadClosed must emit StopAnim{{anim_id==\"bong:read_scroll\"}} \
             when a ScrollReading marker was present, got {emitted:?}"
            );
            assert!(
                app.world().get::<ScrollReading>(entity).is_none(),
                "ScrollReadClosed must remove the ScrollReading marker after stopping the anim"
            );
        }

        // ── 边界: 未开卷即发 ScrollReadClosed → no-op（不 panic，不发 StopAnim）──
        #[test]
        fn scroll_read_closed_without_active_reading_is_noop() {
            let mut app = App::new();
            register_request_app(&mut app);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();

            send_scroll_read_closed(&mut app, entity);
            app.update();

            let emitted = scroll_anim_drain_vfx(&mut app);
            assert!(
                scroll_anim_find_stop(&emitted, "bong:read_scroll").is_none(),
                "ScrollReadClosed with no prior ScrollReadRequest must not emit StopAnim \
             (no ScrollReading marker to act on), got {emitted:?}"
            );
            assert!(
                app.world().get::<ScrollReading>(entity).is_none(),
                "no marker should exist to begin with"
            );
        }

        // ── 重复关屏: 第二次 ScrollReadClosed 不再重复发 StopAnim ────────────
        #[test]
        fn repeated_scroll_read_closed_only_stops_once() {
            let mut app = App::new();
            register_request_app(&mut app);
            app.insert_resource(ItemRegistry::from_map(HashMap::from([(
                "scroll_meridian_primer".to_string(),
                readable_scroll_template("scroll_meridian_primer", Some("bong:read_scroll")),
            )])));

            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_scroll(42, "scroll_meridian_primer"),
                ))
                .id();

            send_scroll_read_request(&mut app, entity, 42);
            app.update();
            let _ = scroll_anim_drain_vfx(&mut app);

            send_scroll_read_closed(&mut app, entity);
            app.update();
            let first_close = scroll_anim_drain_vfx(&mut app);
            assert!(
                scroll_anim_find_stop(&first_close, "bong:read_scroll").is_some(),
                "first ScrollReadClosed must stop the animation, got {first_close:?}"
            );

            send_scroll_read_closed(&mut app, entity);
            app.update();
            let second_close = scroll_anim_drain_vfx(&mut app);
            assert!(
                scroll_anim_find_stop(&second_close, "bong:read_scroll").is_none(),
                "second ScrollReadClosed after marker already removed must be a no-op \
             (idempotent close, not a repeated StopAnim), got {second_close:?}"
            );
        }
    }
    mod freshness_probe_handler_tests {
        use super::*;
        use crate::inventory::{
            ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        };
        use crate::lingtian::events::{
            StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest, StartRenewRequest,
            StartReplenishRequest, StartTillRequest,
        };
        use valence::prelude::{ident, App, EventReader, IntoSystemConfigs, ResMut, Update};
        use valence::testing::create_mock_client;

        #[derive(Default)]
        struct CapturedFreshnessProbes(Vec<FreshnessProbeIntent>);
        impl valence::prelude::Resource for CapturedFreshnessProbes {}

        fn capture_freshness_probes(
            mut events: EventReader<FreshnessProbeIntent>,
            mut captured: ResMut<CapturedFreshnessProbes>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn empty_inventory() -> PlayerInventory {
            PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(0),
                containers: vec![ContainerState {
                    quick_access: false,
                    id: "main_pack".into(),
                    name: "main_pack".into(),
                    rows: 5,
                    cols: 7,
                    items: Vec::new(),

                    owner_instance_id: None,
                }],
                equipped: Default::default(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            }
        }

        fn inventory_with_item(item: ItemInstance) -> PlayerInventory {
            PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(0),
                containers: vec![ContainerState {
                    quick_access: false,
                    id: "main_pack".into(),
                    name: "main_pack".into(),
                    rows: 5,
                    cols: 7,
                    items: vec![PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: item,
                    }],

                    owner_instance_id: None,
                }],
                equipped: Default::default(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            }
        }

        /// helper：为 FreshnessProbe 测试注册最小 app。
        /// 镜像 mineral_probe_request_emits_probe_intent 的 app 构造模式。
        fn setup_freshness_probe_app() -> (App, valence::prelude::Entity) {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(super::load_test_technique_registry());
            app.insert_resource(CapturedFreshnessProbes::default());
            app.insert_resource(CombatClock { tick: 42 });
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<FreshnessProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_systems(
                Update,
                (handle_client_request_payloads, capture_freshness_probes).chain(),
            );
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app.world_mut().spawn(client_bundle).id();
            (app, entity)
        }

        /// FreshnessProbe 请求：inventory 中存在 instance_id → emit FreshnessProbeIntent 正确字段。
        #[test]
        fn freshness_probe_request_emits_probe_intent() {
            let (mut app, entity) = setup_freshness_probe_app();
            let item = ItemInstance {
                instance_id: 7777,
                template_id: "xi_zhi_herb".to_string(),
                display_name: "细枝草".to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.1,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 0.8,
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
            };
            app.world_mut()
                .entity_mut(entity)
                .insert(inventory_with_item(item));
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"freshness_probe","v":1,"instance_id":7777}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();

            let captured = app.world().resource::<CapturedFreshnessProbes>();
            assert_eq!(
                captured.0.len(),
                1,
                "应 emit 1 个 FreshnessProbeIntent，实际 {}",
                captured.0.len()
            );
            assert_eq!(captured.0[0].player, entity, "player entity 应匹配");
            assert_eq!(
                captured.0[0].instance_id, 7777,
                "instance_id 应 round-trip 为 7777"
            );
            assert_eq!(
                captured.0[0].issued_at_tick, 42,
                "issued_at_tick 应等于 CombatClock.tick=42"
            );
        }

        /// FreshnessProbe 请求：instance_id 不在 inventory → 不 emit，不 panic。
        #[test]
        fn freshness_probe_request_not_found_does_not_emit() {
            let (mut app, entity) = setup_freshness_probe_app();
            // inventory 为空，instance_id=9999 不存在
            app.world_mut().entity_mut(entity).insert(empty_inventory());
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"freshness_probe","v":1,"instance_id":9999}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();

            let captured = app.world().resource::<CapturedFreshnessProbes>();
            assert!(
                captured.0.is_empty(),
                "instance_id 不存在时不应 emit FreshnessProbeIntent"
            );
        }

        /// FreshnessProbe 请求：instance_id 在非首容器中也能找到并 emit（多容器覆盖）。
        #[test]
        fn freshness_probe_request_finds_item_in_secondary_container() {
            let (mut app, entity) = setup_freshness_probe_app();
            // 构造含两个容器的 inventory，物品在第二个容器
            let item = ItemInstance {
                instance_id: 1234,
                template_id: "xi_zhi_herb".to_string(),
                display_name: "细枝草".to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.1,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
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
            };
            let inv = PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(0),
                containers: vec![
                    // 第一容器（空）
                    ContainerState {
                        quick_access: false,
                        id: "main_pack".into(),
                        name: "main_pack".into(),
                        rows: 5,
                        cols: 7,
                        items: Vec::new(),

                        owner_instance_id: None,
                    },
                    // 第二容器持有目标物品
                    ContainerState {
                        quick_access: false,
                        id: "side_pack".into(),
                        name: "side_pack".into(),
                        rows: 3,
                        cols: 4,
                        items: vec![PlacedItemState {
                            row: 1,
                            col: 2,
                            instance: item,
                        }],

                        owner_instance_id: None,
                    },
                ],
                equipped: Default::default(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            };
            app.world_mut().entity_mut(entity).insert(inv);
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"freshness_probe","v":1,"instance_id":1234}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();

            let captured = app.world().resource::<CapturedFreshnessProbes>();
            assert_eq!(
                captured.0.len(),
                1,
                "第二容器中的物品也应能 emit FreshnessProbeIntent"
            );
            assert_eq!(
                captured.0[0].instance_id, 1234,
                "instance_id 应匹配第二容器物品"
            );
        }

        /// FreshnessProbe gate 扩展：instance_id 在 hotbar 中也应 emit（原 bug：只扫 containers）。
        #[test]
        fn freshness_probe_request_finds_item_in_hotbar() {
            let (mut app, entity) = setup_freshness_probe_app();
            let item = ItemInstance {
                instance_id: 5555,
                template_id: "zhi_xiang_cao".to_string(),
                display_name: "止香草".to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.05,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 0.6,
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
            };
            // 放进 hotbar slot 1（容器为空）
            let mut inv = empty_inventory();
            inv.hotbar[1] = Some(item);
            app.world_mut().entity_mut(entity).insert(inv);

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"freshness_probe","v":1,"instance_id":5555}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();

            let captured = app.world().resource::<CapturedFreshnessProbes>();
            assert_eq!(
            captured.0.len(),
            1,
            "hotbar 中的物品也应能通过 gate 并 emit FreshnessProbeIntent（修复前只扫 containers 导致误拒）"
        );
            assert_eq!(
                captured.0[0].instance_id, 5555,
                "instance_id 应匹配 hotbar 物品"
            );
        }

        /// FreshnessProbe gate 扩展：instance_id 在 equipped 中也应 emit。
        #[test]
        fn freshness_probe_request_finds_item_in_equipped() {
            let (mut app, entity) = setup_freshness_probe_app();
            let item = ItemInstance {
                instance_id: 6666,
                template_id: "spirit_robe".to_string(),
                display_name: "灵袍".to_string(),
                grid_w: 2,
                grid_h: 3,
                weight: 1.5,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 0.9,
                durability: 0.8,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            };
            // 放进 equipped（模拟穿戴槽），容器与 hotbar 均为空
            let mut inv = empty_inventory();
            inv.equipped.insert(
                "chest".to_string(),
                crate::inventory::SlotContents::worn_single(item),
            );
            app.world_mut().entity_mut(entity).insert(inv);

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"freshness_probe","v":1,"instance_id":6666}"#
                        .to_vec()
                        .into_boxed_slice(),
                });
            app.update();

            let captured = app.world().resource::<CapturedFreshnessProbes>();
            assert_eq!(
            captured.0.len(),
            1,
            "equipped 中的物品也应能通过 gate 并 emit FreshnessProbeIntent（修复前只扫 containers 导致误拒）"
        );
            assert_eq!(
                captured.0[0].instance_id, 6666,
                "instance_id 应匹配 equipped 物品"
            );
        }

        // ── plan-shield-block-v1 P1 e2e — 举盾 / 放盾全链路 ─────────────────────
        // 验证：JSON payload {"type":"raise_shield","v":1} → handle_client_request_payloads
        // 解析 → 投递 RaiseShieldIntent（client entity 匹配）；
        // 以及 lower_shield payload → LowerShieldIntent 投递。
        // 这是「客户端发 CustomPayload → server dispatch intent」的完整链路断言。

        #[derive(Default)]
        struct CapturedRaiseShieldIntents(Vec<crate::combat::shield_block::RaiseShieldIntent>);
        impl valence::prelude::Resource for CapturedRaiseShieldIntents {}

        #[derive(Default)]
        struct CapturedLowerShieldIntents(Vec<crate::combat::shield_block::LowerShieldIntent>);
        impl valence::prelude::Resource for CapturedLowerShieldIntents {}

        fn capture_raise_shield_intents(
            mut events: EventReader<crate::combat::shield_block::RaiseShieldIntent>,
            mut captured: ResMut<CapturedRaiseShieldIntents>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn capture_lower_shield_intents(
            mut events: EventReader<crate::combat::shield_block::LowerShieldIntent>,
            mut captured: ResMut<CapturedLowerShieldIntents>,
        ) {
            captured.0.extend(events.read().cloned());
        }

        fn setup_shield_e2e_app() -> (App, valence::prelude::Entity) {
            let mut app = App::new();
            app.init_resource::<ClientRequestBudget>();
            app.insert_resource(super::load_test_technique_registry());
            app.insert_resource(CapturedRaiseShieldIntents::default());
            app.insert_resource(CapturedLowerShieldIntents::default());
            app.insert_resource(CombatClock::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(AlchemyMockState::default());
            app.insert_resource(DroppedLootRegistry::default());
            // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
            app.add_event::<crate::inventory::RemainsLootIntent>();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            app.insert_resource(ItemRegistry::default());
            app.insert_resource(RecipeRegistry::default());
            app.add_event::<CustomPayloadEvent>();
            app.add_event::<BreakthroughRequest>();
            app.add_event::<ForgeRequest>();
            app.add_event::<InsightChosen>();
            app.add_event::<DefenseIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<PlaceFurnaceRequest>();
            app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
            app.add_event::<StartTillRequest>();
            app.add_event::<StartRenewRequest>();
            app.add_event::<StartPlantingRequest>();
            app.add_event::<StartHarvestRequest>();
            app.add_event::<StartReplenishRequest>();
            app.add_event::<StartDrainQiRequest>();
            app.add_event::<StartExtractRequestEvent>();
            app.add_event::<CancelExtractRequestEvent>();
            app.add_event::<MineralProbeIntent>();
            app.add_event::<FreshnessProbeIntent>();
            app.add_event::<SkillXpGain>();
            app.add_event::<SkillScrollUsed>();
            app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
            app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
            app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
            app.add_systems(
                Update,
                (
                    handle_client_request_payloads,
                    capture_raise_shield_intents,
                    capture_lower_shield_intents,
                )
                    .chain(),
            );
            let (client_bundle, _helper) = create_mock_client("Shield");
            let entity = app.world_mut().spawn(client_bundle).id();
            (app, entity)
        }

        /// e2e：JSON {"type":"raise_shield","v":1} payload → RaiseShieldIntent(player=entity) 投递。
        /// 验证 client_request_handler 正确解析 raise_shield 并路由到 intent event。
        #[test]
        fn raise_shield_payload_dispatches_raise_shield_intent() {
            let (mut app, entity) = setup_shield_e2e_app();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"raise_shield","v":1}"#.to_vec().into_boxed_slice(),
                });
            app.update();

            let captured = app.world().resource::<CapturedRaiseShieldIntents>();
            assert_eq!(
                captured.0.len(),
                1,
                "raise_shield payload 应 dispatch 恰好 1 个 RaiseShieldIntent，实际 {}",
                captured.0.len()
            );
            assert_eq!(
                captured.0[0].player, entity,
                "RaiseShieldIntent.player 应等于发送 payload 的 client entity"
            );
        }

        /// e2e：JSON {"type":"lower_shield","v":1} payload → LowerShieldIntent(player=entity) 投递。
        /// 验证松开右键边沿的 lower_shield 路由正确。
        #[test]
        fn lower_shield_payload_dispatches_lower_shield_intent() {
            let (mut app, entity) = setup_shield_e2e_app();
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"lower_shield","v":1}"#.to_vec().into_boxed_slice(),
                });
            app.update();

            let captured = app.world().resource::<CapturedLowerShieldIntents>();
            assert_eq!(
                captured.0.len(),
                1,
                "lower_shield payload 应 dispatch 恰好 1 个 LowerShieldIntent，实际 {}",
                captured.0.len()
            );
            assert_eq!(
                captured.0[0].player, entity,
                "LowerShieldIntent.player 应等于发送 payload 的 client entity"
            );
        }

        /// e2e：raise 后接 lower → 两个 intent 均投递，顺序正确。
        #[test]
        fn raise_then_lower_shield_payload_dispatches_both_intents_in_order() {
            let (mut app, entity) = setup_shield_e2e_app();

            // Raise
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"raise_shield","v":1}"#.to_vec().into_boxed_slice(),
                });
            app.update();

            // Lower
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"{"type":"lower_shield","v":1}"#.to_vec().into_boxed_slice(),
                });
            app.update();

            let raised = app.world().resource::<CapturedRaiseShieldIntents>();
            let lowered = app.world().resource::<CapturedLowerShieldIntents>();
            assert_eq!(
                raised.0.len(),
                1,
                "raise 后应有 1 个 RaiseShieldIntent，实际 {}",
                raised.0.len()
            );
            assert_eq!(
                lowered.0.len(),
                1,
                "lower 后应有 1 个 LowerShieldIntent，实际 {}",
                lowered.0.len()
            );
        }

        /// plan-shield-block-v1 P1 CR#4 — 同 tick 内同时发送 raise + lower 两个 payload，
        /// 断言两个 intent 在同一 update() 内均被 dispatch（区别于 raise_then_lower 使用两次 update）。
        #[test]
        fn raise_and_lower_same_tick_dispatches_both_intents() {
            let (mut app, entity) = setup_shield_e2e_app();

            // 在同一 update 前发送 raise + lower 两个 CustomPayloadEvent
            let mut events = app
                .world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>();
            events.send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"raise_shield","v":1}"#.to_vec().into_boxed_slice(),
            });
            events.send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"lower_shield","v":1}"#.to_vec().into_boxed_slice(),
            });

            // 单次 update —— 两个 payload 在同一 tick 内被 handle_client_request_payloads 处理
            app.update();

            let raised = app.world().resource::<CapturedRaiseShieldIntents>();
            let lowered = app.world().resource::<CapturedLowerShieldIntents>();
            assert_eq!(
                raised.0.len(),
                1,
                "同 tick raise+lower：应有 1 个 RaiseShieldIntent，实际 {}; \
             期望 handle_client_request_payloads 在单次 update 内 dispatch raise+lower 两个 intent",
                raised.0.len()
            );
            assert_eq!(
                lowered.0.len(),
                1,
                "同 tick raise+lower：应有 1 个 LowerShieldIntent，实际 {}; \
             期望 handle_client_request_payloads 在单次 update 内 dispatch raise+lower 两个 intent",
                lowered.0.len()
            );
            assert_eq!(
                raised.0[0].player, entity,
                "RaiseShieldIntent.player 应等于发送 payload 的 client entity，同 tick 场景"
            );
            assert_eq!(
                lowered.0[0].player, entity,
                "LowerShieldIntent.player 应等于发送 payload 的 client entity，同 tick 场景"
            );
        }

        /// plan-shield-block-v1 P1 CR#4 — 协议错误分支：v!=1 的 raise_shield payload 被版本校验拒绝，不 dispatch intent。
        #[test]
        fn raise_shield_bad_version_is_not_dispatched() {
            let (mut app, entity) = setup_shield_e2e_app();

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    // v:2 应被 SUPPORTED_VERSION 校验拒绝（warn + continue，不 dispatch）
                    data: br#"{"type":"raise_shield","v":2}"#.to_vec().into_boxed_slice(),
                });
            app.update();

            let captured = app.world().resource::<CapturedRaiseShieldIntents>();
            assert_eq!(
                captured.0.len(),
                0,
                "raise_shield with v:2 must not dispatch RaiseShieldIntent \
             because SUPPORTED_VERSION check rejects unsupported protocol versions; \
             actual intent count={}",
                captured.0.len()
            );
        }

        /// plan-shield-block-v1 P1 CR#4 — 协议错误分支：malformed JSON 不 dispatch 任何 intent。
        #[test]
        fn raise_shield_malformed_json_is_not_dispatched() {
            let (mut app, entity) = setup_shield_e2e_app();

            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: br#"not valid json"#.to_vec().into_boxed_slice(),
                });
            app.update();

            let captured = app.world().resource::<CapturedRaiseShieldIntents>();
            assert_eq!(
                captured.0.len(),
                0,
                "malformed JSON payload must not dispatch any RaiseShieldIntent; \
             actual intent count={}",
                captured.0.len()
            );
        }
    }
}
