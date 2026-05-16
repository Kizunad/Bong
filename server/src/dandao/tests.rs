//! 丹道底盘 P0 — 饱和测试。

use super::components::{DandaoStyle, MutationStage, MUTATION_STAGE_THRESHOLDS};

// --- DandaoStyle 组件逻辑 ---

#[test]
fn dandao_style_default_is_stage_zero() {
    let style = DandaoStyle::default();
    assert_eq!(style.mutation_stage, 0);
    assert_eq!(style.cumulative_toxin, 0.0);
    assert_eq!(style.pill_intake_count, 0);
    assert_eq!(style.brew_count, 0);
}

#[test]
fn advance_toxin_increments_pill_count() {
    let mut style = DandaoStyle::default();
    style.advance_toxin(0.5);
    assert_eq!(style.pill_intake_count, 1, "单次 advance 应递增 pill_intake_count");
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
    assert_eq!(
        style.cumulative_toxin, before,
        "zero toxin 不应改变累计值"
    );
}

// --- 阶段阈值 ---

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
    assert_eq!(DandaoStyle::stage_for_toxin(MUTATION_STAGE_THRESHOLDS[1]), 2);
}

#[test]
fn stage_for_toxin_at_third_threshold_is_three() {
    assert_eq!(DandaoStyle::stage_for_toxin(MUTATION_STAGE_THRESHOLDS[2]), 3);
}

#[test]
fn stage_for_toxin_at_fourth_threshold_is_four() {
    assert_eq!(DandaoStyle::stage_for_toxin(MUTATION_STAGE_THRESHOLDS[3]), 4);
}

#[test]
fn stage_for_toxin_beyond_max_is_four() {
    assert_eq!(DandaoStyle::stage_for_toxin(99999.0), 4);
}

// --- advance_toxin 阶段跃迁 ---

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
    assert_eq!(
        result,
        Some(1),
        "从 0 跨越第一阈值应返回 Some(1)"
    );
    assert_eq!(style.mutation_stage, 1);
}

#[test]
fn advance_toxin_can_skip_stages() {
    let mut style = DandaoStyle::default();
    let result = style.advance_toxin(MUTATION_STAGE_THRESHOLDS[2] + 1.0);
    assert_eq!(
        result,
        Some(3),
        "一次性跨越多个阈值应直接到正确阶段"
    );
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

// --- record_brew ---

#[test]
fn record_brew_increments() {
    let mut style = DandaoStyle::default();
    style.record_brew();
    style.record_brew();
    assert_eq!(style.brew_count, 2);
}

// --- MutationStage enum ---

#[test]
fn mutation_stage_from_u8_all_variants() {
    assert_eq!(MutationStage::from(0), MutationStage::None);
    assert_eq!(MutationStage::from(1), MutationStage::Subtle);
    assert_eq!(MutationStage::from(2), MutationStage::Visible);
    assert_eq!(MutationStage::from(3), MutationStage::Heavy);
    assert_eq!(MutationStage::from(4), MutationStage::Bestial);
    assert_eq!(MutationStage::from(255), MutationStage::Bestial, "越界值应 clamp 到 Bestial");
}

// --- Serde round-trip ---

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
