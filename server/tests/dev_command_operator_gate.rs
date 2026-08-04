use std::collections::BTreeSet;
use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard};

use bong_server::cmd::dev;
use bong_server::cmd::dev::qi::{QiCmd, QiCmd as RepresentativeDevCommand};
use bong_server::cmd::dev::season::{self, SeasonCmd, SeasonCmd as SeasonDevCommand};
use bong_server::cmd::gameplay::war::FactionCmd;
use bong_server::cmd::gameplay::BongCmd;
use bong_server::cmd::ping::PingCmd;
use bong_server::cmd::registry_pin;
use bong_server::identity::command::IdentityCmd;
use bong_server::world::season::{Season, WorldSeasonState};
use valence::command::handler::CommandResultEvent;
use valence::command::manager::CommandPlugin;
use valence::command::scopes::{CommandScopeRegistry, CommandScopes};
use valence::command::{AddCommand, CommandExecutionEvent, CommandRegistry};
use valence::prelude::{
    App, Client, ConnectionMode, Entity, EventLoopPreUpdate, EventLoopUpdate, Events,
    NetworkSettings, Schedule,
};
use valence::protocol::packets::play::command_tree_s2c::NodeData;
use valence::protocol::packets::play::GameMessageS2c;
use valence::testing::{create_mock_client, MockClientHelper};

static ENV_MUTEX: Mutex<()> = Mutex::new(());
const DEV_COMMAND_SCOPE: &str = "bong.dev";

struct ScopedEnvVars {
    previous: Vec<(&'static str, Option<OsString>)>,
    _lock: MutexGuard<'static, ()>,
}

impl ScopedEnvVars {
    fn set(vars: &[(&'static str, Option<&str>)]) -> Self {
        let lock = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = vars
            .iter()
            .map(|(key, value)| {
                let previous = std::env::var_os(key);
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
                (*key, previous)
            })
            .collect();
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for ScopedEnvVars {
    fn drop(&mut self) {
        for (key, previous) in self.previous.drain(..).rev() {
            if let Some(previous) = previous {
                std::env::set_var(key, previous);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

fn setup_app(connection_mode: ConnectionMode) -> App {
    let mut app = App::new();
    app.insert_resource(NetworkSettings {
        connection_mode,
        ..Default::default()
    });
    app.add_event::<valence::event_loop::PacketEvent>();
    app.add_plugins(CommandPlugin);
    dev::register(&mut app);
    app.add_command::<PingCmd>();
    app.add_command::<BongCmd>();
    app.add_command::<FactionCmd>();
    app.add_command::<IdentityCmd>();
    app.finish();
    app.cleanup();
    app.world_mut().run_schedule(valence::prelude::PostStartup);
    app
}

fn online_mode() -> ConnectionMode {
    ConnectionMode::Online {
        prevent_proxy_connections: true,
    }
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

fn qi_command_is_accepted(
    connection_mode: ConnectionMode,
    operators: Option<&str>,
    allow_offline: Option<&str>,
    username: &str,
) -> bool {
    let _env = ScopedEnvVars::set(&[
        ("BONG_DEV_MODE", Some("1")),
        ("BONG_OPERATORS", operators),
        ("BONG_OPERATORS_ALLOW_OFFLINE", allow_offline),
    ]);
    let mut app = setup_app(connection_mode);
    let (player, _helper) = spawn_client(&mut app, username);
    execute(&mut app, player, "qi set 40");
    !app.world()
        .resource::<Events<CommandResultEvent<RepresentativeDevCommand>>>()
        .is_empty()
}

fn operator_is_allowed(operators: Option<&str>, username: &str) -> bool {
    qi_command_is_accepted(online_mode(), operators, None, username)
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

fn root_scopes(app: &App) -> Vec<(String, Vec<String>)> {
    let registry = app.world().resource::<CommandRegistry>();
    let mut roots = registry
        .graph
        .graph
        .neighbors(registry.graph.root)
        .filter_map(|node| match &registry.graph.graph[node].data {
            NodeData::Literal { name } => {
                Some((name.clone(), registry.graph.graph[node].scopes.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    roots.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    roots
}

fn root_is_reachable(
    required_scopes: &[String],
    command_scopes: &CommandScopes,
    scope_registry: &CommandScopeRegistry,
) -> bool {
    required_scopes.is_empty()
        || required_scopes.iter().any(|required_scope| {
            scope_registry.any_grants(
                &command_scopes.iter().map(String::as_str).collect(),
                required_scope,
            )
        })
}

#[test]
fn operators_env_parsing_covers_defaults_empty_lists_and_identity_rules() {
    assert!(
        operator_is_allowed(None, "Admin"),
        "absent BONG_OPERATORS must retain the Admin default"
    );
    assert!(
        operator_is_allowed(None, "admin"),
        "absent BONG_OPERATORS must retain the lowercase admin default"
    );
    assert!(
        !operator_is_allowed(None, "Builder"),
        "absent BONG_OPERATORS must not grant an unrelated username"
    );
    assert!(
        !operator_is_allowed(Some(""), "Admin"),
        "an explicitly empty BONG_OPERATORS must not fall back to Admin"
    );
    assert!(
        !operator_is_allowed(Some("  , \t"), "Admin"),
        "whitespace-only operator entries must be filtered without restoring defaults"
    );
    assert!(
        operator_is_allowed(Some("Builder"), "Builder"),
        "a single configured operator must be accepted"
    );
    assert!(
        operator_is_allowed(Some("Builder, Another"), "Another"),
        "every comma-separated operator must be accepted"
    );
    assert!(
        operator_is_allowed(Some("  Builder  ,  Another  "), "Builder"),
        "surrounding whitespace must be trimmed from the first operator"
    );
    assert!(
        operator_is_allowed(Some("  Builder  ,  Another  "), "Another"),
        "surrounding whitespace must be trimmed from later operators"
    );
    assert!(
        operator_is_allowed(Some("Builder, Builder, Another"), "Builder"),
        "duplicate operator entries must preserve the configured operator"
    );
    assert!(
        operator_is_allowed(Some("Builder, Builder, Another"), "Another"),
        "duplicate entries must not discard a distinct later operator"
    );
    assert!(
        !operator_is_allowed(Some("Builder"), "builder"),
        "operator matching must remain case-sensitive"
    );
}

#[test]
fn offline_opt_in_flag_controls_operator_authentication_for_online_and_offline_clients() {
    assert!(
        qi_command_is_accepted(online_mode(), Some("Builder"), None, "Builder"),
        "online operators must be accepted when the offline opt-in is unset"
    );
    assert!(
        qi_command_is_accepted(online_mode(), Some("Builder"), Some("false"), "Builder"),
        "online operators must be accepted when the offline opt-in is false"
    );
    assert!(
        !qi_command_is_accepted(ConnectionMode::Offline, Some("Builder"), None, "Builder"),
        "offline usernames must be rejected when the offline opt-in is unset"
    );
    assert!(
        !qi_command_is_accepted(
            ConnectionMode::Offline,
            Some("Builder"),
            Some("false"),
            "Builder"
        ),
        "offline usernames must be rejected for a false opt-in value"
    );
    assert!(
        qi_command_is_accepted(
            ConnectionMode::Offline,
            Some("Builder"),
            Some("1"),
            "Builder"
        ),
        "offline usernames must be accepted for the explicit 1 opt-in"
    );
    assert!(
        qi_command_is_accepted(
            ConnectionMode::Offline,
            Some("Builder"),
            Some(" true "),
            "Builder"
        ),
        "offline usernames must accept case-insensitive trimmed true"
    );
    assert!(
        qi_command_is_accepted(
            ConnectionMode::Offline,
            Some("Builder"),
            Some("YES"),
            "Builder"
        ),
        "offline usernames must accept case-insensitive yes"
    );
}

#[test]
fn configured_operator_from_env_can_use_dev_command_in_online_mode() {
    let _env = ScopedEnvVars::set(&[
        ("BONG_DEV_MODE", Some("1")),
        ("BONG_OPERATORS", Some("Builder, Another")),
        ("BONG_OPERATORS_ALLOW_OFFLINE", None),
    ]);
    let mut app = setup_app(online_mode());
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
        "authenticated BONG_OPERATORS username must receive operator access"
    );
    assert_eq!(results[0].result, QiCmd::Set { value: 40.0 });
}

#[test]
fn offline_mode_denies_username_operator_by_default() {
    let _env = ScopedEnvVars::set(&[
        ("BONG_DEV_MODE", Some("1")),
        ("BONG_OPERATORS", Some("Builder")),
        ("BONG_OPERATORS_ALLOW_OFFLINE", None),
    ]);
    let mut app = setup_app(ConnectionMode::Offline);
    let (spoofed_operator, mut helper) = spawn_client(&mut app, "Builder");

    execute(&mut app, spoofed_operator, "qi set 40");

    assert!(
        app.world()
            .resource::<Events<CommandResultEvent<RepresentativeDevCommand>>>()
            .is_empty(),
        "offline username must not grant operator authority without explicit opt-in"
    );
    assert!(
        chat_messages(&mut app, &mut helper)
            .iter()
            .any(|message| message.contains("Command requires operator permission")),
        "spoofed offline operator should receive a permission rejection"
    );
}

#[test]
fn offline_mode_allows_username_operator_only_with_explicit_opt_in() {
    let _env = ScopedEnvVars::set(&[
        ("BONG_DEV_MODE", Some("1")),
        ("BONG_OPERATORS", Some("Builder")),
        ("BONG_OPERATORS_ALLOW_OFFLINE", Some("1")),
    ]);
    let mut app = setup_app(ConnectionMode::Offline);
    let (operator, _helper) = spawn_client(&mut app, "Builder");

    execute(&mut app, operator, "qi set 40");

    let events = app
        .world()
        .resource::<Events<CommandResultEvent<RepresentativeDevCommand>>>();
    let mut reader = events.get_reader();
    assert_eq!(
        reader.read(events).count(),
        1,
        "BONG_OPERATORS_ALLOW_OFFLINE=1 must explicitly enable offline username operators"
    );
}

#[test]
fn every_public_root_remains_reachable_to_non_operators() {
    let _env = ScopedEnvVars::set(&[
        ("BONG_DEV_MODE", Some("1")),
        ("BONG_OPERATORS", None),
        ("BONG_OPERATORS_ALLOW_OFFLINE", None),
    ]);
    let mut app = setup_app(online_mode());
    let (_player, _helper) = spawn_client(&mut app, "Alice");
    app.world_mut().run_schedule(EventLoopPreUpdate);

    let roots = root_scopes(&app);
    let actual_roots = roots
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>();
    let mut expected_roots = registry_pin::COMMAND_NAMES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    expected_roots.insert("identity");
    assert_eq!(
        actual_roots, expected_roots,
        "gate test must cover every root-level literal registered in production"
    );

    let public_roots = roots
        .iter()
        .filter(|(_, scopes)| scopes.is_empty())
        .map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        public_roots,
        dev::PUBLIC_COMMAND_ROOTS.iter().copied().collect(),
        "the complete public root set must remain reachable without operator scopes"
    );
}

#[test]
fn every_dev_root_blocks_non_operators_and_allows_operators() {
    let _env = ScopedEnvVars::set(&[
        ("BONG_DEV_MODE", Some("1")),
        ("BONG_OPERATORS", Some("Builder")),
        ("BONG_OPERATORS_ALLOW_OFFLINE", None),
    ]);
    let mut app = setup_app(online_mode());
    let (player, _player_helper) = spawn_client(&mut app, "Alice");
    let (operator, _operator_helper) = spawn_client(&mut app, "Builder");
    app.world_mut().run_schedule(EventLoopPreUpdate);

    let player_scopes = app.world().get::<CommandScopes>(player).unwrap();
    let operator_scopes = app.world().get::<CommandScopes>(operator).unwrap();
    assert!(!player_scopes.contains(DEV_COMMAND_SCOPE));
    assert!(operator_scopes.contains(DEV_COMMAND_SCOPE));

    let scope_registry = app.world().resource::<CommandScopeRegistry>();
    for (root, required_scopes) in root_scopes(&app) {
        if dev::PUBLIC_COMMAND_ROOTS.contains(&root.as_str()) {
            continue;
        }
        assert_eq!(
            required_scopes,
            [DEV_COMMAND_SCOPE],
            "/{root} must require the shared dev scope"
        );
        assert!(
            !root_is_reachable(&required_scopes, player_scopes, scope_registry),
            "non-operator must be blocked from /{root}"
        );
        assert!(
            root_is_reachable(&required_scopes, operator_scopes, scope_registry),
            "operator must be allowed to use /{root}"
        );
    }
}

#[test]
fn configured_operator_can_use_season_through_shared_permission_source() {
    let _env = ScopedEnvVars::set(&[
        ("BONG_DEV_MODE", Some("1")),
        ("BONG_OPERATORS", Some("Builder")),
        ("BONG_OPERATORS_ALLOW_OFFLINE", None),
    ]);
    let mut app = setup_app(online_mode());
    let (operator, _helper) = spawn_client(&mut app, "Builder");

    execute(&mut app, operator, "season set winter");
    let events = app
        .world()
        .resource::<Events<CommandResultEvent<SeasonDevCommand>>>();
    let mut reader = events.get_reader();
    assert_eq!(
        reader
            .read(events)
            .map(|event| event.result)
            .collect::<Vec<_>>(),
        [SeasonCmd::Set {
            phase: Season::Winter
        }],
        "configured operator must pass the shared command-tree permission gate for /season"
    );

    let mut schedule = Schedule::default();
    schedule.add_systems(season::handle_season);
    schedule.run(app.world_mut());
    assert_eq!(
        app.world().resource::<WorldSeasonState>().current.season,
        Season::Winter,
        "/season must use the same DevCommandPermissions resource as every other dev root"
    );
}
