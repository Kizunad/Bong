use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Position, Query, ResMut, Update};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::npc::scenario::{PendingScenario, ScenarioType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcScenarioAction {
    Chase,
    Flee,
    Fight,
    Kite,
    Swarm,
    Duel,
    Clear,
}

impl NpcScenarioAction {
    pub fn into_scenario_type(self) -> ScenarioType {
        match self {
            Self::Chase => ScenarioType::Chase,
            Self::Flee => ScenarioType::Flee,
            Self::Fight => ScenarioType::Fight,
            Self::Kite => ScenarioType::Kite,
            Self::Swarm => ScenarioType::Swarm,
            Self::Duel => ScenarioType::Duel,
            Self::Clear => ScenarioType::Clear,
        }
    }
}

impl CommandArg for NpcScenarioAction {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        let Some(scenario) = ScenarioType::from_str(raw.as_str()) else {
            return Err(CommandArgParseError::InvalidArgument {
                expected: "chase|flee|fight|kite|swarm|duel|clear".to_string(),
                got: raw,
            });
        };

        Ok(match scenario {
            ScenarioType::Chase => Self::Chase,
            ScenarioType::Flee => Self::Flee,
            ScenarioType::Fight => Self::Fight,
            ScenarioType::Kite => Self::Kite,
            ScenarioType::Swarm => Self::Swarm,
            ScenarioType::Duel => Self::Duel,
            ScenarioType::Clear => Self::Clear,
        })
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcScenarioCmd {
    Run { scenario: NpcScenarioAction },
}

impl Command for NpcScenarioCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("npc_scenario")
            .argument("scenario")
            .with_parser::<NpcScenarioAction>()
            .with_executable(|input| NpcScenarioCmd::Run {
                scenario: NpcScenarioAction::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<NpcScenarioCmd>()
        .add_systems(Update, handle_npc_scenario);
}

pub fn handle_npc_scenario(
    mut events: EventReader<CommandResultEvent<NpcScenarioCmd>>,
    mut pending: ResMut<PendingScenario>,
    mut players: Query<(&Position, &mut Client)>,
) {
    for event in events.read() {
        let NpcScenarioCmd::Run { scenario } = event.result;
        let Ok((position, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        pending.request = Some((scenario.into_scenario_type(), position.get()));
        client.send_chat_message("Scenario queued.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    #[test]
    fn scenario_action_parses_known_values() {
        assert_eq!(
            NpcScenarioAction::arg_from_str("clear").unwrap(),
            NpcScenarioAction::Clear
        );
    }

    #[test]
    fn scenario_action_rejects_unknown_value() {
        assert!(NpcScenarioAction::arg_from_str("other").is_err());
    }

    #[test]
    fn scenario_action_maps_all_variants_to_runtime_scenario_type() {
        for (action, scenario) in [
            (NpcScenarioAction::Chase, ScenarioType::Chase),
            (NpcScenarioAction::Flee, ScenarioType::Flee),
            (NpcScenarioAction::Fight, ScenarioType::Fight),
            (NpcScenarioAction::Kite, ScenarioType::Kite),
            (NpcScenarioAction::Swarm, ScenarioType::Swarm),
            (NpcScenarioAction::Duel, ScenarioType::Duel),
            (NpcScenarioAction::Clear, ScenarioType::Clear),
        ] {
            assert_eq!(
                std::mem::discriminant(&action.into_scenario_type()),
                std::mem::discriminant(&scenario)
            );
        }
    }

    #[test]
    fn scenario_command_queues_request() {
        let mut app = App::new();
        app.insert_resource(PendingScenario::default());
        app.add_event::<CommandResultEvent<NpcScenarioCmd>>();
        app.add_systems(Update, handle_npc_scenario);
        let player = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<NpcScenarioCmd>>>()
            .send(CommandResultEvent {
                result: NpcScenarioCmd::Run {
                    scenario: NpcScenarioAction::Clear,
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let (scenario, pos) = app
            .world()
            .resource::<PendingScenario>()
            .request
            .expect("scenario request should be queued");
        assert!(matches!(scenario, ScenarioType::Clear));
        assert_eq!(pos.to_array(), [8.0, 66.0, 8.0]);
    }
}
