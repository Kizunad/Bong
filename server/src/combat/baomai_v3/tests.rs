use crate::world::zone::ZoneRegistry;
use valence::prelude::{App, DVec3, Events, Position};

use super::events::{
    BaomaiSkillEvent, BaomaiSkillId, BloodBurnEvent, DispersedQiEvent, MountainShakeEvent,
    BAOMAI_BENG_QUAN_SKILL_ID, BAOMAI_DISPERSE_SKILL_ID, BAOMAI_FULL_POWER_CHARGE_SKILL_ID,
    BAOMAI_FULL_POWER_RELEASE_SKILL_ID,
};
use super::physics::{
    beng_quan_cooldown_ticks, blood_burn_profile, disperse_profile, mountain_shake_profile,
};
use super::skills::{
    cast_beng_quan, cast_blood_burn, cast_disperse, cast_full_power_charge,
    cast_full_power_release, cast_mountain_shake, declare_meridian_dependencies,
};
use super::state::{BaomaiMastery, BloodBurnActive, BodyTranscendence, MeridianRippleScar};
use crate::combat::components::{Lifecycle, SkillBarBindings, Wounds};
use crate::combat::events::{ApplyStatusEffectIntent, AttackIntent, FIST_REACH};
use crate::combat::CombatClock;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{
    ColorKind, Contamination, Cultivation, MeridianId, MeridianSystem, QiColor, Realm,
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

// plan-combat-skill-feedback-bridges-v1 P2 — disperse-failed pin tests
// 凡躯散功（!has_transcendence）必须 emit BaomaiSkillEvent，flow_rate_multiplier 等于
// disperse_profile 非超越档返回值（当前 physics = 1.0），agent else 分支用此值路由叙事。
#[test]
fn disperse_lower_realm_emits_skill_event_with_non_transcendence_flow_rate() {
    // Condense realm → has_transcendence=false → physics flow_rate_multiplier=1.0
    // 修复前：!has_transcendence 分支不 emit BaomaiSkillEvent，agent 收不到叙事。
    // 修复后：无条件 emit，flow_rate_multiplier 取 physics 返回值（1.0），
    //         命中 agent renderBaomaiV3Narration disperse else 分支（< 10）。
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    cast_disperse(app.world_mut(), caster, 0, None);

    let skill_events: Vec<_> = app
        .world()
        .resource::<Events<BaomaiSkillEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(
        skill_events.len(),
        1,
        "凡躯 cast_disperse 必须 emit 恰好 1 条 BaomaiSkillEvent，\
         供 agent baomai-v3-runtime 叙事路由；实际 emit {}",
        skill_events.len()
    );

    let evt = skill_events[0];
    assert_eq!(
        evt.skill,
        BaomaiSkillId::Disperse,
        "emit 的 skill 字段应为 Disperse"
    );

    let expected_flow = disperse_profile(Realm::Condense, 0).flow_rate_multiplier;
    assert_eq!(
        evt.flow_rate_multiplier, expected_flow,
        "flow_rate_multiplier 应等于 disperse_profile 非超越档返回值 ({expected_flow})；\
         值 < 10 命中 agent else 分支「强行散功，凡躯没有应声」；\
         值 >= 10 会走 transcendence 分支（回归错误），实际={:?}",
        evt.flow_rate_multiplier
    );
    // schema minimum:1 守护：flow_rate_multiplier 不能为 0（会被 agent validateBaomaiSkillEventV1Contract 拒绝）
    assert!(
        evt.flow_rate_multiplier >= 1.0,
        "flow_rate_multiplier={} 低于 schema minimum:1，agent 会 rejectedContract；\
         非超越档 physics 应返回 1.0",
        evt.flow_rate_multiplier
    );
}

#[test]
fn disperse_void_realm_still_emits_skill_event_with_transcendence_flow_rate() {
    // 回归测试：Void 档超越 emit 不受影响
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Void, 1_000.0, 1_000.0, DVec3::ZERO);
    cast_disperse(app.world_mut(), caster, 0, None);

    let skill_events: Vec<_> = app
        .world()
        .resource::<Events<BaomaiSkillEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(
        skill_events.len(),
        1,
        "Void 档 cast_disperse 也应 emit 恰好 1 条 BaomaiSkillEvent；实际 {}",
        skill_events.len()
    );

    let evt = skill_events[0];
    let expected_flow = disperse_profile(Realm::Void, 0).flow_rate_multiplier;
    assert!(
        evt.flow_rate_multiplier >= 10.0,
        "Void 超越档 flow_rate_multiplier={} 应 >= 10.0，命中 agent transcendence 分支；\
         期望 physics 返回值 {expected_flow}",
        evt.flow_rate_multiplier
    );
}

// P1: 杂色 guard ─ heavy_color_multiplier 在杂色时必须清零 -------------------

/// Heavy 色练习量足够（>= 30%）时，非杂色玩家的崩拳 damage 应享有 1.05x 加成。
/// 杂色玩家同等练习量下 damage 应等于无加成基础值（heavy multiplier=1.0）。
/// worldview §六.2「只剩基础真元属性」。
#[test]
fn beng_quan_heavy_color_multiplier_cleared_when_chaotic() {
    // 先测正常 Heavy 色（基准）
    let mut app_normal = app();
    let normal_caster = spawn_actor(&mut app_normal, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    let mut log = PracticeLog::default();
    log.add(ColorKind::Heavy, 3.0); // 75% Heavy => >= 30%
    log.add(ColorKind::Sharp, 1.0);
    app_normal
        .world_mut()
        .entity_mut(normal_caster)
        .insert(log.clone());
    let target_normal = spawn_target(&mut app_normal, DVec3::new(1.0, 0.0, 0.0));
    cast_beng_quan(
        app_normal.world_mut(),
        normal_caster,
        0,
        Some(target_normal),
    );
    let normal_events: Vec<_> = app_normal
        .world()
        .resource::<Events<BaomaiSkillEvent>>()
        .iter_current_update_events()
        .collect();
    let normal_damage = normal_events[0].damage;

    // 再测杂色（Heavy practice 相同但 is_chaotic=true）
    let mut app_chaotic = app();
    let chaotic_caster = spawn_actor(&mut app_chaotic, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    app_chaotic
        .world_mut()
        .entity_mut(chaotic_caster)
        .insert(log);
    app_chaotic
        .world_mut()
        .entity_mut(chaotic_caster)
        .insert(QiColor {
            main: ColorKind::Heavy,
            is_chaotic: true,
            ..Default::default()
        });
    let target_chaotic = spawn_target(&mut app_chaotic, DVec3::new(1.0, 0.0, 0.0));
    cast_beng_quan(
        app_chaotic.world_mut(),
        chaotic_caster,
        0,
        Some(target_chaotic),
    );
    let chaotic_events: Vec<_> = app_chaotic
        .world()
        .resource::<Events<BaomaiSkillEvent>>()
        .iter_current_update_events()
        .collect();
    let chaotic_damage = chaotic_events[0].damage;

    // 杂色伤害 < 正常色伤害（heavy 加成清零）
    assert!(
        chaotic_damage < normal_damage,
        "期望杂色时崩拳 damage={chaotic_damage} < 正常 Heavy 色 damage={normal_damage}（heavy_color_multiplier=1.0 vs 1.05），\
         杂色应清零 Heavy 专项加成（worldview §六.2）"
    );
}

/// 无 QiColor 组件时（旧实体）heavy_color_multiplier 正常返回 1.0，不 panic。
#[test]
fn beng_quan_heavy_multiplier_defaults_to_one_without_qi_color() {
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Condense, 100.0, 100.0, DVec3::ZERO);
    let mut log = PracticeLog::default();
    log.add(ColorKind::Heavy, 3.0);
    app.world_mut().entity_mut(caster).insert(log);
    // 故意不插入 QiColor 组件
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
    let result = cast_beng_quan(app.world_mut(), caster, 0, Some(target));
    assert!(
        matches!(result, CastResult::Started { .. }),
        "期望无 QiColor 时 cast_beng_quan 正常完成，实际={result:?}"
    );
}

// --- bao_mai ↔ baomai ID 门控 pin (regression: r2-P3 bughunt fix) ---

#[test]
fn full_power_charge_skill_id_constant_is_canonical_baomai() {
    // 期望 "baomai.full_power_charge"（无下划线），
    // 因为 SkillMeridianDependencies::declare 用此键注册门控；
    // 若常数改回 "bao_mai.*" 则 check_static_deps 查不到门控条目，
    // 经脉断裂时玩家仍可施放全力一击。
    assert_eq!(
        BAOMAI_FULL_POWER_CHARGE_SKILL_ID, "baomai.full_power_charge",
        "BAOMAI_FULL_POWER_CHARGE_SKILL_ID 应为 'baomai.full_power_charge'（无下划线）"
    );
}

#[test]
fn full_power_release_skill_id_constant_is_canonical_baomai() {
    assert_eq!(
        BAOMAI_FULL_POWER_RELEASE_SKILL_ID, "baomai.full_power_release",
        "BAOMAI_FULL_POWER_RELEASE_SKILL_ID 应为 'baomai.full_power_release'（无下划线）"
    );
}

// ── 真元守恒测试 ────────────────────────────────────────────────────────────────
//
// BUG-QP-01: baomai_v3 spend_qi 曾只发 QiTransfer 事件（Collision reason）但不更新
// zone.spirit_qi，导致真元从系统中蒸发。修复后，cast_beng_quan / cast_mountain_shake
// 必须将消耗真元通过 emit_spent_qi_release 归还给 zone 或 overflow。
//
// 守恒语义：player.qi_current 减少 amount → zone.spirit_qi 增加 accepted / overflow 事件
// 携带剩余量（二者合计 = amount）。

fn app_with_zone() -> App {
    let mut app = app();
    app.insert_resource(ZoneRegistry::fallback());
    app
}

/// 玩家在 spawn zone 内施放崩拳 → zone.spirit_qi 必须上升（真元归还区域）。
/// 修复前：record_qi_transfer 只发死亡事件，zone.spirit_qi 维持 0.9 不变。
/// 修复后：emit_spent_qi_release 直接写 zone.spirit_qi。
#[test]
fn beng_quan_in_zone_credits_zone_spirit_qi() {
    let mut app = app_with_zone();
    // spawn zone 的 y 范围是 [64, 80]；x/z 范围是 [-128, 128]。
    let caster = spawn_actor(
        &mut app,
        Realm::Condense,
        100.0,
        100.0,
        DVec3::new(0.0, 65.0, 0.0),
    );
    let target = spawn_target(&mut app, DVec3::new(1.0, 65.0, 0.0));
    let initial_spirit_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    cast_beng_quan(app.world_mut(), caster, 0, Some(target));

    let after_spirit_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    assert!(
        after_spirit_qi > initial_spirit_qi,
        "崩拳消耗真元应归还 spawn zone（spirit_qi 应上升）：\
         before={initial_spirit_qi}, after={after_spirit_qi}；\
         修复前 record_qi_transfer 只发事件不写 zone，spirit_qi 永远不变"
    );
}

/// 区域满载（spirit_qi 接近上限 1.0）时，超出部分走 overflow QiTransfer 事件。
/// zone_after 被 clamp(-1.0, 1.0)，所以即使大量注入也不超过 1.0。
#[test]
fn beng_quan_with_full_zone_routes_overflow_to_event() {
    let mut app = app_with_zone();
    // 预先把 zone 灵气撑满
    {
        let mut registry = app.world_mut().resource_mut::<ZoneRegistry>();
        registry.find_zone_mut("spawn").unwrap().spirit_qi = 1.0;
    }
    let caster = spawn_actor(
        &mut app,
        Realm::Condense,
        100.0,
        100.0,
        DVec3::new(0.0, 65.0, 0.0),
    );
    let target = spawn_target(&mut app, DVec3::new(1.0, 65.0, 0.0));

    cast_beng_quan(app.world_mut(), caster, 0, Some(target));

    // 满区域：spirit_qi 不变（没有 room），overflow 走 QiTransfer 事件
    let after_spirit_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    assert_eq!(
        after_spirit_qi, 1.0,
        "区域满载时 spirit_qi 应保持 1.0，实际={after_spirit_qi}"
    );
    // 应有至少一条 QiTransfer overflow 事件
    let transfer_count = app.world().resource::<Events<QiTransfer>>().len();
    assert!(
        transfer_count >= 1,
        "满载区域 beng_quan 应 emit overflow QiTransfer 事件，实际数量={transfer_count}"
    );
}

/// 玩家在 zone 外（无区域命中）施放崩拳 → 发 overflow QiTransfer 事件；qi_current 正确减少。
#[test]
fn beng_quan_outside_zone_emits_overflow_transfer_and_deducts_qi() {
    let mut app = app_with_zone();
    // 位于 zone 外（y < 64 不在 spawn 范围内）
    let caster = spawn_actor(
        &mut app,
        Realm::Condense,
        100.0,
        100.0,
        DVec3::new(0.0, 10.0, 0.0), // y=10 < zone min y=64
    );
    let target = spawn_target(&mut app, DVec3::new(1.0, 10.0, 0.0));

    cast_beng_quan(app.world_mut(), caster, 0, Some(target));

    // qi_current 应已减少（spend_qi 先于 emit_spent_qi_release 执行）
    let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    assert!(
        qi_after < 100.0,
        "cast 应先 spend_qi 扣除 qi_current；期望 < 100.0，实际={qi_after}"
    );
    // 无 zone 命中 → overflow 走 QiTransfer 事件
    let transfer_count = app.world().resource::<Events<QiTransfer>>().len();
    assert!(
        transfer_count >= 1,
        "zone 外崩拳应 emit overflow QiTransfer 事件，实际数量={transfer_count}"
    );
}

/// 玩家在 spawn zone 内施放山动崩霆 → zone.spirit_qi 应上升。
#[test]
fn mountain_shake_in_zone_credits_zone_spirit_qi() {
    let mut app = app_with_zone();
    let caster = spawn_actor(
        &mut app,
        Realm::Solidify,
        100.0,
        1_000.0,
        DVec3::new(0.0, 65.0, 0.0),
    );
    let initial_spirit_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    cast_mountain_shake(app.world_mut(), caster, 0, None);

    let after_spirit_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    assert!(
        after_spirit_qi > initial_spirit_qi,
        "山动崩霆消耗真元应归还 spawn zone（spirit_qi 应上升）：\
         before={initial_spirit_qi}, after={after_spirit_qi}"
    );
}

/// qi_current 与 spirit_qi 之间的守恒量化检验：
/// 崩拳消耗 base_cost（qi_max=100 时 = 40），zone 内全额接纳时
/// zone.spirit_qi 的上升量恰好对应 accepted/QI_ZONE_UNIT_CAPACITY。
#[test]
fn beng_quan_qi_conservation_amount_is_consistent() {
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;

    let mut app = app_with_zone();
    // 先将 zone 清空（spirit_qi=0）以便精确计算接纳量
    {
        let mut registry = app.world_mut().resource_mut::<ZoneRegistry>();
        registry.find_zone_mut("spawn").unwrap().spirit_qi = 0.0;
    }
    let caster = spawn_actor(
        &mut app,
        Realm::Condense,
        100.0,
        100.0,
        DVec3::new(0.0, 65.0, 0.0),
    );
    let target = spawn_target(&mut app, DVec3::new(1.0, 65.0, 0.0));

    let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    cast_beng_quan(app.world_mut(), caster, 0, Some(target));
    let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    let spent = qi_before - qi_after;

    let zone_spirit_qi_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    let zone_accepted = zone_spirit_qi_after * QI_ZONE_UNIT_CAPACITY;

    // accepted + overflow_emitted = spent（守恒）
    // 当 zone 有足够容量且 spent < capacity 时 overflow=0，accepted=spent。
    assert!(
        (zone_accepted - spent).abs() < 1e-9,
        "守恒断言失败：spent={spent}, zone_accepted={zone_accepted}（差 {}）；\
         baomai_v3 emit_spent_qi_release 应将全部消耗真元归还 zone",
        (zone_accepted - spent).abs()
    );
}

#[test]
fn full_power_meridian_deps_declared_under_canonical_id() {
    // 门控声明使用的是 BAOMAI_FULL_POWER_*_SKILL_ID（"baomai.*" 形式），
    // 若 ID 改回 "bao_mai.*" 则 declare 写入的键与 lookup 的键不同，
    // check_static_deps 永远找不到依赖项（静默通过）。
    let mut deps = SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut deps);
    let charge_deps = deps.lookup(BAOMAI_FULL_POWER_CHARGE_SKILL_ID);
    assert_eq!(
        charge_deps.len(),
        5,
        "期望 full_power_charge 有 5 个经脉依赖 (Ren/Du/LargeIntestine/SmallIntestine/TripleEnergizer)，\
         实际={len}——可能因 ID 失配导致 lookup 返回空",
        len = charge_deps.len()
    );
    let release_deps = deps.lookup(BAOMAI_FULL_POWER_RELEASE_SKILL_ID);
    assert_eq!(
        release_deps.len(),
        5,
        "期望 full_power_release 有 5 个经脉依赖，实际={len}——可能因 ID 失配导致 lookup 返回空",
        len = release_deps.len()
    );
}

#[test]
fn full_power_charge_rejected_when_ren_meridian_severed() {
    // 端到端门控验证：Ren 经脉断裂后 full_power_charge 应被拒绝。
    // 若 ID 失配（bao_mai. vs baomai.），check_static_deps 查不到门控，
    // 导致断脉玩家仍能施放——此测试在失配情况下会变绿（误报）。
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Induce, 400.0, 400.0, DVec3::ZERO);

    // 先插入正确的 SkillMeridianDependencies
    let mut meridian_deps = SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut meridian_deps);
    app.world_mut().insert_resource(meridian_deps);

    // 断 Ren 经（全力一击依赖之一）
    sever(&mut app, caster, MeridianId::Ren);

    let result = cast_full_power_charge(app.world_mut(), caster, 0, None);
    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(MeridianId::Ren))
        },
        "期望 Ren 断裂时 full_power_charge 返回 MeridianSevered(Ren)，\
         实际={result:?}——若为 Started 则说明 ID 失配导致门控旁路"
    );
}

#[test]
fn full_power_release_rejected_when_du_meridian_severed() {
    // 端到端门控验证：Du 经脉断裂后 full_power_release 应被拒绝。
    let mut app = app();
    let caster = spawn_actor(&mut app, Realm::Induce, 400.0, 400.0, DVec3::ZERO);

    let mut meridian_deps = SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut meridian_deps);
    app.world_mut().insert_resource(meridian_deps);

    sever(&mut app, caster, MeridianId::Du);

    let result = cast_full_power_release(app.world_mut(), caster, 0, None);
    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(MeridianId::Du))
        },
        "期望 Du 断裂时 full_power_release 返回 MeridianSevered(Du)，\
         实际={result:?}——若为其他 Reject 原因则说明门控命中但 charge 状态前置拦截先于经脉检查"
    );
}
