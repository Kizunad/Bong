use valence::client::ClientMarker;
use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, Commands, EventReader, Position, Query, Res, Resource, Update, With,
};

use crate::botany::components::{Plant, PlantLifecycleClock};
use crate::botany::registry::{BotanyPlantId, PlantVariant};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

const FIXTURE_OFFSET_X: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotanySpawnCmd {
    SpiritGrass,
}

impl Command for BotanySpawnCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("botany_spawn")
            .literal("spirit_grass")
            .with_executable(|_| BotanySpawnCmd::SpiritGrass);
    }
}

struct BotanySpawnDevAccess;

impl Resource for BotanySpawnDevAccess {}

pub(super) fn register_enabled(app: &mut App) {
    app.insert_resource(BotanySpawnDevAccess)
        .add_command::<BotanySpawnCmd>()
        .add_systems(Update, handle_botany_spawn);
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

fn handle_botany_spawn(
    mut commands: Commands,
    mut events: EventReader<CommandResultEvent<BotanySpawnCmd>>,
    mut players: PlayerQuery<'_, '_>,
    zones: Option<Res<ZoneRegistry>>,
    clock: Option<Res<PlantLifecycleClock>>,
    dev_access: Option<Res<BotanySpawnDevAccess>>,
) {
    let dev_access_enabled = dev_access.is_some();
    for event in events.read() {
        let Ok((position, current_dimension, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        if !dev_access_enabled {
            client.send_chat_message("[dev] botany_spawn rejected: dev mode disabled");
            continue;
        }
        let Some(position) = position else {
            client.send_chat_message("[dev] botany_spawn rejected: executor lacks Position");
            continue;
        };
        let Some(current_dimension) = current_dimension else {
            client
                .send_chat_message("[dev] botany_spawn rejected: executor lacks CurrentDimension");
            continue;
        };
        if current_dimension.0 != DimensionKind::Overworld {
            client.send_chat_message("[dev] botany_spawn rejected: executor must be in overworld");
            continue;
        }

        let player_pos = position.get();
        if !player_pos.is_finite() {
            client
                .send_chat_message("[dev] botany_spawn rejected: executor position must be finite");
            continue;
        }
        let target = [player_pos.x + FIXTURE_OFFSET_X, player_pos.y, player_pos.z];
        let Some(zones) = zones.as_deref() else {
            client.send_chat_message("[dev] botany_spawn rejected: zone registry unavailable");
            continue;
        };
        let Some(player_zone) = zones.find_zone(
            DimensionKind::Overworld,
            valence::prelude::DVec3::new(player_pos.x, player_pos.y, player_pos.z),
        ) else {
            client.send_chat_message("[dev] botany_spawn rejected: no overworld zone at player");
            continue;
        };
        let Some(target_zone) = zones.find_zone(
            DimensionKind::Overworld,
            valence::prelude::DVec3::new(target[0], target[1], target[2]),
        ) else {
            client.send_chat_message("[dev] botany_spawn rejected: no overworld zone at target");
            continue;
        };
        if player_zone.name != target_zone.name {
            client.send_chat_message("[dev] botany_spawn rejected: target crosses zone boundary");
            continue;
        }
        let Some(clock) = clock.as_deref() else {
            client.send_chat_message("[dev] botany_spawn rejected: lifecycle clock unavailable");
            continue;
        };

        // Dev fixture only: create the real ECS Plant shape consumed by the production nearest-target
        // resolver. This intentionally bypasses natural lifecycle density and qi accounting.
        let entity = commands
            .spawn(Plant {
                id: BotanyPlantId::SpiritGrass,
                zone_name: target_zone.name.clone(),
                position: target,
                planted_at_tick: clock.tick,
                wither_progress: 0,
                source_point: None,
                harvested: false,
                trampled: false,
                variant: PlantVariant::None,
            })
            .id();
        client.send_chat_message(format!(
            "[dev] botany_spawn accepted: plant_id=plant-{} kind=spirit_grass pos=[{:.17},{:.17},{:.17}] zone={}",
            entity.to_bits(),
            target[0],
            target[1],
            target[2],
            target_zone.name
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::command::manager::CommandPlugin;
    use valence::command::CommandRegistry;
    use valence::prelude::{Entity, Events};
    use valence::protocol::packets::play::command_tree_s2c::{CommandTreeS2c, NodeData};
    use valence::protocol::packets::play::GameMessageS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn setup_app(dev_access_enabled: bool, include_zones: bool, include_clock: bool) -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<BotanySpawnCmd>>();
        if dev_access_enabled {
            app.insert_resource(BotanySpawnDevAccess);
        }
        if include_zones {
            app.insert_resource(ZoneRegistry::fallback());
        }
        if include_clock {
            app.insert_resource(PlantLifecycleClock { tick: 73 });
        }
        app.add_systems(Update, handle_botany_spawn);
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

    fn send(app: &mut App, executor: Entity) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<BotanySpawnCmd>>>()
            .send(CommandResultEvent {
                result: BotanySpawnCmd::SpiritGrass,
                executor,
                modifiers: Default::default(),
            });
    }

    fn collect_chat(app: &mut App, helper: &mut MockClientHelper) -> Vec<String> {
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

    #[test]
    fn command_graph_exposes_only_fixed_spirit_grass_fixture_leaf() {
        let mut app = App::new();
        app.add_plugins(CommandPlugin);
        register_enabled(&mut app);
        app.finish();
        app.cleanup();
        app.update();

        let registry = app.world().resource::<CommandRegistry>();
        let packet = CommandTreeS2c::from(registry.graph.clone());
        let node = |index: valence::protocol::VarInt| &packet.commands[index.0 as usize];
        let root = node(packet.root_index);
        let botany_spawn = root
            .children
            .iter()
            .copied()
            .find(|child| {
                matches!(&node(*child).data, NodeData::Literal { name } if name == "botany_spawn")
            })
            .expect("dev command tree must contain botany_spawn root");
        let children = &node(botany_spawn).children;
        assert_eq!(children.len(), 1);
        let spirit_grass = children[0];
        assert!(matches!(
            &node(spirit_grass).data,
            NodeData::Literal { name } if name == "spirit_grass"
        ));
        assert!(node(spirit_grass).executable);
        assert!(node(spirit_grass).children.is_empty());
    }

    #[test]
    fn accepted_command_spawns_exact_real_plant_and_reports_identity() {
        let mut app = setup_app(true, true, true);
        let (player, mut helper) =
            spawn_client(&mut app, [10.0, 64.0, -4.0], Some(DimensionKind::Overworld));
        send(&mut app, player);

        app.update();

        let world = app.world_mut();
        let mut plants = world.query::<(Entity, &Plant)>();
        let plants = plants.iter(world).collect::<Vec<_>>();
        let [(entity, plant)] = plants.as_slice() else {
            panic!("accepted fixture command must spawn exactly one Plant");
        };
        assert_eq!(plant.id, BotanyPlantId::SpiritGrass);
        assert_eq!(plant.zone_name, "spawn");
        assert_eq!(plant.position, [11.0, 64.0, -4.0]);
        assert_eq!(plant.planted_at_tick, 73);
        assert_eq!(plant.wither_progress, 0);
        assert_eq!(plant.source_point, None);
        assert!(!plant.harvested);
        assert!(!plant.trampled);
        assert_eq!(plant.variant, PlantVariant::None);
        let expected = format!(
            "[dev] botany_spawn accepted: plant_id=plant-{} kind=spirit_grass pos=[11.00000000000000000,64.00000000000000000,-4.00000000000000000] zone=spawn",
            entity.to_bits()
        );
        assert_eq!(collect_chat(&mut app, &mut helper), vec![expected]);
    }

    #[test]
    fn non_player_executor_cannot_spawn_fixture() {
        let mut app = setup_app(true, true, true);
        let executor = app.world_mut().spawn_empty().id();
        send(&mut app, executor);
        app.update();

        assert_eq!(
            app.world_mut().query::<&Plant>().iter(app.world()).count(),
            0
        );
    }

    #[test]
    fn disabled_dev_access_rejects_without_spawning() {
        let mut app = setup_app(false, true, true);
        let (player, mut helper) =
            spawn_client(&mut app, [0.0, 64.0, 0.0], Some(DimensionKind::Overworld));
        send(&mut app, player);
        app.update();

        assert_eq!(
            app.world_mut().query::<&Plant>().iter(app.world()).count(),
            0
        );
        assert_eq!(
            collect_chat(&mut app, &mut helper),
            vec!["[dev] botany_spawn rejected: dev mode disabled"]
        );
    }

    #[test]
    fn missing_dimension_wrong_dimension_and_non_finite_position_are_rejected() {
        for (position, dimension, expected) in [
            (
                [0.0, 64.0, 0.0],
                None,
                "[dev] botany_spawn rejected: executor lacks CurrentDimension",
            ),
            (
                [0.0, 64.0, 0.0],
                Some(DimensionKind::Tsy),
                "[dev] botany_spawn rejected: executor must be in overworld",
            ),
            (
                [f64::NAN, 64.0, 0.0],
                Some(DimensionKind::Overworld),
                "[dev] botany_spawn rejected: executor position must be finite",
            ),
        ] {
            let mut app = setup_app(true, true, true);
            let (player, mut helper) = spawn_client(&mut app, position, dimension);
            send(&mut app, player);
            app.update();

            assert_eq!(
                app.world_mut().query::<&Plant>().iter(app.world()).count(),
                0
            );
            assert_eq!(collect_chat(&mut app, &mut helper), vec![expected]);
        }
    }

    #[test]
    fn missing_zone_or_clock_rejects_without_spawning() {
        for (include_zones, include_clock, expected) in [
            (
                false,
                true,
                "[dev] botany_spawn rejected: zone registry unavailable",
            ),
            (
                true,
                false,
                "[dev] botany_spawn rejected: lifecycle clock unavailable",
            ),
        ] {
            let mut app = setup_app(true, include_zones, include_clock);
            let (player, mut helper) =
                spawn_client(&mut app, [0.0, 64.0, 0.0], Some(DimensionKind::Overworld));
            send(&mut app, player);
            app.update();

            assert_eq!(
                app.world_mut().query::<&Plant>().iter(app.world()).count(),
                0
            );
            assert_eq!(collect_chat(&mut app, &mut helper), vec![expected]);
        }
    }
}
