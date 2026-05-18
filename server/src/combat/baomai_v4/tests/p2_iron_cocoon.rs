//! P2 活茧（Iron Cocoon）测试。

use valence::prelude::{App, Events};

use crate::combat::baomai_v3::events::OverloadMeridianRippleEvent;
use crate::combat::baomai_v4::constants::{
    IRON_BONE_FRACTURE_DOWNGRADE_CHANCE, IRON_COCOON_THRESHOLDS,
    TOUGH_SKIN_BRUISE_THRESHOLD_MULTIPLIER,
};
use crate::combat::baomai_v4::events::IronCocoonStageUpEvent;
use crate::combat::baomai_v4::iron_cocoon::{IronCocoonStage, IronCocoonTracker};
use crate::combat::baomai_v4::scar_circuit::ActiveScarCircuits;
use crate::combat::baomai_v4::scar_history::ScarHistory;
use crate::combat::components::DerivedAttrs;
use crate::combat::events::CombatEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::MeridianSystem;

fn app() -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 100 });
    app.add_event::<OverloadMeridianRippleEvent>();
    app.add_event::<CombatEvent>();
    crate::combat::baomai_v4::register(&mut app);
    app
}

fn spawn_cocoon_entity(app: &mut App, total_overloads: u32) -> valence::prelude::Entity {
    app.world_mut()
        .spawn((
            ScarHistory {
                total_overloads,
                ..Default::default()
            },
            DerivedAttrs::default(),
            IronCocoonTracker::default(),
            ActiveScarCircuits::default(),
            MeridianSystem::default(),
        ))
        .id()
}

// ── from_overloads pin tests ──

#[test]
fn cocoon_stage_none_below_threshold() {
    assert_eq!(
        IronCocoonStage::from_overloads(0),
        IronCocoonStage::None,
        "0 overloads should be None"
    );
    assert_eq!(
        IronCocoonStage::from_overloads(49),
        IronCocoonStage::None,
        "49 overloads (just below 50) should still be None"
    );
}

#[test]
fn cocoon_stage_tough_skin_at_50() {
    assert_eq!(
        IronCocoonStage::from_overloads(50),
        IronCocoonStage::ToughSkin,
        "50 overloads should be ToughSkin"
    );
    assert_eq!(
        IronCocoonStage::from_overloads(119),
        IronCocoonStage::ToughSkin,
        "119 overloads (just below 120) should still be ToughSkin"
    );
}

#[test]
fn cocoon_stage_iron_bone_at_120() {
    assert_eq!(
        IronCocoonStage::from_overloads(120),
        IronCocoonStage::IronBone,
        "120 overloads should be IronBone"
    );
    assert_eq!(
        IronCocoonStage::from_overloads(249),
        IronCocoonStage::IronBone,
        "249 overloads (just below 250) should still be IronBone"
    );
}

#[test]
fn cocoon_stage_dull_flesh_at_250() {
    assert_eq!(
        IronCocoonStage::from_overloads(250),
        IronCocoonStage::DullFlesh,
        "250 overloads should be DullFlesh"
    );
    assert_eq!(
        IronCocoonStage::from_overloads(499),
        IronCocoonStage::DullFlesh,
        "499 overloads (just below 500) should still be DullFlesh"
    );
}

#[test]
fn cocoon_stage_scar_forged_at_500() {
    assert_eq!(
        IronCocoonStage::from_overloads(500),
        IronCocoonStage::ScarForged,
        "500 overloads should be ScarForged"
    );
    assert_eq!(
        IronCocoonStage::from_overloads(9999),
        IronCocoonStage::ScarForged,
        "Very high overloads should still be ScarForged (max stage)"
    );
}

// ── Passive effect tests ──

#[test]
fn cocoon_bruise_threshold_at_tough_skin() {
    let mut app = app();
    let entity = spawn_cocoon_entity(&mut app, 50);

    app.update();

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        (attrs.bruise_threshold_multiplier - TOUGH_SKIN_BRUISE_THRESHOLD_MULTIPLIER).abs() < 1e-6,
        "ToughSkin should set bruise_threshold_multiplier to {}, got {}",
        TOUGH_SKIN_BRUISE_THRESHOLD_MULTIPLIER,
        attrs.bruise_threshold_multiplier
    );
}

#[test]
fn cocoon_fracture_downgrade_probability() {
    // IronBone sets fracture_downgrade_chance to 0.20.
    // We verify the DerivedAttrs field, not actual RNG (that's at resolution time).
    let mut app = app();
    let entity = spawn_cocoon_entity(&mut app, 120);

    app.update();

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        (attrs.fracture_downgrade_chance - IRON_BONE_FRACTURE_DOWNGRADE_CHANCE).abs() < 1e-6,
        "IronBone should set fracture_downgrade_chance to {}, got {}",
        IRON_BONE_FRACTURE_DOWNGRADE_CHANCE,
        attrs.fracture_downgrade_chance
    );
}

#[test]
fn cocoon_cut_wound_downgrade() {
    // DullFlesh sets cut_pierce_downgrade = true.
    let mut app = app();
    let entity = spawn_cocoon_entity(&mut app, 250);

    app.update();

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        attrs.cut_pierce_downgrade,
        "DullFlesh should set cut_pierce_downgrade = true"
    );
}

#[test]
fn cocoon_blunt_not_downgraded() {
    // DullFlesh only downgrades Cut/Pierce. The flag is just cut_pierce_downgrade.
    // Blunt would be checked at resolution time (not this flag).
    // Here we verify the flag is purely for Cut/Pierce — a None stage has it false.
    let mut app = app();
    let entity = spawn_cocoon_entity(&mut app, 0);

    app.update();

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        !attrs.cut_pierce_downgrade,
        "None stage should have cut_pierce_downgrade = false (blunt never downgraded)"
    );

    // And verify DullFlesh has it true (Cut/Pierce only).
    let entity2 = spawn_cocoon_entity(&mut app, 250);
    app.update();
    let attrs2 = app.world().get::<DerivedAttrs>(entity2).unwrap();
    assert!(
        attrs2.cut_pierce_downgrade,
        "DullFlesh (250 overloads) should have cut_pierce_downgrade = true for Cut/Pierce"
    );
}

#[test]
fn cocoon_scar_forged_flow_rate_bonus() {
    // ScarForged sets scar_forged_flow_bonus = true.
    let mut app = app();
    let entity = spawn_cocoon_entity(&mut app, 500);

    app.update();

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        attrs.scar_forged_flow_bonus,
        "ScarForged should set scar_forged_flow_bonus = true"
    );
}

#[test]
fn cocoon_scar_forged_no_bonus_without_high_overloads() {
    // Below ScarForged threshold, scar_forged_flow_bonus should be false.
    let mut app = app();
    let entity = spawn_cocoon_entity(&mut app, 499);

    app.update();

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        !attrs.scar_forged_flow_bonus,
        "Below ScarForged threshold (499 < 500), scar_forged_flow_bonus should be false"
    );
}

#[test]
fn cocoon_preserves_across_death() {
    // ScarHistory persists as a Component — simulated by verifying the value survives
    // across multiple updates without mutation.
    let mut app = app();
    let entity = spawn_cocoon_entity(&mut app, 120);

    app.update();
    app.update();
    app.update();

    let history = app.world().get::<ScarHistory>(entity).unwrap();
    assert_eq!(
        history.total_overloads, 120,
        "ScarHistory should persist across multiple updates (death simulation)"
    );

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        (attrs.fracture_downgrade_chance - IRON_BONE_FRACTURE_DOWNGRADE_CHANCE).abs() < 1e-6,
        "IronBone passive should persist across updates"
    );
}

#[test]
fn cocoon_event_emitted_on_stage_up() {
    // Spawn at 49 (None), then bump to 50 (ToughSkin) → event emitted.
    let mut app = app();
    let entity = spawn_cocoon_entity(&mut app, 49);

    // First update — stage is None, tracker records None.
    app.update();

    // Bump overloads to 50 → ToughSkin.
    app.world_mut()
        .get_mut::<ScarHistory>(entity)
        .unwrap()
        .total_overloads = 50;

    app.update();

    let events = app.world().resource::<Events<IronCocoonStageUpEvent>>();
    let mut reader = events.get_reader();
    let stage_ups: Vec<_> = reader.read(events).collect();
    assert!(
        stage_ups.iter().any(|e| e.entity == entity
            && e.from == IronCocoonStage::None
            && e.to == IronCocoonStage::ToughSkin
            && e.total_overloads == 50),
        "Should emit IronCocoonStageUpEvent from None to ToughSkin at 50 overloads. Got: {:?}",
        stage_ups
    );
}

#[test]
fn cocoon_no_event_when_stage_unchanged() {
    // Stay at same stage → no event on second update.
    let mut app = app();
    let _entity = spawn_cocoon_entity(&mut app, 55);

    // First update — tracker initializes to ToughSkin (may emit None→ToughSkin event).
    app.update();

    // Create a persistent reader and drain all events from the first update.
    let mut reader = app
        .world()
        .resource::<Events<IronCocoonStageUpEvent>>()
        .get_reader();
    {
        let events = app.world().resource::<Events<IronCocoonStageUpEvent>>();
        let _: Vec<_> = reader.read(events).collect();
    }

    // Second update — still ToughSkin, should NOT emit any new event.
    app.update();

    let events = app.world().resource::<Events<IronCocoonStageUpEvent>>();
    let stage_ups: Vec<_> = reader.read(events).collect();
    assert_eq!(
        stage_ups.len(),
        0,
        "Should not emit any stage-up event when stage is unchanged on second update. Got {} events: {:?}",
        stage_ups.len(),
        stage_ups
    );
}

#[test]
fn cocoon_stages_are_ordered() {
    // Verify PartialOrd ordering.
    assert!(IronCocoonStage::None < IronCocoonStage::ToughSkin);
    assert!(IronCocoonStage::ToughSkin < IronCocoonStage::IronBone);
    assert!(IronCocoonStage::IronBone < IronCocoonStage::DullFlesh);
    assert!(IronCocoonStage::DullFlesh < IronCocoonStage::ScarForged);
}

#[test]
fn cocoon_passives_cumulative() {
    // Higher stages include all lower-stage passives.
    // DullFlesh should have bruise_threshold + fracture_downgrade + cut_pierce_downgrade.
    let mut app = app();
    let entity = spawn_cocoon_entity(&mut app, 250);

    app.update();

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        (attrs.bruise_threshold_multiplier - TOUGH_SKIN_BRUISE_THRESHOLD_MULTIPLIER).abs() < 1e-6,
        "DullFlesh should also have ToughSkin's bruise_threshold_multiplier"
    );
    assert!(
        (attrs.fracture_downgrade_chance - IRON_BONE_FRACTURE_DOWNGRADE_CHANCE).abs() < 1e-6,
        "DullFlesh should also have IronBone's fracture_downgrade_chance"
    );
    assert!(
        attrs.cut_pierce_downgrade,
        "DullFlesh should have its own cut_pierce_downgrade"
    );
}

#[test]
fn cocoon_from_overloads_uses_thresholds_constant() {
    // Verify from_overloads uses the IRON_COCOON_THRESHOLDS constant.
    assert_eq!(
        IronCocoonStage::from_overloads(IRON_COCOON_THRESHOLDS[0]),
        IronCocoonStage::ToughSkin,
        "from_overloads at threshold[0]={} should be ToughSkin",
        IRON_COCOON_THRESHOLDS[0]
    );
    assert_eq!(
        IronCocoonStage::from_overloads(IRON_COCOON_THRESHOLDS[0] - 1),
        IronCocoonStage::None,
        "from_overloads at threshold[0]-1={} should be None",
        IRON_COCOON_THRESHOLDS[0] - 1
    );
    assert_eq!(
        IronCocoonStage::from_overloads(IRON_COCOON_THRESHOLDS[1]),
        IronCocoonStage::IronBone,
    );
    assert_eq!(
        IronCocoonStage::from_overloads(IRON_COCOON_THRESHOLDS[2]),
        IronCocoonStage::DullFlesh,
    );
    assert_eq!(
        IronCocoonStage::from_overloads(IRON_COCOON_THRESHOLDS[3]),
        IronCocoonStage::ScarForged,
    );
}
