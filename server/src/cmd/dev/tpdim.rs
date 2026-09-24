//! `/tpdim <overworld|tsy>` — 保持在同一裸 XYZ 授权邻域内的权威跨维调试命令。
//!
//! 该入口专门用于验证“裸坐标相同也必须按逻辑位面授权”的服务端契约：命令只
//! emit [`DimensionTransferRequest`]，实际 layer、`CurrentDimension`、`Position` 与
//! Respawn 仍由正式 dimension-transfer consumer 一次性更新。目标 X 会按跨维方向
//! 偏移 0.25 格，以强制客户端收到可核验的 XYZ 绝对 PositionLook（视角位可保持
//! 相对）；该距离仍远小于 open 的 4.5 格旧门限，不能让旧 XYZ-only 实现靠距离
//! 拒绝假绿。Valence 当前可能先发初次 PositionLook、再发 Respawn，因此命令还会在
//! 后续 tick 先做 0.001 格的可逆 X 脉冲、再恢复同一最终 Position。Valence 只在
//! `Position != TeleportState.synced_pos` 时发包，因此单纯重写同值不会产生
//! post-Respawn PositionLook；这个双 tick 脉冲提供可核验观察点且最终坐标不漂移。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, App, Client, Commands, Component, DVec3, Entity, EventReader, EventWriter,
    IntoSystemConfigs, Position, Query, Update,
};

use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::dimension_transfer::{DimensionTransferRequest, DimensionTransferSet};

const OBSERVABLE_X_OFFSET: f64 = 0.25;
const POSITION_CONFIRM_PULSE_X: f64 = 0.001;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TpdimCmd {
    Transfer { dimension: String },
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
struct TpdimPositionConfirmPending {
    target_pos: DVec3,
    phase: TpdimPositionConfirmPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TpdimPositionConfirmPhase {
    Arm,
    Pulse,
    Restore,
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
        .add_systems(
            Update,
            // Producers must run before the DimensionTransferSet consumer in the same
            // authoritative commit phase; set membership alone does not order them.
            handle_tpdim
                .before(DimensionTransferSet)
                .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
        )
        .add_systems(
            Update,
            confirm_tpdim_position
                .after(DimensionTransferSet)
                .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
        );
}

pub fn handle_tpdim(
    mut commands: Commands,
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

        let mut target_pos = position.get();
        target_pos.x += match target {
            DimensionKind::Tsy => OBSERVABLE_X_OFFSET,
            DimensionKind::Overworld => -OBSERVABLE_X_OFFSET,
        };
        transfers.send(DimensionTransferRequest {
            entity: event.executor,
            target,
            target_pos,
        });
        commands
            .entity(event.executor)
            .insert(TpdimPositionConfirmPending {
                target_pos,
                phase: TpdimPositionConfirmPhase::Arm,
            });
        client.send_chat_message(format!(
            "Queued /tpdim {} within current XYZ gate.",
            dimension_label(target)
        ));
    }
}

fn confirm_tpdim_position(
    mut commands: Commands,
    mut pending: Query<(Entity, &mut Position, &mut TpdimPositionConfirmPending)>,
) {
    for (entity, mut position, mut confirmation) in &mut pending {
        match confirmation.phase {
            TpdimPositionConfirmPhase::Arm => {
                confirmation.phase = TpdimPositionConfirmPhase::Pulse;
            }
            TpdimPositionConfirmPhase::Pulse => {
                let mut pulse_pos = confirmation.target_pos;
                pulse_pos.x += POSITION_CONFIRM_PULSE_X;
                position.set(pulse_pos);
                confirmation.phase = TpdimPositionConfirmPhase::Restore;
            }
            TpdimPositionConfirmPhase::Restore => {
                position.set(confirmation.target_pos);
                commands
                    .entity(entity)
                    .remove::<TpdimPositionConfirmPending>();
            }
        }
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
    fn transfer_emits_authoritative_request_inside_old_xyz_gate() {
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
        assert_eq!(collected[0].target_pos, DVec3::new(8.25, 96.0, -3.0));
        assert_eq!(
            app.world()
                .get::<TpdimPositionConfirmPending>(player)
                .map(|pending| pending.target_pos),
            Some(DVec3::new(8.25, 96.0, -3.0)),
            "valid /tpdim must queue a post-Respawn authoritative position confirmation"
        );
    }

    #[test]
    fn post_respawn_confirmation_reapplies_final_position_on_later_tick() {
        let mut app = setup_app();
        app.add_systems(Update, confirm_tpdim_position);
        let player = spawn_player(&mut app, DimensionKind::Overworld);
        send(&mut app, player, "tsy");

        run_update(&mut app);
        run_update(&mut app);
        assert!(
            app.world()
                .get::<TpdimPositionConfirmPending>(player)
                .is_some(),
            "confirmation must remain pending for at least one later update"
        );
        run_update(&mut app);

        assert_eq!(
            app.world().get::<Position>(player).unwrap().get(),
            DVec3::new(8.25 + POSITION_CONFIRM_PULSE_X, 96.0, -3.0),
            "confirmation pulse must make Valence observe a real position delta"
        );
        assert_eq!(
            app.world()
                .get::<TpdimPositionConfirmPending>(player)
                .map(|pending| pending.phase),
            Some(TpdimPositionConfirmPhase::Restore),
            "pulse must remain pending until the exact target is restored"
        );

        run_update(&mut app);

        assert_eq!(
            app.world().get::<Position>(player).unwrap().get(),
            DVec3::new(8.25, 96.0, -3.0),
            "later confirmation must reapply the exact transfer target"
        );
        assert!(
            app.world()
                .get::<TpdimPositionConfirmPending>(player)
                .is_none(),
            "confirmation marker must be one-shot"
        );
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
        assert!(
            app.world()
                .get::<TpdimPositionConfirmPending>(player)
                .is_none(),
            "same-dimension no-op must not queue a position confirmation"
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
        assert!(
            app.world()
                .get::<TpdimPositionConfirmPending>(player)
                .is_none(),
            "rejected dimension must not queue a position confirmation"
        );
    }
}
