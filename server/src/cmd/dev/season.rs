use std::collections::HashSet;

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, App, Client, EventReader, EventWriter, Query, Res, ResMut, Resource, Update, Username,
};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::cultivation::tick::CultivationClock;
use crate::world::season::{
    query_season, Season, SeasonChangedEvent, WorldSeasonState, VANILLA_DAY_TICKS, YEAR_TICKS,
};

#[derive(Debug, Clone, Resource)]
pub struct SeasonCommandPermissions {
    allowed_usernames: HashSet<String>,
    allow_all: bool,
}

impl Default for SeasonCommandPermissions {
    fn default() -> Self {
        let mut allowed_usernames = HashSet::new();
        allowed_usernames.insert("Admin".to_string());
        allowed_usernames.insert("admin".to_string());
        Self {
            allowed_usernames,
            allow_all: false,
        }
    }
}

impl SeasonCommandPermissions {
    #[cfg(test)]
    pub fn allow_user(username: impl Into<String>) -> Self {
        let mut allowed_usernames = HashSet::new();
        allowed_usernames.insert(username.into());
        Self {
            allowed_usernames,
            allow_all: false,
        }
    }

    pub fn is_allowed(&self, username: &str) -> bool {
        self.allow_all || self.allowed_usernames.contains(username)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeasonPhaseArg(pub Season);

impl CommandArg for SeasonPhaseArg {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        let phase = match raw.as_str() {
            "summer" => Season::Summer,
            "winter" => Season::Winter,
            "summer_to_winter" | "xizhuan_to_winter" => Season::SummerToWinter,
            "winter_to_summer" | "xizhuan_to_summer" => Season::WinterToSummer,
            _ => {
                return Err(CommandArgParseError::InvalidArgument {
                    expected: "summer|winter|summer_to_winter|winter_to_summer".to_string(),
                    got: raw,
                });
            }
        };
        Ok(Self(phase))
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeasonAdvanceArg {
    pub ticks: u64,
}

impl CommandArg for SeasonAdvanceArg {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        parse_advance_ticks(raw.as_str())
            .map(|ticks| Self { ticks })
            .ok_or_else(|| CommandArgParseError::InvalidArgument {
                expected: "<N>[h|d|y|t]".to_string(),
                got: raw,
            })
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeasonCmd {
    Query,
    Set { phase: Season },
    Advance { ticks: u64 },
}

impl Command for SeasonCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let season = graph.root().literal("season").id();

        graph
            .at(season)
            .literal("query")
            .with_executable(|_| SeasonCmd::Query);

        graph
            .at(season)
            .literal("set")
            .argument("phase")
            .with_parser::<SeasonPhaseArg>()
            .with_executable(|input| SeasonCmd::Set {
                phase: SeasonPhaseArg::parse_arg(input).unwrap().0,
            });

        graph
            .at(season)
            .literal("advance")
            .argument("amount")
            .with_parser::<SeasonAdvanceArg>()
            .with_executable(|input| SeasonCmd::Advance {
                ticks: SeasonAdvanceArg::parse_arg(input).unwrap().ticks,
            });
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<WorldSeasonState>()
        .add_event::<SeasonChangedEvent>()
        .insert_resource(SeasonCommandPermissions::default())
        .add_command::<SeasonCmd>()
        .add_systems(Update, handle_season);
}

pub fn handle_season(
    mut events: EventReader<CommandResultEvent<SeasonCmd>>,
    clock: Option<Res<CultivationClock>>,
    mut season_state: ResMut<WorldSeasonState>,
    permissions: Res<SeasonCommandPermissions>,
    usernames: Query<&Username>,
    mut clients: Query<&mut Client>,
    mut season_events: EventWriter<SeasonChangedEvent>,
) {
    let clock_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for event in events.read() {
        let username = usernames
            .get(event.executor)
            .map(|username| username.0.as_str())
            .unwrap_or_default();
        if !permissions.is_allowed(username) {
            send_direct_message(
                &mut clients,
                event.executor,
                "Command requires operator permission.",
            );
            continue;
        }

        let before = query_season("", season_state.effective_tick(clock_tick));
        let after = match event.result {
            SeasonCmd::Query => before,
            SeasonCmd::Set { phase } => season_state.set_phase(phase, clock_tick),
            SeasonCmd::Advance { ticks } => season_state.advance_by_ticks(ticks, clock_tick),
        };
        if matches!(
            event.result,
            SeasonCmd::Set { .. } | SeasonCmd::Advance { .. }
        ) && before.season != after.season
        {
            season_events.send(SeasonChangedEvent {
                from: before.season,
                to: after.season,
                tick: clock_tick,
            });
        }

        send_direct_message(
            &mut clients,
            event.executor,
            format!(
                "season={} tick_into_phase={} phase_total_ticks={} year_index={}",
                after.season.as_wire_str(),
                after.tick_into_phase,
                after.phase_total_ticks,
                after.year_index
            ),
        );
    }
}

fn send_direct_message(
    clients: &mut Query<&mut Client>,
    executor: valence::prelude::Entity,
    message: impl Into<String>,
) {
    if let Ok(mut client) = clients.get_mut(executor) {
        client.send_chat_message(message.into());
    }
}

fn parse_advance_ticks(raw: &str) -> Option<u64> {
    if raw.is_empty() {
        return None;
    }
    let (digits, multiplier) = match raw.as_bytes().last().copied()? as char {
        'h' => (&raw[..raw.len() - 1], 3600 * 20),
        'd' => (&raw[..raw.len() - 1], VANILLA_DAY_TICKS),
        'y' => (&raw[..raw.len() - 1], YEAR_TICKS),
        't' => (&raw[..raw.len() - 1], 1),
        c if c.is_ascii_digit() => (raw, 1),
        _ => return None,
    };
    let amount = digits.parse::<u64>().ok()?;
    amount.checked_mul(multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    #[test]
    fn season_phase_parser_accepts_aliases() {
        assert_eq!(
            SeasonPhaseArg::arg_from_str("summer").unwrap().0,
            Season::Summer
        );
        assert_eq!(
            SeasonPhaseArg::arg_from_str("winter").unwrap().0,
            Season::Winter
        );
        assert_eq!(
            SeasonPhaseArg::arg_from_str("xizhuan_to_winter").unwrap().0,
            Season::SummerToWinter
        );
        assert_eq!(
            SeasonPhaseArg::arg_from_str("winter_to_summer").unwrap().0,
            Season::WinterToSummer
        );
    }

    #[test]
    fn season_advance_parser_accepts_units() {
        assert_eq!(
            SeasonAdvanceArg::arg_from_str("5h").unwrap().ticks,
            5 * 3600 * 20
        );
        assert_eq!(
            SeasonAdvanceArg::arg_from_str("2d").unwrap().ticks,
            2 * VANILLA_DAY_TICKS
        );
        assert_eq!(
            SeasonAdvanceArg::arg_from_str("1y").unwrap().ticks,
            YEAR_TICKS
        );
        assert_eq!(SeasonAdvanceArg::arg_from_str("99t").unwrap().ticks, 99);
        assert_eq!(SeasonAdvanceArg::arg_from_str("42").unwrap().ticks, 42);
    }

    fn setup_command_app(allow_alice: bool) -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<SeasonCmd>>();
        app.add_event::<SeasonChangedEvent>();
        app.insert_resource(WorldSeasonState::default());
        app.insert_resource(CultivationClock::default());
        if allow_alice {
            app.insert_resource(SeasonCommandPermissions::allow_user("Alice"));
        } else {
            app.insert_resource(SeasonCommandPermissions::default());
        }
        app.add_systems(Update, handle_season);
        app
    }

    #[test]
    fn slash_season_set_winter_advances_to_winter_phase_zero() {
        let mut app = setup_command_app(true);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<SeasonCmd>>>()
            .send(CommandResultEvent {
                result: SeasonCmd::Set {
                    phase: Season::Winter,
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let state = app.world().resource::<WorldSeasonState>();
        assert_eq!(state.current.season, Season::Winter);
        assert_eq!(state.current.tick_into_phase, 0);
    }

    #[test]
    fn slash_season_advance_5h_advances_correctly() {
        let mut app = setup_command_app(true);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<SeasonCmd>>>()
            .send(CommandResultEvent {
                result: SeasonCmd::Advance {
                    ticks: SeasonAdvanceArg::arg_from_str("5h").unwrap().ticks,
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let state = app.world().resource::<WorldSeasonState>();
        assert_eq!(state.current.season, Season::Summer);
        assert_eq!(state.current.tick_into_phase, 5 * 3600 * 20);
    }

    #[test]
    fn slash_season_command_rejected_for_non_op_player() {
        let mut app = setup_command_app(false);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<SeasonCmd>>>()
            .send(CommandResultEvent {
                result: SeasonCmd::Set {
                    phase: Season::Winter,
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let state = app.world().resource::<WorldSeasonState>();
        assert_eq!(state.current.season, Season::Summer);
    }

    #[test]
    fn slash_season_set_emits_season_changed_event() {
        let mut app = setup_command_app(true);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<SeasonCmd>>>()
            .send(CommandResultEvent {
                result: SeasonCmd::Set {
                    phase: Season::Winter,
                },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let events = app.world().resource::<Events<SeasonChangedEvent>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).copied().collect::<Vec<_>>();
        assert_eq!(
            collected,
            vec![SeasonChangedEvent {
                from: Season::Summer,
                to: Season::Winter,
                tick: 0,
            }]
        );
    }
}
