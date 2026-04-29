use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Client, Entity, EventReader, Query, With};

use crate::cultivation::tribulation::{
    TribulationState, TribulationWaveCleared, DUXU_HEART_DEMON_TIMEOUT_TICKS, DUXU_HEART_DEMON_WAVE,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{
    HeartDemonOfferChoiceV1, HeartDemonOfferV1, ServerDataPayloadV1, ServerDataV1,
};

const MILLIS_PER_TICK: u64 = 50;

pub fn emit_heart_demon_offer_payloads(
    mut clients: Query<(Entity, &mut Client), With<Client>>,
    tribulations: Query<&TribulationState>,
    mut cleared: EventReader<TribulationWaveCleared>,
) {
    for ev in cleared.read() {
        if ev.wave != DUXU_HEART_DEMON_WAVE {
            continue;
        }
        let Ok(state) = tribulations.get(ev.entity) else {
            continue;
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::HeartDemonOffer(
            default_heart_demon_offer(ev.entity, state),
        ));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(payload) => payload,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        for (entity, mut client) in &mut clients {
            if entity == ev.entity {
                send_server_data_payload(&mut client, payload_bytes.as_slice());
            }
        }
    }
}

fn default_heart_demon_offer(entity: Entity, state: &TribulationState) -> HeartDemonOfferV1 {
    HeartDemonOfferV1 {
        offer_id: format!("heart_demon:{}:{}", entity.index(), state.started_tick),
        trigger_id: format!("heart_demon:{}:{}", entity.index(), state.started_tick),
        trigger_label: "心魔劫临身".to_string(),
        realm_label: "渡虚劫 · 心魔".to_string(),
        composure: 0.5,
        quota_remaining: 1,
        quota_total: 1,
        expires_at_ms: now_ms()
            .saturating_add(DUXU_HEART_DEMON_TIMEOUT_TICKS.saturating_mul(MILLIS_PER_TICK)),
        choices: vec![
            HeartDemonOfferChoiceV1 {
                choice_id: "heart_demon_choice_0".to_string(),
                category: "Composure".to_string(),
                title: "守本心".to_string(),
                effect_summary: "稳住心神，回复少量当前真元".to_string(),
                flavor: "你不逐影，不辩幻象，只把呼吸压回丹田。雷声暂远，心火自息。".to_string(),
                style_hint: "稳妥".to_string(),
            },
            HeartDemonOfferChoiceV1 {
                choice_id: "heart_demon_choice_1".to_string(),
                category: "Breakthrough".to_string(),
                title: "斩执念".to_string(),
                effect_summary: "若斩错心魔，将损当前真元并强化下一道开天雷".to_string(),
                flavor: "你提起一念作刀，刀锋却照见自己的影。此路或快，亦可能更重。".to_string(),
                style_hint: "冒险".to_string(),
            },
            HeartDemonOfferChoiceV1 {
                choice_id: "heart_demon_choice_2".to_string(),
                category: "Perception".to_string(),
                title: "无解".to_string(),
                effect_summary: "承认无解，不得增益也不受真元惩罚".to_string(),
                flavor: "你看清此问无门，便不再替天道补题。空处落空，反倒没有再咬你。".to_string(),
                style_hint: "止损".to_string(),
            },
        ],
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_mock_client(app: &mut App, name: &str) -> (Entity, MockClientHelper) {
        let (bundle, helper) = create_mock_client(name);
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

    fn collect_heart_demon_offers(helper: &mut MockClientHelper) -> Vec<HeartDemonOfferV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server data payload should decode");
            if let ServerDataPayloadV1::HeartDemonOffer(data) = payload.payload {
                payloads.push(data);
            }
        }
        payloads
    }

    #[test]
    fn heart_demon_offer_is_sent_only_to_tribulator() {
        let mut app = App::new();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, emit_heart_demon_offer_payloads);

        let (tribulator, mut tribulator_helper) = spawn_mock_client(&mut app, "Azure");
        let (_spectator, mut spectator_helper) = spawn_mock_client(&mut app, "Spectator");
        app.world_mut()
            .entity_mut(tribulator)
            .insert(TribulationState {
                kind: crate::cultivation::tribulation::TribulationKind::DuXu,
                phase: crate::cultivation::tribulation::TribulationPhase::HeartDemon,
                epicenter: [0.0, 64.0, 0.0],
                wave_current: DUXU_HEART_DEMON_WAVE,
                waves_total: 5,
                started_tick: 1_000,
                phase_started_tick: 1_200,
                next_wave_tick: 1_500,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
                half_step_on_success: false,
            });
        app.world_mut().send_event(TribulationWaveCleared {
            entity: tribulator,
            wave: DUXU_HEART_DEMON_WAVE,
        });

        app.update();
        flush_all_client_packets(&mut app);

        let tribulator_payloads = collect_heart_demon_offers(&mut tribulator_helper);
        let spectator_payloads = collect_heart_demon_offers(&mut spectator_helper);
        assert_eq!(tribulator_payloads.len(), 1);
        assert_eq!(spectator_payloads.len(), 0);
        assert_eq!(
            tribulator_payloads[0].trigger_id,
            format!("heart_demon:{}:1000", tribulator.index())
        );
        assert_eq!(tribulator_payloads[0].choices.len(), 3);
    }
}
