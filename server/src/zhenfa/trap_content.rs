//! plan-zhenfa-content-v1 -- 三种凡阵的规格与轻量运行时规则。
//!
//! 这里不重建阵法系统，只把"警示 / 爆炸 / 缓阵"作为现有 `ZhenfaRegistry`
//! 的内容型 kind 接入，避免和 zhenfa-v1/v2 的阵眼生命周期、拆除、VFX 重复。

use serde::{Deserialize, Serialize};
use valence::prelude::DVec3;

use super::ZhenfaKind;

const TICKS_PER_SECOND: u64 = 20;
const TICKS_PER_HOUR: u64 = 60 * 60 * TICKS_PER_SECOND;

pub const WARNING_TRAP_ITEM_ID: &str = "warning_trap";
pub const BLAST_TRAP_ITEM_ID: &str = "blast_trap";
pub const SLOW_TRAP_ITEM_ID: &str = "slow_trap";

pub const WARNING_TRIGGER_THROTTLE_TICKS: u64 = 5 * TICKS_PER_SECOND;
pub const SLOW_TRAP_MAX_CHARGES: u8 = 3;
pub const SLOW_TRAP_EFFECT_TICKS: u64 = 3 * TICKS_PER_SECOND;
pub const CHUNK_DENSITY_GAZE_THRESHOLD: f64 = 0.85;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrdinaryTrapKind {
    Warning,
    Blast,
    Slow,
}

impl OrdinaryTrapKind {
    pub fn from_zhenfa_kind(kind: ZhenfaKind) -> Option<Self> {
        match kind {
            ZhenfaKind::WarningTrap => Some(Self::Warning),
            ZhenfaKind::BlastTrap => Some(Self::Blast),
            ZhenfaKind::SlowTrap => Some(Self::Slow),
            _ => None,
        }
    }

    pub fn expected_item_id(self) -> &'static str {
        match self {
            Self::Warning => WARNING_TRAP_ITEM_ID,
            Self::Blast => BLAST_TRAP_ITEM_ID,
            Self::Slow => SLOW_TRAP_ITEM_ID,
        }
    }

    pub fn detection_radius(self) -> f64 {
        match self {
            Self::Warning | Self::Blast => 1.5,
            Self::Slow => 2.0,
        }
    }

    pub fn vertical_height(self) -> f64 {
        match self {
            Self::Warning | Self::Slow => 3.0,
            Self::Blast => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrapTargetFace {
    Top,
    Bottom,
    North,
    South,
    East,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapPlacementShape {
    EmbeddedVertical,
    SurfaceHorizontal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrapQiCost {
    pub sealed_qi: f64,
    pub ratio_of_max: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrapDiscoveryProfile {
    pub signature_qi: f64,
    pub induce_chance: f64,
    pub condense_chance: f64,
    pub scan_range_blocks: f64,
}

impl TrapDiscoveryProfile {
    pub fn reveal_threshold(self) -> f64 {
        if self.signature_qi >= 0.10 || self.induce_chance >= 1.0 {
            10.0
        } else if self.signature_qi >= 0.05 || self.condense_chance >= 1.0 {
            20.0
        } else {
            30.0
        }
    }
}

pub fn placement_shape(kind: OrdinaryTrapKind) -> TrapPlacementShape {
    match kind {
        OrdinaryTrapKind::Warning | OrdinaryTrapKind::Slow => TrapPlacementShape::EmbeddedVertical,
        OrdinaryTrapKind::Blast => TrapPlacementShape::SurfaceHorizontal,
    }
}

pub fn placement_allowed(kind: OrdinaryTrapKind, face: TrapTargetFace) -> bool {
    match placement_shape(kind) {
        TrapPlacementShape::EmbeddedVertical => face == TrapTargetFace::Top,
        TrapPlacementShape::SurfaceHorizontal => face != TrapTargetFace::Bottom,
    }
}

pub fn resolve_qi_cost(kind: OrdinaryTrapKind, qi_max: f64, requested_ratio: f64) -> TrapQiCost {
    let qi_max = qi_max.max(1.0);
    let sealed_qi = match kind {
        OrdinaryTrapKind::Warning => 2.0,
        OrdinaryTrapKind::Slow => 8.0,
        OrdinaryTrapKind::Blast => {
            let requested = if requested_ratio.is_finite() {
                requested_ratio.clamp(0.0, 1.0)
            } else {
                0.0
            };
            15.0 + requested * 15.0
        }
    };
    TrapQiCost {
        sealed_qi,
        ratio_of_max: (sealed_qi / qi_max).clamp(0.0, 1.0),
    }
}

pub fn half_life_ticks(kind: OrdinaryTrapKind) -> u64 {
    match kind {
        OrdinaryTrapKind::Warning => 8 * TICKS_PER_HOUR,
        OrdinaryTrapKind::Blast => 2 * TICKS_PER_HOUR,
        OrdinaryTrapKind::Slow => 4 * TICKS_PER_HOUR,
    }
}

pub fn survival_ticks(kind: OrdinaryTrapKind) -> u64 {
    let half_life = half_life_ticks(kind);
    match kind {
        OrdinaryTrapKind::Warning => ((half_life as f64) * 6.25).round() as u64,
        OrdinaryTrapKind::Blast => half_life * 4,
        OrdinaryTrapKind::Slow => half_life * 3,
    }
}

pub fn survival_ticks_with_environment(kind: OrdinaryTrapKind, zone_qi: f64) -> u64 {
    ((survival_ticks(kind) as f64) * env_half_life_multiplier(zone_qi))
        .round()
        .max(1.0) as u64
}

pub fn discovery_profile(kind: OrdinaryTrapKind) -> TrapDiscoveryProfile {
    match kind {
        OrdinaryTrapKind::Warning => TrapDiscoveryProfile {
            signature_qi: 0.02,
            induce_chance: 0.0,
            condense_chance: 0.30,
            scan_range_blocks: 1.0,
        },
        OrdinaryTrapKind::Blast => TrapDiscoveryProfile {
            signature_qi: 0.15,
            induce_chance: 1.0,
            condense_chance: 1.0,
            scan_range_blocks: 2.0,
        },
        OrdinaryTrapKind::Slow => TrapDiscoveryProfile {
            signature_qi: 0.08,
            induce_chance: 0.50,
            condense_chance: 1.0,
            scan_range_blocks: 2.0,
        },
    }
}

pub fn blast_damage(sealed_qi: f64) -> f32 {
    (sealed_qi.max(0.0) * 0.6) as f32
}

pub fn vertical_column_contains(
    position: DVec3,
    trap_pos: [i32; 3],
    radius: f64,
    height: f64,
) -> bool {
    let dx = position.x - (f64::from(trap_pos[0]) + 0.5);
    let dz = position.z - (f64::from(trap_pos[2]) + 0.5);
    let horizontal_sq = dx * dx + dz * dz;
    let y_min = f64::from(trap_pos[1]);
    let y_max = y_min + height;
    horizontal_sq <= radius * radius && position.y >= y_min && position.y <= y_max
}

pub fn horizontal_same_layer_contains(position: DVec3, trap_pos: [i32; 3], radius: f64) -> bool {
    let dx = position.x - (f64::from(trap_pos[0]) + 0.5);
    let dz = position.z - (f64::from(trap_pos[2]) + 0.5);
    let horizontal_sq = dx * dx + dz * dz;
    horizontal_sq <= radius * radius && (position.y - f64::from(trap_pos[1])).abs() <= 1.0
}

pub fn chunk_coord(pos: [i32; 3]) -> [i32; 2] {
    [pos[0].div_euclid(16), pos[2].div_euclid(16)]
}

pub fn chunk_density_load(total_sealed_qi: f64) -> f64 {
    // Plan examples treat 2 qi x 42 ~= threshold 0.85, so normalize by 100 qi.
    total_sealed_qi.max(0.0) / 100.0
}

pub fn chunk_density_exceeded(total_sealed_qi: f64) -> bool {
    chunk_density_load(total_sealed_qi) > CHUNK_DENSITY_GAZE_THRESHOLD
}

pub fn env_half_life_multiplier(zone_qi: f64) -> f64 {
    if zone_qi < 0.0 {
        0.3
    } else if zone_qi == 0.0 {
        0.8
    } else if zone_qi > 0.3 {
        1.2
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_rules_split_vertical_and_surface_traps() {
        assert!(placement_allowed(
            OrdinaryTrapKind::Warning,
            TrapTargetFace::Top
        ));
        assert!(!placement_allowed(
            OrdinaryTrapKind::Warning,
            TrapTargetFace::North
        ));
        assert!(placement_allowed(
            OrdinaryTrapKind::Slow,
            TrapTargetFace::Top
        ));
        assert!(placement_allowed(
            OrdinaryTrapKind::Blast,
            TrapTargetFace::North
        ));
        assert!(placement_allowed(
            OrdinaryTrapKind::Blast,
            TrapTargetFace::Top
        ));
        assert!(!placement_allowed(
            OrdinaryTrapKind::Blast,
            TrapTargetFace::Bottom
        ));
    }

    #[test]
    fn qi_costs_pin_three_ordinary_traps() {
        assert_eq!(
            resolve_qi_cost(OrdinaryTrapKind::Warning, 100.0, 1.0).sealed_qi,
            2.0
        );
        assert_eq!(
            resolve_qi_cost(OrdinaryTrapKind::Slow, 100.0, 1.0).sealed_qi,
            8.0
        );
        assert_eq!(
            resolve_qi_cost(OrdinaryTrapKind::Blast, 100.0, 0.0).sealed_qi,
            15.0
        );
        assert_eq!(
            resolve_qi_cost(OrdinaryTrapKind::Blast, 100.0, 1.0).sealed_qi,
            30.0
        );
    }

    #[test]
    fn detection_geometry_matches_plan_axes() {
        assert!(vertical_column_contains(
            DVec3::new(0.5, 67.0, 0.5),
            [0, 64, 0],
            1.5,
            3.0
        ));
        assert!(!vertical_column_contains(
            DVec3::new(0.5, 68.1, 0.5),
            [0, 64, 0],
            1.5,
            3.0
        ));
        assert!(horizontal_same_layer_contains(
            DVec3::new(1.9, 64.0, 0.5),
            [0, 64, 0],
            1.5
        ));
        assert!(!horizontal_same_layer_contains(
            DVec3::new(1.9, 66.0, 0.5),
            [0, 64, 0],
            1.5
        ));
    }

    #[test]
    fn lifetime_and_damage_pin_plan_values() {
        assert_eq!(
            half_life_ticks(OrdinaryTrapKind::Warning),
            8 * TICKS_PER_HOUR
        );
        assert_eq!(half_life_ticks(OrdinaryTrapKind::Blast), 2 * TICKS_PER_HOUR);
        assert_eq!(half_life_ticks(OrdinaryTrapKind::Slow), 4 * TICKS_PER_HOUR);
        assert_eq!(survival_ticks(OrdinaryTrapKind::Blast), 8 * TICKS_PER_HOUR);
        assert_eq!(blast_damage(15.0), 9.0);
        assert_eq!(blast_damage(30.0), 18.0);
    }

    #[test]
    fn chunk_density_threshold_normalizes_by_hundred_qi() {
        assert!(!chunk_density_exceeded(84.0));
        assert!(chunk_density_exceeded(86.0));
    }

    #[test]
    fn discovery_profiles_keep_warning_hidden_and_blast_obvious() {
        let warning = discovery_profile(OrdinaryTrapKind::Warning);
        assert!(warning.signature_qi < 0.05);
        assert_eq!(warning.induce_chance, 0.0);
        let blast = discovery_profile(OrdinaryTrapKind::Blast);
        assert!(blast.signature_qi >= 0.15);
        assert_eq!(blast.induce_chance, 1.0);
    }

    #[test]
    fn environment_half_life_multipliers_match_plan_pins() {
        assert_eq!(env_half_life_multiplier(-0.1), 0.3);
        assert_eq!(env_half_life_multiplier(0.0), 0.8);
        assert_eq!(env_half_life_multiplier(0.31), 1.2);
        assert_eq!(env_half_life_multiplier(0.2), 1.0);
        assert_eq!(
            survival_ticks_with_environment(OrdinaryTrapKind::Slow, -0.1),
            (12 * TICKS_PER_HOUR * 3) / 10
        );
    }
}
