use big_brain::prelude::{ActionBuilder, ActionState, Actor, BigBrainSet, Score, ScorerBuilder};
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EventWriter, IntoSystemConfigs, ParamSet,
    Position, PreUpdate, Query, With, Without,
};

use crate::combat::rat_bite::RatBiteEvent;
use crate::cultivation::components::Cultivation;
use crate::fauna::rat_phase::{
    chunk_pos_from_world, is_drained_chunk, remember_drained_chunk, MeditatingState, RatGroupId,
};
use crate::npc::navigator::Navigator;
use crate::npc::spawn::NpcMarker;
use crate::npc::spawn_rat::RatBlackboard;
use crate::world::dimension::{CurrentDimension, DimensionKind};

const QI_SOURCE_SCAN_RANGE: f64 = 32.0;
const QI_SOURCE_ARRIVAL_DISTANCE: f64 = 0.8;
const QI_SOURCE_SPEED_FACTOR: f64 = 1.0;
const REGROUP_SUCCESS_DISTANCE: f64 = 4.0;
const REGROUP_SPEED_FACTOR: f64 = 1.05;
const GROUP_COHESION_RADIUS: f64 = 16.0;
const MEDITATING_QI_SOURCE_WEIGHT: f32 = 3.0;

type RegroupReadQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, &'static RatGroupId), With<NpcMarker>>;
type QiSourceRatQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static RatBlackboard,
    ),
    With<NpcMarker>,
>;
type QiSourceTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static Cultivation,
        Option<&'static MeditatingState>,
    ),
    Without<NpcMarker>,
>;
type SeekRatQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static mut RatBlackboard,
        &'static mut Navigator,
    ),
    With<NpcMarker>,
>;
type RegroupNavigateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static RatGroupId,
        &'static mut Navigator,
    ),
    With<NpcMarker>,
>;

#[derive(Clone, Copy, Debug, Component)]
pub struct QiSourceProximityScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct GroupCohesionScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct DrainedChunkAvoidScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct SeekQiSourceAction;

#[derive(Clone, Copy, Debug, Component)]
pub struct RegroupAction;

impl ScorerBuilder for QiSourceProximityScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("QiSourceProximityScorer")
    }
}

impl ScorerBuilder for GroupCohesionScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("GroupCohesionScorer")
    }
}

impl ScorerBuilder for DrainedChunkAvoidScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DrainedChunkAvoidScorer")
    }
}

impl ActionBuilder for SeekQiSourceAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SeekQiSourceAction")
    }
}

impl ActionBuilder for RegroupAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("RegroupAction")
    }
}

pub fn register(app: &mut App) {
    app.add_systems(
        PreUpdate,
        (
            qi_source_proximity_scorer_system,
            group_cohesion_scorer_system,
            drained_chunk_avoid_scorer_system,
        )
            .in_set(BigBrainSet::Scorers),
    )
    .add_systems(
        PreUpdate,
        (seek_qi_source_action_system, regroup_action_system).in_set(BigBrainSet::Actions),
    );
}

fn qi_source_proximity_scorer_system(
    rats: QiSourceRatQuery<'_, '_>,
    targets: QiSourceTargetQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<QiSourceProximityScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let Ok((position, dimension, blackboard)) = rats.get(*actor) else {
            score.set(0.0);
            continue;
        };
        let current_chunk = chunk_pos_from_world(position.get());
        if is_drained_chunk(blackboard, current_chunk) {
            score.set(0.0);
            continue;
        }
        let value = nearest_qi_source_entity(position.get(), dimension_kind(dimension), &targets)
            .map(|source| qi_source_score(position.get(), source.position, source.weight))
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn group_cohesion_scorer_system(
    rats: Query<(Entity, &Position, &RatGroupId), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<GroupCohesionScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let Ok((_, position, group_id)) = rats.get(*actor) else {
            score.set(0.0);
            continue;
        };
        let Some(centroid) = group_centroid(group_id.0, actor, &rats) else {
            score.set(0.0);
            continue;
        };
        let distance = xz_distance(position.get(), centroid);
        score.set((distance / GROUP_COHESION_RADIUS).clamp(0.0, 1.0) as f32);
    }
}

fn drained_chunk_avoid_scorer_system(
    rats: Query<(&Position, &RatBlackboard), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<DrainedChunkAvoidScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = rats
            .get(*actor)
            .map(|(position, blackboard)| {
                if is_drained_chunk(blackboard, chunk_pos_from_world(position.get())) {
                    1.0
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn seek_qi_source_action_system(
    mut rats: SeekRatQuery<'_, '_>,
    targets: QiSourceTargetQuery<'_, '_>,
    mut bites: EventWriter<RatBiteEvent>,
    mut actions: Query<(&Actor, &mut ActionState), With<SeekQiSourceAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, dimension, mut blackboard, mut navigator)) = rats.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => *state = ActionState::Executing,
            ActionState::Executing => {
                let Some(source) =
                    nearest_qi_source_entity(position.get(), dimension_kind(dimension), &targets)
                else {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                };
                blackboard.last_pressure_target = Some(source.position);
                if position.get().distance(source.position) <= QI_SOURCE_ARRIVAL_DISTANCE {
                    bites.send(RatBiteEvent {
                        rat: *actor,
                        target: source.entity,
                        qi_steal: 1,
                    });
                    remember_drained_chunk(&mut blackboard, chunk_pos_from_world(position.get()));
                    navigator.stop();
                    *state = ActionState::Success;
                } else {
                    navigator.set_goal(source.position, QI_SOURCE_SPEED_FACTOR);
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn regroup_action_system(
    mut rats: ParamSet<(RegroupReadQuery<'_, '_>, RegroupNavigateQuery<'_, '_>)>,
    mut actions: Query<(&Actor, &mut ActionState), With<RegroupAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let group_id = {
            let group_query = rats.p0();
            let Ok((_, _, group_id)) = group_query.get(*actor) else {
                *state = ActionState::Failure;
                continue;
            };
            group_id.0
        };
        let centroid = {
            let group_query = rats.p0();
            let Some(centroid) = group_centroid(group_id, actor, &group_query) else {
                *state = ActionState::Success;
                continue;
            };
            centroid
        };
        let mut actor_query = rats.p1();
        let Ok((_, position, _, mut navigator)) = actor_query.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => *state = ActionState::Executing,
            ActionState::Executing => {
                if xz_distance(position.get(), centroid) <= REGROUP_SUCCESS_DISTANCE {
                    navigator.stop();
                    *state = ActionState::Success;
                } else {
                    navigator.set_goal(centroid, REGROUP_SPEED_FACTOR);
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct QiSource {
    entity: Entity,
    position: DVec3,
    weight: f32,
}

fn nearest_qi_source_entity(
    origin: DVec3,
    origin_dimension: DimensionKind,
    targets: &QiSourceTargetQuery<'_, '_>,
) -> Option<QiSource> {
    targets
        .iter()
        .filter(|(_, _, dimension, cultivation, _)| {
            cultivation.qi_current > 0.0 && dimension_kind(*dimension) == origin_dimension
        })
        .map(|(entity, position, _, _, meditating)| QiSource {
            entity,
            position: position.get(),
            weight: if meditating.is_some() {
                MEDITATING_QI_SOURCE_WEIGHT
            } else {
                1.0
            },
        })
        .filter(|source| origin.distance(source.position) <= QI_SOURCE_SCAN_RANGE)
        .max_by(|left, right| {
            qi_source_score(origin, left.position, left.weight).total_cmp(&qi_source_score(
                origin,
                right.position,
                right.weight,
            ))
        })
}

fn dimension_kind(dimension: Option<&CurrentDimension>) -> DimensionKind {
    dimension.map(|dim| dim.0).unwrap_or_default()
}

fn qi_source_score(origin: DVec3, source: DVec3, weight: f32) -> f32 {
    let distance_score = 1.0 - (origin.distance(source) / QI_SOURCE_SCAN_RANGE).clamp(0.0, 1.0);
    (distance_score as f32 * weight).clamp(0.0, 1.0)
}

fn group_centroid(
    group_id: u64,
    exclude: &Entity,
    rats: &Query<(Entity, &Position, &RatGroupId), With<NpcMarker>>,
) -> Option<DVec3> {
    group_centroid_from_iter(
        group_id,
        *exclude,
        rats.iter()
            .map(|(entity, position, group)| (entity, position.get(), *group)),
    )
}

fn group_centroid_from_iter<I>(group_id: u64, exclude: Entity, rats: I) -> Option<DVec3>
where
    I: IntoIterator<Item = (Entity, DVec3, RatGroupId)>,
{
    let mut sum = DVec3::ZERO;
    let mut count = 0.0;
    for (entity, position, group) in rats {
        if entity == exclude || group.0 != group_id {
            continue;
        }
        sum += position;
        count += 1.0;
    }
    (count > 0.0).then_some(sum / count)
}

fn xz_distance(a: DVec3, b: DVec3) -> f64 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events, Update};

    use crate::cultivation::components::{Cultivation, Realm};

    fn cultivation(qi_current: f64) -> Cultivation {
        Cultivation {
            realm: Realm::Induce,
            qi_current,
            qi_max: 10.0,
            ..Default::default()
        }
    }

    #[test]
    fn qi_source_proximity_scorer_ranks_nearest_meditator_first() {
        let origin = DVec3::ZERO;
        let near = DVec3::new(8.0, 0.0, 0.0);
        let far = DVec3::new(20.0, 0.0, 0.0);

        assert!(
            qi_source_score(origin, near, 1.0) > qi_source_score(origin, far, 1.0),
            "closer qi sources should score higher at equal weight"
        );
        assert!(
            qi_source_score(origin, far, MEDITATING_QI_SOURCE_WEIGHT)
                > qi_source_score(origin, near, 1.0),
            "meditating qi sources should carry the plan's 修炼苍蝇 weight"
        );
    }

    #[test]
    fn group_cohesion_pulls_lone_rat_back_to_centroid() {
        let group = RatGroupId(7);
        let lone = Entity::from_raw(1);
        let rats = [
            (lone, DVec3::new(32.0, 64.0, 0.0), group),
            (Entity::from_raw(2), DVec3::new(0.0, 64.0, 0.0), group),
            (Entity::from_raw(3), DVec3::new(0.0, 64.0, 16.0), group),
        ];

        let centroid = group_centroid_from_iter(group.0, lone, rats)
            .expect("other rats in group should define a centroid");

        assert_eq!(centroid, DVec3::new(0.0, 64.0, 8.0));
    }

    #[test]
    fn seek_qi_source_action_triggers_rat_bite_at_close_range() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_systems(Update, seek_qi_source_action_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                RatBlackboard {
                    home_chunk: crate::fauna::rat_phase::chunk_pos_from_world(DVec3::ZERO),
                    home_zone: "spawn".to_string(),
                    group_id: RatGroupId(7),
                    last_pressure_target: None,
                    recently_drained: Vec::new(),
                    drained_qi: 0.0,
                },
                Navigator::new(),
            ))
            .id();
        let target = app
            .world_mut()
            .spawn((
                Position::new([0.2, 64.0, 0.0]),
                cultivation(5.0),
                MeditatingState { since_tick: 1 },
            ))
            .id();
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, SeekQiSourceAction));

        app.update();

        let bites = app.world().resource::<Events<RatBiteEvent>>();
        let event = bites
            .iter_current_update_events()
            .next()
            .expect("close qi source should emit RatBiteEvent");
        assert_eq!(event.rat, rat);
        assert_eq!(event.target, target);
        assert_eq!(event.qi_steal, 1);
    }

    #[test]
    fn seek_qi_source_action_filters_targets_by_dimension() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_systems(Update, seek_qi_source_action_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
                RatBlackboard {
                    home_chunk: crate::fauna::rat_phase::chunk_pos_from_world(DVec3::ZERO),
                    home_zone: "spawn".to_string(),
                    group_id: RatGroupId(7),
                    last_pressure_target: None,
                    recently_drained: Vec::new(),
                    drained_qi: 0.0,
                },
                Navigator::new(),
            ))
            .id();
        let cross_dimension_target = app
            .world_mut()
            .spawn((
                Position::new([0.1, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Tsy),
                cultivation(5.0),
            ))
            .id();
        let same_dimension_target = app
            .world_mut()
            .spawn((
                Position::new([0.3, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
                cultivation(5.0),
            ))
            .id();
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, SeekQiSourceAction));

        app.update();

        let bites = app.world().resource::<Events<RatBiteEvent>>();
        let event = bites
            .iter_current_update_events()
            .next()
            .expect("same-dimension qi source in bite range should be selected");
        assert_eq!(event.rat, rat);
        assert_eq!(event.target, same_dimension_target);
        assert_ne!(
            event.target, cross_dimension_target,
            "rats must not bite qi targets from another dimension"
        );
    }
}
