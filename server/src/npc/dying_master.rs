use crate::cultivation::tick::CultivationClock;
use valence::prelude::{bevy_ecs, Commands, Component, Entity, Event, Query, Res};

pub const DYING_MASTER_SPAWN_CHANCE: f64 = 0.005;
pub const DYING_MASTER_DESPAWN_TICKS: u64 = 20 * 30;

#[derive(Debug, Clone, Component, PartialEq)]
pub struct DyingMaster {
    pub spawned_at_tick: u64,
    pub despawn_at_tick: u64,
    pub negative_zone_qi: f64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct DyingMasterEncounterEvent {
    pub player: Entity,
    pub npc_entity: Entity,
    pub zone_qi: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DyingMasterAidOutcome {
    Teaches,
    SeizeBody,
}

pub fn should_spawn_in_negative_zone(zone_qi: f64, is_cave_chunk: bool, roll: f64) -> bool {
    is_cave_chunk && zone_qi < -0.3 && roll.is_finite() && roll < DYING_MASTER_SPAWN_CHANCE
}

pub fn path_a_aid_outcome(roll: f64) -> DyingMasterAidOutcome {
    if roll.is_finite() && roll < 0.5 {
        DyingMasterAidOutcome::Teaches
    } else {
        DyingMasterAidOutcome::SeizeBody
    }
}

pub fn path_c_earth_scroll_drop(seed: u64) -> &'static str {
    const EARTH_SCROLLS: [&str; 2] = ["scroll_woliu_heart", "scroll_woliu_turbulence_burst"];
    EARTH_SCROLLS[(seed as usize) % EARTH_SCROLLS.len()]
}

pub fn seize_body_triggers_combat(outcome: DyingMasterAidOutcome) -> bool {
    matches!(outcome, DyingMasterAidOutcome::SeizeBody)
}

pub fn log_dying_master_contract() {
    let can_spawn = should_spawn_in_negative_zone(-0.31, true, DYING_MASTER_SPAWN_CHANCE / 2.0);
    let aid_outcome = path_a_aid_outcome(0.75);
    tracing::debug!(
        "[bong][woliu-v4] dying master contract spawn_chance={} can_spawn={} path_a_combat={} path_c_drop={}",
        DYING_MASTER_SPAWN_CHANCE,
        can_spawn,
        seize_body_triggers_combat(aid_outcome),
        path_c_earth_scroll_drop(0)
    );
}

pub fn dying_master_despawn_tick(
    mut commands: Commands,
    clock: Res<CultivationClock>,
    masters: Query<(Entity, &DyingMaster)>,
) {
    let now = clock.tick;
    for (entity, master) in &masters {
        let despawn_at = if master.despawn_at_tick == 0 {
            master
                .spawned_at_tick
                .saturating_add(DYING_MASTER_DESPAWN_TICKS)
        } else {
            master.despawn_at_tick
        };
        if now >= despawn_at {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_spawn_in_negative_zone() {
        assert!(should_spawn_in_negative_zone(-0.31, true, 0.004));
        assert!(!should_spawn_in_negative_zone(-0.29, true, 0.004));
        assert!(!should_spawn_in_negative_zone(-0.31, false, 0.004));
    }

    #[test]
    fn component_sets_30s_despawn_window() {
        let master = DyingMaster {
            spawned_at_tick: 100,
            despawn_at_tick: 100 + DYING_MASTER_DESPAWN_TICKS,
            negative_zone_qi: -0.6,
        };
        assert_eq!(100, master.spawned_at_tick);
        assert_eq!(100 + DYING_MASTER_DESPAWN_TICKS, master.despawn_at_tick);
        assert_eq!(-0.6, master.negative_zone_qi);
    }

    #[test]
    fn event_probability_0_5_percent() {
        assert!(should_spawn_in_negative_zone(-0.5, true, 0.00499));
        assert!(!should_spawn_in_negative_zone(-0.5, true, 0.005));
    }

    #[test]
    fn path_c_drop_earth_scroll() {
        for seed in 0..10 {
            assert!(matches!(
                path_c_earth_scroll_drop(seed),
                "scroll_woliu_heart" | "scroll_woliu_turbulence_burst"
            ));
        }
    }

    #[test]
    fn path_a_50_50_split() {
        assert_eq!(path_a_aid_outcome(0.49), DyingMasterAidOutcome::Teaches);
        assert_eq!(path_a_aid_outcome(0.50), DyingMasterAidOutcome::SeizeBody);
    }

    #[test]
    fn seize_body_triggers_combat() {
        assert!(super::seize_body_triggers_combat(
            DyingMasterAidOutcome::SeizeBody
        ));
        assert!(!super::seize_body_triggers_combat(
            DyingMasterAidOutcome::Teaches
        ));
    }
}
