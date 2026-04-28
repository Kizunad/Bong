pub mod breakthrough;
pub mod combat;
pub mod gather;

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, ResMut, Update, Username};

use crate::player::gameplay::{GameplayAction, GameplayActionQueue};
use crate::player::state::canonical_player_id;

#[derive(Debug, Clone, PartialEq)]
pub enum BongCmd {
    Combat { target: String, qi_invest: f64 },
    Gather { resource: String },
    Breakthrough,
}

impl Command for BongCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let bong = graph.root().literal("bong").id();

        graph
            .at(bong)
            .literal("combat")
            .argument("target")
            .with_parser::<String>()
            .argument("qi_invest")
            .with_parser::<f64>()
            .with_executable(|input| BongCmd::Combat {
                target: String::parse_arg(input).unwrap(),
                qi_invest: f64::parse_arg(input).unwrap(),
            });

        graph
            .at(bong)
            .literal("gather")
            .argument("resource")
            .with_parser::<String>()
            .with_executable(|input| BongCmd::Gather {
                resource: String::parse_arg(input).unwrap(),
            });

        graph
            .at(bong)
            .literal("breakthrough")
            .with_executable(|_| BongCmd::Breakthrough);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<BongCmd>()
        .add_systems(Update, handle_bong_gameplay);
}

fn into_gameplay_action(command: &BongCmd) -> GameplayAction {
    match command {
        BongCmd::Combat { target, qi_invest } => combat::action(target.clone(), *qi_invest),
        BongCmd::Gather { resource } => gather::action(resource.clone()),
        BongCmd::Breakthrough => breakthrough::action(),
    }
}

pub fn handle_bong_gameplay(
    mut events: EventReader<CommandResultEvent<BongCmd>>,
    mut gameplay_queue: ResMut<GameplayActionQueue>,
    mut players: Query<(&Username, &mut Client)>,
) {
    for event in events.read() {
        let Ok((username, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        gameplay_queue.enqueue(
            canonical_player_id(username.0.as_str()),
            into_gameplay_action(&event.result),
        );
        client.send_chat_message("Gameplay action queued.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::player::gameplay::QueuedGameplayAction;
    use crate::player::gameplay::{CombatAction, GatherAction};
    use valence::prelude::Events;

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(GameplayActionQueue::default());
        app.add_event::<CommandResultEvent<BongCmd>>();
        app.add_systems(Update, handle_bong_gameplay);
        app
    }

    fn send(app: &mut App, executor: valence::prelude::Entity, result: BongCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<BongCmd>>>()
            .send(CommandResultEvent {
                result,
                executor,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn combat_command_enqueues_combat_action() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send(
            &mut app,
            player,
            BongCmd::Combat {
                target: "Crimson".to_string(),
                qi_invest: 12.5,
            },
        );

        run_update(&mut app);

        assert_eq!(
            app.world()
                .resource::<GameplayActionQueue>()
                .pending_actions_snapshot(),
            vec![QueuedGameplayAction {
                player: "offline:Alice".to_string(),
                action: GameplayAction::Combat(CombatAction {
                    target: "Crimson".to_string(),
                    qi_invest: 12.5,
                }),
            }]
        );
    }

    #[test]
    fn gather_command_enqueues_gather_action() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send(
            &mut app,
            player,
            BongCmd::Gather {
                resource: "spirit_herb".to_string(),
            },
        );

        run_update(&mut app);

        assert!(matches!(
            app.world()
                .resource::<GameplayActionQueue>()
                .pending_actions_snapshot()
                .as_slice(),
            [QueuedGameplayAction {
                action: GameplayAction::Gather(GatherAction { resource, .. }),
                ..
            }] if resource == "spirit_herb"
        ));
    }

    #[test]
    fn breakthrough_command_enqueues_attempt() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send(&mut app, player, BongCmd::Breakthrough);

        run_update(&mut app);

        assert!(matches!(
            app.world()
                .resource::<GameplayActionQueue>()
                .pending_actions_snapshot()
                .as_slice(),
            [QueuedGameplayAction {
                action: GameplayAction::AttemptBreakthrough,
                ..
            }]
        ));
    }

    #[test]
    fn parser_contract_keeps_combat_argument_as_qi_invest() {
        let action = into_gameplay_action(&BongCmd::Combat {
            target: "Crimson".to_string(),
            qi_invest: 40.0,
        });

        assert_eq!(
            action,
            GameplayAction::Combat(CombatAction {
                target: "Crimson".to_string(),
                qi_invest: 40.0,
            })
        );
    }
}
