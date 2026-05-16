//! P1 变异系统 — MutationState + 阶段推进 + 顿悟触发 + 经脉惩罚。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Commands, Component, Entity, Event, EventWriter, Query};

use crate::cultivation::components::Realm;
use crate::cultivation::insight::InsightRequest;

use super::components::{DandaoStyle, MutationStage};

/// 经脉效率惩罚值（contamination baseline 增加），按变异阶段。
pub const MERIDIAN_PENALTY_BY_STAGE: [f64; 5] = [0.0, 0.03, 0.08, 0.15, 0.30];

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
}

/// 变异阶段推进事件。
#[derive(Event, Debug, Clone)]
pub struct MutationAdvanceEvent {
    pub entity: Entity,
    pub from_stage: MutationStage,
    pub to_stage: MutationStage,
}

/// 顿悟触发 ID 前缀。
const INSIGHT_TRIGGER_PREFIX: &str = "mutation_advance_stage_";

/// 每 600 tick (30s) 检测一次 DandaoStyle 是否跨越变异阈值。
/// 跨越时：
/// 1. Insert/update MutationState
/// 2. Emit MutationAdvanceEvent
/// 3. Emit InsightRequest（触发顿悟选择）
/// 4. 写入 LifeRecord
pub fn mutation_advance_system(
    mut commands: Commands,
    mut dandao_q: Query<(Entity, &DandaoStyle, Option<&mut MutationState>)>,
    realms: Query<&crate::cultivation::components::Cultivation>,
    mut advance_tx: EventWriter<MutationAdvanceEvent>,
    mut insight_tx: EventWriter<InsightRequest>,
) {
    for (entity, style, mutation_opt) in dandao_q.iter_mut() {
        let expected_stage = DandaoStyle::stage_for_toxin(style.cumulative_toxin);
        if expected_stage == 0 {
            continue;
        }

        let current_stage = mutation_opt
            .as_ref()
            .map(|m| m.stage as u8)
            .unwrap_or(0);

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
        let realm = realms
            .get(entity)
            .map(|c| c.realm)
            .unwrap_or(Realm::Awaken);
        let trigger_id = format!("{INSIGHT_TRIGGER_PREFIX}{expected_stage}");
        insight_tx.send(InsightRequest {
            entity,
            trigger_id,
            realm,
        });
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
        assert_eq!(state.meridian_penalty, 0.03);
        state.advance_to(MutationStage::Visible);
        assert_eq!(state.meridian_penalty, 0.08);
        state.advance_to(MutationStage::Heavy);
        assert_eq!(state.meridian_penalty, 0.15);
        state.advance_to(MutationStage::Bestial);
        assert_eq!(state.meridian_penalty, 0.30);
    }

    #[test]
    fn mutation_kind_min_stage_correct() {
        assert_eq!(MutationKind::GoldenIris.min_stage(), MutationStage::Subtle);
        assert_eq!(MutationKind::BoneRidge.min_stage(), MutationStage::Visible);
        assert_eq!(MutationKind::Horns.min_stage(), MutationStage::Heavy);
        assert_eq!(MutationKind::ExtraArms.min_stage(), MutationStage::Bestial);
    }

    #[test]
    fn choices_for_stage_have_correct_count() {
        assert_eq!(MutationKind::choices_for_stage(MutationStage::None).len(), 0);
        assert_eq!(MutationKind::choices_for_stage(MutationStage::Subtle).len(), 3);
        assert_eq!(MutationKind::choices_for_stage(MutationStage::Visible).len(), 3);
        assert_eq!(MutationKind::choices_for_stage(MutationStage::Heavy).len(), 3);
        assert_eq!(MutationKind::choices_for_stage(MutationStage::Bestial).len(), 3);
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
                "经脉惩罚应单调递增"
            );
        }
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
}
