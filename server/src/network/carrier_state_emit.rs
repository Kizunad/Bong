use std::collections::HashMap;

use valence::prelude::{Client, Entity, Local, Query, Res, UniqueId, Username, With};

use crate::combat::carrier::{CarrierCharging, CarrierStore};
use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::woliu::entity_wire_id;
use crate::combat::CombatClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_carrier::{CarrierChargePhaseV1, CarrierStateV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

#[derive(Default)]
pub struct CarrierStateEmitCache {
    last: HashMap<Entity, CarrierStateV1>,
}

type CarrierStateClientItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    Option<&'a UniqueId>,
    Option<&'a CarrierCharging>,
    Option<&'a CarrierStore>,
);

pub fn emit_carrier_state_payloads(
    clock: Res<CombatClock>,
    mut cache: Local<CarrierStateEmitCache>,
    mut clients: Query<CarrierStateClientItem<'_>, With<Client>>,
) {
    let periodic = clock.tick.is_multiple_of(TICKS_PER_SECOND);
    for (entity, mut client, username, unique_id, charging, store) in &mut clients {
        let state = carrier_state_payload(
            entity_wire_id(unique_id, entity),
            charging,
            store,
            clock.tick,
        );
        let changed = cache.last.get(&entity) != Some(&state);
        if !periodic && !changed {
            continue;
        }
        cache.last.insert(entity, state.clone());
        let payload = ServerDataV1::new(ServerDataPayloadV1::CarrierState(state));
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
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}`",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0
        );
    }
}

pub fn carrier_state_payload(
    carrier: String,
    charging: Option<&CarrierCharging>,
    store: Option<&CarrierStore>,
    now_tick: u64,
) -> CarrierStateV1 {
    if let Some(charging) = charging {
        let elapsed = now_tick.saturating_sub(charging.started_at_tick);
        return CarrierStateV1 {
            carrier,
            phase: CarrierChargePhaseV1::Charging,
            progress: (elapsed as f32 / crate::combat::carrier::CHARGE_DURATION_TICKS as f32)
                .clamp(0.0, 1.0),
            sealed_qi: charging.qi_target,
            sealed_qi_initial: charging.qi_target,
            half_life_remaining_ticks: 0,
            item_instance_id: Some(charging.instance_id),
        };
    }
    let best = store.and_then(|store| {
        store
            .imprints_by_instance
            .iter()
            .max_by(|a, b| a.1.qi_amount.total_cmp(&b.1.qi_amount))
    });
    if let Some((instance_id, imprint)) = best {
        let ratio =
            (imprint.qi_amount / imprint.qi_amount_initial.max(f32::EPSILON)).clamp(0.0, 1.0);
        let elapsed_ticks = now_tick.saturating_sub(imprint.decay_started_at_tick);
        let full_life_ticks = (imprint.half_life_min * 60.0 * TICKS_PER_SECOND as f32) as u64;
        let remaining = ((ratio / crate::combat::carrier::NATURAL_DECAY_BREAK_RATIO)
            .log2()
            .max(0.0)
            * full_life_ticks as f32) as u64;
        return CarrierStateV1 {
            carrier,
            phase: CarrierChargePhaseV1::Charged,
            progress: ratio,
            sealed_qi: imprint.qi_amount,
            sealed_qi_initial: imprint.qi_amount_initial,
            half_life_remaining_ticks: remaining.saturating_sub(elapsed_ticks.min(remaining)),
            item_instance_id: Some(*instance_id),
        };
    }
    CarrierStateV1 {
        carrier,
        phase: CarrierChargePhaseV1::Idle,
        progress: 0.0,
        sealed_qi: 0.0,
        sealed_qi_initial: 0.0,
        half_life_remaining_ticks: 0,
        item_instance_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::carrier::{BondKind, CarrierImprint, CarrierSlot};
    use crate::cultivation::components::{ColorKind, Realm};

    #[test]
    fn charging_payload_reports_progress() {
        let state = carrier_state_payload(
            "entity:1".to_string(),
            Some(&CarrierCharging {
                slot: CarrierSlot::MainHand,
                instance_id: 7,
                qi_target: 30.0,
                prepaid_qi: 15.0,
                started_at_tick: 100,
                start_pos: valence::prelude::DVec3::ZERO,
            }),
            None,
            200,
        );
        assert_eq!(state.phase, CarrierChargePhaseV1::Charging);
        assert_eq!(state.item_instance_id, Some(7));
        assert!(state.progress > 0.0);
    }

    #[test]
    fn charged_payload_picks_largest_imprint() {
        let store = CarrierStore {
            imprints_by_instance: HashMap::from([(
                9,
                CarrierImprint {
                    qi_amount: 20.0,
                    qi_amount_initial: 40.0,
                    qi_color: ColorKind::Solid,
                    source_realm: Realm::Condense,
                    half_life_min: 120.0,
                    decay_started_at_tick: 0,
                    bond_kind: BondKind::HandheldCarrier,
                },
            )]),
        };
        let state = carrier_state_payload("entity:1".to_string(), None, Some(&store), 20);
        assert_eq!(state.phase, CarrierChargePhaseV1::Charged);
        assert_eq!(state.sealed_qi, 20.0);
        assert_eq!(state.item_instance_id, Some(9));
    }
}
