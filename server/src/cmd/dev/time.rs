use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, ResMut, Update};

use crate::cultivation::tick::CultivationClock;
use crate::npc::movement::GameTick;

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
        // F28 — `handle_time` now also syncs `GameTick` (npc/movement.rs); init here (not
        // just relying on `npc::movement::register` having already run) so any caller that
        // wires `cmd::register`/`dev::register` without the full NPC stack still gets a
        // valid resource instead of `handle_time` panicking on a missing `ResMut<GameTick>`.
        // `init_resource` is a no-op if `npc::movement::register` already inserted one.
        .init_resource::<GameTick>()
        .add_command::<TimeCmd>()
        .add_systems(Update, handle_time);
}

pub fn handle_time(
    mut events: EventReader<CommandResultEvent<TimeCmd>>,
    mut clock: ResMut<CultivationClock>,
    mut game_tick: ResMut<GameTick>,
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
        // F28 — `/time advance` 此前只推 CultivationClock，NPC AI 的冷却锚
        // （heiwushi/dormant/lod/navigator/relic/scattered_cultivator/skull_fiend 等）
        // 都读 GameTick（npc/movement.rs），不同步就会让"快进"后 NPC 节拍仍停在旧值，
        // dev 测试冷却校验与生产态脱节。MAX_ADVANCE_TICKS=1_000_000 < u32::MAX，
        // wrapping_add 在此值域内不会绕回，仅为与 `increment_game_tick` 保持同一累加语义。
        let game_tick_before = game_tick.0;
        game_tick.0 = game_tick.0.wrapping_add(ticks as u32);
        tracing::warn!(
            "[dev-cmd] advance cultivation clock by {ticks} ticks: {before} -> {}; \
             synced GameTick {game_tick_before} -> {}",
            clock.tick,
            game_tick.0
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
        app.insert_resource(GameTick(10));
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

    // ───────────────────────── F28 — GameTick 同步 ─────────────────────────

    #[test]
    fn time_advance_synchronizes_game_tick_by_the_same_delta() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, 100);
        run_update(&mut app);

        assert_eq!(
            app.world().resource::<CultivationClock>().tick,
            110,
            "sanity check: CultivationClock advances as before"
        );
        assert_eq!(
            app.world().resource::<GameTick>().0,
            110,
            "F28: GameTick must advance by the exact same delta as CultivationClock \
             (started at 10, advanced by 100 -> 110), otherwise NPC cooldown anchors \
             (heiwushi/dormant/lod/navigator/relic/...) stay stuck on the pre-advance tick"
        );
    }

    #[test]
    fn time_advance_zero_leaves_game_tick_unchanged() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, 0);
        run_update(&mut app);

        assert_eq!(
            app.world().resource::<GameTick>().0,
            10,
            "ticks=0 is a rejected no-op — GameTick must not move at all"
        );
    }

    #[test]
    fn time_advance_rejects_over_max_and_leaves_game_tick_unchanged() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, MAX_ADVANCE_TICKS + 1);
        run_update(&mut app);

        assert_eq!(
            app.world().resource::<CultivationClock>().tick,
            10,
            "sanity check: over-MAX request must still be rejected for CultivationClock too"
        );
        assert_eq!(
            app.world().resource::<GameTick>().0,
            10,
            "F28: an over-MAX_ADVANCE_TICKS request must be rejected before touching GameTick, \
             same as it already is for CultivationClock"
        );
    }

    #[test]
    fn time_advance_at_exactly_max_ticks_boundary_syncs_game_tick() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, MAX_ADVANCE_TICKS);
        run_update(&mut app);

        assert_eq!(
            app.world().resource::<CultivationClock>().tick,
            10 + MAX_ADVANCE_TICKS,
            "off-by-one boundary: ticks == MAX_ADVANCE_TICKS must be accepted, not rejected"
        );
        assert_eq!(
            app.world().resource::<GameTick>().0,
            10 + MAX_ADVANCE_TICKS as u32,
            "F28: the MAX_ADVANCE_TICKS boundary (accepted) must still sync GameTick — \
             MAX_ADVANCE_TICKS (1_000_000) is well under u32::MAX so no wrap should occur here"
        );
    }

    #[test]
    fn time_advance_wraps_game_tick_like_increment_game_tick_does() {
        let mut app = setup_app();
        app.insert_resource(GameTick(u32::MAX - 5));
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, 10);
        run_update(&mut app);

        assert_eq!(
            app.world().resource::<GameTick>().0,
            4,
            "F28: GameTick sync must use wrapping_add (same overflow semantics as the \
             production `increment_game_tick` system), not panic or saturate — \
             (u32::MAX - 5) + 10 wraps to 4"
        );
    }

    #[test]
    fn multiple_advances_accumulate_on_game_tick_across_events() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, 40);
        send(&mut app, player, 60);
        run_update(&mut app);

        assert_eq!(
            app.world().resource::<CultivationClock>().tick,
            110,
            "sanity check: both events applied to CultivationClock in the same tick"
        );
        assert_eq!(
            app.world().resource::<GameTick>().0,
            110,
            "F28: GameTick must accumulate across multiple events in the same tick exactly \
             like CultivationClock does (10 + 40 + 60 = 110)"
        );
    }
}
