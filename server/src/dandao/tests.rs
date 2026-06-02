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

// ============================================================
// P0 runtime wiring — plan-dandao-runtime-wiring-v1
// ============================================================

// ===== register pin =====
// 调 dandao::register 后，两个 Event 资源必须存在；
// track_pill_intake_system 行为：emit PillIntakeTracked → update → PracticeLog 记到 Mellow。
#[cfg(test)]
mod p0_wiring_tests {
    use valence::prelude::{App, Entity, Events, Update};

    use crate::cultivation::color::PracticeLog;
    use crate::cultivation::components::ColorKind;
    use crate::cultivation::tick::CultivationClock;
    use crate::dandao::components::{DandaoStyle, MutationStage, MUTATION_STAGE_THRESHOLDS};
    use crate::dandao::mutation::{MutationAdvanceEvent, MutationState};
    use crate::dandao::toxin_tracker::PillIntakeTracked;

    // ── register pin: 两 event 资源存在 ────────────────────────────────────────

    #[test]
    fn register_adds_pill_intake_tracked_event_resource() {
        let mut app = App::new();
        crate::dandao::register(&mut app);
        assert!(
            app.world()
                .get_resource::<Events<PillIntakeTracked>>()
                .is_some(),
            "dandao::register 应添加 Events::<PillIntakeTracked> 资源"
        );
    }

    #[test]
    fn register_adds_mutation_advance_event_resource() {
        let mut app = App::new();
        crate::dandao::register(&mut app);
        assert!(
            app.world()
                .get_resource::<Events<MutationAdvanceEvent>>()
                .is_some(),
            "dandao::register 应添加 Events::<MutationAdvanceEvent> 资源"
        );
    }

    // ── track_pill_intake_system 行为断言 ────────────────────────────────────

    /// emit PillIntakeTracked(toxin>0) → update → entity PracticeLog 追加 Mellow 权重
    #[test]
    fn track_pill_intake_system_records_mellow_on_toxin_event() {
        let mut app = App::new();
        app.add_systems(
            Update,
            crate::dandao::toxin_tracker::track_pill_intake_system,
        );
        app.add_event::<PillIntakeTracked>();

        let entity = app.world_mut().spawn(PracticeLog::default()).id();

        // Emit a PillIntakeTracked with positive toxin
        app.world_mut()
            .resource_mut::<Events<PillIntakeTracked>>()
            .send(PillIntakeTracked {
                entity,
                toxin_amount: 0.5,
                new_stage: None,
            });

        app.update();

        let log = app.world().entity(entity).get::<PracticeLog>().unwrap();
        let mellow = log.weights.get(&ColorKind::Mellow).copied().unwrap_or(0.0);
        assert!(
            mellow > 0.0,
            "track_pill_intake_system 应在 PracticeLog 记录 Mellow 权重，实际={mellow}"
        );
    }

    /// toxin_amount <= 0.0 时 system 应跳过，不修改 PracticeLog
    #[test]
    fn track_pill_intake_system_skips_zero_toxin_event() {
        let mut app = App::new();
        app.add_systems(
            Update,
            crate::dandao::toxin_tracker::track_pill_intake_system,
        );
        app.add_event::<PillIntakeTracked>();

        let entity = app.world_mut().spawn(PracticeLog::default()).id();

        app.world_mut()
            .resource_mut::<Events<PillIntakeTracked>>()
            .send(PillIntakeTracked {
                entity,
                toxin_amount: 0.0,
                new_stage: None,
            });

        app.update();

        let log = app.world().entity(entity).get::<PracticeLog>().unwrap();
        let mellow = log.weights.get(&ColorKind::Mellow).copied().unwrap_or(0.0);
        assert_eq!(
            mellow, 0.0,
            "toxin_amount=0.0 时不应在 PracticeLog 记录 Mellow，实际={mellow}"
        );
    }

    /// 多次服丹 → 每次都记录 Mellow（幂等累积）
    #[test]
    fn track_pill_intake_system_accumulates_multiple_events() {
        let mut app = App::new();
        app.add_systems(
            Update,
            crate::dandao::toxin_tracker::track_pill_intake_system,
        );
        app.add_event::<PillIntakeTracked>();

        let entity = app.world_mut().spawn(PracticeLog::default()).id();

        // Emit twice, update twice
        for _ in 0..2 {
            app.world_mut()
                .resource_mut::<Events<PillIntakeTracked>>()
                .send(PillIntakeTracked {
                    entity,
                    toxin_amount: 1.0,
                    new_stage: None,
                });
            app.update();
        }

        let log = app.world().entity(entity).get::<PracticeLog>().unwrap();
        let mellow = log.weights.get(&ColorKind::Mellow).copied().unwrap_or(0.0);
        assert!(mellow > 0.0, "多次服丹后 Mellow 权重应 > 0，实际={mellow}");
    }

    /// entity 无 PracticeLog 时，system 应静默跳过（不 panic）
    #[test]
    fn track_pill_intake_system_no_practice_log_no_panic() {
        let mut app = App::new();
        app.add_systems(
            Update,
            crate::dandao::toxin_tracker::track_pill_intake_system,
        );
        app.add_event::<PillIntakeTracked>();

        // Entity without PracticeLog
        let entity = app.world_mut().spawn(()).id();

        app.world_mut()
            .resource_mut::<Events<PillIntakeTracked>>()
            .send(PillIntakeTracked {
                entity,
                toxin_amount: 0.5,
                new_stage: None,
            });

        // Should not panic
        app.update();
    }

    // ── writer（生产路径）逻辑单测 ────────────────────────────────────────────
    // handle_alchemy_take_pill 是 private fn，无法直接测。
    // 测 toxin_for_intake 的来源：combat_pill_spec 对已知 pill 返回正 toxin_amount。

    /// 已知 combat pill 有正 toxin_amount（代表 CombatPill 路径会 emit PillIntakeTracked）
    #[test]
    fn combat_pill_spec_toxin_amount_positive_for_known_pills() {
        // 按 COMBAT_PILL_IDS 中每个 pill 验证 spec.toxin_amount > 0
        for id in crate::alchemy::pill::COMBAT_PILL_IDS {
            let spec = crate::alchemy::pill::combat_pill_spec(id)
                .unwrap_or_else(|| panic!("combat_pill_spec({id}) 应返回 Some"));
            assert!(
                spec.toxin_amount > 0.0,
                "CombatPill `{id}` 的 toxin_amount 应 > 0（代表服丹会 emit PillIntakeTracked），实际={:.3}",
                spec.toxin_amount
            );
        }
    }

    /// 未知 pill_item_id → spec 返回 None → toxin_for_intake = 0.0 → 不 emit
    #[test]
    fn unknown_pill_id_gives_zero_toxin() {
        let result = crate::alchemy::pill::combat_pill_spec("nonexistent_pill");
        assert!(result.is_none(), "未知 pill id 应返回 None");
        // toxin_for_intake = spec.map(...).unwrap_or(0.0) = 0.0
    }

    /// PillIntakeTracked 事件结构：toxin_amount=0.0 时 system 跳过（边界）
    #[test]
    fn pill_intake_tracked_zero_toxin_is_boundary_case() {
        let event = PillIntakeTracked {
            entity: Entity::PLACEHOLDER,
            toxin_amount: 0.0,
            new_stage: None,
        };
        assert_eq!(event.toxin_amount, 0.0);
        // track_pill_intake_system 对 toxin_amount <= 0.0 跳过
    }

    /// PillIntakeTracked 携带 new_stage 字段（阶段跃迁路径）
    #[test]
    fn pill_intake_tracked_with_new_stage() {
        let event = PillIntakeTracked {
            entity: Entity::PLACEHOLDER,
            toxin_amount: 1.5,
            new_stage: Some(1),
        };
        assert_eq!(event.new_stage, Some(1));
        assert!(event.toxin_amount > 0.0);
    }

    // ── 变异链（register 接通后，advance_toxin 喂 toxin → mutation_advance_system）──

    /// advance_toxin 越过 stage 1 阈值 → mutation_advance_system 推进 MutationState + emit MutationAdvanceEvent
    #[test]
    fn mutation_advance_system_advances_stage_on_threshold_cross() {
        let mut app = App::new();
        // 使用 register 确保系统和事件都就绪
        crate::dandao::register(&mut app);
        // mutation_advance_system 还需要 InsightRequest event（EventWriter）
        app.add_event::<crate::cultivation::insight::InsightRequest>();
        // mutation_advance_system 需要 CultivationClock（tick 必须整除 600）
        app.insert_resource(CultivationClock { tick: 0 });

        let mut style = DandaoStyle::default();
        // 超过阶段 1 阈值
        style.advance_toxin(MUTATION_STAGE_THRESHOLDS[0] + 1.0);

        let entity = app.world_mut().spawn(style).id();

        app.update();

        // MutationState 应被插入且 stage = Subtle
        let mutation_state = app.world().entity(entity).get::<MutationState>();
        assert!(
            mutation_state.is_some(),
            "advance_toxin 跨越阈值后 mutation_advance_system 应插入 MutationState"
        );
        assert_eq!(
            mutation_state.unwrap().stage,
            MutationStage::Subtle,
            "阶段 1 阈值后应为 Subtle"
        );

        // MutationAdvanceEvent 应被 emit
        let events = app.world().resource::<Events<MutationAdvanceEvent>>();
        let count = events.iter_current_update_events().count();
        assert_eq!(
            count, 1,
            "跨越阈值应 emit 1 条 MutationAdvanceEvent，实际={count}"
        );
    }

    /// 未越阈值 → mutation_advance_system 不插入 MutationState，不 emit 事件
    #[test]
    fn mutation_advance_system_does_not_advance_below_threshold() {
        let mut app = App::new();
        crate::dandao::register(&mut app);
        app.add_event::<crate::cultivation::insight::InsightRequest>();
        app.insert_resource(CultivationClock { tick: 0 });

        let mut style = DandaoStyle::default();
        // 不超过阶段 1 阈值
        style.advance_toxin(MUTATION_STAGE_THRESHOLDS[0] - 1.0);

        let entity = app.world_mut().spawn(style).id();
        app.update();

        let mutation_state = app.world().entity(entity).get::<MutationState>();
        assert!(
            mutation_state.is_none(),
            "未跨越阈值时不应插入 MutationState"
        );

        let events = app.world().resource::<Events<MutationAdvanceEvent>>();
        let count = events.iter_current_update_events().count();
        assert_eq!(
            count, 0,
            "未跨越阈值时不应 emit MutationAdvanceEvent，实际={count}"
        );
    }

    /// stage N → N+1 精确推进（阶段 1 → 2）
    #[test]
    fn mutation_advance_system_advances_stage1_to_stage2() {
        let mut app = App::new();
        crate::dandao::register(&mut app);
        app.add_event::<crate::cultivation::insight::InsightRequest>();
        app.insert_resource(CultivationClock { tick: 0 });

        // 已处于阶段 1，cumulative_toxin 超过阶段 2 阈值
        let style = DandaoStyle {
            cumulative_toxin: MUTATION_STAGE_THRESHOLDS[1] + 1.0,
            mutation_stage: 1,
            ..DandaoStyle::default()
        };
        let existing_state = {
            let mut s = MutationState::default();
            s.advance_to(MutationStage::Subtle);
            s
        };
        let entity = app.world_mut().spawn((style, existing_state)).id();

        app.update();

        let state = app.world().entity(entity).get::<MutationState>().unwrap();
        assert_eq!(
            state.stage,
            MutationStage::Visible,
            "阶段 1→2 推进后应为 Visible"
        );
    }

    /// 阈值边界：恰好在阈值上 → 应推进（不是 just-below）
    #[test]
    fn mutation_advance_system_advances_at_exact_threshold() {
        let mut app = App::new();
        crate::dandao::register(&mut app);
        app.add_event::<crate::cultivation::insight::InsightRequest>();
        app.insert_resource(CultivationClock { tick: 0 });

        let mut style = DandaoStyle::default();
        style.advance_toxin(MUTATION_STAGE_THRESHOLDS[0]); // 恰好在阈值

        let entity = app.world_mut().spawn(style).id();
        app.update();

        let mutation_state = app.world().entity(entity).get::<MutationState>();
        assert!(
            mutation_state.is_some(),
            "cumulative_toxin 恰好等于阈值时应推进 MutationState"
        );
    }

    /// tick 节流：非整除 600 的 tick 时，mutation_advance_system 不运行
    #[test]
    fn mutation_advance_system_throttled_at_non_600_tick() {
        let mut app = App::new();
        crate::dandao::register(&mut app);
        app.add_event::<crate::cultivation::insight::InsightRequest>();
        // tick=1（非整除 600），system 应跳过
        app.insert_resource(CultivationClock { tick: 1 });

        let mut style = DandaoStyle::default();
        style.advance_toxin(MUTATION_STAGE_THRESHOLDS[3] + 100.0); // 超越最高阈值

        let entity = app.world_mut().spawn(style).id();
        app.update();

        // 因为 tick=1 不整除 600，系统不运行，MutationState 不被插入
        let mutation_state = app.world().entity(entity).get::<MutationState>();
        assert!(
            mutation_state.is_none(),
            "非 600 整除 tick 时 mutation_advance_system 应因节流跳过，不插入 MutationState"
        );
    }

    // ── 守恒回归：consume_pill 既有 qi 恢复行为不变 ────────────────────────────

    /// consume_pill 正常路径：qi 恢复 + contam 追加（守恒回归 — P0 不动 consume_pill）
    #[test]
    fn consume_pill_conserved_qi_recovery_regression() {
        use crate::alchemy::pill::{consume_pill, PillEffect};
        use crate::cultivation::components::ColorKind;
        use crate::cultivation::components::{Contamination, Cultivation};
        use crate::shelflife::{AgePeakCheck, SpoilCheckOutcome};

        let mut contam = Contamination::default();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let effect = PillEffect {
            toxin_amount: 0.3,
            toxin_color: ColorKind::Mellow,
            qi_gain: Some(30.0),
            meridian_progress_bonus: None,
        };
        let outcome = consume_pill(
            &effect,
            &mut contam,
            &mut cult,
            0,
            SpoilCheckOutcome::NotApplicable,
            false,
            AgePeakCheck::NotApplicable,
        );
        assert_eq!(
            outcome.qi_gained, 30.0,
            "守恒回归: consume_pill qi 恢复应为 30.0（P0 未改动 consume_pill 逻辑）"
        );
        assert!(!outcome.blocked, "守恒回归: blocked 应为 false");
        assert_eq!(
            cult.qi_current, 30.0,
            "守恒回归: qi_current 应等于 qi_gain=30.0"
        );
        assert_eq!(
            contam.entries.len(),
            1,
            "守恒回归: 应 push 1 条 ContamSource"
        );
    }

    /// consume_pill CriticalBlock + !force_consume → blocked，qi 不变（守恒回归）
    #[test]
    fn consume_pill_critical_block_no_force_is_blocked_regression() {
        use crate::alchemy::pill::{consume_pill, PillEffect};
        use crate::cultivation::components::ColorKind;
        use crate::cultivation::components::{Contamination, Cultivation};
        use crate::shelflife::{AgePeakCheck, SpoilCheckOutcome};

        let mut contam = Contamination::default();
        let mut cult = Cultivation {
            qi_current: 50.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let effect = PillEffect {
            toxin_amount: 0.3,
            toxin_color: ColorKind::Mellow,
            qi_gain: Some(10.0),
            meridian_progress_bonus: None,
        };
        let outcome = consume_pill(
            &effect,
            &mut contam,
            &mut cult,
            0,
            SpoilCheckOutcome::CriticalBlock {
                current_qi: 50.0,
                spoil_threshold: 100.0,
            },
            false,
            AgePeakCheck::NotApplicable,
        );
        assert!(
            outcome.blocked,
            "守恒回归: CriticalBlock+!force 应 blocked=true"
        );
        assert_eq!(
            cult.qi_current, 50.0,
            "守恒回归: blocked 时 qi_current 不应改变"
        );
        assert!(
            contam.entries.is_empty(),
            "守恒回归: blocked 时不应 push ContamSource"
        );
    }
}
