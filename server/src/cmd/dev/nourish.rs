use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Update};

use crate::nourishment::{band_of, Nourishment, NourishmentAxis, NourishmentValueError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NourishCmd {
    Set { axis: NourishmentAxis, value: f32 },
    Show,
}

impl Command for NourishCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let nourish = graph.root().literal("nourish").id();
        let set = graph.at(nourish).literal("set").id();

        graph
            .at(set)
            .literal("satiety")
            .argument("value")
            .with_parser::<f32>()
            .with_executable(|input| NourishCmd::Set {
                axis: NourishmentAxis::Satiety,
                value: f32::parse_arg(input).unwrap(),
            });

        graph
            .at(set)
            .literal("hydration")
            .argument("value")
            .with_parser::<f32>()
            .with_executable(|input| NourishCmd::Set {
                axis: NourishmentAxis::Hydration,
                value: f32::parse_arg(input).unwrap(),
            });

        graph
            .at(nourish)
            .literal("show")
            .with_executable(|_| NourishCmd::Show);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<NourishCmd>()
        .add_systems(Update, handle_nourish);
}

pub fn handle_nourish(
    mut events: EventReader<CommandResultEvent<NourishCmd>>,
    mut players: Query<(&mut Client, Option<&mut Nourishment>)>,
) {
    for event in events.read() {
        let Ok((mut client, nourishment)) = players.get_mut(event.executor) else {
            continue;
        };
        let Some(mut nourishment) = nourishment else {
            client.send_chat_message(
                "[dev] nourish unavailable: player has no Nourishment component",
            );
            continue;
        };

        match &event.result {
            NourishCmd::Set { axis, value } => {
                let before = nourishment.value(*axis);
                match nourishment.try_set(*axis, *value) {
                    Ok(after) => {
                        tracing::warn!(
                            "[dev-cmd] bypass nourishment consumption: {} {:.3} -> {:.3}",
                            axis.wire_name(),
                            before,
                            after,
                        );
                        client.send_chat_message(format!(
                            "[dev] nourish {} {:.1} -> {:.1}",
                            axis.wire_name(),
                            before,
                            after,
                        ));
                    }
                    Err(NourishmentValueError::NonFinite) => {
                        client
                            .send_chat_message("[dev] nourish set rejected: value must be finite");
                    }
                }
            }
            NourishCmd::Show => {
                client.send_chat_message(format!(
                    "[dev] nourish satiety={:.1}/120 ({:?}) hydration={:.1}/120 ({:?})",
                    nourishment.satiety,
                    band_of(nourishment.satiety),
                    nourishment.hydration,
                    band_of(nourishment.hydration),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::{Entity, Events};
    use valence::protocol::packets::play::{CommandExecutionC2s, GameMessageS2c};
    use valence::protocol::{Bounded, FixedBitSet, VarInt};
    use valence::testing::{create_mock_client, MockClientHelper};

    fn setup_command_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            valence::event_loop::EventLoopPlugin,
            valence::command::manager::CommandPlugin,
        ));
        register(&mut app);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    fn execute_command(app: &mut App, helper: &mut MockClientHelper, command: &str) {
        helper.send(&CommandExecutionC2s {
            command: Bounded(command),
            timestamp: 0,
            salt: 0,
            argument_signatures: Vec::new(),
            message_count: VarInt(0),
            acknowledgement: FixedBitSet::default(),
        });
        app.update();
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

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<NourishCmd>>();
        app.add_systems(Update, handle_nourish);
        app
    }

    fn spawn_nourished_player(app: &mut App, nourishment: Nourishment) -> Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert(nourishment);
        player
    }

    fn send(app: &mut App, player: Entity, result: NourishCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<NourishCmd>>>()
            .send(CommandResultEvent {
                result,
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn command_integration_nourish_show_reports_both_axes_without_mutation() {
        let mut app = setup_command_app();
        let (bundle, mut helper) = create_mock_client("Alice");
        let player = app.world_mut().spawn(bundle).id();
        let initial = Nourishment {
            satiety: 120.0,
            hydration: 20.0,
        };
        app.world_mut().entity_mut(player).insert(initial);

        execute_command(&mut app, &mut helper, "nourish show");

        let chats = flush_and_collect_chat(&mut app, &mut helper);
        assert!(
            chats.iter().any(|text| {
                text == "[dev] nourish satiety=120.0/120 (Overfull) hydration=20.0/120 (Critical)"
            }),
            "/nourish show must report both axes through the production C2S command path, actual: {chats:?}"
        );
        assert_eq!(
            *app.world().get::<Nourishment>(player).unwrap(),
            initial,
            "/nourish show must not mutate either nourishment axis"
        );
    }

    #[test]
    fn command_integration_nourish_set_unknown_literal_does_not_mutate() {
        let mut app = setup_command_app();
        let (bundle, mut helper) = create_mock_client("Alice");
        let player = app.world_mut().spawn(bundle).id();
        let initial = Nourishment::spawn_default();
        app.world_mut().entity_mut(player).insert(initial);

        execute_command(&mut app, &mut helper, "nourish set food 10");

        assert_eq!(
            *app.world().get::<Nourishment>(player).unwrap(),
            initial,
            "an unknown /nourish set literal must not reach the typed set handler"
        );
    }

    #[test]
    fn nourish_command_value_parser_accepts_numeric_float_and_rejects_non_numeric_input() {
        assert_eq!(f32::arg_from_str("10.5").unwrap(), 10.5);
        assert!(f32::arg_from_str("many").is_err());
    }

    #[test]
    fn nourish_set_updates_each_axis_independently() {
        let mut app = setup_app();
        let player = spawn_nourished_player(&mut app, Nourishment::spawn_default());

        send(
            &mut app,
            player,
            NourishCmd::Set {
                axis: NourishmentAxis::Satiety,
                value: 25.0,
            },
        );
        send(
            &mut app,
            player,
            NourishCmd::Set {
                axis: NourishmentAxis::Hydration,
                value: 45.0,
            },
        );
        run_update(&mut app);

        assert_eq!(
            *app.world().get::<Nourishment>(player).unwrap(),
            Nourishment {
                satiety: 25.0,
                hydration: 45.0,
            }
        );
    }

    #[test]
    fn nourish_set_clamps_finite_values_to_closed_range() {
        let mut app = setup_app();
        let player = spawn_nourished_player(&mut app, Nourishment::spawn_default());

        send(
            &mut app,
            player,
            NourishCmd::Set {
                axis: NourishmentAxis::Satiety,
                value: -1.0,
            },
        );
        send(
            &mut app,
            player,
            NourishCmd::Set {
                axis: NourishmentAxis::Hydration,
                value: 121.0,
            },
        );
        run_update(&mut app);

        assert_eq!(
            *app.world().get::<Nourishment>(player).unwrap(),
            Nourishment {
                satiety: 0.0,
                hydration: 120.0,
            }
        );
    }

    #[test]
    fn nourish_set_rejects_every_non_finite_value_without_mutation() {
        let mut app = setup_app();
        let player = spawn_nourished_player(&mut app, Nourishment::spawn_default());

        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            send(
                &mut app,
                player,
                NourishCmd::Set {
                    axis: NourishmentAxis::Satiety,
                    value,
                },
            );
        }
        run_update(&mut app);

        assert_eq!(
            *app.world().get::<Nourishment>(player).unwrap(),
            Nourishment::spawn_default()
        );
    }

    #[test]
    fn nourish_show_is_read_only() {
        let mut app = setup_app();
        let initial = Nourishment {
            satiety: 100.0,
            hydration: 20.0,
        };
        let player = spawn_nourished_player(&mut app, initial);

        send(&mut app, player, NourishCmd::Show);
        run_update(&mut app);

        assert_eq!(*app.world().get::<Nourishment>(player).unwrap(), initial);
    }

    #[test]
    fn nourish_commands_ignore_entities_without_nourishment_safely() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(
            &mut app,
            player,
            NourishCmd::Set {
                axis: NourishmentAxis::Satiety,
                value: 10.0,
            },
        );
        send(&mut app, player, NourishCmd::Show);
        run_update(&mut app);

        assert!(app.world().get::<Nourishment>(player).is_none());
    }
}
