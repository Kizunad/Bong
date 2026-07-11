//! P1 变异系统 — MutationState + 阶段推进 + 顿悟触发 + 经脉惩罚。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Commands, Component, Entity, Event, EventWriter, Query};

use crate::body_plan::{body_part_for_mutation_slot, id_to_legacy_body_part, BodyPlan};
use crate::combat::components::BodyPart;
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

/// plan-race-system-v1 P0 review 修复（BLOCKING-2）—— humanoid.json 的
/// `mutation_slot_mapping`（`BodySlot → BodyPartId`）此前只在 [`body_part_for_mutation_slot`]
/// 内部有一条 API，全仓无任何运行时消费者（client `MutationFeatureRenderer` 尚未接线，
/// 依赖 P2 的 `body_plan_layout` payload 才能过 wire——本 plan 范围外）。本函数是**第一个
/// 真实 server 侧消费点**：给定目标实体已解析出的 Intrinsic [`BodyPlan`] + 其
/// [`MutationState`]，把每条 [`ActiveMutation::slot`] 经 [`body_part_for_mutation_slot`]
/// 解析成 `BodyPartId`，再经 [`id_to_legacy_body_part`] 转回 legacy [`BodyPart`]（战斗
/// wire / `Wounds.location` 现状仍是 legacy enum，见 `body_plan::legacy` 桥文档）——命中
/// 同一部位且该条 mutation 的 [`MutationKind::effect`] 是 `DamageReduction` 时叠乘减伤
/// 系数（`reduction_pct`），供 `combat::resolve::resolve_attack_intents` 与既有的
/// `combat::status::body_part_damage_multiplier`（丹药/状态效果驱动的同类型 per-part
/// 倍率）同一处叠加消费。
///
/// **消费点选择依据**：dandao 现状确实没有任何"按 `BodySlot` 定位身体部位并施加效果"
/// 的既有运行时逻辑——`MutationState.slots` 在生产代码里从未被写入过（变异获取/顿悟
/// 选择尚未接线到 `ActiveMutation` 落地这一步，是本 plan 范围外的既有缺口），
/// `network::mutation_visual_emit` / `dandao::visual_sync` 唯二引用 `BodySlot` 的地方
/// 只是把枚举变体名原样序列化成字符串发给 client 渲染，不查询 `mutation_slot_mapping`。
/// 因此本函数落在"最贴近的真实语义处"：`MutationEffect::DamageReduction` 早已声明
/// `body_part: &'static str` 字段却零消费（另一个既有孤岛），本函数用
/// `body_part_for_mutation_slot` 把它接上真实战斗结算，一次性补齐两处孤岛的交汇点。
/// `MutationEffect::NaturalArmor`（伤势分级降档，语义与"倍率"不同）不在本函数消费
/// 范围内——刻意保持零消费，非本次改动引入的新缺口，避免借题发挥造出未经设计评审的
/// 降档机制。
///
/// **悬空/缺失映射静默跳过**（不 panic）：`body_part_for_mutation_slot` 对未在
/// `mutation_slot_mapping` 里配置该 slot 的 plan 返回 `None` 是合法状态（见其文档——
/// 非 humanoid plan 留空合法）；`id_to_legacy_body_part` 对非 8 段 humanoid legacy
/// 字符串（如未来 whale 部位 id）同样返回 `None` 且合法。两种情况下该条 mutation
/// 对本次伤害结算无影响，不阻断其余 mutation 继续参与折算。
pub fn mutation_damage_multiplier_for_part(
    state: Option<&MutationState>,
    plan: &BodyPlan,
    part: BodyPart,
) -> f32 {
    let Some(state) = state else {
        return 1.0;
    };
    state.slots.iter().fold(1.0_f32, |acc, active| {
        let Some(mapped_id) = body_part_for_mutation_slot(plan, active.slot) else {
            return acc; // 悬空/缺失映射：静默跳过，不 panic。
        };
        let Some(mapped_part) = id_to_legacy_body_part(mapped_id) else {
            return acc; // 非人形 plan 部位无 legacy 对应物：静默跳过。
        };
        if mapped_part != part {
            return acc;
        }
        match active.kind.effect() {
            MutationEffect::DamageReduction { reduction_pct, .. } => {
                acc * (1.0 - reduction_pct.clamp(0.0, 1.0) as f32)
            }
            _ => acc,
        }
    })
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
        let plan = crate::body_plan::humanoid_plan_static();
        assert_eq!(
            mutation_damage_multiplier_for_part(None, plan, BodyPart::Back),
            1.0,
            "无 MutationState（entity 从未变异）不应影响任何部位的伤害倍率"
        );
    }

    #[test]
    fn mutation_damage_multiplier_for_part_empty_slots_is_neutral() {
        let plan = crate::body_plan::humanoid_plan_static();
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
        let plan = crate::body_plan::humanoid_plan_static();
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
        let plan = crate::body_plan::humanoid_plan_static();
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
        let plan = crate::body_plan::humanoid_plan_static();
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
        let plan = crate::body_plan::humanoid_plan_static();
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
        let plan = crate::body_plan::humanoid_plan_static();
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
        let plan = crate::body_plan::humanoid_plan_static();
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
        let plan = crate::body_plan::humanoid_plan_static();
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
        use crate::body_plan::{BodyPartDef, BodyPlanId, PartConsequence};

        let humanoid = crate::body_plan::humanoid_plan_static();
        BodyPlan {
            id: BodyPlanId::new("test_no_mutation_mapping"),
            display_name: "测试用无变异映射构型".to_string(),
            is_humanoid: true,
            parts: vec![BodyPartDef {
                id: crate::body_plan::legacy_body_part_to_id(BodyPart::Back),
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
        use crate::body_plan::{BodyPartDef, BodyPlanId, PartConsequence};

        let humanoid = crate::body_plan::humanoid_plan_static();
        let mut mapping = std::collections::HashMap::new();
        // 悬空映射：BodySlot::Back 指向一个不在 8 段 legacy 字符串集合里的 id
        // （模拟未来非人形 plan 的部位 id，如飞鲸尾鳍）——id_to_legacy_body_part 必须
        // 对此返回 None，而不是 panic 或误判成某个 legacy 部位。
        mapping.insert(
            BodySlot::Back,
            crate::body_plan::BodyPartId::new("tail_fin"),
        );
        BodyPlan {
            id: BodyPlanId::new("test_dangling_mutation_mapping"),
            display_name: "测试用悬空变异映射构型".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: crate::body_plan::legacy_body_part_to_id(BodyPart::Back),
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
}
