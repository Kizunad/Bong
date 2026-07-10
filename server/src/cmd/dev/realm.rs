use crate::cultivation::components::{Cultivation, Realm};
use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Update};

pub const ALLOWED_REALM_IDS: &str = "awaken|induce|condense|solidify|spirit|void";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealmCmd {
    Set { raw: String },
}

impl Command for RealmCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("realm")
            .literal("set")
            .argument("id")
            .with_parser::<String>()
            .with_executable(|input: &mut ParseInput| RealmCmd::Set {
                raw: String::parse_arg(input)
                    .expect("brigadier should pre-validate realm id as a string"),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<RealmCmd>()
        .add_systems(Update, handle_realm);
}

pub fn handle_realm(
    mut events: EventReader<CommandResultEvent<RealmCmd>>,
    mut players: Query<(&mut Cultivation, &mut Client)>,
) {
    for event in events.read() {
        let Ok((mut cultivation, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        let RealmCmd::Set { raw } = &event.result;
        let Some(id) = parse_realm(raw) else {
            client.send_chat_message(format!(
                "[dev] realm set rejected: unknown realm {raw:?}; allowed: {ALLOWED_REALM_IDS}"
            ));
            continue;
        };
        let prev = cultivation.realm;
        cultivation.realm = id;
        tracing::warn!("[dev-cmd] bypass breakthrough: realm {prev:?} -> {id:?}");
        client.send_chat_message(format!("[dev] realm set {prev:?} -> {id:?}"));
    }
}

pub fn parse_realm(raw: &str) -> Option<Realm> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "awaken" | "醒灵" => Some(Realm::Awaken),
        "induce" | "引气" => Some(Realm::Induce),
        "condense" | "凝脉" => Some(Realm::Condense),
        "solidify" | "固元" => Some(Realm::Solidify),
        "spirit" | "通灵" => Some(Realm::Spirit),
        "void" | "化虚" => Some(Realm::Void),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::cultivation::life_record::LifeRecord;
    use crate::qi_physics::QiTransfer;
    use valence::prelude::Events;
    use valence::protocol::packets::play::{CommandExecutionC2s, GameMessageS2c};
    use valence::protocol::{Bounded, FixedBitSet, VarInt};
    use valence::testing::{create_mock_client, MockClientHelper};

    fn setup_command_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            valence::event_loop::EventLoopPlugin,
            valence::command::manager::CommandPlugin,
        ));
        register(&mut app);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    fn execute_command(app: &mut App, helper: &mut MockClientHelper, command: &str) {
        helper.send(&CommandExecutionC2s {
            command: Bounded(command),
            timestamp: 0,
            salt: 0,
            argument_signatures: Vec::new(),
            message_count: VarInt(0),
            acknowledgement: FixedBitSet::default(),
        });
        app.update();
    }

    fn flush_and_collect_chat(app: &mut App, helper: &mut MockClientHelper) -> Vec<String> {
        let world = app.world_mut();
        let mut clients = world.query::<&mut Client>();
        for mut client in clients.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
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
    fn parse_realm_accepts_english_chinese_and_rejects_unknown() {
        for (raw, realm) in [
            ("awaken", Realm::Awaken),
            ("醒灵", Realm::Awaken),
            ("induce", Realm::Induce),
            ("引气", Realm::Induce),
            ("condense", Realm::Condense),
            ("凝脉", Realm::Condense),
            ("solidify", Realm::Solidify),
            ("固元", Realm::Solidify),
            ("spirit", Realm::Spirit),
            ("通灵", Realm::Spirit),
            ("void", Realm::Void),
            ("化虚", Realm::Void),
            ("  AwAkEn  ", Realm::Awaken),
            ("SPIRIT  ", Realm::Spirit),
            ("  化虚  ", Realm::Void),
        ] {
            assert_eq!(parse_realm(raw), Some(realm));
        }
        assert_eq!(parse_realm("immortal"), None);
        assert_eq!(parse_realm("   "), None);
    }

    #[test]
    fn command_integration_valid_realm_preserves_success_contract() {
        let mut app = setup_command_app();
        let (bundle, mut helper) = create_mock_client("Alice");
        let player = app.world_mut().spawn(bundle).id();
        app.world_mut()
            .entity_mut(player)
            .insert(Cultivation::default());

        execute_command(&mut app, &mut helper, "realm set induce");

        let chats = flush_and_collect_chat(&mut app, &mut helper);
        assert!(
            chats
                .iter()
                .any(|text| text == "[dev] realm set Awaken -> Induce"),
            "合法 realm 成功反馈契约不得变化，实际：{chats:?}"
        );
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().realm,
            Realm::Induce
        );
    }

    #[test]
    fn command_integration_invalid_realm_returns_player_visible_feedback() {
        let mut app = setup_command_app();
        let (bundle, mut helper) = create_mock_client("Alice");
        let player = app.world_mut().spawn(bundle).id();
        app.world_mut().entity_mut(player).insert(Cultivation {
            realm: Realm::Induce,
            ..Default::default()
        });

        execute_command(&mut app, &mut helper, "realm set bot_e2e_no_such_realm");

        let chats = flush_and_collect_chat(&mut app, &mut helper);
        assert!(
            chats.iter().any(|text| {
                text.contains("bot_e2e_no_such_realm")
                    && text.contains("awaken|induce|condense|solidify|spirit|void")
            }),
            "非法 realm id 必须返回包含原输入和允许值的玩家 chat，实际：{chats:?}"
        );
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().realm,
            Realm::Induce,
            "非法 realm id 不得修改境界"
        );
    }

    #[test]
    fn realm_set_mutates_realm_without_life_record_side_effect() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<RealmCmd>>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, handle_realm);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .entity_mut(player)
            .insert((Cultivation::default(), LifeRecord::default()));

        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<RealmCmd>>>()
            .send(CommandResultEvent {
                result: RealmCmd::Set {
                    raw: "void".to_string(),
                },
                executor: player,
                modifiers: Default::default(),
            });
        run_update(&mut app);

        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().realm,
            Realm::Void
        );
        assert!(
            app.world()
                .get::<LifeRecord>(player)
                .unwrap()
                .biography
                .is_empty(),
            "/realm set is dev-only state mutation, not a real breakthrough"
        );
        assert_eq!(
            app.world().resource::<Events<QiTransfer>>().len(),
            0,
            "/realm set directly mutates realm and must not enter qi_physics ledger"
        );
    }
}
