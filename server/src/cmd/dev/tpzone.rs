use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Position, Query, Res, Update};

use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TpzoneCmd {
    Teleport { zone: String },
}

impl Command for TpzoneCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("tpzone")
            .argument("zone")
            .with_parser::<String>()
            .with_executable(|input| TpzoneCmd::Teleport {
                zone: String::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<TpzoneCmd>()
        .add_systems(Update, handle_tpzone);
}

pub fn handle_tpzone(
    mut events: EventReader<CommandResultEvent<TpzoneCmd>>,
    zone_registry: Option<Res<ZoneRegistry>>,
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
        let TpzoneCmd::Teleport { zone } = &event.result;
        let Ok((mut position, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        let Some(zone_def) = zones.find_zone_by_name(zone.as_str()) else {
            client.send_chat_message("Unknown zone.");
            continue;
        };
        let center = zone_def.center();
        position.set([center.x, center.y + 24.0, center.z]);
        client.send_chat_message(format!("Teleported to zone `{zone}`."));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::{Events, Position};

    #[test]
    fn zone_argument_parser_accepts_single_word_zone() {
        assert_eq!(
            String::arg_from_str("qingyun_peaks").unwrap(),
            "qingyun_peaks"
        );
    }

    #[test]
    fn zone_argument_parser_stops_at_whitespace() {
        assert_eq!(String::arg_from_str("spawn extra").unwrap(), "spawn");
    }

    #[test]
    fn tpzone_spawn_uses_fallback_registry() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<TpzoneCmd>>();
        app.add_systems(Update, handle_tpzone);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TpzoneCmd>>>()
            .send(CommandResultEvent {
                result: TpzoneCmd::Teleport {
                    zone: "spawn".to_string(),
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let position = app.world().get::<Position>(player).unwrap();
        assert_eq!(position.get().x, 128.0);
        assert_eq!(position.get().y, 96.0);
        assert_eq!(position.get().z, 128.0);
    }

    #[test]
    fn tpzone_unknown_zone_does_not_move_player() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<TpzoneCmd>>();
        app.add_systems(Update, handle_tpzone);
        let player = spawn_test_client(&mut app, "Alice", [1.0, 2.0, 3.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TpzoneCmd>>>()
            .send(CommandResultEvent {
                result: TpzoneCmd::Teleport {
                    zone: "missing".to_string(),
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        assert_eq!(
            app.world()
                .get::<Position>(player)
                .unwrap()
                .get()
                .to_array(),
            [1.0, 2.0, 3.0]
        );
    }
}
