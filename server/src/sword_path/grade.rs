//! plan-sword-path-v1 P0 — 剑品阶与乘数。

use serde::{Deserialize, Serialize};

use crate::cultivation::components::Realm;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SwordGrade {
    Mortal,
    Awakened,
    Induced,
    Condensed,
    Solidified,
    Spirit,
    Void,
}

impl SwordGrade {
    pub const ALL: [Self; 7] = [
        Self::Mortal,
        Self::Awakened,
        Self::Induced,
        Self::Condensed,
        Self::Solidified,
        Self::Spirit,
        Self::Void,
    ];

    pub fn tier(self) -> u8 {
        match self {
            Self::Mortal => 0,
            Self::Awakened => 1,
            Self::Induced => 2,
            Self::Condensed => 3,
            Self::Solidified => 4,
            Self::Spirit => 5,
            Self::Void => 6,
        }
    }

    pub fn from_tier(tier: u8) -> Option<Self> {
        match tier {
            0 => Some(Self::Mortal),
            1 => Some(Self::Awakened),
            2 => Some(Self::Induced),
            3 => Some(Self::Condensed),
            4 => Some(Self::Solidified),
            5 => Some(Self::Spirit),
            6 => Some(Self::Void),
            _ => None,
        }
    }

    pub fn next(self) -> Option<Self> {
        Self::from_tier(self.tier() + 1)
    }

    pub fn stored_qi_cap(self) -> f64 {
        match self {
            Self::Mortal => 0.0,
            Self::Awakened => 0.0,
            Self::Induced => 0.0,
            Self::Condensed => 15.0,
            Self::Solidified => 100.0,
            Self::Spirit => 500.0,
            Self::Void => 3000.0,
        }
    }

    pub fn can_store_qi(self) -> bool {
        self.tier() >= 3
    }

    pub fn grade_mult(self) -> f32 {
        match self {
            Self::Mortal => 1.0,
            Self::Awakened => 1.05,
            Self::Induced => 1.1,
            Self::Condensed => 1.25,
            Self::Solidified => 1.6,
            Self::Spirit => 2.2,
            Self::Void => 3.5,
        }
    }

    pub fn shatter_threshold(self) -> f64 {
        self.stored_qi_cap() * 1.5
    }

    pub fn required_realm(self) -> Realm {
        match self {
            Self::Mortal => Realm::Awaken,
            Self::Awakened => Realm::Awaken,
            Self::Induced => Realm::Induce,
            Self::Condensed => Realm::Condense,
            Self::Solidified => Realm::Solidify,
            Self::Spirit => Realm::Spirit,
            Self::Void => Realm::Void,
        }
    }

    pub fn upgrade_qi_cost(self) -> UpgradeQiCost {
        match self {
            Self::Mortal => UpgradeQiCost::Fixed(0.0),
            Self::Awakened => UpgradeQiCost::Fixed(0.0),
            Self::Induced => UpgradeQiCost::Fixed(5.0),
            Self::Condensed => UpgradeQiCost::Fixed(30.0),
            Self::Solidified => UpgradeQiCost::Fixed(150.0),
            Self::Spirit => UpgradeQiCost::All,
            Self::Void => UpgradeQiCost::All,
        }
    }

    pub fn upgrade_time_ticks(self) -> u64 {
        match self {
            Self::Mortal => 400,
            Self::Awakened => 800,
            Self::Induced => 1600,
            Self::Condensed => 3200,
            Self::Solidified => 6400,
            Self::Spirit => 12800,
            Self::Void => 0,
        }
    }

    pub fn upgrade_fail_chance(self) -> f32 {
        match self {
            Self::Mortal => 0.0,
            Self::Awakened => 0.05,
            Self::Induced => 0.15,
            Self::Condensed => 0.25,
            Self::Solidified => 0.35,
            Self::Spirit => 0.50,
            Self::Void => 0.0,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Mortal => "凡铁",
            Self::Awakened => "醒灵",
            Self::Induced => "引气",
            Self::Condensed => "凝脉",
            Self::Solidified => "固元",
            Self::Spirit => "通灵",
            Self::Void => "化虚",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpgradeQiCost {
    Fixed(f64),
    All,
}

impl UpgradeQiCost {
    pub fn resolve(self, qi_current: f64) -> f64 {
        match self {
            Self::Fixed(v) => v,
            Self::All => qi_current,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_caps_monotonic() {
        let caps: Vec<f64> = SwordGrade::ALL.iter().map(|g| g.stored_qi_cap()).collect();
        for pair in caps.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "stored_qi_cap should be monotonically non-decreasing: {} -> {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn grade_mult_monotonic() {
        let mults: Vec<f32> = SwordGrade::ALL.iter().map(|g| g.grade_mult()).collect();
        for pair in mults.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "grade_mult should be monotonically non-decreasing: {} -> {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn specific_caps_match_plan() {
        assert_eq!(SwordGrade::Mortal.stored_qi_cap(), 0.0);
        assert_eq!(SwordGrade::Awakened.stored_qi_cap(), 0.0);
        assert_eq!(SwordGrade::Induced.stored_qi_cap(), 0.0);
        assert_eq!(SwordGrade::Condensed.stored_qi_cap(), 15.0);
        assert_eq!(SwordGrade::Solidified.stored_qi_cap(), 100.0);
        assert_eq!(SwordGrade::Spirit.stored_qi_cap(), 500.0);
        assert_eq!(SwordGrade::Void.stored_qi_cap(), 3000.0);
    }

    #[test]
    fn specific_mults_match_plan() {
        assert!((SwordGrade::Mortal.grade_mult() - 1.0).abs() < 1e-6);
        assert!((SwordGrade::Awakened.grade_mult() - 1.05).abs() < 1e-6);
        assert!((SwordGrade::Induced.grade_mult() - 1.1).abs() < 1e-6);
        assert!((SwordGrade::Condensed.grade_mult() - 1.25).abs() < 1e-6);
        assert!((SwordGrade::Solidified.grade_mult() - 1.6).abs() < 1e-6);
        assert!((SwordGrade::Spirit.grade_mult() - 2.2).abs() < 1e-6);
        assert!((SwordGrade::Void.grade_mult() - 3.5).abs() < 1e-6);
    }

    #[test]
    fn can_store_qi_only_grade_3_plus() {
        assert!(!SwordGrade::Mortal.can_store_qi());
        assert!(!SwordGrade::Awakened.can_store_qi());
        assert!(!SwordGrade::Induced.can_store_qi());
        assert!(SwordGrade::Condensed.can_store_qi());
        assert!(SwordGrade::Solidified.can_store_qi());
        assert!(SwordGrade::Spirit.can_store_qi());
        assert!(SwordGrade::Void.can_store_qi());
    }

    #[test]
    fn tier_roundtrip() {
        for g in SwordGrade::ALL {
            assert_eq!(
                SwordGrade::from_tier(g.tier()),
                Some(g),
                "roundtrip failed for {:?}",
                g
            );
        }
    }

    #[test]
    fn next_grade() {
        assert_eq!(SwordGrade::Mortal.next(), Some(SwordGrade::Awakened));
        assert_eq!(SwordGrade::Spirit.next(), Some(SwordGrade::Void));
        assert_eq!(SwordGrade::Void.next(), None);
    }

    #[test]
    fn shatter_threshold_zero_for_no_cap() {
        assert_eq!(SwordGrade::Mortal.shatter_threshold(), 0.0);
        assert_eq!(SwordGrade::Awakened.shatter_threshold(), 0.0);
        assert_eq!(SwordGrade::Induced.shatter_threshold(), 0.0);
    }

    #[test]
    fn shatter_threshold_is_1_5x_cap() {
        assert!((SwordGrade::Condensed.shatter_threshold() - 22.5).abs() < 1e-6);
        assert!((SwordGrade::Solidified.shatter_threshold() - 150.0).abs() < 1e-6);
        assert!((SwordGrade::Spirit.shatter_threshold() - 750.0).abs() < 1e-6);
        assert!((SwordGrade::Void.shatter_threshold() - 4500.0).abs() < 1e-6);
    }

    #[test]
    fn upgrade_qi_cost_low_grades_zero() {
        assert_eq!(
            SwordGrade::Mortal.upgrade_qi_cost(),
            UpgradeQiCost::Fixed(0.0)
        );
        assert_eq!(
            SwordGrade::Awakened.upgrade_qi_cost(),
            UpgradeQiCost::Fixed(0.0)
        );
    }

    #[test]
    fn upgrade_qi_cost_resolve_all() {
        let cost = UpgradeQiCost::All;
        assert!((cost.resolve(500.0) - 500.0).abs() < 1e-6);
    }

    #[test]
    fn upgrade_fail_chance_mortal_zero() {
        assert!((SwordGrade::Mortal.upgrade_fail_chance()).abs() < 1e-6);
    }

    #[test]
    fn upgrade_fail_chance_monotonic() {
        let chances: Vec<f32> = SwordGrade::ALL[..6]
            .iter()
            .map(|g| g.upgrade_fail_chance())
            .collect();
        for pair in chances.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "upgrade_fail_chance should be non-decreasing: {} -> {}",
                pair[0],
                pair[1]
            );
        }
    }
}
