//! plan-lingtian-v1 §4 / UI 切片 — 把 `ActiveLingtianSessions` 当前快照推到客户端。
//!
//! 设计：每帧扫所有 (player Entity, &mut Client)。如果该 player 有活 session
//! 推一份 active=true 的 LingtianSessionDataV1；否则推 active=false 让客户端
//! 隐藏 HUD 进度条。
//!
//! 流量优化（增量去重）留 P+1：当前直推每帧 + active 字段，让客户端覆盖式更新。
//! 由于 lingtian session 玩家少且字段小（< 100 字节），完全可接受。

use valence::prelude::{Client, Entity, Query, Res};

use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::lingtian::{LingtianSessionDataV1, LingtianSessionKindV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

use super::session::{HarvestSession, PlantingSession};
use super::systems::{ActiveLingtianSessions, ActiveSession};

pub fn emit_lingtian_session_to_clients(
    sessions: Res<ActiveLingtianSessions>,
    mut clients: Query<(Entity, &mut Client)>,
) {
    for (player, mut client) in clients.iter_mut() {
        let payload_data = sessions
            .get(player)
            .map(active_session_to_v1)
            .unwrap_or_default();
        let payload =
            ServerDataV1::new(ServerDataPayloadV1::LingtianSession(Box::new(payload_data)));
        let label = payload_type_label(payload.payload_type());
        let bytes = match serialize_server_data_payload(&payload) {
            Ok(b) => b,
            Err(err) => {
                log_payload_build_error(label, &err);
                continue;
            }
        };
        send_server_data_payload(&mut client, bytes.as_slice());
        // 不打 info — 每帧都发，info 会刷屏；trace 也只在 debug 时开
        tracing::trace!(
            "[bong][lingtian][emit] sent {} {} payload to {player:?}",
            SERVER_DATA_CHANNEL,
            label,
        );
    }
}

fn active_session_to_v1(session: &ActiveSession) -> LingtianSessionDataV1 {
    match session {
        ActiveSession::Till(s) => LingtianSessionDataV1 {
            active: true,
            kind: LingtianSessionKindV1::Till,
            pos: [s.pos.x, s.pos.y, s.pos.z],
            elapsed_ticks: s.elapsed_ticks,
            target_ticks: s.target_ticks(),
            plant_id: None,
        },
        ActiveSession::Renew(s) => LingtianSessionDataV1 {
            active: true,
            kind: LingtianSessionKindV1::Renew,
            pos: [s.pos.x, s.pos.y, s.pos.z],
            elapsed_ticks: s.elapsed_ticks,
            target_ticks: s.target_ticks(),
            plant_id: None,
        },
        ActiveSession::Planting(s) => planting_to_v1(s),
        ActiveSession::Harvest(s) => harvest_to_v1(s),
        ActiveSession::Replenish(s) => LingtianSessionDataV1 {
            active: true,
            kind: LingtianSessionKindV1::Replenish,
            pos: [s.pos.x, s.pos.y, s.pos.z],
            elapsed_ticks: s.elapsed_ticks,
            target_ticks: s.target_ticks(),
            plant_id: None,
        },
        ActiveSession::DrainQi(s) => LingtianSessionDataV1 {
            active: true,
            kind: LingtianSessionKindV1::DrainQi,
            pos: [s.pos.x, s.pos.y, s.pos.z],
            elapsed_ticks: s.elapsed_ticks,
            target_ticks: s.target_ticks(),
            plant_id: None,
        },
    }
}

fn planting_to_v1(s: &PlantingSession) -> LingtianSessionDataV1 {
    LingtianSessionDataV1 {
        active: true,
        kind: LingtianSessionKindV1::Planting,
        pos: [s.pos.x, s.pos.y, s.pos.z],
        elapsed_ticks: s.elapsed_ticks,
        target_ticks: s.target_ticks(),
        plant_id: Some(s.plant_id.clone()),
    }
}

fn harvest_to_v1(s: &HarvestSession) -> LingtianSessionDataV1 {
    LingtianSessionDataV1 {
        active: true,
        kind: LingtianSessionKindV1::Harvest,
        pos: [s.pos.x, s.pos.y, s.pos.z],
        elapsed_ticks: s.elapsed_ticks,
        target_ticks: s.target_ticks(),
        plant_id: Some(s.plant_id.clone()),
    }
}
