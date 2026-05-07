use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, GameMode, Query, Update};
use valence::protocol::packets::play::command_tree_s2c::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GmMode {
    pub fn as_game_mode(self) -> GameMode {
        match self {
            Self::Survival => GameMode::Survival,
            Self::Creative => GameMode::Creative,
            Self::Adventure => GameMode::Adventure,
            Self::Spectator => GameMode::Spectator,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Survival => "Survival",
            Self::Creative => "Creative",
            Self::Adventure => "Adventure",
            Self::Spectator => "Spectator",
        }
    }
}

impl CommandArg for GmMode {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        // 别名同 vanilla `/gamemode`：s=survival、c=creative、a=adventure、sp=spectator。
        match raw.as_str() {
            "s" | "survival" => Ok(Self::Survival),
            "c" | "creative" => Ok(Self::Creative),
            "a" | "adventure" => Ok(Self::Adventure),
            "sp" | "spectator" => Ok(Self::Spectator),
            _ => Err(CommandArgParseError::InvalidArgument {
                expected: "s|c|a|sp".to_string(),
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
            ("s", GmMode::Survival),
            ("survival", GmMode::Survival),
            ("c", GmMode::Creative),
            ("creative", GmMode::Creative),
            ("a", GmMode::Adventure),
            ("adventure", GmMode::Adventure),
            ("sp", GmMode::Spectator),
            ("spectator", GmMode::Spectator),
        ] {
            assert_eq!(GmMode::arg_from_str(raw).unwrap(), expected);
        }
    }

    #[test]
    fn gm_mode_rejects_unknown_mode() {
        // 历史上 `s` 曾是 spectator 的别名（与 vanilla 不一致），切到 survival 之后
        // 旧别名失效；下面这些字串确认 parser 不会接受非正典写法。
        for raw in ["spec", "creat", "adv", "survive", "0", ""] {
            assert!(
                GmMode::arg_from_str(raw).is_err(),
                "unexpectedly parsed `{raw}`"
            );
        }
    }

    #[test]
    fn gm_modes_map_to_expected_labels_and_game_modes() {
        for (mode, label, game_mode) in [
            (GmMode::Survival, "Survival", GameMode::Survival),
            (GmMode::Creative, "Creative", GameMode::Creative),
            (GmMode::Adventure, "Adventure", GameMode::Adventure),
            (GmMode::Spectator, "Spectator", GameMode::Spectator),
        ] {
            assert_eq!(mode.label(), label);
            assert_eq!(mode.as_game_mode(), game_mode);
        }
    }

    #[test]
    fn gm_command_sets_game_mode() {
        for target_mode in [
            GmMode::Survival,
            GmMode::Creative,
            GmMode::Adventure,
            GmMode::Spectator,
        ] {
            let mut app = App::new();
            app.add_event::<CommandResultEvent<GmCmd>>();
            app.add_systems(Update, handle_gm);
            let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
            app.world_mut()
                .resource_mut::<Events<CommandResultEvent<GmCmd>>>()
                .send(CommandResultEvent {
                    result: GmCmd::Set { mode: target_mode },
                    executor: player,
                    modifiers: Default::default(),
                });

            run_update(&mut app);

            assert_eq!(
                *app.world().get::<GameMode>(player).unwrap(),
                target_mode.as_game_mode(),
                "command failed to set {target_mode:?}"
            );
        }
    }
}
