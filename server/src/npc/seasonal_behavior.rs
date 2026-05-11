use valence::prelude::{bevy_ecs, App, Commands, Component, Entity, Query, Res, Update, With};

use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;
use crate::world::season::Season;
use crate::world::season::WorldSeasonState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcSeasonArchetype {
    ScatteredCultivator,
    Commoner,
    HighRealmCultivator,
    GraveKeeper,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcSeasonBehavior {
    pub speed_multiplier: f32,
    pub movement_radius_multiplier: f32,
    pub dialogue_frequency_multiplier: f32,
    pub yaw_jitter_degrees: f32,
    pub indoor_bias: f32,
    pub prepares_tribulation: bool,
    pub bubble: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct NpcSeasonRuntime {
    pub archetype: NpcSeasonArchetype,
    pub behavior: NpcSeasonBehavior,
}

pub fn register(app: &mut App) {
    app.add_systems(Update, apply_npc_season_runtime_system);
}

pub fn apply_npc_season_runtime_system(
    season_state: Option<Res<WorldSeasonState>>,
    mut commands: Commands,
    mut npcs: Query<(Entity, &NpcArchetype, Option<&mut NpcSeasonRuntime>), With<NpcMarker>>,
) {
    let season = season_state
        .as_deref()
        .map(|state| state.current.season)
        .unwrap_or_default();
    for (entity, archetype, runtime) in &mut npcs {
        let season_archetype = season_archetype_for(*archetype);
        let next = NpcSeasonRuntime {
            archetype: season_archetype,
            behavior: behavior_for_season(season, season_archetype),
        };
        if let Some(mut runtime) = runtime {
            *runtime = next;
        } else {
            commands.entity(entity).insert(next);
        }
    }
}

pub const fn season_archetype_for(archetype: NpcArchetype) -> NpcSeasonArchetype {
    match archetype {
        NpcArchetype::Commoner => NpcSeasonArchetype::Commoner,
        NpcArchetype::Disciple => NpcSeasonArchetype::HighRealmCultivator,
        NpcArchetype::GuardianRelic => NpcSeasonArchetype::GraveKeeper,
        _ => NpcSeasonArchetype::ScatteredCultivator,
    }
}

pub fn behavior_for_season(season: Season, archetype: NpcSeasonArchetype) -> NpcSeasonBehavior {
    if matches!(archetype, NpcSeasonArchetype::GraveKeeper) {
        return NpcSeasonBehavior::neutral();
    }
    match season {
        Season::Summer => summer_behavior(archetype),
        Season::Winter => winter_behavior(archetype),
        Season::SummerToWinter | Season::WinterToSummer => tide_turn_behavior(archetype),
    }
}

impl NpcSeasonBehavior {
    const fn neutral() -> Self {
        Self {
            speed_multiplier: 1.0,
            movement_radius_multiplier: 1.0,
            dialogue_frequency_multiplier: 1.0,
            yaw_jitter_degrees: 0.0,
            indoor_bias: 0.0,
            prepares_tribulation: false,
            bubble: None,
        }
    }
}

fn summer_behavior(archetype: NpcSeasonArchetype) -> NpcSeasonBehavior {
    match archetype {
        NpcSeasonArchetype::ScatteredCultivator => NpcSeasonBehavior {
            speed_multiplier: 1.30,
            movement_radius_multiplier: 1.15,
            dialogue_frequency_multiplier: 1.30,
            bubble: Some("天热，灵草长得倒快。"),
            ..NpcSeasonBehavior::neutral()
        },
        NpcSeasonArchetype::Commoner => NpcSeasonBehavior {
            speed_multiplier: 0.90,
            movement_radius_multiplier: 0.70,
            indoor_bias: 0.65,
            bubble: Some("树荫下还能喘口气。"),
            ..NpcSeasonBehavior::neutral()
        },
        NpcSeasonArchetype::HighRealmCultivator => NpcSeasonBehavior {
            speed_multiplier: 1.10,
            movement_radius_multiplier: 1.05,
            prepares_tribulation: true,
            ..NpcSeasonBehavior::neutral()
        },
        NpcSeasonArchetype::GraveKeeper => NpcSeasonBehavior::neutral(),
    }
}

fn winter_behavior(archetype: NpcSeasonArchetype) -> NpcSeasonBehavior {
    match archetype {
        NpcSeasonArchetype::ScatteredCultivator => NpcSeasonBehavior {
            speed_multiplier: 0.70,
            movement_radius_multiplier: 0.50,
            dialogue_frequency_multiplier: 0.70,
            bubble: Some("...太冷了。骨币也缩水了。"),
            ..NpcSeasonBehavior::neutral()
        },
        NpcSeasonArchetype::Commoner => NpcSeasonBehavior {
            speed_multiplier: 0.65,
            movement_radius_multiplier: 0.35,
            indoor_bias: 1.0,
            bubble: Some("大仙，小人家里没柴了..."),
            ..NpcSeasonBehavior::neutral()
        },
        NpcSeasonArchetype::HighRealmCultivator => NpcSeasonBehavior {
            speed_multiplier: 0.80,
            movement_radius_multiplier: 0.55,
            ..NpcSeasonBehavior::neutral()
        },
        NpcSeasonArchetype::GraveKeeper => NpcSeasonBehavior::neutral(),
    }
}

fn tide_turn_behavior(archetype: NpcSeasonArchetype) -> NpcSeasonBehavior {
    match archetype {
        NpcSeasonArchetype::HighRealmCultivator => NpcSeasonBehavior {
            yaw_jitter_degrees: 30.0,
            prepares_tribulation: true,
            bubble: Some("你也感觉到了？...天地间，不太对。"),
            ..NpcSeasonBehavior::neutral()
        },
        NpcSeasonArchetype::ScatteredCultivator | NpcSeasonArchetype::Commoner => {
            NpcSeasonBehavior {
                speed_multiplier: 1.05,
                movement_radius_multiplier: 1.20,
                dialogue_frequency_multiplier: 1.25,
                yaw_jitter_degrees: 30.0,
                bubble: Some("你也感觉到了？...天地间，不太对。"),
                ..NpcSeasonBehavior::neutral()
            }
        }
        NpcSeasonArchetype::GraveKeeper => NpcSeasonBehavior::neutral(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::season::{SeasonState, WorldSeasonState};
    use valence::prelude::{App, DVec3};

    #[test]
    fn npc_activity_by_season() {
        let summer = behavior_for_season(Season::Summer, NpcSeasonArchetype::ScatteredCultivator);
        let winter = behavior_for_season(Season::Winter, NpcSeasonArchetype::ScatteredCultivator);
        let tide = behavior_for_season(
            Season::SummerToWinter,
            NpcSeasonArchetype::ScatteredCultivator,
        );
        let commoner = behavior_for_season(Season::Winter, NpcSeasonArchetype::Commoner);
        let high_realm = behavior_for_season(
            Season::SummerToWinter,
            NpcSeasonArchetype::HighRealmCultivator,
        );

        assert!((summer.speed_multiplier - 1.30).abs() < 1e-6);
        assert!((winter.movement_radius_multiplier - 0.50).abs() < 1e-6);
        assert_eq!(tide.yaw_jitter_degrees, 30.0);
        assert_eq!(commoner.indoor_bias, 1.0);
        assert!(high_realm.prepares_tribulation);
    }

    #[test]
    fn grave_keeper_ignores_all_season_visual_behavior() {
        for season in [
            Season::Summer,
            Season::SummerToWinter,
            Season::Winter,
            Season::WinterToSummer,
        ] {
            assert_eq!(
                behavior_for_season(season, NpcSeasonArchetype::GraveKeeper),
                NpcSeasonBehavior::neutral()
            );
        }
    }

    #[test]
    fn seasonal_runtime_is_applied_to_npc_components() {
        let mut app = App::new();
        let mut season_state = WorldSeasonState::default();
        season_state.current = SeasonState {
            season: Season::Winter,
            tick_into_phase: 10,
            phase_total_ticks: 100,
            year_index: 0,
        };
        app.insert_resource(season_state);
        app.add_systems(Update, apply_npc_season_runtime_system);
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Commoner,
                valence::prelude::Position(DVec3::new(0.0, 64.0, 0.0)),
            ))
            .id();

        app.update();

        let runtime = app.world().get::<NpcSeasonRuntime>(npc).unwrap();
        assert_eq!(runtime.archetype, NpcSeasonArchetype::Commoner);
        assert_eq!(runtime.behavior.indoor_bias, 1.0);
        assert!(runtime.behavior.speed_multiplier < 1.0);
    }
}
