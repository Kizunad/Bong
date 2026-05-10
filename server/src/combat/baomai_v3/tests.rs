use valence::prelude::{App, DVec3, Events, Position};

use super::events::{
    BaomaiSkillEvent, BaomaiSkillId, BloodBurnEvent, DispersedQiEvent, MountainShakeEvent,
    BAOMAI_BENG_QUAN_SKILL_ID, BAOMAI_DISPERSE_SKILL_ID,
};
use super::physics::{
    beng_quan_cooldown_ticks, blood_burn_profile, disperse_profile, mountain_shake_profile,
};
use super::skills::{
    cast_beng_quan, cast_blood_burn, cast_disperse, cast_full_power_charge, cast_mountain_shake,
    declare_meridian_dependencies,
};
use super::state::{BaomaiMastery, BloodBurnActive, BodyTranscendence, MeridianRippleScar};
use crate::combat::components::{Lifecycle, SkillBarBindings, Wounds};
use crate::combat::events::{ApplyStatusEffectIntent, AttackIntent, FIST_REACH};
use crate::combat::CombatClock;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{
    ColorKind, Contamination, Cultivation, MeridianId, MeridianSystem, Realm,
};
use crate::cultivation::meridian::severed::{
    MeridianSeveredEvent, MeridianSeveredPermanent, SeveredSource, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::tribulation::JueBiTriggerEvent;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::qi_physics::QiTransfer;
use crate::skill::events::SkillXpGain;

fn app() -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 100 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<SkillXpGain>();
    app.add_event::<crate::cultivation::full_power_strike::ChargeStartedEvent>();
    app.add_event::<MeridianSeveredEvent>();
    app.add_event::<JueBiTriggerEvent>();
    super::register(&mut app);
    app
}

fn spawn_actor(
    app: &mut App,
    realm: Realm,
    qi_current: f64,
    qi_max: f64,
    pos: DVec3,
) -> valence::prelude::Entity {
    app.world_mut()
        .spawn((
            Cultivation {
                realm,
                qi_current,
                qi_max,
                ..Default::default()
            },
            MeridianSystem::default(),
            SkillBarBindings::default(),
            Wounds::default(),
            Lifecycle::default(),
            PracticeLog::default(),
            BaomaiMastery::default(),
            Position::new([pos.x, pos.y, pos.z]),
        ))
        .id()
}

fn spawn_target(app: &mut App, pos: DVec3) -> valence::prelude::Entity {
    app.world_mut()
        .spawn(Position::new([pos.x, pos.y, pos.z]))
        .id()
}

fn sever(app: &mut App, entity: valence::prelude::Entity, id: MeridianId) {
    let mut permanent = MeridianSeveredPermanent::default();
    permanent.insert(id, SeveredSource::OverloadTear, 90);
    app.world_mut().entity_mut(entity).insert(permanent);
}

#[test]
fn register_skills_adds_all_baomai_v3_ids() {
    let mut registry = SkillRegistry::default();
    super::register_skills(&mut registry);
    for skill in BaomaiSkillId::ALL {
        assert!(
            registry.lookup(skill.as_str()).is_some(),
            "{} registered",
            skill.as_str()
        );
    }
}

#[test]
fn declare_dependencies_covers_disperse_all_20_meridians() {
    let mut deps = SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut deps);
    assert_eq!(deps.lookup(BAOMAI_DISPERSE_SKILL_ID).len(), 20);
    assert_eq!(deps.lookup(BAOMAI_BENG_QUAN_SKILL_ID).len(), 3);
}

#[test]
fn beng_quan_happy_path_spends_base_qi_and_emits_attack() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(f64::from(FIST_REACH.max), 0.0, 0.0));
    let result = cast_beng_quan(app.world_mut(), caster, 0, Some(target));
    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 60,
            anim_duration_ticks: 8
        }
    );
    assert_eq!(
        app.world().get::<Cultivation>(caster).unwrap().qi_current,
        60.0
    );
    assert_eq!(app.world().resource::<Events<AttackIntent>>().len(), 1);
    assert_eq!(app.world().resource::<Events<BaomaiSkillEvent>>().len(), 1);
}

#[test]
fn beng_quan_requires_target() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    assert_eq!(
        cast_beng_quan(app.world_mut(), caster, 0, None),
        CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget
        }
    );
}

#[test]
fn beng_quan_rejects_out_of_reach_target() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(100.0, 0.0, 0.0));
    assert_eq!(
        cast_beng_quan(app.world_mut(), caster, 0, Some(target)),
        CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget
        }
    );
}

#[test]
fn beng_quan_severed_hand_halves_damage_instead_of_full_reject() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    sever(&mut app, caster, MeridianId::LargeIntestine);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
    let result = cast_beng_quan(app.world_mut(), caster, 0, Some(target));
    assert!(matches!(result, CastResult::Started { .. }));
    let events = app.world().resource::<Events<BaomaiSkillEvent>>();
    let event = events.iter_current_update_events().next().unwrap();
    assert_eq!(event.damage, 30.0);
}

#[test]
fn beng_quan_cooldown_uses_mastery_level() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<BaomaiMastery>(caster)
        .unwrap()
        .set_level(BaomaiSkillId::BengQuan, 100.0);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
    assert_eq!(
        cast_beng_quan(app.world_mut(), caster, 0, Some(target)),
        CastResult::Started {
            cooldown_ticks: 10,
            anim_duration_ticks: 8
        }
    );
}

#[test]
fn full_power_charge_uses_baomai_mastery_rate_override() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 400.0, 400.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<BaomaiMastery>(caster)
        .unwrap()
        .set_level(BaomaiSkillId::FullPowerCharge, 100.0);

    let result = cast_full_power_charge(app.world_mut(), caster, 0, None);

    assert!(matches!(result, CastResult::Started { .. }));
    assert_eq!(
        app.world()
            .get::<crate::cultivation::full_power_strike::FullPowerChargeRateOverride>(caster)
            .unwrap()
            .rate_per_tick,
        200.0
    );
}

#[test]
fn beng_quan_records_ripple_scar_and_meridian_cracks() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
    cast_beng_quan(app.world_mut(), caster, 0, Some(target));
    assert!(
        app.world()
            .get::<MeridianRippleScar>(caster)
            .unwrap()
            .severity
            > 0.0
    );
    assert!(!app
        .world()
        .get::<MeridianSystem>(caster)
        .unwrap()
        .get(MeridianId::LargeIntestine)
        .cracks
        .is_empty());
}

#[test]
fn overload_sends_meridian_severed_only_on_first_drop_to_zero() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
    app.world_mut()
        .get_mut::<MeridianSystem>(caster)
        .unwrap()
        .get_mut(MeridianId::LargeIntestine)
        .integrity = 0.04;

    cast_beng_quan(app.world_mut(), caster, 0, Some(target));
    app.world_mut()
        .get_mut::<SkillBarBindings>(caster)
        .unwrap()
        .cooldown_until_tick = [0; SkillBarBindings::SLOT_COUNT];
    cast_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        app.world().resource::<Events<MeridianSeveredEvent>>().len(),
        1
    );
}

#[test]
fn mountain_shake_hits_targets_inside_radius_and_stuns() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Solidify, 100.0, 1_000.0, DVec3::ZERO);
    let inside = spawn_target(&mut app, DVec3::new(5.0, 0.0, 0.0));
    let outside = spawn_target(&mut app, DVec3::new(20.0, 0.0, 0.0));
    let result = cast_mountain_shake(app.world_mut(), caster, 0, None);
    assert!(matches!(result, CastResult::Started { .. }));
    let event = app
        .world()
        .resource::<Events<MountainShakeEvent>>()
        .iter_current_update_events()
        .next()
        .unwrap();
    assert!(event.affected.contains(&inside));
    assert!(!event.affected.contains(&outside));
    assert_eq!(
        app.world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .len(),
        1
    );
}

#[test]
fn mountain_shake_rejects_severed_foot_yang() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Solidify, 100.0, 1_000.0, DVec3::ZERO);
    sever(&mut app, caster, MeridianId::Stomach);
    assert_eq!(
        cast_mountain_shake(app.world_mut(), caster, 0, None),
        CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(MeridianId::Stomach))
        }
    );
}

#[test]
fn blood_burn_inserts_active_state_when_health_remains_above_ten_percent() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Induce, 100.0, 100.0, DVec3::ZERO);
    let result = cast_blood_burn(app.world_mut(), caster, 0, None);
    assert!(matches!(result, CastResult::Started { .. }));
    let active = app.world().get::<BloodBurnActive>(caster).unwrap();
    assert_eq!(active.hp_burned, 20.0);
    assert!(active.qi_multiplier >= 1.5);
    let skill_event = app
        .world()
        .resource::<Events<BaomaiSkillEvent>>()
        .iter_current_update_events()
        .next()
        .unwrap();
    assert_eq!(skill_event.skill, BaomaiSkillId::BloodBurn);
    assert_eq!(skill_event.blood_multiplier, active.qi_multiplier);
}

#[test]
fn blood_burn_rejects_when_hp_is_insufficient() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Void, 100.0, 1_000.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<Wounds>(caster)
        .unwrap()
        .health_current = 10.0;
    assert_eq!(
        cast_blood_burn(app.world_mut(), caster, 0, None),
        CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget
        }
    );
}

#[test]
fn blood_burn_near_death_does_not_keep_active_multiplier() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Induce, 100.0, 100.0, DVec3::ZERO);
    app.world_mut()
        .entity_mut(caster)
        .insert(Contamination::default());
    app.world_mut()
        .get_mut::<Wounds>(caster)
        .unwrap()
        .health_current = 21.0;
    cast_blood_burn(app.world_mut(), caster, 0, None);
    assert!(app.world().get::<BloodBurnActive>(caster).is_none());
    assert!(
        app.world()
            .resource::<Events<BloodBurnEvent>>()
            .iter_current_update_events()
            .next()
            .unwrap()
            .ended_in_near_death
    );
    let contamination = app.world().get::<Contamination>(caster).unwrap();
    assert_eq!(contamination.entries.len(), 1);
    assert_eq!(contamination.entries[0].color, ColorKind::Violent);
}

#[test]
fn blood_burn_tick_expires_and_adds_contamination() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Induce, 100.0, 100.0, DVec3::ZERO);
    app.world_mut()
        .entity_mut(caster)
        .insert(crate::cultivation::components::Contamination::default());
    cast_blood_burn(app.world_mut(), caster, 0, None);
    app.insert_resource(CombatClock { tick: 1_000 });
    app.update();
    assert!(app.world().get::<BloodBurnActive>(caster).is_none());
    assert!(!app
        .world()
        .get::<crate::cultivation::components::Contamination>(caster)
        .unwrap()
        .entries
        .is_empty());
}

#[test]
fn disperse_void_burns_qi_max_and_inserts_transcendence() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Void, 10_700.0, 10_700.0, DVec3::ZERO);
    let result = cast_disperse(app.world_mut(), caster, 0, None);
    assert!(matches!(result, CastResult::Started { .. }));
    assert_eq!(
        app.world().get::<Cultivation>(caster).unwrap().qi_max,
        5_350.0
    );
    assert_eq!(
        app.world()
            .get::<BodyTranscendence>(caster)
            .unwrap()
            .flow_rate_multiplier,
        10.0
    );
    let skill_event = app
        .world()
        .resource::<Events<BaomaiSkillEvent>>()
        .iter_current_update_events()
        .next()
        .unwrap();
    assert_eq!(skill_event.skill, BaomaiSkillId::Disperse);
    assert_eq!(skill_event.qi_invested, 5_350.0);
    assert_eq!(skill_event.flow_rate_multiplier, 10.0);
}

#[test]
fn disperse_spirit_uses_mortal_rejection_profile() {
    let profile = disperse_profile(Realm::Spirit, 0);
    assert_eq!(profile.qi_max_loss_ratio, 0.05);
    assert_eq!(profile.flow_rate_multiplier, 1.0);
    assert!(!profile.has_transcendence);
}

#[test]
fn disperse_lower_realm_only_punishes_pool() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    cast_disperse(app.world_mut(), caster, 0, None);
    assert_eq!(app.world().get::<Cultivation>(caster).unwrap().qi_max, 95.0);
    assert!(app.world().get::<BodyTranscendence>(caster).is_none());
}

#[test]
fn disperse_lower_realm_does_not_emit_transcendence_particle() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);

    cast_disperse(app.world_mut(), caster, 0, None);

    assert_eq!(app.world().resource::<Events<VfxEventRequest>>().len(), 0);
}

#[test]
fn disperse_fails_on_any_severed_meridian_without_burning_pool() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Void, 100.0, 100.0, DVec3::ZERO);
    sever(&mut app, caster, MeridianId::Lung);
    assert_eq!(
        cast_disperse(app.world_mut(), caster, 0, None),
        CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(MeridianId::Lung))
        }
    );
    assert_eq!(
        app.world().get::<Cultivation>(caster).unwrap().qi_max,
        100.0
    );
    assert_eq!(app.world().resource::<Events<DispersedQiEvent>>().len(), 0);
}

#[test]
fn body_transcendence_tick_restores_flow_rates() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Void, 100.0, 100.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<MeridianSystem>(caster)
        .unwrap()
        .get_mut(MeridianId::Lung)
        .flow_rate = 2.0;
    cast_disperse(app.world_mut(), caster, 0, None);
    assert_eq!(
        app.world()
            .get::<MeridianSystem>(caster)
            .unwrap()
            .get(MeridianId::Lung)
            .flow_rate,
        20.0
    );
    app.insert_resource(CombatClock { tick: 1_000 });
    app.update();
    assert!(app.world().get::<BodyTranscendence>(caster).is_none());
    assert_eq!(
        app.world()
            .get::<MeridianSystem>(caster)
            .unwrap()
            .get(MeridianId::Lung)
            .flow_rate,
        2.0
    );
}

#[test]
fn three_void_disperses_emit_juebi_trigger() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Void, 1_000.0, 1_000.0, DVec3::ZERO);
    for tick in [100, 200, 300] {
        app.insert_resource(CombatClock { tick });
        app.world_mut()
            .get_mut::<SkillBarBindings>(caster)
            .unwrap()
            .cooldown_until_tick = [0; 9];
        cast_disperse(app.world_mut(), caster, 0, None);
    }
    assert_eq!(app.world().resource::<Events<JueBiTriggerEvent>>().len(), 1);
}

#[test]
fn profile_tables_keep_plan_anchor_values() {
    assert_eq!(beng_quan_cooldown_ticks(100), 10);
    assert_eq!(mountain_shake_profile(Realm::Awaken, 0).radius_blocks, 3.0);
    assert_eq!(mountain_shake_profile(Realm::Void, 0).shock_damage, 850.0);
    assert_eq!(blood_burn_profile(Realm::Void, 0).hp_burn, 300.0);
}

#[test]
fn skill_wire_kinds_match_client_contract() {
    assert_eq!(BaomaiSkillId::MountainShake.wire_kind(), "mountain_shake");
    assert_eq!(BaomaiSkillId::BloodBurn.wire_kind(), "blood_burn");
    assert_eq!(BaomaiSkillId::Disperse.wire_kind(), "disperse");
}

#[test]
fn disperse_event_records_no_immunity_concept() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Void, 1_000.0, 1_000.0, DVec3::ZERO);
    cast_disperse(app.world_mut(), caster, 0, None);
    let event = app
        .world()
        .resource::<Events<DispersedQiEvent>>()
        .iter_current_update_events()
        .next()
        .unwrap();
    assert_eq!(event.flow_rate_multiplier, 10.0);
    assert!(event.failed_reason.is_none());
}
