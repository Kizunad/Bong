//! plan-HUD-v1 §11.4 server-side emit for `combat_hud_state` payload.
//!
//! 监听每个客户端的 HUD 源组件，推送聚合百分比与权威战斗窗口，
//! 让左下角小人、真元/体力条和社交开屏策略共享同一份服务端真相。
//! HP 百分比当前直接取 Wounds 聚合结果；Flying/Phasing/TribulationLocked
//! 组件接入前，DerivedAttrFlags 默认全部为 false。

use valence::prelude::{Changed, Client, Entity, Or, Query, Res, Username, With};

use crate::combat::components::{CombatState, Stamina, Wounds};
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::cultivation::tribulation::TribulationState;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{CombatHudStateV1, DerivedAttrFlagsV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type CombatHudEmitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a Cultivation,
    &'a Stamina,
    &'a Wounds,
    Option<&'a TribulationState>,
    Option<&'a CombatState>,
);

type CombatHudEmitFilter = (
    With<Client>,
    Or<(
        Changed<Cultivation>,
        Changed<Stamina>,
        Changed<Wounds>,
        Changed<TribulationState>,
        Changed<CombatState>,
    )>,
);

fn combat_active_at_tick(combat_state: Option<&CombatState>, current_tick: u64) -> bool {
    combat_state.is_some_and(|state| {
        state
            .in_combat_until_tick
            .is_some_and(|until_tick| current_tick < until_tick)
    })
}

pub fn emit_combat_hud_state_payloads(
    clock: Res<CombatClock>,
    mut clients: Query<CombatHudEmitQueryItem<'_>, CombatHudEmitFilter>,
) {
    for (entity, mut client, username, cultivation, stamina, wounds, tribulation, combat_state) in
        &mut clients
    {
        let qi_percent = if cultivation.qi_max > 0.0 {
            (cultivation.qi_current / cultivation.qi_max).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let stamina_percent = if stamina.max > 0.0 {
            (stamina.current / stamina.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let hp_percent = if wounds.health_max > 0.0 {
            (wounds.health_current / wounds.health_max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let combat_active = combat_active_at_tick(combat_state, clock.tick);
        let payload = ServerDataV1::new(ServerDataPayloadV1::CombatHudState(CombatHudStateV1 {
            hp_percent,
            qi_percent,
            stamina_percent,
            combat_active,
            derived: DerivedAttrFlagsV1 {
                tribulation_locked: tribulation.is_some(),
                ..DerivedAttrFlagsV1::default()
            },
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
        tracing::debug!(
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` (hp={:.2} qi={:.2} stam={:.2})",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0,
            hp_percent,
            qi_percent,
            stamina_percent,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_active_uses_server_window_with_exclusive_expiry() {
        let state = CombatState {
            in_combat_until_tick: Some(120),
            ..CombatState::default()
        };

        assert!(
            combat_active_at_tick(Some(&state), 119),
            "战斗窗口结束前必须保持 combat_active=true"
        );
        assert!(
            !combat_active_at_tick(Some(&state), 120),
            "到达 in_combat_until_tick 时必须已经进入脱战态"
        );
        assert!(
            !combat_active_at_tick(
                Some(&CombatState {
                    in_combat_until_tick: Some(0),
                    ..CombatState::default()
                }),
                0
            ),
            "过期的零 tick 战斗窗口必须保持 combat_active=false"
        );
        assert!(
            combat_active_at_tick(Some(&state), 0),
            "当前 tick 早于战斗窗口截止值时必须保持 combat_active=true"
        );
        assert!(
            !combat_active_at_tick(None, 119),
            "缺少 CombatState 时必须按脱战处理，不能伪造战斗态"
        );
    }
}
