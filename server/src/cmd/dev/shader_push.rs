use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{ident, App, Client, EventReader, Query, ResMut, Update};

use crate::shader::ShaderStatePayload;

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderPushCmd {
    Set { name: String, value: f64 },
    Broadcast,
}

impl Command for ShaderPushCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let root = graph.root().literal("shader_push").id();

        graph
            .at(root)
            .literal("set")
            .argument("name")
            .with_parser::<String>()
            .argument("value")
            .with_parser::<f64>()
            .with_executable(|input| ShaderPushCmd::Set {
                name: String::parse_arg(input).unwrap(),
                value: f64::parse_arg(input).unwrap(),
            });

        graph
            .at(root)
            .literal("broadcast")
            .with_executable(|_| ShaderPushCmd::Broadcast);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<ShaderPushCmd>()
        .add_systems(Update, handle_shader_push);
}

pub fn handle_shader_push(
    mut events: EventReader<CommandResultEvent<ShaderPushCmd>>,
    mut state: ResMut<ShaderStatePayload>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        match &event.result {
            ShaderPushCmd::Set { name, value } => {
                let Ok(mut executor_client) = clients.get_mut(event.executor) else {
                    continue;
                };
                if !value.is_finite() {
                    executor_client
                        .send_chat_message("[dev] shader_push rejected: value must be finite");
                    continue;
                }
                let val = *value as f32;
                if let Some(field) = state.field_mut(name) {
                    let before = *field;
                    *field = val;
                    executor_client
                        .send_chat_message(format!("[dev] shader {name}: {before:.3} -> {val:.3}"));
                } else {
                    let hints = ShaderStatePayload::FIELD_NAMES.join(", ");
                    executor_client.send_chat_message(format!(
                        "[dev] unknown shader field `{name}`; known: {hints}"
                    ));
                }
            }
            ShaderPushCmd::Broadcast => {
                let bytes = state.to_json_bytes();
                let mut count = 0u32;
                for mut client in clients.iter_mut() {
                    client.send_custom_payload(ident!("bong:shader_state"), &bytes);
                    count += 1;
                }
                if let Ok(mut exec) = clients.get_mut(event.executor) {
                    exec.send_chat_message(format!(
                        "[dev] shader_push broadcast to {count} client(s)"
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(ShaderStatePayload::default());
        app.add_event::<CommandResultEvent<ShaderPushCmd>>();
        app.add_systems(Update, handle_shader_push);
        app
    }

    fn send_set(app: &mut App, player: valence::prelude::Entity, name: &str, value: f64) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ShaderPushCmd>>>()
            .send(CommandResultEvent {
                result: ShaderPushCmd::Set {
                    name: name.to_string(),
                    value,
                },
                executor: player,
                modifiers: Default::default(),
            });
    }

    fn send_broadcast(app: &mut App, player: valence::prelude::Entity) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ShaderPushCmd>>>()
            .send(CommandResultEvent {
                result: ShaderPushCmd::Broadcast,
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn set_known_field_updates_resource() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send_set(&mut app, player, "bong_bloodmoon", 0.9);
        run_update(&mut app);
        assert_eq!(
            app.world().resource::<ShaderStatePayload>().bong_bloodmoon,
            0.9
        );
    }

    #[test]
    fn set_unknown_field_does_not_panic() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send_set(&mut app, player, "bong_nonexistent", 0.5);
        run_update(&mut app);
        // no panic, resource unchanged
        let state = app.world().resource::<ShaderStatePayload>();
        assert_eq!(state.bong_bloodmoon, 0.0);
    }

    #[test]
    fn set_rejects_nan() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send_set(&mut app, player, "bong_realm", f64::NAN);
        run_update(&mut app);
        assert_eq!(app.world().resource::<ShaderStatePayload>().bong_realm, 0.0);
    }

    #[test]
    fn broadcast_does_not_panic_with_no_clients() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send_broadcast(&mut app, player);
        run_update(&mut app);
        // no panic is the assertion
    }

    #[test]
    fn set_multiple_fields() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send_set(&mut app, player, "bong_wind_strength", 0.7);
        run_update(&mut app);
        send_set(&mut app, player, "bong_wind_angle", 3.12);
        run_update(&mut app);
        let state = app.world().resource::<ShaderStatePayload>();
        assert_eq!(state.bong_wind_strength, 0.7);
        assert!((state.bong_wind_angle - 3.12).abs() < 0.001);
    }
}
