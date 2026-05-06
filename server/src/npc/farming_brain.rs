use big_brain::prelude::{ActionBuilder, ActionState, Actor, BigBrainSet, Score, ScorerBuilder};
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, BlockPos, Commands, Component, DVec3, Entity, IntoSystemConfigs, Position,
    PreUpdate, Query, Res, ResMut, With, Without,
};

use crate::botany::PlantKindRegistry;
use crate::cultivation::components::Cultivation;
use crate::lingtian::environment::PlotEnvironment;
use crate::lingtian::hoe::HoeKind;
use crate::lingtian::plot::LingtianPlot;
use crate::lingtian::session::{
    HarvestSession, PlantingSession, ReplenishSession, ReplenishSource, SessionMode, TillSession,
};
use crate::lingtian::systems::{ActiveLingtianSessions, ActiveSession};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::world::zone::ZoneRegistry;

use super::movement::GameTick;
use super::scattered_cultivator::{FarmingTemperament, ScatteredCultivator};

const FARMING_ACTION_SPEED: f64 = 0.65;
const MIGRATION_SUCCESS_DISTANCE: f64 = 3.0;

type FarmingNpcQueryItem<'a> = (
    &'a Position,
    &'a mut NpcPatrol,
    &'a mut Navigator,
    &'a mut ScatteredCultivator,
);
type FarmingNpcQueryFilter = (With<NpcMarker>, Without<ClientMarker>);

#[derive(Clone, Copy, Debug, Component)]
pub struct SoilSuitabilityScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct QiDensityScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct OwnQiPoolScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct SeasonScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct NearbyThreatScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct ToolPossessionScorer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LingtianFarmingIntent {
    Till,
    Plant,
    Harvest,
    Replenish,
    Migrate,
}

#[derive(Clone, Copy, Debug, Component)]
pub struct LingtianFarmingScorer {
    pub intent: LingtianFarmingIntent,
}

impl LingtianFarmingScorer {
    pub const fn till() -> Self {
        Self {
            intent: LingtianFarmingIntent::Till,
        }
    }

    pub const fn plant() -> Self {
        Self {
            intent: LingtianFarmingIntent::Plant,
        }
    }

    pub const fn harvest() -> Self {
        Self {
            intent: LingtianFarmingIntent::Harvest,
        }
    }

    pub const fn replenish() -> Self {
        Self {
            intent: LingtianFarmingIntent::Replenish,
        }
    }

    pub const fn migrate() -> Self {
        Self {
            intent: LingtianFarmingIntent::Migrate,
        }
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub struct TillAction;

#[derive(Clone, Copy, Debug, Component)]
pub struct PlantAction;

#[derive(Clone, Copy, Debug, Component)]
pub struct HarvestAction;

#[derive(Clone, Copy, Debug, Component)]
pub struct ReplenishAction;

#[derive(Clone, Copy, Debug, Component)]
pub struct MigrateAction;

macro_rules! simple_scorer_builder {
    ($ty:ty, $label:literal) => {
        impl ScorerBuilder for $ty {
            fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
                cmd.entity(scorer).insert(*self);
            }

            fn label(&self) -> Option<&str> {
                Some($label)
            }
        }
    };
}

simple_scorer_builder!(SoilSuitabilityScorer, "SoilSuitabilityScorer");
simple_scorer_builder!(QiDensityScorer, "QiDensityScorer");
simple_scorer_builder!(OwnQiPoolScorer, "OwnQiPoolScorer");
simple_scorer_builder!(SeasonScorer, "SeasonScorer");
simple_scorer_builder!(NearbyThreatScorer, "NearbyThreatScorer");
simple_scorer_builder!(ToolPossessionScorer, "ToolPossessionScorer");

impl ScorerBuilder for LingtianFarmingScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some(match self.intent {
            LingtianFarmingIntent::Till => "LingtianFarmingScorer::Till",
            LingtianFarmingIntent::Plant => "LingtianFarmingScorer::Plant",
            LingtianFarmingIntent::Harvest => "LingtianFarmingScorer::Harvest",
            LingtianFarmingIntent::Replenish => "LingtianFarmingScorer::Replenish",
            LingtianFarmingIntent::Migrate => "LingtianFarmingScorer::Migrate",
        })
    }
}

macro_rules! simple_action_builder {
    ($ty:ty, $label:literal) => {
        impl ActionBuilder for $ty {
            fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
                cmd.entity(action).insert(*self);
            }

            fn label(&self) -> Option<&str> {
                Some($label)
            }
        }
    };
}

simple_action_builder!(TillAction, "TillAction");
simple_action_builder!(PlantAction, "PlantAction");
simple_action_builder!(HarvestAction, "HarvestAction");
simple_action_builder!(ReplenishAction, "ReplenishAction");
simple_action_builder!(MigrateAction, "MigrateAction");

pub fn register(app: &mut App) {
    app.add_systems(
        PreUpdate,
        (
            soil_suitability_scorer_system,
            qi_density_scorer_system,
            own_qi_pool_scorer_system,
            season_scorer_system,
            nearby_threat_scorer_system,
            tool_possession_scorer_system,
            lingtian_farming_scorer_system,
        )
            .in_set(BigBrainSet::Scorers),
    )
    .add_systems(
        PreUpdate,
        (
            till_action_system,
            plant_action_system,
            harvest_action_system,
            replenish_action_system,
            migrate_action_system,
        )
            .in_set(BigBrainSet::Actions),
    );
}

fn soil_suitability_scorer_system(
    cultivators: Query<&ScatteredCultivator, With<NpcMarker>>,
    plots: Query<&LingtianPlot>,
    mut scorers: Query<(&Actor, &mut Score), With<SoilSuitabilityScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = cultivators
            .get(*actor)
            .map(|cultivator| {
                soil_suitability_score(
                    cultivator.home_plot.is_none(),
                    plots.iter().any(|plot| plot.owner == Some(*actor)),
                )
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn qi_density_scorer_system(
    npcs: Query<(&NpcPatrol, &ScatteredCultivator), With<NpcMarker>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut scorers: Query<(&Actor, &mut Score), With<QiDensityScorer>>,
) {
    let zones = zone_registry.as_deref();
    for (Actor(actor), mut score) in &mut scorers {
        let value = npcs
            .get(*actor)
            .ok()
            .and_then(|(patrol, cultivator)| {
                zones
                    .and_then(|registry| registry.find_zone_by_name(&patrol.home_zone))
                    .map(|zone| qi_density_score(zone.spirit_qi as f32, cultivator.temperament))
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn own_qi_pool_scorer_system(
    cultivations: Query<&Cultivation, With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<OwnQiPoolScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = cultivations
            .get(*actor)
            .map(|cultivation| own_qi_pool_score(cultivation.qi_current, cultivation.qi_max))
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn season_scorer_system(mut scorers: Query<&mut Score, With<SeasonScorer>>) {
    for mut score in &mut scorers {
        score.set(0.5);
    }
}

fn nearby_threat_scorer_system(
    blackboards: Query<&NpcBlackboard, With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<NearbyThreatScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = blackboards
            .get(*actor)
            .map(|blackboard| nearby_threat_score(blackboard.player_distance))
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn tool_possession_scorer_system(mut scorers: Query<&mut Score, With<ToolPossessionScorer>>) {
    for mut score in &mut scorers {
        score.set(tool_possession_score(true));
    }
}

fn lingtian_farming_scorer_system(
    cultivators: Query<(&ScatteredCultivator, &NpcPatrol), With<NpcMarker>>,
    plots: Query<&LingtianPlot>,
    zone_registry: Option<Res<ZoneRegistry>>,
    sessions: Option<Res<ActiveLingtianSessions>>,
    mut scorers: Query<(&Actor, &LingtianFarmingScorer, &mut Score)>,
) {
    for (Actor(actor), scorer, mut score) in &mut scorers {
        let value = match cultivators.get(*actor) {
            Ok((cultivator, patrol)) if !actor_has_session(sessions.as_deref(), *actor) => {
                farming_intent_score(
                    scorer.intent,
                    *actor,
                    cultivator,
                    patrol,
                    &plots,
                    zone_registry.as_deref(),
                )
            }
            _ => 0.0,
        };
        score.set(value);
    }
}

fn actor_has_session(sessions: Option<&ActiveLingtianSessions>, actor: Entity) -> bool {
    sessions
        .map(|sessions| sessions.has_session(actor))
        .unwrap_or(false)
}

fn farming_intent_score(
    intent: LingtianFarmingIntent,
    actor: Entity,
    cultivator: &ScatteredCultivator,
    patrol: &NpcPatrol,
    plots: &Query<&LingtianPlot>,
    zone_registry: Option<&ZoneRegistry>,
) -> f32 {
    match intent {
        LingtianFarmingIntent::Till => {
            let owns_plot = plots.iter().any(|plot| plot.owner == Some(actor));
            if cultivator.home_plot.is_none() && !owns_plot {
                0.65
            } else {
                0.0
            }
        }
        LingtianFarmingIntent::Plant => cultivator
            .home_plot
            .and_then(|home| {
                plots
                    .iter()
                    .find(|plot| plot.pos == home)
                    .map(plot_needs_planting)
            })
            .unwrap_or(0.0),
        LingtianFarmingIntent::Harvest => cultivator
            .home_plot
            .and_then(|home| {
                plots
                    .iter()
                    .find(|plot| plot.pos == home)
                    .map(plot_needs_harvest)
            })
            .unwrap_or(0.0),
        LingtianFarmingIntent::Replenish => cultivator
            .home_plot
            .and_then(|home| {
                plots
                    .iter()
                    .find(|plot| plot.pos == home)
                    .map(plot_needs_replenish)
            })
            .unwrap_or(0.0),
        LingtianFarmingIntent::Migrate => {
            if cultivator.fail_streak >= 3 || home_zone_qi(patrol, zone_registry) < 0.2 {
                0.95
            } else {
                0.0
            }
        }
    }
}

fn plot_needs_planting(plot: &LingtianPlot) -> f32 {
    if plot.is_empty() && !plot.is_barren() {
        0.7
    } else {
        0.0
    }
}

fn plot_needs_harvest(plot: &LingtianPlot) -> f32 {
    if plot.crop.as_ref().is_some_and(|crop| crop.is_ripe()) {
        0.8
    } else {
        0.0
    }
}

fn plot_needs_replenish(plot: &LingtianPlot) -> f32 {
    if plot.plot_qi < 0.3 && !plot.is_barren() {
        0.75
    } else {
        0.0
    }
}

fn home_zone_qi(patrol: &NpcPatrol, zone_registry: Option<&ZoneRegistry>) -> f32 {
    zone_registry
        .and_then(|registry| registry.find_zone_by_name(&patrol.home_zone))
        .map(|zone| zone.spirit_qi as f32)
        .unwrap_or(1.0)
}

fn till_action_system(
    mut npcs: Query<(&Position, &mut ScatteredCultivator), With<NpcMarker>>,
    mut sessions: Option<ResMut<ActiveLingtianSessions>>,
    mut actions: Query<(&Actor, &mut ActionState), With<TillAction>>,
) {
    let Some(sessions) = sessions.as_deref_mut() else {
        for (Actor(actor), mut state) in &mut actions {
            if let Ok((_, mut cultivator)) = npcs.get_mut(*actor) {
                cultivator.record_farming_failure();
            }
            *state = ActionState::Failure;
        }
        return;
    };

    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                let Ok((position, mut cultivator)) = npcs.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };
                let pos = plot_pos_from_world(position.get());
                let inserted = sessions.try_insert(
                    *actor,
                    ActiveSession::Till(TillSession::new(
                        pos,
                        HoeKind::Iron,
                        0,
                        SessionMode::Auto,
                        PlotEnvironment::base(),
                    )),
                );
                if inserted {
                    cultivator.home_plot = Some(pos);
                    cultivator.record_farming_success();
                    *state = ActionState::Executing;
                } else {
                    cultivator.record_farming_failure();
                    *state = ActionState::Failure;
                }
            }
            ActionState::Executing => {
                if !sessions.has_session(*actor) {
                    if let Ok((_, mut cultivator)) = npcs.get_mut(*actor) {
                        cultivator.record_farming_success();
                    }
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                if let Ok((_, mut cultivator)) = npcs.get_mut(*actor) {
                    cultivator.record_farming_failure();
                }
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn plant_action_system(
    mut cultivators: Query<&mut ScatteredCultivator, With<NpcMarker>>,
    plant_registry: Option<Res<PlantKindRegistry>>,
    mut sessions: Option<ResMut<ActiveLingtianSessions>>,
    mut actions: Query<(&Actor, &mut ActionState), With<PlantAction>>,
) {
    let Some(sessions) = sessions.as_deref_mut() else {
        for (Actor(actor), mut state) in &mut actions {
            if let Ok(mut cultivator) = cultivators.get_mut(*actor) {
                cultivator.record_farming_failure();
            }
            *state = ActionState::Failure;
        }
        return;
    };
    let plant_id = plant_registry
        .as_deref()
        .and_then(|registry| registry.cultivable_ids().next().cloned());

    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                let Ok(mut cultivator) = cultivators.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };
                let Some(plant_id) = plant_id.clone() else {
                    cultivator.record_farming_failure();
                    *state = ActionState::Failure;
                    continue;
                };
                let Some(pos) = cultivator.home_plot else {
                    cultivator.record_farming_failure();
                    *state = ActionState::Failure;
                    continue;
                };
                if sessions.try_insert(
                    *actor,
                    ActiveSession::Planting(PlantingSession::new(pos, plant_id)),
                ) {
                    cultivator.record_farming_success();
                    *state = ActionState::Executing;
                } else {
                    cultivator.record_farming_failure();
                    *state = ActionState::Failure;
                }
            }
            ActionState::Executing => {
                if !sessions.has_session(*actor) {
                    if let Ok(mut cultivator) = cultivators.get_mut(*actor) {
                        cultivator.record_farming_success();
                    }
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                if let Ok(mut cultivator) = cultivators.get_mut(*actor) {
                    cultivator.record_farming_failure();
                }
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn harvest_action_system(
    mut cultivators: Query<&mut ScatteredCultivator, With<NpcMarker>>,
    plots: Query<&LingtianPlot>,
    mut sessions: Option<ResMut<ActiveLingtianSessions>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HarvestAction>>,
) {
    let Some(sessions) = sessions.as_deref_mut() else {
        for (Actor(actor), mut state) in &mut actions {
            if let Ok(mut cultivator) = cultivators.get_mut(*actor) {
                cultivator.record_farming_failure();
            }
            *state = ActionState::Failure;
        }
        return;
    };
    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                let Ok(mut cultivator) = cultivators.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };
                let Some(home) = cultivator.home_plot else {
                    cultivator.record_farming_failure();
                    *state = ActionState::Failure;
                    continue;
                };
                let Some((pos, plant_id)) =
                    plots.iter().find(|plot| plot.pos == home).and_then(|plot| {
                        plot.crop
                            .as_ref()
                            .filter(|crop| crop.is_ripe())
                            .map(|crop| (plot.pos, crop.kind.clone()))
                    })
                else {
                    cultivator.record_farming_failure();
                    *state = ActionState::Failure;
                    continue;
                };
                if sessions.try_insert(
                    *actor,
                    ActiveSession::Harvest(HarvestSession::new(pos, plant_id, SessionMode::Auto)),
                ) {
                    cultivator.record_farming_success();
                    *state = ActionState::Executing;
                } else {
                    cultivator.record_farming_failure();
                    *state = ActionState::Failure;
                }
            }
            ActionState::Executing => {
                if !sessions.has_session(*actor) {
                    if let Ok(mut cultivator) = cultivators.get_mut(*actor) {
                        cultivator.record_farming_success();
                    }
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                if let Ok(mut cultivator) = cultivators.get_mut(*actor) {
                    cultivator.record_farming_failure();
                }
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn replenish_action_system(
    mut cultivators: Query<&mut ScatteredCultivator, With<NpcMarker>>,
    tick: Option<Res<GameTick>>,
    mut sessions: Option<ResMut<ActiveLingtianSessions>>,
    mut actions: Query<(&Actor, &mut ActionState), With<ReplenishAction>>,
) {
    let Some(sessions) = sessions.as_deref_mut() else {
        for (Actor(actor), mut state) in &mut actions {
            if let Ok(mut cultivator) = cultivators.get_mut(*actor) {
                cultivator.record_farming_failure();
            }
            *state = ActionState::Failure;
        }
        return;
    };
    let now = tick.as_deref().map(|tick| u64::from(tick.0)).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                let Ok(mut cultivator) = cultivators.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };
                let Some(pos) = cultivator.home_plot else {
                    cultivator.record_farming_failure();
                    *state = ActionState::Failure;
                    continue;
                };
                if sessions.try_insert(
                    *actor,
                    ActiveSession::Replenish(ReplenishSession::new(pos, ReplenishSource::Zone)),
                ) {
                    cultivator.last_replenish_tick = now;
                    cultivator.record_farming_success();
                    *state = ActionState::Executing;
                } else {
                    cultivator.record_farming_failure();
                    *state = ActionState::Failure;
                }
            }
            ActionState::Executing => {
                if !sessions.has_session(*actor) {
                    if let Ok(mut cultivator) = cultivators.get_mut(*actor) {
                        cultivator.record_farming_success();
                    }
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                if let Ok(mut cultivator) = cultivators.get_mut(*actor) {
                    cultivator.record_farming_failure();
                }
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn migrate_action_system(
    mut npcs: Query<FarmingNpcQueryItem, FarmingNpcQueryFilter>,
    zone_registry: Option<Res<ZoneRegistry>>,
    tick: Option<Res<GameTick>>,
    mut actions: Query<(&Actor, &mut ActionState), With<MigrateAction>>,
) {
    let now = tick.as_deref().map(|tick| u64::from(tick.0)).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, mut patrol, mut navigator, mut cultivator)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };
        match *state {
            ActionState::Requested => {
                if !cultivator.migration_ready(now) {
                    *state = ActionState::Failure;
                    continue;
                }
                let Some(target) = best_migration_zone(&patrol.home_zone, zone_registry.as_deref())
                else {
                    cultivator.record_farming_failure();
                    *state = ActionState::Failure;
                    continue;
                };
                patrol.home_zone = target.0;
                patrol.current_target = target.1;
                navigator.set_goal(target.1, FARMING_ACTION_SPEED);
                cultivator.mark_migrated(now);
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if position.get().distance(patrol.current_target) <= MIGRATION_SUCCESS_DISTANCE {
                    navigator.stop();
                    *state = ActionState::Success;
                } else {
                    navigator.set_goal(patrol.current_target, FARMING_ACTION_SPEED);
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                cultivator.record_farming_failure();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn plot_pos_from_world(pos: DVec3) -> BlockPos {
    BlockPos::new(
        pos.x.floor() as i32,
        pos.y.floor() as i32 - 1,
        pos.z.floor() as i32,
    )
}

fn best_migration_zone(
    current_zone: &str,
    zone_registry: Option<&ZoneRegistry>,
) -> Option<(String, DVec3)> {
    let registry = zone_registry?;
    registry
        .zones
        .iter()
        .filter(|zone| zone.name != current_zone && zone.spirit_qi > 0.0)
        .max_by(|left, right| left.spirit_qi.total_cmp(&right.spirit_qi))
        .map(|zone| (zone.name.clone(), zone.center()))
}

pub fn soil_suitability_score(needs_home_plot: bool, owns_existing_plot: bool) -> f32 {
    if needs_home_plot && !owns_existing_plot {
        0.8
    } else {
        0.0
    }
}

pub fn qi_density_score(spirit_qi: f32, temperament: FarmingTemperament) -> f32 {
    (spirit_qi.clamp(0.0, 1.0) * temperament.weights().qi_density).clamp(0.0, 1.0)
}

pub fn own_qi_pool_score(qi_current: f64, qi_max: f64) -> f32 {
    if qi_max <= f64::EPSILON {
        return 0.0;
    }
    (qi_current / qi_max).clamp(0.0, 1.0) as f32
}

pub fn nearby_threat_score(player_distance: f32) -> f32 {
    if player_distance <= 5.0 {
        1.0
    } else if player_distance <= 16.0 {
        0.5
    } else {
        0.0
    }
}

pub fn tool_possession_score(has_tool: bool) -> f32 {
    if has_tool {
        0.6
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::Update;

    #[test]
    fn qi_density_respects_temperament_weight() {
        assert!(
            qi_density_score(0.6, FarmingTemperament::Greedy)
                > qi_density_score(0.6, FarmingTemperament::Patient)
        );
        assert_eq!(qi_density_score(2.0, FarmingTemperament::Greedy), 1.0);
    }

    #[test]
    fn plot_scores_are_mutually_specific() {
        let empty = LingtianPlot::new(BlockPos::new(1, 64, 1), Some(Entity::from_raw(7)));
        assert_eq!(plot_needs_planting(&empty), 0.7);
        assert_eq!(plot_needs_harvest(&empty), 0.0);

        let ripe = LingtianPlot {
            pos: BlockPos::new(1, 64, 1),
            owner: Some(Entity::from_raw(7)),
            crop: Some(crate::lingtian::plot::CropInstance {
                kind: "ci_she_hao".to_string(),
                growth: 1.0,
                quality_accum: 0.0,
            }),
            plot_qi: 0.5,
            plot_qi_cap: 1.0,
            harvest_count: 0,
            last_replenish_at: 0,
            dye_contamination: 0.0,
        };
        assert_eq!(plot_needs_planting(&ripe), 0.0);
        assert_eq!(plot_needs_harvest(&ripe), 0.8);
    }

    #[test]
    fn nearby_threat_has_three_bands() {
        assert_eq!(nearby_threat_score(4.0), 1.0);
        assert_eq!(nearby_threat_score(12.0), 0.5);
        assert_eq!(nearby_threat_score(40.0), 0.0);
    }

    #[test]
    fn plant_action_failures_increment_cultivator_fail_streak() {
        let mut app = App::new();
        app.insert_resource(ActiveLingtianSessions::new())
            .add_systems(Update, plant_action_system);

        let actor = app
            .world_mut()
            .spawn((
                NpcMarker,
                ScatteredCultivator::new(FarmingTemperament::Anxious),
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((Actor(actor), ActionState::Requested, PlantAction))
            .id();

        for expected in 1..=3 {
            app.world_mut()
                .entity_mut(action)
                .insert(ActionState::Requested);
            app.update();
            let cultivator = app.world().get::<ScatteredCultivator>(actor).unwrap();
            assert_eq!(cultivator.fail_streak, expected);
        }
    }
}
