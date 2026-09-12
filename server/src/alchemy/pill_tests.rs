use super::*;
use crate::body_plan::BodyPartId;
use crate::cultivation::components::{Contamination, Cultivation};

fn fresh_contam() -> Contamination {
    Contamination::default()
}

fn wound_at(location: &str, severity: f32) -> Wound {
    Wound {
        location: BodyPartId::new(location),
        kind: WoundKind::Cut,
        severity,
        bleeding_per_sec: 0.0,
        created_at_tick: 0,
        inflicted_by: None,
    }
}

fn wounds_with(entries: Vec<Wound>) -> Wounds {
    Wounds {
        entries,
        ..Default::default()
    }
}

// ─── bughunt minor：worst_non_severed_part / worst_severed_part 必须先过滤
// 无 legacy `BodyPart` 对应物的伤口，再在剩下的里选最重的，不能反过来 ───────────

#[test]
fn worst_non_severed_part_skips_unmappable_higher_severity_and_returns_mappable_lower_one() {
    let wounds = wounds_with(vec![
        // 全局最重伤（0.7）落在无 legacy 对应物的非人形部位 id 上。
        wound_at("tail_fin", 0.7),
        // 次重伤（0.4，仍 < 0.85 断裂阈值）落在合法 legacy 部位上。
        wound_at("chest", 0.4),
    ]);
    assert_eq!(
        worst_non_severed_part(&wounds),
        Some(BodyPart::Chest),
        "全局最重伤(tail_fin=0.7)没有 legacy 对应物时，必须回落到次重但可定向的 \
             chest(0.4)，而不是因为全局最重伤转换失败就返回 None——先转换失败才丢弃 \
             是把'找不到最重伤'和'最重伤无法定向'混为一谈"
    );
}

#[test]
fn worst_non_severed_part_returns_none_when_every_wound_is_unmappable() {
    let wounds = wounds_with(vec![
        wound_at("tail_fin", 0.7),
        wound_at("left_pincer", 0.3),
    ]);
    assert_eq!(
        worst_non_severed_part(&wounds),
        None,
        "全部候选伤口都没有 legacy 对应物时，仍必须返回 None（不能瞎猜一个部位）"
    );
}

#[test]
fn worst_non_severed_part_ignores_severed_like_wounds_regardless_of_mappability() {
    let wounds = wounds_with(vec![
        // severity >= 0.85 视为"断裂样"，即便是合法 legacy 部位也应被
        // worst_non_severed_part 过滤掉。
        wound_at("head", 0.9),
        wound_at("chest", 0.5),
    ]);
    assert_eq!(
        worst_non_severed_part(&wounds),
        Some(BodyPart::Chest),
        "断裂样伤口(head=0.9)必须被排除在'非断裂伤'候选之外"
    );
}

#[test]
fn worst_severed_part_skips_unmappable_higher_severity_and_returns_mappable_lower_one() {
    let wounds = wounds_with(vec![
        // 全局最重断裂样伤（0.95）落在无 legacy 对应物的部位上。
        wound_at("tail_fin", 0.95),
        // 次重断裂样伤（0.86）落在合法 legacy 部位上。
        wound_at("leg_l", 0.86),
    ]);
    assert_eq!(
        worst_severed_part(&wounds),
        Some(BodyPart::LegL),
        "全局最重断裂样伤(tail_fin=0.95)没有 legacy 对应物时，必须回落到次重但可\
             定向的 leg_l(0.86)"
    );
}

#[test]
fn worst_severed_part_ignores_non_severed_wounds_regardless_of_mappability() {
    let wounds = wounds_with(vec![
        wound_at("chest", 0.5), // 非断裂样，应被排除
        wound_at("arm_r", 0.9), // 断裂样，合法部位
    ]);
    assert_eq!(
        worst_severed_part(&wounds),
        Some(BodyPart::ArmR),
        "非断裂样伤口(chest=0.5)必须被排除在'断裂伤'候选之外"
    );
}

#[test]
fn worst_non_severed_part_empty_wounds_returns_none() {
    assert_eq!(worst_non_severed_part(&wounds_with(vec![])), None);
}

#[test]
fn worst_severed_part_empty_wounds_returns_none() {
    assert_eq!(worst_severed_part(&wounds_with(vec![])), None);
}

fn basic_effect(qi_gain: Option<f64>) -> PillEffect {
    PillEffect {
        toxin_amount: 0.3,
        toxin_color: ColorKind::Mellow,
        qi_gain,
        meridian_progress_bonus: None,
    }
}

#[test]
fn consume_pill_normal_appends_contam_and_restores_qi() {
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::NotApplicable,
        false,
        AgePeakCheck::NotApplicable,
    );
    assert_eq!(outcome.qi_gained, 24.0);
    assert!(!outcome.blocked);
    assert_eq!(outcome.extra_toxin_added, 0.0);
    assert_eq!(cult.qi_current, 24.0);
    assert_eq!(contam.entries.len(), 1);
    assert_eq!(contam.entries[0].color, ColorKind::Mellow);
    assert!(contam.entries[0].attacker_id.is_none());
    assert_eq!(contam.entries[0].introduced_at, 10);
}

#[test]
fn qi_gain_clamped_to_qi_max() {
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 90.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(50.0)),
        &mut contam,
        &mut cult,
        0,
        SpoilCheckOutcome::NotApplicable,
        false,
        AgePeakCheck::NotApplicable,
    );
    assert_eq!(outcome.qi_gained, 10.0);
    assert_eq!(cult.qi_current, 100.0);
}

#[test]
fn can_take_pill_blocks_when_same_color_exceeds_threshold() {
    let mut contam = fresh_contam();
    contam.entries.push(ContamSource {
        amount: 0.6,
        color: ColorKind::Mellow,
        meridian_id: None,
        attacker_id: None,
        introduced_at: 0,
    });
    contam.entries.push(ContamSource {
        amount: 0.5,
        color: ColorKind::Mellow,
        meridian_id: None,
        attacker_id: None,
        introduced_at: 1,
    });
    // 总量 1.1 ≥ 1.0 阈值
    assert!(!can_take_pill(&contam, ColorKind::Mellow));
    assert!(can_take_pill(&contam, ColorKind::Violent));
}

#[test]
fn combat_contamination_not_counted_as_drug() {
    let mut contam = fresh_contam();
    contam.entries.push(ContamSource {
        amount: 2.0,
        color: ColorKind::Mellow,
        meridian_id: None,
        attacker_id: Some("offline:Attacker".into()), // 战斗来源
        introduced_at: 0,
    });
    assert!(can_take_pill(&contam, ColorKind::Mellow));
    assert_eq!(sum_drug_toxin(&contam, ColorKind::Mellow), 0.0);
}

#[test]
fn overdose_penalty_scales_with_excess() {
    let mut contam = fresh_contam();
    contam.entries.push(ContamSource {
        amount: 1.5, // 超 0.5
        color: ColorKind::Violent,
        meridian_id: None,
        attacker_id: None,
        introduced_at: 0,
    });
    let severity = overdose_penalty(&contam, ColorKind::Violent);
    assert!((severity - 0.05).abs() < 1e-9);
}

#[test]
fn overdose_penalty_zero_below_threshold() {
    let mut contam = fresh_contam();
    contam.entries.push(ContamSource {
        amount: 0.8,
        color: ColorKind::Violent,
        meridian_id: None,
        attacker_id: None,
        introduced_at: 0,
    });
    assert_eq!(overdose_penalty(&contam, ColorKind::Violent), 0.0);
}

// ============== M5b Spoil 分支 ==============

#[test]
fn consume_pill_spoil_safe_same_as_normal() {
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::Safe { current_qi: 80.0 },
        false,
        AgePeakCheck::NotApplicable,
    );
    assert_eq!(outcome.qi_gained, 24.0);
    assert!(!outcome.blocked);
    assert_eq!(outcome.extra_toxin_added, 0.0);
    assert_eq!(contam.entries.len(), 1);
}

#[test]
fn consume_pill_spoil_warn_adds_extra_contam() {
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    // current=25, threshold=50 → ratio=0.5 → extra = 0.3 × 0.5 × 1.0 = 0.15
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::Warn {
            current_qi: 25.0,
            spoil_threshold: 50.0,
        },
        false,
        AgePeakCheck::NotApplicable,
    );
    assert_eq!(outcome.qi_gained, 24.0);
    assert!(!outcome.blocked);
    assert!((outcome.extra_toxin_added - 0.15).abs() < 1e-9);
    assert_eq!(contam.entries.len(), 2);
    // 第二条 entry 应为 extra toxin，color 同基础
    assert_eq!(contam.entries[1].color, ColorKind::Mellow);
    assert!((contam.entries[1].amount - 0.15).abs() < 1e-9);
}

#[test]
fn consume_pill_spoil_warn_edge_current_equals_threshold_zero_extra() {
    // current ≈ threshold → ratio=0 → extra=0（即便是 Warn 档亦然，边界场景）
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::Warn {
            current_qi: 50.0,
            spoil_threshold: 50.0,
        },
        false,
        AgePeakCheck::NotApplicable,
    );
    assert_eq!(outcome.extra_toxin_added, 0.0);
    assert_eq!(contam.entries.len(), 1); // 仅基础，无 extra
}

#[test]
fn consume_pill_spoil_warn_near_critical_near_full_extra() {
    // current=5, threshold=50 → ratio=0.9 → extra = 0.3 × 0.9 × 1.0 = 0.27
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::Warn {
            current_qi: 5.0,
            spoil_threshold: 50.0,
        },
        false,
        AgePeakCheck::NotApplicable,
    );
    assert!((outcome.extra_toxin_added - 0.27).abs() < 1e-9);
    assert_eq!(contam.entries.len(), 2);
}

#[test]
fn consume_pill_spoil_critical_block_refuses_all_effects() {
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::CriticalBlock {
            current_qi: 2.0,
            spoil_threshold: 50.0,
        },
        false,
        AgePeakCheck::NotApplicable,
    );
    assert_eq!(outcome.qi_gained, 0.0);
    assert!(outcome.blocked);
    assert_eq!(outcome.extra_toxin_added, 0.0);
    // 无 contam 新增，qi 不变
    assert_eq!(contam.entries.len(), 0);
    assert_eq!(cult.qi_current, 50.0);
}

#[test]
fn consume_pill_spoil_critical_block_force_consume_goes_through() {
    // Codex P2 (PR #38) 回归：CriticalBlock + force_consume=true 应按 Warn 公式消费，
    // 不再永久 blocked；plan §5.2 "拒绝自动消费，需玩家二次确认"的二次确认路径。
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 100.0,
        ..Default::default()
    };
    // current=2, threshold=50 → ratio=0.96 → extra = 0.3 × 0.96 × 1.0 = 0.288
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::CriticalBlock {
            current_qi: 2.0,
            spoil_threshold: 50.0,
        },
        true,
        AgePeakCheck::NotApplicable,
    );
    assert!(!outcome.blocked, "force_consume should bypass block");
    assert_eq!(outcome.qi_gained, 24.0);
    assert!((outcome.extra_toxin_added - 0.288).abs() < 1e-9);
    // 基础 + extra = 2 条 contam
    assert_eq!(contam.entries.len(), 2);
    assert_eq!(cult.qi_current, 74.0);
}

#[test]
fn consume_pill_force_consume_noop_when_not_critical() {
    // Safe / Warn / NotApplicable 下 force_consume 应无副作用（行为一致）
    let mut contam_a = fresh_contam();
    let mut cult_a = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let mut contam_b = fresh_contam();
    let mut cult_b = cult_a.clone();

    let a = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam_a,
        &mut cult_a,
        10,
        SpoilCheckOutcome::Safe { current_qi: 80.0 },
        false,
        AgePeakCheck::NotApplicable,
    );
    let b = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam_b,
        &mut cult_b,
        10,
        SpoilCheckOutcome::Safe { current_qi: 80.0 },
        true,
        AgePeakCheck::NotApplicable,
    );
    assert_eq!(a, b);
    assert_eq!(cult_a.qi_current, cult_b.qi_current);
    assert_eq!(contam_a.entries.len(), contam_b.entries.len());
}

#[test]
fn consume_pill_spoil_warn_zero_threshold_defensive() {
    // 防御性：malformed spoil_threshold=0 时 ratio=1.0（完全腐败），不除零 panic
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::Warn {
            current_qi: 0.0,
            spoil_threshold: 0.0,
        },
        false,
        AgePeakCheck::NotApplicable,
    );
    assert!((outcome.extra_toxin_added - 0.3).abs() < 1e-9);
}

// ============== M5d Age Peaking 分支 ==============

#[test]
fn age_peaking_applies_qi_bonus() {
    // Peaking bonus_strength=0.5 → qi_gain 24 × (1 + 0.5) = 36
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::NotApplicable,
        false,
        AgePeakCheck::Peaking {
            bonus_strength: 0.5,
        },
    );
    assert_eq!(outcome.qi_gained, 36.0);
    assert_eq!(outcome.age_bonus_applied, Some(0.5));
    assert!(!outcome.blocked);
    assert_eq!(cult.qi_current, 36.0);
}

#[test]
fn age_not_peaking_no_bonus() {
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::NotApplicable,
        false,
        AgePeakCheck::NotPeaking,
    );
    assert_eq!(outcome.qi_gained, 24.0);
    assert_eq!(outcome.age_bonus_applied, None);
}

#[test]
fn age_peaking_respects_qi_max_clamp() {
    // qi_max=100, qi_current=90, qi_gain=50 × 1.5 = 75 → 实际补 10
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 90.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(50.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::NotApplicable,
        false,
        AgePeakCheck::Peaking {
            bonus_strength: 0.5,
        },
    );
    assert_eq!(outcome.qi_gained, 10.0);
    assert_eq!(outcome.age_bonus_applied, Some(0.5));
    assert_eq!(cult.qi_current, 100.0);
}

#[test]
fn blocked_suppresses_age_bonus() {
    // CriticalBlock + !force：blocked=true 且 age_bonus_applied=None（无消费 = 无加成）。
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::CriticalBlock {
            current_qi: 2.0,
            spoil_threshold: 50.0,
        },
        false,
        AgePeakCheck::Peaking {
            bonus_strength: 0.5,
        },
    );
    assert!(outcome.blocked);
    assert_eq!(outcome.qi_gained, 0.0);
    assert_eq!(outcome.age_bonus_applied, None);
    assert_eq!(cult.qi_current, 50.0);
}

#[test]
fn age_peaking_stacks_with_spoil_warn() {
    // 同时 Warn（额外 contam）和 Peaking（qi bonus）：两种效果叠加。
    let mut contam = fresh_contam();
    let mut cult = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    // Warn: current=25, threshold=50 → extra = 0.3 × 0.5 × 1.0 = 0.15
    // Peaking: bonus=0.5 → qi_gain = 24 × 1.5 = 36
    let outcome = consume_pill(
        &basic_effect(Some(24.0)),
        &mut contam,
        &mut cult,
        10,
        SpoilCheckOutcome::Warn {
            current_qi: 25.0,
            spoil_threshold: 50.0,
        },
        false,
        AgePeakCheck::Peaking {
            bonus_strength: 0.5,
        },
    );
    assert_eq!(outcome.qi_gained, 36.0);
    assert!((outcome.extra_toxin_added - 0.15).abs() < 1e-9);
    assert_eq!(outcome.age_bonus_applied, Some(0.5));
    assert_eq!(contam.entries.len(), 2);
}

#[test]
fn mortal_pill_realm_scale_matches_combat_plan_breakpoints() {
    assert_eq!(mortal_pill_realm_scale(Realm::Awaken), (1.0, 1.0));
    assert_eq!(mortal_pill_realm_scale(Realm::Solidify), (0.5, 0.8));
    assert_eq!(mortal_pill_realm_scale(Realm::Spirit), (0.15, 0.6));
    assert_eq!(mortal_pill_realm_scale(Realm::Void), (0.05, 0.4));
    assert_eq!(
        scaled_grades(1, 0.15),
        0,
        "通灵服活血丹的凡药恢复等级应衰减到 0"
    );
    assert_eq!(
        scaled_grades(1, 0.4),
        0,
        "化虚服缩地散的腿伤副作用应衰减到 0"
    );
}

#[test]
fn wound_heal_ignores_severed_like_wounds() {
    let mut wounds = Wounds {
        health_current: 50.0,
        ..Default::default()
    };
    wounds.entries.push(Wound {
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL),
        kind: WoundKind::Cut,
        severity: 0.90,
        bleeding_per_sec: 1.0,
        created_at_tick: 0,
        inflicted_by: None,
    });
    wounds.entries.push(Wound {
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
        kind: WoundKind::Cut,
        severity: 0.50,
        bleeding_per_sec: 1.0,
        created_at_tick: 0,
        inflicted_by: None,
    });

    let changed = apply_wound_heal(&mut wounds, None, 1);

    assert_eq!(changed, 1);
    assert!(wounds.entries.iter().any(|wound| {
        wound.location == crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL)
            && (wound.severity - 0.90).abs() < 1e-6
    }));
    assert!(wounds.entries.iter().any(|wound| {
        wound.location == crate::body_plan::legacy_body_part_to_id(BodyPart::Chest)
            && (wound.severity - 0.25).abs() < 1e-6
    }));
}

#[test]
fn severed_mend_downgrades_only_severed_target() {
    let mut wounds = Wounds::default();
    wounds.entries.push(Wound {
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmR),
        kind: WoundKind::Cut,
        severity: 0.92,
        bleeding_per_sec: 2.0,
        created_at_tick: 0,
        inflicted_by: None,
    });

    assert!(apply_severed_mend(&mut wounds, Some(BodyPart::ArmR), 1.0));

    let wound = &wounds.entries[0];
    assert_eq!(
        wound.location,
        crate::body_plan::legacy_body_part_to_id(BodyPart::ArmR)
    );
    assert_eq!(wound.kind, WoundKind::Concussion);
    assert!((wound.severity - 0.55).abs() < 1e-6);
    assert!((wound.bleeding_per_sec - 0.7).abs() < 1e-6);
}

#[test]
fn combat_pill_status_intents_scale_resist_and_qi_drain() {
    let entity = valence::prelude::Entity::from_raw(7);
    let tie_bi = combat_pill_spec("tie_bi_san").unwrap();
    let tie_bi_intents = combat_pill_status_intents(entity, tie_bi, 0.5, 0.8, 10);
    assert!(tie_bi_intents.iter().any(|intent| {
        intent.kind == StatusEffectKind::BodyPartResist(BodyPart::Chest)
            && (intent.magnitude - 0.20).abs() < 1e-6
    }));

    let hui_li = combat_pill_spec("hui_li_dan").unwrap();
    let hui_li_intents = combat_pill_status_intents(entity, hui_li, 1.0, 0.6, 10);
    assert!(hui_li_intents.iter().any(|intent| {
        intent.kind == StatusEffectKind::QiDrainForStamina && (intent.magnitude - 1.2).abs() < 1e-6
    }));
}

// ══════════════════════════════════════════════════════════════════
// plan-cultivation-pacing-v1 P1.4–P1.6 修炼丹药验收测试
// ══════════════════════════════════════════════════════════════════

use crate::combat::components::StatusEffects;

fn fresh_status_effects() -> StatusEffects {
    StatusEffects::default()
}

// ── §1 CultivationPillKind enum + spec pin 测试 ──

#[test]
fn cultivation_pill_ids_count_is_eight() {
    assert_eq!(CULTIVATION_PILL_IDS.len(), 8);
}

#[test]
fn all_cultivation_pill_ids_resolve_to_spec() {
    for id in &CULTIVATION_PILL_IDS {
        let spec = cultivation_pill_spec(id);
        assert!(
            spec.is_some(),
            "cultivation_pill_spec(\"{id}\") should return Some"
        );
        let spec = spec.unwrap();
        assert_eq!(spec.id, *id);
    }
}

#[test]
fn unknown_cultivation_pill_id_returns_none() {
    assert!(cultivation_pill_spec("nonexistent_pill").is_none());
    assert!(cultivation_pill_spec("huo_xue_dan").is_none()); // combat pill, not cultivation
}

#[test]
fn ling_xi_wan_spec_pin() {
    let s = cultivation_pill_spec("ling_xi_wan").unwrap();
    assert_eq!(s.kind, CultivationPillKind::LingXiWan);
    assert_eq!(s.name, "灵息丸");
    assert!((s.toxin_amount - 0.15).abs() < 1e-9);
    assert_eq!(s.toxin_color, ColorKind::Gentle);
}

#[test]
fn ju_ling_dan_spec_pin() {
    let s = cultivation_pill_spec("ju_ling_dan").unwrap();
    assert_eq!(s.kind, CultivationPillKind::JuLingDan);
    assert!((s.toxin_amount - 0.20).abs() < 1e-9);
    assert_eq!(s.toxin_color, ColorKind::Mellow);
}

#[test]
fn tong_mai_san_spec_pin() {
    let s = cultivation_pill_spec("tong_mai_san").unwrap();
    assert_eq!(s.kind, CultivationPillKind::TongMaiSan);
    assert!((s.toxin_amount - 0.30).abs() < 1e-9);
    assert_eq!(s.toxin_color, ColorKind::Solid);
}

#[test]
fn ning_yuan_dan_spec_pin() {
    let s = cultivation_pill_spec("ning_yuan_dan").unwrap();
    assert_eq!(s.kind, CultivationPillKind::NingYuanDan);
    assert!((s.toxin_amount - 0.35).abs() < 1e-9);
    assert_eq!(s.toxin_color, ColorKind::Heavy);
}

#[test]
fn xi_sui_ye_spec_pin() {
    let s = cultivation_pill_spec("xi_sui_ye").unwrap();
    assert_eq!(s.kind, CultivationPillKind::XiSuiYe);
    assert!((s.toxin_amount - 0.40).abs() < 1e-9);
    assert_eq!(s.toxin_color, ColorKind::Violent);
}

#[test]
fn po_jing_dan_spec_pin() {
    let s = cultivation_pill_spec("po_jing_dan").unwrap();
    assert_eq!(s.kind, CultivationPillKind::PoJingDan);
    assert!((s.toxin_amount - 0.45).abs() < 1e-9);
    assert_eq!(s.toxin_color, ColorKind::Insidious);
}

#[test]
fn kai_qiao_dan_spec_pin() {
    let s = cultivation_pill_spec("kai_qiao_dan").unwrap();
    assert_eq!(s.kind, CultivationPillKind::KaiQiaoDan);
    assert!((s.toxin_amount - 0.50).abs() < 1e-9);
    assert_eq!(s.toxin_color, ColorKind::Turbid);
}

#[test]
fn du_jie_dan_spec_pin() {
    let s = cultivation_pill_spec("du_jie_dan").unwrap();
    assert_eq!(s.kind, CultivationPillKind::DuJieDan);
    assert!((s.toxin_amount - 0.60).abs() < 1e-9);
    assert_eq!(s.toxin_color, ColorKind::Insidious);
}

// ── §2 cultivation_pill_effects pin 测试 ──

#[test]
fn ling_xi_wan_effects_single_cultivation_acceleration() {
    let effects = cultivation_pill_effects(CultivationPillKind::LingXiWan);
    assert_eq!(effects.len(), 1, "灵息丸应有 1 个 effect");
    assert_eq!(effects[0].kind, StatusEffectKind::CultivationAcceleration);
    assert!((effects[0].magnitude - 0.5).abs() < 1e-6);
    assert_eq!(effects[0].duration_ticks, 36_000);
}

#[test]
fn ju_ling_dan_effects_single_cultivation_acceleration() {
    let effects = cultivation_pill_effects(CultivationPillKind::JuLingDan);
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].kind, StatusEffectKind::CultivationAcceleration);
    assert!((effects[0].magnitude - 1.0).abs() < 1e-6);
    assert_eq!(effects[0].duration_ticks, 24_000);
}

#[test]
fn tong_mai_san_effects_single_cultivation_acceleration() {
    let effects = cultivation_pill_effects(CultivationPillKind::TongMaiSan);
    assert_eq!(effects.len(), 1);
    assert!((effects[0].magnitude - 1.5).abs() < 1e-6);
    assert_eq!(effects[0].duration_ticks, 18_000);
}

#[test]
fn ning_yuan_dan_effects_dual_accel_plus_breakthrough() {
    let effects = cultivation_pill_effects(CultivationPillKind::NingYuanDan);
    assert_eq!(effects.len(), 2, "凝元丹应有 2 个 effect");
    let accel = effects
        .iter()
        .find(|e| e.kind == StatusEffectKind::CultivationAcceleration)
        .expect("应含 CultivationAcceleration");
    assert!((accel.magnitude - 2.0).abs() < 1e-6);
    assert_eq!(accel.duration_ticks, 18_000);
    let bt = effects
        .iter()
        .find(|e| e.kind == StatusEffectKind::BreakthroughBoost)
        .expect("应含 BreakthroughBoost");
    assert!((bt.magnitude - 0.10).abs() < 1e-6);
}

#[test]
fn xi_sui_ye_effects_accel_plus_vulnerability() {
    let effects = cultivation_pill_effects(CultivationPillKind::XiSuiYe);
    assert_eq!(effects.len(), 2, "洗髓液应有 2 个 effect");
    let accel = effects
        .iter()
        .find(|e| e.kind == StatusEffectKind::CultivationAcceleration)
        .expect("应含 CultivationAcceleration");
    assert!((accel.magnitude - 3.0).abs() < 1e-6);
    assert_eq!(accel.duration_ticks, 12_000);
    let vuln = effects
        .iter()
        .find(|e| e.kind == StatusEffectKind::DamageVulnerability)
        .expect("应含 DamageVulnerability");
    assert!((vuln.magnitude - 1.0).abs() < 1e-6);
    assert_eq!(vuln.duration_ticks, 12_000);
}

#[test]
fn po_jing_dan_effects_single_breakthrough_boost() {
    let effects = cultivation_pill_effects(CultivationPillKind::PoJingDan);
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].kind, StatusEffectKind::BreakthroughBoost);
    assert!((effects[0].magnitude - 0.20).abs() < 1e-6);
}

#[test]
fn kai_qiao_dan_effects_single_extraordinary_meridian() {
    let effects = cultivation_pill_effects(CultivationPillKind::KaiQiaoDan);
    assert_eq!(effects.len(), 1);
    assert_eq!(
        effects[0].kind,
        StatusEffectKind::ExtraordinaryMeridianAcceleration
    );
    assert!((effects[0].magnitude - 4.0).abs() < 1e-6);
    assert_eq!(effects[0].duration_ticks, 12_000);
}

#[test]
fn du_jie_dan_effects_breakthrough_plus_damage_reduction() {
    let effects = cultivation_pill_effects(CultivationPillKind::DuJieDan);
    assert_eq!(effects.len(), 2, "渡劫丹应有 2 个 effect");
    let bt = effects
        .iter()
        .find(|e| e.kind == StatusEffectKind::BreakthroughBoost)
        .expect("应含 BreakthroughBoost");
    assert!((bt.magnitude - 0.25).abs() < 1e-6);
    assert_eq!(bt.duration_ticks, u64::MAX, "渡劫丹应持续全程");
    let dr = effects
        .iter()
        .find(|e| e.kind == StatusEffectKind::DamageReduction)
        .expect("应含 DamageReduction(0.30)");
    assert!((dr.magnitude - 0.30).abs() < 1e-6);
}

// ── §3 consume_cultivation_pill 测试 ──

#[test]
fn consume_ling_xi_wan_mounts_cultivation_acceleration() {
    let spec = cultivation_pill_spec("ling_xi_wan").unwrap();
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 100);

    assert_eq!(
        result.applied_effects.len(),
        1,
        "灵息丸应挂载 1 个 CultivationAcceleration"
    );
    assert_eq!(
        result.applied_effects[0].kind,
        StatusEffectKind::CultivationAcceleration
    );
    assert!((result.applied_effects[0].magnitude - 0.5).abs() < 1e-6);
    assert_eq!(result.applied_effects[0].duration_ticks, 36_000);

    // 验证丹毒注入
    assert!((result.toxin_injected - 0.15).abs() < 1e-9);
    assert_eq!(contam.entries.len(), 1);
    assert_eq!(contam.entries[0].color, ColorKind::Gentle);

    // 验证 StatusEffects 内的 ActiveStatusEffect
    assert_eq!(se.active.len(), 1);
    assert_eq!(se.active[0].kind, StatusEffectKind::CultivationAcceleration);
    assert_eq!(se.active[0].source_pill.as_deref(), Some("ling_xi_wan"));
    assert_eq!(se.active[0].remaining_ticks, 36_000);

    // 无堆叠拦截
    assert_eq!(result.blocked_by_cap, 0);
    // 非洗髓液，无延迟 debuff
    assert!(result.deferred_qi_regen_slowed.is_none());
}

#[test]
fn consume_ning_yuan_dan_mounts_dual_effects() {
    let spec = cultivation_pill_spec("ning_yuan_dan").unwrap();
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 200);

    assert_eq!(
        result.applied_effects.len(),
        2,
        "凝元丹应挂载 CultivationAcceleration + BreakthroughBoost"
    );
    assert!(result
        .applied_effects
        .iter()
        .any(|e| e.kind == StatusEffectKind::CultivationAcceleration
            && (e.magnitude - 2.0).abs() < 1e-6));
    assert!(result.applied_effects.iter().any(
        |e| e.kind == StatusEffectKind::BreakthroughBoost && (e.magnitude - 0.10).abs() < 1e-6
    ));

    assert_eq!(se.active.len(), 2);
    assert!((result.toxin_injected - 0.35).abs() < 1e-9);
}

#[test]
fn consume_xi_sui_ye_mounts_accel_plus_vulnerability() {
    let spec = cultivation_pill_spec("xi_sui_ye").unwrap();
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 300);

    assert_eq!(result.applied_effects.len(), 2);
    assert!(result
        .applied_effects
        .iter()
        .any(|e| e.kind == StatusEffectKind::CultivationAcceleration
            && (e.magnitude - 3.0).abs() < 1e-6));
    assert!(result.applied_effects.iter().any(
        |e| e.kind == StatusEffectKind::DamageVulnerability && (e.magnitude - 1.0).abs() < 1e-6
    ));

    // 洗髓液应有延迟 debuff
    let deferred = result
        .deferred_qi_regen_slowed
        .expect("洗髓液应有 deferred_qi_regen_slowed");
    assert!((deferred.magnitude - 0.8).abs() < 1e-6);
    assert_eq!(deferred.duration_ticks, 12_000);
}

#[test]
fn consume_kai_qiao_dan_mounts_extraordinary_meridian_acceleration() {
    let spec = cultivation_pill_spec("kai_qiao_dan").unwrap();
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 400);

    assert_eq!(result.applied_effects.len(), 1);
    assert_eq!(
        result.applied_effects[0].kind,
        StatusEffectKind::ExtraordinaryMeridianAcceleration
    );
    assert!((result.applied_effects[0].magnitude - 4.0).abs() < 1e-6);
    assert_eq!(result.applied_effects[0].duration_ticks, 12_000);
}

#[test]
fn consume_du_jie_dan_mounts_breakthrough_and_damage_reduction() {
    let spec = cultivation_pill_spec("du_jie_dan").unwrap();
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 500);

    assert_eq!(result.applied_effects.len(), 2);
    assert!(result.applied_effects.iter().any(
        |e| e.kind == StatusEffectKind::BreakthroughBoost && (e.magnitude - 0.25).abs() < 1e-6
    ));
    assert!(result
        .applied_effects
        .iter()
        .any(|e| e.kind == StatusEffectKind::DamageReduction && (e.magnitude - 0.30).abs() < 1e-6));
    assert!((result.toxin_injected - 0.60).abs() < 1e-9);
}

#[test]
fn consume_po_jing_dan_mounts_single_breakthrough_boost() {
    let spec = cultivation_pill_spec("po_jing_dan").unwrap();
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 600);

    assert_eq!(result.applied_effects.len(), 1);
    assert_eq!(
        result.applied_effects[0].kind,
        StatusEffectKind::BreakthroughBoost
    );
    assert!((result.applied_effects[0].magnitude - 0.20).abs() < 1e-6);
}

// ── §4 堆叠 cap 测试 ──

#[test]
fn same_pill_third_dose_blocked_by_per_pill_cap() {
    let spec = cultivation_pill_spec("ling_xi_wan").unwrap();
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    // 第 1 颗
    let r1 = consume_cultivation_pill(&spec, &mut contam, &mut se, 100);
    assert_eq!(r1.blocked_by_cap, 0);
    assert_eq!(se.active.len(), 1);

    // 第 2 颗
    let r2 = consume_cultivation_pill(&spec, &mut contam, &mut se, 200);
    assert_eq!(r2.blocked_by_cap, 0);
    assert_eq!(se.active.len(), 2);

    // 第 3 颗——被拦截
    let r3 = consume_cultivation_pill(&spec, &mut contam, &mut se, 300);
    assert_eq!(
        r3.blocked_by_cap, 1,
        "同种丹药第 3 颗应被 per-pill 2 层 cap 拦截"
    );
    assert_eq!(r3.applied_effects.len(), 0);
    // effects 仍然只有 2 条
    assert_eq!(se.active.len(), 2);
    // 但丹毒仍然注入（吞了但 effect 不生效）
    assert_eq!(contam.entries.len(), 3, "丹毒应照常注入即使 effect 被拦截");
}

#[test]
fn same_pill_magnitude_not_aggregated_on_third() {
    // 灵息丸 ×3 = 只有 2×0.5=1.0 加速（不是 1.5）
    let spec = cultivation_pill_spec("ling_xi_wan").unwrap();
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    consume_cultivation_pill(&spec, &mut contam, &mut se, 100);
    consume_cultivation_pill(&spec, &mut contam, &mut se, 200);
    consume_cultivation_pill(&spec, &mut contam, &mut se, 300);

    let total_mag: f32 = se
        .active
        .iter()
        .filter(|e| e.kind == StatusEffectKind::CultivationAcceleration)
        .map(|e| e.magnitude)
        .sum();
    assert!(
        (total_mag - 1.0).abs() < 1e-6,
        "灵息丸 ×3 生效 magnitude 总和应为 2×0.5=1.0，不是 1.5；实际为 {total_mag}"
    );
}

#[test]
fn different_pills_not_blocked_by_each_other() {
    let ling_xi = cultivation_pill_spec("ling_xi_wan").unwrap();
    let ju_ling = cultivation_pill_spec("ju_ling_dan").unwrap();
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    // ling_xi_wan ×2
    consume_cultivation_pill(&ling_xi, &mut contam, &mut se, 100);
    consume_cultivation_pill(&ling_xi, &mut contam, &mut se, 200);

    // ju_ling_dan ×1 — 不应被 ling_xi_wan cap 拦截
    let r = consume_cultivation_pill(&ju_ling, &mut contam, &mut se, 300);
    assert_eq!(
        r.blocked_by_cap, 0,
        "不同丹药不应被其他丹药的 per-pill cap 拦截"
    );
    assert_eq!(se.active.len(), 3);
}

#[test]
fn different_toxin_colors_accumulate_independently() {
    // Gentle + Solid 各自独立累积
    let ling_xi = cultivation_pill_spec("ling_xi_wan").unwrap(); // Gentle
    let tong_mai = cultivation_pill_spec("tong_mai_san").unwrap(); // Solid
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    consume_cultivation_pill(&ling_xi, &mut contam, &mut se, 100);
    consume_cultivation_pill(&tong_mai, &mut contam, &mut se, 200);

    let gentle_total = sum_drug_toxin(&contam, ColorKind::Gentle);
    let solid_total = sum_drug_toxin(&contam, ColorKind::Solid);
    assert!((gentle_total - 0.15).abs() < 1e-9, "Gentle 应独立累积");
    assert!((solid_total - 0.30).abs() < 1e-9, "Solid 应独立累积");
    // 两色互不干扰——各自仍可继续服药
    assert!(can_take_pill(&contam, ColorKind::Gentle));
    assert!(can_take_pill(&contam, ColorKind::Solid));
}

// ── §5 can_take_pill 回归测试 ──

#[test]
fn can_take_pill_blocks_when_same_color_at_threshold() {
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();

    // 连服 7 颗灵息丸（each 0.15），总毒 1.05 ≥ 1.0
    let spec = cultivation_pill_spec("ling_xi_wan").unwrap();
    for _ in 0..7 {
        consume_cultivation_pill(&spec, &mut contam, &mut se, 0);
    }

    let total = sum_drug_toxin(&contam, ColorKind::Gentle);
    assert!(
        total >= TOXIN_THRESHOLD,
        "7 颗灵息丸丹毒 {total} 应超阈值 {TOXIN_THRESHOLD}"
    );
    assert!(
        !can_take_pill(&contam, ColorKind::Gentle),
        "丹毒超阈值后 can_take_pill 应返回 false"
    );
    // 其他颜色仍可
    assert!(can_take_pill(&contam, ColorKind::Mellow));
}

// ── §6 洗髓液到期回调测试 ──

#[test]
fn xi_sui_ye_expiry_pushes_qi_regen_slowed() {
    use crate::combat::components::ActiveStatusEffect;
    let mut se = StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::CultivationAcceleration,
            magnitude: 3.0,
            remaining_ticks: 0, // 刚刚到期
            source_pill: Some("xi_sui_ye".to_string()),
        }],
    };

    let triggered = check_xi_sui_ye_expiry_and_push_debuff(&mut se);
    assert!(triggered, "xi_sui_ye 到期应触发 QiRegenSlowed 追加");

    // 到期的 CultivationAcceleration 还在（remaining=0），加上新的 QiRegenSlowed
    let slowed = se
        .active
        .iter()
        .find(|e| e.kind == StatusEffectKind::QiRegenSlowed);
    assert!(slowed.is_some(), "应追加 QiRegenSlowed effect");
    let slowed = slowed.unwrap();
    assert!((slowed.magnitude - 0.8).abs() < 1e-6);
    assert_eq!(slowed.remaining_ticks, 12_000);
    assert_eq!(slowed.source_pill.as_deref(), Some("xi_sui_ye"));
}

#[test]
fn xi_sui_ye_expiry_not_triggered_when_still_active() {
    use crate::combat::components::ActiveStatusEffect;
    let mut se = StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::CultivationAcceleration,
            magnitude: 3.0,
            remaining_ticks: 100, // 仍有效
            source_pill: Some("xi_sui_ye".to_string()),
        }],
    };

    let triggered = check_xi_sui_ye_expiry_and_push_debuff(&mut se);
    assert!(!triggered, "xi_sui_ye 仍有效时不应触发 QiRegenSlowed 追加");
    assert_eq!(se.active.len(), 1, "不应追加任何 effect");
}

#[test]
fn xi_sui_ye_expiry_not_triggered_for_other_pill() {
    use crate::combat::components::ActiveStatusEffect;
    let mut se = StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::CultivationAcceleration,
            magnitude: 0.5,
            remaining_ticks: 0,
            source_pill: Some("ling_xi_wan".to_string()), // 不是 xi_sui_ye
        }],
    };

    let triggered = check_xi_sui_ye_expiry_and_push_debuff(&mut se);
    assert!(!triggered, "非 xi_sui_ye 丹药到期不应触发回调");
}

#[test]
fn xi_sui_ye_expiry_not_triggered_for_no_source_pill() {
    use crate::combat::components::ActiveStatusEffect;
    let mut se = StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::CultivationAcceleration,
            magnitude: 3.0,
            remaining_ticks: 0,
            source_pill: None,
        }],
    };

    let triggered = check_xi_sui_ye_expiry_and_push_debuff(&mut se);
    assert!(!triggered, "source_pill=None 不应触发洗髓液回调");
}

// ── §7 P1.6 meridian_progress_bonus 测试 ──

#[test]
fn activate_meridian_progress_bonus_mounts_cultivation_acceleration() {
    let effect = PillEffect {
        toxin_amount: 0.2,
        toxin_color: ColorKind::Mellow,
        qi_gain: Some(24.0),
        meridian_progress_bonus: Some(1.5),
    };
    let mut se = fresh_status_effects();

    let result = activate_meridian_progress_bonus(&effect, &mut se, "test_pill");
    assert!(result, "meridian_progress_bonus>0 应成功挂载");
    assert_eq!(se.active.len(), 1);
    assert_eq!(se.active[0].kind, StatusEffectKind::CultivationAcceleration);
    assert!((se.active[0].magnitude - 1.5).abs() < 1e-6);
    assert_eq!(se.active[0].source_pill.as_deref(), Some("test_pill"));
}

#[test]
fn activate_meridian_progress_bonus_none_does_nothing() {
    let effect = PillEffect {
        toxin_amount: 0.2,
        toxin_color: ColorKind::Mellow,
        qi_gain: Some(24.0),
        meridian_progress_bonus: None,
    };
    let mut se = fresh_status_effects();

    let result = activate_meridian_progress_bonus(&effect, &mut se, "test_pill");
    assert!(!result, "meridian_progress_bonus=None 应返回 false");
    assert!(se.active.is_empty());
}

#[test]
fn activate_meridian_progress_bonus_zero_does_nothing() {
    let effect = PillEffect {
        toxin_amount: 0.2,
        toxin_color: ColorKind::Mellow,
        qi_gain: None,
        meridian_progress_bonus: Some(0.0),
    };
    let mut se = fresh_status_effects();

    let result = activate_meridian_progress_bonus(&effect, &mut se, "test_pill");
    assert!(!result, "meridian_progress_bonus=0 应返回 false");
}

#[test]
fn activate_meridian_progress_bonus_negative_does_nothing() {
    let effect = PillEffect {
        toxin_amount: 0.2,
        toxin_color: ColorKind::Mellow,
        qi_gain: None,
        meridian_progress_bonus: Some(-1.0),
    };
    let mut se = fresh_status_effects();

    let result = activate_meridian_progress_bonus(&effect, &mut se, "test_pill");
    assert!(!result, "meridian_progress_bonus<0 应返回 false");
}

// ── §8 丹方 JSON 加载测试 ──

#[test]
fn all_eight_cultivation_recipes_load_from_json() {
    let registry = crate::alchemy::recipe::load_recipe_registry().unwrap();
    for id in &[
        "ling_xi_wan_v1",
        "ju_ling_dan_v1",
        "tong_mai_san_v1",
        "ning_yuan_dan_v1",
        "xi_sui_ye_v1",
        "po_jing_dan_v1",
        "kai_qiao_dan_v1",
        "du_jie_dan_v1",
    ] {
        assert!(
            registry.get(id).is_some(),
            "配方 {id} 应存在于 RecipeRegistry 中"
        );
    }
}

#[test]
fn ling_xi_wan_recipe_structure_valid() {
    let registry = crate::alchemy::recipe::load_recipe_registry().unwrap();
    let r = registry.get("ling_xi_wan_v1").unwrap();
    assert_eq!(r.name, "灵息丸·入门修炼丹");
    assert_eq!(r.furnace_tier_min, 1);
    assert_eq!(r.stages.len(), 1);
    assert_eq!(r.stages[0].required.len(), 1);
    assert_eq!(r.stages[0].required[0].material, "spirit_grass");
    assert_eq!(r.stages[0].required[0].count, 3);
    let perfect = r.outcomes.perfect.as_ref().expect("should have perfect");
    assert_eq!(perfect.pill, "ling_xi_wan");
    assert!((perfect.toxin_amount - 0.15).abs() < 1e-9);
    assert_eq!(perfect.toxin_color, ColorKind::Gentle);
}

#[test]
fn du_jie_dan_recipe_requires_tier_3() {
    let registry = crate::alchemy::recipe::load_recipe_registry().unwrap();
    let r = registry.get("du_jie_dan_v1").unwrap();
    assert_eq!(r.furnace_tier_min, 3, "渡劫丹应需要 3 级炉");
    // 材料检查
    let stage0 = r.stage0_ingredients();
    assert_eq!(stage0.get("long_lin_tai"), Some(&1));
    assert_eq!(stage0.get("xu_yuan_rui"), Some(&1));
    assert_eq!(stage0.get("ling_shi"), Some(&3));
}

#[test]
fn xi_sui_ye_recipe_requires_tier_2() {
    let registry = crate::alchemy::recipe::load_recipe_registry().unwrap();
    let r = registry.get("xi_sui_ye_v1").unwrap();
    assert_eq!(r.furnace_tier_min, 2, "洗髓液应需要 2 级炉");
}

// ── §9 聚灵丹 delta 验证（zone_qi=0.6 下首条正经应 ≤30 min）──

#[test]
fn ju_ling_dan_with_zone_qi_first_meridian_under_30min() {
    const THIRTY_MINUTES_TICKS: u64 = 30 * 60 * crate::combat::components::TICKS_PER_SECOND;

    // 使用 PR-1 的 cultivation_acceleration_multiplier 验证：
    // 聚灵丹 mag=1.0 → multiplier=(1+1.0)=2.0。这个中间量断言保留，
    // 但下面还会通过真实的 meridian_open_tick 锁住首经在 30 分钟内打通。
    use crate::combat::CombatClock;
    use crate::cultivation::components::{MeridianId, MeridianSystem};
    use crate::cultivation::life_record::LifeRecord;
    use crate::cultivation::meridian_open::{meridian_open_tick, MeridianTarget};
    use crate::cultivation::tick::cultivation_acceleration_multiplier;
    use crate::cultivation::tick::CultivationClock;
    use crate::qi_physics::WorldQiAccount;
    use valence::prelude::{App, IntoSystemConfigs, Position, Update};

    let mut pill_statuses = fresh_status_effects();
    let mut contam = fresh_contam();
    let pill = cultivation_pill_spec("ju_ling_dan").expect("聚灵丹规格应存在");
    let consumed = consume_cultivation_pill(&pill, &mut contam, &mut pill_statuses, 0);
    assert_eq!(
        consumed.applied_effects.len(),
        1,
        "聚灵丹应真实挂载加速状态"
    );

    let mult = cultivation_acceleration_multiplier(&pill_statuses);
    assert!(
        (mult - 2.0).abs() < 1e-9,
        "聚灵丹 mag=1.0 应给 2× 修炼加速；实际为 {mult}"
    );

    fn first_meridian_open_tick(with_ju_ling_dan: bool) -> (Option<u64>, f64) {
        const ZONE_QI: f64 = 0.6;

        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(WorldQiAccount::default());

        let mut zones = crate::world::zone::ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = ZONE_QI;
        app.insert_resource(zones);

        // 与生产生命周期一致：药效按 CombatClock 到期，开脉在每次 Update 消费当前
        // StatusEffects。两个系统的显式顺序让到期 tick 先移除状态，再推进开脉。
        app.add_systems(Update, crate::combat::status::status_effect_tick);
        app.add_systems(
            Update,
            meridian_open_tick.after(crate::combat::status::status_effect_tick),
        );

        let mut statuses = fresh_status_effects();
        if with_ju_ling_dan {
            let mut contam = fresh_contam();
            let pill = cultivation_pill_spec("ju_ling_dan").expect("聚灵丹规格应存在");
            let result = consume_cultivation_pill(&pill, &mut contam, &mut statuses, 0);
            assert_eq!(
                result.applied_effects.len(),
                1,
                "正向场景必须通过真实消费流程挂载聚灵丹状态"
            );
        }

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                MeridianTarget(MeridianId::Lung.channel_id()),
                Cultivation {
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                statuses,
                LifeRecord::new(if with_ju_ling_dan {
                    "ju-ling-dan-first-meridian"
                } else {
                    "no-pill-first-meridian-control"
                }),
            ))
            .id();

        for tick in 1..=THIRTY_MINUTES_TICKS {
            app.world_mut().resource_mut::<CultivationClock>().tick = tick;
            app.world_mut().resource_mut::<CombatClock>().tick = tick;
            app.update();

            let meridians = app.world().entity(player).get::<MeridianSystem>().unwrap();
            if meridians.get(MeridianId::Lung).opened {
                return (Some(tick), meridians.get(MeridianId::Lung).open_progress);
            }
        }

        let meridians = app.world().entity(player).get::<MeridianSystem>().unwrap();
        (None, meridians.get(MeridianId::Lung).open_progress)
    }

    let (pill_opened_at, pill_progress) = first_meridian_open_tick(true);
    let pill_opened_at = pill_opened_at.expect("服下聚灵丹后首条正经应在 30 分钟内打通");
    assert!(
        pill_opened_at <= THIRTY_MINUTES_TICKS,
        "聚灵丹首经打通 tick={pill_opened_at} 不应超过 30 分钟 tick 上限"
    );
    assert!(
        pill_progress >= 1.0,
        "首经打通时 open_progress 应达到 1.0，实际 {pill_progress}"
    );

    let (control_opened_at, control_progress) = first_meridian_open_tick(false);
    assert!(
        control_opened_at.is_none(),
        "无聚灵丹对照在 30 分钟内不应打通首经，实际 tick={control_opened_at:?}"
    );
    assert!(
        control_progress > 0.0 && control_progress < 1.0,
        "无丹对照必须真实推进但尚未打通，实际 open_progress={control_progress}"
    );
}

// ── §10 全 8 种丹药消费后的 source_pill 字段正确 ──

#[test]
fn all_pills_set_source_pill_on_status_effects() {
    for id in &CULTIVATION_PILL_IDS {
        let spec = cultivation_pill_spec(id).unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 0);

        for active in &se.active {
            assert_eq!(
                active.source_pill.as_deref(),
                Some(*id),
                "pill {id} 的 StatusEffect.source_pill 应为 {id}"
            );
        }
        // 至少挂载了 1 个 effect（无堆叠限制首次服用）
        assert!(
            !result.applied_effects.is_empty(),
            "首颗 {id} 应至少挂载 1 个 effect"
        );
    }
}

// ── plan-cultivation-pacing-v1 P2.2 flawed 丹药测试 ──

#[test]
fn flawed_ling_xi_wan_spec_resolves() {
    let s = cultivation_pill_spec("ling_xi_wan_flawed").unwrap();
    assert_eq!(
        s.kind,
        CultivationPillKind::LingXiWan,
        "次品灵息丸 kind 应为 LingXiWan"
    );
    assert_eq!(s.name, "灵息丸（次品）");
    assert!(
        (s.toxin_amount - 0.15).abs() < 1e-9,
        "次品灵息丸丹毒与正品相同（0.15）"
    );
    assert_eq!(s.toxin_color, ColorKind::Gentle);
}

#[test]
fn flawed_ju_ling_dan_spec_resolves() {
    let s = cultivation_pill_spec("ju_ling_dan_flawed").unwrap();
    assert_eq!(
        s.kind,
        CultivationPillKind::JuLingDan,
        "次品聚灵丹 kind 应为 JuLingDan"
    );
    assert_eq!(s.name, "聚灵丹（次品）");
    assert!(
        (s.toxin_amount - 0.20).abs() < 1e-9,
        "次品聚灵丹丹毒与正品相同（0.20）"
    );
}

#[test]
fn is_flawed_cultivation_pill_detects_suffix() {
    assert!(is_flawed_cultivation_pill("ling_xi_wan_flawed"));
    assert!(is_flawed_cultivation_pill("ju_ling_dan_flawed"));
    assert!(!is_flawed_cultivation_pill("ling_xi_wan"));
    assert!(!is_flawed_cultivation_pill("ju_ling_dan"));
    assert!(!is_flawed_cultivation_pill("flawed_ling_xi_wan")); // prefix doesn't count
}

#[test]
fn flawed_ling_xi_wan_magnitude_is_0_6x_normal() {
    let normal = cultivation_pill_spec("ling_xi_wan").unwrap();
    let flawed = cultivation_pill_spec("ling_xi_wan_flawed").unwrap();

    let mut normal_contam = fresh_contam();
    let mut normal_se = fresh_status_effects();
    consume_cultivation_pill(&normal, &mut normal_contam, &mut normal_se, 0);

    let mut flawed_contam = fresh_contam();
    let mut flawed_se = fresh_status_effects();
    consume_cultivation_pill(&flawed, &mut flawed_contam, &mut flawed_se, 0);

    assert!(!normal_se.active.is_empty(), "正品应挂载 effect");
    assert!(!flawed_se.active.is_empty(), "次品应挂载 effect");

    let normal_mag = normal_se.active[0].magnitude;
    let flawed_mag = flawed_se.active[0].magnitude;

    // 正品 mag=0.5，次品 mag=0.5×0.6=0.3
    assert!(
        (normal_mag - 0.5).abs() < 1e-6,
        "正品灵息丸 magnitude 应为 0.5，实际 {normal_mag}"
    );
    assert!(
        (flawed_mag - 0.3).abs() < 1e-6,
        "次品灵息丸 magnitude 应为 0.3（0.5×0.6），实际 {flawed_mag}"
    );
}

#[test]
fn flawed_ju_ling_dan_magnitude_is_0_6x_normal() {
    let normal = cultivation_pill_spec("ju_ling_dan").unwrap();
    let flawed = cultivation_pill_spec("ju_ling_dan_flawed").unwrap();

    let mut normal_contam = fresh_contam();
    let mut normal_se = fresh_status_effects();
    consume_cultivation_pill(&normal, &mut normal_contam, &mut normal_se, 0);

    let mut flawed_contam = fresh_contam();
    let mut flawed_se = fresh_status_effects();
    consume_cultivation_pill(&flawed, &mut flawed_contam, &mut flawed_se, 0);

    let normal_mag = normal_se.active[0].magnitude;
    let flawed_mag = flawed_se.active[0].magnitude;

    // 正品 mag=1.0，次品 mag=1.0×0.6=0.6
    assert!(
        (normal_mag - 1.0).abs() < 1e-6,
        "正品聚灵丹 magnitude 应为 1.0，实际 {normal_mag}"
    );
    assert!(
        (flawed_mag - 0.6).abs() < 1e-6,
        "次品聚灵丹 magnitude 应为 0.6（1.0×0.6），实际 {flawed_mag}"
    );
}

#[test]
fn flawed_pill_same_duration_as_normal() {
    let normal = cultivation_pill_spec("ling_xi_wan").unwrap();
    let flawed = cultivation_pill_spec("ling_xi_wan_flawed").unwrap();

    let mut nc = fresh_contam();
    let mut nse = fresh_status_effects();
    consume_cultivation_pill(&normal, &mut nc, &mut nse, 0);

    let mut fc = fresh_contam();
    let mut fse = fresh_status_effects();
    consume_cultivation_pill(&flawed, &mut fc, &mut fse, 0);

    assert_eq!(
        nse.active[0].remaining_ticks, fse.active[0].remaining_ticks,
        "次品丹药 duration 应与正品相同"
    );
}

#[test]
fn flawed_pill_source_pill_contains_flawed_suffix() {
    let spec = cultivation_pill_spec("ling_xi_wan_flawed").unwrap();
    let mut contam = fresh_contam();
    let mut se = fresh_status_effects();
    consume_cultivation_pill(&spec, &mut contam, &mut se, 0);

    assert_eq!(
        se.active[0].source_pill.as_deref(),
        Some("ling_xi_wan_flawed"),
        "次品丹药 source_pill 应为 ling_xi_wan_flawed"
    );
}

#[test]
fn flawed_pill_acceleration_multiplier_lower_than_normal() {
    use crate::cultivation::tick::cultivation_acceleration_multiplier;

    // 正品灵息丸 → 1.5× accel
    let spec_normal = cultivation_pill_spec("ling_xi_wan").unwrap();
    let mut nc = fresh_contam();
    let mut nse = fresh_status_effects();
    consume_cultivation_pill(&spec_normal, &mut nc, &mut nse, 0);
    let mult_normal = cultivation_acceleration_multiplier(&nse);

    // 次品灵息丸 → 1.3× accel
    let spec_flawed = cultivation_pill_spec("ling_xi_wan_flawed").unwrap();
    let mut fc = fresh_contam();
    let mut fse = fresh_status_effects();
    consume_cultivation_pill(&spec_flawed, &mut fc, &mut fse, 0);
    let mult_flawed = cultivation_acceleration_multiplier(&fse);

    assert!(
        (mult_normal - 1.5).abs() < 1e-6,
        "正品灵息丸加速应为 1.5×，实际 {mult_normal}"
    );
    assert!(
        (mult_flawed - 1.3).abs() < 1e-4,
        "次品灵息丸加速应为 1.3×（0.5×0.6=0.3 → 1+0.3），实际 {mult_flawed}"
    );
}

// ══════════════════════════════════════════════════════════════════
// plan-depth-loop-v1 P3.1 — 战斗丹供给链材料可达性验证
// ══════════════════════════════════════════════════════════════════

#[test]
fn hui_yuan_zhi_spawns_in_marsh_zone() {
    use crate::botany::registry::{BotanyKindRegistry, BotanyPlantId, HUI_YUAN_ZHI};
    use crate::world::zone::BotanyZoneTag;

    let registry = BotanyKindRegistry::default();
    let kind = registry
        .get(BotanyPlantId::HuiYuanZhi)
        .expect("HuiYuanZhi should be registered in BotanyPlantKindRegistry");
    assert_eq!(
        kind.item_id, HUI_YUAN_ZHI,
        "HuiYuanZhi item_id should be '{HUI_YUAN_ZHI}'"
    );
    assert!(
        kind.zone_tags.contains(&BotanyZoneTag::Marsh),
        "HuiYuanZhi should spawn in Marsh zone (lingquan_marsh), got {:?}",
        kind.zone_tags
    );
}

#[test]
fn hui_yuan_pill_v0_recipe_materials_are_hui_yuan_zhi_and_ling_shui() {
    let recipe_json = include_str!("../../assets/alchemy/recipes/hui_yuan_pill_v0.json");
    let recipe: serde_json::Value =
        serde_json::from_str(recipe_json).expect("hui_yuan_pill_v0 recipe should parse");
    let materials: Vec<&str> = recipe["stages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stage| stage["required"].as_array().unwrap().iter())
        .map(|req| req["material"].as_str().unwrap())
        .collect();
    assert!(
        materials.contains(&"hui_yuan_zhi"),
        "hui_yuan_pill_v0 recipe must require hui_yuan_zhi, got {materials:?}"
    );
    assert!(
        materials.contains(&"ling_shui"),
        "hui_yuan_pill_v0 recipe must require ling_shui, got {materials:?}"
    );
}

#[test]
fn huo_xue_dan_recipe_materials_all_in_registry() {
    let recipe_json = include_str!("../../assets/alchemy/recipes/huo_xue_dan_v1.json");
    let recipe: serde_json::Value =
        serde_json::from_str(recipe_json).expect("huo_xue_dan_v1 recipe should parse");
    let materials: Vec<&str> = recipe["stages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stage| stage["required"].as_array().unwrap().iter())
        .map(|req| req["material"].as_str().unwrap())
        .collect();
    assert!(
        !materials.is_empty(),
        "huo_xue_dan_v1 recipe should have at least one material"
    );
    let items_toml_core = include_str!("../../assets/items/core.toml");
    let items_toml_botany = include_str!("../../assets/items/botany_v2.toml");
    let items_toml_pills = include_str!("../../assets/items/pills.toml");
    let combined = format!("{items_toml_core}\n{items_toml_botany}\n{items_toml_pills}");
    for mat in &materials {
        assert!(
            combined.contains(&format!("id = \"{mat}\"")),
            "huo_xue_dan material '{mat}' not found in item registry (core/botany_v2/pills.toml)"
        );
    }
}

#[test]
fn tie_bi_san_recipe_materials_all_in_registry() {
    let recipe_json = include_str!("../../assets/alchemy/recipes/tie_bi_san_v1.json");
    let recipe: serde_json::Value =
        serde_json::from_str(recipe_json).expect("tie_bi_san_v1 recipe should parse");
    let materials: Vec<&str> = recipe["stages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stage| stage["required"].as_array().unwrap().iter())
        .map(|req| req["material"].as_str().unwrap())
        .collect();
    assert!(
        !materials.is_empty(),
        "tie_bi_san_v1 recipe should have at least one material"
    );
    let items_toml_core = include_str!("../../assets/items/core.toml");
    let items_toml_fauna = include_str!("../../assets/items/fauna.toml");
    let items_toml_zhenfa = include_str!("../../assets/items/zhenfa.toml");
    let items_toml_forge = include_str!("../../assets/items/forge.toml");
    let combined =
        format!("{items_toml_core}\n{items_toml_fauna}\n{items_toml_zhenfa}\n{items_toml_forge}");
    for mat in &materials {
        assert!(
            combined.contains(&format!("id = \"{mat}\"")),
            "tie_bi_san material '{mat}' not found in item registry (core/fauna/zhenfa/forge.toml)"
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// plan-depth-loop-v1 P3.4 — pill buff status payload structure
// ══════════════════════════════════════════════════════════════════

#[test]
fn pill_buff_status_emitted_on_consume() {
    use crate::schema::server_data::PillBuffStatusV1;

    let spec = combat_pill_spec("huo_xue_dan").expect("huo_xue_dan should be a known combat pill");
    assert_eq!(
        spec.kind,
        CombatPillKind::HuoXueDan,
        "combat_pill_spec should resolve huo_xue_dan to HuoXueDan kind"
    );

    let entity = valence::prelude::Entity::from_raw(42);
    let intents = combat_pill_status_intents(entity, spec, 1.0, 1.0, 100);
    assert!(
        !intents.is_empty(),
        "huo_xue_dan should produce at least one status effect intent"
    );

    let buff = PillBuffStatusV1 {
        buff_id: "huo_xue_dan".to_string(),
        remaining_ticks: spec.positive_duration_ticks as u32,
        effect_multiplier: 1.0,
    };
    assert_eq!(buff.buff_id, "huo_xue_dan");
    assert!(
        buff.remaining_ticks > 0,
        "huo_xue_dan positive_duration_ticks should be > 0, got {}",
        buff.remaining_ticks
    );

    let envelope = crate::schema::server_data::ServerDataV1::new(
        crate::schema::server_data::ServerDataPayloadV1::PillBuffStatus(buff.clone()),
    );
    let bytes = serde_json::to_vec(&envelope).expect("PillBuffStatus envelope should serialize");
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        value["type"], "pill_buff_status",
        "wire type should be 'pill_buff_status'"
    );
    assert_eq!(value["buff_id"], "huo_xue_dan");
    assert_eq!(
        value["remaining_ticks"], spec.positive_duration_ticks,
        "remaining_ticks should match spec positive_duration_ticks"
    );
}

#[test]
fn tie_bi_san_combat_pill_spec_resolves() {
    let spec = combat_pill_spec("tie_bi_san").expect("tie_bi_san should be a known combat pill");
    assert_eq!(spec.kind, CombatPillKind::TieBiSan);

    let entity = valence::prelude::Entity::from_raw(99);
    let intents = combat_pill_status_intents(entity, spec, 1.0, 1.0, 0);
    assert!(
        !intents.is_empty(),
        "tie_bi_san should produce status effect intents"
    );
}

// ─── plan-depth-loop-v1 P5 — 循环闭合校准 e2e test ───

/// P5.1 E2E: Verify combat pill specs exist for known combat pills and
/// PillBuffStatusV1 can represent their buff state.
///
/// Validates the full pill-buff pipeline: spec lookup -> status intents ->
/// PillBuffStatusV1 schema serialization.
#[test]
fn e2e_pill_buff_active_during_combat() {
    use crate::schema::server_data::PillBuffStatusV1;

    // Verify combat pill specs exist for the two primary combat pills
    for pill_id in ["huo_xue_dan", "tie_bi_san"] {
        let spec = combat_pill_spec(pill_id).unwrap_or_else(|| {
            panic!(
                "combat_pill_spec(\"{pill_id}\") should return Some — \
                     {pill_id} is a primary combat pill required for the depth loop"
            )
        });
        assert_eq!(spec.id, pill_id, "spec.id should match the lookup key");

        // Verify the spec produces usable status effect intents
        let entity = valence::prelude::Entity::from_raw(42);
        let intents = combat_pill_status_intents(entity, spec, 1.0, 1.0, 0);
        assert!(
            !intents.is_empty(),
            "{pill_id} should produce at least one status effect intent — \
                 a combat pill with no effects would be useless in the depth loop"
        );

        // Verify positive_duration_ticks is non-zero (pills must have duration)
        assert!(
            spec.positive_duration_ticks > 0,
            "{pill_id} positive_duration_ticks should be > 0, got {} — \
                 a zero-duration buff cannot be \"active during combat\"",
            spec.positive_duration_ticks
        );

        // Verify PillBuffStatusV1 can represent this pill's buff state
        let buff_status = PillBuffStatusV1 {
            buff_id: pill_id.to_string(),
            remaining_ticks: spec.positive_duration_ticks as u32,
            effect_multiplier: 1.0,
        };
        let json = serde_json::to_string(&buff_status).unwrap_or_else(|e| {
            panic!("PillBuffStatusV1 for {pill_id} should serialize to JSON — {e}")
        });
        let roundtrip: PillBuffStatusV1 = serde_json::from_str(&json).unwrap_or_else(|e| {
            panic!("PillBuffStatusV1 for {pill_id} should roundtrip through JSON — {e}")
        });
        assert_eq!(
            roundtrip.buff_id, pill_id,
            "PillBuffStatusV1 roundtrip buff_id should be '{pill_id}'"
        );
        assert_eq!(
            roundtrip.remaining_ticks, spec.positive_duration_ticks as u32,
            "PillBuffStatusV1 roundtrip remaining_ticks should match spec"
        );
    }

    // Verify all 10 combat pills resolve
    for pill_id in &COMBAT_PILL_IDS {
        assert!(
            combat_pill_spec(pill_id).is_some(),
            "combat_pill_spec(\"{pill_id}\") should resolve — \
                 all {n} combat pills must be available for the depth loop",
            n = COMBAT_PILL_IDS.len()
        );
    }
}
