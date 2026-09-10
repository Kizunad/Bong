use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, Commands, EventReader, Query, Update};

use crate::network::AmbientServerDataIsolation;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmbientIsolationCmd {
    Enable,
}

impl Command for AmbientIsolationCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("ping")
            .literal("ambient_isolation")
            .with_executable(|_| AmbientIsolationCmd::Enable);
    }
}

pub fn register(app: &mut App) {
    register_with_ambient_isolation(app, ambient_isolation_enabled());
}

fn register_with_ambient_isolation(app: &mut App, enabled: bool) {
    app.add_command::<PingCmd>()
        .add_systems(Update, handle_ping);
    if enabled {
        app.add_command::<AmbientIsolationCmd>()
            .add_systems(Update, handle_ambient_isolation);
    }
}

fn ambient_isolation_enabled() -> bool {
    std::env::var("BONG_E2E_AMBIENT_ISOLATION")
        .ok()
        .is_some_and(|value| value.trim() == "1")
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

fn handle_ambient_isolation(
    mut events: EventReader<CommandResultEvent<AmbientIsolationCmd>>,
    mut commands: Commands,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.executor) else {
            continue;
        };
        commands
            .entity(event.executor)
            .insert(AmbientServerDataIsolation);
        client.send_chat_message("[dev] ambient server_data isolation enabled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::command::CommandRegistry;
    use valence::prelude::{Entity, Events};
    use valence::testing::create_mock_client;

    fn setup_app() -> App {
        setup_app_with_ambient_isolation(false)
    }

    fn setup_app_with_ambient_isolation(enabled: bool) -> App {
        let mut app = App::new();
        app.add_plugins(valence::command::manager::CommandPlugin);
        register_with_ambient_isolation(&mut app, enabled);
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

    #[test]
    fn ambient_isolation_command_is_opt_in_and_marks_only_executor() {
        let disabled = setup_app_with_ambient_isolation(false);
        let disabled_literals = disabled
            .world()
            .resource::<CommandRegistry>()
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
            !disabled_literals.contains(&"ambient_isolation"),
            "ambient isolation capability must be absent when the server opt-in is disabled"
        );

        let mut enabled = setup_app_with_ambient_isolation(true);
        let (client_bundle, _helper) = create_mock_client("AmbientTester");
        let client = enabled.world_mut().spawn(client_bundle).id();
        enabled
            .world_mut()
            .resource_mut::<Events<CommandResultEvent<AmbientIsolationCmd>>>()
            .send(CommandResultEvent {
                result: AmbientIsolationCmd::Enable,
                executor: client,
                modifiers: Default::default(),
            });

        enabled.update();

        assert!(
            enabled
                .world()
                .get::<AmbientServerDataIsolation>(client)
                .is_some(),
            "ambient isolation command must mark its executor after the explicit success path"
        );
    }
}
