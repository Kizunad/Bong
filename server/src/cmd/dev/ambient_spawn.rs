use valence::client::ClientMarker;
use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, ChunkLayer, Client, Commands, DVec3, EventReader, Position, Query, Res, Update, With,
};

use crate::npc::movement::GameTick;
use crate::npc::spawn::ambient_scheduler::{submit_ambient_dev_spawn_once, AmbientDevSpawnKind};
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::season::WorldSeasonState;
use crate::world::terrain::TerrainProviders;
use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmbientSpawnCmd {
    OnceMundane { x: f64, z: f64 },
    OnceThreat { x: f64, z: f64 },
}

impl Command for AmbientSpawnCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let once = graph.root().literal("ambient_spawn").literal("once").id();

        graph
            .at(once)
            .literal("mundane")
            .argument("x")
            .with_parser::<f64>()
            .argument("z")
            .with_parser::<f64>()
            .with_executable(|input| AmbientSpawnCmd::OnceMundane {
                x: f64::parse_arg(input).unwrap(),
                z: f64::parse_arg(input).unwrap(),
            });

        graph
            .at(once)
            .literal("threat")
            .argument("x")
            .with_parser::<f64>()
            .argument("z")
            .with_parser::<f64>()
            .with_executable(|input| AmbientSpawnCmd::OnceThreat {
                x: f64::parse_arg(input).unwrap(),
                z: f64::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<AmbientSpawnCmd>()
        .add_systems(Update, handle_ambient_spawn);
}

type PlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static Position>,
        Option<&'static CurrentDimension>,
        &'static mut Client,
    ),
    With<ClientMarker>,
>;

#[allow(clippy::too_many_arguments)]
pub fn handle_ambient_spawn(
    mut commands: Commands,
    mut events: EventReader<CommandResultEvent<AmbientSpawnCmd>>,
    mut players: PlayerQuery<'_, '_>,
    zones: Option<Res<ZoneRegistry>>,
    layers: Option<Res<DimensionLayers>>,
    chunk_layers: Query<&ChunkLayer>,
    terrain_providers: Option<Res<TerrainProviders>>,
    season: Option<Res<WorldSeasonState>>,
    tick: Option<Res<GameTick>>,
) {
    for event in events.read() {
        let Ok((position, current_dimension, mut client)) = players.get_mut(event.executor) else {
            // A command result may be injected for a non-player entity by a malformed dev test or
            // an internal caller. Do not let that entity reach the ambient production path.
            continue;
        };

        let (kind, x, z) = match event.result {
            AmbientSpawnCmd::OnceMundane { x, z } => (AmbientDevSpawnKind::Mundane, x, z),
            AmbientSpawnCmd::OnceThreat { x, z } => (AmbientDevSpawnKind::Threat, x, z),
        };
        if !x.is_finite() || !z.is_finite() {
            client.send_chat_message("[dev] ambient_spawn rejected: x/z must be finite");
            continue;
        }
        let Some(position) = position else {
            client.send_chat_message("[dev] ambient_spawn rejected: executor lacks Position");
            continue;
        };
        let Some(current_dimension) = current_dimension else {
            client
                .send_chat_message("[dev] ambient_spawn rejected: executor lacks CurrentDimension");
            continue;
        };
        if current_dimension.0 != DimensionKind::Overworld {
            client.send_chat_message("[dev] ambient_spawn rejected: executor must be in overworld");
            continue;
        }

        // Candidate Y has exactly one authority: the executor's server-side Position. The command
        // never accepts a Y argument, and the one-shot seam will replace this reference height with
        // the resolved feet Y before invoking the real pool.
        let candidate = DVec3::new(x, position.get().y, z);
        let Some(zone_registry) = zones.as_deref() else {
            client.send_chat_message("[dev] ambient_spawn rejected: zone registry unavailable");
            continue;
        };
        let Some(zone) = zone_registry.find_zone(DimensionKind::Overworld, candidate) else {
            client.send_chat_message("[dev] ambient_spawn rejected: no overworld zone at target");
            continue;
        };
        let Some(dimension_layers) = layers.as_deref() else {
            client.send_chat_message("[dev] ambient_spawn rejected: dimension layers unavailable");
            continue;
        };

        // The layer entity is required by the real pool. Its ChunkLayer component is deliberately
        // optional: if absent, submit_ambient_dev_spawn_once may still resolve through raster.
        let overworld_layer = dimension_layers.overworld;
        let runtime_layer = chunk_layers.get(overworld_layer).ok();
        let terrain = terrain_providers
            .as_deref()
            .map(|providers| &providers.overworld);
        let season = season
            .as_deref()
            .map(|state| state.current.season)
            .unwrap_or_default();
        let now = u64::from(tick.as_deref().map(|tick| tick.0).unwrap_or_default());

        let accepted = submit_ambient_dev_spawn_once(
            &mut commands,
            kind,
            overworld_layer,
            runtime_layer,
            terrain,
            zone,
            candidate,
            season,
            now,
        )
        .is_some();
        if accepted {
            client.send_chat_message(format!(
                "[dev] ambient_spawn accepted: kind={} x={x:.3} z={z:.3}",
                kind.as_str()
            ));
        } else {
            client.send_chat_message(format!(
                "[dev] ambient_spawn rejected: kind={} x={x:.3} z={z:.3} no safe surface or pool",
                kind.as_str()
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fauna::mundane::{MundaneFaunaKind, MundaneFaunaMarker, MundaneFaunaSpecies};
    use crate::npc::spawn::ambient_scheduler::AmbientThreatMarker;
    use crate::world::terrain::TerrainProvider;
    use crate::world::zone::Zone;
    use valence::command::manager::CommandPlugin;
    use valence::command::CommandRegistry;
    use valence::prelude::{BlockState, Chunk, Entity, Events, UnloadedChunk};
    use valence::protocol::packets::play::command_tree_s2c::{CommandTreeS2c, NodeData, Parser};
    use valence::protocol::packets::play::GameMessageS2c;
    use valence::testing::{create_mock_client, MockClientHelper, ScenarioSingleClient};

    const TARGET_X: f64 = 5.0;
    const TARGET_Z: f64 = 3.0;
    const EXECUTOR_Y: f64 = 152.0;
    const RUNTIME_GROUND_Y: i32 = 140;

    #[test]
    fn command_graph_has_one_shared_once_root_with_two_executable_kinds() {
        let mut app = App::new();
        app.add_plugins(CommandPlugin);
        register(&mut app);
        app.finish();
        app.cleanup();
        app.update();

        let registry = app.world().resource::<CommandRegistry>();
        let packet = CommandTreeS2c::from(registry.graph.clone());
        let node = |index: valence::protocol::VarInt| &packet.commands[index.0 as usize];
        let children = |index: valence::protocol::VarInt| node(index).children.clone();
        let root = packet.root_index;

        let ambient_children = children(root)
            .into_iter()
            .filter(|child| {
                matches!(
                    &node(*child).data,
                    NodeData::Literal { name } if name == "ambient_spawn"
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ambient_children.len(),
            1,
            "wire command tree must expose one ambient_spawn root, not duplicate same-name siblings"
        );
        let ambient = ambient_children[0];

        let once_children = children(ambient)
            .into_iter()
            .filter(
                |child| matches!(&node(*child).data, NodeData::Literal { name } if name == "once"),
            )
            .collect::<Vec<_>>();
        assert_eq!(
            once_children.len(),
            1,
            "wire ambient_spawn root must expose exactly one shared once node"
        );
        let once = once_children[0];

        let mut kinds = children(once)
            .into_iter()
            .map(|kind_node| match &node(kind_node).data {
                NodeData::Literal { name } => (name.clone(), kind_node),
                other => panic!("once child must be a kind literal, actual {other:?}"),
            })
            .collect::<Vec<_>>();
        kinds.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            kinds
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["mundane", "threat"],
            "both deterministic ambient kinds must be sibling literals under the same once node"
        );

        for (kind, kind_node) in kinds {
            let x_children = children(kind_node);
            assert_eq!(
                x_children.len(),
                1,
                "{kind} must have exactly one x argument child"
            );
            let x = x_children[0];
            assert!(
                matches!(
                    &node(x).data,
                    NodeData::Argument {
                        name,
                        parser: Parser::Double { min: None, max: None },
                        suggestion: None,
                    } if name == "x"
                ),
                "{kind}'s first argument must be an unconstrained x:double"
            );

            let z_children = children(x);
            assert_eq!(
                z_children.len(),
                1,
                "{kind}'s x node must have exactly one z argument child"
            );
            let z = z_children[0];
            assert!(
                matches!(
                    &node(z).data,
                    NodeData::Argument {
                        name,
                        parser: Parser::Double { min: None, max: None },
                        suggestion: None,
                    } if name == "z"
                ),
                "{kind}'s second argument must be an unconstrained z:double"
            );
            assert!(node(z).executable, "{kind}'s z leaf must be executable");
            assert!(
                node(z).children.is_empty(),
                "{kind}'s executable z node must remain the command leaf"
            );
        }
    }

    fn test_zone(spirit_qi: f64, danger_level: u8) -> Zone {
        Zone {
            name: "spawn".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                DVec3::new(-128.0, -64.0, -128.0),
                DVec3::new(128.0, 320.0, 128.0),
            ),
            spirit_qi,
            danger_level,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<AmbientSpawnCmd>>();
        app.add_systems(Update, handle_ambient_spawn);
        app
    }

    fn spawn_client(
        app: &mut App,
        position: [f64; 3],
        dimension: Option<DimensionKind>,
    ) -> (Entity, MockClientHelper) {
        let (mut bundle, helper) = create_mock_client("Alice");
        bundle.player.position = Position::new(position);
        let player = app.world_mut().spawn(bundle).id();
        if let Some(dimension) = dimension {
            app.world_mut()
                .entity_mut(player)
                .insert(CurrentDimension(dimension));
        }
        (player, helper)
    }

    fn send(app: &mut App, executor: Entity, result: AmbientSpawnCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<AmbientSpawnCmd>>>()
            .send(CommandResultEvent {
                result,
                executor,
                modifiers: Default::default(),
            });
    }

    fn flush_and_collect_chat(app: &mut App, helper: &mut MockClientHelper) -> Vec<String> {
        let world = app.world_mut();
        let mut clients = world.query::<&mut Client>();
        for mut client in clients.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                frame
                    .decode::<GameMessageS2c>()
                    .ok()
                    .map(|packet| packet.chat.to_legacy_lossy())
            })
            .collect()
    }

    fn assert_single_chat(app: &mut App, helper: &mut MockClientHelper, expected: &str) {
        let chats = flush_and_collect_chat(app, helper);
        assert_eq!(
            chats,
            vec![expected.to_string()],
            "ambient_spawn feedback protocol must stay exact"
        );
    }

    fn install_loaded_ground(app: &mut App, layer_entity: Entity) {
        let (min_y, height) = {
            let layer = app
                .world()
                .get::<ChunkLayer>(layer_entity)
                .expect("scenario must provide ChunkLayer");
            (layer.min_y(), layer.height())
        };
        let mut chunk = UnloadedChunk::with_height(height);
        chunk.set_block_state(
            TARGET_X as u32,
            (RUNTIME_GROUND_Y - min_y) as u32,
            TARGET_Z as u32,
            BlockState::STONE,
        );
        app.world_mut()
            .get_mut::<ChunkLayer>(layer_entity)
            .expect("scenario layer must be mutable")
            .insert_chunk([0, 0], chunk);
    }

    fn setup_runtime_app(
        spirit_qi: f64,
        danger_level: u8,
    ) -> (App, Entity, MockClientHelper, Entity) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.add_event::<CommandResultEvent<AmbientSpawnCmd>>();
        app.add_systems(Update, handle_ambient_spawn);
        app.world_mut().entity_mut(scenario.client).insert((
            Position::new([0.0, EXECUTOR_Y, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
        ));
        install_loaded_ground(&mut app, scenario.layer);
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers {
            overworld: scenario.layer,
            tsy,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![test_zone(spirit_qi, danger_level)],
        });
        (app, scenario.client, scenario.helper, scenario.layer)
    }

    #[test]
    fn non_player_executor_cannot_submit_ambient_entity() {
        let mut app = setup_app();
        let executor = app.world_mut().spawn_empty().id();
        send(
            &mut app,
            executor,
            AmbientSpawnCmd::OnceMundane {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();

        let mut markers = app.world_mut().query::<&MundaneFaunaMarker>();
        assert_eq!(
            markers.iter(app.world()).count(),
            0,
            "non-player executor must not reach the ambient production seam"
        );
    }

    #[test]
    fn every_non_finite_x_or_z_is_rejected_before_resources() {
        for (x, z) in [
            (f64::NAN, TARGET_Z),
            (f64::INFINITY, TARGET_Z),
            (f64::NEG_INFINITY, TARGET_Z),
            (TARGET_X, f64::NAN),
            (TARGET_X, f64::INFINITY),
            (TARGET_X, f64::NEG_INFINITY),
        ] {
            let mut app = setup_app();
            let (player, mut helper) = spawn_client(
                &mut app,
                [0.0, EXECUTOR_Y, 0.0],
                Some(DimensionKind::Overworld),
            );
            send(&mut app, player, AmbientSpawnCmd::OnceThreat { x, z });
            app.update();
            assert_single_chat(
                &mut app,
                &mut helper,
                "[dev] ambient_spawn rejected: x/z must be finite",
            );
        }
    }

    #[test]
    fn missing_position_and_dimension_are_explicitly_rejected() {
        let mut app = setup_app();
        let (without_position, mut position_helper) = spawn_client(
            &mut app,
            [0.0, EXECUTOR_Y, 0.0],
            Some(DimensionKind::Overworld),
        );
        app.world_mut()
            .entity_mut(without_position)
            .remove::<Position>();
        send(
            &mut app,
            without_position,
            AmbientSpawnCmd::OnceMundane {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut position_helper,
            "[dev] ambient_spawn rejected: executor lacks Position",
        );

        let mut app = setup_app();
        let (without_dimension, mut dimension_helper) =
            spawn_client(&mut app, [0.0, EXECUTOR_Y, 0.0], None);
        send(
            &mut app,
            without_dimension,
            AmbientSpawnCmd::OnceMundane {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut dimension_helper,
            "[dev] ambient_spawn rejected: executor lacks CurrentDimension",
        );
    }

    #[test]
    fn tsy_executor_is_rejected_before_world_resources() {
        let mut app = setup_app();
        let (player, mut helper) =
            spawn_client(&mut app, [0.0, EXECUTOR_Y, 0.0], Some(DimensionKind::Tsy));
        send(
            &mut app,
            player,
            AmbientSpawnCmd::OnceThreat {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut helper,
            "[dev] ambient_spawn rejected: executor must be in overworld",
        );
    }

    #[test]
    fn missing_zone_registry_target_zone_and_layers_have_stable_feedback() {
        let mut app = setup_app();
        let (player, mut helper) = spawn_client(
            &mut app,
            [0.0, EXECUTOR_Y, 0.0],
            Some(DimensionKind::Overworld),
        );
        send(
            &mut app,
            player,
            AmbientSpawnCmd::OnceMundane {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut helper,
            "[dev] ambient_spawn rejected: zone registry unavailable",
        );

        let mut app = setup_app();
        app.insert_resource(ZoneRegistry { zones: Vec::new() });
        let (player, mut helper) = spawn_client(
            &mut app,
            [0.0, EXECUTOR_Y, 0.0],
            Some(DimensionKind::Overworld),
        );
        send(
            &mut app,
            player,
            AmbientSpawnCmd::OnceMundane {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut helper,
            "[dev] ambient_spawn rejected: no overworld zone at target",
        );

        let mut app = setup_app();
        app.insert_resource(ZoneRegistry {
            zones: vec![test_zone(0.5, 1)],
        });
        let (player, mut helper) = spawn_client(
            &mut app,
            [0.0, EXECUTOR_Y, 0.0],
            Some(DimensionKind::Overworld),
        );
        send(
            &mut app,
            player,
            AmbientSpawnCmd::OnceMundane {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut helper,
            "[dev] ambient_spawn rejected: dimension layers unavailable",
        );
    }

    #[test]
    fn missing_runtime_layer_and_provider_rejects_without_spawning() {
        let mut app = setup_app();
        app.insert_resource(ZoneRegistry {
            zones: vec![test_zone(0.5, 1)],
        });
        app.insert_resource(DimensionLayers {
            overworld: Entity::PLACEHOLDER,
            tsy: Entity::PLACEHOLDER,
        });
        let (player, mut helper) = spawn_client(
            &mut app,
            [0.0, EXECUTOR_Y, 0.0],
            Some(DimensionKind::Overworld),
        );
        send(
            &mut app,
            player,
            AmbientSpawnCmd::OnceMundane {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut helper,
            "[dev] ambient_spawn rejected: kind=mundane x=5.000 z=3.000 no safe surface or pool",
        );
        let mut markers = app.world_mut().query::<&MundaneFaunaMarker>();
        assert_eq!(markers.iter(app.world()).count(), 0);
    }

    #[test]
    fn runtime_layer_without_provider_season_or_tick_spawns_mundane_at_resolved_feet() {
        let (mut app, player, mut helper, _) = setup_runtime_app(0.5, 1);
        assert!(
            app.world().get_resource::<WorldSeasonState>().is_none(),
            "test precondition: command must exercise missing season resource fallback"
        );
        assert!(
            app.world().get_resource::<GameTick>().is_none(),
            "test precondition: command must exercise missing tick resource fallback"
        );
        send(
            &mut app,
            player,
            AmbientSpawnCmd::OnceMundane {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut helper,
            "[dev] ambient_spawn accepted: kind=mundane x=5.000 z=3.000",
        );

        let world = app.world_mut();
        let mut spawned = world.query::<(&Position, &MundaneFaunaMarker, &MundaneFaunaSpecies)>();
        let rows = spawned
            .iter(world)
            .map(|(position, marker, species)| {
                (
                    position.get(),
                    marker.spawned_at,
                    marker.home_zone.as_str(),
                    species.0,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows.len(),
            1,
            "one-shot command must create exactly one mundane"
        );
        assert_eq!(rows[0].0, DVec3::new(TARGET_X, 141.0, TARGET_Z));
        assert_ne!(
            rows[0].0.y, EXECUTOR_Y,
            "entity must not inherit executor Y"
        );
        assert_eq!(
            rows[0].1, 0,
            "missing GameTick must fall back to spawned_at=0"
        );
        assert_eq!(rows[0].2, "spawn");
        assert_eq!(rows[0].3, MundaneFaunaKind::Cow);
    }

    #[test]
    fn runtime_layer_without_provider_season_or_tick_spawns_threat_rat_at_resolved_feet() {
        let (mut app, player, mut helper, _) = setup_runtime_app(0.5, 1);
        assert!(
            app.world().get_resource::<WorldSeasonState>().is_none(),
            "test precondition: command must exercise missing season resource fallback"
        );
        assert!(
            app.world().get_resource::<GameTick>().is_none(),
            "test precondition: command must exercise missing tick resource fallback"
        );
        send(
            &mut app,
            player,
            AmbientSpawnCmd::OnceThreat {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut helper,
            "[dev] ambient_spawn accepted: kind=threat x=5.000 z=3.000",
        );

        let world = app.world_mut();
        let mut spawned = world.query::<(&Position, &AmbientThreatMarker)>();
        let rows = spawned
            .iter(world)
            .map(|(position, marker)| {
                (position.get(), marker.spawned_at, marker.home_zone.as_str())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows.len(),
            1,
            "one-shot command must create exactly one threat"
        );
        assert_eq!(rows[0].0, DVec3::new(TARGET_X, 141.0, TARGET_Z));
        assert_ne!(
            rows[0].0.y, EXECUTOR_Y,
            "entity must not inherit executor Y"
        );
        assert_eq!(
            rows[0].1, 0,
            "missing GameTick must fall back to spawned_at=0"
        );
        assert_eq!(rows[0].2, "spawn");
    }

    #[test]
    fn provider_without_runtime_layer_is_a_valid_fallback() {
        let mut app = setup_app();
        app.insert_resource(ZoneRegistry {
            zones: vec![test_zone(0.5, 1)],
        });
        app.insert_resource(DimensionLayers {
            overworld: Entity::PLACEHOLDER,
            tsy: Entity::PLACEHOLDER,
        });
        app.insert_resource(TerrainProviders {
            overworld: TerrainProvider::empty_for_tests(),
            tsy: None,
        });
        let (player, mut helper) = spawn_client(
            &mut app,
            [0.0, EXECUTOR_Y, 0.0],
            Some(DimensionKind::Overworld),
        );
        send(
            &mut app,
            player,
            AmbientSpawnCmd::OnceThreat {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut helper,
            "[dev] ambient_spawn accepted: kind=threat x=5.000 z=3.000",
        );
        let mut markers = app.world_mut().query::<&AmbientThreatMarker>();
        assert_eq!(
            markers.iter(app.world()).count(),
            1,
            "missing runtime ChunkLayer must still succeed through passable raster provider"
        );
    }

    #[test]
    fn resolved_surface_followed_by_mundane_pool_rejection_has_no_entity() {
        let (mut app, player, mut helper, _) = setup_runtime_app(0.0, 1);
        send(
            &mut app,
            player,
            AmbientSpawnCmd::OnceMundane {
                x: TARGET_X,
                z: TARGET_Z,
            },
        );
        app.update();
        assert_single_chat(
            &mut app,
            &mut helper,
            "[dev] ambient_spawn rejected: kind=mundane x=5.000 z=3.000 no safe surface or pool",
        );
        let mut markers = app.world_mut().query::<&MundaneFaunaMarker>();
        assert_eq!(
            markers.iter(app.world()).count(),
            0,
            "dead-zone mundane pool rejection must not leave a marker/entity side effect"
        );
    }
}
