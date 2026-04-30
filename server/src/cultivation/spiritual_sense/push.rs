use valence::prelude::{Client, Query, With};

use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::realm_vision::SpiritualSenseTargetsV1;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

pub fn send_spiritual_sense_targets(client: &mut Client, targets: SpiritualSenseTargetsV1) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::SpiritualSenseTargets(targets));
    let payload_type = payload_type_label(payload.payload_type());
    let bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, bytes.as_slice());
    tracing::debug!(
        "[bong][spiritual_sense] sent {} {} payload",
        SERVER_DATA_CHANNEL,
        payload_type
    );
}

pub fn push_empty_spiritual_sense_targets(mut clients: Query<&mut Client, With<Client>>) {
    for mut client in &mut clients {
        send_spiritual_sense_targets(
            &mut client,
            SpiritualSenseTargetsV1 {
                entries: Vec::new(),
                generation: 0,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::realm_vision::{SenseEntryV1, SenseKindV1};

    #[test]
    fn server_data_v1_targets_variant() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::SpiritualSenseTargets(
            SpiritualSenseTargetsV1 {
                generation: 2,
                entries: vec![SenseEntryV1 {
                    kind: SenseKindV1::LivingQi,
                    x: 1.0,
                    y: 64.0,
                    z: 2.0,
                    intensity: 0.5,
                }],
            },
        ));
        let value: serde_json::Value = serde_json::to_value(payload).expect("serialize");
        assert_eq!(
            value.get("type").and_then(|v| v.as_str()),
            Some("spiritual_sense_targets")
        );
        assert_eq!(value.get("generation").and_then(|v| v.as_u64()), Some(2));
    }
}
