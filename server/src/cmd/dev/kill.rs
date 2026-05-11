use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, EventWriter, Query, Res, Update};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::death_hooks::{
    CultivationDeathCause, CultivationDeathTrigger, PlayerTerminated,
};
use crate::cultivation::tick::CultivationClock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillCmd {
    Self_,
}

impl Command for KillCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("kill")
            .literal("self")
            .with_executable(|_| KillCmd::Self_);
    }
}

pub fn register(app: &mut App) {
    app.add_event::<PlayerTerminated>()
        .add_event::<CultivationDeathTrigger>()
        .add_command::<KillCmd>()
        .add_systems(Update, handle_kill);
}

pub fn handle_kill(
    mut events: EventReader<CommandResultEvent<KillCmd>>,
    clock: Option<Res<CultivationClock>>,
    mut terminated: EventWriter<PlayerTerminated>,
    mut cultivation_deaths: EventWriter<CultivationDeathTrigger>,
    mut players: Query<(&mut Lifecycle, &mut Client)>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for event in events.read() {
        let Ok((mut lifecycle, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        if lifecycle.state != LifecycleState::Alive {
            client.send_chat_message(format!(
                "[dev] kill self ignored: lifecycle={:?}",
                lifecycle.state
            ));
            continue;
        }

        lifecycle.terminate(tick);
        terminated.send(PlayerTerminated {
            entity: event.executor,
        });
        cultivation_deaths.send(CultivationDeathTrigger {
            entity: event.executor,
            cause: CultivationDeathCause::DevCommand,
            context: serde_json::json!({
                "source": "dev_command",
                "command": "kill self",
                "tick": tick,
            }),
        });
        tracing::warn!(
            "[dev-cmd] bypass combat damage: kill self with {:?}",
            CultivationDeathCause::DevCommand
        );
        client.send_chat_message("[dev] kill self queued PlayerTerminated");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 77 });
        app.add_event::<CommandResultEvent<KillCmd>>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, handle_kill);
        app
    }

    fn spawn_player(app: &mut App, lifecycle: Lifecycle) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert(lifecycle);
        player
    }

    fn send(app: &mut App, player: valence::prelude::Entity) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<KillCmd>>>()
            .send(CommandResultEvent {
                result: KillCmd::Self_,
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn kill_self_emits_player_terminated_and_marks_lifecycle_terminated() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, Lifecycle::default());

        send(&mut app, player);
        run_update(&mut app);

        assert_eq!(
            app.world().get::<Lifecycle>(player).unwrap().state,
            LifecycleState::Terminated
        );
        assert_eq!(
            app.world()
                .get::<Lifecycle>(player)
                .unwrap()
                .last_death_tick,
            Some(77)
        );
        assert_eq!(app.world().resource::<Events<PlayerTerminated>>().len(), 1);
        let deaths = app.world().resource::<Events<CultivationDeathTrigger>>();
        let collected = deaths
            .get_reader()
            .read(deaths)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].entity, player);
        assert_eq!(collected[0].cause, CultivationDeathCause::DevCommand);
        assert_eq!(collected[0].context["command"], "kill self");
    }

    #[test]
    fn kill_self_is_noop_when_player_is_already_dead() {
        let mut app = setup_app();
        let mut lifecycle = Lifecycle::default();
        lifecycle.terminate(10);
        let player = spawn_player(&mut app, lifecycle);

        send(&mut app, player);
        run_update(&mut app);

        assert_eq!(app.world().resource::<Events<PlayerTerminated>>().len(), 0);
        assert_eq!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .len(),
            0
        );
        assert_eq!(
            app.world()
                .get::<Lifecycle>(player)
                .unwrap()
                .last_death_tick,
            Some(10)
        );
    }
}
