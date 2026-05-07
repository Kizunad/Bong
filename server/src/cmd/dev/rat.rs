use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, Commands, DVec3, Entity, EntityLayerId, EventReader, EventWriter, ParamSet,
    Position, Query, Res, Update, With,
};

use crate::combat::CombatClock;
use crate::fauna::rat_phase::{
    chunk_pos_from_world, PressureSensor, RatGroupId, RatPhase, RatPhaseChangeEvent,
    RAT_PHASE_QI_GRADIENT_THRESHOLD, SURGE_TRIGGER_THRESHOLD,
};
use crate::npc::spawn::NpcMarker;
use crate::npc::spawn_rat::{spawn_rat_npc_at, RatBlackboard};
use crate::world::dimension::{CurrentDimension, DimensionKind, OverworldLayer};
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const RAT_SUMMON_OFFSET: DVec3 = DVec3::new(1.5, 0.0, 1.5);
const RAT_ACTIVATE_SEARCH_RADIUS: f64 = 48.0;

type PlayerCommandQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        Option<&'static EntityLayerId>,
        Option<&'static CurrentDimension>,
        &'static mut Client,
    ),
>;

type RatReadQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static RatGroupId,
        &'static RatBlackboard,
        &'static RatPhase,
    ),
    With<NpcMarker>,
>;

type RatWriteQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static RatGroupId,
        &'static mut RatPhase,
        &'static mut PressureSensor,
    ),
    With<NpcMarker>,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatTestCmd {
    SummonRat,
    Activate,
}

impl Command for RatTestCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("summon")
            .literal("rat")
            .with_executable(|_| Self::SummonRat);

        graph
            .root()
            .literal("rat")
            .literal("activate")
            .with_executable(|_| Self::Activate);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<RatTestCmd>()
        .add_systems(Update, handle_rat_test_commands);
}

#[allow(clippy::too_many_arguments)]
pub fn handle_rat_test_commands(
    mut commands: Commands,
    mut events: EventReader<CommandResultEvent<RatTestCmd>>,
    mut players: PlayerCommandQuery<'_, '_>,
    layers: Query<Entity, With<OverworldLayer>>,
    zones: Option<Res<ZoneRegistry>>,
    clock: Option<Res<CombatClock>>,
    mut rats: ParamSet<(RatReadQuery<'_, '_>, RatWriteQuery<'_, '_>)>,
    mut phase_events: EventWriter<RatPhaseChangeEvent>,
) {
    for event in events.read() {
        let Ok((position, player_layer, dimension, mut client)) = players.get_mut(event.executor)
        else {
            continue;
        };

        let player_pos = position.get();
        let dimension = dimension.map(|dim| dim.0).unwrap_or_default();
        match event.result {
            RatTestCmd::SummonRat => {
                let Some(layer) = player_layer
                    .map(|layer| layer.0)
                    .or_else(|| layers.iter().next())
                else {
                    client.send_chat_message("/summon rat failed: no active layer.");
                    continue;
                };
                let zone = resolve_zone_context(zones.as_deref(), dimension, player_pos);
                let rat = spawn_rat_npc_at(
                    &mut commands,
                    layer,
                    zone.name.as_str(),
                    player_pos + RAT_SUMMON_OFFSET,
                    zone.patrol_target,
                );
                client.send_chat_message(format!(
                    "/summon rat spawned {:?} in zone `{}`.",
                    rat, zone.name
                ));
            }
            RatTestCmd::Activate => {
                let Some(result) = activate_nearest_rat(
                    &mut rats,
                    player_pos,
                    clock.as_deref().map(|clock| clock.tick).unwrap_or_default(),
                    &mut phase_events,
                ) else {
                    client.send_chat_message("No rat within 48 blocks; run /summon rat first.");
                    continue;
                };
                client.send_chat_message(format!(
                    "/rat activate forced {:?} -> {:?} for {} rat(s) in `{}`.",
                    result.from, result.to, result.rat_count, result.zone
                ));
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ZoneContext {
    name: String,
    patrol_target: DVec3,
}

#[derive(Debug, Clone)]
struct RatActivationResult {
    zone: String,
    from: RatPhase,
    to: RatPhase,
    rat_count: u32,
}

#[derive(Debug, Clone)]
struct RatActivationTarget {
    group_id: RatGroupId,
    chunk: valence::prelude::ChunkPos,
    zone: String,
    from: RatPhase,
}

fn resolve_zone_context(
    zones: Option<&ZoneRegistry>,
    dimension: DimensionKind,
    position: DVec3,
) -> ZoneContext {
    if let Some(zone) = zones.and_then(|zones| zones.find_zone(dimension, position)) {
        return ZoneContext {
            name: zone.name.clone(),
            patrol_target: zone.patrol_target(0),
        };
    }

    ZoneContext {
        name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        patrol_target: position,
    }
}

fn activate_nearest_rat(
    rats: &mut ParamSet<(RatReadQuery<'_, '_>, RatWriteQuery<'_, '_>)>,
    player_pos: DVec3,
    tick: u64,
    phase_events: &mut EventWriter<RatPhaseChangeEvent>,
) -> Option<RatActivationResult> {
    let target = nearest_rat(rats, player_pos)?;
    let rat_count = count_group_chunk_rats(rats, target.group_id, target.chunk);
    let to = RatPhase::Transitioning { progress: 0 };
    let activated = force_group_chunk_phase(rats, target.group_id, target.chunk, to);
    if activated == 0 {
        return None;
    }

    phase_events.send(RatPhaseChangeEvent {
        chunk: [target.chunk.x, target.chunk.z],
        zone: target.zone.clone(),
        group_id: target.group_id.0,
        from: target.from,
        to,
        rat_count,
        local_qi: 1.0,
        qi_gradient: RAT_PHASE_QI_GRADIENT_THRESHOLD,
        tick,
    });

    Some(RatActivationResult {
        zone: target.zone,
        from: target.from,
        to,
        rat_count,
    })
}

fn nearest_rat(
    rats: &mut ParamSet<(RatReadQuery<'_, '_>, RatWriteQuery<'_, '_>)>,
    player_pos: DVec3,
) -> Option<RatActivationTarget> {
    let max_distance_sq = RAT_ACTIVATE_SEARCH_RADIUS * RAT_ACTIVATE_SEARCH_RADIUS;
    let mut best: Option<(RatActivationTarget, f64)> = None;
    for (_, position, group_id, blackboard, phase) in rats.p0().iter() {
        let distance_sq = position.get().distance_squared(player_pos);
        if distance_sq > max_distance_sq {
            continue;
        }
        if best
            .as_ref()
            .is_some_and(|(_, best_distance_sq)| *best_distance_sq <= distance_sq)
        {
            continue;
        }
        best = Some((
            RatActivationTarget {
                group_id: *group_id,
                chunk: chunk_pos_from_world(position.get()),
                zone: blackboard.home_zone.clone(),
                from: *phase,
            },
            distance_sq,
        ));
    }

    best.map(|(target, _)| target)
}

fn count_group_chunk_rats(
    rats: &mut ParamSet<(RatReadQuery<'_, '_>, RatWriteQuery<'_, '_>)>,
    group_id: RatGroupId,
    chunk: valence::prelude::ChunkPos,
) -> u32 {
    rats.p0()
        .iter()
        .filter(|(_, position, candidate_group, _, _)| {
            **candidate_group == group_id && chunk_pos_from_world(position.get()) == chunk
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

fn force_group_chunk_phase(
    rats: &mut ParamSet<(RatReadQuery<'_, '_>, RatWriteQuery<'_, '_>)>,
    group_id: RatGroupId,
    chunk: valence::prelude::ChunkPos,
    to: RatPhase,
) -> u32 {
    let mut activated = 0u32;
    for (position, candidate_group, mut phase, mut sensor) in rats.p1().iter_mut() {
        if *candidate_group != group_id || chunk_pos_from_world(position.get()) != chunk {
            continue;
        }

        *phase = to;
        sensor.local_density = 1.0;
        sensor.qi_pressure_grad = RAT_PHASE_QI_GRADIENT_THRESHOLD;
        sensor.surge_intensity = SURGE_TRIGGER_THRESHOLD;
        activated = activated.saturating_add(1);
    }
    activated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::{EntityKind, Events};

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<RatTestCmd>>();
        app.add_event::<RatPhaseChangeEvent>();
        app.add_systems(Update, handle_rat_test_commands);
        app
    }

    fn send(app: &mut App, executor: Entity, result: RatTestCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<RatTestCmd>>>()
            .send(CommandResultEvent {
                result,
                executor,
                modifiers: Default::default(),
            });
    }

    fn spawn_layer(app: &mut App) -> Entity {
        app.world_mut().spawn(OverworldLayer).id()
    }

    #[test]
    fn summon_rat_command_spawns_silverfish_rat_near_player() {
        let mut app = setup_app();
        let layer = spawn_layer(&mut app);
        let player = spawn_test_client(&mut app, "Alice", [4.0, 65.0, 4.0]);
        app.world_mut()
            .entity_mut(player)
            .insert(EntityLayerId(layer));

        send(&mut app, player, RatTestCmd::SummonRat);
        run_update(&mut app);

        let mut rats = app
            .world_mut()
            .query::<(&EntityKind, &RatBlackboard, &RatPhase)>();
        let spawned = rats
            .iter(app.world())
            .collect::<Vec<(&EntityKind, &RatBlackboard, &RatPhase)>>();
        assert_eq!(spawned.len(), 1);
        assert_eq!(spawned[0].0, &EntityKind::SILVERFISH);
        assert_eq!(spawned[0].1.home_zone, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(spawned[0].2, &RatPhase::Solitary);
    }

    #[test]
    fn rat_activate_command_forces_phase_event_for_nearest_rat_group() {
        let mut app = setup_app();
        let layer = spawn_layer(&mut app);
        let player = spawn_test_client(&mut app, "Alice", [4.0, 65.0, 4.0]);
        app.world_mut()
            .entity_mut(player)
            .insert(EntityLayerId(layer));
        send(&mut app, player, RatTestCmd::SummonRat);
        run_update(&mut app);

        send(&mut app, player, RatTestCmd::Activate);
        run_update(&mut app);

        let mut phases = app.world_mut().query::<(&RatPhase, &PressureSensor)>();
        let phase_snapshot = phases
            .iter(app.world())
            .collect::<Vec<(&RatPhase, &PressureSensor)>>();
        assert_eq!(phase_snapshot.len(), 1);
        assert_eq!(
            phase_snapshot[0].0,
            &RatPhase::Transitioning { progress: 0 }
        );
        assert_eq!(phase_snapshot[0].1.surge_intensity, SURGE_TRIGGER_THRESHOLD);

        let phase_events = app.world().resource::<Events<RatPhaseChangeEvent>>();
        let event = phase_events
            .iter_current_update_events()
            .next()
            .expect("/rat activate should emit a phase event");
        assert_eq!(event.zone, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(event.from, RatPhase::Solitary);
        assert_eq!(event.to, RatPhase::Transitioning { progress: 0 });
        assert_eq!(event.rat_count, 1);
    }
}
