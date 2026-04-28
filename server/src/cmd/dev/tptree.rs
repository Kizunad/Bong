use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Position, Query, Res, Update};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::world::terrain::{TerrainProvider, TerrainProviders};
use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeTarget {
    Spirit,
    Dead,
}

impl TreeTarget {
    pub fn zone_name(self) -> &'static str {
        match self {
            Self::Spirit => "spawn",
            Self::Dead => "north_wastes",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Spirit => "spirit",
            Self::Dead => "dead",
        }
    }
}

impl CommandArg for TreeTarget {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        match raw.as_str() {
            "spirit" => Ok(Self::Spirit),
            "dead" => Ok(Self::Dead),
            _ => Err(CommandArgParseError::InvalidArgument {
                expected: "spirit|dead".to_string(),
                got: raw,
            }),
        }
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TptreeCmd {
    Teleport { tree: TreeTarget },
}

impl Command for TptreeCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("tptree")
            .argument("tree")
            .with_parser::<TreeTarget>()
            .with_executable(|input| TptreeCmd::Teleport {
                tree: TreeTarget::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<TptreeCmd>()
        .add_systems(Update, handle_tptree);
}

pub fn target_y_for_tree(
    center: valence::prelude::DVec3,
    terrain: Option<&TerrainProvider>,
) -> f64 {
    if let Some(terrain) = terrain {
        let sample = terrain.sample(center.x.floor() as i32, center.z.floor() as i32);
        sample.height.round() as f64 + 40.0
    } else {
        center.y + 60.0
    }
}

pub fn handle_tptree(
    mut events: EventReader<CommandResultEvent<TptreeCmd>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    providers: Option<Res<TerrainProviders>>,
    mut players: Query<(&mut Position, &mut Client)>,
) {
    let fallback_registry;
    let zones = if let Some(registry) = zone_registry.as_deref() {
        registry
    } else {
        fallback_registry = ZoneRegistry::fallback();
        &fallback_registry
    };

    for event in events.read() {
        let TptreeCmd::Teleport { tree } = event.result;
        let Ok((mut position, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        let Some(zone) = zones.find_zone_by_name(tree.zone_name()) else {
            client.send_chat_message("Zone not found.");
            continue;
        };
        let center = zone.center();
        let target_y = target_y_for_tree(center, providers.as_deref().map(|p| &p.overworld));
        position.set([center.x, target_y, center.z]);
        client.send_chat_message(format!(
            "Teleported above {} tree zone (`{}`).",
            tree.label(),
            tree.zone_name()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::{DVec3, Events, Position};

    #[test]
    fn tree_target_parses_known_targets() {
        assert_eq!(
            TreeTarget::arg_from_str("spirit").unwrap(),
            TreeTarget::Spirit
        );
        assert_eq!(TreeTarget::arg_from_str("dead").unwrap(), TreeTarget::Dead);
    }

    #[test]
    fn tree_target_rejects_unknown_target() {
        assert!(TreeTarget::arg_from_str("ash").is_err());
    }

    #[test]
    fn target_y_without_terrain_uses_zone_center_plus_sixty() {
        assert_eq!(target_y_for_tree(DVec3::new(1.0, 70.0, 1.0), None), 130.0);
    }

    #[test]
    fn tptree_spirit_uses_spawn_fallback_zone() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<TptreeCmd>>();
        app.add_systems(Update, handle_tptree);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TptreeCmd>>>()
            .send(CommandResultEvent {
                result: TptreeCmd::Teleport {
                    tree: TreeTarget::Spirit,
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let position = app.world().get::<Position>(player).unwrap();
        assert_eq!(position.get().x, 128.0);
        assert_eq!(position.get().y, 132.0);
        assert_eq!(position.get().z, 128.0);
    }
}
