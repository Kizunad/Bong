use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard};

use bong_server::cmd::dev;
use bong_server::cmd::dev::qi::{QiCmd, QiCmd as RepresentativeDevCommand};
use valence::command::handler::CommandResultEvent;
use valence::command::manager::CommandPlugin;
use valence::command::scopes::CommandScopes;
use valence::command::CommandExecutionEvent;
use valence::prelude::{App, Client, Entity, EventLoopPreUpdate, EventLoopUpdate, Events};
use valence::protocol::packets::play::GameMessageS2c;
use valence::testing::{create_mock_client, MockClientHelper};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

struct ScopedEnvVar {
    key: &'static str,
    previous: Option<OsString>,
    _lock: MutexGuard<'static, ()>,
}

impl ScopedEnvVar {
    fn set(key: &'static str, value: Option<&str>) -> Self {
        let lock = ENV_MUTEX
            .lock()
            .expect("environment mutex should not be poisoned");
        let previous = std::env::var_os(key);
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
        Self {
            key,
            previous,
            _lock: lock,
        }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}
fn setup_app() -> App {
    let mut app = App::new();
    app.add_event::<valence::event_loop::PacketEvent>();
    app.add_plugins(CommandPlugin);
    dev::register(&mut app);
    app.finish();
    app.cleanup();
    app.world_mut().run_schedule(valence::prelude::PostStartup);
    app
}

fn spawn_client(app: &mut App, username: &str) -> (Entity, MockClientHelper) {
    let (client_bundle, helper) = create_mock_client(username);
    let player = app
        .world_mut()
        .spawn((client_bundle, CommandScopes::new()))
        .id();
    (player, helper)
}

fn execute(app: &mut App, executor: Entity, command: &str) {
    app.world_mut()
        .resource_mut::<Events<CommandExecutionEvent>>()
        .send(CommandExecutionEvent {
            command: command.to_string(),
            executor,
        });
    app.world_mut().run_schedule(EventLoopPreUpdate);
    app.world_mut().run_schedule(EventLoopUpdate);
}

fn chat_messages(app: &mut App, helper: &mut MockClientHelper) -> Vec<String> {
    let world = app.world_mut();
    let mut clients = world.query::<&mut Client>();
    for mut client in clients.iter_mut(world) {
        client.flush_packets().unwrap();
    }
    helper
        .collect_received()
        .0
        .into_iter()
        .filter_map(|frame| {
            frame
                .decode::<GameMessageS2c>()
                .ok()
                .map(|packet| packet.chat.to_legacy_lossy())
        })
        .collect()
}

#[test]
fn configured_operator_from_env_can_use_dev_command() {
    let _env = ScopedEnvVar::set("BONG_OPERATORS", Some("Builder, Another"));
    let mut app = setup_app();
    let (operator, _operator_helper) = spawn_client(&mut app, "Builder");
    app.world_mut().run_schedule(EventLoopPreUpdate);

    execute(&mut app, operator, "qi set 40");
    let events = app
        .world()
        .resource::<Events<CommandResultEvent<RepresentativeDevCommand>>>();
    let mut reader = events.get_reader();
    let results = reader.read(events).collect::<Vec<_>>();
    assert_eq!(
        results.len(),
        1,
        "BONG_OPERATORS must grant configured usernames operator access"
    );
    assert_eq!(results[0].result, QiCmd::Set { value: 40.0 });
}

#[test]
fn representative_dev_command_rejects_non_operator_and_allows_operator() {
    let _env = ScopedEnvVar::set("BONG_OPERATORS", None);
    let mut app = setup_app();
    let (player, mut player_helper) = spawn_client(&mut app, "Alice");
    let (operator, _operator_helper) = spawn_client(&mut app, "Admin");
    app.world_mut().run_schedule(EventLoopPreUpdate);

    execute(&mut app, player, "qi set 40");
    assert!(
        app.world()
            .resource::<Events<CommandResultEvent<RepresentativeDevCommand>>>()
            .is_empty(),
        "non-operator command must not reach its dev handler"
    );
    assert!(
        chat_messages(&mut app, &mut player_helper)
            .iter()
            .any(|message| message.contains("Command requires operator permission")),
        "non-operator should receive a clear permission rejection"
    );

    execute(&mut app, operator, "qi set 40");
    let events = app
        .world()
        .resource::<Events<CommandResultEvent<RepresentativeDevCommand>>>();
    let mut reader = events.get_reader();
    let results = reader.read(events).collect::<Vec<_>>();
    assert_eq!(
        results.len(),
        1,
        "operator command should reach its dev handler"
    );
    assert_eq!(results[0].result, QiCmd::Set { value: 40.0 });
}
