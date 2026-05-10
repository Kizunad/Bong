use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::dugu_v2::events::TaintTier;
use crate::combat::dugu_v2::physics::{
    defender_resistance, dirty_qi_collision, eclipse_effect, fake_qi_color_for_realm,
    penetrate_spec, reveal_probability, self_cure_gain_percent, shroud_spec,
};
use crate::cultivation::components::{ColorKind, Cultivation, Realm};
use crate::qi_physics::reverse_burst_all_marks;

fn assert_eclipse_tier(realm: Realm, tier: TaintTier) {
    assert_eq!(eclipse_effect(realm, 0.0).tier, tier);
}

fn assert_eclipse_hp(realm: Realm, hp_loss: f32) {
    assert!((eclipse_effect(realm, 0.0).hp_loss - hp_loss).abs() < f32::EPSILON);
}

fn assert_eclipse_qi(realm: Realm, qi_loss: f32) {
    assert!((eclipse_effect(realm, 0.0).qi_loss - qi_loss).abs() < f32::EPSILON);
}

fn assert_self_cure_gain(current: f32, hours: f32, today: f32, expected: f32) {
    let actual = self_cure_gain_percent(current, hours, today);
    assert!(
        (actual - expected).abs() < 1e-4,
        "self-cure gain mismatch for current={current}, hours={hours}, today={today}: {actual}"
    );
}

fn assert_shroud_strength(realm: Realm, expected: f32) {
    assert!((shroud_spec(realm).strength - expected).abs() < f32::EPSILON);
}

fn assert_penetrate_multiplier(realm: Realm, expected: f32) {
    assert!((penetrate_spec(realm).multiplier - expected).abs() < f32::EPSILON);
}

fn assert_reveal_decreases_with_distance(realm: Realm) {
    let near = reveal_probability(realm, 0.0, 3.0, Realm::Solidify);
    let mid = reveal_probability(realm, 0.0, 10.0, Realm::Solidify);
    let far = reveal_probability(realm, 0.0, 20.0, Realm::Solidify);
    assert!(near > mid && mid > far);
}

#[test]
fn eclipse_awaken_is_immediate() {
    assert_eclipse_tier(Realm::Awaken, TaintTier::Immediate);
}
#[test]
fn eclipse_induce_is_immediate() {
    assert_eclipse_tier(Realm::Induce, TaintTier::Immediate);
}
#[test]
fn eclipse_condense_is_immediate() {
    assert_eclipse_tier(Realm::Condense, TaintTier::Immediate);
}
#[test]
fn eclipse_solidify_is_temporary() {
    assert_eclipse_tier(Realm::Solidify, TaintTier::Temporary);
}
#[test]
fn eclipse_spirit_is_permanent() {
    assert_eclipse_tier(Realm::Spirit, TaintTier::Permanent);
}
#[test]
fn eclipse_void_is_permanent() {
    assert_eclipse_tier(Realm::Void, TaintTier::Permanent);
}
#[test]
fn eclipse_awaken_hp_loss_matches_table() {
    assert_eclipse_hp(Realm::Awaken, 2.0);
}
#[test]
fn eclipse_induce_hp_loss_matches_table() {
    assert_eclipse_hp(Realm::Induce, 5.0);
}
#[test]
fn eclipse_condense_hp_loss_matches_table() {
    assert_eclipse_hp(Realm::Condense, 10.0);
}
#[test]
fn eclipse_solidify_hp_loss_matches_table() {
    assert_eclipse_hp(Realm::Solidify, 15.0);
}
#[test]
fn eclipse_spirit_hp_loss_matches_table() {
    assert_eclipse_hp(Realm::Spirit, 20.0);
}
#[test]
fn eclipse_void_hp_loss_matches_table() {
    assert_eclipse_hp(Realm::Void, 40.0);
}
#[test]
fn eclipse_awaken_qi_loss_matches_table() {
    assert_eclipse_qi(Realm::Awaken, 3.0);
}
#[test]
fn eclipse_induce_qi_loss_matches_table() {
    assert_eclipse_qi(Realm::Induce, 8.0);
}
#[test]
fn eclipse_condense_qi_loss_matches_table() {
    assert_eclipse_qi(Realm::Condense, 15.0);
}
#[test]
fn eclipse_solidify_qi_loss_matches_table() {
    assert_eclipse_qi(Realm::Solidify, 25.0);
}
#[test]
fn eclipse_spirit_qi_loss_matches_table() {
    assert_eclipse_qi(Realm::Spirit, 40.0);
}
#[test]
fn eclipse_void_qi_loss_matches_table() {
    assert_eclipse_qi(Realm::Void, 100.0);
}
#[test]
fn eclipse_self_cure_0_percent_keeps_base() {
    assert_eclipse_hp(Realm::Spirit, 20.0);
}
#[test]
fn eclipse_self_cure_5_percent_scales_damage() {
    assert!((eclipse_effect(Realm::Spirit, 5.0).hp_loss - 22.0).abs() < 1e-6);
}
#[test]
fn eclipse_self_cure_10_percent_scales_damage() {
    assert!((eclipse_effect(Realm::Spirit, 10.0).hp_loss - 24.0).abs() < 1e-6);
}
#[test]
fn eclipse_self_cure_30_percent_scales_damage() {
    assert!((eclipse_effect(Realm::Spirit, 30.0).hp_loss - 32.0).abs() < 1e-6);
}
#[test]
fn eclipse_self_cure_60_percent_scales_damage() {
    assert!((eclipse_effect(Realm::Spirit, 60.0).hp_loss - 44.0).abs() < 1e-6);
}
#[test]
fn eclipse_self_cure_90_percent_scales_damage() {
    assert!((eclipse_effect(Realm::Spirit, 90.0).hp_loss - 56.0).abs() < 1e-6);
}
#[test]
fn eclipse_solidify_temporary_fraction_is_present() {
    assert!(eclipse_effect(Realm::Solidify, 0.0).temporary_qi_max_loss_fraction > 0.0);
}
#[test]
fn eclipse_spirit_has_permanent_decay() {
    assert!(eclipse_effect(Realm::Spirit, 0.0).permanent_decay_rate_per_min > 0.0);
}
#[test]
fn eclipse_void_decay_exceeds_spirit_decay() {
    assert!(
        eclipse_effect(Realm::Void, 0.0).permanent_decay_rate_per_min
            > eclipse_effect(Realm::Spirit, 0.0).permanent_decay_rate_per_min
    );
}

#[test]
fn self_cure_zero_current_one_hour_gain() {
    assert_self_cure_gain(0.0, 1.0, 0.0, 1.5);
}
#[test]
fn self_cure_zero_current_six_hour_gain() {
    assert_self_cure_gain(0.0, 6.0, 0.0, 9.0);
}
#[test]
fn self_cure_daily_cap_blocks_after_six_hours() {
    assert_self_cure_gain(0.0, 1.0, 6.0, 0.0);
}
#[test]
fn self_cure_daily_cap_uses_remaining_two_hours() {
    assert_self_cure_gain(0.0, 10.0, 4.0, 3.0);
}
#[test]
fn self_cure_ten_percent_has_diminishing_gain() {
    assert_self_cure_gain(10.0, 1.0, 0.0, 1.1852);
}
#[test]
fn self_cure_twenty_percent_has_diminishing_gain() {
    assert_self_cure_gain(20.0, 1.0, 0.0, 0.9074);
}
#[test]
fn self_cure_thirty_percent_has_diminishing_gain() {
    assert_self_cure_gain(30.0, 1.0, 0.0, 0.6667);
}
#[test]
fn self_cure_sixty_percent_has_diminishing_gain() {
    assert_self_cure_gain(60.0, 1.0, 0.0, 0.1667);
}
#[test]
fn self_cure_ninety_percent_reaches_zero_gain() {
    assert_self_cure_gain(90.0, 1.0, 0.0, 0.0);
}
#[test]
fn self_cure_nan_hours_are_rejected() {
    assert_self_cure_gain(0.0, f32::NAN, 0.0, 0.0);
}
#[test]
fn self_cure_nan_today_is_rejected() {
    assert_self_cure_gain(0.0, 1.0, f32::NAN, 0.0);
}
#[test]
fn self_cure_negative_today_treated_as_zero() {
    assert_self_cure_gain(0.0, 1.0, -5.0, 1.5);
}
#[test]
fn self_cure_negative_hours_clamp_to_zero() {
    assert_self_cure_gain(0.0, -1.0, 0.0, 0.0);
}
#[test]
fn self_cure_above_soft_cap_clamps_to_zero_gain() {
    assert_self_cure_gain(120.0, 1.0, 0.0, 0.0);
}
#[test]
fn self_cure_five_hours_from_thirty_percent_is_diminished() {
    assert_self_cure_gain(30.0, 5.0, 0.0, 3.3333);
}
#[test]
fn self_cure_cap_remaining_half_hour() {
    assert_self_cure_gain(0.0, 2.0, 5.5, 0.75);
}

#[test]
fn shroud_awaken_strength_matches_table() {
    assert_shroud_strength(Realm::Awaken, 0.20);
}
#[test]
fn shroud_induce_strength_matches_table() {
    assert_shroud_strength(Realm::Induce, 0.30);
}
#[test]
fn shroud_condense_strength_matches_table() {
    assert_shroud_strength(Realm::Condense, 0.50);
}
#[test]
fn shroud_solidify_strength_matches_table() {
    assert_shroud_strength(Realm::Solidify, 0.70);
}
#[test]
fn shroud_spirit_strength_matches_table() {
    assert_shroud_strength(Realm::Spirit, 0.85);
}
#[test]
fn shroud_void_strength_matches_table() {
    assert_shroud_strength(Realm::Void, 0.95);
}
#[test]
fn shroud_awaken_duration_is_one_minute() {
    assert_eq!(
        shroud_spec(Realm::Awaken).duration_ticks,
        60 * TICKS_PER_SECOND
    );
}
#[test]
fn shroud_induce_duration_is_three_minutes() {
    assert_eq!(
        shroud_spec(Realm::Induce).duration_ticks,
        3 * 60 * TICKS_PER_SECOND
    );
}
#[test]
fn shroud_condense_duration_is_five_minutes() {
    assert_eq!(
        shroud_spec(Realm::Condense).duration_ticks,
        5 * 60 * TICKS_PER_SECOND
    );
}
#[test]
fn shroud_solidify_duration_is_ten_minutes() {
    assert_eq!(
        shroud_spec(Realm::Solidify).duration_ticks,
        10 * 60 * TICKS_PER_SECOND
    );
}
#[test]
fn shroud_spirit_duration_is_thirty_minutes() {
    assert_eq!(
        shroud_spec(Realm::Spirit).duration_ticks,
        30 * 60 * TICKS_PER_SECOND
    );
}
#[test]
fn shroud_void_is_permanent_until_cancelled() {
    assert!(shroud_spec(Realm::Void).permanent_until_cancelled);
}
#[test]
fn fake_qi_awaken_keeps_mellow_main() {
    assert_eq!(
        fake_qi_color_for_realm(Realm::Awaken).main,
        ColorKind::Mellow
    );
}
#[test]
fn fake_qi_induce_uses_heavy() {
    assert_eq!(
        fake_qi_color_for_realm(Realm::Induce).main,
        ColorKind::Heavy
    );
}
#[test]
fn fake_qi_condense_uses_solid() {
    assert_eq!(
        fake_qi_color_for_realm(Realm::Condense).main,
        ColorKind::Solid
    );
}
#[test]
fn fake_qi_solidify_uses_sharp() {
    assert_eq!(
        fake_qi_color_for_realm(Realm::Solidify).main,
        ColorKind::Sharp
    );
}
#[test]
fn fake_qi_spirit_uses_dual_color() {
    assert_eq!(
        fake_qi_color_for_realm(Realm::Spirit).secondary,
        Some(ColorKind::Solid)
    );
}
#[test]
fn fake_qi_void_sets_hunyuan_flag() {
    assert!(fake_qi_color_for_realm(Realm::Void).is_hunyuan);
}

#[test]
fn penetrate_awaken_multiplier_matches_table() {
    assert_penetrate_multiplier(Realm::Awaken, 1.5);
}
#[test]
fn penetrate_induce_multiplier_matches_table() {
    assert_penetrate_multiplier(Realm::Induce, 1.8);
}
#[test]
fn penetrate_condense_multiplier_matches_table() {
    assert_penetrate_multiplier(Realm::Condense, 2.0);
}
#[test]
fn penetrate_solidify_multiplier_matches_table() {
    assert_penetrate_multiplier(Realm::Solidify, 2.5);
}
#[test]
fn penetrate_spirit_multiplier_matches_table() {
    assert_penetrate_multiplier(Realm::Spirit, 3.0);
}
#[test]
fn penetrate_void_multiplier_matches_table() {
    assert_penetrate_multiplier(Realm::Void, 5.0);
}
#[test]
fn penetrate_awaken_has_no_radius() {
    assert_eq!(penetrate_spec(Realm::Awaken).radius_blocks, 0.0);
}
#[test]
fn penetrate_induce_has_no_radius() {
    assert_eq!(penetrate_spec(Realm::Induce).radius_blocks, 0.0);
}
#[test]
fn penetrate_condense_has_no_radius() {
    assert_eq!(penetrate_spec(Realm::Condense).radius_blocks, 0.0);
}
#[test]
fn penetrate_solidify_has_no_radius() {
    assert_eq!(penetrate_spec(Realm::Solidify).radius_blocks, 0.0);
}
#[test]
fn penetrate_spirit_has_single_target_radius() {
    assert_eq!(penetrate_spec(Realm::Spirit).radius_blocks, 0.0);
}
#[test]
fn penetrate_void_has_zone_radius() {
    assert!(penetrate_spec(Realm::Void).radius_blocks.is_infinite());
}
#[test]
fn penetrate_awaken_has_no_extra_decay() {
    assert_eq!(
        penetrate_spec(Realm::Awaken).extra_permanent_decay_rate_per_min,
        0.0
    );
}
#[test]
fn penetrate_solidify_has_no_extra_decay() {
    assert_eq!(
        penetrate_spec(Realm::Solidify).extra_permanent_decay_rate_per_min,
        0.0
    );
}
#[test]
fn penetrate_spirit_adds_permanent_decay() {
    assert!(penetrate_spec(Realm::Spirit).extra_permanent_decay_rate_per_min > 0.0);
}
#[test]
fn penetrate_void_decay_exceeds_spirit() {
    assert!(
        penetrate_spec(Realm::Void).extra_permanent_decay_rate_per_min
            > penetrate_spec(Realm::Spirit).extra_permanent_decay_rate_per_min
    );
}

#[test]
fn reveal_awaken_decreases_with_distance() {
    assert_reveal_decreases_with_distance(Realm::Awaken);
}
#[test]
fn reveal_induce_decreases_with_distance() {
    assert_reveal_decreases_with_distance(Realm::Induce);
}
#[test]
fn reveal_condense_decreases_with_distance() {
    assert_reveal_decreases_with_distance(Realm::Condense);
}
#[test]
fn reveal_solidify_decreases_with_distance() {
    assert_reveal_decreases_with_distance(Realm::Solidify);
}
#[test]
fn reveal_spirit_decreases_with_distance() {
    assert_reveal_decreases_with_distance(Realm::Spirit);
}
#[test]
fn reveal_void_decreases_with_distance() {
    assert_reveal_decreases_with_distance(Realm::Void);
}
#[test]
fn reveal_shroud_lowers_awaken_probability() {
    assert!(
        reveal_probability(Realm::Awaken, 0.8, 3.0, Realm::Solidify)
            < reveal_probability(Realm::Awaken, 0.0, 3.0, Realm::Solidify)
    );
}
#[test]
fn reveal_shroud_lowers_induce_probability() {
    assert!(
        reveal_probability(Realm::Induce, 0.8, 3.0, Realm::Solidify)
            < reveal_probability(Realm::Induce, 0.0, 3.0, Realm::Solidify)
    );
}
#[test]
fn reveal_shroud_lowers_condense_probability() {
    assert!(
        reveal_probability(Realm::Condense, 0.8, 3.0, Realm::Solidify)
            < reveal_probability(Realm::Condense, 0.0, 3.0, Realm::Solidify)
    );
}
#[test]
fn reveal_shroud_lowers_solidify_probability() {
    assert!(
        reveal_probability(Realm::Solidify, 0.8, 3.0, Realm::Solidify)
            < reveal_probability(Realm::Solidify, 0.0, 3.0, Realm::Solidify)
    );
}
#[test]
fn reveal_shroud_lowers_spirit_probability() {
    assert!(
        reveal_probability(Realm::Spirit, 0.8, 3.0, Realm::Solidify)
            < reveal_probability(Realm::Spirit, 0.0, 3.0, Realm::Solidify)
    );
}
#[test]
fn reveal_shroud_lowers_void_probability() {
    assert!(
        reveal_probability(Realm::Void, 0.8, 3.0, Realm::Solidify)
            < reveal_probability(Realm::Void, 0.0, 3.0, Realm::Solidify)
    );
}
#[test]
fn reveal_solidify_victim_triples_awaken_risk() {
    assert!(
        reveal_probability(Realm::Awaken, 0.0, 3.0, Realm::Solidify)
            > reveal_probability(Realm::Awaken, 0.0, 3.0, Realm::Awaken)
    );
}
#[test]
fn reveal_spirit_victim_triples_spirit_risk() {
    assert!(
        reveal_probability(Realm::Spirit, 0.0, 3.0, Realm::Spirit)
            > reveal_probability(Realm::Spirit, 0.0, 3.0, Realm::Awaken)
    );
}
#[test]
fn reveal_void_base_is_lower_than_awaken() {
    assert!(
        reveal_probability(Realm::Void, 0.0, 3.0, Realm::Awaken)
            < reveal_probability(Realm::Awaken, 0.0, 3.0, Realm::Awaken)
    );
}
#[test]
fn reveal_strength_is_clamped() {
    assert_eq!(
        reveal_probability(Realm::Awaken, 2.0, 3.0, Realm::Awaken),
        reveal_probability(Realm::Awaken, 0.95, 3.0, Realm::Awaken)
    );
}
#[test]
fn reveal_far_distance_uses_lowest_bucket() {
    assert_eq!(
        reveal_probability(Realm::Awaken, 0.0, 16.0, Realm::Awaken),
        reveal_probability(Realm::Awaken, 0.0, 60.0, Realm::Awaken)
    );
}
#[test]
fn reveal_mid_distance_uses_middle_bucket() {
    assert_eq!(
        reveal_probability(Realm::Awaken, 0.0, 6.0, Realm::Awaken),
        reveal_probability(Realm::Awaken, 0.0, 15.0, Realm::Awaken)
    );
}

#[test]
fn dirty_qi_collision_keeps_injected_budget() {
    assert_eq!(dirty_qi_collision(12.0, 0.0, 1.0).injected_qi, 12.0);
}
#[test]
fn dirty_qi_collision_returns_ninety_nine_percent() {
    assert!((dirty_qi_collision(12.0, 0.0, 1.0).returned_zone_qi - 11.88).abs() < 1e-5);
}
#[test]
fn dirty_qi_collision_clamps_negative_injected_qi() {
    assert_eq!(dirty_qi_collision(-5.0, 0.0, 1.0).injected_qi, 0.0);
}
#[test]
fn dirty_qi_collision_resistance_reduces_hit() {
    assert!(
        dirty_qi_collision(20.0, 0.8, 1.0).effective_hit
            < dirty_qi_collision(20.0, 0.0, 1.0).effective_hit
    );
}
#[test]
fn dirty_qi_collision_distance_reduces_hit() {
    assert!(
        dirty_qi_collision(20.0, 0.0, 20.0).effective_hit
            < dirty_qi_collision(20.0, 0.0, 1.0).effective_hit
    );
}
#[test]
fn defender_resistance_awaken_lowest() {
    assert!(
        defender_resistance(&Cultivation {
            realm: Realm::Awaken,
            ..Default::default()
        }) < defender_resistance(&Cultivation {
            realm: Realm::Void,
            ..Default::default()
        })
    );
}
#[test]
fn defender_resistance_void_is_clamped_under_cap() {
    assert!(
        defender_resistance(&Cultivation {
            realm: Realm::Void,
            ..Default::default()
        }) <= 0.35
    );
}
#[test]
fn reverse_burst_empty_has_zero_count() {
    assert_eq!(reverse_burst_all_marks([]).mark_count, 0);
}
#[test]
fn reverse_burst_filters_non_finite_marks() {
    assert_eq!(
        reverse_burst_all_marks([f64::NAN, f64::INFINITY, 3.0]).mark_count,
        1
    );
}
#[test]
fn reverse_burst_filters_negative_marks() {
    assert_eq!(reverse_burst_all_marks([-1.0, 0.0, 2.0]).mark_count, 1);
}
#[test]
fn reverse_burst_damage_scales_with_intensity() {
    assert_eq!(reverse_burst_all_marks([2.0, 3.0]).burst_damage, 60.0);
}
#[test]
fn reverse_burst_zone_return_scales_with_intensity() {
    assert!((reverse_burst_all_marks([2.0, 3.0]).returned_zone_qi - 4.95).abs() < 1e-9);
}
#[test]
fn reverse_burst_counts_each_positive_mark() {
    assert_eq!(reverse_burst_all_marks([1.0, 1.0, 1.0]).mark_count, 3);
}
