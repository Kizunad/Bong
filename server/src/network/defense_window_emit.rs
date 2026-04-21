//! plan-HUD-v1 §3.2 server-side emit for `defense_window` payload.
//!
//! 反应模式 (C): 玩家按 V → handler 发 `DefenseIntent` →
//! `apply_defense_intents` 在 Intent set 同 tick 把 incoming_window 写入
//! `CombatState`。本系统监听 `Changed<CombatState>` 找到 `incoming_window` 在
//! 当前 tick 刚刚开启的实体，推 `defense_window` 给该 client 让红环渲染。

use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Changed, Client, Entity, Query, Res, Username, With};

use crate::combat::components::CombatState;
use crate::combat::CombatClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::DefenseWindowV1;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type DefenseWindowEmitFilter = (With<Client>, Changed<CombatState>);

pub fn emit_defense_window_payloads(
    clock: Res<CombatClock>,
    mut clients: Query<(Entity, &mut Client, &Username, &CombatState), DefenseWindowEmitFilter>,
) {
    let now_ms = current_unix_millis();

    for (entity, mut client, username, combat_state) in &mut clients {
        let Some(window) = combat_state.incoming_window.as_ref() else {
            continue;
        };
        // 只在窗口刚开启的那一 tick 推送，避免重复（CombatState 还有别的字段
        // 也会触发 Changed，例如 last_attack_at_tick）。
        if window.opened_at_tick != clock.tick {
            continue;
        }

        let duration_ms = window.duration_ms;
        let payload = ServerDataV1::new(ServerDataPayloadV1::DefenseWindow(DefenseWindowV1 {
            duration_ms,
            started_at_ms: now_ms,
            expires_at_ms: now_ms.saturating_add(u64::from(duration_ms)),
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
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` (duration_ms={duration_ms}, tick={})",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0,
            clock.tick
        );
    }
}

fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
