use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity, Resource};

pub const SUPPRESS_TSY_QI_COST: f64 = 200.0;
pub const EXPLODE_ZONE_QI_COST: f64 = 300.0;
pub const BARRIER_QI_COST: f64 = 150.0;
pub const SUPPRESS_TSY_LIFESPAN_COST_YEARS: u32 = 50;
pub const EXPLODE_ZONE_LIFESPAN_COST_YEARS: u32 = 100;
pub const BARRIER_LIFESPAN_COST_YEARS: u32 = 30;
pub const TICKS_PER_DAY: u64 = 24 * 60 * 60 * 20;
pub const TICKS_PER_MONTH: u64 = 30 * TICKS_PER_DAY;
pub const SUPPRESS_TSY_COOLDOWN_TICKS: u64 = 30 * TICKS_PER_DAY;
pub const EXPLODE_ZONE_COOLDOWN_TICKS: u64 = 90 * TICKS_PER_DAY;
pub const BARRIER_COOLDOWN_TICKS: u64 = 7 * TICKS_PER_DAY;
pub const BARRIER_TTL_TICKS: u64 = TICKS_PER_MONTH;
pub const EXPLODE_ZONE_DECAY_TICKS: u64 = 6 * TICKS_PER_MONTH;
pub const DAOXIANG_SUPPRESS_EXTENSION_TICKS: u64 = 3 * TICKS_PER_MONTH;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VoidActionKind {
    SuppressTsy,
    ExplodeZone,
    Barrier,
    LegacyAssign,
}

impl VoidActionKind {
    pub const ALL: [Self; 4] = [
        Self::SuppressTsy,
        Self::ExplodeZone,
        Self::Barrier,
        Self::LegacyAssign,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::SuppressTsy => "suppress_tsy",
            Self::ExplodeZone => "explode_zone",
            Self::Barrier => "barrier",
            Self::LegacyAssign => "legacy_assign",
        }
    }

    pub const fn qi_cost(self) -> f64 {
        match self {
            Self::SuppressTsy => SUPPRESS_TSY_QI_COST,
            Self::ExplodeZone => EXPLODE_ZONE_QI_COST,
            Self::Barrier => BARRIER_QI_COST,
            Self::LegacyAssign => 0.0,
        }
    }

    pub const fn lifespan_cost_years(self) -> u32 {
        match self {
            Self::SuppressTsy => SUPPRESS_TSY_LIFESPAN_COST_YEARS,
            Self::ExplodeZone => EXPLODE_ZONE_LIFESPAN_COST_YEARS,
            Self::Barrier => BARRIER_LIFESPAN_COST_YEARS,
            Self::LegacyAssign => 0,
        }
    }

    pub const fn cooldown_ticks(self) -> u64 {
        match self {
            Self::SuppressTsy => SUPPRESS_TSY_COOLDOWN_TICKS,
            Self::ExplodeZone => EXPLODE_ZONE_COOLDOWN_TICKS,
            Self::Barrier => BARRIER_COOLDOWN_TICKS,
            Self::LegacyAssign => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct VoidActionCost {
    pub qi: f64,
    pub lifespan_years: u32,
    pub cooldown_ticks: u64,
}

impl VoidActionCost {
    pub const fn for_kind(kind: VoidActionKind) -> Self {
        Self {
            qi: kind.qi_cost(),
            lifespan_years: kind.lifespan_cost_years(),
            cooldown_ticks: kind.cooldown_ticks(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BarrierGeometry {
    Circle { center: [f64; 3], radius: f64 },
}

impl BarrierGeometry {
    pub fn circle(center: [f64; 3], radius: f64) -> Self {
        Self::Circle {
            center,
            radius: radius.max(0.0),
        }
    }

    pub fn contains(self, point: [f64; 3]) -> bool {
        match self {
            Self::Circle { center, radius } => {
                let dx = point[0] - center[0];
                let dz = point[2] - center[2];
                dx.mul_add(dx, dz * dz) <= radius * radius
            }
        }
    }

    pub fn radius(self) -> f64 {
        match self {
            Self::Circle { radius, .. } => radius,
        }
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct BarrierField {
    pub owner_id: String,
    pub zone_id: String,
    pub geometry: BarrierGeometry,
    pub qi_locked: f64,
    pub created_at_tick: u64,
    pub expires_at_tick: u64,
}

impl BarrierField {
    pub fn new(
        owner_id: impl Into<String>,
        zone_id: impl Into<String>,
        geometry: BarrierGeometry,
        now_tick: u64,
    ) -> Self {
        Self {
            owner_id: owner_id.into(),
            zone_id: zone_id.into(),
            geometry,
            qi_locked: BARRIER_QI_COST,
            created_at_tick: now_tick,
            expires_at_tick: now_tick.saturating_add(BARRIER_TTL_TICKS),
        }
    }

    pub fn expired(&self, now_tick: u64) -> bool {
        now_tick >= self.expires_at_tick
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoidActionLogEntry {
    pub kind: VoidActionKind,
    pub target: String,
    pub at_tick: u64,
    pub qi_cost: f64,
    pub lifespan_cost_years: u32,
    pub outcome: String,
}

impl VoidActionLogEntry {
    pub fn accepted(
        kind: VoidActionKind,
        target: impl Into<String>,
        at_tick: u64,
        outcome: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            target: target.into(),
            at_tick,
            qi_cost: kind.qi_cost(),
            lifespan_cost_years: kind.lifespan_cost_years(),
            outcome: outcome.into(),
        }
    }
}

#[derive(Debug, Default, Clone, Resource)]
pub struct VoidActionCooldowns {
    ready_at: HashMap<(Entity, VoidActionKind), u64>,
}

impl VoidActionCooldowns {
    pub fn ready_at(&self, entity: Entity, kind: VoidActionKind) -> u64 {
        self.ready_at.get(&(entity, kind)).copied().unwrap_or(0)
    }

    pub fn is_ready(&self, entity: Entity, kind: VoidActionKind, now_tick: u64) -> bool {
        now_tick >= self.ready_at(entity, kind)
    }

    pub fn set_used(&mut self, entity: Entity, kind: VoidActionKind, now_tick: u64) {
        let cooldown = kind.cooldown_ticks();
        if cooldown == 0 {
            return;
        }
        self.ready_at
            .insert((entity, kind), now_tick.saturating_add(cooldown));
    }

    pub fn force_ready_at(&mut self, entity: Entity, kind: VoidActionKind, tick: u64) {
        self.ready_at.insert((entity, kind), tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::Entity;

    #[test]
    fn action_kind_wire_names_are_stable() {
        assert_eq!(VoidActionKind::SuppressTsy.wire_name(), "suppress_tsy");
        assert_eq!(VoidActionKind::ExplodeZone.wire_name(), "explode_zone");
        assert_eq!(VoidActionKind::Barrier.wire_name(), "barrier");
        assert_eq!(VoidActionKind::LegacyAssign.wire_name(), "legacy_assign");
    }

    #[test]
    fn suppress_tsy_cost_matches_plan() {
        assert_eq!(
            VoidActionCost::for_kind(VoidActionKind::SuppressTsy).qi,
            200.0
        );
        assert_eq!(
            VoidActionCost::for_kind(VoidActionKind::SuppressTsy).lifespan_years,
            50
        );
    }

    #[test]
    fn explode_zone_cost_matches_plan() {
        assert_eq!(
            VoidActionCost::for_kind(VoidActionKind::ExplodeZone).qi,
            300.0
        );
        assert_eq!(
            VoidActionCost::for_kind(VoidActionKind::ExplodeZone).lifespan_years,
            100
        );
    }

    #[test]
    fn barrier_cost_matches_plan() {
        assert_eq!(VoidActionCost::for_kind(VoidActionKind::Barrier).qi, 150.0);
        assert_eq!(
            VoidActionCost::for_kind(VoidActionKind::Barrier).lifespan_years,
            30
        );
    }

    #[test]
    fn legacy_assign_has_no_runtime_cost() {
        assert_eq!(VoidActionKind::LegacyAssign.qi_cost(), 0.0);
        assert_eq!(VoidActionKind::LegacyAssign.lifespan_cost_years(), 0);
    }

    #[test]
    fn cooldowns_follow_plan_days() {
        assert_eq!(VoidActionKind::Barrier.cooldown_ticks(), 7 * TICKS_PER_DAY);
        assert_eq!(
            VoidActionKind::SuppressTsy.cooldown_ticks(),
            30 * TICKS_PER_DAY
        );
    }

    #[test]
    fn barrier_geometry_contains_center() {
        let geometry = BarrierGeometry::circle([10.0, 64.0, 20.0], 8.0);
        assert!(geometry.contains([10.0, 70.0, 20.0]));
    }

    #[test]
    fn barrier_geometry_rejects_outside_radius() {
        let geometry = BarrierGeometry::circle([0.0, 64.0, 0.0], 5.0);
        assert!(!geometry.contains([6.0, 64.0, 0.0]));
    }

    #[test]
    fn barrier_geometry_ignores_vertical_distance() {
        let geometry = BarrierGeometry::circle([0.0, 64.0, 0.0], 5.0);
        assert!(geometry.contains([3.0, 120.0, 4.0]));
    }

    #[test]
    fn barrier_radius_is_clamped_non_negative() {
        let geometry = BarrierGeometry::circle([0.0, 0.0, 0.0], -5.0);
        assert_eq!(geometry.radius(), 0.0);
    }

    #[test]
    fn barrier_field_expiry_uses_one_month_ttl() {
        let field = BarrierField::new(
            "offline:Void",
            "spawn",
            BarrierGeometry::circle([0.0, 64.0, 0.0], 12.0),
            100,
        );
        assert_eq!(field.expires_at_tick, 100 + BARRIER_TTL_TICKS);
    }

    #[test]
    fn barrier_field_reports_not_expired_before_deadline() {
        let field = BarrierField::new(
            "offline:Void",
            "spawn",
            BarrierGeometry::circle([0.0, 64.0, 0.0], 12.0),
            100,
        );
        assert!(!field.expired(field.expires_at_tick - 1));
    }

    #[test]
    fn barrier_field_reports_expired_at_deadline() {
        let field = BarrierField::new(
            "offline:Void",
            "spawn",
            BarrierGeometry::circle([0.0, 64.0, 0.0], 12.0),
            100,
        );
        assert!(field.expired(field.expires_at_tick));
    }

    #[test]
    fn log_entry_copies_action_costs() {
        let entry = VoidActionLogEntry::accepted(
            VoidActionKind::SuppressTsy,
            "tsy_lingxu_01",
            42,
            "declining",
        );
        assert_eq!(entry.qi_cost, 200.0);
        assert_eq!(entry.lifespan_cost_years, 50);
    }

    #[test]
    fn cooldown_default_is_ready() {
        let cooldowns = VoidActionCooldowns::default();
        assert!(cooldowns.is_ready(Entity::PLACEHOLDER, VoidActionKind::Barrier, 0));
    }

    #[test]
    fn cooldown_set_used_blocks_until_ready_tick() {
        let mut cooldowns = VoidActionCooldowns::default();
        cooldowns.set_used(Entity::PLACEHOLDER, VoidActionKind::Barrier, 10);
        assert!(!cooldowns.is_ready(Entity::PLACEHOLDER, VoidActionKind::Barrier, 10));
        assert!(cooldowns.is_ready(
            Entity::PLACEHOLDER,
            VoidActionKind::Barrier,
            10 + BARRIER_COOLDOWN_TICKS
        ));
    }

    #[test]
    fn legacy_assign_does_not_set_cooldown() {
        let mut cooldowns = VoidActionCooldowns::default();
        cooldowns.set_used(Entity::PLACEHOLDER, VoidActionKind::LegacyAssign, 10);
        assert_eq!(
            cooldowns.ready_at(Entity::PLACEHOLDER, VoidActionKind::LegacyAssign),
            0
        );
    }

    #[test]
    fn force_ready_at_overrides_cooldown_for_tests() {
        let mut cooldowns = VoidActionCooldowns::default();
        cooldowns.set_used(Entity::PLACEHOLDER, VoidActionKind::Barrier, 10);
        cooldowns.force_ready_at(Entity::PLACEHOLDER, VoidActionKind::Barrier, 12);
        assert_eq!(
            cooldowns.ready_at(Entity::PLACEHOLDER, VoidActionKind::Barrier),
            12
        );
    }
}
