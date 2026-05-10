use valence::prelude::{App, DVec3, Entity, Events, Position, Startup, Update};

use crate::combat::components::{SkillBarBindings, TICKS_PER_SECOND};
use crate::combat::CombatClock;
use crate::cultivation::components::{
    Contamination, Cultivation, MeridianId, MeridianSystem, Realm,
};
use crate::cultivation::meridian::severed::{
    MeridianSeveredPermanent, SeveredSource, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult};
use crate::cultivation::tribulation::{JueBiTriggerEvent, JueBiTriggerSource};
use crate::qi_physics::{QiAccountId, QiAccountKind, QiTransfer, QiTransferReason};
use crate::skill::events::SkillXpGain;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::{default_spawn_bounds, Zone, ZoneRegistry};

use super::backfire::{
    apply_backfire_to_hand_meridians, backfire_level_for_overflow, forced_backfire,
};
use super::events::{
    BackfireCauseV2, BackfireLevel, EntityDisplacedByVortexPull, TurbulenceFieldDecayed,
    TurbulenceFieldSpawned, VortexBackfireEventV2, VortexCastEvent, WoliuSkillId,
};
use super::physics::{
    lethal_and_influence_radius, pull_displacement_blocks, realm_absorption_rate, stir_99_1,
    turbulence_decay_step, StirInput,
};
use super::skills::{
    declare_woliu_v2_meridian_dependencies, resolve_woliu_v2_skill, skill_spec, visual_for,
};
use super::state::{PassiveVortex, TurbulenceExposure, TurbulenceField, VortexV2State};

fn realm_case(index: usize) -> Realm {
    match index % 6 {
        0 => Realm::Awaken,
        1 => Realm::Induce,
        2 => Realm::Condense,
        3 => Realm::Solidify,
        4 => Realm::Spirit,
        _ => Realm::Void,
    }
}

fn skill_case(index: usize) -> WoliuSkillId {
    WoliuSkillId::ALL[index % WoliuSkillId::ALL.len()]
}

fn assert_spec_case(index: usize) {
    let skill = skill_case(index);
    let realm = realm_case(index / WoliuSkillId::ALL.len());
    let spec = skill_spec(skill, realm);
    assert_eq!(spec.skill, skill);
    assert_eq!(spec.visual, visual_for(skill));
    assert!(spec.cooldown_ticks > 0);
    assert!(spec.cast_ticks > 0);
    assert!(spec.duration_ticks > 0);
    assert!(spec.total_qi_cost().is_finite());
    assert!(spec.total_drained().is_finite());
    if skill == WoliuSkillId::Heart && realm == Realm::Void {
        assert!(spec.influence_radius >= 100.0);
        assert!(!spec.passive_default_enabled);
    }
    if skill == WoliuSkillId::Burst {
        assert!(spec.field_strength <= 0.5);
    }
}

fn app(tick: u64) -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick });
    app.add_event::<VortexCastEvent>();
    app.add_event::<VortexBackfireEventV2>();
    app.add_event::<TurbulenceFieldSpawned>();
    app.add_event::<TurbulenceFieldDecayed>();
    app.add_event::<EntityDisplacedByVortexPull>();
    app.add_event::<JueBiTriggerEvent>();
    app.add_event::<QiTransfer>();
    app.add_event::<SkillXpGain>();
    app
}

fn spawn_actor(app: &mut App, realm: Realm, qi_current: f64) -> Entity {
    app.world_mut()
        .spawn((
            Cultivation {
                realm,
                qi_current,
                qi_max: 1_000.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            SkillBarBindings::default(),
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id()
}

fn open_all_meridians(app: &mut App, actor: Entity, capacity: f64) {
    let mut meridians = app.world_mut().get_mut::<MeridianSystem>(actor).unwrap();
    for meridian in meridians.iter_mut() {
        meridian.opened = true;
        meridian.integrity = 1.0;
        meridian.flow_capacity = capacity;
    }
}

fn two_zone_registry() -> ZoneRegistry {
    let (spawn_min, spawn_max) = default_spawn_bounds();
    ZoneRegistry {
        zones: vec![
            Zone {
                name: "spawn".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (spawn_min, spawn_max),
                spirit_qi: 0.9,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: vec![DVec3::new(8.0, 66.0, 8.0)],
                blocked_tiles: Vec::new(),
            },
            Zone {
                name: "nearby_training_ring".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(19.0, 64.0, 7.0), DVec3::new(21.0, 80.0, 9.0)),
                spirit_qi: 0.7,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: vec![DVec3::new(20.0, 66.0, 8.0)],
                blocked_tiles: Vec::new(),
            },
        ],
    }
}

#[test]
fn all_skill_ids_are_stable_wire_strings() {
    assert_eq!(WoliuSkillId::Hold.as_str(), "woliu.hold");
    assert_eq!(WoliuSkillId::Burst.as_str(), "woliu.burst");
    assert_eq!(WoliuSkillId::Mouth.as_str(), "woliu.mouth");
    assert_eq!(WoliuSkillId::Pull.as_str(), "woliu.pull");
    assert_eq!(WoliuSkillId::Heart.as_str(), "woliu.heart");
    assert_eq!(WoliuSkillId::VacuumPalm.as_str(), "woliu.vacuum_palm");
    assert_eq!(WoliuSkillId::VortexShield.as_str(), "woliu.vortex_shield");
    assert_eq!(WoliuSkillId::VacuumLock.as_str(), "woliu.vacuum_lock");
    assert_eq!(
        WoliuSkillId::VortexResonance.as_str(),
        "woliu.vortex_resonance"
    );
    assert_eq!(
        WoliuSkillId::TurbulenceBurst.as_str(),
        "woliu.turbulence_burst"
    );
}

#[test]
fn practice_xp_matches_plan_amounts() {
    assert_eq!(WoliuSkillId::Hold.practice_xp(), 1);
    assert_eq!(WoliuSkillId::Burst.practice_xp(), 2);
    assert_eq!(WoliuSkillId::Mouth.practice_xp(), 2);
    assert_eq!(WoliuSkillId::Pull.practice_xp(), 3);
    assert_eq!(WoliuSkillId::Heart.practice_xp(), 5);
    assert_eq!(WoliuSkillId::VacuumPalm.practice_xp(), 2);
    assert_eq!(WoliuSkillId::VortexShield.practice_xp(), 2);
    assert_eq!(WoliuSkillId::VacuumLock.practice_xp(), 3);
    assert_eq!(WoliuSkillId::VortexResonance.practice_xp(), 4);
    assert_eq!(WoliuSkillId::TurbulenceBurst.practice_xp(), 5);
}

#[test]
fn stir_99_1_conserves_drained_qi() {
    let outcome = stir_99_1(StirInput {
        total_drained: 120.0,
        realm: Realm::Void,
        contamination_ratio: 0.0,
        meridian_flow_capacity: 10.0,
        dt_seconds: 1.0,
    });
    assert!((outcome.total_drained - outcome.total_output()).abs() < 1e-9);
}

#[test]
fn stir_99_1_caps_absorption_by_meridian_capacity() {
    let outcome = stir_99_1(StirInput {
        total_drained: 10_000.0,
        realm: Realm::Void,
        contamination_ratio: 0.0,
        meridian_flow_capacity: 1.0,
        dt_seconds: 1.0,
    });
    assert_eq!(outcome.actual_absorbed, 1.0);
    assert!(outcome.overflow > 0.0);
}

#[test]
fn backfire_thresholds_match_four_step_table() {
    assert_eq!(
        backfire_level_for_overflow(1.0, 100.0),
        Some(BackfireLevel::Sensation)
    );
    assert_eq!(
        backfire_level_for_overflow(15.0, 100.0),
        Some(BackfireLevel::MicroTear)
    );
    assert_eq!(
        backfire_level_for_overflow(40.0, 100.0),
        Some(BackfireLevel::Torn)
    );
    assert_eq!(
        backfire_level_for_overflow(70.0, 100.0),
        Some(BackfireLevel::Severed)
    );
}

#[test]
fn tsy_heart_forces_severed_backfire() {
    assert_eq!(
        forced_backfire(WoliuSkillId::Heart, DimensionKind::Tsy, 1.0),
        Some((BackfireLevel::Severed, BackfireCauseV2::TsyNegativeField))
    );
}

#[test]
fn void_heart_long_active_triggers_tribulation_backfire() {
    assert_eq!(
        forced_backfire(WoliuSkillId::Heart, DimensionKind::Overworld, 30.0),
        Some((
            BackfireLevel::Severed,
            BackfireCauseV2::VoidHeartTribulation
        ))
    );
}

#[test]
fn severed_backfire_closes_lung_meridian() {
    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    apply_backfire_to_hand_meridians(&mut meridians, BackfireLevel::Severed);
    let lung = meridians.get(MeridianId::Lung);
    assert!(!lung.opened);
    assert_eq!(lung.integrity, 0.0);
    assert_eq!(lung.flow_capacity, 0.0);
}

#[test]
fn turbulence_decay_returns_qi_to_static_zone_over_time() {
    let (decayed, remaining) = turbulence_decay_step(100.0, 0.05, 1.0);
    assert!(decayed > 0.0);
    assert!(remaining < 100.0);
    assert!((decayed + remaining - 100.0).abs() < 1e-9);
}

#[test]
fn turbulence_exposure_effects_match_plan_multipliers() {
    let exposure = TurbulenceExposure::new(Entity::from_raw(1), 1.0, 1);
    assert_eq!(exposure.absorption_multiplier(), 0.0);
    assert_eq!(exposure.cast_precision_multiplier(), 0.5);
    assert_eq!(exposure.env_field().turbulence_shelflife_factor(), 3.0);
    assert_eq!(exposure.defense_drain_multiplier(), 1.2);
}

#[test]
fn pull_displacement_never_moves_zero_qi_targets_by_infinite_amount() {
    assert_eq!(pull_displacement_blocks(100.0, 0.0, 8.0), 0.0);
    assert_eq!(pull_displacement_blocks(0.0, 10.0, 8.0), 0.0);
}

#[test]
fn pull_displacement_scales_by_caster_to_target_qi_ratio() {
    assert_eq!(pull_displacement_blocks(100.0, 10.0, 2.5), 25.0);
}

#[test]
fn realm_absorption_rate_is_monotonic() {
    let chain = [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ];
    for pair in chain.windows(2) {
        assert!(realm_absorption_rate(pair[1]) >= realm_absorption_rate(pair[0]));
    }
}

#[test]
fn lethal_and_influence_radius_uses_same_spec_surface() {
    let (lethal, influence) = lethal_and_influence_radius(WoliuSkillId::Heart, Realm::Void);
    let spec = skill_spec(WoliuSkillId::Heart, Realm::Void);
    assert_eq!(lethal, spec.lethal_radius);
    assert_eq!(influence, spec.influence_radius);
}

#[test]
fn resolve_hold_emits_cast_xp_and_qi_transfers() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 0, None, WoliuSkillId::Hold);
    assert!(matches!(result, CastResult::Started { .. }));
    assert!(app.world().get::<VortexV2State>(actor).is_some());
    assert!(app.world().get::<TurbulenceField>(actor).is_some());
    assert_eq!(
        app.world()
            .resource::<Events<VortexCastEvent>>()
            .iter_current_update_events()
            .count(),
        1
    );
    assert_eq!(
        app.world()
            .resource::<Events<SkillXpGain>>()
            .iter_current_update_events()
            .count(),
        1
    );
    assert!(app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .any(|event| event.reason == QiTransferReason::Channeling));
}

#[test]
fn resolve_hold_persists_stir_contamination_gain() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
    open_all_meridians(&mut app, actor, 10_000.0);

    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 0, None, WoliuSkillId::Hold);

    assert!(matches!(result, CastResult::Started { .. }));
    let contamination = app.world().get::<Contamination>(actor).unwrap();
    let entry = contamination
        .entries
        .last()
        .expect("stir should persist contamination gain");
    assert!(entry.amount > 0.0);
    assert_eq!(
        entry.color,
        crate::cultivation::components::ColorKind::Intricate
    );
    assert_eq!(entry.introduced_at, 10);
}

#[test]
fn startup_declares_expected_meridian_dependencies_for_woliu_skills() {
    let mut app = App::new();
    app.insert_resource(SkillMeridianDependencies::default());
    app.add_systems(Startup, declare_woliu_v2_meridian_dependencies);

    app.update();

    let deps = app.world().resource::<SkillMeridianDependencies>();
    for skill in [
        WoliuSkillId::Hold,
        WoliuSkillId::Burst,
        WoliuSkillId::Mouth,
        WoliuSkillId::Pull,
        WoliuSkillId::Heart,
    ] {
        assert_eq!(deps.lookup(skill.as_str()), &[MeridianId::Lung]);
    }
    for skill in [
        WoliuSkillId::VacuumPalm,
        WoliuSkillId::VortexShield,
        WoliuSkillId::VacuumLock,
        WoliuSkillId::VortexResonance,
        WoliuSkillId::TurbulenceBurst,
    ] {
        assert_eq!(
            deps.lookup(skill.as_str()),
            &[MeridianId::Lung, MeridianId::Heart]
        );
    }
}

#[test]
fn resolve_rejects_when_lung_meridian_is_permanently_severed() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
    let mut severed = MeridianSeveredPermanent::default();
    severed.insert(MeridianId::Lung, SeveredSource::BackfireOverload, 10);
    app.world_mut().entity_mut(actor).insert(severed);

    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 0, None, WoliuSkillId::Hold);

    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(MeridianId::Lung))
        }
    );
}

#[test]
fn resolve_v3_skill_rejects_when_heart_meridian_is_permanently_severed() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
    let mut severed = MeridianSeveredPermanent::default();
    severed.insert(MeridianId::Heart, SeveredSource::BackfireOverload, 10);
    app.world_mut().entity_mut(actor).insert(severed);

    let result =
        resolve_woliu_v2_skill(app.world_mut(), actor, 0, None, WoliuSkillId::VortexShield);

    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(MeridianId::Heart))
        }
    );
}

#[test]
fn turbulence_field_projects_runtime_exposure_to_overlapping_targets() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 42 });
    app.add_systems(Update, super::tick::update_turbulence_exposure_tick);
    let source = app
        .world_mut()
        .spawn((CurrentDimension(DimensionKind::Overworld),))
        .id();
    app.world_mut()
        .entity_mut(source)
        .insert(TurbulenceField::new(
            source,
            DVec3::new(8.0, 66.0, 8.0),
            3.0,
            1.0,
            100.0,
            42,
        ));
    let target = app
        .world_mut()
        .spawn((
            Cultivation::default(),
            Position::new([9.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();

    app.update();

    let exposure = app
        .world()
        .get::<TurbulenceExposure>(target)
        .expect("overlapping cultivator should receive turbulence exposure");
    assert_eq!(exposure.source, source);
    assert_eq!(exposure.absorption_multiplier(), 0.0);
    assert_eq!(exposure.cast_precision_multiplier(), 0.5);
    assert_eq!(exposure.defense_drain_multiplier(), 1.2);
}

#[test]
fn depleted_turbulence_field_does_not_project_runtime_exposure() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 42 });
    app.add_systems(Update, super::tick::update_turbulence_exposure_tick);
    let source = app
        .world_mut()
        .spawn((CurrentDimension(DimensionKind::Overworld),))
        .id();
    app.world_mut()
        .entity_mut(source)
        .insert(TurbulenceField::new(
            source,
            DVec3::new(8.0, 66.0, 8.0),
            3.0,
            1.0,
            0.0,
            42,
        ));
    let target = app
        .world_mut()
        .spawn((
            Cultivation::default(),
            Position::new([9.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();

    app.update();

    assert!(app.world().get::<TurbulenceExposure>(target).is_none());
}

#[test]
fn turbulence_exposure_halves_woliu_cast_precision_output() {
    fn swirl_for(exposure: Option<TurbulenceExposure>) -> f32 {
        let mut app = app(10);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        open_all_meridians(&mut app, actor, 10_000.0);
        if let Some(exposure) = exposure {
            app.world_mut().entity_mut(actor).insert(exposure);
        }

        let result = resolve_woliu_v2_skill(app.world_mut(), actor, 0, None, WoliuSkillId::Hold);

        assert!(matches!(result, CastResult::Started { .. }));
        let swirl_qi = app
            .world()
            .resource::<Events<VortexCastEvent>>()
            .iter_current_update_events()
            .next()
            .expect("cast should emit vortex event")
            .swirl_qi;
        swirl_qi
    }

    let normal = swirl_for(None);
    let exposed = swirl_for(Some(TurbulenceExposure::new(Entity::from_raw(99), 1.0, 11)));

    assert!(normal > 0.0);
    assert!((exposed - normal * 0.5).abs() < 1e-5);
}

#[test]
fn stir_transfers_use_registered_zone_accounts_instead_of_synthetic_turbulence_sink() {
    let mut app = app(10);
    app.insert_resource(two_zone_registry());
    let actor = spawn_actor(&mut app, Realm::Void, 1_000.0);
    open_all_meridians(&mut app, actor, 10_000.0);

    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 0, None, WoliuSkillId::Hold);

    assert!(matches!(result, CastResult::Started { .. }));
    let transfers: Vec<_> = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .collect();
    assert!(transfers
        .iter()
        .any(|transfer| transfer.to.kind == QiAccountKind::Zone
            && transfer.to.id == "nearby_training_ring"));
    assert!(!transfers.iter().any(|transfer| {
        transfer.to.kind == QiAccountKind::Zone && transfer.to.id.starts_with("woliu_v2_turbulence")
    }));
}

#[test]
fn resolve_rejects_qi_insufficient_without_cooldown() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Awaken, 1.0);
    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 2, None, WoliuSkillId::Mouth);
    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient
        }
    );
    assert!(!app
        .world()
        .get::<SkillBarBindings>(actor)
        .unwrap()
        .is_on_cooldown(2, 10));
}

#[test]
fn resolve_pull_rejects_missing_target_without_cooldown() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);

    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 1, None, WoliuSkillId::Pull);

    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget
        }
    );
    assert!(!app
        .world()
        .get::<SkillBarBindings>(actor)
        .unwrap()
        .is_on_cooldown(1, 10));
}

#[test]
fn resolve_pull_rejects_target_without_position_before_cooldown() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);
    let target = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Induce,
                qi_current: 20.0,
                qi_max: 1_000.0,
                ..Default::default()
            },
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();

    let result =
        resolve_woliu_v2_skill(app.world_mut(), actor, 1, Some(target), WoliuSkillId::Pull);

    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget
        }
    );
    assert!(!app
        .world()
        .get::<SkillBarBindings>(actor)
        .unwrap()
        .is_on_cooldown(1, 10));
}

#[test]
fn resolve_pull_rejects_out_of_range_target_without_cooldown() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);
    let target = spawn_actor(&mut app, Realm::Induce, 20.0);
    app.world_mut()
        .get_mut::<Position>(target)
        .unwrap()
        .set([20.0, 66.0, 8.0]);

    let result =
        resolve_woliu_v2_skill(app.world_mut(), actor, 1, Some(target), WoliuSkillId::Pull);

    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget
        }
    );
    assert!(!app
        .world()
        .get::<SkillBarBindings>(actor)
        .unwrap()
        .is_on_cooldown(1, 10));
}

#[test]
fn resolve_pull_rejects_zero_qi_target_without_cooldown() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);
    let target = spawn_actor(&mut app, Realm::Induce, 0.0);
    app.world_mut()
        .get_mut::<Position>(target)
        .unwrap()
        .set([10.0, 66.0, 8.0]);

    let result =
        resolve_woliu_v2_skill(app.world_mut(), actor, 1, Some(target), WoliuSkillId::Pull);

    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget
        }
    );
    assert!(!app
        .world()
        .get::<SkillBarBindings>(actor)
        .unwrap()
        .is_on_cooldown(1, 10));
}

#[test]
fn resolve_mouth_uses_target_position_as_cast_center() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);
    let target = spawn_actor(&mut app, Realm::Induce, 20.0);
    let target_pos = DVec3::new(10.0, 66.0, 8.0);
    app.world_mut()
        .get_mut::<Position>(target)
        .unwrap()
        .set(target_pos);

    let result =
        resolve_woliu_v2_skill(app.world_mut(), actor, 2, Some(target), WoliuSkillId::Mouth);

    assert!(matches!(result, CastResult::Started { .. }));
    let event = app
        .world()
        .resource::<Events<VortexCastEvent>>()
        .iter_current_update_events()
        .next()
        .unwrap();
    assert!(event.center.distance(target_pos) < f64::EPSILON);
}

#[test]
fn resolve_mouth_rejects_out_of_range_target_without_cooldown() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);
    let target = spawn_actor(&mut app, Realm::Induce, 20.0);
    app.world_mut()
        .get_mut::<Position>(target)
        .unwrap()
        .set([20.0, 66.0, 8.0]);

    let result =
        resolve_woliu_v2_skill(app.world_mut(), actor, 2, Some(target), WoliuSkillId::Mouth);

    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget
        }
    );
    assert!(!app
        .world()
        .get::<SkillBarBindings>(actor)
        .unwrap()
        .is_on_cooldown(2, 10));
}

#[test]
fn resolve_vacuum_palm_requires_target_before_spending_qi() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);

    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 1, None, WoliuSkillId::VacuumPalm);

    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget
        }
    );
    assert_eq!(
        app.world().get::<Cultivation>(actor).unwrap().qi_current,
        200.0
    );
    assert!(!app
        .world()
        .get::<SkillBarBindings>(actor)
        .unwrap()
        .is_on_cooldown(1, 10));
}

#[test]
fn resolve_vacuum_palm_pulls_target_and_siphons_qi() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);
    let target = spawn_actor(&mut app, Realm::Induce, 40.0);
    app.world_mut()
        .get_mut::<Position>(target)
        .unwrap()
        .set([11.0, 66.0, 8.0]);

    let result = resolve_woliu_v2_skill(
        app.world_mut(),
        actor,
        1,
        Some(target),
        WoliuSkillId::VacuumPalm,
    );

    assert!(matches!(result, CastResult::Started { .. }));
    assert_eq!(
        app.world().get::<Cultivation>(target).unwrap().qi_current,
        25.0
    );
    assert!(
        app.world().get::<Cultivation>(actor).unwrap().qi_current > 180.0,
        "caster should recover the siphoned target qi after paying the cast cost"
    );
    assert!(app
        .world()
        .resource::<Events<EntityDisplacedByVortexPull>>()
        .iter_current_update_events()
        .any(|event| event.target == target && event.displacement_blocks > 0.0));
    assert!(app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .any(
            |event| event.from == QiAccountId::player(format!("entity:{}", target.to_bits()))
                && event.to == QiAccountId::player(format!("entity:{}", actor.to_bits()))
                && (event.amount - 15.0).abs() < f64::EPSILON
        ));
}

#[test]
fn resolve_v3_area_skills_emit_distinct_visual_contracts() {
    let cases = [
        (
            WoliuSkillId::VortexShield,
            "bong:woliu_vortex_shield",
            "bong:woliu_vortex_shield_sphere",
            "woliu_vortex_shield",
        ),
        (
            WoliuSkillId::VortexResonance,
            "bong:woliu_vortex_resonance",
            "bong:woliu_vortex_resonance_field",
            "woliu_vortex_resonance",
        ),
        (
            WoliuSkillId::TurbulenceBurst,
            "bong:woliu_turbulence_burst",
            "bong:woliu_turbulence_burst_wave",
            "woliu_turbulence_burst",
        ),
    ];
    for (idx, (skill, animation, particle, sound)) in cases.into_iter().enumerate() {
        let mut app = app(10 + idx as u64);
        let actor = spawn_actor(&mut app, Realm::Condense, 500.0);

        let result = resolve_woliu_v2_skill(app.world_mut(), actor, idx as u8, None, skill);

        assert!(matches!(result, CastResult::Started { .. }));
        let event = app
            .world()
            .resource::<Events<VortexCastEvent>>()
            .iter_current_update_events()
            .next()
            .expect("v3 area skill should emit cast event");
        assert_eq!(event.skill, skill);
        assert_eq!(event.visual.animation_id, animation);
        assert_eq!(event.visual.particle_id, particle);
        assert_eq!(event.visual.sound_recipe_id, sound);
    }
}

#[test]
fn resolve_pull_emits_displacement_for_target() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);
    let target = spawn_actor(&mut app, Realm::Induce, 20.0);
    app.world_mut()
        .get_mut::<Position>(target)
        .unwrap()
        .set([13.0, 66.0, 8.0]);
    let result =
        resolve_woliu_v2_skill(app.world_mut(), actor, 1, Some(target), WoliuSkillId::Pull);
    assert!(matches!(result, CastResult::Started { .. }));
    let displacement = app
        .world()
        .resource::<Events<EntityDisplacedByVortexPull>>()
        .iter_current_update_events()
        .next()
        .unwrap();
    assert_eq!(displacement.target, target);
    assert!(displacement.displacement_blocks > 0.0);
    assert!(
        app.world()
            .get::<Position>(target)
            .unwrap()
            .get()
            .distance(DVec3::new(8.0, 66.0, 8.0))
            < f64::EPSILON
    );
}

#[test]
fn resolve_pull_skips_displacement_event_when_target_does_not_move() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);
    let target = spawn_actor(&mut app, Realm::Induce, 20.0);

    let result =
        resolve_woliu_v2_skill(app.world_mut(), actor, 1, Some(target), WoliuSkillId::Pull);

    assert!(matches!(result, CastResult::Started { .. }));
    assert_eq!(
        app.world()
            .resource::<Events<EntityDisplacedByVortexPull>>()
            .iter_current_update_events()
            .count(),
        0
    );
}

#[test]
fn resolve_heart_in_tsy_emits_severed_backfire() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Void, 1_000.0);
    app.world_mut()
        .entity_mut(actor)
        .insert(CurrentDimension(DimensionKind::Tsy));
    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 4, None, WoliuSkillId::Heart);
    assert!(matches!(result, CastResult::Started { .. }));
    let event = app
        .world()
        .resource::<Events<VortexBackfireEventV2>>()
        .iter_current_update_events()
        .next()
        .unwrap();
    assert_eq!(event.level, BackfireLevel::Severed);
    assert_eq!(event.cause, BackfireCauseV2::TsyNegativeField);
}

#[test]
fn void_heart_tribulation_waits_for_runtime_active_duration() {
    let started_at = 100;
    let mut app = app(started_at);
    app.add_systems(Update, super::tick::heart_active_backfire_tick);
    let actor = spawn_actor(&mut app, Realm::Void, 1_000.0);
    open_all_meridians(&mut app, actor, 10_000.0);

    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 4, None, WoliuSkillId::Heart);

    assert!(matches!(result, CastResult::Started { .. }));
    assert!(app
        .world()
        .get::<VortexV2State>(actor)
        .is_some_and(|state| state.backfire_level.is_none()));
    assert_eq!(
        app.world()
            .resource::<Events<VortexBackfireEventV2>>()
            .iter_current_update_events()
            .filter(|event| event.cause == BackfireCauseV2::VoidHeartTribulation)
            .count(),
        0
    );

    app.world_mut().resource_mut::<CombatClock>().tick = started_at + 30 * TICKS_PER_SECOND - 1;
    app.update();
    assert_eq!(
        app.world()
            .resource::<Events<VortexBackfireEventV2>>()
            .iter_current_update_events()
            .filter(|event| event.cause == BackfireCauseV2::VoidHeartTribulation)
            .count(),
        0
    );

    app.world_mut().resource_mut::<CombatClock>().tick = started_at + 30 * TICKS_PER_SECOND;
    app.update();
    let event = app
        .world()
        .resource::<Events<VortexBackfireEventV2>>()
        .iter_current_update_events()
        .find(|event| event.cause == BackfireCauseV2::VoidHeartTribulation)
        .expect("void heart should trigger tribulation after 30 active seconds");
    assert_eq!(event.level, BackfireLevel::Severed);
    let juebi = app
        .world()
        .resource::<Events<JueBiTriggerEvent>>()
        .iter_current_update_events()
        .next()
        .expect("void heart should enqueue JueBi trigger");
    assert_eq!(juebi.source, JueBiTriggerSource::WoliuVortexHeart);
}

#[test]
fn vortex_v2_lifecycle_removes_expired_state_and_passive_heart() {
    let mut app = app(10);
    app.add_systems(Update, super::tick::vortex_v2_state_lifecycle_tick);
    let actor = spawn_actor(&mut app, Realm::Void, 1_000.0);
    open_all_meridians(&mut app, actor, 10_000.0);

    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 4, None, WoliuSkillId::Heart);

    assert!(matches!(result, CastResult::Started { .. }));
    let active_until_tick = app
        .world()
        .get::<VortexV2State>(actor)
        .expect("heart cast should insert v2 state")
        .active_until_tick;
    assert!(app.world().get::<PassiveVortex>(actor).is_some());

    app.world_mut().resource_mut::<CombatClock>().tick = active_until_tick;
    app.update();

    assert!(app.world().get::<VortexV2State>(actor).is_none());
    assert!(app.world().get::<PassiveVortex>(actor).is_none());
}

#[test]
fn vortex_v2_lifecycle_preserves_cooldown_state_after_active_window() {
    let mut app = app(10);
    app.add_systems(Update, super::tick::vortex_v2_state_lifecycle_tick);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);

    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 1, None, WoliuSkillId::Burst);

    assert!(matches!(result, CastResult::Started { .. }));
    let state = *app
        .world()
        .get::<VortexV2State>(actor)
        .expect("burst cast should insert v2 state");
    assert!(state.cooldown_until_tick > state.active_until_tick);

    app.world_mut().resource_mut::<CombatClock>().tick = state.active_until_tick;
    app.update();
    assert!(
        app.world().get::<VortexV2State>(actor).is_some(),
        "v2 state should stay available while cooldown HUD still needs it"
    );

    app.world_mut().resource_mut::<CombatClock>().tick = state.cooldown_until_tick;
    app.update();
    assert!(app.world().get::<VortexV2State>(actor).is_none());
}

#[test]
fn cooldown_blocks_second_cast() {
    let mut app = app(10);
    let actor = spawn_actor(&mut app, Realm::Condense, 200.0);
    assert!(matches!(
        resolve_woliu_v2_skill(app.world_mut(), actor, 0, None, WoliuSkillId::Burst),
        CastResult::Started { .. }
    ));
    let result = resolve_woliu_v2_skill(app.world_mut(), actor, 0, None, WoliuSkillId::Burst);
    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::OnCooldown
        }
    );
}

#[test]
fn generated_spec_case_001() {
    assert_spec_case(1);
}
#[test]
fn generated_spec_case_002() {
    assert_spec_case(2);
}
#[test]
fn generated_spec_case_003() {
    assert_spec_case(3);
}
#[test]
fn generated_spec_case_004() {
    assert_spec_case(4);
}
#[test]
fn generated_spec_case_005() {
    assert_spec_case(5);
}
#[test]
fn generated_spec_case_006() {
    assert_spec_case(6);
}
#[test]
fn generated_spec_case_007() {
    assert_spec_case(7);
}
#[test]
fn generated_spec_case_008() {
    assert_spec_case(8);
}
#[test]
fn generated_spec_case_009() {
    assert_spec_case(9);
}
#[test]
fn generated_spec_case_010() {
    assert_spec_case(10);
}
#[test]
fn generated_spec_case_011() {
    assert_spec_case(11);
}
#[test]
fn generated_spec_case_012() {
    assert_spec_case(12);
}
#[test]
fn generated_spec_case_013() {
    assert_spec_case(13);
}
#[test]
fn generated_spec_case_014() {
    assert_spec_case(14);
}
#[test]
fn generated_spec_case_015() {
    assert_spec_case(15);
}
#[test]
fn generated_spec_case_016() {
    assert_spec_case(16);
}
#[test]
fn generated_spec_case_017() {
    assert_spec_case(17);
}
#[test]
fn generated_spec_case_018() {
    assert_spec_case(18);
}
#[test]
fn generated_spec_case_019() {
    assert_spec_case(19);
}
#[test]
fn generated_spec_case_020() {
    assert_spec_case(20);
}
#[test]
fn generated_spec_case_021() {
    assert_spec_case(21);
}
#[test]
fn generated_spec_case_022() {
    assert_spec_case(22);
}
#[test]
fn generated_spec_case_023() {
    assert_spec_case(23);
}
#[test]
fn generated_spec_case_024() {
    assert_spec_case(24);
}
#[test]
fn generated_spec_case_025() {
    assert_spec_case(25);
}
#[test]
fn generated_spec_case_026() {
    assert_spec_case(26);
}
#[test]
fn generated_spec_case_027() {
    assert_spec_case(27);
}
#[test]
fn generated_spec_case_028() {
    assert_spec_case(28);
}
#[test]
fn generated_spec_case_029() {
    assert_spec_case(29);
}
#[test]
fn generated_spec_case_030() {
    assert_spec_case(30);
}
#[test]
fn generated_spec_case_031() {
    assert_spec_case(31);
}
#[test]
fn generated_spec_case_032() {
    assert_spec_case(32);
}
#[test]
fn generated_spec_case_033() {
    assert_spec_case(33);
}
#[test]
fn generated_spec_case_034() {
    assert_spec_case(34);
}
#[test]
fn generated_spec_case_035() {
    assert_spec_case(35);
}
#[test]
fn generated_spec_case_036() {
    assert_spec_case(36);
}
#[test]
fn generated_spec_case_037() {
    assert_spec_case(37);
}
#[test]
fn generated_spec_case_038() {
    assert_spec_case(38);
}
#[test]
fn generated_spec_case_039() {
    assert_spec_case(39);
}
#[test]
fn generated_spec_case_040() {
    assert_spec_case(40);
}
#[test]
fn generated_spec_case_041() {
    assert_spec_case(41);
}
#[test]
fn generated_spec_case_042() {
    assert_spec_case(42);
}
#[test]
fn generated_spec_case_043() {
    assert_spec_case(43);
}
#[test]
fn generated_spec_case_044() {
    assert_spec_case(44);
}
#[test]
fn generated_spec_case_045() {
    assert_spec_case(45);
}
#[test]
fn generated_spec_case_046() {
    assert_spec_case(46);
}
#[test]
fn generated_spec_case_047() {
    assert_spec_case(47);
}
#[test]
fn generated_spec_case_048() {
    assert_spec_case(48);
}
#[test]
fn generated_spec_case_049() {
    assert_spec_case(49);
}
#[test]
fn generated_spec_case_050() {
    assert_spec_case(50);
}
#[test]
fn generated_spec_case_051() {
    assert_spec_case(51);
}
#[test]
fn generated_spec_case_052() {
    assert_spec_case(52);
}
#[test]
fn generated_spec_case_053() {
    assert_spec_case(53);
}
#[test]
fn generated_spec_case_054() {
    assert_spec_case(54);
}
#[test]
fn generated_spec_case_055() {
    assert_spec_case(55);
}
#[test]
fn generated_spec_case_056() {
    assert_spec_case(56);
}
#[test]
fn generated_spec_case_057() {
    assert_spec_case(57);
}
#[test]
fn generated_spec_case_058() {
    assert_spec_case(58);
}
#[test]
fn generated_spec_case_059() {
    assert_spec_case(59);
}
#[test]
fn generated_spec_case_060() {
    assert_spec_case(60);
}
#[test]
fn generated_spec_case_061() {
    assert_spec_case(61);
}
#[test]
fn generated_spec_case_062() {
    assert_spec_case(62);
}
#[test]
fn generated_spec_case_063() {
    assert_spec_case(63);
}
#[test]
fn generated_spec_case_064() {
    assert_spec_case(64);
}
#[test]
fn generated_spec_case_065() {
    assert_spec_case(65);
}
#[test]
fn generated_spec_case_066() {
    assert_spec_case(66);
}
#[test]
fn generated_spec_case_067() {
    assert_spec_case(67);
}
#[test]
fn generated_spec_case_068() {
    assert_spec_case(68);
}
#[test]
fn generated_spec_case_069() {
    assert_spec_case(69);
}
#[test]
fn generated_spec_case_070() {
    assert_spec_case(70);
}
#[test]
fn generated_spec_case_071() {
    assert_spec_case(71);
}
#[test]
fn generated_spec_case_072() {
    assert_spec_case(72);
}
#[test]
fn generated_spec_case_073() {
    assert_spec_case(73);
}
#[test]
fn generated_spec_case_074() {
    assert_spec_case(74);
}
#[test]
fn generated_spec_case_075() {
    assert_spec_case(75);
}
#[test]
fn generated_spec_case_076() {
    assert_spec_case(76);
}
#[test]
fn generated_spec_case_077() {
    assert_spec_case(77);
}
#[test]
fn generated_spec_case_078() {
    assert_spec_case(78);
}
#[test]
fn generated_spec_case_079() {
    assert_spec_case(79);
}
#[test]
fn generated_spec_case_080() {
    assert_spec_case(80);
}
#[test]
fn generated_spec_case_081() {
    assert_spec_case(81);
}
#[test]
fn generated_spec_case_082() {
    assert_spec_case(82);
}
#[test]
fn generated_spec_case_083() {
    assert_spec_case(83);
}
#[test]
fn generated_spec_case_084() {
    assert_spec_case(84);
}
#[test]
fn generated_spec_case_085() {
    assert_spec_case(85);
}
#[test]
fn generated_spec_case_086() {
    assert_spec_case(86);
}
#[test]
fn generated_spec_case_087() {
    assert_spec_case(87);
}
#[test]
fn generated_spec_case_088() {
    assert_spec_case(88);
}
#[test]
fn generated_spec_case_089() {
    assert_spec_case(89);
}
#[test]
fn generated_spec_case_090() {
    assert_spec_case(90);
}
#[test]
fn generated_spec_case_091() {
    assert_spec_case(91);
}
#[test]
fn generated_spec_case_092() {
    assert_spec_case(92);
}
#[test]
fn generated_spec_case_093() {
    assert_spec_case(93);
}
#[test]
fn generated_spec_case_094() {
    assert_spec_case(94);
}
#[test]
fn generated_spec_case_095() {
    assert_spec_case(95);
}
#[test]
fn generated_spec_case_096() {
    assert_spec_case(96);
}
#[test]
fn generated_spec_case_097() {
    assert_spec_case(97);
}
#[test]
fn generated_spec_case_098() {
    assert_spec_case(98);
}
#[test]
fn generated_spec_case_099() {
    assert_spec_case(99);
}
#[test]
fn generated_spec_case_100() {
    assert_spec_case(100);
}
