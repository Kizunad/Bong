use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Update};

use crate::cultivation::components::Cultivation;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QiCmd {
    Set { value: f64 },
    Max { value: f64 },
}

impl Command for QiCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let qi = graph.root().literal("qi").id();

        graph
            .at(qi)
            .literal("set")
            .argument("value")
            .with_parser::<f64>()
            .with_executable(|input| QiCmd::Set {
                value: f64::parse_arg(input).unwrap(),
            });

        graph
            .at(qi)
            .literal("max")
            .argument("value")
            .with_parser::<f64>()
            .with_executable(|input| QiCmd::Max {
                value: f64::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<QiCmd>().add_systems(Update, handle_qi);
}

pub fn handle_qi(
    mut events: EventReader<CommandResultEvent<QiCmd>>,
    mut players: Query<(&mut Cultivation, &mut Client)>,
) {
    for event in events.read() {
        let Ok((mut cultivation, mut client)) = players.get_mut(event.executor) else {
            continue;
        };

        match event.result {
            QiCmd::Set { value } => {
                if !value.is_finite() || value < 0.0 {
                    client.send_chat_message("[dev] qi set rejected: value must be finite >= 0");
                    continue;
                }
                let before = cultivation.qi_current;
                cultivation.qi_current = value.min(cultivation.qi_max.max(0.0));
                tracing::warn!(
                    "[dev-cmd] bypass ledger: qi_current {:.3} -> {:.3}",
                    before,
                    cultivation.qi_current
                );
                client.send_chat_message(format!(
                    "[dev] qi set {:.1} -> {:.1}",
                    before, cultivation.qi_current
                ));
            }
            QiCmd::Max { value } => {
                if !value.is_finite() || value < 0.0 {
                    client.send_chat_message("[dev] qi max rejected: value must be finite >= 0");
                    continue;
                }
                let before_max = cultivation.qi_max;
                cultivation.qi_max = value;
                cultivation.qi_current = cultivation.qi_current.min(value);
                tracing::warn!(
                    "[dev-cmd] bypass ledger: qi_max {:.3} -> {:.3}",
                    before_max,
                    cultivation.qi_max
                );
                client.send_chat_message(format!(
                    "[dev] qi max {:.1} -> {:.1}; current={:.1}",
                    before_max, cultivation.qi_max, cultivation.qi_current
                ));
            }
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
        app.add_event::<CommandResultEvent<QiCmd>>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, handle_qi);
        app
    }

    fn spawn_cultivator(app: &mut App, qi_current: f64, qi_max: f64) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert(Cultivation {
            qi_current,
            qi_max,
            ..Default::default()
        });
        player
    }

    fn send(app: &mut App, player: valence::prelude::Entity, result: QiCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<QiCmd>>>()
            .send(CommandResultEvent {
                result,
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn qi_set_writes_current_and_clamps_to_max() {
        let mut app = setup_app();
        let player = spawn_cultivator(&mut app, 0.0, 100.0);

        send(&mut app, player, QiCmd::Set { value: 50.0 });
        run_update(&mut app);
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            50.0
        );

        send(&mut app, player, QiCmd::Set { value: 999.0 });
        run_update(&mut app);
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            100.0
        );
    }

    #[test]
    fn qi_set_rejects_negative_and_non_finite_values() {
        let mut app = setup_app();
        let player = spawn_cultivator(&mut app, 25.0, 100.0);

        for value in [-10.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            send(&mut app, player, QiCmd::Set { value });
        }
        run_update(&mut app);

        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            25.0
        );
    }

    #[test]
    fn qi_max_updates_max_and_clamps_current() {
        let mut app = setup_app();
        let player = spawn_cultivator(&mut app, 100.0, 100.0);

        send(&mut app, player, QiCmd::Max { value: 200.0 });
        run_update(&mut app);
        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(cultivation.qi_max, 200.0);
        assert_eq!(cultivation.qi_current, 100.0);

        send(&mut app, player, QiCmd::Max { value: 50.0 });
        run_update(&mut app);
        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(cultivation.qi_max, 50.0);
        assert_eq!(cultivation.qi_current, 50.0);
    }

    #[test]
    fn qi_commands_do_not_emit_qi_transfer_events() {
        let mut app = setup_app();
        let player = spawn_cultivator(&mut app, 0.0, 100.0);

        send(&mut app, player, QiCmd::Set { value: 40.0 });
        send(&mut app, player, QiCmd::Max { value: 80.0 });
        run_update(&mut app);

        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);
    }
}
