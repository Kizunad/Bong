//! plan-sword-path-v1 P0 struct 定义 / P4 实装 — 天道盲区。

use valence::prelude::{bevy_ecs, Component, DVec3};

#[derive(Debug, Clone, Component)]
pub struct TiandaoBlindZone {
    pub center: DVec3,
    pub radius: f64,
    pub ttl_ticks: u64,
    pub created_tick: u64,
}

impl TiandaoBlindZone {
    pub fn contains(&self, pos: DVec3) -> bool {
        pos.distance(self.center) <= self.radius
    }

    pub fn is_expired(&self, current_tick: u64) -> bool {
        current_tick >= self.created_tick + self.ttl_ticks
    }

    pub fn remaining_ticks(&self, current_tick: u64) -> u64 {
        let end = self.created_tick + self.ttl_ticks;
        end.saturating_sub(current_tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_zone() -> TiandaoBlindZone {
        TiandaoBlindZone {
            center: DVec3::new(100.0, 64.0, 200.0),
            radius: 100.0,
            ttl_ticks: 6000,
            created_tick: 1000,
        }
    }

    #[test]
    fn contains_center() {
        let zone = make_zone();
        assert!(zone.contains(DVec3::new(100.0, 64.0, 200.0)));
    }

    #[test]
    fn contains_at_edge() {
        let zone = make_zone();
        assert!(zone.contains(DVec3::new(200.0, 64.0, 200.0)));
    }

    #[test]
    fn not_contains_outside() {
        let zone = make_zone();
        assert!(!zone.contains(DVec3::new(201.0, 64.0, 200.0)));
    }

    #[test]
    fn not_expired_at_creation() {
        let zone = make_zone();
        assert!(!zone.is_expired(1000));
    }

    #[test]
    fn expired_after_ttl() {
        let zone = make_zone();
        assert!(zone.is_expired(7000));
    }

    #[test]
    fn expired_exactly_at_end() {
        let zone = make_zone();
        assert!(zone.is_expired(7000));
    }

    #[test]
    fn remaining_ticks_midway() {
        let zone = make_zone();
        assert_eq!(zone.remaining_ticks(4000), 3000);
    }

    #[test]
    fn remaining_ticks_after_expiry() {
        let zone = make_zone();
        assert_eq!(zone.remaining_ticks(9000), 0);
    }
}
