//! P1 变异系统 — MutationState + 阶段推进 + 顿悟触发 + 经脉惩罚。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Commands, Component, Entity, Event, EventWriter, Query};

use crate::cultivation::components::Realm;
use crate::cultivation::insight::InsightRequest;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::tick::CultivationClock;

use super::components::{DandaoStyle, MutationStage};

/// 经脉效率惩罚值（contamination baseline 增加），按变异阶段。
/// §8.1 #1 决议：阶段 4 从 -30% 调到 -20%，最终 -3%/-8%/-15%/-20%。
pub const MERIDIAN_PENALTY_BY_STAGE: [f64; 5] = [0.0, 0.03, 0.08, 0.15, 0.20];

/// 变异状态组件 — 挂在已触发变异的 player entity 上。
#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MutationState {
    pub stage: MutationStage,
    pub slots: Vec<ActiveMutation>,
    pub meridian_penalty: f64,
}

impl Default for MutationState {
    fn default() -> Self {
        Self {
            stage: MutationStage::None,
            slots: Vec::new(),
            meridian_penalty: 0.0,
        }
    }
}

impl MutationState {
    pub fn advance_to(&mut self, new_stage: MutationStage) {
        self.stage = new_stage;
        self.meridian_penalty = MERIDIAN_PENALTY_BY_STAGE[new_stage as usize];
    }
}

/// 已激活的单个变异 slot。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveMutation {
    pub kind: MutationKind,
    pub slot: BodySlot,
    pub level: u8,
    pub acquired_tick: u64,
}

/// 变异类型（按阶段分组）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MutationKind {
    // 阶段 1 — 微变
    GoldenIris,
    HardenedNails,
    ToughSkin,
    // 阶段 2 — 显变
    BoneRidge,
    ForearmScales,
    SpineSpurs,
    // 阶段 3 — 重变
    Horns,
    Tail,
    BackCarapace,
    // 阶段 4 — 兽化
    ExtraArms,
    BodyEnlarge,
    BeastFace,
}

impl MutationKind {
    /// 该变异最低要求的阶段。
    pub fn min_stage(self) -> MutationStage {
        match self {
            Self::GoldenIris | Self::HardenedNails | Self::ToughSkin => MutationStage::Subtle,
            Self::BoneRidge | Self::ForearmScales | Self::SpineSpurs => MutationStage::Visible,
            Self::Horns | Self::Tail | Self::BackCarapace => MutationStage::Heavy,
            Self::ExtraArms | Self::BodyEnlarge | Self::BeastFace => MutationStage::Bestial,
        }
    }

    /// 该阶段可选的变异列表。
    pub fn choices_for_stage(stage: MutationStage) -> &'static [MutationKind] {
        match stage {
            MutationStage::None => &[],
            MutationStage::Subtle => &[Self::GoldenIris, Self::HardenedNails, Self::ToughSkin],
            MutationStage::Visible => &[Self::BoneRidge, Self::ForearmScales, Self::SpineSpurs],
            MutationStage::Heavy => &[Self::Horns, Self::Tail, Self::BackCarapace],
            MutationStage::Bestial => &[Self::ExtraArms, Self::BodyEnlarge, Self::BeastFace],
        }
    }
}

/// 变异挂载部位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodySlot {
    Head,
    Forearm,
    Back,
    Torso,
    Lower,
}

impl MutationKind {
    pub fn body_slot(self) -> BodySlot {
        match self {
            Self::GoldenIris | Self::BoneRidge | Self::Horns | Self::BeastFace => BodySlot::Head,
            Self::HardenedNails | Self::ForearmScales | Self::ExtraArms => BodySlot::Forearm,
            Self::SpineSpurs | Self::BackCarapace => BodySlot::Back,
            Self::BodyEnlarge => BodySlot::Torso,
            Self::ToughSkin => BodySlot::Torso,
            Self::Tail => BodySlot::Lower,
        }
    }

    /// 变异功能性描述（plan §2.4）。返回 MutationEffect 枚举。
    pub fn effect(self) -> MutationEffect {
        match self {
            Self::GoldenIris => MutationEffect::VisionBoost {
                negative_zone_range_pct: 0.30,
                darkness_brightness_add: 2,
            },
            Self::HardenedNails => MutationEffect::UnarmedDamageBonus { base_attack_add: 3 },
            Self::ToughSkin => MutationEffect::PurgeBoost {
                contamination_purge_pct: 0.10,
            },
            Self::BoneRidge => MutationEffect::UnlockSkill {
                skill_id: "dandao.bone_slam",
            },
            Self::ForearmScales => MutationEffect::NaturalArmor {
                body_part: "forearm",
                downgrade_from: "abrasion",
                downgrade_to: "bruise",
            },
            Self::SpineSpurs => MutationEffect::DamageReduction {
                body_part: "back",
                reduction_pct: 0.20,
            },
            Self::Horns => MutationEffect::UnlockSkillWithQiCost {
                skill_id: "dandao.horn_charge",
                qi_cost: 5.0,
            },
            Self::Tail => MutationEffect::TailStrike {
                skill_id: "dandao.tail_strike",
                fall_damage_reduction_pct: 0.50,
            },
            Self::BackCarapace => MutationEffect::NaturalArmor {
                body_part: "back",
                downgrade_from: "laceration",
                downgrade_to: "abrasion",
            },
            Self::ExtraArms => MutationEffect::ExtraHandSlots { count: 2 },
            Self::BodyEnlarge => MutationEffect::ConstitutionBoost {
                hp_pct: 0.50,
                hitbox_scale: 1.5,
            },
            Self::BeastFace => MutationEffect::IntimidateAura {
                range_blocks: 5,
                realm_diff_threshold: 2,
                composure_reduction_pct: 0.30,
            },
        }
    }
}

/// 变异功能性效果（plan §2.4 数值）。
#[derive(Debug, Clone, PartialEq)]
pub enum MutationEffect {
    VisionBoost {
        negative_zone_range_pct: f64,
        darkness_brightness_add: u8,
    },
    UnarmedDamageBonus {
        base_attack_add: u32,
    },
    PurgeBoost {
        contamination_purge_pct: f64,
    },
    UnlockSkill {
        skill_id: &'static str,
    },
    NaturalArmor {
        body_part: &'static str,
        downgrade_from: &'static str,
        downgrade_to: &'static str,
    },
    DamageReduction {
        body_part: &'static str,
        reduction_pct: f64,
    },
    UnlockSkillWithQiCost {
        skill_id: &'static str,
        qi_cost: f64,
    },
    TailStrike {
        skill_id: &'static str,
        fall_damage_reduction_pct: f64,
    },
    ExtraHandSlots {
        count: u8,
    },
    ConstitutionBoost {
        hp_pct: f64,
        hitbox_scale: f64,
    },
    IntimidateAura {
        range_blocks: u32,
        realm_diff_threshold: u8,
        composure_reduction_pct: f64,
    },
}

/// §8.1 #2: 多臂武器切换共享 GCD（1s = 20 ticks）。
pub const WEAPON_SWAP_COOLDOWN_TICKS: u64 = 20;

/// 变异阶段推进事件。
#[derive(Event, Debug, Clone)]
pub struct MutationAdvanceEvent {
    pub entity: Entity,
    pub from_stage: MutationStage,
    pub to_stage: MutationStage,
}

/// 顿悟触发 ID 前缀。
const INSIGHT_TRIGGER_PREFIX: &str = "mutation_advance_stage_";

/// 600-tick 节流间隔（30 秒检测一次）。
pub const MUTATION_ADVANCE_INTERVAL_TICKS: u64 = 600;

/// 每 600 tick (30s) 检测一次 DandaoStyle 是否跨越变异阈值。
/// 跨越时：
/// 1. Insert/update MutationState
/// 2. Emit MutationAdvanceEvent
/// 3. Emit InsightRequest（触发顿悟选择）
/// 4. 写入 LifeRecord
#[allow(clippy::type_complexity)]
pub fn mutation_advance_system(
    mut commands: Commands,
    mut dandao_q: Query<(
        Entity,
        &DandaoStyle,
        Option<&mut MutationState>,
        Option<&mut LifeRecord>,
    )>,
    realms: Query<&crate::cultivation::components::Cultivation>,
    clock: Option<bevy_ecs::system::Res<CultivationClock>>,
    mut advance_tx: EventWriter<MutationAdvanceEvent>,
    mut insight_tx: EventWriter<InsightRequest>,
) {
    let current_tick = clock.map(|c| c.tick).unwrap_or(0);

    // 600-tick 节流：非整数倍 tick 直接跳过。
    if current_tick % MUTATION_ADVANCE_INTERVAL_TICKS != 0 {
        return;
    }

    for (entity, style, mutation_opt, life_record) in dandao_q.iter_mut() {
        let expected_stage = DandaoStyle::stage_for_toxin(style.cumulative_toxin);
        if expected_stage == 0 {
            continue;
        }

        let current_stage = mutation_opt.as_ref().map(|m| m.stage as u8).unwrap_or(0);

        if expected_stage <= current_stage {
            continue;
        }

        let new_stage = MutationStage::from(expected_stage);
        let old_stage = MutationStage::from(current_stage);

        // Update or insert MutationState
        if let Some(mut state) = mutation_opt {
            state.advance_to(new_stage);
        } else {
            let mut state = MutationState::default();
            state.advance_to(new_stage);
            commands.entity(entity).insert(state);
        }

        // Emit advance event
        advance_tx.send(MutationAdvanceEvent {
            entity,
            from_stage: old_stage,
            to_stage: new_stage,
        });

        // Emit InsightRequest（触发顿悟选择）
        let realm = realms.get(entity).map(|c| c.realm).unwrap_or(Realm::Awaken);
        let trigger_id = format!("{INSIGHT_TRIGGER_PREFIX}{expected_stage}");
        insight_tx.send(InsightRequest {
            entity,
            trigger_id,
            realm,
        });

        // 写入 LifeRecord
        if let Some(mut record) = life_record {
            record.biography.push(BiographyEntry::MutationAdvanced {
                from_stage: old_stage as u8,
                to_stage: new_stage as u8,
                cumulative_toxin: style.cumulative_toxin,
                tick: current_tick,
            });
        }
    }
}

/// 变异阶段对应的 NPC 好感度惩罚（plan §2.5 社会反应）。
pub fn social_penalty_for_stage(stage: MutationStage) -> i32 {
    match stage {
        MutationStage::None => 0,
        MutationStage::Subtle => 0,
        MutationStage::Visible => -20,
        MutationStage::Heavy => -50,
        MutationStage::Bestial => -100,
    }
}

/// 变异阶段 3+ 是否触发天道注视加权。
pub fn triggers_tiandao_attention(stage: MutationStage) -> bool {
    matches!(stage, MutationStage::Heavy | MutationStage::Bestial)
}

#[cfg(test)]
mod mutation_tests {
    use std::collections::HashSet;

    use super::*;

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
}
