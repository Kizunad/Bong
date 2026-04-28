use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, Entity, EventReader, EventWriter, Query, Update};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::combat::components::{BodyPart, WoundKind};
use crate::combat::events::{DebugCombatCommand, DebugCombatCommandKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyPartArg(pub BodyPart);

impl CommandArg for BodyPartArg {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        match raw.as_str() {
            "head" => Ok(Self(BodyPart::Head)),
            "chest" => Ok(Self(BodyPart::Chest)),
            "abdomen" => Ok(Self(BodyPart::Abdomen)),
            "arml" => Ok(Self(BodyPart::ArmL)),
            "armr" => Ok(Self(BodyPart::ArmR)),
            "legl" => Ok(Self(BodyPart::LegL)),
            "legr" => Ok(Self(BodyPart::LegR)),
            _ => Err(CommandArgParseError::InvalidArgument {
                expected: "head|chest|abdomen|arml|armr|legl|legr".to_string(),
                got: raw,
            }),
        }
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WoundCmd {
    Add {
        part: BodyPartArg,
        severity: Option<f32>,
    },
}

impl Command for WoundCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let add_part = graph
            .root()
            .literal("wound")
            .literal("add")
            .argument("part")
            .with_parser::<BodyPartArg>()
            .with_executable(|input| WoundCmd::Add {
                part: BodyPartArg::parse_arg(input).unwrap(),
                severity: None,
            })
            .id();

        graph
            .at(add_part)
            .argument("severity")
            .with_parser::<f32>()
            .with_executable(|input| WoundCmd::Add {
                part: BodyPartArg::parse_arg(input).unwrap(),
                severity: Some(f32::parse_arg(input).unwrap()),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<WoundCmd>()
        .add_systems(Update, handle_wound);
}

pub fn queue_wound_command(
    target: Entity,
    location: BodyPart,
    severity: f32,
    tx: &mut EventWriter<DebugCombatCommand>,
) {
    tx.send(DebugCombatCommand {
        target,
        kind: DebugCombatCommandKind::AddWound {
            location,
            kind: WoundKind::Blunt,
            severity,
        },
    });
}

pub fn handle_wound(
    mut events: EventReader<CommandResultEvent<WoundCmd>>,
    mut debug_combat_tx: EventWriter<DebugCombatCommand>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let WoundCmd::Add { part, severity } = event.result;
        let severity = severity.unwrap_or(0.3);
        queue_wound_command(event.executor, part.0, severity, &mut debug_combat_tx);
        if let Ok(mut client) = clients.get_mut(event.executor) {
            client.send_chat_message(format!(
                "Queued /wound add {:?} severity={severity:.2}",
                part.0
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    #[test]
    fn body_part_arg_parses_all_supported_parts() {
        for (raw, expected) in [
            ("head", BodyPart::Head),
            ("chest", BodyPart::Chest),
            ("abdomen", BodyPart::Abdomen),
            ("arml", BodyPart::ArmL),
            ("armr", BodyPart::ArmR),
            ("legl", BodyPart::LegL),
            ("legr", BodyPart::LegR),
        ] {
            assert_eq!(
                BodyPartArg::arg_from_str(raw).unwrap(),
                BodyPartArg(expected)
            );
        }
    }

    #[test]
    fn body_part_arg_rejects_unknown_part() {
        assert!(BodyPartArg::arg_from_str("tail").is_err());
    }

    #[test]
    fn wound_command_defaults_severity() {
        let mut app = App::new();
        app.add_event::<DebugCombatCommand>();
        app.add_event::<CommandResultEvent<WoundCmd>>();
        app.add_systems(Update, handle_wound);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<WoundCmd>>>()
            .send(CommandResultEvent {
                result: WoundCmd::Add {
                    part: BodyPartArg(BodyPart::Chest),
                    severity: None,
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
                kind: DebugCombatCommandKind::AddWound {
                    location: BodyPart::Chest,
                    severity,
                    ..
                },
                ..
            }] if (*severity - 0.3).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn wound_command_uses_explicit_severity() {
        let mut app = App::new();
        app.add_event::<DebugCombatCommand>();
        app.add_event::<CommandResultEvent<WoundCmd>>();
        app.add_systems(Update, handle_wound);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<WoundCmd>>>()
            .send(CommandResultEvent {
                result: WoundCmd::Add {
                    part: BodyPartArg(BodyPart::Head),
                    severity: Some(0.7),
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
                kind: DebugCombatCommandKind::AddWound {
                    location: BodyPart::Head,
                    severity,
                    ..
                },
                ..
            }] if (*severity - 0.7).abs() < f32::EPSILON
        ));
    }
}
