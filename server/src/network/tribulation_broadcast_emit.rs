use std::collections::HashMap;

use valence::prelude::{Client, Entity, EventReader, Local, Position, Query, Res, With};

use crate::cultivation::tribulation::{
    JueBiTriggeredEvent, TribulationAnnounce, TribulationLocked, TribulationSettled,
    TribulationWaveCleared,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1, TribulationBroadcastV1};
use crate::time::MILLIS_PER_TICK;
use crate::world::event_rhythm::{
    default_event_rhythm, event_trigger_timing_by_player_loop_phase, EventRhythmConfig,
    PlayerLoopPhase, RhythmEventKind,
};
use crate::world::heartbeat::WorldHeartbeat;

const BROADCAST_LIFETIME_MS: u64 = 60_000;
const SPECTATE_INVITE_RADIUS: f64 = 50.0;
const PUBLIC_COORDINATE_GRID_BLOCKS: f64 = 200.0;

#[derive(Debug, Clone)]
pub(crate) struct ActiveTribulationBroadcast {
    data: TribulationBroadcastV1,
    exact_x: f64,
    exact_z: f64,
}

impl ActiveTribulationBroadcast {
    fn active(
        actor_name: impl Into<String>,
        stage: impl Into<String>,
        exact_x: f64,
        exact_z: f64,
        ttl_ms: u64,
    ) -> Self {
        Self {
            data: TribulationBroadcastV1::active(
                actor_name,
                stage,
                public_tribulation_coordinate(exact_x),
                public_tribulation_coordinate(exact_z),
                ttl_ms,
            ),
            exact_x,
            exact_z,
        }
    }

    fn refresh(&mut self, ttl_ms: u64) {
        self.data.refresh(ttl_ms);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn emit_tribulation_broadcast_payloads(
    mut clients: Query<(&mut Client, Option<&Position>), With<Client>>,
    heartbeat: Option<Res<WorldHeartbeat>>,
    mut active_broadcasts: Local<HashMap<Entity, ActiveTribulationBroadcast>>,
    mut announce: EventReader<TribulationAnnounce>,
    mut juebi_triggered: EventReader<JueBiTriggeredEvent>,
    mut locked: EventReader<TribulationLocked>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut settled: EventReader<TribulationSettled>,
) {
    let loop_phase = heartbeat
        .as_deref()
        .map(|heartbeat| heartbeat.loop_phase)
        .unwrap_or(PlayerLoopPhase::SafeShelter);
    let ttl_ms = tribulation_broadcast_ttl_ms(loop_phase);

    for ev in announce.read() {
        let data = ActiveTribulationBroadcast::active(
            ev.actor_name.clone(),
            "warn",
            ev.epicenter[0],
            ev.epicenter[2],
            ttl_ms,
        );
        active_broadcasts.insert(ev.entity, data.clone());
        broadcast(&mut clients, data);
    }
    for ev in juebi_triggered.read() {
        let data = ActiveTribulationBroadcast::active(
            "绝壁劫",
            "jue_bi",
            ev.epicenter[0],
            ev.epicenter[2],
            ttl_ms,
        );
        active_broadcasts.insert(ev.entity, data.clone());
        broadcast(&mut clients, data);
    }
    for ev in locked.read() {
        let data = active_broadcasts.entry(ev.entity).or_insert_with(|| {
            ActiveTribulationBroadcast::active(
                ev.actor_name.clone(),
                "locked",
                ev.epicenter[0],
                ev.epicenter[2],
                ttl_ms,
            )
        });
        data.data.stage = "locked".to_string();
        data.refresh(ttl_ms);
        broadcast(&mut clients, data.clone());
    }
    for ev in cleared.read() {
        let stage = if ev.wave == 0 { "warn" } else { "striking" };
        let data = active_broadcasts
            .entry(ev.entity)
            .or_insert_with(|| ActiveTribulationBroadcast::active("", stage, 0.0, 0.0, ttl_ms));
        data.data.stage = stage.to_string();
        data.refresh(ttl_ms);
        broadcast(&mut clients, data.clone());
    }
    for ev in settled.read() {
        active_broadcasts.remove(&ev.entity);
        broadcast(&mut clients, TribulationBroadcastV1::clear());
    }
}

fn tribulation_broadcast_ttl_ms(loop_phase: PlayerLoopPhase) -> u64 {
    tribulation_broadcast_ttl_ms_from_config(default_event_rhythm(), loop_phase)
}

fn tribulation_broadcast_ttl_ms_from_config(
    config: &EventRhythmConfig,
    loop_phase: PlayerLoopPhase,
) -> u64 {
    event_trigger_timing_by_player_loop_phase(
        config,
        RhythmEventKind::TribulationBroadcast,
        loop_phase,
    )
    .map(|decision| {
        decision
            .timing
            .lead_ticks
            .saturating_add(decision.timing.max_duration_ticks)
            .saturating_mul(MILLIS_PER_TICK)
    })
    .unwrap_or(BROADCAST_LIFETIME_MS)
}

fn broadcast(
    clients: &mut Query<(&mut Client, Option<&Position>), With<Client>>,
    data: impl TribulationBroadcastClientView,
) {
    for (mut client, position) in clients.iter_mut() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::TribulationBroadcast(
            data.for_client(position),
        ));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(payload) => payload,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(&mut client, payload_bytes.as_slice());
    }
}

trait TribulationBroadcastClientView {
    fn for_client(&self, position: Option<&Position>) -> TribulationBroadcastV1;
}

impl TribulationBroadcastClientView for ActiveTribulationBroadcast {
    fn for_client(&self, position: Option<&Position>) -> TribulationBroadcastV1 {
        let mut data = self.data.clone();
        if !data.active {
            return data;
        }
        let Some(position) = position else {
            data.spectate_invite = false;
            data.spectate_distance = 0.0;
            return data;
        };
        let pos = position.get();
        let dx = pos.x - self.exact_x;
        let dz = pos.z - self.exact_z;
        let distance = (dx * dx + dz * dz).sqrt();
        data.spectate_distance = distance;
        data.spectate_invite = distance <= SPECTATE_INVITE_RADIUS;
        data
    }
}

impl TribulationBroadcastClientView for TribulationBroadcastV1 {
    fn for_client(&self, _position: Option<&Position>) -> TribulationBroadcastV1 {
        self.clone()
    }
}

fn public_tribulation_coordinate(value: f64) -> f64 {
    (value / PUBLIC_COORDINATE_GRID_BLOCKS).round() * PUBLIC_COORDINATE_GRID_BLOCKS
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cultivation::tribulation::TribulationAnnounce;
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_mock_client_at(app: &mut App, name: &str, pos: [f64; 3]) -> MockClientHelper {
        let (mut bundle, helper) = create_mock_client(name);
        bundle.player.position = Position::new(pos);
        app.world_mut().spawn(bundle);
        helper
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

    fn collect_tribulation_broadcasts(
        helper: &mut MockClientHelper,
    ) -> Vec<TribulationBroadcastV1> {
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
            if let ServerDataPayloadV1::TribulationBroadcast(data) = payload.payload {
                payloads.push(data);
            }
        }
        payloads
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    }

    fn expected_ttl_from_default_config(loop_phase: PlayerLoopPhase) -> u64 {
        let decision = event_trigger_timing_by_player_loop_phase(
            default_event_rhythm(),
            RhythmEventKind::TribulationBroadcast,
            loop_phase,
        )
        .expect("default event rhythm should define tribulation_broadcast timing");
        decision
            .timing
            .lead_ticks
            .saturating_add(decision.timing.max_duration_ticks)
            .saturating_mul(MILLIS_PER_TICK)
    }

    #[test]
    fn broadcast_fills_distance_per_client() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_broadcast_payloads);

        let mut near = spawn_mock_client_at(&mut app, "Near", [30.0, 66.0, 40.0]);
        let mut far = spawn_mock_client_at(&mut app, "Far", [300.0, 66.0, 400.0]);
        app.world_mut().send_event(TribulationAnnounce {
            entity: Entity::PLACEHOLDER,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [0.0, 66.0, 0.0],
            waves_total: 3,
            started_tick: 0,
        });

        app.update();
        flush_all_client_packets(&mut app);

        let near_payloads = collect_tribulation_broadcasts(&mut near);
        let far_payloads = collect_tribulation_broadcasts(&mut far);
        assert_eq!(near_payloads.len(), 1);
        assert_eq!(far_payloads.len(), 1);
        assert!(near_payloads[0].spectate_invite);
        assert_eq!(near_payloads[0].spectate_distance, 50.0);
        assert!(!far_payloads[0].spectate_invite);
        assert_eq!(far_payloads[0].spectate_distance, 500.0);
        assert_eq!(near_payloads[0].world_x, 0.0);
        assert_eq!(near_payloads[0].world_z, 0.0);
        assert_eq!(near_payloads[0].actor_name, "Azure");
    }

    #[test]
    fn broadcast_public_coordinates_are_rounded_to_poi_grid() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_broadcast_payloads);

        let mut helper = spawn_mock_client_at(&mut app, "Near", [120.0, 66.0, -80.0]);
        app.world_mut().send_event(TribulationAnnounce {
            entity: Entity::PLACEHOLDER,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [301.0, 66.0, -301.0],
            waves_total: 3,
            started_tick: 0,
        });

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_tribulation_broadcasts(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].world_x, 400.0);
        assert_eq!(payloads[0].world_z, -400.0);
    }

    #[test]
    fn broadcast_lifetime_uses_event_rhythm_current_loop_phase() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat.loop_phase = PlayerLoopPhase::OutboundSearch;
        app.insert_resource(heartbeat);
        app.add_systems(Update, emit_tribulation_broadcast_payloads);

        let mut helper = spawn_mock_client_at(&mut app, "RoutePicker", [0.0, 66.0, 0.0]);
        app.world_mut().send_event(TribulationAnnounce {
            entity: Entity::PLACEHOLDER,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [0.0, 66.0, 0.0],
            waves_total: 3,
            started_tick: 0,
        });

        let before_update_ms = now_ms();
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_tribulation_broadcasts(&mut helper);
        assert_eq!(payloads.len(), 1);

        let expected_ttl = expected_ttl_from_default_config(PlayerLoopPhase::OutboundSearch);
        let observed_ttl = payloads[0].expires_at_ms.saturating_sub(before_update_ms);
        assert!(
            observed_ttl >= expected_ttl.saturating_sub(1_000)
                && observed_ttl <= expected_ttl.saturating_add(1_000),
            "天劫广播 TTL 应消费 event_rhythm.json 的 outbound_search timing：expected≈{expected_ttl}ms observed={observed_ttl}ms"
        );
        assert_eq!(
            tribulation_broadcast_ttl_ms(PlayerLoopPhase::OutboundSearch),
            expected_ttl,
            "helper 返回值应来自默认 rhythm 配置，而不是私有硬编码常量"
        );
        assert!(
            expected_ttl > BROADCAST_LIFETIME_MS,
            "测试应证明 rhythm 配置覆盖了旧固定 60s TTL，而不是继续使用死常量"
        );
    }

    #[test]
    fn broadcast_lifetime_differs_between_loop_phases() {
        let outbound = tribulation_broadcast_ttl_ms(PlayerLoopPhase::OutboundSearch);
        let safe = tribulation_broadcast_ttl_ms(PlayerLoopPhase::SafeShelter);

        assert_eq!(
            outbound,
            expected_ttl_from_default_config(PlayerLoopPhase::OutboundSearch),
            "outbound_search TTL 应由配置中的天劫 timing 推导"
        );
        assert_eq!(
            safe,
            expected_ttl_from_default_config(PlayerLoopPhase::SafeShelter),
            "safe_shelter TTL 应由配置 fallback timing 推导"
        );
        assert_ne!(
            outbound, safe,
            "不同循环阶段的天劫广播 TTL 应体现 event_rhythm timing 差异"
        );
    }

    #[test]
    fn broadcast_lifetime_falls_back_when_rhythm_rule_missing() {
        let mut config = default_event_rhythm().clone();
        config
            .rules
            .retain(|rule| rule.event != RhythmEventKind::TribulationBroadcast);

        assert_eq!(
            tribulation_broadcast_ttl_ms_from_config(&config, PlayerLoopPhase::OutboundSearch),
            BROADCAST_LIFETIME_MS,
            "缺少 tribulation_broadcast 规则时应回退旧固定 TTL，而不是 panic 或返回 0"
        );
    }
}
