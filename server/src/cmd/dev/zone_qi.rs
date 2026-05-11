use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, ResMut, Update};

use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, PartialEq)]
pub enum ZoneQiCmd {
    Set { name: String, value: f64 },
}

impl Command for ZoneQiCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("zone_qi")
            .literal("set")
            .argument("name")
            .with_parser::<String>()
            .argument("value")
            .with_parser::<f64>()
            .with_executable(|input| ZoneQiCmd::Set {
                name: String::parse_arg(input).unwrap(),
                value: f64::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<ZoneQiCmd>()
        .add_systems(Update, handle_zone_qi);
}

pub fn handle_zone_qi(
    mut events: EventReader<CommandResultEvent<ZoneQiCmd>>,
    zones: Option<ResMut<ZoneRegistry>>,
    mut clients: Query<&mut Client>,
) {
    let Some(mut zones) = zones else {
        for event in events.read() {
            if let Ok(mut client) = clients.get_mut(event.executor) {
                client.send_chat_message("[dev] zone_qi failed: ZoneRegistry missing");
            }
        }
        return;
    };

    for event in events.read() {
        let ZoneQiCmd::Set { name, value } = &event.result;
        let Ok(mut client) = clients.get_mut(event.executor) else {
            continue;
        };
        if !value.is_finite() {
            client.send_chat_message("[dev] zone_qi rejected: value must be finite");
            continue;
        }
        if let Some(zone) = zones.find_zone_mut(name) {
            let before = zone.spirit_qi;
            zone.spirit_qi = *value;
            tracing::warn!(
                "[dev-cmd] bypass ledger and zone qi tick: zone `{}` {:.3} -> {:.3}",
                name,
                before,
                value
            );
            client.send_chat_message(format!(
                "[dev] zone_qi `{name}` {:.2} -> {:.2}",
                before, value
            ));
        } else {
            let hints = zones
                .zones
                .iter()
                .take(10)
                .map(|zone| zone.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            client.send_chat_message(format!("[dev] unknown zone `{name}`; known: {hints}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::qi_physics::QiTransfer;
    use valence::prelude::Events;

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<CommandResultEvent<ZoneQiCmd>>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, handle_zone_qi);
        app
    }

    fn send(app: &mut App, player: valence::prelude::Entity, name: &str, value: f64) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ZoneQiCmd>>>()
            .send(CommandResultEvent {
                result: ZoneQiCmd::Set {
                    name: name.to_string(),
                    value,
                },
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn zone_qi_set_updates_spawn_and_allows_negative_values() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, "spawn", 0.8);
        run_update(&mut app);
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi,
            0.8
        );

        send(&mut app, player, "spawn", -0.3);
        run_update(&mut app);
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi,
            -0.3
        );
    }

    #[test]
    fn zone_qi_unknown_zone_does_not_mutate_existing_zones() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, "missing", 0.5);
        run_update(&mut app);

        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi,
            0.9
        );
    }

    #[test]
    fn zone_qi_set_does_not_emit_qi_transfer() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, "spawn", 1.0);
        run_update(&mut app);

        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);
    }
}
