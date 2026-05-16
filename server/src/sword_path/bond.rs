//! plan-sword-path-v1 P0 — 人剑共生绑定。

use valence::prelude::{bevy_ecs, Component, Entity, Event};

use super::grade::SwordGrade;

pub const BOND_TRIGGER_USES: u32 = 20;
pub const UNBIND_TIME_TICKS: u64 = 30 * 20;
pub const UNBIND_STRENGTH_PENALTY: f32 = 0.5;
pub const QI_INJECT_RATIO: f64 = 0.1;
pub const SHATTER_BACKLASH_QI_CURRENT_RATIO: f64 = 0.6;
pub const SHATTER_BACKLASH_QI_MAX_RATIO: f64 = 0.05;
pub const SHATTER_SWORD_SOUL_CHANCE: f64 = 0.10;

#[derive(Debug, Clone, Component)]
pub struct SwordBondComponent {
    pub bonded_weapon_entity: Entity,
    pub bond_strength: f32,
    pub stored_qi: f64,
    pub grade: SwordGrade,
}

impl SwordBondComponent {
    pub fn new(weapon_entity: Entity) -> Self {
        Self {
            bonded_weapon_entity: weapon_entity,
            bond_strength: 0.0,
            stored_qi: 0.0,
            grade: SwordGrade::Mortal,
        }
    }

    pub fn shatter_threshold(&self) -> f64 {
        self.grade.shatter_threshold()
    }

    pub fn should_shatter(&self) -> bool {
        let threshold = self.shatter_threshold();
        threshold > 0.0 && self.stored_qi >= threshold
    }

    pub fn try_inject_qi(&mut self, qi_cost: f64) -> f64 {
        if !self.grade.can_store_qi() || qi_cost <= 0.0 {
            return 0.0;
        }
        let inject = qi_cost * QI_INJECT_RATIO;
        let cap = self.grade.stored_qi_cap();
        let headroom = (cap - self.stored_qi).max(0.0);
        let actual = inject.min(headroom);
        self.stored_qi += actual;
        actual
    }

    pub fn backlash_qi_current(&self) -> f64 {
        self.stored_qi * SHATTER_BACKLASH_QI_CURRENT_RATIO
    }

    pub fn backlash_qi_max(&self) -> f64 {
        self.stored_qi * SHATTER_BACKLASH_QI_MAX_RATIO
    }
}

#[derive(Debug, Clone, Component)]
pub struct SwordBondProgress {
    pub consecutive_uses: u32,
    pub tracked_weapon_entity: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct SwordBondFormedEvent {
    pub player: Entity,
    pub weapon: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct SwordShatterEvent {
    pub player: Entity,
    pub weapon: Entity,
    pub stored_qi: f64,
    pub grade: SwordGrade,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_entity() -> Entity {
        Entity::from_raw(42)
    }

    #[test]
    fn new_bond_defaults() {
        let bond = SwordBondComponent::new(dummy_entity());
        assert_eq!(bond.grade, SwordGrade::Mortal);
        assert_eq!(bond.stored_qi, 0.0);
        assert_eq!(bond.bond_strength, 0.0);
    }

    #[test]
    fn inject_qi_zero_for_low_grade() {
        let mut bond = SwordBondComponent::new(dummy_entity());
        bond.grade = SwordGrade::Mortal;
        assert_eq!(bond.try_inject_qi(10.0), 0.0, "Mortal should not store qi");

        bond.grade = SwordGrade::Awakened;
        assert_eq!(
            bond.try_inject_qi(10.0),
            0.0,
            "Awakened should not store qi"
        );

        bond.grade = SwordGrade::Induced;
        assert_eq!(bond.try_inject_qi(10.0), 0.0, "Induced should not store qi");
    }

    #[test]
    fn inject_qi_works_for_condensed() {
        let mut bond = SwordBondComponent::new(dummy_entity());
        bond.grade = SwordGrade::Condensed;
        let injected = bond.try_inject_qi(20.0);
        assert!(
            (injected - 2.0).abs() < 1e-6,
            "expected 20 * 0.1 = 2.0, got {injected}"
        );
        assert!(
            (bond.stored_qi - 2.0).abs() < 1e-6,
            "stored_qi should be 2.0, got {}",
            bond.stored_qi
        );
    }

    #[test]
    fn inject_qi_respects_cap() {
        let mut bond = SwordBondComponent::new(dummy_entity());
        bond.grade = SwordGrade::Condensed;
        bond.stored_qi = 14.5;
        let injected = bond.try_inject_qi(100.0);
        assert!(
            (injected - 0.5).abs() < 1e-6,
            "should only inject 0.5 to reach cap 15, got {injected}"
        );
        assert!(
            (bond.stored_qi - 15.0).abs() < 1e-6,
            "stored_qi should be capped at 15.0, got {}",
            bond.stored_qi
        );
    }

    #[test]
    fn inject_qi_zero_cost_returns_zero() {
        let mut bond = SwordBondComponent::new(dummy_entity());
        bond.grade = SwordGrade::Solidified;
        assert_eq!(bond.try_inject_qi(0.0), 0.0);
        assert_eq!(bond.try_inject_qi(-5.0), 0.0);
    }

    #[test]
    fn shatter_threshold_zero_for_low_grades() {
        let mut bond = SwordBondComponent::new(dummy_entity());
        bond.grade = SwordGrade::Mortal;
        assert!(!bond.should_shatter());
        bond.grade = SwordGrade::Awakened;
        assert!(!bond.should_shatter());
    }

    #[test]
    fn should_shatter_when_over_threshold() {
        let mut bond = SwordBondComponent::new(dummy_entity());
        bond.grade = SwordGrade::Condensed;
        bond.stored_qi = 22.4;
        assert!(!bond.should_shatter(), "22.4 < threshold 22.5");
        bond.stored_qi = 22.5;
        assert!(bond.should_shatter(), "22.5 >= threshold 22.5");
    }

    #[test]
    fn backlash_values() {
        let mut bond = SwordBondComponent::new(dummy_entity());
        bond.stored_qi = 100.0;
        assert!(
            (bond.backlash_qi_current() - 60.0).abs() < 1e-6,
            "qi_current backlash = 100 * 0.6 = 60"
        );
        assert!(
            (bond.backlash_qi_max() - 5.0).abs() < 1e-6,
            "qi_max backlash = 100 * 0.05 = 5"
        );
    }

    #[test]
    fn backlash_zero_when_empty() {
        let bond = SwordBondComponent::new(dummy_entity());
        assert_eq!(bond.backlash_qi_current(), 0.0);
        assert_eq!(bond.backlash_qi_max(), 0.0);
    }
}
