//! 丹道底盘 P0 + 变异系统 P1 — 饱和测试。

use super::components::{DandaoStyle, MutationStage, MUTATION_STAGE_THRESHOLDS};
use super::mutation::{
    social_penalty_for_stage, MutationAdvanceEvent, MERIDIAN_PENALTY_BY_STAGE,
    WEAPON_SWAP_COOLDOWN_TICKS,
};
use super::skills::dandao_qi_cost_base;
use crate::cultivation::components::Realm;

// ============================================================
// DandaoStyle 组件逻辑
// ============================================================

#[test]
fn dandao_style_default_is_stage_zero() {
    let style = DandaoStyle::default();
    assert_eq!(style.mutation_stage, 0);
    assert_eq!(style.cumulative_toxin, 0.0);
    assert_eq!(style.pill_intake_count, 0);
    assert_eq!(style.brew_count, 0);
    assert_eq!(style.mastery_ticks, 0);
}

#[test]
fn advance_toxin_increments_pill_count() {
    let mut style = DandaoStyle::default();
    style.advance_toxin(0.5);
    assert_eq!(
        style.pill_intake_count, 1,
        "单次 advance 应递增 pill_intake_count"
    );
    style.advance_toxin(0.3);
    assert_eq!(style.pill_intake_count, 2);
}

#[test]
fn advance_toxin_accumulates_correctly() {
    let mut style = DandaoStyle::default();
    style.advance_toxin(0.5);
    style.advance_toxin(1.0);
    style.advance_toxin(0.3);
    let expected = 0.5 + 1.0 + 0.3;
    assert!(
        (style.cumulative_toxin - expected).abs() < f64::EPSILON,
        "累计丹毒应为各次之和：expected={expected}, got={}",
        style.cumulative_toxin
    );
}

#[test]
fn advance_toxin_never_decreases() {
    let mut style = DandaoStyle::default();
    style.advance_toxin(10.0);
    let before = style.cumulative_toxin;
    style.advance_toxin(0.0);
    assert_eq!(style.cumulative_toxin, before, "zero toxin 不应改变累计值");
}

#[test]
fn advance_toxin_rejects_negative() {
    let mut style = DandaoStyle::default();
    style.advance_toxin(10.0);
    let before = style.cumulative_toxin;
    let result = style.advance_toxin(-5.0);
    assert_eq!(result, None, "负数 toxin 不应有任何效果");
    assert_eq!(style.cumulative_toxin, before, "负数 toxin 不应改变累计值");
}

#[test]
fn advance_toxin_rejects_nan() {
    let mut style = DandaoStyle::default();
    let result = style.advance_toxin(f64::NAN);
    assert_eq!(result, None, "NaN toxin 不应有任何效果");
    assert_eq!(style.cumulative_toxin, 0.0);
}

#[test]
fn advance_toxin_rejects_infinity() {
    let mut style = DandaoStyle::default();
    let result = style.advance_toxin(f64::INFINITY);
    assert_eq!(result, None, "Infinity toxin 不应有任何效果");
    assert_eq!(style.cumulative_toxin, 0.0);
}

// ============================================================
// 阶段阈值
// ============================================================

#[test]
fn stage_thresholds_are_ordered() {
    for i in 1..MUTATION_STAGE_THRESHOLDS.len() {
        assert!(
            MUTATION_STAGE_THRESHOLDS[i] > MUTATION_STAGE_THRESHOLDS[i - 1],
            "阈值必须严格递增: [{}]={} <= [{}]={}",
            i - 1,
            MUTATION_STAGE_THRESHOLDS[i - 1],
            i,
            MUTATION_STAGE_THRESHOLDS[i]
        );
    }
}

#[test]
fn stage_thresholds_exact_values() {
    assert_eq!(
        MUTATION_STAGE_THRESHOLDS,
        [30.0, 100.0, 250.0, 500.0],
        "§8.1 #7: 阈值 0/30/100/250/500 保留不变"
    );
}

#[test]
fn stage_for_toxin_below_first_threshold_is_zero() {
    assert_eq!(DandaoStyle::stage_for_toxin(0.0), 0);
    assert_eq!(DandaoStyle::stage_for_toxin(29.99), 0);
}

#[test]
fn stage_for_toxin_at_first_threshold_is_one() {
    assert_eq!(
        DandaoStyle::stage_for_toxin(MUTATION_STAGE_THRESHOLDS[0]),
        1,
        "恰好等于阈值 [0]={} 应为 stage 1",
        MUTATION_STAGE_THRESHOLDS[0]
    );
}

#[test]
fn stage_for_toxin_at_second_threshold_is_two() {
    assert_eq!(
        DandaoStyle::stage_for_toxin(MUTATION_STAGE_THRESHOLDS[1]),
        2
    );
}

#[test]
fn stage_for_toxin_at_third_threshold_is_three() {
    assert_eq!(
        DandaoStyle::stage_for_toxin(MUTATION_STAGE_THRESHOLDS[2]),
        3
    );
}

#[test]
fn stage_for_toxin_at_fourth_threshold_is_four() {
    assert_eq!(
        DandaoStyle::stage_for_toxin(MUTATION_STAGE_THRESHOLDS[3]),
        4
    );
}

#[test]
fn stage_for_toxin_beyond_max_is_four() {
    assert_eq!(DandaoStyle::stage_for_toxin(99999.0), 4);
}

#[test]
fn stage_for_toxin_between_thresholds() {
    assert_eq!(
        DandaoStyle::stage_for_toxin(50.0),
        1,
        "50 在 30-100 之间应为 stage 1"
    );
    assert_eq!(
        DandaoStyle::stage_for_toxin(150.0),
        2,
        "150 在 100-250 之间应为 stage 2"
    );
    assert_eq!(
        DandaoStyle::stage_for_toxin(400.0),
        3,
        "400 在 250-500 之间应为 stage 3"
    );
}

// ============================================================
// advance_toxin 阶段跃迁
// ============================================================

#[test]
fn advance_toxin_returns_none_when_no_stage_change() {
    let mut style = DandaoStyle::default();
    let result = style.advance_toxin(1.0);
    assert_eq!(result, None, "从 0 到 1.0 不跨阈值，应返回 None");
}

#[test]
fn advance_toxin_returns_new_stage_on_threshold_cross() {
    let mut style = DandaoStyle::default();
    let result = style.advance_toxin(MUTATION_STAGE_THRESHOLDS[0]);
    assert_eq!(result, Some(1), "从 0 跨越第一阈值应返回 Some(1)");
    assert_eq!(style.mutation_stage, 1);
}

#[test]
fn advance_toxin_can_skip_stages() {
    let mut style = DandaoStyle::default();
    let result = style.advance_toxin(MUTATION_STAGE_THRESHOLDS[2] + 1.0);
    assert_eq!(result, Some(3), "一次性跨越多个阈值应直接到正确阶段");
    assert_eq!(style.mutation_stage, 3);
}

#[test]
fn advance_toxin_stage_never_decreases() {
    let mut style = DandaoStyle::default();
    style.advance_toxin(MUTATION_STAGE_THRESHOLDS[3] + 100.0);
    assert_eq!(style.mutation_stage, 4);
    let result = style.advance_toxin(0.0);
    assert_eq!(result, None);
    assert_eq!(style.mutation_stage, 4, "阶段不可降级");
}

#[test]
fn advance_toxin_from_stage_1_to_2() {
    let mut style = DandaoStyle {
        cumulative_toxin: MUTATION_STAGE_THRESHOLDS[0] + 1.0,
        mutation_stage: 1,
        brew_count: 0,
        pill_intake_count: 0,
        mastery_ticks: 0,
    };
    let need = MUTATION_STAGE_THRESHOLDS[1] - style.cumulative_toxin;
    let result = style.advance_toxin(need + 0.01);
    assert_eq!(result, Some(2), "从阶段 1 跨越阈值 [1] 到阶段 2");
}

#[test]
fn advance_toxin_from_stage_2_to_3() {
    let mut style = DandaoStyle {
        cumulative_toxin: MUTATION_STAGE_THRESHOLDS[1] + 1.0,
        mutation_stage: 2,
        brew_count: 0,
        pill_intake_count: 0,
        mastery_ticks: 0,
    };
    let need = MUTATION_STAGE_THRESHOLDS[2] - style.cumulative_toxin;
    let result = style.advance_toxin(need + 0.01);
    assert_eq!(result, Some(3), "从阶段 2 跨越阈值 [2] 到阶段 3");
}

#[test]
fn advance_toxin_from_stage_3_to_4() {
    let mut style = DandaoStyle {
        cumulative_toxin: MUTATION_STAGE_THRESHOLDS[2] + 1.0,
        mutation_stage: 3,
        brew_count: 0,
        pill_intake_count: 0,
        mastery_ticks: 0,
    };
    let need = MUTATION_STAGE_THRESHOLDS[3] - style.cumulative_toxin;
    let result = style.advance_toxin(need + 0.01);
    assert_eq!(result, Some(4), "从阶段 3 跨越阈值 [3] 到阶段 4");
}

// ============================================================
// record_brew
// ============================================================

#[test]
fn record_brew_increments() {
    let mut style = DandaoStyle::default();
    style.record_brew();
    style.record_brew();
    assert_eq!(style.brew_count, 2);
}

// ============================================================
// MutationStage enum
// ============================================================

#[test]
fn mutation_stage_from_u8_all_variants() {
    assert_eq!(MutationStage::from(0), MutationStage::None);
    assert_eq!(MutationStage::from(1), MutationStage::Subtle);
    assert_eq!(MutationStage::from(2), MutationStage::Visible);
    assert_eq!(MutationStage::from(3), MutationStage::Heavy);
    assert_eq!(MutationStage::from(4), MutationStage::Bestial);
    assert_eq!(
        MutationStage::from(255),
        MutationStage::Bestial,
        "越界值应 clamp 到 Bestial"
    );
}

// ============================================================
// Serde round-trip
// ============================================================

#[test]
fn dandao_style_serde_roundtrip() {
    let style = DandaoStyle {
        brew_count: 42,
        pill_intake_count: 100,
        cumulative_toxin: 123.456,
        mutation_stage: 3,
        mastery_ticks: 99999,
    };
    let json = serde_json::to_string(&style).expect("serialize");
    let back: DandaoStyle = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(style, back, "serde round-trip 不应丢失数据");
}

// ============================================================
// P1: 变异阶段惩罚精确值（§8.1 #1）
// ============================================================

#[test]
fn meridian_penalty_exact_values() {
    assert_eq!(MERIDIAN_PENALTY_BY_STAGE[0], 0.0, "阶段 0 惩罚 = 0");
    assert_eq!(MERIDIAN_PENALTY_BY_STAGE[1], 0.03, "阶段 1 惩罚 = 3%");
    assert_eq!(MERIDIAN_PENALTY_BY_STAGE[2], 0.08, "阶段 2 惩罚 = 8%");
    assert_eq!(MERIDIAN_PENALTY_BY_STAGE[3], 0.15, "阶段 3 惩罚 = 15%");
    assert_eq!(
        MERIDIAN_PENALTY_BY_STAGE[4], 0.20,
        "阶段 4 惩罚 = 20%（§8.1 #1 从 30% 下调）"
    );
}

// ============================================================
// P1: 社会反应（§2.5）
// ============================================================

#[test]
fn social_penalty_stage_0_and_1_no_penalty() {
    assert_eq!(
        social_penalty_for_stage(MutationStage::None),
        0,
        "阶段 0 无社会惩罚"
    );
    assert_eq!(
        social_penalty_for_stage(MutationStage::Subtle),
        0,
        "阶段 1 微变无社会惩罚"
    );
}

#[test]
fn social_penalty_stage_2_is_minus_20() {
    assert_eq!(
        social_penalty_for_stage(MutationStage::Visible),
        -20,
        "阶段 2 显变 NPC 好感度 -20"
    );
}

#[test]
fn social_penalty_stage_3_is_minus_50() {
    assert_eq!(
        social_penalty_for_stage(MutationStage::Heavy),
        -50,
        "阶段 3 重变 NPC 好感度 -50"
    );
}

#[test]
fn social_penalty_stage_4_is_minus_100() {
    assert_eq!(
        social_penalty_for_stage(MutationStage::Bestial),
        -100,
        "阶段 4 兽化 NPC 完全敌对 -100"
    );
}

// ============================================================
// P1: MutationAdvanceEvent struct 完整性
// ============================================================

#[test]
fn mutation_advance_event_fields() {
    let event = MutationAdvanceEvent {
        entity: valence::prelude::Entity::PLACEHOLDER,
        from_stage: MutationStage::Subtle,
        to_stage: MutationStage::Visible,
    };
    assert_eq!(event.from_stage, MutationStage::Subtle);
    assert_eq!(event.to_stage, MutationStage::Visible);
}

// ============================================================
// §8.1 #4: qi 消耗使用 capacity_for_tier
// ============================================================

#[test]
fn dandao_qi_cost_base_scales_with_realm() {
    let costs: Vec<f64> = [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ]
    .iter()
    .map(|&r| dandao_qi_cost_base(r))
    .collect();

    for i in 1..costs.len() {
        assert!(
            costs[i] > costs[i - 1],
            "qi 消耗应随境界递增: [{i}]={} <= [{}]={}",
            costs[i],
            i - 1,
            costs[i - 1]
        );
    }
}

#[test]
fn dandao_qi_cost_base_awaken_is_0_3() {
    // capacity_for_tier(0) = 10.0, * 0.03 = 0.3
    let cost = dandao_qi_cost_base(Realm::Awaken);
    assert!(
        (cost - 0.3).abs() < f64::EPSILON,
        "醒灵 qi 消耗应为 10.0*0.03=0.3, got {cost}"
    );
}

// ============================================================
// §8.1 #2: 多臂切换 GCD
// ============================================================

#[test]
fn weapon_swap_cooldown_is_1_second() {
    assert_eq!(
        WEAPON_SWAP_COOLDOWN_TICKS, 20,
        "§8.1 #2: 多臂武器切换 GCD = 1s = 20 ticks (20 tps)"
    );
}

// ============================================================
// EquipSlotV1 多臂扩展
// ============================================================

#[test]
fn equip_slot_v1_extra_hand_serde() {
    use crate::schema::inventory::EquipSlotV1;

    let slot0 = EquipSlotV1::ExtraHand0;
    let json0 = serde_json::to_string(&slot0).expect("serialize ExtraHand0");
    assert_eq!(json0, "\"extra_hand_0\"", "ExtraHand0 wire format");
    let back0: EquipSlotV1 = serde_json::from_str(&json0).expect("deserialize");
    assert_eq!(back0, slot0);

    let slot1 = EquipSlotV1::ExtraHand1;
    let json1 = serde_json::to_string(&slot1).expect("serialize ExtraHand1");
    assert_eq!(json1, "\"extra_hand_1\"", "ExtraHand1 wire format");
    let back1: EquipSlotV1 = serde_json::from_str(&json1).expect("deserialize");
    assert_eq!(back1, slot1);
}

// ============================================================
// LifeRecord MutationAdvanced variant
// ============================================================

#[test]
fn biography_mutation_advanced_serde() {
    use crate::cultivation::life_record::BiographyEntry;

    let entry = BiographyEntry::MutationAdvanced {
        from_stage: 1,
        to_stage: 2,
        cumulative_toxin: 105.0,
        tick: 50000,
    };
    let json = serde_json::to_string(&entry).expect("serialize MutationAdvanced");
    let back: BiographyEntry = serde_json::from_str(&json).expect("deserialize");
    match back {
        BiographyEntry::MutationAdvanced {
            from_stage,
            to_stage,
            cumulative_toxin,
            tick,
        } => {
            assert_eq!(from_stage, 1);
            assert_eq!(to_stage, 2);
            assert!((cumulative_toxin - 105.0).abs() < f64::EPSILON);
            assert_eq!(tick, 50000);
        }
        _ => panic!("反序列化后应为 MutationAdvanced"),
    }
}
