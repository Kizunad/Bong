use std::collections::HashMap;

use valence::prelude::{bevy_ecs, App, Entity, Events, Update};

use crate::combat::components::{DerivedAttrs, SkillBarBindings, TICKS_PER_SECOND};
use crate::combat::CombatClock;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{ColorKind, ContamSource, Contamination, Cultivation, Realm};
use crate::cultivation::meridian::severed::SkillMeridianDependencies;
use crate::cultivation::skill_registry::{CastRejectReason, CastResult};
use crate::inventory::{
    InventoryRevision, ItemInstance, ItemRarity, PlayerInventory, EQUIP_SLOT_FALSE_SKIN,
};

use super::events::{
    ContamTransferredEvent, DonFalseSkinEvent, FalseSkinDecayedToAshEvent, FalseSkinSheddedEvent,
    PermanentTaintAbsorbedEvent, TuikeSkillId, TuikeSkillVisual,
};
use super::physics::{
    can_absorb_permanent_taint, can_wear_tier, maintenance_qi_per_sec, max_layers_for_realm,
    max_tier_for_realm, naked_defense_damage_multiplier, residue_decay_ticks_for_tier,
    shed_start_cost, shed_to_carrier, transfer_cooldown_ticks, transfer_limit_percent,
    transfer_qi_per_contam_percent, transfer_taint_to_outer_skin, ACTIVE_SHED_COOLDOWN_TICKS,
    RESIDUE_DECAY_MAX_TICKS, RESIDUE_DECAY_MIN_TICKS, TRANSFER_PERMANENT_COOLDOWN_TICKS,
    TRANSFER_STANDARD_COOLDOWN_TICKS,
};
use super::skills::{
    cast_don, cast_shed, cast_transfer_taint, declare_meridian_dependencies, shed_outer_layer,
};
use super::state::{
    false_skin_tier_for_item, FalseSkinLayer, FalseSkinResidue, FalseSkinTier, PermanentQiMaxDecay,
    StackedFalseSkins, WornFalseSkin, FALSE_SKIN_ANCIENT_ITEM_ID, FALSE_SKIN_FAN_ITEM_ID,
    FALSE_SKIN_HEAVY_ITEM_ID, FALSE_SKIN_LIGHT_ITEM_ID, FALSE_SKIN_MID_ITEM_ID,
};
use super::tick::false_skin_maintenance_tick;

fn cultivation(realm: Realm, qi_current: f64, qi_max: f64) -> Cultivation {
    Cultivation {
        realm,
        qi_current,
        qi_max,
        ..Cultivation::default()
    }
}

fn skin_item(instance_id: u64, template_id: &str, spirit_quality: f64) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 1.0,
        rarity: ItemRarity::Rare,
        description: String::new(),
        stack_count: 1,
        spirit_quality,
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

fn inventory_with_skin(template_id: &str, spirit_quality: f64) -> PlayerInventory {
    let mut equipped = HashMap::new();
    equipped.insert(
        EQUIP_SLOT_FALSE_SKIN.to_string(),
        skin_item(1001, template_id, spirit_quality),
    );
    PlayerInventory {
        revision: InventoryRevision(1),
        containers: Vec::new(),
        equipped,
        hotbar: std::array::from_fn(|_| None),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

fn add_tuike_events(world: &mut bevy_ecs::world::World) {
    world.insert_resource(Events::<DonFalseSkinEvent>::default());
    world.insert_resource(Events::<FalseSkinSheddedEvent>::default());
    world.insert_resource(Events::<ContamTransferredEvent>::default());
    world.insert_resource(Events::<FalseSkinDecayedToAshEvent>::default());
    world.insert_resource(Events::<PermanentTaintAbsorbedEvent>::default());
    world.insert_resource(Events::<crate::qi_physics::QiTransfer>::default());
    world.insert_resource(Events::<crate::skill::events::SkillXpGain>::default());
}

fn world_with_player(
    realm: Realm,
    qi_current: f64,
    template_id: &str,
) -> (bevy_ecs::world::World, Entity) {
    let mut world = bevy_ecs::world::World::default();
    world.insert_resource(CombatClock { tick: 100 });
    add_tuike_events(&mut world);
    let entity = world
        .spawn((
            cultivation(realm, qi_current, qi_current.max(1.0)),
            inventory_with_skin(template_id, 1.0),
            SkillBarBindings::default(),
            DerivedAttrs::default(),
            PracticeLog::default(),
        ))
        .id();
    (world, entity)
}

fn assert_started(result: CastResult) -> u64 {
    match result {
        CastResult::Started { cooldown_ticks, .. } => cooldown_ticks,
        other => panic!("expected Started, got {other:?}"),
    }
}

fn assert_rejected(result: CastResult, expected: CastRejectReason) {
    match result {
        CastResult::Rejected { reason } => assert_eq!(reason, expected),
        other => panic!("expected Rejected({expected:?}), got {other:?}"),
    }
}

fn layer(tier: FalseSkinTier, quality: f64) -> FalseSkinLayer {
    FalseSkinLayer::new(7, tier, quality, 11)
}

fn stack_with(tier: FalseSkinTier, quality: f64) -> StackedFalseSkins {
    StackedFalseSkins::with_layer(layer(tier, quality))
}

fn contam(amount: f64) -> Contamination {
    Contamination {
        entries: vec![ContamSource {
            amount,
            color: ColorKind::Insidious,
            meridian_id: None,
            attacker_id: None,
            introduced_at: 1,
        }],
    }
}

#[test]
fn skill_id_don_wire_id_is_stable() {
    assert_eq!(TuikeSkillId::Don.as_str(), "tuike.don");
}

#[test]
fn skill_id_shed_wire_id_is_stable() {
    assert_eq!(TuikeSkillId::Shed.as_str(), "tuike.shed");
}

#[test]
fn skill_id_transfer_wire_id_is_stable() {
    assert_eq!(TuikeSkillId::TransferTaint.as_str(), "tuike.transfer_taint");
}

#[test]
fn meridian_dependencies_explicitly_declare_tuike_as_empty() {
    let mut dependencies = SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut dependencies);

    for skill_id in ["tuike.don", "tuike.shed", "tuike.transfer_taint"] {
        assert!(dependencies.is_declared(skill_id));
        assert!(
            dependencies.lookup(skill_id).is_empty(),
            "tuike is a wallet-based style and must intentionally declare no meridian deps"
        );
    }
}

#[test]
fn visual_don_uses_don_assets() {
    let visual = TuikeSkillVisual::for_skill(TuikeSkillId::Don, false);
    assert_eq!(visual.animation_id, "bong:tuike_don_skin");
    assert_eq!(visual.particle_id, "bong:false_skin_don_dust");
}

#[test]
fn visual_shed_uses_burst_assets() {
    let visual = TuikeSkillVisual::for_skill(TuikeSkillId::Shed, false);
    assert_eq!(visual.animation_id, "bong:tuike_shed_burst");
    assert_eq!(visual.sound_recipe_id, "shed_skin_burst");
}

#[test]
fn visual_shed_ancient_uses_glow_particle() {
    let visual = TuikeSkillVisual::for_skill(TuikeSkillId::Shed, true);
    assert_eq!(visual.particle_id, "bong:ancient_skin_glow");
}

#[test]
fn visual_transfer_uses_transfer_animation() {
    let visual = TuikeSkillVisual::for_skill(TuikeSkillId::TransferTaint, false);
    assert_eq!(visual.animation_id, "bong:tuike_taint_transfer");
}

#[test]
fn visual_transfer_ancient_uses_glow_particle() {
    let visual = TuikeSkillVisual::for_skill(TuikeSkillId::TransferTaint, true);
    assert_eq!(visual.particle_id, "bong:ancient_skin_glow");
}

#[test]
fn fan_item_maps_to_fan_tier() {
    assert_eq!(
        false_skin_tier_for_item(FALSE_SKIN_FAN_ITEM_ID),
        Some(FalseSkinTier::Fan)
    );
}

#[test]
fn light_item_maps_to_light_tier() {
    assert_eq!(
        false_skin_tier_for_item(FALSE_SKIN_LIGHT_ITEM_ID),
        Some(FalseSkinTier::Light)
    );
}

#[test]
fn mid_item_maps_to_mid_tier() {
    assert_eq!(
        false_skin_tier_for_item(FALSE_SKIN_MID_ITEM_ID),
        Some(FalseSkinTier::Mid)
    );
}

#[test]
fn heavy_item_maps_to_heavy_tier() {
    assert_eq!(
        false_skin_tier_for_item(FALSE_SKIN_HEAVY_ITEM_ID),
        Some(FalseSkinTier::Heavy)
    );
}

#[test]
fn ancient_item_maps_to_ancient_tier() {
    assert_eq!(
        false_skin_tier_for_item(FALSE_SKIN_ANCIENT_ITEM_ID),
        Some(FalseSkinTier::Ancient)
    );
}

#[test]
fn unknown_item_is_not_false_skin() {
    assert_eq!(false_skin_tier_for_item("stone"), None);
}

#[test]
fn fan_material_factor_matches_plan() {
    assert_eq!(FalseSkinTier::Fan.material_factor(), 0.2);
}

#[test]
fn light_material_factor_matches_plan() {
    assert_eq!(FalseSkinTier::Light.material_factor(), 0.5);
}

#[test]
fn mid_material_factor_matches_plan() {
    assert_eq!(FalseSkinTier::Mid.material_factor(), 1.5);
}

#[test]
fn heavy_material_factor_matches_plan() {
    assert_eq!(FalseSkinTier::Heavy.material_factor(), 4.0);
}

#[test]
fn ancient_material_factor_matches_plan() {
    assert_eq!(FalseSkinTier::Ancient.material_factor(), 10.0);
}

#[test]
fn fan_maintenance_cost_is_lowest() {
    assert_eq!(FalseSkinTier::Fan.maintain_qi_per_sec(), 0.1);
}

#[test]
fn ancient_maintenance_cost_is_highest() {
    assert_eq!(FalseSkinTier::Ancient.maintain_qi_per_sec(), 1.0);
}

#[test]
fn ancient_residue_returns_relic_shard() {
    assert_eq!(
        FalseSkinTier::Ancient.residue_output_item_id(),
        "ancient_false_skin_shard"
    );
}

#[test]
fn non_ancient_residue_returns_ash() {
    assert_eq!(
        FalseSkinTier::Mid.residue_output_item_id(),
        "tuike_false_skin_ash"
    );
}

#[test]
fn shed_cost_zero_when_qi_zero() {
    assert_eq!(shed_start_cost(0.0), 0.0);
}

#[test]
fn shed_cost_uses_beta_and_current_qi() {
    assert_eq!(
        super::physics::TUIKE_BETA,
        crate::qi_physics::constants::TUIKE_BETA
    );
    assert!((shed_start_cost(80.0) - 4.8).abs() < 1e-9);
}

#[test]
fn shed_cost_clamps_negative_qi() {
    assert_eq!(shed_start_cost(-10.0), 0.0);
}

#[test]
fn shed_cost_handles_non_finite_qi() {
    assert_eq!(shed_start_cost(f64::NAN), 0.0);
}

#[test]
fn awaken_has_one_layer() {
    assert_eq!(max_layers_for_realm(Realm::Awaken), 1);
}

#[test]
fn induce_has_one_layer() {
    assert_eq!(max_layers_for_realm(Realm::Induce), 1);
}

#[test]
fn condense_has_one_layer() {
    assert_eq!(max_layers_for_realm(Realm::Condense), 1);
}

#[test]
fn solidify_has_two_layers() {
    assert_eq!(max_layers_for_realm(Realm::Solidify), 2);
}

#[test]
fn spirit_has_two_layers() {
    assert_eq!(max_layers_for_realm(Realm::Spirit), 2);
}

#[test]
fn void_has_three_layers() {
    assert_eq!(max_layers_for_realm(Realm::Void), 3);
}

#[test]
fn awaken_max_tier_is_fan() {
    assert_eq!(max_tier_for_realm(Realm::Awaken), FalseSkinTier::Fan);
}

#[test]
fn condense_max_tier_is_light() {
    assert_eq!(max_tier_for_realm(Realm::Condense), FalseSkinTier::Light);
}

#[test]
fn solidify_max_tier_is_mid() {
    assert_eq!(max_tier_for_realm(Realm::Solidify), FalseSkinTier::Mid);
}

#[test]
fn spirit_max_tier_is_heavy() {
    assert_eq!(max_tier_for_realm(Realm::Spirit), FalseSkinTier::Heavy);
}

#[test]
fn void_max_tier_is_ancient() {
    assert_eq!(max_tier_for_realm(Realm::Void), FalseSkinTier::Ancient);
}

#[test]
fn awaken_can_wear_fan() {
    assert!(can_wear_tier(Realm::Awaken, FalseSkinTier::Fan));
}

#[test]
fn awaken_cannot_wear_light() {
    assert!(!can_wear_tier(Realm::Awaken, FalseSkinTier::Light));
}

#[test]
fn condense_can_wear_light() {
    assert!(can_wear_tier(Realm::Condense, FalseSkinTier::Light));
}

#[test]
fn condense_cannot_wear_mid() {
    assert!(!can_wear_tier(Realm::Condense, FalseSkinTier::Mid));
}

#[test]
fn solidify_can_wear_mid() {
    assert!(can_wear_tier(Realm::Solidify, FalseSkinTier::Mid));
}

#[test]
fn solidify_cannot_wear_heavy() {
    assert!(!can_wear_tier(Realm::Solidify, FalseSkinTier::Heavy));
}

#[test]
fn spirit_can_wear_heavy() {
    assert!(can_wear_tier(Realm::Spirit, FalseSkinTier::Heavy));
}

#[test]
fn spirit_cannot_wear_ancient() {
    assert!(!can_wear_tier(Realm::Spirit, FalseSkinTier::Ancient));
}

#[test]
fn void_can_wear_ancient() {
    assert!(can_wear_tier(Realm::Void, FalseSkinTier::Ancient));
}

#[test]
fn transfer_rate_awaken_is_fifteen() {
    assert_eq!(transfer_qi_per_contam_percent(Realm::Awaken), 15.0);
}

#[test]
fn transfer_rate_induce_is_thirteen() {
    assert_eq!(transfer_qi_per_contam_percent(Realm::Induce), 13.0);
}

#[test]
fn transfer_rate_condense_is_eleven() {
    assert_eq!(transfer_qi_per_contam_percent(Realm::Condense), 11.0);
}

#[test]
fn transfer_rate_solidify_is_ten() {
    assert_eq!(transfer_qi_per_contam_percent(Realm::Solidify), 10.0);
}

#[test]
fn transfer_rate_spirit_is_nine() {
    assert_eq!(transfer_qi_per_contam_percent(Realm::Spirit), 9.0);
}

#[test]
fn transfer_rate_void_is_seven() {
    assert_eq!(transfer_qi_per_contam_percent(Realm::Void), 7.0);
}

#[test]
fn transfer_limit_awaken_is_one() {
    assert_eq!(transfer_limit_percent(Realm::Awaken), 1.0);
}

#[test]
fn transfer_limit_induce_is_two() {
    assert_eq!(transfer_limit_percent(Realm::Induce), 2.0);
}

#[test]
fn transfer_limit_condense_is_three() {
    assert_eq!(transfer_limit_percent(Realm::Condense), 3.0);
}

#[test]
fn transfer_limit_solidify_is_five() {
    assert_eq!(transfer_limit_percent(Realm::Solidify), 5.0);
}

#[test]
fn transfer_limit_spirit_is_eight() {
    assert_eq!(transfer_limit_percent(Realm::Spirit), 8.0);
}

#[test]
fn transfer_limit_void_is_fifteen() {
    assert_eq!(transfer_limit_percent(Realm::Void), 15.0);
}

#[test]
fn only_void_ancient_absorbs_permanent_taint() {
    assert!(can_absorb_permanent_taint(
        Realm::Void,
        FalseSkinTier::Ancient
    ));
}

#[test]
fn void_heavy_does_not_absorb_permanent_taint() {
    assert!(!can_absorb_permanent_taint(
        Realm::Void,
        FalseSkinTier::Heavy
    ));
}

#[test]
fn spirit_ancient_does_not_absorb_permanent_taint() {
    assert!(!can_absorb_permanent_taint(
        Realm::Spirit,
        FalseSkinTier::Ancient
    ));
}

#[test]
fn residue_fan_uses_min_decay() {
    assert_eq!(
        residue_decay_ticks_for_tier(FalseSkinTier::Fan),
        RESIDUE_DECAY_MIN_TICKS
    );
}

#[test]
fn residue_light_uses_min_decay() {
    assert_eq!(
        residue_decay_ticks_for_tier(FalseSkinTier::Light),
        RESIDUE_DECAY_MIN_TICKS
    );
}

#[test]
fn residue_mid_uses_midpoint_decay() {
    assert_eq!(
        residue_decay_ticks_for_tier(FalseSkinTier::Mid),
        (RESIDUE_DECAY_MIN_TICKS + RESIDUE_DECAY_MAX_TICKS) / 2
    );
}

#[test]
fn residue_heavy_uses_midpoint_decay() {
    assert_eq!(
        residue_decay_ticks_for_tier(FalseSkinTier::Heavy),
        (RESIDUE_DECAY_MIN_TICKS + RESIDUE_DECAY_MAX_TICKS) / 2
    );
}

#[test]
fn residue_ancient_uses_max_decay() {
    assert_eq!(
        residue_decay_ticks_for_tier(FalseSkinTier::Ancient),
        RESIDUE_DECAY_MAX_TICKS
    );
}

#[test]
fn maintenance_sums_layers() {
    let mut stack = stack_with(FalseSkinTier::Fan, 1.0);
    stack.push_outer(layer(FalseSkinTier::Mid, 1.0), 2);
    assert!((maintenance_qi_per_sec(&stack, None) - 0.4).abs() < 1e-9);
}

#[test]
fn maintenance_discount_applies_for_solid_practice() {
    let stack = stack_with(FalseSkinTier::Heavy, 1.0);
    let mut log = PracticeLog::default();
    log.add(ColorKind::Solid, 3.0);
    log.add(ColorKind::Sharp, 1.0);
    assert!((maintenance_qi_per_sec(&stack, Some(&log)) - 0.25).abs() < 1e-9);
}

#[test]
fn maintenance_discount_does_not_apply_below_threshold() {
    let stack = stack_with(FalseSkinTier::Heavy, 1.0);
    let mut log = PracticeLog::default();
    log.add(ColorKind::Solid, 1.0);
    log.add(ColorKind::Sharp, 3.0);
    assert!((maintenance_qi_per_sec(&stack, Some(&log)) - 0.5).abs() < 1e-9);
}

#[test]
fn maintenance_sheds_outer_layer_when_qi_cannot_pay_upkeep() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 120 });
    app.add_event::<FalseSkinSheddedEvent>();
    app.add_systems(Update, false_skin_maintenance_tick);
    let entity = app
        .world_mut()
        .spawn((
            cultivation(Realm::Awaken, 0.05, 10.0),
            StackedFalseSkins::with_layer(FalseSkinLayer::new(1001, FalseSkinTier::Fan, 1.0, 0)),
            WornFalseSkin {
                instance_id: 1001,
                tier: FalseSkinTier::Fan,
                spirit_quality: 1.0,
                contam_load: 0.0,
                permanent_taint_load: 0.0,
            },
            inventory_with_skin(FALSE_SKIN_FAN_ITEM_ID, 1.0),
            DerivedAttrs {
                tuike_layers: 1,
                ..Default::default()
            },
            PracticeLog::default(),
        ))
        .id();

    app.update();

    let stack = app.world().get::<StackedFalseSkins>(entity).unwrap();
    assert!(stack.is_empty());
    assert_eq!(stack.naked_until_tick, 120 + 5 * TICKS_PER_SECOND);
    assert!(app.world().get::<WornFalseSkin>(entity).is_none());
    assert_eq!(
        app.world()
            .get::<DerivedAttrs>(entity)
            .unwrap()
            .tuike_layers,
        0
    );
    assert!(!app
        .world()
        .get::<PlayerInventory>(entity)
        .unwrap()
        .equipped
        .contains_key(EQUIP_SLOT_FALSE_SKIN));
    let mut residue_query = app.world_mut().query::<&FalseSkinResidue>();
    assert_eq!(residue_query.iter(app.world()).count(), 1);
}

#[test]
fn naked_defense_window_amplifies_incoming_damage_only_while_empty() {
    let stack = StackedFalseSkins {
        naked_until_tick: 200,
        ..Default::default()
    };
    assert_eq!(naked_defense_damage_multiplier(Some(&stack), 199), 1.5);
    assert_eq!(naked_defense_damage_multiplier(Some(&stack), 200), 1.0);

    let layered = stack_with(FalseSkinTier::Fan, 1.0);
    assert_eq!(naked_defense_damage_multiplier(Some(&layered), 199), 1.0);
}

#[test]
fn shed_to_carrier_absorbs_damage_with_capacity() {
    let mut l = layer(FalseSkinTier::Light, 1.0);
    let outcome = shed_to_carrier(&mut l, 30.0, 2.0);
    assert_eq!(outcome.damage_absorbed, 30.0);
    assert_eq!(outcome.damage_overflow, 0.0);
}

#[test]
fn shed_to_carrier_overflows_damage_above_capacity() {
    let mut l = layer(FalseSkinTier::Fan, 1.0);
    let outcome = shed_to_carrier(&mut l, 50.0, 0.0);
    assert_eq!(outcome.damage_absorbed, 20.0);
    assert_eq!(outcome.damage_overflow, 30.0);
}

#[test]
fn shed_to_carrier_records_damage_taken() {
    let mut l = layer(FalseSkinTier::Mid, 1.0);
    shed_to_carrier(&mut l, 12.0, 0.0);
    assert_eq!(l.damage_taken, 12.0);
}

#[test]
fn shed_to_carrier_records_contam_load() {
    let mut l = layer(FalseSkinTier::Mid, 1.0);
    shed_to_carrier(&mut l, 0.0, 7.0);
    assert_eq!(l.contam_load, 7.0);
}

#[test]
fn shed_to_carrier_marks_depleted_on_damage_capacity() {
    let mut l = layer(FalseSkinTier::Fan, 1.0);
    let outcome = shed_to_carrier(&mut l, 20.0, 0.0);
    assert!(outcome.depleted);
}

#[test]
fn shed_to_carrier_marks_depleted_on_contam_capacity() {
    let mut l = layer(FalseSkinTier::Ancient, 1.0);
    let outcome = shed_to_carrier(&mut l, 0.0, 100.0);
    assert!(outcome.depleted);
}

#[test]
fn transfer_moves_realm_limit() {
    let mut stack = stack_with(FalseSkinTier::Mid, 1.0);
    let outcome =
        transfer_taint_to_outer_skin(&mut stack, Realm::Solidify, 20.0, 200.0, None).unwrap();
    assert_eq!(outcome.contam_moved_percent, 5.0);
    assert_eq!(outcome.qi_cost, 50.0);
}

#[test]
fn transfer_is_limited_by_qi() {
    let mut stack = stack_with(FalseSkinTier::Mid, 1.0);
    let outcome =
        transfer_taint_to_outer_skin(&mut stack, Realm::Solidify, 5.0, 20.0, None).unwrap();
    assert_eq!(outcome.contam_moved_percent, 2.0);
}

#[test]
fn transfer_is_limited_by_capacity() {
    let mut stack = stack_with(FalseSkinTier::Mid, 1.0);
    stack.outer_mut().unwrap().contam_load = 99.0;
    let outcome =
        transfer_taint_to_outer_skin(&mut stack, Realm::Solidify, 5.0, 200.0, None).unwrap();
    assert_eq!(outcome.contam_moved_percent, 1.0);
}

#[test]
fn transfer_backflow_caps_at_five_percent() {
    let mut stack = stack_with(FalseSkinTier::Mid, 1.0);
    stack.outer_mut().unwrap().contam_load = 99.0;
    let outcome = transfer_taint_to_outer_skin(&mut stack, Realm::Void, 20.0, 999.0, None).unwrap();
    assert_eq!(outcome.backflow_percent, 5.0);
}

#[test]
fn transfer_records_contam_on_outer_skin() {
    let mut stack = stack_with(FalseSkinTier::Mid, 1.0);
    transfer_taint_to_outer_skin(&mut stack, Realm::Solidify, 3.0, 100.0, None).unwrap();
    assert_eq!(stack.outer().unwrap().contam_load, 3.0);
}

#[test]
fn transfer_standard_cooldown_is_shorter_than_permanent_absorb_cooldown() {
    assert_eq!(
        transfer_cooldown_ticks(0.0),
        TRANSFER_STANDARD_COOLDOWN_TICKS
    );
    assert_eq!(
        transfer_cooldown_ticks(0.25),
        TRANSFER_PERMANENT_COOLDOWN_TICKS
    );
}

#[test]
fn transfer_absorbs_permanent_on_void_ancient() {
    let mut stack = stack_with(FalseSkinTier::Ancient, 1.0);
    let outcome =
        transfer_taint_to_outer_skin(&mut stack, Realm::Void, 0.0, 100.0, Some(0.25)).unwrap();
    assert_eq!(outcome.permanent_absorbed, 0.25);
}

#[test]
fn transfer_does_not_absorb_permanent_on_non_ancient() {
    let mut stack = stack_with(FalseSkinTier::Heavy, 1.0);
    let outcome =
        transfer_taint_to_outer_skin(&mut stack, Realm::Void, 0.0, 100.0, Some(0.25)).unwrap();
    assert_eq!(outcome.permanent_absorbed, 0.0);
}

#[test]
fn transfer_returns_none_without_skin() {
    let mut stack = StackedFalseSkins::default();
    assert!(transfer_taint_to_outer_skin(&mut stack, Realm::Void, 1.0, 100.0, None).is_none());
}

#[test]
fn stack_push_respects_limit() {
    let mut stack = stack_with(FalseSkinTier::Fan, 1.0);
    assert!(!stack.push_outer(layer(FalseSkinTier::Light, 1.0), 1));
}

#[test]
fn stack_shed_sets_naked_window_when_empty() {
    let mut stack = stack_with(FalseSkinTier::Fan, 1.0);
    assert!(stack.shed_outer(10).is_some());
    assert_eq!(stack.naked_until_tick, 10 + 5 * TICKS_PER_SECOND);
}

#[test]
fn stack_shed_keeps_inner_layer() {
    let mut stack = stack_with(FalseSkinTier::Fan, 1.0);
    assert!(stack.push_outer(layer(FalseSkinTier::Light, 1.0), 2));
    assert_eq!(stack.shed_outer(10).unwrap().tier, FalseSkinTier::Light);
    assert_eq!(stack.outer().unwrap().tier, FalseSkinTier::Fan);
}

#[test]
fn worn_false_skin_mirrors_outer_layer() {
    let l = layer(FalseSkinTier::Heavy, 2.0);
    let worn = WornFalseSkin::from(&l);
    assert_eq!(worn.tier, FalseSkinTier::Heavy);
    assert_eq!(worn.spirit_quality, 2.0);
}

#[test]
fn false_skin_layer_quality_is_clamped_low() {
    assert_eq!(
        FalseSkinLayer::new(1, FalseSkinTier::Fan, -1.0, 0).spirit_quality,
        0.1
    );
}

#[test]
fn false_skin_layer_quality_is_clamped_high() {
    assert_eq!(
        FalseSkinLayer::new(1, FalseSkinTier::Fan, 20.0, 0).spirit_quality,
        10.0
    );
}

#[test]
fn cast_don_rejects_missing_inventory_skin() {
    let (mut world, entity) = world_with_player(Realm::Void, 1000.0, "stone");
    assert_rejected(
        cast_don(&mut world, entity, 0, None),
        CastRejectReason::InvalidTarget,
    );
}

#[test]
fn cast_don_rejects_realm_too_low_for_ancient() {
    let (mut world, entity) = world_with_player(Realm::Spirit, 1000.0, FALSE_SKIN_ANCIENT_ITEM_ID);
    assert_rejected(
        cast_don(&mut world, entity, 0, None),
        CastRejectReason::RealmTooLow,
    );
}

#[test]
fn cast_don_adds_outer_skin_and_event() {
    let (mut world, entity) = world_with_player(Realm::Void, 1000.0, FALSE_SKIN_ANCIENT_ITEM_ID);
    assert_eq!(assert_started(cast_don(&mut world, entity, 0, None)), 20);
    assert_eq!(
        world
            .get::<StackedFalseSkins>(entity)
            .unwrap()
            .layer_count(),
        1
    );
    assert_eq!(
        world
            .resource::<Events<DonFalseSkinEvent>>()
            .get_reader()
            .len(world.resource::<Events<DonFalseSkinEvent>>()),
        1
    );
}

#[test]
fn cast_don_updates_derived_attrs_layers() {
    let (mut world, entity) = world_with_player(Realm::Void, 1000.0, FALSE_SKIN_ANCIENT_ITEM_ID);
    assert_started(cast_don(&mut world, entity, 0, None));
    assert_eq!(world.get::<DerivedAttrs>(entity).unwrap().tuike_layers, 1);
}

#[test]
fn cast_shed_rejects_when_no_stack() {
    let (mut world, entity) = world_with_player(Realm::Void, 1000.0, "stone");
    assert_rejected(
        cast_shed(&mut world, entity, 0, None),
        CastRejectReason::InvalidTarget,
    );
    assert_eq!(world.get::<Cultivation>(entity).unwrap().qi_current, 1000.0);
}

#[test]
fn cast_shed_spends_qi_and_sheds() {
    let (mut world, entity) = world_with_player(Realm::Void, 1000.0, FALSE_SKIN_ANCIENT_ITEM_ID);
    assert_started(cast_don(&mut world, entity, 0, None));
    assert_eq!(
        assert_started(cast_shed(&mut world, entity, 1, None)),
        ACTIVE_SHED_COOLDOWN_TICKS
    );
    assert!(world.get::<WornFalseSkin>(entity).is_none());
    assert!((world.get::<Cultivation>(entity).unwrap().qi_current - 940.0).abs() < 1e-9);
}

#[test]
fn cast_shed_routes_spent_qi_to_overflow_without_zone_context() {
    let (mut world, entity) = world_with_player(Realm::Void, 1000.0, FALSE_SKIN_ANCIENT_ITEM_ID);
    assert_started(cast_don(&mut world, entity, 0, None));
    assert_started(cast_shed(&mut world, entity, 1, None));

    let qi_transfers = world.resource::<Events<crate::qi_physics::QiTransfer>>();
    let transfers = qi_transfers
        .iter_current_update_events()
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 1);
    assert_eq!(transfers[0].amount, 60.0);
    assert_eq!(
        transfers[0].reason,
        crate::qi_physics::QiTransferReason::ReleaseToZone
    );
    assert_eq!(
        transfers[0].to.kind,
        crate::qi_physics::QiAccountKind::Overflow
    );
}

#[test]
fn shed_outer_layer_emits_residue() {
    let (mut world, entity) = world_with_player(Realm::Void, 1000.0, FALSE_SKIN_ANCIENT_ITEM_ID);
    assert_started(cast_don(&mut world, entity, 0, None));
    let event = shed_outer_layer(&mut world, entity, None, 12.0, 3.0, false, 200).unwrap();
    assert_eq!(event.damage_absorbed, 12.0);
    let residues = world
        .query::<&FalseSkinResidue>()
        .iter(&world)
        .collect::<Vec<_>>();
    assert_eq!(residues.len(), 1);
    assert_eq!(residues[0].owner, entity);
}

#[test]
fn cast_transfer_rejects_without_contam_or_permanent_marker() {
    let (mut world, entity) = world_with_player(Realm::Void, 1000.0, FALSE_SKIN_ANCIENT_ITEM_ID);
    assert_started(cast_don(&mut world, entity, 0, None));
    assert_rejected(
        cast_transfer_taint(&mut world, entity, 1, None),
        CastRejectReason::InvalidTarget,
    );
}

#[test]
fn cast_transfer_moves_contamination_and_spends_qi() {
    let (mut world, entity) = world_with_player(Realm::Solidify, 100.0, FALSE_SKIN_MID_ITEM_ID);
    world.entity_mut(entity).insert(contam(8.0));
    assert_started(cast_don(&mut world, entity, 0, None));
    assert_eq!(
        assert_started(cast_transfer_taint(&mut world, entity, 1, None)),
        TRANSFER_STANDARD_COOLDOWN_TICKS
    );
    assert_eq!(
        world.get::<Contamination>(entity).unwrap().entries[0].amount,
        3.0
    );
    assert_eq!(world.get::<WornFalseSkin>(entity).unwrap().contam_load, 5.0);
    assert!((world.get::<Cultivation>(entity).unwrap().qi_current - 50.0).abs() < 1e-9);
}

#[test]
fn cast_transfer_absorbs_permanent_marker_on_void_ancient() {
    let (mut world, entity) = world_with_player(Realm::Void, 1000.0, FALSE_SKIN_ANCIENT_ITEM_ID);
    world.entity_mut(entity).insert(PermanentQiMaxDecay {
        source: entity,
        amount: 0.3,
        applied_at_tick: 90,
    });
    assert_started(cast_don(&mut world, entity, 0, None));
    assert_eq!(
        assert_started(cast_transfer_taint(&mut world, entity, 1, None)),
        TRANSFER_PERMANENT_COOLDOWN_TICKS
    );
    assert!(world.get::<PermanentQiMaxDecay>(entity).is_none());
    assert_eq!(
        world
            .get::<WornFalseSkin>(entity)
            .unwrap()
            .permanent_taint_load,
        0.3
    );
}
