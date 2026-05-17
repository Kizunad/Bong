//! plan-knockback-physics-v1 — combat-facing knockback mapping and events.

use valence::prelude::{bevy_ecs, Entity, Event};

use crate::combat::body_mass::{BodyMass, Stance};
use crate::combat::events::AttackSource;
use crate::combat::weapon::WeaponKind;
use crate::cultivation::components::Cultivation;
use crate::qi_physics::{compute_knockback, KnockbackInput, KnockbackResult, QiPhysicsError};

pub const COMBAT_KNOCKBACK_FORCE_UNIT: f64 = 250.0;
pub const DEFAULT_CHAIN_DEPTH: u8 = 3;

#[derive(Debug, Clone, Event)]
#[allow(dead_code)]
pub struct KnockbackEvent {
    pub attacker: Entity,
    pub target: Entity,
    pub source: AttackSource,
    pub distance_blocks: f64,
    pub velocity_blocks_per_tick: f64,
    pub duration_ticks: u32,
    pub kinetic_energy: f64,
    pub collision_damage: Option<f32>,
    pub chain_depth: u8,
    pub block_broken: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CombatKnockbackInput<'a> {
    pub physical_damage: f32,
    pub qi_invest: f32,
    pub attacker_mass: Option<&'a BodyMass>,
    pub target_mass: Option<&'a BodyMass>,
    pub target_stance: Option<&'a Stance>,
    pub target_cultivation: Option<&'a Cultivation>,
    pub weapon_kind: Option<WeaponKind>,
    pub source: AttackSource,
}

pub fn compute_combat_knockback(
    input: CombatKnockbackInput<'_>,
) -> Result<KnockbackResult, QiPhysicsError> {
    let attacker_mass = input
        .attacker_mass
        .copied()
        .unwrap_or_default()
        .total_mass();
    let target_mass = input.target_mass.copied().unwrap_or_default().total_mass();
    let stance_factor = input.target_stance.copied().unwrap_or_default().factor();
    let target_qi_fill_ratio = input
        .target_cultivation
        .map(qi_fill_ratio)
        .unwrap_or_default();
    let knockback_efficiency =
        weapon_efficiency(input.weapon_kind) * attack_source_knockback_modifier(input.source);

    compute_knockback(KnockbackInput {
        physical_damage: f64::from(input.physical_damage.max(0.0)) * COMBAT_KNOCKBACK_FORCE_UNIT,
        qi_invest: f64::from(input.qi_invest.max(0.0)) * COMBAT_KNOCKBACK_FORCE_UNIT,
        attacker_mass,
        target_mass,
        stance_factor,
        target_qi_fill_ratio,
        knockback_efficiency,
    })
}

pub fn weapon_efficiency(weapon_kind: Option<WeaponKind>) -> f64 {
    match weapon_kind {
        Some(WeaponKind::Staff) => 1.2,
        Some(WeaponKind::Saber) => 0.9,
        Some(WeaponKind::Fist) | None => 0.8,
        Some(WeaponKind::Sword) => 0.6,
        Some(WeaponKind::Spear) => 0.4,
        Some(WeaponKind::Dagger) => 0.3,
        Some(WeaponKind::Bow) => 0.1,
    }
}

pub fn attack_source_knockback_modifier(source: AttackSource) -> f64 {
    match source {
        AttackSource::Melee => 1.0,
        AttackSource::BurstMeridian => 2.5,
        AttackSource::FullPower => 5.0,
        AttackSource::QiNeedle => 0.05,
        AttackSource::SwordCleave => 1.2,
        AttackSource::SwordThrust => 0.7,
        // plan-sword-path-v2 §P1.6：凝锋是下一击的附魔（贴近物理斩击量级），
        // 剑气斩远袭轻盈，剑鸣 AoE 偏震慑（轻击退），剑意化形是凝实剑灵（中等）。
        AttackSource::SwordPathCondenseEdge => 1.4,
        AttackSource::SwordPathQiSlash => 0.5,
        AttackSource::SwordPathResonance => 0.3,
        AttackSource::SwordPathManifest => 1.0,
    }
}

pub fn qi_fill_ratio(cultivation: &Cultivation) -> f64 {
    if cultivation.qi_max <= f64::EPSILON {
        0.0
    } else {
        (cultivation.qi_current / cultivation.qi_max).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Cultivation, Realm};

    #[test]
    fn weapon_efficiency_matches_plan_table() {
        assert_eq!(weapon_efficiency(Some(WeaponKind::Staff)), 1.2);
        assert_eq!(weapon_efficiency(Some(WeaponKind::Sword)), 0.6);
        assert_eq!(weapon_efficiency(Some(WeaponKind::Bow)), 0.1);
        assert_eq!(weapon_efficiency(None), 0.8);
    }

    #[test]
    fn source_modifier_keeps_qi_needle_nearly_stationary() {
        assert_eq!(
            attack_source_knockback_modifier(AttackSource::BurstMeridian),
            2.5
        );
        assert_eq!(
            attack_source_knockback_modifier(AttackSource::FullPower),
            5.0
        );
        assert!(attack_source_knockback_modifier(AttackSource::QiNeedle) < 0.1);
        assert_eq!(
            attack_source_knockback_modifier(AttackSource::SwordCleave),
            1.2
        );
        assert_eq!(
            attack_source_knockback_modifier(AttackSource::SwordThrust),
            0.7
        );
    }

    #[test]
    fn full_qi_target_resists_more_than_empty_target() {
        let attacker = BodyMass::human();
        let empty = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            realm: Realm::Awaken,
            ..Default::default()
        };
        let full = Cultivation {
            qi_current: 100.0,
            ..empty.clone()
        };

        let target_mass = BodyMass::human();
        let empty_result = compute_combat_knockback(CombatKnockbackInput {
            physical_damage: 10.0,
            qi_invest: 10.0,
            attacker_mass: Some(&attacker),
            target_mass: Some(&target_mass),
            target_stance: Some(&Stance::Standing),
            target_cultivation: Some(&empty),
            weapon_kind: None,
            source: AttackSource::Melee,
        })
        .unwrap();
        let full_result = compute_combat_knockback(CombatKnockbackInput {
            physical_damage: 10.0,
            qi_invest: 10.0,
            attacker_mass: Some(&attacker),
            target_mass: Some(&target_mass),
            target_stance: Some(&Stance::Standing),
            target_cultivation: Some(&full),
            weapon_kind: None,
            source: AttackSource::Melee,
        })
        .unwrap();

        assert!(full_result.distance_blocks < empty_result.distance_blocks);
    }

    #[test]
    fn npc_baseline_keeps_legacy_four_block_feel() {
        let attacker = BodyMass::human();
        let target = BodyMass::human();
        let result = compute_combat_knockback(CombatKnockbackInput {
            physical_damage: 10.0,
            qi_invest: 10.0,
            attacker_mass: Some(&attacker),
            target_mass: Some(&target),
            target_stance: Some(&Stance::Standing),
            target_cultivation: None,
            weapon_kind: None,
            source: AttackSource::Melee,
        })
        .unwrap();

        assert!(
            (3.0..=5.0).contains(&result.distance_blocks),
            "legacy NPC melee should remain around 4 blocks, got {}",
            result.distance_blocks
        );
    }
}
