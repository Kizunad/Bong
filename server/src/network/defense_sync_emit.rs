//! plan-HUD-v1 §3.4 / §11.4 server-side `defense_sync` 推送。
//!
//! 监听 `Changed<DefenseStance>`：玩家切换姿态、伪皮层数变化、涡流激活/冷却写入
//! 都会触发，把当前完整状态推给该 client，HUD 据此渲染流派指示器。
//!
//! 当前 v1 只在 `switch_defense_stance` 显式切换时变化；伪皮层数 / 涡流冷却
//! 等进阶 mutation 留给战斗系统后续接（当 server 端真打 fake_skin / vortex 时
//! `Changed<DefenseStance>` 自动触发本系统）。

use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Changed, Client, Entity, Query, Res, Username, With};

use crate::combat::components::{DefenseStance, DefenseStanceKind};
use crate::combat::CombatClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{DefenseStanceV1, DefenseSyncV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

const TICK_MS: u64 = 50;

type DefenseSyncEmitFilter = (With<Client>, Changed<DefenseStance>);

pub fn emit_defense_sync_payloads(
    clock: Res<CombatClock>,
    mut clients: Query<(Entity, &mut Client, &Username, &DefenseStance), DefenseSyncEmitFilter>,
) {
    let now_ms = current_unix_millis();
    let now_tick = clock.tick;

    for (entity, mut client, username, stance) in &mut clients {
        let vortex_ready_at_ms = if stance.vortex_ready_at_tick > now_tick {
            let delta = stance.vortex_ready_at_tick - now_tick;
            now_ms.saturating_add(delta.saturating_mul(TICK_MS))
        } else {
            0
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::DefenseSync(DefenseSyncV1 {
            stance: map_stance(stance.stance),
            fake_skin_layers: stance.fake_skin_layers,
            vortex_active: stance.vortex_active,
            vortex_ready_at_ms,
        }));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };

        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracing::info!(
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` (stance={:?} layers={} vortex={})",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0,
            stance.stance,
            stance.fake_skin_layers,
            stance.vortex_active
        );
    }
}

fn map_stance(kind: DefenseStanceKind) -> DefenseStanceV1 {
    match kind {
        DefenseStanceKind::None => DefenseStanceV1::None,
        DefenseStanceKind::Jiemai => DefenseStanceV1::Jiemai,
        DefenseStanceKind::Tishi => DefenseStanceV1::Tishi,
        DefenseStanceKind::Jueling => DefenseStanceV1::Jueling,
    }
}

fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
