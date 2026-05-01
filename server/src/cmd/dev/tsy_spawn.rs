use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, Entity, EventReader, EventWriter, Position, Query, Update};

use crate::world::tsy_dev_command::TsySpawnRequested;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TsySpawnCmd {
    Spawn { family_id: String },
}

impl Command for TsySpawnCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("tsy_spawn")
            .argument("family_id")
            .with_parser::<String>()
            .with_executable(|input| TsySpawnCmd::Spawn {
                family_id: String::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<TsySpawnCmd>()
        .add_systems(Update, handle_tsy_spawn);
}

pub fn queue_tsy_spawn(
    player_entity: Entity,
    player_pos: valence::prelude::DVec3,
    family_id: String,
    tx: &mut EventWriter<TsySpawnRequested>,
) {
    tx.send(TsySpawnRequested {
        player_entity,
        player_pos,
        family_id,
    });
}

pub fn handle_tsy_spawn(
    mut events: EventReader<CommandResultEvent<TsySpawnCmd>>,
    mut tsy_spawn_tx: EventWriter<TsySpawnRequested>,
    mut clients: Query<(&Position, &mut Client)>,
) {
    for event in events.read() {
        let TsySpawnCmd::Spawn { family_id } = &event.result;
        let Ok((position, mut client)) = clients.get_mut(event.executor) else {
            continue;
        };
        queue_tsy_spawn(
            event.executor,
            position.get(),
            family_id.clone(),
            &mut tsy_spawn_tx,
        );
        client.send_chat_message(format!(
            "Queued /tsy_spawn {family_id} (查看 server 日志确认结果)"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::{Events, Position};

    #[test]
    fn family_id_parser_accepts_single_word_family_id() {
        assert_eq!(
            String::arg_from_str("tsy_lingxu_01").unwrap(),
            "tsy_lingxu_01"
        );
    }

    #[test]
    fn family_id_parser_stops_at_whitespace() {
        assert_eq!(
            String::arg_from_str("tsy_lingxu_01 extra").unwrap(),
            "tsy_lingxu_01"
        );
    }

    #[test]
    fn tsy_spawn_emits_requested_event() {
        let mut app = App::new();
        app.add_event::<TsySpawnRequested>();
        app.add_event::<CommandResultEvent<TsySpawnCmd>>();
        app.add_systems(Update, handle_tsy_spawn);
        let player = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TsySpawnCmd>>>()
            .send(CommandResultEvent {
                result: TsySpawnCmd::Spawn {
                    family_id: "tsy_lingxu_01".to_string(),
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let events = app.world().resource::<Events<TsySpawnRequested>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).cloned().collect::<Vec<_>>();
        assert!(matches!(
            collected.as_slice(),
            [TsySpawnRequested {
                family_id,
                ..
            }] if family_id == "tsy_lingxu_01"
        ));
    }

    #[test]
    fn tsy_spawn_uses_executor_position() {
        let mut app = App::new();
        app.add_event::<TsySpawnRequested>();
        let player = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        app.add_event::<CommandResultEvent<TsySpawnCmd>>();
        app.add_systems(Update, handle_tsy_spawn);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TsySpawnCmd>>>()
            .send(CommandResultEvent {
                result: TsySpawnCmd::Spawn {
                    family_id: "tsy_lingxu_01".to_string(),
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let events = app.world().resource::<Events<TsySpawnRequested>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).cloned().collect::<Vec<_>>();
        assert!(matches!(
            collected.as_slice(),
            [TsySpawnRequested { player_pos, .. }] if *player_pos == Position::new([8.0, 66.0, 8.0]).get()
        ));
    }

    #[test]
    fn tsy_spawn_without_executor_does_not_emit_request() {
        let mut app = App::new();
        app.add_event::<TsySpawnRequested>();
        app.add_event::<CommandResultEvent<TsySpawnCmd>>();
        app.add_systems(Update, handle_tsy_spawn);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TsySpawnCmd>>>()
            .send(CommandResultEvent {
                result: TsySpawnCmd::Spawn {
                    family_id: "tsy_lingxu_01".to_string(),
                },
                executor: valence::prelude::Entity::PLACEHOLDER,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let events = app.world().resource::<Events<TsySpawnRequested>>();
        let mut reader = events.get_reader();
        assert_eq!(reader.read(events).count(), 0);
    }
}
