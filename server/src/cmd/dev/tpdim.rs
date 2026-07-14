//! `/tpdim <overworld|tsy>` — 保持裸 XYZ 不变的服务端权威跨维调试命令。
//!
//! 该入口专门用于验证“裸坐标相同也必须按逻辑位面授权”的服务端契约：命令只
//! emit [`DimensionTransferRequest`]，实际 layer、`CurrentDimension`、`Position` 与
//! Respawn 仍由正式 dimension-transfer consumer 一次性更新。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, EventWriter, Position, Query, Update};

use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::dimension_transfer::DimensionTransferRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TpdimCmd {
    Transfer { dimension: String },
}

impl Command for TpdimCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("tpdim")
            .argument("dimension")
            .with_parser::<String>()
            .with_executable(|input| TpdimCmd::Transfer {
                dimension: String::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    // dimension_transfer::register 在正式 App 中也会注册该事件；init_resource
    // 语义允许这里重复 add_event，同时保证 cmd 单测可独立运行。
    app.add_event::<DimensionTransferRequest>()
        .add_command::<TpdimCmd>()
        .add_systems(Update, handle_tpdim);
}

pub fn handle_tpdim(
    mut events: EventReader<CommandResultEvent<TpdimCmd>>,
    mut transfers: EventWriter<DimensionTransferRequest>,
    mut players: Query<(&Position, &CurrentDimension, &mut Client)>,
) {
    for event in events.read() {
        let TpdimCmd::Transfer { dimension } = &event.result;
        let Ok((position, current_dimension, mut client)) = players.get_mut(event.executor) else {
            continue;
        };

        let Some(target) = parse_dimension(dimension) else {
            client.send_chat_message(format!(
                "Unknown dimension `{dimension}`; expected `overworld` or `tsy`."
            ));
            continue;
        };
        if current_dimension.0 == target {
            client.send_chat_message(format!(
                "Already in dimension `{}`.",
                dimension_label(target)
            ));
            continue;
        }

        transfers.send(DimensionTransferRequest {
            entity: event.executor,
            target,
            target_pos: position.get(),
        });
        client.send_chat_message(format!(
            "Queued /tpdim {} at current XYZ.",
            dimension_label(target)
        ));
    }
}

fn parse_dimension(value: &str) -> Option<DimensionKind> {
    match value {
        "overworld" => Some(DimensionKind::Overworld),
        "tsy" => Some(DimensionKind::Tsy),
        _ => None,
    }
}

const fn dimension_label(dimension: DimensionKind) -> &'static str {
    match dimension {
        DimensionKind::Overworld => "overworld",
        DimensionKind::Tsy => "tsy",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::{DVec3, Entity, Events};

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<TpdimCmd>>();
        app.add_event::<DimensionTransferRequest>();
        app.add_systems(Update, handle_tpdim);
        app
    }

    fn spawn_player(app: &mut App, dimension: DimensionKind) -> Entity {
        let player = spawn_test_client(app, "Alice", [8.0, 96.0, -3.0]);
        app.world_mut()
            .entity_mut(player)
            .insert(CurrentDimension(dimension));
        player
    }

    fn send(app: &mut App, executor: Entity, dimension: &str) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TpdimCmd>>>()
            .send(CommandResultEvent {
                result: TpdimCmd::Transfer {
                    dimension: dimension.to_string(),
                },
                executor,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn dimension_argument_parser_accepts_single_word() {
        assert_eq!(String::arg_from_str("tsy").unwrap(), "tsy");
    }

    #[test]
    fn dimension_argument_parser_stops_at_whitespace() {
        assert_eq!(
            String::arg_from_str("overworld extra").unwrap(),
            "overworld"
        );
    }

    #[test]
    fn transfer_emits_same_xyz_authoritative_request() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, DimensionKind::Overworld);
        send(&mut app, player, "tsy");

        run_update(&mut app);

        let events = app.world().resource::<Events<DimensionTransferRequest>>();
        let collected = events
            .get_reader()
            .read(events)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(collected.len(), 1, "valid /tpdim must emit one transfer");
        assert_eq!(collected[0].entity, player);
        assert_eq!(collected[0].target, DimensionKind::Tsy);
        assert_eq!(collected[0].target_pos, DVec3::new(8.0, 96.0, -3.0));
    }

    #[test]
    fn same_dimension_is_noop() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, DimensionKind::Tsy);
        send(&mut app, player, "tsy");

        run_update(&mut app);

        let events = app.world().resource::<Events<DimensionTransferRequest>>();
        assert_eq!(
            events.get_reader().read(events).count(),
            0,
            "same-dimension /tpdim must not emit a redundant Respawn transfer"
        );
    }

    #[test]
    fn unknown_dimension_is_rejected() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, DimensionKind::Overworld);
        send(&mut app, player, "nether");

        run_update(&mut app);

        let events = app.world().resource::<Events<DimensionTransferRequest>>();
        assert_eq!(
            events.get_reader().read(events).count(),
            0,
            "unknown dimension must not emit a transfer"
        );
    }
}
