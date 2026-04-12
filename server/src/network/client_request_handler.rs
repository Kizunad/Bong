//! 客户端 → 服务端 `bong:client_request` 通道处理（plan-cultivation-v1 §P1 剩余）。
//!
//! Fabric 客户端通过 Minecraft CustomPayload 发送 `ClientRequestV1` JSON；
//! 本系统读取 Valence `CustomPayloadEvent`，按 channel 过滤 → 反序列化
//! → 发射对应 Bevy 事件：
//!   - SetMeridianTarget → 插入/更新 `MeridianTarget` Component
//!   - BreakthroughRequest → emit `BreakthroughRequest` Bevy event
//!   - ForgeRequest → emit `ForgeRequest` Bevy event

use valence::custom_payload::CustomPayloadEvent;
use valence::prelude::{Commands, EventReader, EventWriter};

use crate::cultivation::breakthrough::BreakthroughRequest;
use crate::cultivation::forging::ForgeRequest;
use crate::cultivation::insight::InsightChosen;
use crate::cultivation::meridian_open::MeridianTarget;
use crate::schema::client_request::ClientRequestV1;

const CHANNEL: &str = "bong:client_request";
const SUPPORTED_VERSION: u8 = 1;

pub fn handle_client_request_payloads(
    mut events: EventReader<CustomPayloadEvent>,
    mut breakthrough_tx: EventWriter<BreakthroughRequest>,
    mut forge_tx: EventWriter<ForgeRequest>,
    mut insight_tx: EventWriter<InsightChosen>,
    mut commands: Commands,
) {
    for ev in events.read() {
        if ev.channel.as_str() != CHANNEL {
            continue;
        }

        let payload = match std::str::from_utf8(&ev.data) {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(
                    "[bong][network] client_request payload not utf8 from {:?}: {err}",
                    ev.client
                );
                continue;
            }
        };

        let request: ClientRequestV1 = match serde_json::from_str(payload) {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!(
                    "[bong][network] client_request deserialize failed from {:?}: {err}; body={payload}",
                    ev.client
                );
                continue;
            }
        };

        let v = match &request {
            ClientRequestV1::SetMeridianTarget { v, .. }
            | ClientRequestV1::BreakthroughRequest { v }
            | ClientRequestV1::ForgeRequest { v, .. }
            | ClientRequestV1::InsightDecision { v, .. } => *v,
        };
        if v != SUPPORTED_VERSION {
            tracing::warn!(
                "[bong][network] client_request unsupported version v={v} from {:?}; body={payload}",
                ev.client
            );
            continue;
        }

        match request {
            ClientRequestV1::SetMeridianTarget { meridian, .. } => {
                tracing::info!(
                    "[bong][network] client_request set_meridian_target entity={:?} meridian={:?}",
                    ev.client,
                    meridian
                );
                commands.entity(ev.client).insert(MeridianTarget(meridian));
            }
            ClientRequestV1::BreakthroughRequest { .. } => {
                tracing::info!(
                    "[bong][network] client_request breakthrough entity={:?}",
                    ev.client
                );
                // TODO(plan-cultivation-v1 §P2 inventory)：从玩家背包派生 material_bonus
                //   (0.0..=0.30)；当前占位为 0.0，等价于无灵材加成突破。
                breakthrough_tx.send(BreakthroughRequest {
                    entity: ev.client,
                    material_bonus: 0.0,
                });
            }
            ClientRequestV1::InsightDecision {
                trigger_id,
                choice_idx,
                ..
            } => {
                tracing::info!(
                    "[bong][network] client_request insight_decision entity={:?} trigger={} idx={:?}",
                    ev.client,
                    trigger_id,
                    choice_idx
                );
                insight_tx.send(InsightChosen {
                    entity: ev.client,
                    trigger_id,
                    choice_idx: choice_idx.map(|n| n as usize),
                });
            }
            ClientRequestV1::ForgeRequest { meridian, axis, .. } => {
                tracing::info!(
                    "[bong][network] client_request forge entity={:?} meridian={:?} axis={:?}",
                    ev.client,
                    meridian,
                    axis
                );
                forge_tx.send(ForgeRequest {
                    entity: ev.client,
                    meridian,
                    axis,
                });
            }
        }
    }
}
