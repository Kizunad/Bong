//! 炼丹失败产物与废料的规格、生命周期数据。
//TODO:lingtian_refactor 废料还田效果在新种植实现中定义。

use serde::{Deserialize, Serialize};

use crate::inventory::AlchemyItemData;

pub const FAILED_PILL_RESIDUE_TEMPLATE_ID: &str = "alchemy_residue_failed_pill";
pub const FLAWED_PILL_RESIDUE_TEMPLATE_ID: &str = "alchemy_residue_flawed_pill";
pub const PROCESSING_DREGS_TEMPLATE_ID: &str = "alchemy_residue_processing_dregs";
pub const AGING_SCRAPS_TEMPLATE_ID: &str = "alchemy_residue_aging_scraps";

/// 72h 保鲜期，按 server tick 计（20 ticks/s）。
pub const PILL_RESIDUE_TTL_TICKS: u64 = 72 * 60 * 60 * 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PillResidueKind {
    FailedPill,
    FlawedPill,
    ProcessingDregs,
    AgingScraps,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PillResidueSpec {
    pub template_id: &'static str,
}

impl PillResidueKind {
    pub fn spec(self) -> PillResidueSpec {
        match self {
            Self::FailedPill => PillResidueSpec {
                template_id: FAILED_PILL_RESIDUE_TEMPLATE_ID,
            },
            Self::FlawedPill => PillResidueSpec {
                template_id: FLAWED_PILL_RESIDUE_TEMPLATE_ID,
            },
            Self::ProcessingDregs => PillResidueSpec {
                template_id: PROCESSING_DREGS_TEMPLATE_ID,
            },
            Self::AgingScraps => PillResidueSpec {
                template_id: AGING_SCRAPS_TEMPLATE_ID,
            },
        }
    }
}

pub fn residue_alchemy_data(kind: PillResidueKind, produced_at_tick: u64) -> AlchemyItemData {
    AlchemyItemData::PillResidue {
        residue_kind: kind,
        produced_at_tick,
        expires_at_tick: produced_at_tick.saturating_add(PILL_RESIDUE_TTL_TICKS),
    }
}

pub fn residue_kind_for_recyclable_outcome(
    outcome: &crate::alchemy::ResolvedOutcome,
) -> Option<PillResidueKind> {
    match outcome {
        crate::alchemy::ResolvedOutcome::Pill {
            flawed_path: true, ..
        } => Some(PillResidueKind::FlawedPill),
        crate::alchemy::ResolvedOutcome::Waste { .. }
        | crate::alchemy::ResolvedOutcome::Mismatch
        | crate::alchemy::ResolvedOutcome::Explode { .. } => Some(PillResidueKind::FailedPill),
        crate::alchemy::ResolvedOutcome::Pill {
            flawed_path: false, ..
        } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recyclable_outcomes_map_to_residue_kinds() {
        assert_eq!(
            residue_kind_for_recyclable_outcome(&crate::alchemy::ResolvedOutcome::Waste {
                recipe_id: Some("hui_yuan_pill_v0".to_string()),
            }),
            Some(PillResidueKind::FailedPill)
        );
        assert_eq!(
            residue_kind_for_recyclable_outcome(&crate::alchemy::ResolvedOutcome::Mismatch),
            Some(PillResidueKind::FailedPill)
        );
        assert_eq!(
            residue_kind_for_recyclable_outcome(&crate::alchemy::ResolvedOutcome::Pill {
                recipe_id: "hui_yuan_pill_v0".to_string(),
                pill: "hui_yuan_pill".to_string(),
                quality: 0.4,
                toxin_amount: 0.3,
                toxin_color: crate::cultivation::components::ColorKind::Mellow,
                qi_gain: None,
                quality_tier: 3,
                effect_multiplier: 0.6,
                consecrated: false,
                side_effect: None,
                flawed_path: true,
            }),
            Some(PillResidueKind::FlawedPill)
        );
        assert_eq!(
            residue_kind_for_recyclable_outcome(&crate::alchemy::ResolvedOutcome::Pill {
                recipe_id: "hui_yuan_pill_v0".to_string(),
                pill: "hui_yuan_pill".to_string(),
                quality: 1.0,
                toxin_amount: 0.1,
                toxin_color: crate::cultivation::components::ColorKind::Mellow,
                qi_gain: Some(1.0),
                quality_tier: 5,
                effect_multiplier: 1.0,
                consecrated: false,
                side_effect: None,
                flawed_path: false,
            }),
            None
        );
    }
}
