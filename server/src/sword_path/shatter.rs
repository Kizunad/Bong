//! plan-sword-path-v1 P0 — 剑碎反噬结算。

use super::bond::{SwordShatterEvent, SHATTER_SWORD_SOUL_CHANCE};

#[derive(Debug, Clone, PartialEq)]
pub struct ShatterOutcome {
    pub backlash_qi_current: f64,
    pub backlash_qi_max_permanent: f64,
    pub qi_released_to_zone: f64,
    pub produces_sword_soul: bool,
}

pub fn compute_shatter_outcome(event: &SwordShatterEvent, roll: f64) -> ShatterOutcome {
    let backlash_current = event.stored_qi * super::bond::SHATTER_BACKLASH_QI_CURRENT_RATIO;
    let backlash_max = event.stored_qi * super::bond::SHATTER_BACKLASH_QI_MAX_RATIO;
    let released = event.stored_qi - backlash_current;

    ShatterOutcome {
        backlash_qi_current: backlash_current,
        backlash_qi_max_permanent: backlash_max,
        qi_released_to_zone: released.max(0.0),
        produces_sword_soul: roll < SHATTER_SWORD_SOUL_CHANCE,
    }
}

pub fn compute_heaven_gate_shatter(qi_max: f64, stored_qi: f64) -> HeavenGateOutcome {
    let staging_buffer = qi_max + stored_qi;
    let new_qi_max = qi_max * super::techniques::effects::HEAVEN_GATE_QI_MAX_RETAIN;
    HeavenGateOutcome {
        staging_buffer,
        new_qi_max,
        qi_max_lost: qi_max - new_qi_max,
        realm_drop_to: crate::cultivation::components::Realm::Solidify,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeavenGateOutcome {
    pub staging_buffer: f64,
    pub new_qi_max: f64,
    pub qi_max_lost: f64,
    pub realm_drop_to: crate::cultivation::components::Realm,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sword_path::bond::SwordShatterEvent;
    use crate::sword_path::grade::SwordGrade;
    use valence::prelude::Entity;

    fn dummy_event(stored_qi: f64) -> SwordShatterEvent {
        SwordShatterEvent {
            player: Entity::from_raw(1),
            weapon: Entity::from_raw(2),
            stored_qi,
            grade: SwordGrade::Solidified,
        }
    }

    #[test]
    fn shatter_backlash_proportional() {
        let event = dummy_event(100.0);
        let out = compute_shatter_outcome(&event, 0.5);
        assert!(
            (out.backlash_qi_current - 60.0).abs() < 1e-6,
            "qi_current backlash = 100 * 0.6 = 60, got {}",
            out.backlash_qi_current
        );
        assert!(
            (out.backlash_qi_max_permanent - 5.0).abs() < 1e-6,
            "qi_max backlash = 100 * 0.05 = 5, got {}",
            out.backlash_qi_max_permanent
        );
    }

    #[test]
    fn shatter_qi_conservation() {
        let event = dummy_event(200.0);
        let out = compute_shatter_outcome(&event, 0.5);
        let total = out.backlash_qi_current + out.qi_released_to_zone;
        assert!(
            (total - 200.0).abs() < 1e-6,
            "backlash + released should equal stored_qi (200), got {total}"
        );
    }

    #[test]
    fn shatter_zero_stored_qi() {
        let event = dummy_event(0.0);
        let out = compute_shatter_outcome(&event, 0.5);
        assert_eq!(out.backlash_qi_current, 0.0);
        assert_eq!(out.backlash_qi_max_permanent, 0.0);
        assert_eq!(out.qi_released_to_zone, 0.0);
    }

    #[test]
    fn sword_soul_at_low_roll() {
        let event = dummy_event(100.0);
        let out = compute_shatter_outcome(&event, 0.05);
        assert!(
            out.produces_sword_soul,
            "roll 0.05 < 0.10 should produce sword soul"
        );
    }

    #[test]
    fn no_sword_soul_at_high_roll() {
        let event = dummy_event(100.0);
        let out = compute_shatter_outcome(&event, 0.15);
        assert!(
            !out.produces_sword_soul,
            "roll 0.15 >= 0.10 should not produce sword soul"
        );
    }

    #[test]
    fn heaven_gate_staging_buffer() {
        let out = compute_heaven_gate_shatter(10700.0, 3000.0);
        assert!(
            (out.staging_buffer - 13700.0).abs() < 1e-6,
            "staging = qi_max + stored = 13700, got {}",
            out.staging_buffer
        );
    }

    #[test]
    fn heaven_gate_qi_max_retained() {
        let out = compute_heaven_gate_shatter(10700.0, 3000.0);
        assert!(
            (out.new_qi_max - 1070.0).abs() < 1e-6,
            "new qi_max = 10700 * 0.1 = 1070, got {}",
            out.new_qi_max
        );
    }

    #[test]
    fn heaven_gate_realm_drop() {
        let out = compute_heaven_gate_shatter(10700.0, 3000.0);
        assert_eq!(
            out.realm_drop_to,
            crate::cultivation::components::Realm::Solidify,
            "should drop to 固元"
        );
    }

    #[test]
    fn heaven_gate_qi_max_loss_conservation() {
        let out = compute_heaven_gate_shatter(10700.0, 3000.0);
        assert!(
            (out.new_qi_max + out.qi_max_lost - 10700.0).abs() < 1e-6,
            "new + lost should equal original qi_max"
        );
    }
}
