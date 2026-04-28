use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, GameMode, Query, Update};
use valence::protocol::packets::play::command_tree_s2c::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmMode {
    Creative,
    Adventure,
    Spectator,
}

impl GmMode {
    pub fn as_game_mode(self) -> GameMode {
        match self {
            Self::Creative => GameMode::Creative,
            Self::Adventure => GameMode::Adventure,
            Self::Spectator => GameMode::Spectator,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Creative => "Creative",
            Self::Adventure => "Adventure",
            Self::Spectator => "Spectator",
        }
    }
}

impl CommandArg for GmMode {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        match raw.as_str() {
            "c" | "creative" => Ok(Self::Creative),
            "a" | "adventure" => Ok(Self::Adventure),
            "s" | "spectator" => Ok(Self::Spectator),
            _ => Err(CommandArgParseError::InvalidArgument {
                expected: "c|a|s".to_string(),
                got: raw,
            }),
        }
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmCmd {
    Set { mode: GmMode },
}

impl Command for GmCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("gm")
            .argument("mode")
            .with_parser::<GmMode>()
            .with_executable(|input| GmCmd::Set {
                mode: GmMode::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<GmCmd>().add_systems(Update, handle_gm);
}

pub fn handle_gm(
    mut events: EventReader<CommandResultEvent<GmCmd>>,
    mut players: Query<(&mut GameMode, &mut Client)>,
) {
    for event in events.read() {
        let GmCmd::Set { mode } = event.result;
        let Ok((mut game_mode, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        *game_mode = mode.as_game_mode();
        client.send_chat_message(format!("Gamemode set to {}.", mode.label()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    #[test]
    fn gm_mode_parses_aliases() {
        for (raw, expected) in [
            ("c", GmMode::Creative),
            ("creative", GmMode::Creative),
            ("a", GmMode::Adventure),
            ("adventure", GmMode::Adventure),
            ("s", GmMode::Spectator),
            ("spectator", GmMode::Spectator),
        ] {
            assert_eq!(GmMode::arg_from_str(raw).unwrap(), expected);
        }
    }

    #[test]
    fn gm_mode_rejects_unknown_mode() {
        assert!(GmMode::arg_from_str("survival").is_err());
    }

    #[test]
    fn gm_command_sets_game_mode() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<GmCmd>>();
        app.add_systems(Update, handle_gm);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<GmCmd>>>()
            .send(CommandResultEvent {
                result: GmCmd::Set {
                    mode: GmMode::Spectator,
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        assert_eq!(
            *app.world().get::<GameMode>(player).unwrap(),
            GameMode::Spectator
        );
    }
}
