use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, Entity, EventReader, EventWriter, Position, Query, Res, Update,
};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::combat::events::{DebugCombatCommand, DebugCombatCommandKind};
use crate::player::state::{save_player_shrine_anchor_slice, PlayerStatePersistence};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrineAction {
    Set,
    Clear,
}

impl CommandArg for ShrineAction {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        match raw.as_str() {
            "set" => Ok(Self::Set),
            "clear" => Ok(Self::Clear),
            _ => Err(CommandArgParseError::InvalidArgument {
                expected: "set|clear".to_string(),
                got: raw,
            }),
        }
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrineCmd {
    Run { action: ShrineAction },
}

impl Command for ShrineCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("shrine")
            .argument("action")
            .with_parser::<ShrineAction>()
            .with_executable(|input| ShrineCmd::Run {
                action: ShrineAction::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<ShrineCmd>()
        .add_systems(Update, handle_shrine);
}

fn queue_anchor(
    target: Entity,
    anchor: Option<[f64; 3]>,
    tx: &mut EventWriter<DebugCombatCommand>,
) {
    tx.send(DebugCombatCommand {
        target,
        kind: DebugCombatCommandKind::SetSpawnAnchor(anchor),
    });
}

pub fn handle_shrine(
    mut events: EventReader<CommandResultEvent<ShrineCmd>>,
    mut debug_combat_tx: EventWriter<DebugCombatCommand>,
    player_persistence: Option<Res<PlayerStatePersistence>>,
    mut players: Query<(&Position, &mut Client, &valence::prelude::Username)>,
) {
    for event in events.read() {
        let ShrineCmd::Run { action } = event.result;
        let Ok((position, mut client, username)) = players.get_mut(event.executor) else {
            continue;
        };

        match action {
            ShrineAction::Set => {
                let player_pos = position.get();
                let anchor = [player_pos.x, player_pos.y, player_pos.z];
                queue_anchor(event.executor, Some(anchor), &mut debug_combat_tx);
                if let Some(persistence) = player_persistence.as_deref() {
                    if let Err(error) = save_player_shrine_anchor_slice(
                        persistence,
                        username.0.as_str(),
                        Some(anchor),
                    ) {
                        tracing::warn!(
                            "[bong][cmd] failed to persist shrine anchor for `{}`: {error}",
                            username.0
                        );
                    }
                }
                client.send_chat_message("Shrine anchor set to your current position.");
            }
            ShrineAction::Clear => {
                queue_anchor(event.executor, None, &mut debug_combat_tx);
                if let Some(persistence) = player_persistence.as_deref() {
                    if let Err(error) =
                        save_player_shrine_anchor_slice(persistence, username.0.as_str(), None)
                    {
                        tracing::warn!(
                            "[bong][cmd] failed to clear shrine anchor for `{}`: {error}",
                            username.0
                        );
                    }
                }
                client.send_chat_message("Shrine anchor cleared.");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::persistence::bootstrap_sqlite;
    use crate::player::state::load_player_shrine_anchor_slice;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::Events;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<DebugCombatCommand>();
        app.add_event::<CommandResultEvent<ShrineCmd>>();
        app.add_systems(Update, handle_shrine);
        app
    }

    fn temp_persistence() -> PlayerStatePersistence {
        let db_path = std::env::temp_dir().join(format!(
            "bong-shrine-cmd-{}-{}.db",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ));
        bootstrap_sqlite(&db_path, "shrine-cmd-test").expect("sqlite bootstrap should succeed");
        PlayerStatePersistence::with_db_path(std::env::temp_dir(), &db_path)
    }

    #[test]
    fn shrine_action_parses_known_actions() {
        assert_eq!(
            ShrineAction::arg_from_str("set").unwrap(),
            ShrineAction::Set
        );
        assert_eq!(
            ShrineAction::arg_from_str("clear").unwrap(),
            ShrineAction::Clear
        );
    }

    #[test]
    fn shrine_action_rejects_unknown_action() {
        assert!(ShrineAction::arg_from_str("reset").is_err());
    }

    #[test]
    fn shrine_set_emits_anchor_and_persists_it() {
        let mut app = setup_app();
        let persistence = temp_persistence();
        app.insert_resource(persistence);
        let player = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ShrineCmd>>>()
            .send(CommandResultEvent {
                result: ShrineCmd::Run {
                    action: ShrineAction::Set,
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let events = app.world().resource::<Events<DebugCombatCommand>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).cloned().collect::<Vec<_>>();
        assert!(matches!(
            collected.as_slice(),
            [DebugCombatCommand {
                kind: DebugCombatCommandKind::SetSpawnAnchor(Some(anchor)),
                ..
            }] if *anchor == [8.0, 66.0, 8.0]
        ));

        let persistence = app.world().resource::<PlayerStatePersistence>();
        assert_eq!(
            load_player_shrine_anchor_slice(persistence, "Alice").unwrap(),
            Some([8.0, 66.0, 8.0])
        );
    }

    #[test]
    fn shrine_clear_emits_none_anchor() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ShrineCmd>>>()
            .send(CommandResultEvent {
                result: ShrineCmd::Run {
                    action: ShrineAction::Clear,
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let events = app.world().resource::<Events<DebugCombatCommand>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).cloned().collect::<Vec<_>>();
        assert!(matches!(
            collected.as_slice(),
            [DebugCombatCommand {
                kind: DebugCombatCommandKind::SetSpawnAnchor(None),
                ..
            }]
        ));
    }
}
