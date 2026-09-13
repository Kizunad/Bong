use super::*;
use crate::combat::components::ActiveStatusEffect;
use valence::prelude::{App, Update};

fn blood_burn_until(active_until_tick: u64) -> BloodBurnActive {
    BloodBurnActive {
        started_at_tick: 0,
        active_until_tick,
        hp_burned: 0.0,
        qi_multiplier: 1.0,
        cooldown_until_tick: 0,
    }
}

fn make_status_effects(effects: Vec<ActiveStatusEffect>) -> StatusEffects {
    StatusEffects { active: effects }
}

fn accel_effect(magnitude: f32, remaining_ticks: u64) -> ActiveStatusEffect {
    ActiveStatusEffect {
        kind: StatusEffectKind::CultivationAcceleration,
        magnitude,
        remaining_ticks,
        source_pill: None,
    }
}

fn slowed_effect(magnitude: f32, remaining_ticks: u64) -> ActiveStatusEffect {
    ActiveStatusEffect {
        kind: StatusEffectKind::QiRegenSlowed,
        magnitude,
        remaining_ticks,
        source_pill: None,
    }
}

fn boost_effect(magnitude: f32, remaining_ticks: u64) -> ActiveStatusEffect {
    ActiveStatusEffect {
        kind: StatusEffectKind::QiRegenBoost,
        magnitude,
        remaining_ticks,
        source_pill: None,
    }
}

#[test]
fn scar_qi_regen_multiplier_requires_active_blood_burn() {
    let attrs = DerivedAttrs {
        qi_regen_multiplier: 2.0,
        ..DerivedAttrs::default()
    };
    assert_eq!(
        baomai_scar_qi_regen_multiplier(Some(&attrs), None, 1),
        1.0,
        "无 BloodBurnActive 时心肺短路倍率不能生效"
    );
    assert_eq!(
        baomai_scar_qi_regen_multiplier(Some(&attrs), Some(&blood_burn_until(1)), 1),
        1.0,
        "BloodBurnActive 到期后心肺短路倍率必须清理为中性"
    );
    assert_eq!(
        baomai_scar_qi_regen_multiplier(Some(&attrs), Some(&blood_burn_until(10)), 1),
        2.0,
        "BloodBurnActive 有效期内才消费 DerivedAttrs.qi_regen_multiplier"
    );
}

/// 过期的虚脱 debuff（remaining_ticks=0）不应削减 qi 回复。
#[test]
fn expired_exhausted_does_not_halve_qi_recovery() {
    let se = StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::Exhausted,
            magnitude: 0.5,
            remaining_ticks: 0,
            source_pill: None,
        }],
    };
    assert_eq!(
        exhausted_qi_recovery_multiplier(&se),
        1.0,
        "过期 Exhausted(remaining=0) 不应计入 qi 回复乘数"
    );
}

#[test]
fn session_practice_accumulator_prunes_stale_entities() {
    let mut app = App::new();
    app.insert_resource(CultivationSessionPracticeAccumulator::default());
    app.add_systems(Update, prune_cultivation_session_practice_accumulator);

    let live_entity = app.world_mut().spawn(Cultivation::default()).id();
    let despawned_entity = app.world_mut().spawn(Despawned).id();
    let uncultivated_entity = app.world_mut().spawn_empty().id();
    let missing_entity = Entity::from_raw(99_999);

    {
        let mut accumulator = app
            .world_mut()
            .resource_mut::<CultivationSessionPracticeAccumulator>();
        accumulator.ticks_by_entity.insert(live_entity, 12);
        accumulator.ticks_by_entity.insert(despawned_entity, 24);
        accumulator.ticks_by_entity.insert(uncultivated_entity, 30);
        accumulator.ticks_by_entity.insert(missing_entity, 36);
        accumulator.last_gain_tick_by_entity.insert(live_entity, 12);
        accumulator
            .last_gain_tick_by_entity
            .insert(despawned_entity, 24);
        accumulator
            .last_gain_tick_by_entity
            .insert(uncultivated_entity, 30);
        accumulator
            .last_gain_tick_by_entity
            .insert(missing_entity, 36);
    }

    app.update();

    let accumulator = app
        .world()
        .resource::<CultivationSessionPracticeAccumulator>();
    assert_eq!(accumulator.ticks_by_entity.get(&live_entity), Some(&12));
    assert!(!accumulator.ticks_by_entity.contains_key(&despawned_entity));
    assert!(!accumulator
        .ticks_by_entity
        .contains_key(&uncultivated_entity));
    assert!(!accumulator.ticks_by_entity.contains_key(&missing_entity));
    assert_eq!(
        accumulator.last_gain_tick_by_entity.get(&live_entity),
        Some(&12)
    );
    assert!(!accumulator
        .last_gain_tick_by_entity
        .contains_key(&despawned_entity));
    assert!(!accumulator
        .last_gain_tick_by_entity
        .contains_key(&uncultivated_entity));
    assert!(!accumulator
        .last_gain_tick_by_entity
        .contains_key(&missing_entity));
}

#[test]
fn practice_accumulator_exposes_recent_actual_gain_for_audio() {
    let entity = Entity::from_raw(7);
    let mut accumulator = CultivationSessionPracticeAccumulator::default();

    assert!(!accumulator.is_recently_practicing(entity, 10));

    accumulator.note_practice_tick_for_tests(entity, 20);

    assert!(accumulator.is_recently_practicing(entity, 20));
    assert!(accumulator.is_recently_practicing(
        entity,
        20 + CultivationSessionPracticeAccumulator::AUDIO_RECENT_WINDOW_TICKS
    ));
    assert!(!accumulator.is_recently_practicing(
        entity,
        21 + CultivationSessionPracticeAccumulator::AUDIO_RECENT_WINDOW_TICKS
    ));
}

#[test]
fn cultivation_accel_mag_zero_returns_one() {
    let se = make_status_effects(vec![accel_effect(0.0, 100)]);
    let result = cultivation_acceleration_multiplier(&se);
    assert!(
        (result - 1.0).abs() < 1e-9,
        "mag=0 应返回 1.0×；实际 {result}"
    );
}

#[test]
fn cultivation_accel_mag_half_returns_one_point_five() {
    let se = make_status_effects(vec![accel_effect(0.5, 100)]);
    let result = cultivation_acceleration_multiplier(&se);
    assert!(
        (result - 1.5).abs() < 1e-9,
        "mag=0.5 应返回 1.5×；实际 {result}"
    );
}

#[test]
fn cultivation_accel_mag_one_returns_two() {
    let se = make_status_effects(vec![accel_effect(1.0, 100)]);
    let result = cultivation_acceleration_multiplier(&se);
    assert!(
        (result - 2.0).abs() < 1e-9,
        "mag=1.0 应返回 2.0×；实际 {result}"
    );
}

#[test]
fn cultivation_accel_mag_four_caps_at_five() {
    let se = make_status_effects(vec![accel_effect(4.0, 100)]);
    let result = cultivation_acceleration_multiplier(&se);
    assert!(
        (result - 5.0).abs() < 1e-9,
        "mag=4.0 应命中 cap 返回 5.0×；实际 {result}"
    );
}

#[test]
fn cultivation_accel_mag_ten_caps_at_five() {
    let se = make_status_effects(vec![accel_effect(10.0, 100)]);
    let result = cultivation_acceleration_multiplier(&se);
    assert!(
        (result - 5.0).abs() < 1e-9,
        "mag=10 超过 cap 应返回 5.0×；实际 {result}"
    );
}

#[test]
fn cultivation_accel_stacks_multiple_buffs() {
    let se = make_status_effects(vec![accel_effect(0.5, 100), accel_effect(1.0, 100)]);
    let result = cultivation_acceleration_multiplier(&se);
    // 1 + 0.5 + 1.0 = 2.5
    assert!(
        (result - 2.5).abs() < 1e-9,
        "mag=0.5 + mag=1.0 叠加应返回 2.5×；实际 {result}"
    );
}

#[test]
fn cultivation_accel_expired_ticks_not_counted() {
    let se = make_status_effects(vec![
        accel_effect(1.0, 100),
        accel_effect(2.0, 0), // expired
    ]);
    let result = cultivation_acceleration_multiplier(&se);
    assert!(
        (result - 2.0).abs() < 1e-9,
        "remaining_ticks=0 的 buff 不应计入；期望 2.0×，实际 {result}"
    );
}

#[test]
fn cultivation_accel_empty_effects_returns_one() {
    let se = make_status_effects(vec![]);
    let result = cultivation_acceleration_multiplier(&se);
    assert!(
        (result - 1.0).abs() < 1e-9,
        "空列表应返回 1.0×；实际 {result}"
    );
}

#[test]
fn cultivation_accel_ignores_other_kinds() {
    let se = make_status_effects(vec![ActiveStatusEffect {
        kind: StatusEffectKind::DamageAmp,
        magnitude: 3.0,
        remaining_ticks: 100,
        source_pill: None,
    }]);
    let result = cultivation_acceleration_multiplier(&se);
    assert!(
        (result - 1.0).abs() < 1e-9,
        "不同 kind 不应计入；实际 {result}"
    );
}

#[test]
fn qi_regen_slowed_mag_half_returns_half() {
    let se = make_status_effects(vec![slowed_effect(0.5, 100)]);
    let result = qi_regen_slowed_multiplier(&se);
    assert!(
        (result - 0.5).abs() < 1e-9,
        "mag=0.5 应返回 0.5×；实际 {result}"
    );
}

#[test]
fn qi_regen_slowed_mag_0_8_returns_0_2() {
    let se = make_status_effects(vec![slowed_effect(0.8, 100)]);
    let result = qi_regen_slowed_multiplier(&se);
    // f32(0.8) → f64 有微量精度损失，放宽到 1e-6
    assert!(
        (result - 0.2).abs() < 1e-6,
        "mag=0.8 应返回 ≈0.2×；实际 {result}"
    );
}

#[test]
fn qi_regen_slowed_mag_one_clamps_to_zero() {
    let se = make_status_effects(vec![slowed_effect(1.0, 100)]);
    let result = qi_regen_slowed_multiplier(&se);
    assert!(
        result.abs() < 1e-9,
        "mag=1.0 应 clamp 到 0.0×；实际 {result}"
    );
}

#[test]
fn qi_regen_slowed_mag_oversize_clamps_to_zero() {
    let se = make_status_effects(vec![slowed_effect(1.5, 100)]);
    let result = qi_regen_slowed_multiplier(&se);
    assert!(
        result.abs() < 1e-9,
        "mag=1.5 超出 1.0 应 clamp 到 0.0×（不能为负）；实际 {result}"
    );
}

#[test]
fn qi_regen_slowed_expired_not_counted() {
    let se = make_status_effects(vec![slowed_effect(0.5, 0)]);
    let result = qi_regen_slowed_multiplier(&se);
    assert!(
        (result - 1.0).abs() < 1e-9,
        "remaining_ticks=0 不应计入；期望 1.0×，实际 {result}"
    );
}

#[test]
fn qi_regen_boost_mag_zero_returns_one() {
    let se = make_status_effects(vec![boost_effect(0.0, 100)]);
    let result = qi_regen_boost_multiplier(&se);
    assert!(
        (result - 1.0).abs() < 1e-9,
        "QiRegenBoost mag=0 应返回 1.0×；实际 {result}"
    );
}

#[test]
fn qi_regen_boost_mag_quarter_returns_one_point_25() {
    let se = make_status_effects(vec![boost_effect(0.25, 100)]);
    let result = qi_regen_boost_multiplier(&se);
    assert!(
        (result - 1.25).abs() < 1e-6,
        "QiRegenBoost mag=0.25 应返回 1.25×；实际 {result}"
    );
}

#[test]
fn qi_regen_boost_mag_one_returns_two() {
    let se = make_status_effects(vec![boost_effect(1.0, 100)]);
    let result = qi_regen_boost_multiplier(&se);
    assert!(
        (result - 2.0).abs() < 1e-9,
        "QiRegenBoost mag=1.0 应返回 2.0×；实际 {result}"
    );
}

#[test]
fn qi_regen_boost_mag_two_caps_at_three() {
    let se = make_status_effects(vec![boost_effect(2.0, 100)]);
    let result = qi_regen_boost_multiplier(&se);
    assert!(
        (result - 3.0).abs() < 1e-9,
        "QiRegenBoost mag=2.0 应命中 cap 返回 3.0×；实际 {result}"
    );
}

#[test]
fn qi_regen_boost_mag_ten_caps_at_three() {
    let se = make_status_effects(vec![boost_effect(10.0, 100)]);
    let result = qi_regen_boost_multiplier(&se);
    assert!(
        (result - 3.0).abs() < 1e-9,
        "QiRegenBoost mag=10 超过 cap 应返回 3.0×；实际 {result}"
    );
}

#[test]
fn qi_regen_boost_stacks_multiple_buffs() {
    let se = make_status_effects(vec![boost_effect(0.10, 100), boost_effect(0.25, 100)]);
    let result = qi_regen_boost_multiplier(&se);
    // 1 + 0.10 + 0.25 = 1.35
    assert!(
        (result - 1.35).abs() < 1e-6,
        "QiRegenBoost 0.10 + 0.25 叠加应返回 1.35×；实际 {result}"
    );
}

#[test]
fn qi_regen_boost_expired_not_counted() {
    let se = make_status_effects(vec![boost_effect(0.25, 0)]);
    let result = qi_regen_boost_multiplier(&se);
    assert!(
        (result - 1.0).abs() < 1e-9,
        "remaining_ticks=0 的 QiRegenBoost 不应计入；期望 1.0×，实际 {result}"
    );
}

#[test]
fn qi_regen_boost_empty_effects_returns_one() {
    let se = make_status_effects(vec![]);
    let result = qi_regen_boost_multiplier(&se);
    assert!(
        (result - 1.0).abs() < 1e-9,
        "空列表应返回 1.0×；实际 {result}"
    );
}

#[test]
fn qi_regen_boost_ignores_other_kinds() {
    let se = make_status_effects(vec![ActiveStatusEffect {
        kind: StatusEffectKind::CultivationAcceleration,
        magnitude: 3.0,
        remaining_ticks: 100,
        source_pill: None,
    }]);
    let result = qi_regen_boost_multiplier(&se);
    assert!(
        (result - 1.0).abs() < 1e-9,
        "不同 kind 不应计入 QiRegenBoost；实际 {result}"
    );
}
