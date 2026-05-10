use super::*;
use crate::cultivation::components::Realm;
use valence::prelude::Entity;

fn player() -> Entity {
    Entity::from_raw(1)
}

fn consume_once(pill: PoisonPillKind, digestion_current: f32) -> PoisonConsumeOutcome {
    let mut toxicity = PoisonToxicity::default();
    let mut digestion = DigestionLoad {
        current: digestion_current,
        ..DigestionLoad::default()
    };
    consume_poison_pill_now(
        player(),
        pill,
        Realm::Awaken,
        &mut toxicity,
        &mut digestion,
        100,
    )
}

fn toxicity_after(pill: PoisonPillKind, initial: f32) -> PoisonToxicity {
    let mut toxicity = PoisonToxicity {
        level: initial,
        ..PoisonToxicity::default()
    };
    let mut digestion = DigestionLoad::default();
    consume_poison_pill_now(
        player(),
        pill,
        Realm::Awaken,
        &mut toxicity,
        &mut digestion,
        100,
    );
    toxicity
}

fn digestion_after(pill: PoisonPillKind, initial: f32) -> DigestionLoad {
    let mut toxicity = PoisonToxicity::default();
    let mut digestion = DigestionLoad {
        current: initial,
        ..DigestionLoad::default()
    };
    consume_poison_pill_now(
        player(),
        pill,
        Realm::Awaken,
        &mut toxicity,
        &mut digestion,
        100,
    );
    digestion
}

#[test]
fn pill_wu_sui_amount_is_5() {
    assert_eq!(PoisonPillKind::WuSuiSanXin.spec().poison_amount, 5.0);
}
#[test]
fn pill_chi_tuo_amount_is_8() {
    assert_eq!(PoisonPillKind::ChiTuoZhiSui.spec().poison_amount, 8.0);
}
#[test]
fn pill_qing_lin_amount_is_10() {
    assert_eq!(PoisonPillKind::QingLinManTuo.spec().poison_amount, 10.0);
}
#[test]
fn pill_tie_fu_amount_is_12() {
    assert_eq!(PoisonPillKind::TieFuSheDan.spec().poison_amount, 12.0);
}
#[test]
fn pill_fu_xin_amount_is_15() {
    assert_eq!(PoisonPillKind::FuXinXuanGui.spec().poison_amount, 15.0);
}

#[test]
fn pill_wu_sui_digestion_is_20() {
    assert_eq!(PoisonPillKind::WuSuiSanXin.spec().digestion_load, 20.0);
}
#[test]
fn pill_chi_tuo_digestion_is_25() {
    assert_eq!(PoisonPillKind::ChiTuoZhiSui.spec().digestion_load, 25.0);
}
#[test]
fn pill_qing_lin_digestion_is_35() {
    assert_eq!(PoisonPillKind::QingLinManTuo.spec().digestion_load, 35.0);
}
#[test]
fn pill_tie_fu_digestion_is_45() {
    assert_eq!(PoisonPillKind::TieFuSheDan.spec().digestion_load, 45.0);
}
#[test]
fn pill_fu_xin_digestion_is_55() {
    assert_eq!(PoisonPillKind::FuXinXuanGui.spec().digestion_load, 55.0);
}

#[test]
fn pill_wu_sui_side_effect_tag() {
    assert_eq!(
        PoisonPillKind::WuSuiSanXin.spec().side_effect_tag,
        PoisonSideEffectTag::QiFocusDrift2h
    );
}
#[test]
fn pill_chi_tuo_side_effect_tag() {
    assert_eq!(
        PoisonPillKind::ChiTuoZhiSui.spec().side_effect_tag,
        PoisonSideEffectTag::RageBurst30m
    );
}
#[test]
fn pill_qing_lin_side_effect_tag() {
    assert_eq!(
        PoisonPillKind::QingLinManTuo.spec().side_effect_tag,
        PoisonSideEffectTag::HallucinTint6h
    );
}
#[test]
fn pill_tie_fu_side_effect_tag() {
    assert_eq!(
        PoisonPillKind::TieFuSheDan.spec().side_effect_tag,
        PoisonSideEffectTag::DigestLock6h
    );
}
#[test]
fn pill_fu_xin_side_effect_tag() {
    assert_eq!(
        PoisonPillKind::FuXinXuanGui.spec().side_effect_tag,
        PoisonSideEffectTag::ToxicityTierUnlock
    );
}

#[test]
fn pill_wu_sui_item_roundtrip() {
    assert_eq!(
        PoisonPillKind::from_item_id("poison_pill_wu_sui_san_xin"),
        Some(PoisonPillKind::WuSuiSanXin)
    );
}
#[test]
fn pill_chi_tuo_item_roundtrip() {
    assert_eq!(
        PoisonPillKind::from_item_id("poison_pill_chi_tuo_zhi_sui"),
        Some(PoisonPillKind::ChiTuoZhiSui)
    );
}
#[test]
fn pill_qing_lin_item_roundtrip() {
    assert_eq!(
        PoisonPillKind::from_item_id("poison_pill_qing_lin_man_tuo"),
        Some(PoisonPillKind::QingLinManTuo)
    );
}
#[test]
fn pill_tie_fu_item_roundtrip() {
    assert_eq!(
        PoisonPillKind::from_item_id("poison_pill_tie_fu_she_dan"),
        Some(PoisonPillKind::TieFuSheDan)
    );
}
#[test]
fn pill_fu_xin_item_roundtrip() {
    assert_eq!(
        PoisonPillKind::from_item_id("poison_pill_fu_xin_xuan_gui"),
        Some(PoisonPillKind::FuXinXuanGui)
    );
}

#[test]
fn powder_wu_sui_damage() {
    assert_eq!(PoisonPowderKind::WuSuiSanXin.spec().damage_per_second, 2.0);
}
#[test]
fn powder_chi_tuo_damage() {
    assert_eq!(PoisonPowderKind::ChiTuoZhiSui.spec().damage_per_second, 2.0);
}
#[test]
fn powder_qing_lin_damage() {
    assert_eq!(
        PoisonPowderKind::QingLinManTuo.spec().damage_per_second,
        4.0
    );
}
#[test]
fn powder_tie_fu_damage() {
    assert_eq!(PoisonPowderKind::TieFuSheDan.spec().damage_per_second, 5.0);
}
#[test]
fn powder_fu_xin_damage() {
    assert_eq!(PoisonPowderKind::FuXinXuanGui.spec().damage_per_second, 8.0);
}

#[test]
fn powder_wu_sui_duration() {
    assert_eq!(PoisonPowderKind::WuSuiSanXin.spec().duration_seconds, 3);
}
#[test]
fn powder_chi_tuo_duration() {
    assert_eq!(PoisonPowderKind::ChiTuoZhiSui.spec().duration_seconds, 5);
}
#[test]
fn powder_qing_lin_duration() {
    assert_eq!(PoisonPowderKind::QingLinManTuo.spec().duration_seconds, 5);
}
#[test]
fn powder_tie_fu_duration() {
    assert_eq!(PoisonPowderKind::TieFuSheDan.spec().duration_seconds, 6);
}
#[test]
fn powder_fu_xin_duration() {
    assert_eq!(PoisonPowderKind::FuXinXuanGui.spec().duration_seconds, 8);
}

#[test]
fn powder_wu_sui_source_pill() {
    assert_eq!(
        PoisonPowderKind::WuSuiSanXin.source_pill(),
        PoisonPillKind::WuSuiSanXin
    );
}
#[test]
fn powder_chi_tuo_source_pill() {
    assert_eq!(
        PoisonPowderKind::ChiTuoZhiSui.source_pill(),
        PoisonPillKind::ChiTuoZhiSui
    );
}
#[test]
fn powder_qing_lin_source_pill() {
    assert_eq!(
        PoisonPowderKind::QingLinManTuo.source_pill(),
        PoisonPillKind::QingLinManTuo
    );
}
#[test]
fn powder_tie_fu_source_pill() {
    assert_eq!(
        PoisonPowderKind::TieFuSheDan.source_pill(),
        PoisonPillKind::TieFuSheDan
    );
}
#[test]
fn powder_fu_xin_source_pill() {
    assert_eq!(
        PoisonPowderKind::FuXinXuanGui.source_pill(),
        PoisonPillKind::FuXinXuanGui
    );
}

#[test]
fn consume_wu_sui_emits_dose() {
    assert_eq!(
        consume_once(PoisonPillKind::WuSuiSanXin, 0.0)
            .dose_event
            .dose_amount,
        5.0
    );
}
#[test]
fn consume_chi_tuo_emits_dose() {
    assert_eq!(
        consume_once(PoisonPillKind::ChiTuoZhiSui, 0.0)
            .dose_event
            .dose_amount,
        8.0
    );
}
#[test]
fn consume_qing_lin_emits_dose() {
    assert_eq!(
        consume_once(PoisonPillKind::QingLinManTuo, 0.0)
            .dose_event
            .dose_amount,
        10.0
    );
}
#[test]
fn consume_tie_fu_emits_dose() {
    assert_eq!(
        consume_once(PoisonPillKind::TieFuSheDan, 0.0)
            .dose_event
            .dose_amount,
        12.0
    );
}
#[test]
fn consume_fu_xin_emits_dose() {
    assert_eq!(
        consume_once(PoisonPillKind::FuXinXuanGui, 0.0)
            .dose_event
            .dose_amount,
        15.0
    );
}

#[test]
fn consume_wu_sui_updates_level() {
    assert_eq!(
        toxicity_after(PoisonPillKind::WuSuiSanXin, 10.0).level,
        15.0
    );
}
#[test]
fn consume_chi_tuo_updates_level() {
    assert_eq!(
        toxicity_after(PoisonPillKind::ChiTuoZhiSui, 10.0).level,
        18.0
    );
}
#[test]
fn consume_qing_lin_updates_level() {
    assert_eq!(
        toxicity_after(PoisonPillKind::QingLinManTuo, 10.0).level,
        20.0
    );
}
#[test]
fn consume_tie_fu_updates_level() {
    assert_eq!(
        toxicity_after(PoisonPillKind::TieFuSheDan, 10.0).level,
        22.0
    );
}
#[test]
fn consume_fu_xin_updates_level() {
    assert_eq!(
        toxicity_after(PoisonPillKind::FuXinXuanGui, 10.0).level,
        25.0
    );
}

#[test]
fn consume_clamps_toxicity_at_100_wu() {
    assert_eq!(
        toxicity_after(PoisonPillKind::WuSuiSanXin, 99.0).level,
        100.0
    );
}
#[test]
fn consume_clamps_toxicity_at_100_chi() {
    assert_eq!(
        toxicity_after(PoisonPillKind::ChiTuoZhiSui, 99.0).level,
        100.0
    );
}
#[test]
fn consume_clamps_toxicity_at_100_qing() {
    assert_eq!(
        toxicity_after(PoisonPillKind::QingLinManTuo, 99.0).level,
        100.0
    );
}
#[test]
fn consume_clamps_toxicity_at_100_tie() {
    assert_eq!(
        toxicity_after(PoisonPillKind::TieFuSheDan, 99.0).level,
        100.0
    );
}
#[test]
fn consume_clamps_toxicity_at_100_fu() {
    assert_eq!(
        toxicity_after(PoisonPillKind::FuXinXuanGui, 99.0).level,
        100.0
    );
}

#[test]
fn consume_wu_sui_updates_digestion() {
    assert_eq!(
        digestion_after(PoisonPillKind::WuSuiSanXin, 10.0).current,
        30.0
    );
}
#[test]
fn consume_chi_tuo_updates_digestion() {
    assert_eq!(
        digestion_after(PoisonPillKind::ChiTuoZhiSui, 10.0).current,
        35.0
    );
}
#[test]
fn consume_qing_lin_updates_digestion() {
    assert_eq!(
        digestion_after(PoisonPillKind::QingLinManTuo, 10.0).current,
        45.0
    );
}
#[test]
fn consume_tie_fu_updates_digestion() {
    assert_eq!(
        digestion_after(PoisonPillKind::TieFuSheDan, 10.0).current,
        55.0
    );
}
#[test]
fn consume_fu_xin_updates_digestion() {
    assert_eq!(
        digestion_after(PoisonPillKind::FuXinXuanGui, 10.0).current,
        65.0
    );
}

#[test]
fn overdose_none_when_under_capacity() {
    assert!(consume_once(PoisonPillKind::WuSuiSanXin, 0.0)
        .overdose_event
        .is_none());
}
#[test]
fn consume_chi_tuo_reports_base_lifespan_cost_without_overdose() {
    let outcome = consume_once(PoisonPillKind::ChiTuoZhiSui, 0.0);
    assert_eq!(outcome.base_lifespan_cost_years, 1.0);
    assert!(outcome.overdose_event.is_none());
}
#[test]
fn overdose_mild_when_small_overflow() {
    assert_eq!(
        calculate_overdose_severity(10.0, 100.0),
        Some(PoisonOverdoseSeverity::Mild)
    );
}
#[test]
fn overdose_moderate_when_mid_overflow() {
    assert_eq!(
        calculate_overdose_severity(40.0, 100.0),
        Some(PoisonOverdoseSeverity::Moderate)
    );
}
#[test]
fn overdose_severe_when_large_overflow() {
    assert_eq!(
        calculate_overdose_severity(80.0, 100.0),
        Some(PoisonOverdoseSeverity::Severe)
    );
}
#[test]
fn overdose_ignores_zero_capacity() {
    assert_eq!(calculate_overdose_severity(10.0, 0.0), None);
}

#[test]
fn realm_awaken_digestion_capacity() {
    assert_eq!(digestion_capacity_for_realm(Realm::Awaken), 100.0);
}
#[test]
fn realm_induce_digestion_capacity() {
    assert_eq!(digestion_capacity_for_realm(Realm::Induce), 120.0);
}
#[test]
fn realm_condense_digestion_capacity() {
    assert_eq!(digestion_capacity_for_realm(Realm::Condense), 140.0);
}
#[test]
fn realm_solidify_digestion_capacity() {
    assert_eq!(digestion_capacity_for_realm(Realm::Solidify), 160.0);
}
#[test]
fn realm_spirit_digestion_capacity() {
    assert_eq!(digestion_capacity_for_realm(Realm::Spirit), 180.0);
}
#[test]
fn realm_void_digestion_capacity() {
    assert_eq!(digestion_capacity_for_realm(Realm::Void), 200.0);
}

#[test]
fn toxicity_decay_light_one_hour() {
    let mut t = PoisonToxicity {
        level: 50.0,
        ..Default::default()
    };
    assert_eq!(decay_poison_toxicity(&mut t, tick::TICKS_PER_HOUR), 1.0);
}
#[test]
fn toxicity_decay_heavy_one_hour() {
    let mut t = PoisonToxicity {
        level: 80.0,
        ..Default::default()
    };
    assert_eq!(decay_poison_toxicity(&mut t, tick::TICKS_PER_HOUR), 0.5);
}
#[test]
fn toxicity_decay_never_negative() {
    let mut t = PoisonToxicity {
        level: 0.2,
        ..Default::default()
    };
    assert_eq!(decay_poison_toxicity(&mut t, tick::TICKS_PER_HOUR), 0.2);
}
#[test]
fn toxicity_decay_zero_elapsed_noop() {
    let mut t = PoisonToxicity {
        level: 50.0,
        ..Default::default()
    };
    assert_eq!(decay_poison_toxicity(&mut t, 0), 0.0);
}
#[test]
fn toxicity_decay_zero_level_noop() {
    let mut t = PoisonToxicity::default();
    assert_eq!(decay_poison_toxicity(&mut t, tick::TICKS_PER_HOUR), 0.0);
}

#[test]
fn digestion_decay_one_hour() {
    let mut d = DigestionLoad {
        current: 20.0,
        ..Default::default()
    };
    assert_eq!(decay_digestion_load(&mut d, 100, tick::TICKS_PER_HOUR), 5.0);
}
#[test]
fn digestion_decay_locked_halves_rate() {
    let mut d = DigestionLoad {
        current: 20.0,
        digest_lock_until_tick: Some(200),
        ..Default::default()
    };
    assert_eq!(decay_digestion_load(&mut d, 100, tick::TICKS_PER_HOUR), 2.5);
}
#[test]
fn digestion_decay_unlocks_after_until() {
    let mut d = DigestionLoad {
        current: 20.0,
        digest_lock_until_tick: Some(90),
        ..Default::default()
    };
    decay_digestion_load(&mut d, 100, tick::TICKS_PER_HOUR);
    assert!(d.digest_lock_until_tick.is_none());
}
#[test]
fn digestion_decay_never_negative() {
    let mut d = DigestionLoad {
        current: 1.0,
        ..Default::default()
    };
    assert_eq!(decay_digestion_load(&mut d, 100, tick::TICKS_PER_HOUR), 1.0);
}
#[test]
fn digestion_decay_zero_elapsed_noop() {
    let mut d = DigestionLoad {
        current: 20.0,
        ..Default::default()
    };
    assert_eq!(decay_digestion_load(&mut d, 100, 0), 0.0);
}

#[test]
fn toxicity_threshold_below_30_no_debuff() {
    assert!(poison_debuff_for_toxicity(29.9).is_none());
}
#[test]
fn toxicity_threshold_30_mild() {
    assert_eq!(
        poison_debuff_for_toxicity(30.0).unwrap().tier,
        PoisonDebuffTier::Mild
    );
}
#[test]
fn toxicity_threshold_70_mild() {
    assert_eq!(
        poison_debuff_for_toxicity(70.0).unwrap().tier,
        PoisonDebuffTier::Mild
    );
}
#[test]
fn toxicity_threshold_above_70_severe() {
    assert_eq!(
        poison_debuff_for_toxicity(70.1).unwrap().tier,
        PoisonDebuffTier::Severe
    );
}
#[test]
fn toxicity_debuff_duration_mild_5s() {
    assert_eq!(
        poison_debuff_for_toxicity(30.0).unwrap().duration_ticks,
        100
    );
}
#[test]
fn toxicity_debuff_duration_severe_8s() {
    assert_eq!(
        poison_debuff_for_toxicity(80.0).unwrap().duration_ticks,
        160
    );
}

#[test]
fn powder_debuff_wu_is_mild() {
    assert_eq!(
        poison_debuff_for_powder(PoisonPowderKind::WuSuiSanXin).tier,
        PoisonDebuffTier::Mild
    );
}
#[test]
fn powder_debuff_chi_is_mild() {
    assert_eq!(
        poison_debuff_for_powder(PoisonPowderKind::ChiTuoZhiSui).tier,
        PoisonDebuffTier::Mild
    );
}
#[test]
fn powder_debuff_qing_is_moderate() {
    assert_eq!(
        poison_debuff_for_powder(PoisonPowderKind::QingLinManTuo).tier,
        PoisonDebuffTier::Moderate
    );
}
#[test]
fn powder_debuff_tie_is_moderate() {
    assert_eq!(
        poison_debuff_for_powder(PoisonPowderKind::TieFuSheDan).tier,
        PoisonDebuffTier::Moderate
    );
}
#[test]
fn powder_debuff_fu_is_severe() {
    assert_eq!(
        poison_debuff_for_powder(PoisonPowderKind::FuXinXuanGui).tier,
        PoisonDebuffTier::Severe
    );
}

#[test]
fn attack_modifier_no_toxicity_keeps_damage() {
    let r =
        apply_poison_attack_modifier(player(), None, None, 10.0, PoisonAttackKind::Anqi, None, 1);
    assert_eq!(r.final_damage, 10.0);
}
#[test]
fn attack_modifier_mild_toxicity_multiplies_damage() {
    let t = PoisonToxicity {
        level: 30.0,
        ..Default::default()
    };
    let r = apply_poison_attack_modifier(
        player(),
        None,
        Some(&t),
        10.0,
        PoisonAttackKind::Anqi,
        None,
        1,
    );
    assert!(r.final_damage > 10.0);
}
#[test]
fn attack_modifier_severe_toxicity_multiplies_more() {
    let t = PoisonToxicity {
        level: 80.0,
        ..Default::default()
    };
    let r = apply_poison_attack_modifier(
        player(),
        None,
        Some(&t),
        10.0,
        PoisonAttackKind::Baomai,
        None,
        1,
    );
    assert!(r.final_damage >= 12.0);
}
#[test]
fn attack_modifier_dugu_excludes_toxicity() {
    let t = PoisonToxicity {
        level: 80.0,
        ..Default::default()
    };
    let r = apply_poison_attack_modifier(
        player(),
        None,
        Some(&t),
        10.0,
        PoisonAttackKind::Dugu,
        None,
        1,
    );
    assert!(r.toxicity_debuff.is_none());
}
#[test]
fn attack_modifier_powder_adds_damage() {
    let r = apply_poison_attack_modifier(
        player(),
        None,
        None,
        10.0,
        PoisonAttackKind::Anqi,
        Some(PoisonPowderKind::WuSuiSanXin),
        1,
    );
    assert!(r.final_damage > 10.0);
}
#[test]
fn attack_modifier_powder_emits_consumed_event() {
    let r = apply_poison_attack_modifier(
        player(),
        Some(Entity::from_raw(2)),
        None,
        10.0,
        PoisonAttackKind::Anqi,
        Some(PoisonPowderKind::TieFuSheDan),
        1,
    );
    assert!(r.consumed_powder.is_some());
}
#[test]
fn attack_modifier_double_layer_stacks() {
    let t = PoisonToxicity {
        level: 80.0,
        ..Default::default()
    };
    let r = apply_poison_attack_modifier(
        player(),
        None,
        Some(&t),
        10.0,
        PoisonAttackKind::Zhenfa,
        Some(PoisonPowderKind::FuXinXuanGui),
        1,
    );
    assert!(r.final_damage > 70.0);
}
#[test]
fn attack_modifier_negative_damage_clamps() {
    let r =
        apply_poison_attack_modifier(player(), None, None, -1.0, PoisonAttackKind::Anqi, None, 1);
    assert_eq!(r.final_damage, 0.0);
}

#[test]
fn side_effect_wu_duration() {
    assert_eq!(PoisonSideEffectTag::QiFocusDrift2h.duration_ticks(), 144000);
}
#[test]
fn side_effect_chi_duration() {
    assert_eq!(PoisonSideEffectTag::RageBurst30m.duration_ticks(), 36000);
}
#[test]
fn side_effect_qing_duration() {
    assert_eq!(PoisonSideEffectTag::HallucinTint6h.duration_ticks(), 432000);
}
#[test]
fn side_effect_tie_duration() {
    assert_eq!(PoisonSideEffectTag::DigestLock6h.duration_ticks(), 432000);
}
#[test]
fn side_effect_fu_duration_is_permanent_marker() {
    assert_eq!(PoisonSideEffectTag::ToxicityTierUnlock.duration_ticks(), 0);
}

#[test]
fn recipe_ids_have_five_entries() {
    assert_eq!(poison_alchemy_recipe_ids().len(), 5);
}
#[test]
fn poison_trait_recipes_do_not_use_dugu_insidious_color() {
    let recipes = [
        include_str!("../../../assets/alchemy/recipes/poison_trait_chi_tuo_zhi_sui_v1.json"),
        include_str!("../../../assets/alchemy/recipes/poison_trait_fu_xin_xuan_gui_v1.json"),
        include_str!("../../../assets/alchemy/recipes/poison_trait_qing_lin_man_tuo_v1.json"),
        include_str!("../../../assets/alchemy/recipes/poison_trait_tie_fu_she_dan_v1.json"),
        include_str!("../../../assets/alchemy/recipes/poison_trait_wu_sui_san_xin_v1.json"),
    ];
    for recipe in recipes {
        assert!(
            !recipe.contains("\"toxin_color\": \"Insidious\""),
            "毒性真元毒丹不得使用毒蛊专属 Insidious 染色"
        );
        assert!(
            recipe.contains("\"toxin_color\": \"Turbid\""),
            "毒性真元毒丹 perfect/good 档应使用可洗 Turbid 染色"
        );
    }
}
#[test]
fn craft_recipe_wu_outputs_three_powders() {
    assert_eq!(
        recipes::craft_recipe_for_powder(PoisonPowderKind::WuSuiSanXin)
            .output
            .1,
        3
    );
}
#[test]
fn craft_recipe_uses_poison_powder_category() {
    assert_eq!(
        recipes::craft_recipe_for_powder(PoisonPowderKind::FuXinXuanGui).category,
        crate::craft::CraftCategory::PoisonPowder
    );
}
#[test]
fn craft_recipe_has_unlock_sources() {
    assert!(
        !recipes::craft_recipe_for_powder(PoisonPowderKind::TieFuSheDan)
            .unlock_sources
            .is_empty()
    );
}
#[test]
fn micro_tear_roll_is_stable() {
    assert_eq!(
        poison_micro_tear_roll(player(), 42),
        poison_micro_tear_roll(player(), 42)
    );
}
#[test]
fn micro_tear_roll_is_unit_interval() {
    let r = poison_micro_tear_roll(player(), 42);
    assert!((0.0..=1.0).contains(&r));
}
#[test]
fn normalized_toxicity_clamps_high() {
    assert_eq!(
        PoisonToxicity {
            level: 200.0,
            ..Default::default()
        }
        .normalized()
        .level,
        100.0
    );
}
#[test]
fn normalized_digestion_clamps_high() {
    assert_eq!(
        DigestionLoad {
            current: 200.0,
            capacity: 100.0,
            ..Default::default()
        }
        .normalized()
        .current,
        100.0
    );
}
#[test]
fn side_effect_tag_lookup_for_item() {
    assert_eq!(
        poison_side_effect_tag_for_item("poison_pill_tie_fu_she_dan"),
        Some(PoisonSideEffectTag::DigestLock6h)
    );
}
