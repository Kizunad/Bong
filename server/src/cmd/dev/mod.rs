// plan-bughunt-animal-air-spawn-gravity-v1 P2：确定性 ambient one-shot dev 命令
pub mod ambient_spawn;
// plan-bot-e2e-coverage-v1 P4：确定性真实 Plant 采集夹具
pub mod botany_spawn;
// plan-tribulation-balance-v1 P0：/balance tribulation dev 命令
pub mod balance;
pub mod baolongwang;
pub mod block_picker;
pub mod clearinv;
pub mod coffin;
pub mod fog;
pub mod gallery;
pub mod give;
pub mod gm;
pub mod health;
pub mod heiwushi;
pub mod kill;
pub mod meridian;
pub mod npc_scenario;
pub mod preview_tp;
pub mod qi;
pub mod race;
pub mod rat;
pub mod realm;
pub mod reset;
pub mod revive;
pub mod riskmap;
pub mod season;
pub mod shader_push;
pub mod shrine;
pub mod sparring;
pub mod spawn;
pub mod stamina;
pub mod supply_coffin;
pub mod technique;
pub mod time;
pub mod top;
pub mod tpdim;
pub mod tppoi;
pub mod tptree;
pub mod tpzone;
pub mod tribulation_debug;
pub mod tribulation_rechallenge;
pub mod tsy_spawn;
pub mod whale;
pub mod wound;
pub mod zone_qi;
pub mod zones;

use std::collections::HashSet;

use valence::command::scopes::CommandScopes;
use valence::command::{
    CommandExecutionEvent, CommandRegistry, CommandScopeRegistry, CommandSystemSet,
};
use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, Added, App, Client, ConnectionMode, EventLoopPreUpdate, EventLoopUpdate, EventReader,
    IntoSystemConfigs, Local, NetworkSettings, Query, Res, ResMut, Resource, Username,
};

const DEV_COMMAND_SCOPE: &str = "bong.dev";
pub const PUBLIC_COMMAND_ROOTS: &[&str] = &["bong", "faction", "identity", "ping"];

#[derive(Default, Resource)]
struct DevCommandRoots(HashSet<String>);

#[derive(Resource)]
pub struct DevCommandPermissions {
    allowed_usernames: HashSet<String>,
    usernames_are_authenticated: bool,
}

impl Default for DevCommandPermissions {
    fn default() -> Self {
        Self::from_connection_mode(None)
    }
}

impl DevCommandPermissions {
    fn from_connection_mode(connection_mode: Option<&ConnectionMode>) -> Self {
        let allow_offline = truthy_env_var("BONG_OPERATORS_ALLOW_OFFLINE");
        let usernames_are_authenticated =
            !matches!(connection_mode, Some(ConnectionMode::Offline)) || allow_offline;
        if matches!(connection_mode, Some(ConnectionMode::Offline)) && !allow_offline {
            tracing::warn!(
                "BONG_OPERATORS ignored in offline mode; set BONG_OPERATORS_ALLOW_OFFLINE=1 to explicitly trust client-provided usernames"
            );
        }
        let allowed_usernames = std::env::var("BONG_OPERATORS")
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|username| !username.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_else(|_| HashSet::from(["Admin".to_string(), "admin".to_string()]));
        Self {
            allowed_usernames,
            usernames_are_authenticated,
        }
    }

    #[cfg(test)]
    pub fn allow_user(username: impl Into<String>) -> Self {
        Self {
            allowed_usernames: HashSet::from([username.into()]),
            usernames_are_authenticated: true,
        }
    }

    pub fn is_operator(&self, username: &str) -> bool {
        self.usernames_are_authenticated && self.allowed_usernames.contains(username)
    }
}

fn truthy_env_var(key: &str) -> bool {
    std::env::var(key).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        )
    })
}

pub fn dev_mode_enabled() -> bool {
    truthy_env_var("BONG_DEV_MODE")
}

pub fn register(app: &mut App) {
    register_for_dev_mode(app, dev_mode_enabled());
}

pub(crate) fn register_for_dev_mode(app: &mut App, dev_mode_enabled: bool) {
    if dev_mode_enabled {
        ambient_spawn::register_enabled(app);
        botany_spawn::register_enabled(app);
    }
    balance::register(app);
    baolongwang::register(app);
    block_picker::register(app);
    gallery::register(app);
    coffin::register(app);
    clearinv::register(app);
    fog::register(app);
    give::register(app);
    heiwushi::register(app);
    spawn::register(app);
    top::register(app);
    zones::register(app);
    season::register(app);
    gm::register(app);
    health::register(app);
    kill::register(app);
    meridian::register(app);
    qi::register(app);
    race::register(app);
    realm::register(app);
    reset::register(app);
    revive::register(app);
    stamina::register(app);
    supply_coffin::register(app);
    technique::register(app);
    time::register(app);
    tptree::register(app);
    tpdim::register(app);
    tppoi::register(app);
    tpzone::register(app);
    shrine::register(app);
    sparring::register(app);
    wound::register(app);
    tsy_spawn::register(app);
    npc_scenario::register(app);
    preview_tp::register(app);
    rat::register(app);
    riskmap::register(app);
    whale::register(app);
    zone_qi::register(app);
    shader_push::register(app);
    tribulation_debug::register(app);
    tribulation_rechallenge::register(app);
    register_operator_gate(app);
}

fn register_operator_gate(app: &mut App) {
    let permissions = DevCommandPermissions::from_connection_mode(
        app.world()
            .get_resource::<NetworkSettings>()
            .map(|settings| &settings.connection_mode),
    );
    app.init_resource::<DevCommandRoots>()
        .insert_resource(permissions)
        .add_systems(
            EventLoopPreUpdate,
            (scope_dev_command_roots, sync_operator_scope)
                .chain()
                .before(CommandSystemSet),
        )
        .add_systems(EventLoopUpdate, gate_dev_commands);
}

fn scope_dev_command_roots(
    mut command_registry: ResMut<CommandRegistry>,
    mut scope_registry: ResMut<CommandScopeRegistry>,
    mut dev_roots: ResMut<DevCommandRoots>,
    mut initialized: Local<bool>,
) {
    if *initialized {
        return;
    }

    let root = command_registry.graph.root;
    let root_nodes = command_registry
        .graph
        .graph
        .neighbors(root)
        .collect::<Vec<_>>();
    for node in root_nodes {
        let valence::protocol::packets::play::command_tree_s2c::NodeData::Literal { name } =
            &command_registry.graph.graph[node].data
        else {
            continue;
        };
        if PUBLIC_COMMAND_ROOTS.contains(&name.as_str()) {
            continue;
        }
        dev_roots.0.insert(name.clone());
        command_registry.graph.graph[node].scopes = vec![DEV_COMMAND_SCOPE.to_string()];
    }
    scope_registry.add_scope(DEV_COMMAND_SCOPE);
    *initialized = true;
}

fn sync_operator_scope(
    permissions: Res<DevCommandPermissions>,
    mut clients: Query<(&Username, &mut CommandScopes), Added<CommandScopes>>,
) {
    for (username, mut scopes) in &mut clients {
        if permissions.is_operator(username.0.as_str()) {
            scopes.add(DEV_COMMAND_SCOPE);
        } else {
            scopes.remove(DEV_COMMAND_SCOPE);
        }
    }
}

fn gate_dev_commands(
    mut events: EventReader<CommandExecutionEvent>,
    dev_roots: Res<DevCommandRoots>,
    permissions: Res<DevCommandPermissions>,
    usernames: Query<&Username>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let command_root = event.command.split_whitespace().next().unwrap_or_default();
        if !dev_roots.0.contains(command_root)
            || usernames
                .get(event.executor)
                .is_ok_and(|username| permissions.is_operator(username.0.as_str()))
        {
            continue;
        }
        if let Ok(mut client) = clients.get_mut(event.executor) {
            client.send_chat_message("[dev] Command requires operator permission.");
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use valence::prelude::{App, Entity, Position};
    use valence::testing::create_mock_client;

    pub fn spawn_test_client(app: &mut App, username: &str, position: [f64; 3]) -> Entity {
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);
        app.world_mut().spawn(client_bundle).id()
    }

    pub fn run_update(app: &mut App) {
        app.update();
    }
}
