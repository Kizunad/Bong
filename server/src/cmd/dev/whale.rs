//! Dev 指令：`/whale spawn` —— 在玩家上方 30 块、前方 30 块处生成一只飞鲸。
//! 不复用 `/summon` 前缀（与 rat 共用会让 brigadier root 出现重复 `summon` literal —
//! `command_registry_contains_pinned_root_literals` 测试会撞红）。改用专属根
//! `whale`，与 `rat activate` 同模式。
//!
//! 验证完整 spawn 流程：fauna_tag, blackboard, flight controller, brain
//! thinker 全部就位，flight system 开始 drift。
//!
//! Phase B-2 上线后此命令仍保留，作为手动调试入口。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, Commands, DVec3, Entity, EntityLayerId, EventReader, Look, Position, Query,
    Update, With,
};

use crate::npc::spawn_whale::{spawn_whale_npc_at, DEFAULT_WANDER_RADIUS_XZ};
use crate::world::dimension::OverworldLayer;

/// 玩家面前生成偏移：前 30 块、上方 30 块。鲸体型大，太近会卡视野。
const WHALE_SUMMON_FORWARD: f64 = 30.0;
const WHALE_SUMMON_UP: f64 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhaleTestCmd {
    SummonWhale,
}

impl Command for WhaleTestCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("whale")
            .literal("spawn")
            .with_executable(|_| Self::SummonWhale);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<WhaleTestCmd>()
        .add_systems(Update, handle_whale_test_commands);
}

type WhalePlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Look,
        Option<&'static EntityLayerId>,
        &'static mut Client,
    ),
>;

pub fn handle_whale_test_commands(
    mut commands: Commands,
    mut events: EventReader<CommandResultEvent<WhaleTestCmd>>,
    mut players: WhalePlayerQuery<'_, '_>,
    layers: Query<Entity, With<OverworldLayer>>,
) {
    for event in events.read() {
        tracing::info!(
            "[bong][whale-cmd] /whale spawn dispatched executor={:?}",
            event.executor
        );
        let Ok((position, look, player_layer, mut client)) = players.get_mut(event.executor) else {
            tracing::warn!(
                "[bong][whale-cmd] executor not in player query, skipping (executor={:?})",
                event.executor
            );
            continue;
        };
        let WhaleTestCmd::SummonWhale = event.result;

        let Some(layer) = player_layer
            .map(|layer| layer.0)
            .or_else(|| layers.iter().next())
        else {
            tracing::warn!("[bong][whale-cmd] no active layer found, abort spawn");
            client.send_chat_message("/whale spawn failed: no active layer.");
            continue;
        };
        tracing::info!(
            "[bong][whale-cmd] resolved layer={:?} player_pos={:?} look_yaw={}",
            layer,
            position.get(),
            look.yaw
        );

        // 沿玩家朝向（仅水平）前方 30 块 + 高 30 块。flat XZ vector 避免上看时
        // 把鲸塞天上看不到、下看时把鲸塞地下卡进方块。
        let forward = DVec3::new(
            (-look.yaw.to_radians()).sin() as f64,
            0.0,
            (look.yaw.to_radians()).cos() as f64,
        );
        let player_pos = position.get();
        let home = DVec3::new(
            player_pos.x + forward.x * WHALE_SUMMON_FORWARD,
            player_pos.y + WHALE_SUMMON_UP,
            player_pos.z + forward.z * WHALE_SUMMON_FORWARD,
        );

        let whale = spawn_whale_npc_at(&mut commands, layer, home, DEFAULT_WANDER_RADIUS_XZ);
        tracing::info!(
            "[bong][whale-cmd] spawned whale entity={:?} home=({:.1}, {:.1}, {:.1}) wander_radius={} layer={:?}",
            whale,
            home.x,
            home.y,
            home.z,
            DEFAULT_WANDER_RADIUS_XZ,
            layer
        );
        client.send_chat_message(format!(
            "/whale spawn spawned {:?} at ({:.1}, {:.1}, {:.1}), wander_radius={}",
            whale, home.x, home.y, home.z, DEFAULT_WANDER_RADIUS_XZ
        ));
    }
}
