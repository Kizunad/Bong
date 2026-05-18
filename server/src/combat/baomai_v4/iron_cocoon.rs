//! plan-baomai-v4 §3 — 活茧（Iron Cocoon）。
//!
//! 体修长期过载导致微量真元渗入肌肉/骨膜，在组织中矿化沉积。
//! 四阶被动进化：茧皮 → 茧骨 → 茧肉 → 茧灵。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, EventWriter, Query, Res};

use crate::combat::baomai_v4::constants::IRON_COCOON_THRESHOLDS;
use crate::combat::baomai_v4::events::IronCocoonStageUpEvent;
use crate::combat::baomai_v4::scar_history::ScarHistory;
use crate::combat::components::DerivedAttrs;
use crate::combat::CombatClock;

/// 活茧四阶 + None（普通肉身）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum IronCocoonStage {
    /// 普通肉身（0-49 overloads）。
    None,
    /// 茧皮（50+ overloads）——厚皮：BRUISE 伤害阈值 x1.25。
    ToughSkin,
    /// 茧骨（120+ overloads）——铁骨：FRACTURE 降级为 LACERATION 概率 20%。
    IronBone,
    /// 茧肉（250+ overloads）——钝感：Cut/Pierce LACERATION → ABRASION。
    DullFlesh,
    /// 茧灵（500+ overloads）——逆生：有活跃回路的经脉 flow_rate +5%。
    ScarForged,
}

impl IronCocoonStage {
    /// 根据累计过载次数计算当前活茧阶段。
    pub fn from_overloads(n: u32) -> Self {
        if n >= IRON_COCOON_THRESHOLDS[3] {
            Self::ScarForged
        } else if n >= IRON_COCOON_THRESHOLDS[2] {
            Self::DullFlesh
        } else if n >= IRON_COCOON_THRESHOLDS[1] {
            Self::IronBone
        } else if n >= IRON_COCOON_THRESHOLDS[0] {
            Self::ToughSkin
        } else {
            Self::None
        }
    }

    /// 阶段序号（用于比较）。
    #[allow(dead_code)]
    pub fn ordinal(self) -> u8 {
        match self {
            Self::None => 0,
            Self::ToughSkin => 1,
            Self::IronBone => 2,
            Self::DullFlesh => 3,
            Self::ScarForged => 4,
        }
    }
}

/// 检查 `ScarHistory.total_overloads` 是否跨越阶段阈值。
///
/// 不存储 stage——每次实时计算 `IronCocoonStage::from_overloads()`。
/// 通过对比前后阶段检测跨越，emit `IronCocoonStageUpEvent`。
///
/// 该系统存储上次检测的阶段，使用 `IronCocoonTracker` local resource。
pub fn iron_cocoon_check_system(
    mut query: Query<(Entity, &ScarHistory, &mut IronCocoonTracker)>,
    mut events: EventWriter<IronCocoonStageUpEvent>,
    clock: Res<CombatClock>,
) {
    for (entity, history, mut tracker) in query.iter_mut() {
        let current = IronCocoonStage::from_overloads(history.total_overloads);
        if current > tracker.last_stage {
            events.send(IronCocoonStageUpEvent {
                entity,
                from: tracker.last_stage,
                to: current,
                total_overloads: history.total_overloads,
                tick: clock.tick,
            });
            tracker.last_stage = current;
        }
    }
}

/// 追踪器组件——跟踪上次检测的活茧阶段，用于检测跨越。
#[derive(Debug, Clone, Copy, valence::prelude::Component, Serialize, Deserialize)]
pub struct IronCocoonTracker {
    pub last_stage: IronCocoonStage,
}

impl Default for IronCocoonTracker {
    fn default() -> Self {
        Self {
            last_stage: IronCocoonStage::None,
        }
    }
}

/// 读 `IronCocoonStage`（从 ScarHistory 实时计算）修改 `DerivedAttrs`。
///
/// 各阶被动效果：
/// - 茧皮：bruise_threshold_multiplier = 1.25
/// - 茧骨：fracture_downgrade_chance = 0.20
/// - 茧肉：cut_pierce_downgrade = true
/// - 茧灵：scar_forged_flow_bonus = true（实际 flow_rate 修改由 ScarForged 系统处理）
pub fn iron_cocoon_passive_system(mut query: Query<(&ScarHistory, &mut DerivedAttrs)>) {
    for (history, mut attrs) in query.iter_mut() {
        let stage = IronCocoonStage::from_overloads(history.total_overloads);
        // Reset cocoon-derived fields.
        attrs.bruise_threshold_multiplier = 1.0;
        attrs.fracture_downgrade_chance = 0.0;
        attrs.cut_pierce_downgrade = false;
        attrs.scar_forged_flow_bonus = false;

        match stage {
            IronCocoonStage::None => {}
            IronCocoonStage::ToughSkin => {
                attrs.bruise_threshold_multiplier =
                    super::constants::TOUGH_SKIN_BRUISE_THRESHOLD_MULTIPLIER;
            }
            IronCocoonStage::IronBone => {
                attrs.bruise_threshold_multiplier =
                    super::constants::TOUGH_SKIN_BRUISE_THRESHOLD_MULTIPLIER;
                attrs.fracture_downgrade_chance =
                    super::constants::IRON_BONE_FRACTURE_DOWNGRADE_CHANCE;
            }
            IronCocoonStage::DullFlesh => {
                attrs.bruise_threshold_multiplier =
                    super::constants::TOUGH_SKIN_BRUISE_THRESHOLD_MULTIPLIER;
                attrs.fracture_downgrade_chance =
                    super::constants::IRON_BONE_FRACTURE_DOWNGRADE_CHANCE;
                attrs.cut_pierce_downgrade = true;
            }
            IronCocoonStage::ScarForged => {
                attrs.bruise_threshold_multiplier =
                    super::constants::TOUGH_SKIN_BRUISE_THRESHOLD_MULTIPLIER;
                attrs.fracture_downgrade_chance =
                    super::constants::IRON_BONE_FRACTURE_DOWNGRADE_CHANCE;
                attrs.cut_pierce_downgrade = true;
                attrs.scar_forged_flow_bonus = true;
            }
        }
    }
}
