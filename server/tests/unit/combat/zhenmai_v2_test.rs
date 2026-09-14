#![allow(dead_code, unused_imports)]

use bong_server::combat::components::{SkillBarBindings, WoundKind, Wounds};
use bong_server::combat::events::{AttackSource, DefenseIntent};
use bong_server::combat::zhenmai_v2::*;
use bong_server::combat::CombatClock;
use bong_server::cultivation::color::PracticeLog;
use bong_server::cultivation::components::{
    ColorKind, Contamination, Cultivation, MeridianId, MeridianSystem, Realm,
};
use bong_server::cultivation::meridian::severed::{
    MeridianSeveredEvent, MeridianSeveredPermanent, SeveredSource, SkillMeridianDependencies,
};
use bong_server::network::audio_event_emit::PlaySoundRecipeRequest;
use bong_server::network::vfx_event_emit::VfxEventRequest;
use bong_server::qi_physics::ledger::{QiAccountId, QiTransferReason};
use bong_server::skill::events::SkillXpGain;
use valence::prelude::{App, Entity, Events, GameMode, Username};

fn all_realms() -> [Realm; 6] {
    [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ]
}

fn app_with_events() -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 100 });
    let mut dependencies = SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut dependencies);
    app.insert_resource(dependencies);
    app.add_event::<DefenseIntent>();
    app.add_event::<SkillXpGain>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_event::<LocalNeutralizeEvent>();
    app.add_event::<MeridianSeveredEvent>();
    app.add_event::<MeridianSeveredVoluntaryEvent>();
    app.add_event::<BackfireAmplificationActiveEvent>();
    app.add_event::<ZhenmaiSkillCastEvent>();
    app
}

fn caster(app: &mut App, realm: Realm, qi: f64) -> Entity {
    let mut meridians = MeridianSystem::default();
    for id in MeridianId::ALL {
        meridians.get_mut(id).opened = true;
    }
    app.world_mut()
        .spawn((
            Username("Azure".to_string()),
            Cultivation {
                realm,
                qi_current: qi,
                qi_max: qi.max(100.0),
                ..Default::default()
            },
            meridians,
            Wounds::default(),
            Contamination::default(),
            PracticeLog::default(),
            SkillBarBindings::default(),
            MeridianSeveredPermanent::default(),
        ))
        .id()
}

#[test]
fn declare_meridian_dependencies_registers_all_five_skills() {
    let mut dependencies = SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut dependencies);

    assert!(dependencies.is_declared(PARRY_SKILL_ID));
    assert!(dependencies.is_declared(NEUTRALIZE_SKILL_ID));
    assert!(dependencies.is_declared(MULTIPOINT_SKILL_ID));
    assert!(dependencies.is_declared(HARDEN_SKILL_ID));
    assert!(dependencies.is_declared(SEVER_CHAIN_SKILL_ID));
    assert_eq!(dependencies.lookup(PARRY_SKILL_ID), &[MeridianId::Lung]);
}

#[test]
fn parry_profile_awaken_matches_low_realm_cost() {
    let p = parry_profile(Realm::Awaken, 0);
    assert_eq!(p.k_drain, 0.05);
    assert_eq!(p.self_damage, 8.0);
}

#[test]
fn parry_profile_void_keeps_clamp_and_low_self_damage() {
    let p = parry_profile(Realm::Void, 100);
    assert_eq!(p.k_drain, 0.5);
    assert_eq!(p.self_damage, 3.0);
    assert_eq!(p.window_ms, 250);
}

#[test]
fn parry_window_scales_linearly() {
    assert_eq!(parry_window_ms(0), 100);
    assert_eq!(parry_window_ms(50), 175);
    assert_eq!(parry_window_ms(100), 250);
}

#[test]
fn parry_qi_cost_has_no_realm_gate() {
    for realm in all_realms() {
        assert_eq!(parry_qi_cost_for_realm(realm), Some(PARRY_QI_COST));
    }
}

#[test]
fn neutralize_profile_realm_table_matches_plan() {
    assert_eq!(
        neutralize_profile(Realm::Awaken, 0).qi_per_contam_percent,
        18.0
    );
    assert_eq!(neutralize_profile(Realm::Induce, 0).max_percent, 2.0);
    assert_eq!(neutralize_profile(Realm::Condense, 0).max_percent, 4.0);
    assert_eq!(
        neutralize_profile(Realm::Solidify, 0).qi_per_contam_percent,
        12.0
    );
    assert_eq!(neutralize_profile(Realm::Spirit, 0).max_percent, 10.0);
    assert_eq!(
        neutralize_profile(Realm::Void, 0).qi_per_contam_percent,
        8.0
    );
}

#[test]
fn multipoint_profile_realm_table_matches_plan() {
    assert_eq!(multipoint_profile(Realm::Awaken, 0).points, 3);
    assert_eq!(multipoint_profile(Realm::Induce, 0).points, 4);
    assert_eq!(multipoint_profile(Realm::Condense, 0).points, 5);
    assert_eq!(multipoint_profile(Realm::Solidify, 0).points, 6);
    assert_eq!(multipoint_profile(Realm::Spirit, 0).points, 7);
    assert_eq!(multipoint_profile(Realm::Void, 0).points, 8);
}

#[test]
fn harden_profile_void_allows_two_meridians() {
    let profile = harden_profile(Realm::Void, 0);
    assert_eq!(profile.max_meridians, 2);
    assert_eq!(profile.damage_multiplier, 0.20);
}

#[test]
fn sever_chain_only_spirit_and_void_gain_amplification() {
    assert!(!sever_chain_profile(Realm::Awaken).grants_amplification);
    assert!(sever_chain_profile(Realm::Spirit).grants_amplification);
    assert!(sever_chain_profile(Realm::Void).grants_amplification);
}

#[test]
fn sever_chain_void_breaks_normal_drain_clamp() {
    let profile = sever_chain_profile(Realm::Void);
    assert_eq!(profile.k_drain, 1.5);
    assert!(profile.k_drain > NORMAL_DRAIN_CLAMP);
}

#[test]
fn style_weight_matrix_matches_zhenmai_axis() {
    assert_eq!(style_weight(ZhenmaiAttackKind::RealYuan), 0.5);
    assert_eq!(style_weight(ZhenmaiAttackKind::PhysicalCarrier), 0.7);
    assert_eq!(style_weight(ZhenmaiAttackKind::Array), 0.2);
    assert_eq!(style_weight(ZhenmaiAttackKind::TaintedYuan), 0.0);
}

#[test]
fn tainted_yuan_reflection_is_zero_without_immunity() {
    assert_eq!(
        reflected_qi(100.0, 1.5, ZhenmaiAttackKind::TaintedYuan),
        0.0
    );
}

#[test]
fn reflected_qi_uses_beta_and_weight() {
    assert!((reflected_qi(100.0, 0.5, ZhenmaiAttackKind::PhysicalCarrier) - 21.0).abs() < 1e-6);
}

#[test]
fn backfire_transfer_uses_collision_reason() {
    let transfer = backfire_transfer(
        QiAccountId::player("attacker"),
        QiAccountId::player("defender"),
        12.0,
    )
    .unwrap();
    assert_eq!(transfer.amount, 12.0);
    assert_eq!(transfer.reason, QiTransferReason::Collision);
}

#[test]
fn attack_kind_maps_qi_needle_to_tainted_yuan() {
    assert_eq!(
        attack_kind_for_source(AttackSource::QiNeedle, WoundKind::Pierce),
        ZhenmaiAttackKind::TaintedYuan
    );
}

#[test]
fn attack_kind_maps_piercing_melee_to_physical_carrier() {
    assert_eq!(
        attack_kind_for_source(AttackSource::Melee, WoundKind::Pierce),
        ZhenmaiAttackKind::PhysicalCarrier
    );
}

#[test]
fn multipoint_contact_increments_count_and_reflects() {
    let mut active = MultiPointActive {
        started_at_tick: 1,
        expires_at_tick: 10,
        points: 5,
        k_drain: 0.2,
        qi_per_second: 1.0,
        contact_count: 0,
        self_damage_per_contact: 1.0,
    };
    let reflected = multipoint_contact(&mut active, 50.0, ZhenmaiAttackKind::RealYuan);
    assert_eq!(active.contact_count, 1);
    assert!((reflected - 3.0).abs() < 1e-6);
}

#[test]
fn amplification_active_requires_kind_and_tick() {
    let active = BackfireAmplification {
        meridian_id: MeridianId::Lung,
        attack_kind: ZhenmaiAttackKind::Array,
        started_at_tick: 10,
        expires_at_tick: 30,
        k_drain: 1.5,
        incoming_damage_multiplier: 0.5,
    };
    assert!(active.active_for(ZhenmaiAttackKind::Array, 29));
    assert!(!active.active_for(ZhenmaiAttackKind::Array, 30));
    assert!(!active.active_for(ZhenmaiAttackKind::RealYuan, 20));
}

#[test]
fn apply_reflected_qi_drains_attacker_pool() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Void, 100.0);
    let drained = apply_reflected_qi(app.world_mut(), entity, 30.0);
    assert_eq!(drained, 30.0);
    assert_eq!(
        app.world().get::<Cultivation>(entity).unwrap().qi_current,
        70.0
    );
}

#[test]
fn apply_reflected_qi_clamps_at_zero() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Void, 12.0);
    let drained = apply_reflected_qi(app.world_mut(), entity, 30.0);
    assert_eq!(drained, 12.0);
    assert_eq!(
        app.world().get::<Cultivation>(entity).unwrap().qi_current,
        0.0
    );
}

#[test]
fn apply_self_damage_clamps_health() {
    let mut wounds = Wounds {
        health_current: 5.0,
        ..Default::default()
    };
    let applied = apply_self_damage(&mut wounds, 8.0);
    assert_eq!(applied, 5.0);
    assert_eq!(wounds.health_current, 0.0);
}

#[test]
fn apply_self_damage_to_entity_skips_creative_mode() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Void, 100.0);
    app.world_mut().entity_mut(entity).insert((
        GameMode::Creative,
        Wounds {
            health_current: 12.0,
            ..Default::default()
        },
    ));

    let applied = apply_self_damage_to_entity(app.world_mut(), entity, 8.0);

    assert_eq!(applied, 0.0);
    assert_eq!(
        app.world().get::<Wounds>(entity).unwrap().health_current,
        12.0
    );
}
