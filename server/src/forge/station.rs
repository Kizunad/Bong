//! plan-forge-v1 §1.2 锻炉系统。
//!
//! MVP：Component 挂到 BlockEntity（或 placeholder Entity）上，
//! tier 限制能使用的图谱（凡铁砧最高锻法器）。

use valence::prelude::{bevy_ecs, Component, Entity};

use super::session::ForgeSessionId;

/// 砧 tier：1 凡铁 / 2 灵铁 / 3 玄铁 / 4 道砧。
pub type StationTier = u8;

#[derive(Debug, Clone, Component)]
pub struct WeaponForgeStation {
    pub tier: StationTier,
    pub owner: Option<Entity>,
    pub session: Option<ForgeSessionId>,
    pub integrity: f32,
}

impl Default for WeaponForgeStation {
    fn default() -> Self {
        Self {
            tier: 1,
            owner: None,
            session: None,
            integrity: 1.0,
        }
    }
}

impl WeaponForgeStation {
    pub fn with_tier(tier: StationTier) -> Self {
        Self {
            tier,
            ..Default::default()
        }
    }

    /// 图谱是否可在此砧上使用（本砧 tier ≥ station_tier_min）。
    pub fn can_craft(&self, station_tier_min: StationTier) -> bool {
        self.tier >= station_tier_min && self.integrity > 0.0
    }

    /// 爆炉损耗（clamp 到 0）。
    pub fn apply_wear(&mut self, wear: f32) {
        self.integrity = (self.integrity - wear).max(0.0);
    }

    pub fn is_broken(&self) -> bool {
        self.integrity <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tier_1_can_craft_tier_1() {
        let s = WeaponForgeStation::default();
        assert!(s.can_craft(1));
        assert!(!s.can_craft(2));
    }

    #[test]
    fn wear_clamped_and_breaks() {
        let mut s = WeaponForgeStation::with_tier(2);
        s.apply_wear(0.3);
        assert!((s.integrity - 0.7).abs() < 1e-6);
        s.apply_wear(5.0);
        assert_eq!(s.integrity, 0.0);
        assert!(s.is_broken());
        assert!(!s.can_craft(1));
    }
}
