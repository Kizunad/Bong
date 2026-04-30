use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Position, Query, Update};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnCmd {
    Spawn,
}

impl Command for SpawnCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("spawn")
            .with_executable(|_| SpawnCmd::Spawn);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<SpawnCmd>()
        .add_systems(Update, handle_spawn);
}

pub fn handle_spawn(
    mut events: EventReader<CommandResultEvent<SpawnCmd>>,
    mut players: Query<(&mut Position, &mut Client)>,
) {
    for event in events.read() {
        let Ok((mut position, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        position.set(crate::player::spawn_position());
        client.send_chat_message("Teleported to spawn.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::{Events, Position};

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<SpawnCmd>>();
        app.add_systems(Update, handle_spawn);
        app
    }

    fn send(app: &mut App, executor: valence::prelude::Entity, result: SpawnCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<SpawnCmd>>>()
            .send(CommandResultEvent {
                result,
                executor,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn spawn_teleports_to_player_spawn() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [1.0, 2.0, 3.0]);
        send(&mut app, player, SpawnCmd::Spawn);

        run_update(&mut app);

        let position = app.world().get::<Position>(player).unwrap();
        assert_eq!(position.get().to_array(), crate::player::spawn_position());
    }

    #[test]
    fn spawn_is_noop_for_missing_executor() {
        let mut app = setup_app();
        send(
            &mut app,
            valence::prelude::Entity::PLACEHOLDER,
            SpawnCmd::Spawn,
        );

        run_update(&mut app);
    }

    #[test]
    fn spawn_ignores_executor_without_player_components() {
        let mut app = setup_app();
        let entity = app.world_mut().spawn_empty().id();
        send(&mut app, entity, SpawnCmd::Spawn);

        run_update(&mut app);

        assert!(
            app.world().get::<Position>(entity).is_none(),
            "entity without player components should not be mutated by /spawn"
        );
    }

    #[test]
    fn spawn_handles_multiple_executors_in_one_tick() {
        let mut app = setup_app();
        let alice = spawn_test_client(&mut app, "Alice", [1.0, 2.0, 3.0]);
        let bob = spawn_test_client(&mut app, "Bob", [-8.0, 90.0, 42.0]);
        send(&mut app, alice, SpawnCmd::Spawn);
        send(&mut app, bob, SpawnCmd::Spawn);

        run_update(&mut app);

        assert_eq!(
            app.world().get::<Position>(alice).unwrap().get().to_array(),
            crate::player::spawn_position()
        );
        assert_eq!(
            app.world().get::<Position>(bob).unwrap().get().to_array(),
            crate::player::spawn_position()
        );
    }
}
