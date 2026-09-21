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
    if !current_tick.is_multiple_of(MUTATION_ADVANCE_INTERVAL_TICKS) {
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
