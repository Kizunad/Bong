use std::collections::{HashMap, HashSet};

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
    mut clients: Query<(Entity, &mut Client, Option<&Position>), With<Client>>,
    heartbeat: Option<Res<WorldHeartbeat>>,
    mut active_broadcasts: Local<HashMap<Entity, ActiveTribulationBroadcast>>,
    mut announce: EventReader<TribulationAnnounce>,
    mut juebi_triggered: EventReader<JueBiTriggeredEvent>,
    mut locked: EventReader<TribulationLocked>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut settled: EventReader<TribulationSettled>,
    mut known_clients: Local<Option<HashSet<Entity>>>,
) {
    let current_clients: HashSet<Entity> =
        clients.iter_mut().map(|(entity, _, _)| entity).collect();
    let joined_clients: HashSet<Entity> = known_clients.as_ref().map_or_else(
        || current_clients.clone(),
        |previous| current_clients.difference(previous).copied().collect(),
    );
    *known_clients = Some(current_clients);
    let mut emitted_entities = HashSet::new();
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
        emitted_entities.insert(ev.entity);
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
        emitted_entities.insert(ev.entity);
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
        emitted_entities.insert(ev.entity);
    }
    for ev in cleared.read() {
        let stage = if ev.wave == 0 { "warn" } else { "striking" };
        let data = active_broadcasts
            .entry(ev.entity)
            .or_insert_with(|| ActiveTribulationBroadcast::active("", stage, 0.0, 0.0, ttl_ms));
        data.data.stage = stage.to_string();
        data.refresh(ttl_ms);
        broadcast(&mut clients, data.clone());
        emitted_entities.insert(ev.entity);
    }
    for ev in settled.read() {
        if let Some(mut data) = active_broadcasts.remove(&ev.entity) {
            data.data.active = false;
            data.data.stage = "done".to_string();
            data.data.expires_at_ms = 0;
            data.data.spectate_invite = false;
            data.data.spectate_distance = 0.0;
            broadcast(&mut clients, data);
            emitted_entities.insert(ev.entity);
        } else if active_broadcasts.is_empty() {
            broadcast(&mut clients, TribulationBroadcastV1::clear());
        }
    }

    if !joined_clients.is_empty() {
        for (entity, data) in active_broadcasts.iter() {
            if emitted_entities.contains(entity) {
                continue;
            }
            broadcast_to_clients(&mut clients, &joined_clients, data.clone());
        }
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
    clients: &mut Query<(Entity, &mut Client, Option<&Position>), With<Client>>,
    data: impl TribulationBroadcastClientView,
) {
    broadcast_filtered(clients, None, data);
}

fn broadcast_to_clients(
    clients: &mut Query<(Entity, &mut Client, Option<&Position>), With<Client>>,
    target_clients: &HashSet<Entity>,
    data: impl TribulationBroadcastClientView,
) {
    broadcast_filtered(clients, Some(target_clients), data);
}

fn broadcast_filtered(
    clients: &mut Query<(Entity, &mut Client, Option<&Position>), With<Client>>,
    target_clients: Option<&HashSet<Entity>>,
    data: impl TribulationBroadcastClientView,
) {
    for (entity, mut client, position) in clients.iter_mut() {
        if target_clients.is_some_and(|targets| !targets.contains(&entity)) {
            continue;
        }
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

    use crate::cultivation::tribulation::{TribulationAnnounce, TribulationKind};
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::schema::tribulation::{DuXuOutcomeV1, DuXuResultV1};
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_mock_client_entity_at(
        app: &mut App,
        name: &str,
        pos: [f64; 3],
    ) -> (Entity, MockClientHelper) {
        let (mut bundle, helper) = create_mock_client(name);
        bundle.player.position = Position::new(pos);
        let entity = app.world_mut().spawn(bundle).id();
        (entity, helper)
    }

    fn spawn_mock_client_at(app: &mut App, name: &str, pos: [f64; 3]) -> MockClientHelper {
        spawn_mock_client_entity_at(app, name, pos).1
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
    fn settled_broadcast_targets_finished_tribulation_and_preserves_others() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_broadcast_payloads);

        let mut helper = spawn_mock_client_at(&mut app, "Watcher", [0.0, 66.0, 0.0]);
        let first = app.world_mut().spawn_empty().id();
        let second = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(TribulationAnnounce {
            entity: first,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [0.0, 66.0, 0.0],
            waves_total: 3,
            started_tick: 0,
        });
        app.world_mut().send_event(TribulationAnnounce {
            entity: second,
            char_id: "offline:Beryl".to_string(),
            actor_name: "Beryl".to_string(),
            epicenter: [400.0, 66.0, 0.0],
            waves_total: 3,
            started_tick: 0,
        });

        app.update();
        flush_all_client_packets(&mut app);
        let initial = collect_tribulation_broadcasts(&mut helper);
        assert_eq!(
            initial.len(),
            2,
            "并发起劫应向 client 连发两条活跃 broadcast"
        );

        app.world_mut().send_event(TribulationSettled {
            entity: first,
            kind: TribulationKind::DuXu,
            source: None,
            result: DuXuResultV1 {
                char_id: "offline:Azure".to_string(),
                outcome: DuXuOutcomeV1::Ascended,
                killer: None,
                waves_survived: 3,
                reason: None,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);
        let settled = collect_tribulation_broadcasts(&mut helper);
        assert_eq!(settled.len(), 1);
        assert!(!settled[0].active);
        assert_eq!(settled[0].actor_name, "Azure");
        assert_eq!(settled[0].world_x, 0.0);
        assert_eq!(settled[0].world_z, 0.0);

        app.world_mut().send_event(TribulationWaveCleared {
            entity: second,
            wave: 1,
        });

        app.update();
        flush_all_client_packets(&mut app);
        let remaining = collect_tribulation_broadcasts(&mut helper);
        assert_eq!(remaining.len(), 1);
        assert!(
            remaining[0].active,
            "另一场仍活跃时不应被 settled clear 抹掉"
        );
        assert_eq!(remaining[0].actor_name, "Beryl");
        assert_eq!(remaining[0].stage, "striking");
    }

    #[test]
    fn new_client_join_replays_all_active_broadcasts() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_broadcast_payloads);

        let mut first_client = spawn_mock_client_at(&mut app, "First", [0.0, 66.0, 0.0]);
        let first = app.world_mut().spawn_empty().id();
        let second = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(TribulationAnnounce {
            entity: first,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [0.0, 66.0, 0.0],
            waves_total: 3,
            started_tick: 0,
        });
        app.world_mut().send_event(TribulationAnnounce {
            entity: second,
            char_id: "offline:Beryl".to_string(),
            actor_name: "Beryl".to_string(),
            epicenter: [400.0, 66.0, 0.0],
            waves_total: 3,
            started_tick: 0,
        });

        app.update();
        flush_all_client_packets(&mut app);
        assert_eq!(collect_tribulation_broadcasts(&mut first_client).len(), 2);

        let mut late_client = spawn_mock_client_at(&mut app, "Late", [100.0, 66.0, 0.0]);
        app.update();
        flush_all_client_packets(&mut app);

        let replayed = collect_tribulation_broadcasts(&mut late_client);
        assert_eq!(
            replayed.len(),
            2,
            "中途加入的 client 应收到每一场仍活跃的 tribulation broadcast"
        );
        let mut actor_names = replayed
            .iter()
            .map(|payload| payload.actor_name.clone())
            .collect::<Vec<_>>();
        actor_names.sort();
        assert_eq!(actor_names, vec!["Azure".to_string(), "Beryl".to_string()]);
        assert!(replayed.iter().all(|payload| payload.active));
    }

    #[test]
    fn same_count_client_replacement_replays_active_broadcasts_to_new_entity() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_broadcast_payloads);

        let mut stable_client = spawn_mock_client_at(&mut app, "Stable", [0.0, 66.0, 0.0]);
        let (departing_entity, mut departing_client) =
            spawn_mock_client_entity_at(&mut app, "Departing", [0.0, 66.0, 0.0]);
        let tribulation = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(TribulationAnnounce {
            entity: tribulation,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [0.0, 66.0, 0.0],
            waves_total: 3,
            started_tick: 0,
        });

        app.update();
        flush_all_client_packets(&mut app);
        assert_eq!(collect_tribulation_broadcasts(&mut stable_client).len(), 1);
        assert_eq!(
            collect_tribulation_broadcasts(&mut departing_client).len(),
            1
        );

        app.world_mut().entity_mut(departing_entity).despawn();
        let mut late_client = spawn_mock_client_at(&mut app, "Late", [100.0, 66.0, 0.0]);

        app.update();
        flush_all_client_packets(&mut app);

        let stable_replayed = collect_tribulation_broadcasts(&mut stable_client);
        assert!(
            stable_replayed.is_empty(),
            "active broadcast replay 应只补给新 client，不应刷屏既有 client"
        );
        let replayed = collect_tribulation_broadcasts(&mut late_client);
        assert_eq!(
            replayed.len(),
            1,
            "即使总 client 数不变，新 Entity client 也必须收到 active broadcast replay"
        );
        assert!(replayed[0].active);
        assert_eq!(replayed[0].actor_name, "Azure");
    }

    #[test]
    fn new_client_join_replays_all_active_broadcasts_even_when_same_tick_emits_one() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_broadcast_payloads);

        let mut first_client = spawn_mock_client_at(&mut app, "First", [0.0, 66.0, 0.0]);
        let first = app.world_mut().spawn_empty().id();
        let second = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(TribulationAnnounce {
            entity: first,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [0.0, 66.0, 0.0],
            waves_total: 3,
            started_tick: 0,
        });
        app.world_mut().send_event(TribulationAnnounce {
            entity: second,
            char_id: "offline:Beryl".to_string(),
            actor_name: "Beryl".to_string(),
            epicenter: [400.0, 66.0, 0.0],
            waves_total: 3,
            started_tick: 0,
        });

        app.update();
        flush_all_client_packets(&mut app);
        assert_eq!(collect_tribulation_broadcasts(&mut first_client).len(), 2);

        let mut late_client = spawn_mock_client_at(&mut app, "Late", [100.0, 66.0, 0.0]);
        app.world_mut().send_event(TribulationWaveCleared {
            entity: second,
            wave: 1,
        });

        app.update();
        flush_all_client_packets(&mut app);

        let replayed = collect_tribulation_broadcasts(&mut late_client);
        assert_eq!(
            replayed.len(),
            2,
            "同 tick 增量 + join replay 后，新 client 应收敛到完整 active broadcast 集且不重复"
        );
        assert!(
            replayed
                .iter()
                .any(|payload| payload.active && payload.actor_name == "Azure"),
            "同 tick 有 Beryl wave emit 时，新 client 仍必须补收既有 Azure active broadcast"
        );
        assert!(
            replayed
                .iter()
                .any(|payload| payload.active && payload.actor_name == "Beryl"),
            "新 client 应同时收到本 tick emit 的 Beryl broadcast"
        );
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
