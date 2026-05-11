use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, ResMut, Update};

use crate::cultivation::tick::CultivationClock;

pub const MAX_ADVANCE_TICKS: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeCmd {
    Advance { ticks: u64 },
}

impl Command for TimeCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("time")
            .literal("advance")
            .argument("ticks")
            .with_parser::<u32>()
            .with_executable(|input| TimeCmd::Advance {
                ticks: u64::from(u32::parse_arg(input).unwrap()),
            });
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<CultivationClock>()
        .add_command::<TimeCmd>()
        .add_systems(Update, handle_time);
}

pub fn handle_time(
    mut events: EventReader<CommandResultEvent<TimeCmd>>,
    mut clock: ResMut<CultivationClock>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let TimeCmd::Advance { ticks } = event.result;
        let Ok(mut client) = clients.get_mut(event.executor) else {
            continue;
        };
        if ticks == 0 {
            client.send_chat_message("[dev] time advance 0: no-op");
            continue;
        }
        if ticks > MAX_ADVANCE_TICKS {
            client.send_chat_message(format!(
                "[dev] time advance rejected: ticks must be <= {MAX_ADVANCE_TICKS}"
            ));
            continue;
        }
        let before = clock.tick;
        clock.tick = clock.tick.saturating_add(ticks);
        tracing::warn!(
            "[dev-cmd] advance cultivation clock by {ticks} ticks: {before} -> {}",
            clock.tick
        );
        client.send_chat_message(format!(
            "[dev] time advanced {ticks} ticks: {before} -> {}",
            clock.tick
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 10 });
        app.add_event::<CommandResultEvent<TimeCmd>>();
        app.add_systems(Update, handle_time);
        app
    }

    fn send(app: &mut App, player: valence::prelude::Entity, ticks: u64) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TimeCmd>>>()
            .send(CommandResultEvent {
                result: TimeCmd::Advance { ticks },
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn time_advance_mutates_only_cultivation_clock() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, 100);
        run_update(&mut app);

        assert_eq!(app.world().resource::<CultivationClock>().tick, 110);
    }

    #[test]
    fn time_advance_zero_and_too_large_are_rejected() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, 0);
        send(&mut app, player, 2_000_000);
        run_update(&mut app);

        assert_eq!(app.world().resource::<CultivationClock>().tick, 10);
    }
}
