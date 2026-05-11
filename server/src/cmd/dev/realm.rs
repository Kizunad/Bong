use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Update};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::cultivation::components::{Cultivation, Realm};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealmArg(pub Realm);

impl CommandArg for RealmArg {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        parse_realm(raw.as_str())
            .map(Self)
            .ok_or_else(|| CommandArgParseError::InvalidArgument {
                expected: "awaken|induce|condense|solidify|spirit|void".to_string(),
                got: raw,
            })
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealmCmd {
    Set { id: Realm },
}

impl Command for RealmCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("realm")
            .literal("set")
            .argument("id")
            .with_parser::<RealmArg>()
            .with_executable(|input| RealmCmd::Set {
                id: RealmArg::parse_arg(input)
                    .expect("brigadier should pre-validate realm id")
                    .0,
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
        let RealmCmd::Set { id } = event.result;
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
                result: RealmCmd::Set { id: Realm::Void },
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
