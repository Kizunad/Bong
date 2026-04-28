//! `/preview_tp <x> <y> <z> <yaw> <pitch>` — worldgen-snapshot harness 用的
//! server-side authoritative teleport（plan-worldgen-snapshot-v1 §2.4）。
//!
//! client preview harness 在 SETUP_SHOT 阶段 `sendCommand("/preview_tp ...")`，
//! server 解析后 emit [`PreviewTeleportRequested`] event，
//! [`crate::preview::handle_preview_teleport`] system 消费 event 改写
//! Position + Look + HeadYaw。
//!
//! 仅在 `BONG_PREVIEW_MODE=1` env 下激活实际 teleport（preview module register
//! 守卫）；未激活时命令仍解析 + emit event，但 system 不接，相当于 no-op。
//!
//! 命令名用下划线 `preview_tp` 而非短横 `preview-tp`，与 main 上 `tsy_spawn` /
//! `npc_scenario` 风格一致（valence_command 不允许 `-` 在 literal 里）。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, EventWriter, Query, Update};

use crate::preview::PreviewTeleportRequested;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewTpCmd {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

impl Command for PreviewTpCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        use valence::command::parsers::{CommandArg, ParseInput};

        graph
            .root()
            .literal("preview_tp")
            .argument("x")
            .with_parser::<f64>()
            .argument("y")
            .with_parser::<f64>()
            .argument("z")
            .with_parser::<f64>()
            .argument("yaw")
            .with_parser::<f32>()
            .argument("pitch")
            .with_parser::<f32>()
            .with_executable(|input: &mut ParseInput| PreviewTpCmd {
                x: f64::parse_arg(input).unwrap(),
                y: f64::parse_arg(input).unwrap(),
                z: f64::parse_arg(input).unwrap(),
                yaw: f32::parse_arg(input).unwrap(),
                pitch: f32::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    // PreviewTeleportRequested 也由 preview::register 注册，但 cmd::tests 单独
    // 跑 cmd::register 时不会调 preview::register，所以这里幂等地再 add_event
    // 防 cmd 单测里 handle_preview_tp 找不到 Events resource。bevy add_event
    // 内部用 init_resource 不会重复 panic。
    app.add_event::<PreviewTeleportRequested>()
        .add_command::<PreviewTpCmd>()
        .add_systems(Update, handle_preview_tp);
}

pub fn handle_preview_tp(
    mut events: EventReader<CommandResultEvent<PreviewTpCmd>>,
    mut clients: Query<&mut Client>,
    mut preview_tp_tx: EventWriter<PreviewTeleportRequested>,
) {
    for event in events.read() {
        let cmd = event.result;
        // pitch 范围保护（MC 内部 clamp 到 -90~+90，但显式拒绝越界更直白）
        if !(-90.0..=90.0).contains(&cmd.pitch) {
            if let Ok(mut client) = clients.get_mut(event.executor) {
                client.send_chat_message(format!(
                    "/preview_tp pitch={} 越界（合法 -90~+90）",
                    cmd.pitch
                ));
            }
            continue;
        }
        preview_tp_tx.send(PreviewTeleportRequested {
            player: event.executor,
            pos: [cmd.x, cmd.y, cmd.z],
            yaw: cmd.yaw,
            pitch: cmd.pitch,
        });
        if let Ok(mut client) = clients.get_mut(event.executor) {
            client.send_chat_message(format!(
                "/preview_tp queued ({:.1}, {:.1}, {:.1}) yaw={} pitch={}",
                cmd.x, cmd.y, cmd.z, cmd.yaw, cmd.pitch
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::command::CommandRegistry;
    use valence::prelude::Events;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<PreviewTpCmd>>();
        app.add_event::<PreviewTeleportRequested>();
        app.add_systems(Update, handle_preview_tp);
        app
    }

    fn send(app: &mut App, executor: valence::prelude::Entity, result: PreviewTpCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<PreviewTpCmd>>>()
            .send(CommandResultEvent {
                result,
                executor,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn register_adds_preview_tp_literal_to_command_registry() {
        let mut app = App::new();
        app.add_plugins(valence::command::manager::CommandPlugin);
        register(&mut app);
        app.add_event::<PreviewTeleportRequested>();
        app.finish();
        app.cleanup();
        app.update();

        let registry = app.world().resource::<CommandRegistry>();
        let literals = registry
            .graph
            .graph
            .node_weights()
            .filter_map(|node| match &node.data {
                valence::protocol::packets::play::command_tree_s2c::NodeData::Literal { name } => {
                    Some(name.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            literals.contains(&"preview_tp"),
            "registered command literals should include /preview_tp, got {literals:?}"
        );
    }

    #[test]
    fn happy_path_emits_preview_teleport_event() {
        let mut app = setup_app();
        let player = app.world_mut().spawn_empty().id();
        send(
            &mut app,
            player,
            PreviewTpCmd {
                x: 8.0,
                y: 320.0,
                z: 8.0,
                yaw: 0.0,
                pitch: 90.0,
            },
        );
        app.update();
        let events = app.world().resource::<Events<PreviewTeleportRequested>>();
        let mut reader = events.get_reader();
        let collected: Vec<_> = reader.read(events).copied().collect();
        assert_eq!(
            collected.len(),
            1,
            "应 emit 一个 PreviewTeleportRequested event"
        );
        let ev = collected[0];
        assert_eq!(ev.player, player);
        assert_eq!(ev.pos, [8.0, 320.0, 8.0]);
        assert!((ev.yaw - 0.0).abs() < f32::EPSILON);
        assert!((ev.pitch - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn pitch_out_of_range_high_rejects() {
        let mut app = setup_app();
        let player = app.world_mut().spawn_empty().id();
        send(
            &mut app,
            player,
            PreviewTpCmd {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 91.0,
            },
        );
        app.update();
        let events = app.world().resource::<Events<PreviewTeleportRequested>>();
        let mut reader = events.get_reader();
        assert_eq!(
            reader.read(events).count(),
            0,
            "pitch=91（越界 +90）不应 emit event"
        );
    }

    #[test]
    fn pitch_out_of_range_low_rejects() {
        let mut app = setup_app();
        let player = app.world_mut().spawn_empty().id();
        send(
            &mut app,
            player,
            PreviewTpCmd {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
                pitch: -91.0,
            },
        );
        app.update();
        let events = app.world().resource::<Events<PreviewTeleportRequested>>();
        let mut reader = events.get_reader();
        assert_eq!(
            reader.read(events).count(),
            0,
            "pitch=-91（越界 -90）不应 emit event"
        );
    }

    #[test]
    fn pitch_at_boundary_minus_90_accepted() {
        let mut app = setup_app();
        let player = app.world_mut().spawn_empty().id();
        send(
            &mut app,
            player,
            PreviewTpCmd {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
                pitch: -90.0,
            },
        );
        app.update();
        let events = app.world().resource::<Events<PreviewTeleportRequested>>();
        let mut reader = events.get_reader();
        assert_eq!(
            reader.read(events).count(),
            1,
            "pitch=-90（边界，仰天）应被接受 emit event"
        );
    }
}
