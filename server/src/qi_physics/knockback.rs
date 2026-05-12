//! plan-knockback-physics-v1 — unified knockback and impact physics.

use super::{finite_non_negative, QiPhysicsError};

pub const PHYSICAL_FORCE_RATIO: f64 = 1.0;
pub const QI_FORCE_RATIO: f64 = 2.0;
pub const ATTACKER_MASS_TRANSFER_RATIO: f64 = 0.1;
pub const DISTANCE_SCALE: f64 = 0.05;
pub const QI_ANCHORING_COEFFICIENT: f64 = 0.5;
pub const MAX_KNOCKBACK_DISTANCE: f64 = 30.0;
pub const MAX_BLOCK_PENETRATION: u8 = 3;

const MIN_EFFECTIVE_MASS: f64 = 1.0;
const MIN_STANCE_FACTOR: f64 = 0.05;
const MIN_KNOCKBACK_DURATION_TICKS: u32 = 1;
const TARGET_BLOCKS_PER_TICK: f64 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KnockbackInput {
    pub physical_damage: f64,
    pub qi_invest: f64,
    pub attacker_mass: f64,
    pub target_mass: f64,
    pub stance_factor: f64,
    pub target_qi_fill_ratio: f64,
    pub knockback_efficiency: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KnockbackResult {
    pub force: f64,
    pub resistance: f64,
    pub distance_blocks: f64,
    pub velocity_blocks_per_tick: f64,
    pub duration_ticks: u32,
    pub kinetic_energy: f64,
}

impl KnockbackResult {
    pub fn from_distance(distance_blocks: f64, target_mass: f64) -> Self {
        let distance_blocks = distance_blocks.clamp(0.0, MAX_KNOCKBACK_DISTANCE);
        let duration_ticks = duration_ticks_for_distance(distance_blocks);
        let velocity_blocks_per_tick = velocity_for(distance_blocks, duration_ticks);
        let target_mass = target_mass.max(MIN_EFFECTIVE_MASS);
        Self {
            force: 0.0,
            resistance: target_mass,
            distance_blocks,
            velocity_blocks_per_tick,
            duration_ticks,
            kinetic_energy: kinetic_energy(target_mass, velocity_blocks_per_tick),
        }
    }

    pub fn is_actionable(self) -> bool {
        self.distance_blocks >= 0.05 && self.duration_ticks > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallCollisionInput {
    pub target_mass: f64,
    pub velocity_blocks_per_tick: f64,
    pub block_hardness: f64,
    pub armor_mitigation: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallCollisionResult {
    pub kinetic_energy: f64,
    pub entity_damage: f64,
    pub block_stress: f64,
    pub block_broken: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityCollisionInput {
    pub moving_mass: f64,
    pub hit_mass: f64,
    pub incoming_velocity: f64,
    pub chain_decay: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityCollisionResult {
    pub incoming_damage: f64,
    pub hit_damage: f64,
    pub transferred_velocity: f64,
    pub transferred_distance: f64,
    pub kinetic_energy: f64,
}

pub fn compute_knockback(input: KnockbackInput) -> Result<KnockbackResult, QiPhysicsError> {
    let physical_damage = finite_non_negative(input.physical_damage, "knockback.physical_damage")?;
    let qi_invest = finite_non_negative(input.qi_invest, "knockback.qi_invest")?;
    let attacker_mass = finite_non_negative(input.attacker_mass, "knockback.attacker_mass")?;
    let target_mass =
        finite_non_negative(input.target_mass, "knockback.target_mass")?.max(MIN_EFFECTIVE_MASS);
    let stance_factor =
        finite_non_negative(input.stance_factor, "knockback.stance_factor")?.max(MIN_STANCE_FACTOR);
    let target_qi_fill_ratio =
        finite_non_negative(input.target_qi_fill_ratio, "knockback.target_qi_fill_ratio")?
            .clamp(0.0, 1.0);
    let knockback_efficiency =
        finite_non_negative(input.knockback_efficiency, "knockback.knockback_efficiency")?;

    let force = (physical_damage * PHYSICAL_FORCE_RATIO
        + qi_invest * QI_FORCE_RATIO
        + attacker_mass * ATTACKER_MASS_TRANSFER_RATIO)
        * knockback_efficiency;
    let resistance =
        target_mass * stance_factor * (1.0 + target_qi_fill_ratio * QI_ANCHORING_COEFFICIENT);
    let distance_blocks = if resistance > 0.0 {
        (force / resistance * DISTANCE_SCALE).clamp(0.0, MAX_KNOCKBACK_DISTANCE)
    } else {
        MAX_KNOCKBACK_DISTANCE
    };
    let duration_ticks = duration_ticks_for_distance(distance_blocks);
    let velocity_blocks_per_tick = velocity_for(distance_blocks, duration_ticks);

    Ok(KnockbackResult {
        force,
        resistance,
        distance_blocks,
        velocity_blocks_per_tick,
        duration_ticks,
        kinetic_energy: kinetic_energy(target_mass, velocity_blocks_per_tick),
    })
}

pub fn wall_collision(input: WallCollisionInput) -> Result<WallCollisionResult, QiPhysicsError> {
    let target_mass = finite_non_negative(input.target_mass, "knockback.wall.target_mass")?
        .max(MIN_EFFECTIVE_MASS);
    let velocity = finite_non_negative(
        input.velocity_blocks_per_tick,
        "knockback.wall.velocity_blocks_per_tick",
    )?;
    let block_hardness =
        finite_non_negative(input.block_hardness, "knockback.wall.block_hardness")?;
    let armor_mitigation =
        finite_non_negative(input.armor_mitigation, "knockback.wall.armor_mitigation")?
            .clamp(0.0, 1.0);

    let kinetic_energy = kinetic_energy(target_mass, velocity);
    let entity_damage = kinetic_energy * 0.3 * (1.0 - armor_mitigation);
    let block_stress = kinetic_energy * 0.5;

    Ok(WallCollisionResult {
        kinetic_energy,
        entity_damage,
        block_stress,
        block_broken: block_stress > block_hardness,
    })
}

pub fn entity_collision(
    input: EntityCollisionInput,
) -> Result<EntityCollisionResult, QiPhysicsError> {
    let moving_mass = finite_non_negative(input.moving_mass, "knockback.entity.moving_mass")?
        .max(MIN_EFFECTIVE_MASS);
    let hit_mass =
        finite_non_negative(input.hit_mass, "knockback.entity.hit_mass")?.max(MIN_EFFECTIVE_MASS);
    let incoming_velocity = finite_non_negative(
        input.incoming_velocity,
        "knockback.entity.incoming_velocity",
    )?;
    let chain_decay =
        finite_non_negative(input.chain_decay, "knockback.entity.chain_decay")?.clamp(0.0, 1.0);

    let kinetic_energy = kinetic_energy(moving_mass, incoming_velocity);
    let total_mass = (moving_mass + hit_mass).max(MIN_EFFECTIVE_MASS);
    let moving_ratio = moving_mass / total_mass;
    let hit_ratio = hit_mass / total_mass;
    let transferred_velocity = incoming_velocity * moving_ratio * chain_decay;
    let transferred_distance =
        (transferred_velocity * TARGET_BLOCKS_PER_TICK * 5.0).clamp(0.0, MAX_KNOCKBACK_DISTANCE);

    Ok(EntityCollisionResult {
        incoming_damage: kinetic_energy * hit_ratio * 0.2,
        hit_damage: kinetic_energy * moving_ratio * 0.2,
        transferred_velocity,
        transferred_distance,
        kinetic_energy,
    })
}

fn duration_ticks_for_distance(distance_blocks: f64) -> u32 {
    if distance_blocks <= 0.0 {
        return MIN_KNOCKBACK_DURATION_TICKS;
    }
    (distance_blocks / TARGET_BLOCKS_PER_TICK)
        .ceil()
        .clamp(f64::from(MIN_KNOCKBACK_DURATION_TICKS), 30.0) as u32
}

fn velocity_for(distance_blocks: f64, duration_ticks: u32) -> f64 {
    if duration_ticks == 0 {
        0.0
    } else {
        distance_blocks / f64::from(duration_ticks)
    }
}

fn kinetic_energy(mass: f64, velocity: f64) -> f64 {
    0.5 * mass.max(MIN_EFFECTIVE_MASS) * velocity * velocity
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_input() -> KnockbackInput {
        KnockbackInput {
            physical_damage: 2500.0,
            qi_invest: 2500.0,
            attacker_mass: 70.0,
            target_mass: 70.0,
            stance_factor: 1.0,
            target_qi_fill_ratio: 0.0,
            knockback_efficiency: 1.0,
        }
    }

    #[test]
    fn formula_combines_physical_qi_mass_and_efficiency() {
        let out = compute_knockback(baseline_input()).unwrap();
        assert!((out.force - 7507.0).abs() < f64::EPSILON);
        assert!((out.resistance - 70.0).abs() < f64::EPSILON);
        assert!((out.distance_blocks - 5.362142857142858).abs() < 1e-9);
        assert!(out.is_actionable());
    }

    #[test]
    fn qi_fill_ratio_anchors_the_target() {
        let empty = compute_knockback(baseline_input()).unwrap();
        let full = compute_knockback(KnockbackInput {
            target_qi_fill_ratio: 1.0,
            ..baseline_input()
        })
        .unwrap();

        assert!(full.resistance > empty.resistance);
        assert!(full.distance_blocks < empty.distance_blocks);
    }

    #[test]
    fn rooted_stance_resists_more_than_exhausted_stance() {
        let rooted = compute_knockback(KnockbackInput {
            stance_factor: 2.5,
            ..baseline_input()
        })
        .unwrap();
        let exhausted = compute_knockback(KnockbackInput {
            stance_factor: 0.4,
            ..baseline_input()
        })
        .unwrap();

        assert!(rooted.distance_blocks < exhausted.distance_blocks);
    }

    #[test]
    fn full_power_impacts_clamp_at_max_distance() {
        let out = compute_knockback(KnockbackInput {
            physical_damage: 200_000.0,
            qi_invest: 200_000.0,
            knockback_efficiency: 5.0,
            ..baseline_input()
        })
        .unwrap();

        assert_eq!(out.distance_blocks, MAX_KNOCKBACK_DISTANCE);
    }

    #[test]
    fn wall_collision_applies_armor_and_breaks_soft_blocks() {
        let soft = wall_collision(WallCollisionInput {
            target_mass: 70.0,
            velocity_blocks_per_tick: 0.8,
            block_hardness: 1.0,
            armor_mitigation: 0.5,
        })
        .unwrap();
        let hard = wall_collision(WallCollisionInput {
            block_hardness: 50.0,
            ..WallCollisionInput {
                target_mass: 70.0,
                velocity_blocks_per_tick: 0.8,
                block_hardness: 1.0,
                armor_mitigation: 0.5,
            }
        })
        .unwrap();

        assert!(soft.block_broken);
        assert!(!hard.block_broken);
        assert!((soft.entity_damage - 3.36).abs() < 1e-9);
    }

    #[test]
    fn entity_collision_transfers_less_velocity_to_heavier_targets() {
        let light = entity_collision(EntityCollisionInput {
            moving_mass: 70.0,
            hit_mass: 35.0,
            incoming_velocity: 0.8,
            chain_decay: 0.5,
        })
        .unwrap();
        let heavy = entity_collision(EntityCollisionInput {
            hit_mass: 140.0,
            ..EntityCollisionInput {
                moving_mass: 70.0,
                hit_mass: 35.0,
                incoming_velocity: 0.8,
                chain_decay: 0.5,
            }
        })
        .unwrap();

        assert!(light.transferred_velocity > heavy.transferred_velocity);
        assert!(light.transferred_distance > heavy.transferred_distance);
    }
}
