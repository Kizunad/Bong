use valence::entity::{OnGround, Velocity};
use valence::prelude::{
    bevy_ecs, Client, Commands, Component, DVec3, Entity, Query, Vec3, With, Without,
};

use crate::npc::movement::{MovementController, PendingKnockback};

pub const KNOCKBACK_RECOVERY_TICKS: u32 = 5;
pub const KNOCKBACK_RECOVERY_SPEED_MULTIPLIER: f32 = 0.5;
const GROUND_FRICTION: f64 = 0.85;
const AIR_FRICTION: f64 = 0.95;
const MIN_KNOCKBACK_DISTANCE_BLOCKS: f64 = 0.05;

#[derive(Debug, Clone, Component)]
pub struct ActivePlayerKnockback {
    pub velocity: DVec3,
    pub remaining_ticks: u32,
    pub recovery_ticks: u32,
    pub source_entity: Option<Entity>,
}

impl ActivePlayerKnockback {
    pub fn is_displacing(&self) -> bool {
        self.remaining_ticks > 0
    }

    pub fn is_recovery_only(&self) -> bool {
        self.remaining_ticks == 0 && self.recovery_ticks > 0
    }
}

type PendingPlayerKnockback<'a> = (
    Entity,
    &'a PendingKnockback,
    &'a mut Client,
    Option<&'a mut Velocity>,
);

type ActivePlayerKnockbackItem<'a> = (
    Entity,
    &'a mut ActivePlayerKnockback,
    &'a mut Client,
    Option<&'a mut Velocity>,
    Option<&'a OnGround>,
);

pub fn apply_pending_player_knockback_system(
    mut commands: Commands,
    mut players: Query<PendingPlayerKnockback<'_>, (With<Client>, Without<MovementController>)>,
) {
    for (entity, pending, mut client, velocity) in &mut players {
        commands.entity(entity).remove::<PendingKnockback>();
        if pending.distance_blocks < MIN_KNOCKBACK_DISTANCE_BLOCKS {
            continue;
        }
        let horizontal = normalize_horizontal(pending.direction);
        let initial_velocity = horizontal * pending.velocity_blocks_per_tick.max(0.0);
        let active = ActivePlayerKnockback {
            velocity: initial_velocity,
            remaining_ticks: pending.duration_ticks.max(1),
            recovery_ticks: KNOCKBACK_RECOVERY_TICKS,
            source_entity: pending.attacker,
        };
        set_velocity(&mut client, velocity, initial_velocity);
        commands.entity(entity).insert(active);
    }
}

pub fn tick_active_player_knockback_system(
    mut commands: Commands,
    mut players: Query<ActivePlayerKnockbackItem<'_>>,
) {
    for (entity, mut knockback, mut client, velocity, on_ground) in &mut players {
        if knockback.remaining_ticks > 0 {
            set_velocity(&mut client, velocity, knockback.velocity);
            knockback.remaining_ticks = knockback.remaining_ticks.saturating_sub(1);
            let friction = if on_ground.is_some_and(|ground| ground.0) {
                GROUND_FRICTION
            } else {
                AIR_FRICTION
            };
            knockback.velocity *= friction;
            continue;
        }
        if knockback.recovery_ticks > 0 {
            knockback.recovery_ticks = knockback.recovery_ticks.saturating_sub(1);
            continue;
        }
        tracing::debug!(
            "[bong][movement] player knockback recovery ended entity={entity:?} source={:?}",
            knockback.source_entity
        );
        commands.entity(entity).remove::<ActivePlayerKnockback>();
    }
}

fn set_velocity(
    client: &mut Client,
    velocity: Option<valence::prelude::Mut<'_, Velocity>>,
    v: DVec3,
) {
    let next = Vec3::new(v.x as f32, v.y as f32, v.z as f32);
    if let Some(mut velocity) = velocity {
        velocity.0 = next;
    }
    client.set_velocity(next);
}

fn normalize_horizontal(dir: DVec3) -> DVec3 {
    let horizontal = DVec3::new(dir.x, 0.0, dir.z);
    if horizontal.length_squared() <= 1e-8 {
        DVec3::new(0.0, 0.0, 1.0)
    } else {
        horizontal.normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_knockback_state_reports_displacement_and_recovery() {
        let mut active = ActivePlayerKnockback {
            velocity: DVec3::new(1.0, 0.0, 0.0),
            remaining_ticks: 2,
            recovery_ticks: KNOCKBACK_RECOVERY_TICKS,
            source_entity: None,
        };

        assert!(active.is_displacing());
        active.remaining_ticks = 0;
        assert!(active.is_recovery_only());
    }
}
