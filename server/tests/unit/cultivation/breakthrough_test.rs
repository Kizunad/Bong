#![allow(dead_code, unused_imports)]

use bong_server::cultivation::breakthrough::*;
use bong_server::cultivation::components::{Cultivation, MeridianSystem, Realm};
use bong_server::cultivation::death_hooks::CultivationDeathTrigger;
use bong_server::cultivation::life_record::{BiographyEntry, LifeRecord};
use bong_server::cultivation::tick::CultivationClock;
use bong_server::network::gameplay_vfx;
use bong_server::network::vfx_event_emit::VfxEventRequest;
use bong_server::player::gameplay::PendingGameplayNarrations;
use bong_server::schema::common::NarrationStyle;
use bong_server::skill::events::{SkillCapChanged, SkillXpGain};
use bong_server::world::dimension::DimensionKind;
use bong_server::world::season::Season;
use bong_server::world::spirit_eye::SpiritEyeUsedForBreakthroughEvent;
use valence::prelude::{Entity, Position};


use bong_server::cultivation::components::MeridianId;
use bong_server::cultivation::overload;
use bong_server::npc::spawn::NpcMarker;
use bong_server::qi_physics::{QiAccountId, QiTransferReason, WorldQiAccount};
use bong_server::schema::common::NarrationScope;
use bong_server::schema::vfx_event::VfxEventPayloadV1;
use bong_server::world::karma::KarmaWeightStore;
use bong_server::world::zone::ZoneRegistry;
use valence::prelude::{App, Events, Update, Username};

struct FixedRoll(f64);
impl RollSource for FixedRoll {
    fn roll_unit(&mut self) -> f64 {
        self.0
    }
}

#[test]
fn qi_max_for_realm_matches_worldview_table_exactly() {
    // plan-npc-realm-distribution-v1 §8.1 #2 决议：qi_max_for_realm 的六个输出
    // 必须与 worldview §三:195-203 权威表逐一相等（10/40/150/540/2100/10700）。
    // 严禁与 combat_power.rs:61 test-only fixture（10/30/60/120/200/400）混淆
    // ——那是完全不同的一套非正典数值，本测试专门守住不能被悄悄换成那套。
    assert_eq!(qi_max_for_realm(Realm::Awaken), 10.0, "醒灵进入时 qi_max");
    assert_eq!(qi_max_for_realm(Realm::Induce), 40.0, "引气进入时 qi_max");
    assert_eq!(
        qi_max_for_realm(Realm::Condense),
        150.0,
        "凝脉进入时 qi_max"
    );
    assert_eq!(
        qi_max_for_realm(Realm::Solidify),
        540.0,
        "固元进入时 qi_max"
    );
    assert_eq!(qi_max_for_realm(Realm::Spirit), 2100.0, "通灵进入时 qi_max");
    assert_eq!(qi_max_for_realm(Realm::Void), 10700.0, "化虚进入时 qi_max");
}

#[test]
fn qi_max_for_realm_strictly_increasing_across_all_realm_transitions() {
    // 状态转换饱和覆盖：六境界依 rank 严格递增，不允许任何一档打平或倒退。
    let ordered = [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ];
    for pair in ordered.windows(2) {
        let (prev, next) = (pair[0], pair[1]);
        assert!(
            qi_max_for_realm(prev) < qi_max_for_realm(next),
            "{:?}({}) 必须严格小于 {:?}({})",
            prev,
            qi_max_for_realm(prev),
            next,
            qi_max_for_realm(next)
        );
    }
}

fn setup_for_induce() -> (Cultivation, MeridianSystem) {
    let mut c = Cultivation {
        qi_current: 100.0,
        qi_max: 100.0,
        composure: 1.0,
        realm: Realm::Awaken,
        ..Default::default()
    };
    c.realm = Realm::Awaken;
    let mut m = MeridianSystem::default();
    open_regular(&mut m, 3);
    (c, m)
}

fn open_regular(meridians: &mut MeridianSystem, count: usize) {
    for id in MeridianId::REGULAR.iter().take(count) {
        meridians.get_mut(*id).opened = true;
    }
}

fn open_extraordinary(meridians: &mut MeridianSystem, count: usize) {
    for id in MeridianId::EXTRAORDINARY.iter().take(count) {
        meridians.get_mut(*id).opened = true;
    }
}

fn open_all_meridians(meridians: &mut MeridianSystem) {
    for id in MeridianId::REGULAR
        .iter()
        .chain(MeridianId::EXTRAORDINARY.iter())
    {
        meridians.get_mut(*id).opened = true;
    }
}




#[test]
fn awaken_to_induce_always_succeeds_with_roll_zero() {
    let (mut c, mut m) = setup_for_induce();
    let out = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();
    assert_eq!(out.to, Realm::Induce);
    assert_eq!(c.realm, Realm::Induce);
}

#[test]
fn awaken_to_induce_fails_with_high_roll() {
    let (mut c, mut m) = setup_for_induce();
    // base 0.9 * integrity 1.0 * composure 1.0 * completeness 1.0 = 0.9 → roll 0.99 fails
    let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.99)).unwrap_err();
    assert!(matches!(err, BreakthroughError::RolledFailure { .. }));
    assert_eq!(c.realm, Realm::Awaken);
    // qi 已扣
    assert!(c.qi_current < 100.0);
}

#[test]
fn breakthrough_season_modifier_matches_four_phases() {
    assert_eq!(season_success_modifier(Season::Summer), 1.05);
    assert_eq!(season_success_modifier(Season::Winter), 0.95);
    assert_eq!(season_success_modifier(Season::SummerToWinter), 0.85);
    assert_eq!(season_success_modifier(Season::WinterToSummer), 0.85);
}

#[test]
fn breakthrough_in_xizhuan_phase_has_lower_success_rate() {
    let summer = compute_success_rate_with_env_and_season_bonus(
        Realm::Induce,
        1.0,
        1.0,
        1.0,
        0.0,
        0.0,
        Season::Summer,
    );
    let xizhuan = compute_success_rate_with_env_and_season_bonus(
        Realm::Induce,
        1.0,
        1.0,
        1.0,
        0.0,
        0.0,
        Season::SummerToWinter,
    );

    assert!(xizhuan < summer);
    assert!((xizhuan - 0.765).abs() < 1e-9);
}

#[test]
fn spirit_to_void_is_gated_by_tribulation() {
    let mut c = Cultivation {
        realm: Realm::Spirit,
        qi_current: 1000.0,
        qi_max: 1000.0,
        ..Default::default()
    };
    let mut m = MeridianSystem::default();
    for id in MeridianId::REGULAR
        .iter()
        .chain(MeridianId::EXTRAORDINARY.iter())
    {
        m.get_mut(*id).opened = true;
    }
    let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();
    assert_eq!(err, BreakthroughError::RequiresTribulation);
}




#[test]
fn material_bonus_capped_at_30_percent() {
    let r = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.0, 5.0);
    let r_cap = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.0, 0.30);
    assert!((r - r_cap).abs() < 1e-9);
}

#[test]
fn pending_material_bonus_accumulates_and_caps_at_30_percent() {
    let mut c = Cultivation::default();
    assert!((add_pending_material_bonus(&mut c, 0.12) - 0.12).abs() < 1e-9);
    assert!((add_pending_material_bonus(&mut c, 0.50) - 0.30).abs() < 1e-9);
    assert!((c.pending_material_bonus - 0.30).abs() < 1e-9);
}

#[test]
fn completeness_bounded() {
    // 超额很多不会无限放大
    let r = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.3, 0.0);
    assert!(r <= 1.0);
}

#[test]
fn void_breakthrough_returns_max_realm_error() {
    let mut c = Cultivation {
        realm: Realm::Void,
        ..Default::default()
    };
    let mut m = MeridianSystem::default();
    let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();
    assert_eq!(err, BreakthroughError::AtMaxRealm);
}

#[test]
fn pending_material_bonus_is_consumed_on_real_attempt() {
    let (mut c, mut m) = setup_for_induce();
    c.pending_material_bonus = 0.12;

    let out = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();

    let expected = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.0, 0.12);
    assert!((out.success_rate - expected).abs() < 1e-9);
    assert_eq!(c.pending_material_bonus, 0.0);
}

#[test]
fn pending_material_bonus_is_preserved_when_preconditions_fail() {
    let mut c = Cultivation {
        qi_current: 1.0,
        pending_material_bonus: 0.12,
        ..Default::default()
    };
    let mut m = MeridianSystem::default();
    open_regular(&mut m, 3);

    let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

    assert!(matches!(err, BreakthroughError::NotEnoughQi { .. }));
    assert!((c.pending_material_bonus - 0.12).abs() < 1e-9);
}

#[test]
fn induce_requires_three_regular_meridians_not_extraordinary_padding() {
    let mut c = Cultivation {
        realm: Realm::Awaken,
        qi_current: 100.0,
        qi_max: 100.0,
        composure: 1.0,
        ..Default::default()
    };
    let mut m = MeridianSystem::default();
    open_extraordinary(&mut m, 3);

    let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

    assert_eq!(
        err,
        BreakthroughError::NotEnoughRegularMeridians { need: 3, have: 0 }
    );
    assert_eq!(c.realm, Realm::Awaken);
    assert_eq!(c.qi_current, 100.0);
}

#[test]
fn solidify_requires_all_twelve_regular_meridians() {
    let mut c = Cultivation {
        realm: Realm::Condense,
        qi_current: 500.0,
        qi_max: 500.0,
        composure: 1.0,
        ..Default::default()
    };
    let mut m = MeridianSystem::default();
    open_regular(&mut m, 10);
    open_extraordinary(&mut m, 6);

    let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

    assert_eq!(
        err,
        BreakthroughError::NotEnoughRegularMeridians { need: 12, have: 10 }
    );
    assert_eq!(c.realm, Realm::Condense);
    assert_eq!(c.qi_current, 500.0);
}

#[test]
fn spirit_rejects_before_structure_when_total_meridians_are_too_few() {
    let mut c = Cultivation {
        realm: Realm::Solidify,
        qi_current: 1000.0,
        qi_max: 1000.0,
        composure: 1.0,
        ..Default::default()
    };
    let mut m = MeridianSystem::default();
    open_regular(&mut m, 12);
    open_extraordinary(&mut m, 3);

    let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

    assert_eq!(
        err,
        BreakthroughError::NotEnoughMeridians { need: 16, have: 15 }
    );
    assert_eq!(c.realm, Realm::Solidify);
    assert_eq!(c.qi_current, 1000.0);
}

#[test]
fn spirit_rejects_extraordinary_padding_without_regular_foundation() {
    let mut c = Cultivation {
        realm: Realm::Solidify,
        qi_current: 1000.0,
        qi_max: 1000.0,
        composure: 1.0,
        ..Default::default()
    };
    let mut m = MeridianSystem::default();
    open_regular(&mut m, 8);
    open_extraordinary(&mut m, 8);

    let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

    assert_eq!(
        err,
        BreakthroughError::NotEnoughRegularMeridians { need: 12, have: 8 }
    );
    assert_eq!(c.realm, Realm::Solidify);
    assert_eq!(c.qi_current, 1000.0);
}

#[test]
fn spirit_allows_twelve_regular_and_four_extraordinary_meridians() {
    let mut c = Cultivation {
        realm: Realm::Solidify,
        qi_current: 1000.0,
        qi_max: 1000.0,
        composure: 1.0,
        ..Default::default()
    };
    let mut m = MeridianSystem::default();
    open_regular(&mut m, 12);
    open_extraordinary(&mut m, 4);

    let out = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();

    assert_eq!(out.to, Realm::Spirit);
    assert_eq!(c.realm, Realm::Spirit);
}











#[test]
fn spirit_eye_bonus_raises_guyuan_success_rate() {
    let base = compute_success_rate_with_env_bonus(Realm::Solidify, 1.0, 1.0, 1.0, 0.0, 0.0);
    let boosted = compute_success_rate_with_env_bonus(
        Realm::Solidify,
        1.0,
        1.0,
        1.0,
        0.0,
        SPIRIT_EYE_BREAKTHROUGH_SUCCESS_BONUS,
    );

    assert!(boosted > base);
}


/// plan-skill-v1 §4 cap 表锚点：六境界分别对应 3/5/7/8/9/10。
#[test]
fn skill_cap_for_realm_matches_plan_section_four() {
    assert_eq!(skill_cap_for_realm(Realm::Awaken), 3);
    assert_eq!(skill_cap_for_realm(Realm::Induce), 5);
    assert_eq!(skill_cap_for_realm(Realm::Condense), 7);
    assert_eq!(skill_cap_for_realm(Realm::Solidify), 8);
    assert_eq!(skill_cap_for_realm(Realm::Spirit), 9);
    assert_eq!(skill_cap_for_realm(Realm::Void), 10);
}

fn setup_rapid_breakthrough_karma_app(now: u64) -> App {
    let mut app = App::new();
    app.insert_resource(CultivationClock { tick: now });
    app.insert_resource(KarmaWeightStore::default());
    app.insert_resource(ZoneRegistry::fallback());
    app.add_event::<BreakthroughOutcome>();
    app.add_systems(Update, rapid_breakthrough_karma_mark_system);
    app
}

fn breakthrough_success_outcome(entity: Entity) -> BreakthroughOutcome {
    BreakthroughOutcome {
        entity,
        from: Realm::Awaken,
        result: Ok(BreakthroughSuccess {
            to: Realm::Induce,
            success_rate: 1.0,
            used_qi: 0.0,
        }),
    }
}

#[test]
fn rapid_breakthrough_success_marks_hidden_karma_weight() {
    let now = RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS + 100;
    let mut app = setup_rapid_breakthrough_karma_app(now);
    let mut life = LifeRecord::new("offline:Azure");
    life.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Awaken,
        tick: now - 100,
    });
    life.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Induce,
        tick: now,
    });
    let entity = app
        .world_mut()
        .spawn((
            life,
            Username("Azure".to_string()),
            Position::new([8.8, 66.2, 8.1]),
        ))
        .id();

    app.world_mut()
        .send_event(breakthrough_success_outcome(entity));
    app.update();

    let weights = app.world().resource::<KarmaWeightStore>();
    let entry = weights
        .entry_for_player("Azure")
        .expect("rapid breakthroughs should mark hidden karma weight");
    assert_eq!(entry.weight, RAPID_BREAKTHROUGH_KARMA_WEIGHT_DELTA);
    assert_eq!(entry.zone.as_deref(), Some("spawn"));
    assert_eq!(entry.last_position, [8, 66, 8]);
    assert_eq!(entry.last_tick, now);
}

#[test]
fn old_breakthrough_success_outside_window_does_not_mark_karma() {
    let now = RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS + 100;
    let mut app = setup_rapid_breakthrough_karma_app(now);
    let mut life = LifeRecord::new("offline:Azure");
    life.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Awaken,
        tick: now - RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS - 1,
    });
    life.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Induce,
        tick: now,
    });
    let entity = app
        .world_mut()
        .spawn((
            life,
            Username("Azure".to_string()),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    app.world_mut()
        .send_event(breakthrough_success_outcome(entity));
    app.update();

    let weights = app.world().resource::<KarmaWeightStore>();
    assert!(weights.entry_for_player("Azure").is_none());
}

#[test]
fn failed_breakthrough_outcome_does_not_mark_karma() {
    let now = RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS + 100;
    let mut app = setup_rapid_breakthrough_karma_app(now);
    let mut life = LifeRecord::new("offline:Azure");
    life.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Awaken,
        tick: now - 100,
    });
    life.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Induce,
        tick: now,
    });
    let entity = app
        .world_mut()
        .spawn((
            life,
            Username("Azure".to_string()),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    app.world_mut().send_event(BreakthroughOutcome {
        entity,
        from: Realm::Awaken,
        result: Err(BreakthroughError::RolledFailure { severity: 0.2 }),
    });
    app.update();

    let weights = app.world().resource::<KarmaWeightStore>();
    assert!(weights.entry_for_player("Azure").is_none());
}


/// 连续多次失败，qi_max_frozen 不超过 qi_max * 0.5 的硬上限。
#[test]
fn repeated_breakthrough_failures_frozen_capped_at_half_qi_max() {
    let qi_max = 100.0;
    let mut c = Cultivation {
        realm: Realm::Awaken,
        qi_current: qi_max,
        qi_max,
        composure: 0.0, // composure=0 → success_rate 极低 → severity 接近 0.9
        ..Default::default()
    };
    let mut m = MeridianSystem::default();
    open_regular(&mut m, 3);

    // 失败 20 次：无 cap 时 severity≈0.9, freeze_add≈9.0, 2 次即超 qi_max=100 × 0.5=50
    for _ in 0..20 {
        // qi_current 须 ≥ breakthrough_qi_cost(Induce)=8.0，补满避免 NotEnoughQi 前置错误
        c.qi_current = qi_max;
        let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(1.0));
    }

    let frozen = c
        .qi_max_frozen
        .expect("qi_max_frozen should be Some after repeated failures");
    let cap = qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO;

    assert!(
        (frozen - cap).abs() < 1e-9,
        "期望重复失败后 qi_max_frozen 精确 clamp 到 cap={cap}（qi_max×0.5），实际 = {frozen}"
    );

    let effective_qi_max = c.qi_max - frozen;
    assert!(
        (effective_qi_max - qi_max * (1.0 - BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO)).abs() < 1e-9,
        "期望有效真元上限精确保留 qi_max×(1-cap_ratio)，实际 effective_qi_max = {effective_qi_max}"
    );
}



/// 成功突破不应修改 qi_max_frozen。
#[test]
fn successful_breakthrough_does_not_change_qi_max_frozen() {
    let (mut c, mut m) = setup_for_induce();
    c.qi_max_frozen = Some(5.0); // 预存冻结，验证成功路径不碰它

    let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();

    assert_eq!(
        c.qi_max_frozen,
        Some(5.0),
        "期望成功突破不修改 qi_max_frozen（仍为 5.0），实际 = {:?}",
        c.qi_max_frozen
    );
}
