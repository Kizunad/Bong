//! plan-coffin-tiers-v1 P4 — `/coffin grade <grade>` dev 命令。
//!
//! dev-only：显式绕过 worldview 正典修炼规则（进棺门控、材料配方等），
//! 只做档级直写，不允许从生产 gameplay 路径复用。
//!
//! 用法：`/coffin grade <mundane|jade|stone|bronze>`
//! 效果：若执行者当前在棺内（持有 `CoffinComponent`），直写其档级；
//!       同步更新 `CoffinRegistry` 的档级记录。
//!       若不在棺内，报错提示。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, BlockPos, Client, Entity, EventReader, Query, ResMut, Update};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::coffin::{CoffinComponent, CoffinGrade, CoffinRegistry};

/// 档级参数解析器。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoffinGradeArg(pub CoffinGrade);

impl CommandArg for CoffinGradeArg {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        parse_coffin_grade(&raw)
            .map(Self)
            .ok_or_else(|| CommandArgParseError::InvalidArgument {
                expected: "mundane|jade|stone|bronze".to_string(),
                got: raw,
            })
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoffinCmd {
    Grade { grade: CoffinGrade },
}

impl Command for CoffinCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("coffin")
            .literal("grade")
            .argument("grade")
            .with_parser::<CoffinGradeArg>()
            .with_executable(|input| CoffinCmd::Grade {
                grade: CoffinGradeArg::parse_arg(input)
                    .expect("brigadier pre-validates grade arg")
                    .0,
            });
    }
}

pub fn register(app: &mut App) {
    // 若 CoffinRegistry 尚未由 coffin::register 注入（单元测试环境），在此兜底 init。
    // 生产路径由 coffin::register(&mut app) 优先 insert_resource。
    app.init_resource::<CoffinRegistry>();
    app.add_command::<CoffinCmd>()
        .add_systems(Update, handle_coffin);
}

pub fn handle_coffin(
    mut events: EventReader<CommandResultEvent<CoffinCmd>>,
    mut in_coffin: Query<(Entity, &mut CoffinComponent, &mut Client)>,
    mut no_coffin: Query<&mut Client, valence::prelude::Without<CoffinComponent>>,
    mut coffin_registry: ResMut<CoffinRegistry>,
) {
    for event in events.read() {
        let CoffinCmd::Grade { grade } = event.result;

        if let Ok((entity, mut coffin_comp, mut client)) = in_coffin.get_mut(event.executor) {
            let prev = coffin_comp.grade;
            coffin_comp.grade = grade;

            // 同步 CoffinRegistry 档级记录（via pub set_grade）
            let lower: BlockPos = coffin_comp.coffin_lower;
            let synced = coffin_registry.set_grade(lower, grade);

            tracing::warn!(
                "[dev-cmd] coffin grade bypass: entity={entity:?} lower={lower:?} {prev:?} -> {grade:?}"
            );
            if synced {
                client.send_chat_message(format!(
                    "[dev] coffin grade {prev:?} -> {grade:?} (lower={lower:?})"
                ));
            } else {
                tracing::warn!(
                    "[dev-cmd] coffin grade registry desync: entity={entity:?} lower={lower:?} not in registry; component updated but registry unchanged"
                );
                client.send_chat_message(format!(
                    "[dev] coffin grade {prev:?} -> {grade:?} written to component only — registry desync at lower={lower:?} (no registry entry)"
                ));
            }
        } else if let Ok(mut client) = no_coffin.get_mut(event.executor) {
            client.send_chat_message("[dev] coffin grade: not in a coffin (no CoffinComponent)");
        }
    }
}

/// 解析档级字符串（英文 snake_case）。
pub fn parse_coffin_grade(raw: &str) -> Option<CoffinGrade> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "mundane" | "凡木" => Some(CoffinGrade::Mundane),
        "jade" | "寒玉" => Some(CoffinGrade::Jade),
        "stone" | "玄石" => Some(CoffinGrade::Stone),
        "bronze" | "青铜" => Some(CoffinGrade::Bronze),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::coffin::CoffinComponent;
    use valence::prelude::{BlockPos, Entity, Events, Position};
    use valence::protocol::packets::play::GameMessageS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<CoffinCmd>>();
        app.insert_resource(CoffinRegistry::default());
        app.add_systems(Update, handle_coffin);
        app
    }

    /// Spawns a mock client and retains the helper for packet inspection.
    fn spawn_with_helper(app: &mut App, username: &str) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        (entity, helper)
    }

    /// Flushes all pending outbound packets for every Client in the world so that
    /// MockClientHelper.collect_received() can see them.
    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    /// Drains outgoing packets and returns the text bodies of all GameMessageS2c frames.
    fn collect_chat_texts(helper: &mut MockClientHelper) -> Vec<String> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let pkt = frame.decode::<GameMessageS2c>().ok()?;
                Some(pkt.chat.to_string())
            })
            .collect()
    }

    fn spawn_player_in_coffin(app: &mut App, grade: CoffinGrade, lower: BlockPos) -> Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut().entity_mut(player).insert(CoffinComponent {
            entered_at_tick: 0,
            coffin_lower: lower,
            grade,
        });
        player
    }

    fn send_grade(app: &mut App, player: Entity, grade: CoffinGrade) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<CoffinCmd>>>()
            .send(CommandResultEvent {
                result: CoffinCmd::Grade { grade },
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn parse_grade_accepts_all_variants() {
        for (raw, expected) in [
            ("mundane", CoffinGrade::Mundane),
            ("jade", CoffinGrade::Jade),
            ("stone", CoffinGrade::Stone),
            ("bronze", CoffinGrade::Bronze),
            ("凡木", CoffinGrade::Mundane),
            ("寒玉", CoffinGrade::Jade),
            ("玄石", CoffinGrade::Stone),
            ("青铜", CoffinGrade::Bronze),
            ("BRONZE", CoffinGrade::Bronze),
            ("  Jade  ", CoffinGrade::Jade),
        ] {
            assert_eq!(
                parse_coffin_grade(raw),
                Some(expected),
                "parse_coffin_grade({raw:?}) should yield {expected:?}"
            );
        }
        assert_eq!(parse_coffin_grade("unknown"), None);
        assert_eq!(parse_coffin_grade("   "), None);
    }

    #[test]
    fn grade_cmd_mutates_coffin_component() {
        let mut app = setup_app();
        let lower = BlockPos::new(0, 64, 0);
        let player = spawn_player_in_coffin(&mut app, CoffinGrade::Mundane, lower);

        send_grade(&mut app, player, CoffinGrade::Jade);
        run_update(&mut app);

        assert_eq!(
            app.world().get::<CoffinComponent>(player).unwrap().grade,
            CoffinGrade::Jade,
            "CoffinComponent.grade should be updated to Jade by /coffin grade"
        );
    }

    #[test]
    fn grade_cmd_cycles_through_all_variants() {
        let mut app = setup_app();
        let lower = BlockPos::new(5, 64, 5);
        let player = spawn_player_in_coffin(&mut app, CoffinGrade::Mundane, lower);

        for (target, expected_label) in [
            (CoffinGrade::Jade, "jade"),
            (CoffinGrade::Stone, "stone"),
            (CoffinGrade::Bronze, "bronze"),
            (CoffinGrade::Mundane, "mundane"),
        ] {
            send_grade(&mut app, player, target);
            run_update(&mut app);
            let actual = app.world().get::<CoffinComponent>(player).unwrap().grade;
            assert_eq!(
                actual, target,
                "/coffin grade {expected_label} should write grade={target:?}"
            );
        }
    }

    /// Registry-sync path: after `/coffin grade jade`, the registry entry at `lower` must also
    /// reflect jade — not just the CoffinComponent.
    #[test]
    fn grade_cmd_syncs_registry_grade() {
        let mut app = setup_app();
        let lower = BlockPos::new(10, 64, 10);
        // Pre-insert a registry entry so set_grade can find it.
        app.world_mut()
            .resource_mut::<CoffinRegistry>()
            .insert(lower, 0, CoffinGrade::Mundane);
        let player = spawn_player_in_coffin(&mut app, CoffinGrade::Mundane, lower);

        send_grade(&mut app, player, CoffinGrade::Jade);
        run_update(&mut app);

        // Component grade updated.
        assert_eq!(
            app.world().get::<CoffinComponent>(player).unwrap().grade,
            CoffinGrade::Jade,
            "CoffinComponent.grade should be Jade after /coffin grade jade"
        );
        // Registry grade also updated — not just the component.
        let registry_grade = app
            .world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .expect("registry entry should still exist at lower after grade change")
            .grade;
        assert_eq!(
            registry_grade,
            CoffinGrade::Jade,
            "CoffinRegistry.grade should be Jade after /coffin grade jade (registry desync check)"
        );
    }

    /// No-coffin branch: executor has no CoffinComponent → must receive a "not in a coffin" chat
    /// message and the registry must remain unmodified.
    #[test]
    fn grade_cmd_no_coffin_sends_error_chat() {
        let mut app = setup_app();
        // Pre-insert an unrelated registry entry to verify it is untouched.
        let unrelated = BlockPos::new(99, 64, 99);
        app.world_mut()
            .resource_mut::<CoffinRegistry>()
            .insert(unrelated, 0, CoffinGrade::Stone);

        let (player, mut helper) = spawn_with_helper(&mut app, "Bob");

        send_grade(&mut app, player, CoffinGrade::Jade);
        run_update(&mut app);
        flush_all_client_packets(&mut app);

        // No CoffinComponent should be added by the command.
        assert!(
            app.world().get::<CoffinComponent>(player).is_none(),
            "executor without CoffinComponent should not gain one from /coffin grade"
        );
        // Registry entry must be untouched.
        assert_eq!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(unrelated)
                .expect("unrelated registry entry should be untouched")
                .grade,
            CoffinGrade::Stone,
            "registry must not be modified when executor is not in a coffin"
        );
        // Chat must contain the "not in a coffin" error.
        let chats = collect_chat_texts(&mut helper);
        assert!(
            chats.iter().any(|msg| msg.contains("not in a coffin")),
            "expected 'not in a coffin' chat message for executor without CoffinComponent, got: {chats:?}"
        );
    }

    /// Desync branch: CoffinComponent says `lower` but registry has no matching entry →
    /// set_grade returns false → must send a desync warning chat (not the normal success message).
    /// Component grade IS still updated (by design).
    #[test]
    fn grade_cmd_registry_desync_sends_warning_chat() {
        let mut app = setup_app();
        let orphan_lower = BlockPos::new(20, 64, 20);
        // Intentionally do NOT insert a registry entry for orphan_lower.
        let (player, mut helper) = spawn_with_helper(&mut app, "Carol");
        // Manually attach a CoffinComponent pointing to the orphan lower pos.
        app.world_mut().entity_mut(player).insert(CoffinComponent {
            entered_at_tick: 0,
            coffin_lower: orphan_lower,
            grade: CoffinGrade::Mundane,
        });

        send_grade(&mut app, player, CoffinGrade::Bronze);
        run_update(&mut app);
        flush_all_client_packets(&mut app);

        // Component grade is still updated even under desync.
        assert_eq!(
            app.world().get::<CoffinComponent>(player).unwrap().grade,
            CoffinGrade::Bronze,
            "component grade should be written even when registry has no entry (desync scenario)"
        );
        // Registry must remain empty for orphan_lower.
        assert!(
            app.world()
                .resource::<CoffinRegistry>()
                .lookup(orphan_lower)
                .is_none(),
            "registry should remain empty for orphan_lower under desync"
        );
        // Chat must mention desync, not the normal success message.
        let chats = collect_chat_texts(&mut helper);
        assert!(
            chats.iter().any(|msg| msg.contains("desync")),
            "expected desync warning chat when registry has no entry for coffin_lower, got: {chats:?}"
        );
        assert!(
            !chats.iter().any(|msg| {
                msg.contains("-> Bronze") && !msg.contains("desync")
            }),
            "normal success message should NOT appear when registry desync occurred, got: {chats:?}"
        );
    }
}
