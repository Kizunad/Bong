use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, EventWriter, Query, Res, Update};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::death_hooks::{
    CultivationDeathCause, CultivationDeathTrigger, PlayerTerminated,
};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::tick::CultivationClock;
use crate::persistence::{persist_termination_transition_with_death_context, PersistenceSettings};

const DEV_COMMAND_TERMINATION_CAUSE: &str = "dev_command";
const DEV_COMMAND_DEATH_REGISTRY_CAUSE: &str = "cultivation:DevCommand";

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
    settings: Option<Res<PersistenceSettings>>,
    mut terminated: EventWriter<PlayerTerminated>,
    mut cultivation_deaths: EventWriter<CultivationDeathTrigger>,
    mut players: Query<(&mut Lifecycle, Option<&mut LifeRecord>, &mut Client)>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for event in events.read() {
        let Ok((mut lifecycle, life_record, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        if lifecycle.state != LifecycleState::Alive {
            client.send_chat_message(format!(
                "[dev] kill self ignored: lifecycle={:?}",
                lifecycle.state
            ));
            continue;
        }

        let mut staged_lifecycle = lifecycle.clone();
        staged_lifecycle.terminate(tick);
        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::Terminated {
                cause: DEV_COMMAND_TERMINATION_CAUSE.to_string(),
                tick,
            });
            if let Some(settings) = settings.as_deref() {
                if let Err(error) = persist_termination_transition_with_death_context(
                    settings,
                    &staged_lifecycle,
                    &life_record,
                    Some(DEV_COMMAND_DEATH_REGISTRY_CAUSE),
                    None,
                ) {
                    let _ = life_record.biography.pop();
                    tracing::warn!(
                        "[dev-cmd] kill self rejected: failed to persist dev termination: {error}"
                    );
                    client.send_chat_message(
                        "[dev] kill self rejected: failed to persist termination",
                    );
                    continue;
                }
            } else {
                tracing::warn!(
                    "[dev-cmd] kill self has no PersistenceSettings; termination remains memory-only"
                );
            }
        }
        *lifecycle = staged_lifecycle;
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
    use crate::persistence::bootstrap_sqlite;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::Events;

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bong-dev-kill-{test_name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let root = unique_temp_dir(test_name);
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        let run_id = format!("dev-kill-{test_name}");
        bootstrap_sqlite(&db_path, &run_id).expect("sqlite bootstrap should succeed");
        (
            PersistenceSettings::with_paths(&db_path, &deceased_dir, run_id),
            root,
        )
    }

    fn setup_app(test_name: &str) -> (App, PathBuf) {
        let (settings, root) = persistence_settings(test_name);
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 77 });
        app.insert_resource(settings);
        app.add_event::<CommandResultEvent<KillCmd>>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, handle_kill);
        (app, root)
    }

    fn spawn_player(app: &mut App, lifecycle: Lifecycle) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .entity_mut(player)
            .insert((lifecycle, LifeRecord::new("offline:Alice")));
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
        let (mut app, root) = setup_app("terminates");
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
        assert_eq!(collected[0].context["source"], "dev_command");
        assert_eq!(collected[0].context["command"], "kill self");
        assert_eq!(collected[0].context["tick"], 77);
        assert!(matches!(
            app.world()
                .get::<LifeRecord>(player)
                .unwrap()
                .biography
                .last(),
            Some(BiographyEntry::Terminated { cause, tick })
                if cause == DEV_COMMAND_TERMINATION_CAUSE && *tick == 77
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn kill_self_is_noop_when_player_is_already_dead() {
        let (mut app, root) = setup_app("already-dead");
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
        assert!(app
            .world()
            .get::<LifeRecord>(player)
            .unwrap()
            .biography
            .is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
