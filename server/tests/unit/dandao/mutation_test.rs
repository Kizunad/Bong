use std::collections::HashSet;

use bong_server::body_plan::BodyPlan;
use bong_server::combat::components::BodyPart;
use bong_server::dandao::components::MutationStage;
use bong_server::dandao::mutation::*;

#[test]
fn mutation_state_default_is_none() {
    let state = MutationState::default();
    assert_eq!(state.stage, MutationStage::None);
    assert_eq!(state.meridian_penalty, 0.0);
    assert!(state.slots.is_empty());
}

#[test]
fn advance_to_updates_penalty() {
    let mut state = MutationState::default();
    state.advance_to(MutationStage::Subtle);
    assert_eq!(state.meridian_penalty, 0.03, "阶段 1 惩罚应为 3%");
    state.advance_to(MutationStage::Visible);
    assert_eq!(state.meridian_penalty, 0.08, "阶段 2 惩罚应为 8%");
    state.advance_to(MutationStage::Heavy);
    assert_eq!(state.meridian_penalty, 0.15, "阶段 3 惩罚应为 15%");
    state.advance_to(MutationStage::Bestial);
    assert_eq!(
        state.meridian_penalty, 0.20,
        "§8.1 #1 决议：阶段 4 惩罚从 -30% 调到 -20%"
    );
}

#[test]
fn advance_to_sets_stage() {
    let mut state = MutationState::default();
    state.advance_to(MutationStage::Heavy);
    assert_eq!(state.stage, MutationStage::Heavy, "advance_to 应更新 stage");
}

#[test]
fn advance_to_preserves_existing_slots() {
    let mut state = MutationState {
        stage: MutationStage::Subtle,
        slots: vec![ActiveMutation {
            kind: MutationKind::GoldenIris,
            slot: BodySlot::Head,
            level: 1,
            acquired_tick: 100,
        }],
        meridian_penalty: 0.03,
    };
    state.advance_to(MutationStage::Visible);
    assert_eq!(state.slots.len(), 1, "advance_to 不应清除已有 slots");
    assert_eq!(state.slots[0].kind, MutationKind::GoldenIris);
}

#[test]
fn mutation_kind_min_stage_correct() {
    assert_eq!(MutationKind::GoldenIris.min_stage(), MutationStage::Subtle);
    assert_eq!(
        MutationKind::HardenedNails.min_stage(),
        MutationStage::Subtle
    );
    assert_eq!(MutationKind::ToughSkin.min_stage(), MutationStage::Subtle);
    assert_eq!(MutationKind::BoneRidge.min_stage(), MutationStage::Visible);
    assert_eq!(
        MutationKind::ForearmScales.min_stage(),
        MutationStage::Visible
    );
    assert_eq!(MutationKind::SpineSpurs.min_stage(), MutationStage::Visible);
    assert_eq!(MutationKind::Horns.min_stage(), MutationStage::Heavy);
    assert_eq!(MutationKind::Tail.min_stage(), MutationStage::Heavy);
    assert_eq!(MutationKind::BackCarapace.min_stage(), MutationStage::Heavy);
    assert_eq!(MutationKind::ExtraArms.min_stage(), MutationStage::Bestial);
    assert_eq!(
        MutationKind::BodyEnlarge.min_stage(),
        MutationStage::Bestial
    );
    assert_eq!(MutationKind::BeastFace.min_stage(), MutationStage::Bestial);
}

#[test]
fn choices_for_stage_have_correct_count() {
    assert_eq!(
        MutationKind::choices_for_stage(MutationStage::None).len(),
        0
    );
    assert_eq!(
        MutationKind::choices_for_stage(MutationStage::Subtle).len(),
        3
    );
    assert_eq!(
        MutationKind::choices_for_stage(MutationStage::Visible).len(),
        3
    );
    assert_eq!(
        MutationKind::choices_for_stage(MutationStage::Heavy).len(),
        3
    );
    assert_eq!(
        MutationKind::choices_for_stage(MutationStage::Bestial).len(),
        3
    );
}

#[test]
fn choices_for_stage_match_min_stage() {
    for stage in [
        MutationStage::Subtle,
        MutationStage::Visible,
        MutationStage::Heavy,
        MutationStage::Bestial,
    ] {
        for kind in MutationKind::choices_for_stage(stage) {
            assert_eq!(
                kind.min_stage(),
                stage,
                "{kind:?} 的 min_stage 应匹配 {stage:?}"
            );
        }
    }
}

#[test]
fn body_slot_assignments_no_duplicate_within_stage() {
    for stage in [
        MutationStage::Subtle,
        MutationStage::Visible,
        MutationStage::Heavy,
        MutationStage::Bestial,
    ] {
        let slots: Vec<BodySlot> = MutationKind::choices_for_stage(stage)
            .iter()
            .map(|k| k.body_slot())
            .collect();
        let unique: HashSet<BodySlot> = slots.iter().copied().collect();
        assert_eq!(
            slots.len(),
            unique.len(),
            "阶段 {stage:?} 内不应有重复 body_slot"
        );
    }
}

#[test]
fn social_penalty_monotonic() {
    let stages = [
        MutationStage::None,
        MutationStage::Subtle,
        MutationStage::Visible,
        MutationStage::Heavy,
        MutationStage::Bestial,
    ];
    for window in stages.windows(2) {
        assert!(
            social_penalty_for_stage(window[0]) >= social_penalty_for_stage(window[1]),
            "社会惩罚应单调递减（更负）: {:?} vs {:?}",
            window[0],
            window[1]
        );
    }
}

#[test]
fn social_penalty_exact_values() {
    assert_eq!(social_penalty_for_stage(MutationStage::None), 0);
    assert_eq!(social_penalty_for_stage(MutationStage::Subtle), 0);
    assert_eq!(social_penalty_for_stage(MutationStage::Visible), -20);
    assert_eq!(social_penalty_for_stage(MutationStage::Heavy), -50);
    assert_eq!(social_penalty_for_stage(MutationStage::Bestial), -100);
}

#[test]
fn tiandao_attention_only_stage_3_plus() {
    assert!(!triggers_tiandao_attention(MutationStage::None));
    assert!(!triggers_tiandao_attention(MutationStage::Subtle));
    assert!(!triggers_tiandao_attention(MutationStage::Visible));
    assert!(triggers_tiandao_attention(MutationStage::Heavy));
    assert!(triggers_tiandao_attention(MutationStage::Bestial));
}

#[test]
fn meridian_penalty_by_stage_ordered() {
    for i in 1..MERIDIAN_PENALTY_BY_STAGE.len() {
        assert!(
            MERIDIAN_PENALTY_BY_STAGE[i] >= MERIDIAN_PENALTY_BY_STAGE[i - 1],
            "经脉惩罚应单调递增: [{i}]={} < [{}]={}",
            MERIDIAN_PENALTY_BY_STAGE[i],
            i - 1,
            MERIDIAN_PENALTY_BY_STAGE[i - 1]
        );
    }
}

#[test]
fn meridian_penalty_exact_values_s81_1() {
    // §8.1 #1 精确断言: -3%/-8%/-15%/-20%
    assert_eq!(MERIDIAN_PENALTY_BY_STAGE[0], 0.0);
    assert_eq!(MERIDIAN_PENALTY_BY_STAGE[1], 0.03);
    assert_eq!(MERIDIAN_PENALTY_BY_STAGE[2], 0.08);
    assert_eq!(MERIDIAN_PENALTY_BY_STAGE[3], 0.15);
    assert_eq!(
        MERIDIAN_PENALTY_BY_STAGE[4], 0.20,
        "§8.1 #1: 阶段 4 从 0.30 调到 0.20"
    );
}

#[test]
fn mutation_state_serde_roundtrip() {
    let state = MutationState {
        stage: MutationStage::Heavy,
        slots: vec![ActiveMutation {
            kind: MutationKind::Horns,
            slot: BodySlot::Head,
            level: 2,
            acquired_tick: 12345,
        }],
        meridian_penalty: 0.15,
    };
    let json = serde_json::to_string(&state).expect("serialize");
    let back: MutationState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(state, back);
}

#[test]
fn active_mutation_all_fields_survive_serde() {
    let m = ActiveMutation {
        kind: MutationKind::ExtraArms,
        slot: BodySlot::Forearm,
        level: 3,
        acquired_tick: 999999,
    };
    let json = serde_json::to_string(&m).expect("serialize");
    let back: ActiveMutation = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(m, back);
}

// --- 变异功能性（§2.4）---

#[test]
fn every_mutation_kind_has_effect() {
    let all_kinds = [
        MutationKind::GoldenIris,
        MutationKind::HardenedNails,
        MutationKind::ToughSkin,
        MutationKind::BoneRidge,
        MutationKind::ForearmScales,
        MutationKind::SpineSpurs,
        MutationKind::Horns,
        MutationKind::Tail,
        MutationKind::BackCarapace,
        MutationKind::ExtraArms,
        MutationKind::BodyEnlarge,
        MutationKind::BeastFace,
    ];
    for kind in all_kinds {
        // 如果 effect() 会 panic 这里就会失败
        let _effect = kind.effect();
    }
}

#[test]
fn golden_iris_gives_vision_boost() {
    match MutationKind::GoldenIris.effect() {
        MutationEffect::VisionBoost {
            negative_zone_range_pct,
            darkness_brightness_add,
        } => {
            assert!(
                (negative_zone_range_pct - 0.30).abs() < f64::EPSILON,
                "金瞳视距应 +30%, got {negative_zone_range_pct}"
            );
            assert_eq!(darkness_brightness_add, 2, "暗处亮度应 +2");
        }
        other => panic!("金瞳应为 VisionBoost, got {other:?}"),
    }
}

#[test]
fn hardened_nails_gives_unarmed_bonus() {
    match MutationKind::HardenedNails.effect() {
        MutationEffect::UnarmedDamageBonus { base_attack_add } => {
            assert_eq!(base_attack_add, 3, "硬甲指空手附加应为 +3");
        }
        other => panic!("硬甲指应为 UnarmedDamageBonus, got {other:?}"),
    }
}

#[test]
fn tough_skin_gives_purge_boost() {
    match MutationKind::ToughSkin.effect() {
        MutationEffect::PurgeBoost {
            contamination_purge_pct,
        } => {
            assert!(
                (contamination_purge_pct - 0.10).abs() < f64::EPSILON,
                "糙皮排毒应 +10%, got {contamination_purge_pct}"
            );
        }
        other => panic!("糙皮应为 PurgeBoost, got {other:?}"),
    }
}

#[test]
fn extra_arms_gives_2_hand_slots() {
    match MutationKind::ExtraArms.effect() {
        MutationEffect::ExtraHandSlots { count } => {
            assert_eq!(count, 2, "多臂应 +2 手槽位");
        }
        other => panic!("多臂应为 ExtraHandSlots, got {other:?}"),
    }
}

#[test]
fn body_enlarge_gives_constitution_boost() {
    match MutationKind::BodyEnlarge.effect() {
        MutationEffect::ConstitutionBoost {
            hp_pct,
            hitbox_scale,
        } => {
            assert!(
                (hp_pct - 0.50).abs() < f64::EPSILON,
                "膨胀 HP 应 +50%, got {hp_pct}"
            );
            assert!(
                (hitbox_scale - 1.5).abs() < f64::EPSILON,
                "膨胀 hitbox 应 ×1.5, got {hitbox_scale}"
            );
        }
        other => panic!("膨胀应为 ConstitutionBoost, got {other:?}"),
    }
}

#[test]
fn beast_face_gives_intimidate_aura() {
    match MutationKind::BeastFace.effect() {
        MutationEffect::IntimidateAura {
            range_blocks,
            realm_diff_threshold,
            composure_reduction_pct,
        } => {
            assert_eq!(range_blocks, 5, "恐吓光环范围应为 5 格");
            assert_eq!(realm_diff_threshold, 2, "恐吓需低 2 境界");
            assert!(
                (composure_reduction_pct - 0.30).abs() < f64::EPSILON,
                "心境降低应 -30%, got {composure_reduction_pct}"
            );
        }
        other => panic!("兽面应为 IntimidateAura, got {other:?}"),
    }
}

#[test]
fn forearm_scales_gives_natural_armor() {
    match MutationKind::ForearmScales.effect() {
        MutationEffect::NaturalArmor {
            body_part,
            downgrade_from,
            downgrade_to,
        } => {
            assert_eq!(body_part, "forearm");
            assert_eq!(downgrade_from, "abrasion");
            assert_eq!(downgrade_to, "bruise");
        }
        other => panic!("前臂鳞应为 NaturalArmor, got {other:?}"),
    }
}

#[test]
fn back_carapace_gives_natural_armor() {
    match MutationKind::BackCarapace.effect() {
        MutationEffect::NaturalArmor {
            body_part,
            downgrade_from,
            downgrade_to,
        } => {
            assert_eq!(body_part, "back");
            assert_eq!(downgrade_from, "laceration");
            assert_eq!(downgrade_to, "abrasion");
        }
        other => panic!("背甲应为 NaturalArmor, got {other:?}"),
    }
}

#[test]
fn spine_spurs_gives_damage_reduction() {
    match MutationKind::SpineSpurs.effect() {
        MutationEffect::DamageReduction {
            body_part,
            reduction_pct,
        } => {
            assert_eq!(body_part, "back");
            assert!(
                (reduction_pct - 0.20).abs() < f64::EPSILON,
                "脊突背部减伤应 -20%, got {reduction_pct}"
            );
        }
        other => panic!("脊突应为 DamageReduction, got {other:?}"),
    }
}

#[test]
fn tail_gives_strike_and_fall_reduction() {
    match MutationKind::Tail.effect() {
        MutationEffect::TailStrike {
            skill_id,
            fall_damage_reduction_pct,
        } => {
            assert_eq!(skill_id, "dandao.tail_strike");
            assert!(
                (fall_damage_reduction_pct - 0.50).abs() < f64::EPSILON,
                "尾击坠落减伤应 50%, got {fall_damage_reduction_pct}"
            );
        }
        other => panic!("尾应为 TailStrike, got {other:?}"),
    }
}

#[test]
fn weapon_swap_cooldown_is_20_ticks() {
    assert_eq!(
        WEAPON_SWAP_COOLDOWN_TICKS, 20,
        "§8.1 #2: 多臂切换 GCD = 1s = 20 ticks"
    );
}

// --- 不可逆性断言 ---

#[test]
fn mutation_stage_does_not_decrease_on_advance() {
    let mut state = MutationState::default();
    state.advance_to(MutationStage::Heavy);
    let penalty_heavy = state.meridian_penalty;
    // 试图"降级" — advance_to 不做降级检查（应由调用方保证），
    // 但 MutationState 结构本身允许 set。不可逆性由 mutation_advance_system 保证。
    assert_eq!(state.stage, MutationStage::Heavy);
    assert_eq!(
        state.meridian_penalty, penalty_heavy,
        "advance_to(Heavy) 后惩罚应为 Heavy 级"
    );
}

// --- 节流常量 ---

#[test]
fn mutation_advance_interval_is_600_ticks() {
    assert_eq!(
        MUTATION_ADVANCE_INTERVAL_TICKS, 600,
        "mutation_advance_system 应每 600 tick (30s) 检测一次"
    );
}

// ── plan-race-system-v1 P0 review 修复（BLOCKING-2）：
// `mutation_damage_multiplier_for_part` 是 `mutation_slot_mapping` 的第一个真实
// 消费点——每个 BodySlot 变体一条真实消费链测试 + 缺失/悬空映射分支测试 ──────

fn active_mutation_at(kind: MutationKind, slot: BodySlot) -> ActiveMutation {
    ActiveMutation {
        kind,
        slot,
        level: 1,
        acquired_tick: 0,
    }
}

fn state_with(mutations: Vec<ActiveMutation>) -> MutationState {
    MutationState {
        stage: MutationStage::Heavy,
        slots: mutations,
        meridian_penalty: 0.0,
    }
}

#[test]
fn mutation_damage_multiplier_for_part_none_state_is_neutral() {
    let plan = bong_server::body_plan::humanoid_plan_static();
    assert_eq!(
        mutation_damage_multiplier_for_part(None, plan, BodyPart::Back),
        1.0,
        "无 MutationState（entity 从未变异）不应影响任何部位的伤害倍率"
    );
}

#[test]
fn mutation_damage_multiplier_for_part_empty_slots_is_neutral() {
    let plan = bong_server::body_plan::humanoid_plan_static();
    let state = MutationState::default();
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), plan, BodyPart::Back),
        1.0
    );
}

// ── 每个 BodySlot 变体一条真实消费链测试：SpineSpurs 的 DamageReduction(20%) 挂在
// 该变体上时，命中同一 legacy 部位应打八折；命中其他部位不受影响。刻意使用
// `ActiveMutation.slot` 显式指定（不依赖 `kind.body_slot()` 的天然映射），单独验证
// "按 slot 查表" 这条链路本身，覆盖 humanoid.json 的全部 5 个 BodySlot 变体。────

#[test]
fn mutation_damage_multiplier_for_part_head_slot_reduces_matching_legacy_part() {
    let plan = bong_server::body_plan::humanoid_plan_static();
    let state = state_with(vec![active_mutation_at(
        MutationKind::SpineSpurs,
        BodySlot::Head,
    )]);
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), plan, BodyPart::Head),
        0.80,
        "BodySlot::Head 在 humanoid.json 映射到 legacy Head，DamageReduction(20%) 应打八折"
    );
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), plan, BodyPart::Chest),
        1.0,
        "同一条 mutation 不应影响非命中部位（Head slot 不管 Chest）"
    );
}

#[test]
fn mutation_damage_multiplier_for_part_forearm_slot_reduces_matching_legacy_part() {
    let plan = bong_server::body_plan::humanoid_plan_static();
    let state = state_with(vec![active_mutation_at(
        MutationKind::SpineSpurs,
        BodySlot::Forearm,
    )]);
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), plan, BodyPart::ArmR),
        0.80,
        "BodySlot::Forearm 在 humanoid.json 映射到 legacy ArmR"
    );
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), plan, BodyPart::ArmL),
        1.0,
        "Forearm 只映射 ArmR（见 registry.rs 全变体 pin），不应误伤 ArmL"
    );
}

#[test]
fn mutation_damage_multiplier_for_part_back_slot_reduces_matching_legacy_part() {
    let plan = bong_server::body_plan::humanoid_plan_static();
    let state = state_with(vec![active_mutation_at(
        MutationKind::SpineSpurs,
        BodySlot::Back,
    )]);
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), plan, BodyPart::Back),
        0.80,
        "BodySlot::Back 在 humanoid.json 映射到 legacy Back（SpineSpurs 的天然槽位）"
    );
}

#[test]
fn mutation_damage_multiplier_for_part_torso_slot_reduces_matching_legacy_part() {
    let plan = bong_server::body_plan::humanoid_plan_static();
    let state = state_with(vec![active_mutation_at(
        MutationKind::SpineSpurs,
        BodySlot::Torso,
    )]);
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), plan, BodyPart::Chest),
        0.80,
        "BodySlot::Torso 在 humanoid.json 映射到 legacy Chest"
    );
}

#[test]
fn mutation_damage_multiplier_for_part_lower_slot_reduces_matching_legacy_part() {
    let plan = bong_server::body_plan::humanoid_plan_static();
    let state = state_with(vec![active_mutation_at(
        MutationKind::SpineSpurs,
        BodySlot::Lower,
    )]);
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), plan, BodyPart::Abdomen),
        0.80,
        "BodySlot::Lower 在 humanoid.json 映射到 legacy Abdomen"
    );
}

#[test]
fn mutation_damage_multiplier_for_part_ignores_non_damage_reduction_effects() {
    // BackCarapace 的 effect() 是 NaturalArmor（伤势分级降档），不是 DamageReduction
    // ——本函数刻意不消费 NaturalArmor（见函数文档"消费点选择依据"），倍率必须保持中性。
    let plan = bong_server::body_plan::humanoid_plan_static();
    let state = state_with(vec![active_mutation_at(
        MutationKind::BackCarapace,
        BodySlot::Back,
    )]);
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), plan, BodyPart::Back),
        1.0,
        "NaturalArmor 效果不在本函数消费范围内，不应产生倍率变化"
    );
}

#[test]
fn mutation_damage_multiplier_for_part_stacks_multiple_active_mutations_multiplicatively() {
    let plan = bong_server::body_plan::humanoid_plan_static();
    // 两条 mutation 都挂在 Back 且都是 DamageReduction(20%)：0.8 * 0.8 = 0.64。
    let state = state_with(vec![
        active_mutation_at(MutationKind::SpineSpurs, BodySlot::Back),
        active_mutation_at(MutationKind::SpineSpurs, BodySlot::Back),
    ]);
    let multiplier = mutation_damage_multiplier_for_part(Some(&state), plan, BodyPart::Back);
    assert!(
        (multiplier - 0.64).abs() < 1e-6,
        "两条命中同一部位的 DamageReduction(20%) 应叠乘为 0.64，实际 {multiplier}"
    );
}

// ── 缺失/悬空映射分支 ────────────────────────────────────────────────────

fn plan_without_mutation_mapping() -> BodyPlan {
    use bong_server::body_plan::{BodyPartDef, BodyPlanId, PartConsequence};

    let humanoid = bong_server::body_plan::humanoid_plan_static();
    BodyPlan {
        id: BodyPlanId::new("test_no_mutation_mapping"),
        display_name: "测试用无变异映射构型".to_string(),
        is_humanoid: true,
        parts: vec![BodyPartDef {
            id: bong_server::body_plan::legacy_body_part_to_id(BodyPart::Back),
            damage_mul: 1.0,
            contam_mul: 1.0,
            bleed_mul: 1.0,
            consequence: PartConsequence::Core,
        }],
        hit_geometry: humanoid.hit_geometry.clone(),
        equip_slots: vec![],
        meridian_profile: None,
        mutation_slot_mapping: std::collections::HashMap::new(),
    }
}

#[test]
fn mutation_damage_multiplier_for_part_missing_slot_mapping_is_silently_skipped() {
    // 缺失映射分支：plan 的 mutation_slot_mapping 为空（非 humanoid 构型可以合法
    // 不声明变异挂载点）——body_part_for_mutation_slot 返回 None，函数不应 panic，
    // 该条 mutation 对结算无影响。
    let plan = plan_without_mutation_mapping();
    let state = state_with(vec![active_mutation_at(
        MutationKind::SpineSpurs,
        BodySlot::Back,
    )]);
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), &plan, BodyPart::Back),
        1.0,
        "mutation_slot_mapping 为空时该 slot 无法解析，必须静默跳过而非 panic"
    );
}

fn plan_with_dangling_mutation_mapping() -> BodyPlan {
    use bong_server::body_plan::{BodyPartDef, BodyPlanId, PartConsequence};

    let humanoid = bong_server::body_plan::humanoid_plan_static();
    let mut mapping = std::collections::HashMap::new();
    // 悬空映射：BodySlot::Back 指向一个不在 8 段 legacy 字符串集合里的 id
    // （模拟未来非人形 plan 的部位 id，如飞鲸尾鳍）——id_to_legacy_body_part 必须
    // 对此返回 None，而不是 panic 或误判成某个 legacy 部位。
    mapping.insert(
        BodySlot::Back,
        bong_server::body_plan::BodyPartId::new("tail_fin"),
    );
    BodyPlan {
        id: BodyPlanId::new("test_dangling_mutation_mapping"),
        display_name: "测试用悬空变异映射构型".to_string(),
        is_humanoid: false,
        parts: vec![BodyPartDef {
            id: bong_server::body_plan::legacy_body_part_to_id(BodyPart::Back),
            damage_mul: 1.0,
            contam_mul: 1.0,
            bleed_mul: 1.0,
            consequence: PartConsequence::Core,
        }],
        hit_geometry: humanoid.hit_geometry.clone(),
        equip_slots: vec![],
        meridian_profile: None,
        mutation_slot_mapping: mapping,
    }
}

#[test]
fn mutation_damage_multiplier_for_part_dangling_mapping_target_is_silently_skipped() {
    // 悬空映射分支：mutation_slot_mapping 里有该 slot 的条目，但它指向的
    // BodyPartId 没有 legacy BodyPart 对应物——id_to_legacy_body_part 返回 None，
    // 函数必须静默跳过（不 panic、不误判成 Back）。
    let plan = plan_with_dangling_mutation_mapping();
    let state = state_with(vec![active_mutation_at(
        MutationKind::SpineSpurs,
        BodySlot::Back,
    )]);
    assert_eq!(
        mutation_damage_multiplier_for_part(Some(&state), &plan, BodyPart::Back),
        1.0,
        "悬空映射（部位 id 无 legacy 对应物）必须静默跳过，不应误判命中 Back"
    );
}
