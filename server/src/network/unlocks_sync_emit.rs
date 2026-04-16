//! plan-HUD-v1 §1.3 / §11.4 server-side emit for `unlocks_sync` payload。
//!
//! 监听 `Changed<UnlockedStyles>`（含 `Added` —— join 后首次 attach 也会触发）
//! 把当前解锁状态推给该 client，HUD 据此条件渲染防御姿态指示器。

use valence::prelude::{Changed, Client, Entity, Query, Username, With};

use crate::combat::components::UnlockedStyles;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::UnlocksSyncV1;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type UnlocksEmitFilter = (With<Client>, Changed<UnlockedStyles>);

pub fn emit_unlocks_sync_payloads(
    mut clients: Query<(Entity, &mut Client, &Username, &UnlockedStyles), UnlocksEmitFilter>,
) {
    for (entity, mut client, username, unlocks) in &mut clients {
        let payload = ServerDataV1::new(ServerDataPayloadV1::UnlocksSync(UnlocksSyncV1 {
            jiemai: unlocks.jiemai,
            tishi: unlocks.tishi,
            jueling: unlocks.jueling,
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
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` (jiemai={} tishi={} jueling={})",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0,
            unlocks.jiemai,
            unlocks.tishi,
            unlocks.jueling
        );
    }
}
