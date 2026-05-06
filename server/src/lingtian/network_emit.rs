//! plan-lingtian-v1 §4 / UI 切片 — 把 `ActiveLingtianSessions` 当前快照推到客户端。
//!
//! 设计：每帧扫所有 (player Entity, &mut Client)。如果该 player 有活 session
//! 推一份 active=true 的 LingtianSessionDataV1；否则推 active=false 让客户端
//! 隐藏 HUD 进度条。
//!
//! 流量优化（增量去重）留 P+1：当前直推每帧 + active 字段，让客户端覆盖式更新。
//! 由于 lingtian session 玩家少且字段小（< 100 字节），完全可接受。

use valence::prelude::{BlockPos, Client, Entity, Query, Res};

use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::lingtian::{LingtianSessionDataV1, LingtianSessionKindV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

use super::plot::LingtianPlot;
use super::session::{HarvestSession, PlantingSession, ReplenishSource};
use super::systems::{ActiveLingtianSessions, ActiveSession};

pub fn emit_lingtian_session_to_clients(
    sessions: Res<ActiveLingtianSessions>,
    plots: Query<&LingtianPlot>,
    mut clients: Query<(Entity, &mut Client)>,
) {
    for (player, mut client) in clients.iter_mut() {
        let payload_data = sessions
            .get(player)
            .map(|session| active_session_to_v1(session, &plots))
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

fn active_session_to_v1(
    session: &ActiveSession,
    plots: &Query<&LingtianPlot>,
) -> LingtianSessionDataV1 {
    match session {
        ActiveSession::Till(s) => LingtianSessionDataV1 {
            active: true,
            kind: LingtianSessionKindV1::Till,
            pos: [s.pos.x, s.pos.y, s.pos.z],
            elapsed_ticks: s.elapsed_ticks,
            target_ticks: s.target_ticks(),
            plant_id: None,
            source: None,
            ..plot_status_at(s.pos, plots)
        },
        ActiveSession::Renew(s) => LingtianSessionDataV1 {
            active: true,
            kind: LingtianSessionKindV1::Renew,
            pos: [s.pos.x, s.pos.y, s.pos.z],
            elapsed_ticks: s.elapsed_ticks,
            target_ticks: s.target_ticks(),
            plant_id: None,
            source: None,
            ..plot_status_at(s.pos, plots)
        },
        ActiveSession::Planting(s) => planting_to_v1(s, plots),
        ActiveSession::Harvest(s) => harvest_to_v1(s, plots),
        ActiveSession::Replenish(s) => LingtianSessionDataV1 {
            active: true,
            kind: LingtianSessionKindV1::Replenish,
            pos: [s.pos.x, s.pos.y, s.pos.z],
            elapsed_ticks: s.elapsed_ticks,
            target_ticks: s.target_ticks(),
            plant_id: None,
            source: Some(replenish_source_wire(s.source).to_string()),
            ..plot_status_at(s.pos, plots)
        },
        ActiveSession::DrainQi(s) => LingtianSessionDataV1 {
            active: true,
            kind: LingtianSessionKindV1::DrainQi,
            pos: [s.pos.x, s.pos.y, s.pos.z],
            elapsed_ticks: s.elapsed_ticks,
            target_ticks: s.target_ticks(),
            plant_id: None,
            source: None,
            ..plot_status_at(s.pos, plots)
        },
    }
}

fn planting_to_v1(s: &PlantingSession, plots: &Query<&LingtianPlot>) -> LingtianSessionDataV1 {
    LingtianSessionDataV1 {
        active: true,
        kind: LingtianSessionKindV1::Planting,
        pos: [s.pos.x, s.pos.y, s.pos.z],
        elapsed_ticks: s.elapsed_ticks,
        target_ticks: s.target_ticks(),
        plant_id: Some(s.plant_id.clone()),
        source: None,
        ..plot_status_at(s.pos, plots)
    }
}

fn harvest_to_v1(s: &HarvestSession, plots: &Query<&LingtianPlot>) -> LingtianSessionDataV1 {
    LingtianSessionDataV1 {
        active: true,
        kind: LingtianSessionKindV1::Harvest,
        pos: [s.pos.x, s.pos.y, s.pos.z],
        elapsed_ticks: s.elapsed_ticks,
        target_ticks: s.target_ticks(),
        plant_id: Some(s.plant_id.clone()),
        source: None,
        ..plot_status_at(s.pos, plots)
    }
}

fn plot_status_at(pos: BlockPos, plots: &Query<&LingtianPlot>) -> LingtianSessionDataV1 {
    let Some(plot) = plots.iter().find(|plot| plot.pos == pos) else {
        return LingtianSessionDataV1::default();
    };
    LingtianSessionDataV1 {
        dye_contamination: Some(plot.dye_contamination),
        dye_contamination_warning: plot.has_dye_contamination_warning(),
        ..Default::default()
    }
}

pub(crate) fn replenish_source_wire(source: ReplenishSource) -> &'static str {
    match source {
        ReplenishSource::Zone => "zone",
        ReplenishSource::BoneCoin => "bone_coin",
        ReplenishSource::BeastCore => "beast_core",
        ReplenishSource::LingShui => "ling_shui",
        ReplenishSource::PillResidue { residue_kind } => match residue_kind {
            crate::alchemy::residue::PillResidueKind::FailedPill => "pill_residue_failed_pill",
            crate::alchemy::residue::PillResidueKind::FlawedPill => "pill_residue_flawed_pill",
            crate::alchemy::residue::PillResidueKind::ProcessingDregs => {
                "pill_residue_processing_dregs"
            }
            crate::alchemy::residue::PillResidueKind::AgingScraps => "pill_residue_aging_scraps",
        },
    }
}
