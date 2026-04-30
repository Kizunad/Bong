use valence::prelude::{
    bevy_ecs, Client, Commands, Component, Entity, Query, Res, ViewDistance, With,
};

use crate::cultivation::tick::CultivationClock;

pub const RAMP_INTERVAL_TICKS: u64 = 20;
pub const RAMP_MAX_CHUNKS_PER_STEP: u8 = 2;

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct RealmVisionViewDistanceRamp {
    pub target_chunks: u8,
    pub last_step_tick: u64,
}

pub fn begin_view_distance_ramp(
    commands: &mut Commands,
    entity: Entity,
    view_distance: &mut ViewDistance,
    target_chunks: u8,
    now_tick: u64,
) {
    let current = view_distance.get();
    let next = next_view_distance_step(current, target_chunks, RAMP_MAX_CHUNKS_PER_STEP);
    view_distance.set(next);
    if next == target_chunks {
        commands
            .entity(entity)
            .remove::<RealmVisionViewDistanceRamp>();
    } else {
        commands.entity(entity).insert(RealmVisionViewDistanceRamp {
            target_chunks,
            last_step_tick: now_tick,
        });
    }
}

pub fn view_distance_ramp_system(
    mut commands: Commands,
    clock: Res<CultivationClock>,
    mut clients: Query<(Entity, &mut ViewDistance, &mut RealmVisionViewDistanceRamp), With<Client>>,
) {
    for (entity, mut view_distance, mut ramp) in &mut clients {
        if clock.tick.saturating_sub(ramp.last_step_tick) < RAMP_INTERVAL_TICKS {
            continue;
        }
        let current = view_distance.get();
        let next = next_view_distance_step(current, ramp.target_chunks, RAMP_MAX_CHUNKS_PER_STEP);
        view_distance.set(next);
        ramp.last_step_tick = clock.tick;
        if next == ramp.target_chunks {
            commands
                .entity(entity)
                .remove::<RealmVisionViewDistanceRamp>();
        }
    }
}

pub fn next_view_distance_step(current: u8, target: u8, max_delta: u8) -> u8 {
    if current == target || max_delta == 0 {
        return current;
    }
    if current < target {
        current.saturating_add(max_delta).min(target)
    } else {
        current.saturating_sub(max_delta).max(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_distance_ramp_smoothing_expands_and_contracts_by_two() {
        assert_eq!(next_view_distance_step(4, 20, 2), 6);
        assert_eq!(next_view_distance_step(18, 20, 2), 20);
        assert_eq!(next_view_distance_step(20, 4, 2), 18);
        assert_eq!(next_view_distance_step(5, 4, 2), 4);
    }
}
