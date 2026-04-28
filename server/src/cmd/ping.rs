use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Update};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PingCmd {
    Ping,
}

impl Command for PingCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("ping")
            .with_executable(|_| PingCmd::Ping);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<PingCmd>()
        .add_systems(Update, handle_ping);
}

pub fn handle_ping(
    mut events: EventReader<CommandResultEvent<PingCmd>>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.executor) else {
            continue;
        };
        client.send_chat_message("pong");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::command::CommandRegistry;
    use valence::prelude::{Entity, Events};
    use valence::testing::create_mock_client;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins(valence::command::manager::CommandPlugin);
        register(&mut app);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    #[test]
    fn register_adds_ping_literal_to_command_registry() {
        let app = setup_app();
        let registry = app.world().resource::<CommandRegistry>();

        let literals = registry
            .graph
            .graph
            .node_weights()
            .filter_map(|node| match &node.data {
                valence::protocol::packets::play::command_tree_s2c::NodeData::Literal { name } => {
                    Some(name.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            literals.contains(&"ping"),
            "registered command literals should include /ping, got {literals:?}"
        );
    }

    #[test]
    fn handler_consumes_ping_event_without_panicking() {
        let mut app = setup_app();
        let (client_bundle, _helper) = create_mock_client("PingTester");
        let client = app.world_mut().spawn(client_bundle).id();

        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<PingCmd>>>()
            .send(CommandResultEvent {
                result: PingCmd::Ping,
                executor: client,
                modifiers: Default::default(),
            });

        app.update();

        let mut query = app.world_mut().query::<Entity>();
        assert!(
            query.iter(app.world()).any(|entity| entity == client),
            "ping handler should leave executor entity alive"
        );
    }
}
