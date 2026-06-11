//! `bong:vfx/qi_attrition` S2C CustomPayload 发射器。
//!
//! plan-qi-handling-attrition-v1 P2：灵气操作磨损只反馈给操作者客户端，
//! 不复用 `bong:vfx_event` 的半径广播通道，避免旁观者收到 UI/粒子噪声。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, ident, Client, Entity, Event, EventReader, Query, With};

use crate::qi_physics::constants::QI_EPSILON;

pub const QI_ATTRITION_CHANNEL: &str = "bong:vfx/qi_attrition";
const PAYLOAD_VERSION: u8 = 1;

#[derive(Debug, Clone, Event)]
pub struct AttritionAppliedEvent {
    pub operator: Entity,
    pub item_entity_id: u64,
    pub amount_lost: f64,
    pub world_pos: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QiAttritionPayloadV1 {
    pub v: u8,
    pub item_entity_id: u64,
    pub amount_lost: f64,
    pub world_pos: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QiAttritionPayloadError {
    NonPositiveAmount,
    NonFiniteAmount,
    NonFinitePosition,
    SerializeFailed,
}

pub fn build_qi_attrition_payload_bytes(
    item_entity_id: u64,
    amount_lost: f64,
    world_pos: [f64; 3],
) -> Result<Vec<u8>, QiAttritionPayloadError> {
    if !amount_lost.is_finite() {
        return Err(QiAttritionPayloadError::NonFiniteAmount);
    }
    if amount_lost <= QI_EPSILON {
        return Err(QiAttritionPayloadError::NonPositiveAmount);
    }
    if world_pos.iter().any(|value| !value.is_finite()) {
        return Err(QiAttritionPayloadError::NonFinitePosition);
    }

    serde_json::to_vec(&QiAttritionPayloadV1 {
        v: PAYLOAD_VERSION,
        item_entity_id,
        amount_lost,
        world_pos,
    })
    .map_err(|_| QiAttritionPayloadError::SerializeFailed)
}

pub fn emit_qi_attrition_payloads(
    mut events: EventReader<AttritionAppliedEvent>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in events.read() {
        let bytes = match build_qi_attrition_payload_bytes(
            event.item_entity_id,
            event.amount_lost,
            event.world_pos,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    "[bong][attrition][vfx] dropping payload item={} amount={}: {error:?}",
                    event.item_entity_id,
                    event.amount_lost
                );
                continue;
            }
        };

        let Ok(mut client) = clients.get_mut(event.operator) else {
            tracing::debug!(
                "[bong][attrition][vfx] operator {:?} has no Client component; payload dropped",
                event.operator
            );
            continue;
        };
        let _ = QI_ATTRITION_CHANNEL;
        client.send_custom_payload(ident!("bong:vfx/qi_attrition"), &bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::Value;
    use valence::prelude::{App, Position, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<AttritionAppliedEvent>();
        app.add_systems(Update, emit_qi_attrition_payloads);
        app
    }

    fn spawn_mock_client(app: &mut App, name: &str) -> (Entity, MockClientHelper) {
        let (mut bundle, helper) = create_mock_client(name);
        bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let entity = app.world_mut().spawn(bundle).id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_attrition_payloads(helper: &mut MockClientHelper) -> Vec<QiAttritionPayloadV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != QI_ATTRITION_CHANNEL {
                continue;
            }
            payloads.push(
                serde_json::from_slice(packet.data.0 .0)
                    .expect("qi attrition payload should decode"),
            );
        }
        payloads
    }

    #[test]
    fn payload_format_matches_plan_contract() {
        let bytes = build_qi_attrition_payload_bytes(42, 3.25, [1.5, 65.0, -2.25])
            .expect("valid payload should serialize");
        let value: Value = serde_json::from_slice(&bytes).expect("payload should be json");

        assert_eq!(value["v"], 1);
        assert_eq!(value["item_entity_id"], 42);
        assert_eq!(value["amount_lost"], 3.25);
        assert_eq!(value["world_pos"], serde_json::json!([1.5, 65.0, -2.25]));
    }

    #[test]
    fn payload_builder_rejects_non_positive_amount() {
        assert_eq!(
            build_qi_attrition_payload_bytes(42, 0.0, [0.0, 64.0, 0.0]),
            Err(QiAttritionPayloadError::NonPositiveAmount)
        );
    }

    #[test]
    fn payload_builder_rejects_non_finite_values() {
        assert_eq!(
            build_qi_attrition_payload_bytes(42, f64::NAN, [0.0, 64.0, 0.0]),
            Err(QiAttritionPayloadError::NonFiniteAmount)
        );
        assert_eq!(
            build_qi_attrition_payload_bytes(42, 1.0, [0.0, f64::INFINITY, 0.0]),
            Err(QiAttritionPayloadError::NonFinitePosition)
        );
    }

    #[test]
    fn event_sends_payload_only_to_operator_client() {
        let mut app = setup_app();
        let (operator, mut operator_helper) = spawn_mock_client(&mut app, "operator");
        let (_observer, mut observer_helper) = spawn_mock_client(&mut app, "observer");

        app.world_mut().send_event(AttritionAppliedEvent {
            operator,
            item_entity_id: 99,
            amount_lost: 1.5,
            world_pos: [8.0, 65.0, 9.0],
        });
        app.update();
        flush_all_client_packets(&mut app);

        let operator_payloads = collect_attrition_payloads(&mut operator_helper);
        assert_eq!(operator_payloads.len(), 1);
        assert_eq!(operator_payloads[0].item_entity_id, 99);
        assert!(collect_attrition_payloads(&mut observer_helper).is_empty());
    }

    #[test]
    fn event_for_missing_operator_does_not_broadcast() {
        let mut app = setup_app();
        let (_observer, mut observer_helper) = spawn_mock_client(&mut app, "observer");

        app.world_mut().send_event(AttritionAppliedEvent {
            operator: Entity::from_raw(777),
            item_entity_id: 100,
            amount_lost: 2.0,
            world_pos: [0.0, 64.0, 0.0],
        });
        app.update();
        flush_all_client_packets(&mut app);

        assert!(collect_attrition_payloads(&mut observer_helper).is_empty());
    }
}
