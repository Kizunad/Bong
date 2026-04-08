pub mod agent_bridge;
pub mod chat_collector;
pub mod command_executor;
pub mod redis_bridge;

use std::collections::{HashMap, HashSet};

use agent_bridge::{
    build_heartbeat_payload, build_welcome_payload, AgentCommand, GameEvent, NetworkBridgeResource,
    PayloadBuildError, SERVER_DATA_CHANNEL,
};
use command_executor::{
    execute_agent_commands, validate_and_enqueue_agent_command_batch,
    ActiveThunderTribulationEvent, ActiveWorldEventsResource, CommandExecutorResource,
};
use redis_bridge::{RedisInbound, RedisOutbound};
use valence::prelude::{
    ident, Added, App, Client, DVec3, DetectChanges, Entity, Query, Res, Resource, Update, Uuid,
};

use crate::player::state::PlayerState;
use crate::schema::client_payload::{
    ClientPayloadV1, EventAlertPayload, EventAlertSeverity, PlayerStatePayload, ZoneInfoPayload,
};
use crate::schema::common::EventKind;
use crate::schema::world_state::{
    GameEvent as WorldGameEvent, PlayerProfile, WorldStateV1, ZoneSnapshot,
};
use crate::world::ZoneRegistry;

const REDIS_URL: &str = "redis://127.0.0.1:6379";
const WORLD_STATE_PUBLISH_INTERVAL_TICKS: u64 = 200;

pub struct RedisBridgeResource {
    pub tx_outbound: crossbeam_channel::Sender<RedisOutbound>,
    pub rx_inbound: crossbeam_channel::Receiver<RedisInbound>,
}

impl Resource for RedisBridgeResource {}

#[derive(Default)]
pub struct WorldStateTimer {
    ticks: u64,
}

impl Resource for WorldStateTimer {}

#[derive(Default)]
struct EmissionState {
    last_zone_by_client: HashMap<Entity, String>,
    announced_events: HashSet<String>,
}

impl Resource for EmissionState {}

pub fn register(app: &mut App) {
    app.add_systems(
        Update,
        (send_welcome_payload_on_join, process_bridge_messages),
    );

    let (handle, tx_outbound, rx_inbound) = redis_bridge::spawn_redis_bridge(REDIS_URL);
    std::mem::drop(handle);

    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.insert_resource(WorldStateTimer::default());
    app.insert_resource(EmissionState::default());
    app.init_resource::<CommandExecutorResource>();
    app.init_resource::<ActiveWorldEventsResource>();

    app.add_systems(
        Update,
        (
            chat_collector::collect_player_chat_to_redis,
            publish_world_state_to_redis,
            emit_player_state_payloads_for_changes,
            process_redis_inbound,
            execute_agent_commands,
        ),
    );
}

fn publish_world_state_to_redis(
    redis: Res<RedisBridgeResource>,
    mut timer: valence::prelude::ResMut<WorldStateTimer>,
    clients: Query<(
        &valence::prelude::Username,
        &valence::prelude::UniqueId,
        &valence::prelude::Position,
        Option<&PlayerState>,
    )>,
    zone_registry: Res<ZoneRegistry>,
    active_events: Res<ActiveWorldEventsResource>,
) {
    timer.ticks += 1;
    if !timer
        .ticks
        .is_multiple_of(WORLD_STATE_PUBLISH_INTERVAL_TICKS)
    {
        return;
    }

    let (players, player_counts) = collect_players_for_world_state(
        clients
            .iter()
            .map(|(username, unique_id, pos, player_state)| {
                (username.0.as_str(), unique_id.0, pos.get(), player_state)
            }),
        zone_registry.as_ref(),
    );

    let state = WorldStateV1 {
        v: 1,
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        tick: timer.ticks,
        players,
        npcs: vec![],
        zones: collect_zone_snapshots(zone_registry.as_ref(), &player_counts),
        recent_events: collect_recent_events_for_world_state(active_events.as_ref(), timer.ticks),
    };

    let _ = redis.tx_outbound.send(RedisOutbound::WorldState(state));
}

pub(crate) fn collect_players_for_world_state<'a, I>(
    players: I,
    zone_registry: &ZoneRegistry,
) -> (Vec<PlayerProfile>, HashMap<String, u32>)
where
    I: IntoIterator<Item = (&'a str, Uuid, DVec3, Option<&'a PlayerState>)>,
{
    let mut player_counts = HashMap::new();

    let players = players
        .into_iter()
        .map(|(name, unique_id, position, player_state)| {
            let zone_name = zone_registry.find_zone_or_default(position).name.clone();
            *player_counts.entry(zone_name.clone()).or_insert(0) += 1;

            let normalized_state = player_state
                .map(PlayerState::normalized)
                .unwrap_or_default();
            let breakdown = normalized_state.power_breakdown();
            let composite_power = normalized_state.composite_power();

            PlayerProfile {
                uuid: format!("offline:{unique_id}"),
                name: name.to_string(),
                realm: normalized_state.realm,
                composite_power,
                breakdown,
                trend: crate::schema::common::PlayerTrend::Stable,
                active_hours: 0.0,
                zone: zone_name,
                pos: [position.x, position.y, position.z],
                recent_kills: 0,
                recent_deaths: 0,
            }
        })
        .collect();

    (players, player_counts)
}

fn collect_zone_snapshots(
    zone_registry: &ZoneRegistry,
    player_counts: &HashMap<String, u32>,
) -> Vec<ZoneSnapshot> {
    zone_registry
        .zones()
        .iter()
        .map(|zone| ZoneSnapshot {
            name: zone.name.clone(),
            spirit_qi: zone.spirit_qi,
            danger_level: zone.danger_level,
            active_events: zone.active_events.clone(),
            player_count: player_counts.get(&zone.name).copied().unwrap_or(0),
        })
        .collect()
}

fn collect_recent_events_for_world_state(
    active_events: &ActiveWorldEventsResource,
    tick: u64,
) -> Vec<WorldGameEvent> {
    active_events
        .thunder_tribulations
        .iter()
        .map(|event| map_thunder_event_to_game_event(event, tick))
        .collect()
}

fn map_thunder_event_to_game_event(
    event: &ActiveThunderTribulationEvent,
    tick: u64,
) -> WorldGameEvent {
    let mut details = std::collections::HashMap::new();
    details.insert(
        "intensity".to_string(),
        serde_json::Value::from(event.intensity),
    );
    details.insert(
        "duration_ticks".to_string(),
        serde_json::Value::from(event.duration_ticks),
    );
    details.insert(
        "batch_id".to_string(),
        serde_json::Value::from(event.command_batch_id.clone()),
    );

    WorldGameEvent {
        event_type: crate::schema::common::GameEventType::EventTriggered,
        tick,
        player: event.target_player.clone(),
        target: Some("thunder_tribulation".to_string()),
        zone: Some(event.zone.clone()),
        details: Some(details),
    }
}

fn serialize_checked_payload(payload: ClientPayloadV1) -> Result<Vec<u8>, PayloadBuildError> {
    let bytes = serde_json::to_vec(&payload).map_err(PayloadBuildError::Json)?;
    let max = crate::schema::common::MAX_PAYLOAD_BYTES;
    if bytes.len() > max {
        return Err(PayloadBuildError::Oversize {
            size: bytes.len(),
            max,
        });
    }

    Ok(bytes)
}

fn build_narration_payload(
    narration: &crate::schema::narration::Narration,
) -> Result<Vec<u8>, PayloadBuildError> {
    serialize_checked_payload(ClientPayloadV1::Narration {
        v: 1,
        narrations: vec![narration.clone()],
    })
}

fn build_zone_info_payload(zone: &crate::world::zone::Zone) -> Result<Vec<u8>, PayloadBuildError> {
    serialize_checked_payload(ClientPayloadV1::ZoneInfo {
        v: 1,
        zone_info: ZoneInfoPayload {
            zone: zone.name.clone(),
            spirit_qi: zone.spirit_qi,
            danger_level: zone.danger_level,
            active_events: Some(zone.active_events.clone()),
        },
    })
}

fn build_event_alert_payload(
    event: &ActiveThunderTribulationEvent,
) -> Result<Vec<u8>, PayloadBuildError> {
    let target_suffix = event
        .target_player
        .as_deref()
        .map(|target| format!("，目标：{target}"))
        .unwrap_or_default();

    serialize_checked_payload(ClientPayloadV1::EventAlert {
        v: 1,
        event_alert: EventAlertPayload {
            kind: EventKind::ThunderTribulation,
            title: "雷劫将至".to_string(),
            detail: format!(
                "{} 区域将持续 {} ticks 雷劫，强度 {:.2}{}",
                event.zone, event.duration_ticks, event.intensity, target_suffix
            ),
            severity: EventAlertSeverity::Critical,
            zone: Some(event.zone.clone()),
        },
    })
}

pub(crate) fn build_player_state_payload(
    player_state: &PlayerState,
    zone: &str,
) -> Result<Vec<u8>, PayloadBuildError> {
    let normalized = player_state.normalized();
    let composite_power = normalized.composite_power();
    let payload = PlayerStatePayload {
        realm: normalized.realm,
        spirit_qi: normalized.spirit_qi,
        spirit_qi_max: normalized.spirit_qi_max,
        karma: normalized.karma,
        composite_power,
        zone: zone.to_string(),
    };

    serialize_checked_payload(ClientPayloadV1::PlayerState {
        v: 1,
        player_state: payload,
    })
}

fn process_redis_inbound(
    redis: Res<RedisBridgeResource>,
    mut clients: Query<(Entity, &mut Client, &valence::prelude::Position)>,
    mut command_executor: valence::prelude::ResMut<CommandExecutorResource>,
    zone_registry: Res<ZoneRegistry>,
    active_events: Res<ActiveWorldEventsResource>,
    mut emission_state: valence::prelude::ResMut<EmissionState>,
) {
    emit_zone_info_for_zone_changes(&zone_registry, &mut clients, emission_state.as_mut());
    emit_new_event_alerts(
        active_events.as_ref(),
        &mut clients,
        emission_state.as_mut(),
    );

    while let Ok(msg) = redis.rx_inbound.try_recv() {
        match msg {
            RedisInbound::AgentCommand(cmd) => {
                tracing::info!(
                    "[bong][network] received agent command batch {}: {} commands from {:?}",
                    cmd.id,
                    cmd.commands.len(),
                    cmd.source
                );
                for c in &cmd.commands {
                    tracing::info!(
                        "[bong][network]   cmd: {:?} → {} params={:?}",
                        c.command_type,
                        c.target,
                        c.params
                    );
                }

                let enqueued =
                    validate_and_enqueue_agent_command_batch(command_executor.as_mut(), cmd);
                tracing::info!(
                    "[bong][network] enqueued {} agent commands (pending: {})",
                    enqueued,
                    command_executor.pending_count()
                );
            }
            RedisInbound::AgentNarration(narr) => {
                for single in &narr.narrations {
                    let payload = match build_narration_payload(single) {
                        Ok(payload) => payload,
                        Err(error) => {
                            log_payload_build_error("narration", &error);
                            continue;
                        }
                    };

                    let mut sent_clients = 0usize;
                    for (_, mut client, _) in clients.iter_mut() {
                        send_server_data_payload(&mut client, payload.as_slice());
                        sent_clients += 1;
                    }

                    tracing::info!(
                        "[bong][network] sent bong:server_data narration payload: 1 narrations, {} bytes, {} clients",
                        payload.len(),
                        sent_clients
                    );
                }
            }
        }
    }
}

fn emit_zone_info_for_zone_changes(
    zone_registry: &ZoneRegistry,
    clients: &mut Query<(Entity, &mut Client, &valence::prelude::Position)>,
    emission_state: &mut EmissionState,
) {
    let mut alive_clients = HashSet::new();
    for (entity, mut client, position) in clients.iter_mut() {
        alive_clients.insert(entity);

        let zone_name = zone_registry
            .find_zone_or_default(position.get())
            .name
            .clone();
        if should_emit_zone_info_for_client(emission_state, entity, &zone_name) {
            if let Some(zone) = zone_registry.get_zone(&zone_name) {
                match build_zone_info_payload(zone) {
                    Ok(payload) => {
                        send_server_data_payload(&mut client, payload.as_slice());
                    }
                    Err(error) => log_payload_build_error("zone_info", &error),
                }
            }
        }
    }

    emission_state
        .last_zone_by_client
        .retain(|entity, _| alive_clients.contains(entity));
}

fn should_emit_zone_info_for_client(
    emission_state: &mut EmissionState,
    entity: Entity,
    zone_name: &str,
) -> bool {
    match emission_state.last_zone_by_client.get(&entity) {
        Some(previous_zone) if previous_zone == zone_name => false,
        _ => {
            emission_state
                .last_zone_by_client
                .insert(entity, zone_name.to_string());
            true
        }
    }
}

fn event_announcement_key(event: &ActiveThunderTribulationEvent) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        event.command_batch_id,
        event.zone,
        event.duration_ticks,
        event.intensity,
        event.target_player.as_deref().unwrap_or_default()
    )
}

fn emit_new_event_alerts(
    active_events: &ActiveWorldEventsResource,
    clients: &mut Query<(Entity, &mut Client, &valence::prelude::Position)>,
    emission_state: &mut EmissionState,
) {
    let mut current_event_keys = HashSet::new();

    for event in &active_events.thunder_tribulations {
        let key = event_announcement_key(event);
        current_event_keys.insert(key.clone());
        if emission_state.announced_events.contains(&key) {
            continue;
        }

        match build_event_alert_payload(event) {
            Ok(payload) => {
                for (_, mut client, _) in clients.iter_mut() {
                    send_server_data_payload(&mut client, payload.as_slice());
                }
                emission_state.announced_events.insert(key);
            }
            Err(error) => log_payload_build_error("event_alert", &error),
        }
    }

    emission_state
        .announced_events
        .retain(|key| current_event_keys.contains(key));
}

type PlayerStateEmitQueryItem<'a> = (
    Entity,
    &'a valence::prelude::Username,
    &'a valence::prelude::UniqueId,
    &'a valence::prelude::Position,
    valence::prelude::Ref<'a, PlayerState>,
    &'a mut Client,
);

type PlayerStateEmitQueryFilter = valence::prelude::With<Client>;

fn emit_player_state_payloads_for_changes(
    zone_registry: Res<ZoneRegistry>,
    mut tick_counter: valence::prelude::Local<u64>,
    mut clients: Query<PlayerStateEmitQueryItem<'_>, PlayerStateEmitQueryFilter>,
) {
    *tick_counter += 1;
    let periodic_emit = tick_counter.is_multiple_of(WORLD_STATE_PUBLISH_INTERVAL_TICKS);

    for (entity, username, unique_id, position, player_state, mut client) in &mut clients {
        if !(periodic_emit || player_state.is_added() || player_state.is_changed()) {
            continue;
        }

        let zone_name = zone_registry
            .as_ref()
            .find_zone_or_default(position.get())
            .name
            .clone();
        let payload = match build_player_state_payload(&player_state, zone_name.as_str()) {
            Ok(payload) => payload,
            Err(error) => {
                log_payload_build_error("player_state", &error);
                continue;
            }
        };

        send_server_data_payload(&mut client, payload.as_slice());
        tracing::info!(
            "[bong][network] sent bong:server_data player_state payload to client entity {entity:?} user={} uuid={} ({} bytes)",
            username.0,
            unique_id.0,
            payload.len(),
        );
    }
}

fn send_welcome_payload_on_join(mut joined_clients: Query<(Entity, &mut Client), Added<Client>>) {
    let payload = match build_welcome_payload() {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error("welcome", &error);
            return;
        }
    };

    for (entity, mut client) in &mut joined_clients {
        send_server_data_payload(&mut client, payload.as_slice());
        tracing::info!(
            "[bong][network] sent bong:server_data welcome payload to client entity {entity:?}"
        );
    }
}

fn process_bridge_messages(bridge: Res<NetworkBridgeResource>, mut clients: Query<&mut Client>) {
    let heartbeat_payload = match build_heartbeat_payload() {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error("heartbeat", &error);
            return;
        }
    };

    drain_bridge_commands(&bridge, || {
        for mut client in &mut clients {
            send_server_data_payload(&mut client, heartbeat_payload.as_slice());
        }
    });
}

fn send_server_data_payload(client: &mut Client, payload: &[u8]) {
    client.send_custom_payload(ident!("bong:server_data"), payload);
}

fn drain_bridge_commands(bridge: &NetworkBridgeResource, mut on_heartbeat: impl FnMut()) -> usize {
    let mut drained_messages = 0;

    while let Ok(command) = bridge.rx_from_agent.try_recv() {
        drained_messages += 1;

        match command {
            AgentCommand::Heartbeat => on_heartbeat(),
        }

        let _ = bridge.tx_to_agent.send(GameEvent::Placeholder);
    }

    drained_messages
}

fn log_payload_build_error(payload_type: &str, error: &PayloadBuildError) {
    match error {
        PayloadBuildError::Json(json_error) => tracing::error!(
            "[bong][network] failed to serialize {payload_type} payload for {}: {json_error}",
            SERVER_DATA_CHANNEL
        ),
        PayloadBuildError::Oversize { size, max } => {
            if payload_type == "player_state" {
                tracing::warn!(
                    "[bong][network] {payload_type} payload for {} rejected as oversize: {size} > {max}",
                    SERVER_DATA_CHANNEL
                );
            } else {
                tracing::error!(
                    "[bong][network] {payload_type} payload for {} rejected as oversize: {size} > {max}",
                    SERVER_DATA_CHANNEL
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{bounded, unbounded};
    use std::collections::HashSet;
    use std::time::Duration;
    use valence::prelude::Position;
    use valence::prelude::{DVec3, Uuid};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::create_mock_client;

    use crate::schema::client_payload::ClientPayloadV1;
    use crate::schema::common::{NarrationScope, NarrationStyle};
    use crate::schema::narration::Narration;
    use crate::world::DEFAULT_SPAWN_ZONE;

    #[test]
    fn bridge_drain_is_non_blocking() {
        let (tx_to_agent, _rx_to_agent) = unbounded::<GameEvent>();
        let (_tx_from_agent, rx_from_agent) = unbounded::<AgentCommand>();
        let bridge = NetworkBridgeResource::new(tx_to_agent, rx_from_agent);

        let (done_tx, done_rx) = bounded::<usize>(1);

        std::thread::spawn(move || {
            let drained = drain_bridge_commands(&bridge, || {});
            let _ = done_tx.send(drained);
        });

        let drained = done_rx
            .recv_timeout(Duration::from_millis(100))
            .expect("drain should return immediately when channel is empty");

        assert_eq!(drained, 0);
    }

    #[test]
    fn world_state_uses_real_identity_not_placeholder() {
        let registry = ZoneRegistry::fallback();
        let player_uuid = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000")
            .expect("uuid literal should parse");
        let (players, _counts) = collect_players_for_world_state(
            [(
                "Steve",
                player_uuid,
                DVec3::new(
                    crate::world::DEFAULT_SPAWN_POSITION[0],
                    crate::world::DEFAULT_SPAWN_POSITION[1],
                    crate::world::DEFAULT_SPAWN_POSITION[2],
                ),
                None,
            )],
            &registry,
        );

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].name, "Steve");
        assert_eq!(
            players[0].uuid,
            "offline:123e4567-e89b-12d3-a456-426614174000".to_string()
        );
        assert_ne!(players[0].name, "Player0");
        assert_ne!(players[0].uuid, "offline:player_0");
    }

    #[test]
    fn world_state_zone_data_comes_from_registry() {
        let registry = ZoneRegistry::fallback();
        let (players, player_counts) = collect_players_for_world_state(
            [(
                "Steve",
                Uuid::nil(),
                DVec3::new(
                    crate::world::DEFAULT_SPAWN_POSITION[0],
                    crate::world::DEFAULT_SPAWN_POSITION[1],
                    crate::world::DEFAULT_SPAWN_POSITION[2],
                ),
                None,
            )],
            &registry,
        );
        let zones = collect_zone_snapshots(&registry, &player_counts);

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].zone, DEFAULT_SPAWN_ZONE);
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].name, DEFAULT_SPAWN_ZONE);
        assert_eq!(zones[0].player_count, 1);
        assert_eq!(zones[0].spirit_qi, 0.9);
        assert_eq!(zones[0].danger_level, 0);
    }

    #[test]
    fn world_state_projection_uses_player_state_not_hardcoded_defaults() {
        let registry = ZoneRegistry::fallback();
        let attached = PlayerState {
            realm: "qi_refining_3".to_string(),
            spirit_qi: 78.0,
            spirit_qi_max: 100.0,
            karma: 0.2,
            experience: 1_200,
            inventory_score: 0.4,
        };

        let (players, _counts) = collect_players_for_world_state(
            [(
                "Azure",
                Uuid::parse_str("123e4567-e89b-12d3-a456-426614174333").unwrap(),
                DVec3::new(
                    crate::world::DEFAULT_SPAWN_POSITION[0],
                    crate::world::DEFAULT_SPAWN_POSITION[1],
                    crate::world::DEFAULT_SPAWN_POSITION[2],
                ),
                Some(&attached),
            )],
            &registry,
        );

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].realm, "qi_refining_3");
        assert_ne!(players[0].composite_power, 0.1);
        assert_ne!(players[0].breakdown.combat, 0.1);
        assert_ne!(players[0].breakdown.wealth, 0.1);
    }

    #[test]
    fn payload_builder_zone_info_happy_path() {
        let registry = ZoneRegistry::fallback();
        let zone = registry
            .zones()
            .first()
            .expect("fallback zone should exist");
        let payload = build_zone_info_payload(zone).expect("zone_info payload should build");

        let decoded: ClientPayloadV1 =
            serde_json::from_slice(&payload).expect("zone_info payload should decode");

        match decoded {
            ClientPayloadV1::ZoneInfo { v, zone_info } => {
                assert_eq!(v, 1);
                assert_eq!(zone_info.zone, zone.name);
                assert_eq!(zone_info.danger_level, zone.danger_level);
            }
            other => panic!("expected zone_info payload, got {other:?}"),
        }
    }

    #[test]
    fn payload_builder_event_alert_happy_path() {
        let event = ActiveThunderTribulationEvent {
            command_batch_id: "cmd_1".to_string(),
            source: Some("tests".to_string()),
            zone: "spawn".to_string(),
            intensity: 0.7,
            duration_ticks: 400,
            target_player: Some("offline:Steve".to_string()),
        };
        let payload = build_event_alert_payload(&event).expect("event_alert payload should build");

        let decoded: ClientPayloadV1 =
            serde_json::from_slice(&payload).expect("event_alert payload should decode");

        match decoded {
            ClientPayloadV1::EventAlert { v, event_alert } => {
                assert_eq!(v, 1);
                assert_eq!(event_alert.kind, EventKind::ThunderTribulation);
                assert_eq!(event_alert.zone.as_deref(), Some("spawn"));
                assert_eq!(event_alert.severity, EventAlertSeverity::Critical);
            }
            other => panic!("expected event_alert payload, got {other:?}"),
        }
    }

    #[test]
    fn payload_builder_player_state_happy_path() {
        let payload = build_player_state_payload(
            &PlayerState {
                realm: "qi_refining_3".to_string(),
                spirit_qi: 78.0,
                spirit_qi_max: 100.0,
                karma: -0.2,
                experience: 1_200,
                inventory_score: 0.4,
            },
            "blood_valley",
        )
        .expect("player_state payload should build");

        let decoded: ClientPayloadV1 =
            serde_json::from_slice(&payload).expect("player_state payload should decode");

        match decoded {
            ClientPayloadV1::PlayerState { v, player_state } => {
                assert_eq!(v, 1);
                assert_eq!(player_state.realm, "qi_refining_3");
                assert_eq!(player_state.spirit_qi, 78.0);
                assert_eq!(player_state.zone, "blood_valley");
            }
            other => panic!("expected player_state payload, got {other:?}"),
        }
    }

    #[test]
    fn event_alert_gating_only_emits_new_event_keys() {
        let event = ActiveThunderTribulationEvent {
            command_batch_id: "cmd_42".to_string(),
            source: None,
            zone: "spawn".to_string(),
            intensity: 0.5,
            duration_ticks: 200,
            target_player: None,
        };
        let key = event_announcement_key(&event);

        let mut state = EmissionState::default();
        assert!(!state.announced_events.contains(&key));

        state.announced_events.insert(key.clone());
        let should_skip_duplicate = state.announced_events.contains(&key);
        assert!(should_skip_duplicate);
    }

    #[test]
    fn event_alert_gating_allows_reannounce_after_event_disappears() {
        let event = ActiveThunderTribulationEvent {
            command_batch_id: "cmd_007".to_string(),
            source: Some("tiandao".to_string()),
            zone: "spawn".to_string(),
            intensity: 0.8,
            duration_ticks: 300,
            target_player: Some("offline:Steve".to_string()),
        };
        let key = event_announcement_key(&event);

        let mut state = EmissionState::default();
        state.announced_events.insert(key.clone());

        let current_event_keys = HashSet::<String>::new();
        state
            .announced_events
            .retain(|existing| current_event_keys.contains(existing));

        assert!(!state.announced_events.contains(&key));
    }

    #[test]
    fn zone_gating_detects_join_then_zone_change_without_repeat() {
        let mut state = EmissionState::default();
        let entity = Entity::from_raw(7);

        state
            .last_zone_by_client
            .insert(entity, "spawn".to_string());

        let first_observed = "spawn".to_string();
        let should_emit_same_zone = match state.last_zone_by_client.get(&entity) {
            Some(previous_zone) => previous_zone != &first_observed,
            None => false,
        };
        assert!(!should_emit_same_zone);

        let next_zone = "blood_valley".to_string();
        let should_emit_new_zone = match state.last_zone_by_client.get(&entity) {
            Some(previous_zone) => previous_zone != &next_zone,
            None => false,
        };
        assert!(should_emit_new_zone);
    }

    #[test]
    fn should_emit_zone_info_emits_once_then_on_change() {
        let mut state = EmissionState::default();
        let entity = Entity::from_raw(11);

        assert!(should_emit_zone_info_for_client(
            &mut state, entity, "spawn"
        ));
        assert!(!should_emit_zone_info_for_client(
            &mut state, entity, "spawn"
        ));
        assert!(should_emit_zone_info_for_client(
            &mut state,
            entity,
            "blood_valley"
        ));
    }

    #[test]
    fn payload_builder_zone_info_rejects_oversize() {
        let payload = ClientPayloadV1::ZoneInfo {
            v: 1,
            zone_info: ZoneInfoPayload {
                zone: "spawn".to_string(),
                spirit_qi: 0.9,
                danger_level: 1,
                active_events: Some(vec!["x".repeat(crate::schema::common::MAX_PAYLOAD_BYTES)]),
            },
        };

        let err = serialize_checked_payload(payload).expect_err("oversize zone_info should fail");
        match err {
            PayloadBuildError::Oversize { size, max } => assert!(size > max),
            other => panic!("expected oversize error, got {other:?}"),
        }
    }

    #[test]
    fn payload_builder_event_alert_rejects_oversize() {
        let payload = ClientPayloadV1::EventAlert {
            v: 1,
            event_alert: EventAlertPayload {
                kind: EventKind::ThunderTribulation,
                title: "雷劫".to_string(),
                detail: "x".repeat(crate::schema::common::MAX_PAYLOAD_BYTES),
                severity: EventAlertSeverity::Critical,
                zone: Some("spawn".to_string()),
            },
        };

        let err = serialize_checked_payload(payload).expect_err("oversize event_alert should fail");
        match err {
            PayloadBuildError::Oversize { size, max } => assert!(size > max),
            other => panic!("expected oversize error, got {other:?}"),
        }
    }

    #[test]
    fn payload_builder_player_state_rejects_oversize() {
        let payload = build_player_state_payload(
            &PlayerState {
                realm: "x".repeat(crate::schema::common::MAX_PAYLOAD_BYTES),
                spirit_qi: 78.0,
                spirit_qi_max: 100.0,
                karma: 0.0,
                experience: 0,
                inventory_score: 0.0,
            },
            "spawn",
        )
        .expect_err("oversize player_state should fail");

        match payload {
            PayloadBuildError::Oversize { size, max } => assert!(size > max),
            other => panic!("expected oversize error, got {other:?}"),
        }
    }

    fn flush_all_client_packets(world: &mut valence::prelude::World) {
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_player_state_payloads(
        helper: &mut valence::testing::MockClientHelper,
    ) -> Vec<ClientPayloadV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };

            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }

            let payload: ClientPayloadV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("typed payload should decode as ClientPayloadV1");

            if matches!(payload, ClientPayloadV1::PlayerState { .. }) {
                payloads.push(payload);
            }
        }

        payloads
    }

    #[test]
    fn missing_target_route_player_state_does_not_broadcast_to_all_clients() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, emit_player_state_payloads_for_changes);

        let (mut azure_bundle, mut azure_helper) = create_mock_client("Azure");
        azure_bundle.player.position = Position::new(crate::world::DEFAULT_SPAWN_POSITION);
        let azure_entity = app
            .world_mut()
            .spawn((
                azure_bundle,
                PlayerState {
                    realm: "qi_refining_3".to_string(),
                    spirit_qi: 78.0,
                    spirit_qi_max: 100.0,
                    karma: 0.0,
                    experience: 0,
                    inventory_score: 0.0,
                },
            ))
            .id();

        let (mut bob_bundle, mut bob_helper) = create_mock_client("Bob");
        bob_bundle.player.position = Position::new(crate::world::DEFAULT_SPAWN_POSITION);
        let _bob_entity = app
            .world_mut()
            .spawn((
                bob_bundle,
                PlayerState {
                    realm: "mortal".to_string(),
                    spirit_qi: 0.0,
                    spirit_qi_max: 100.0,
                    karma: 0.0,
                    experience: 0,
                    inventory_score: 0.0,
                },
            ))
            .id();

        app.update();
        flush_all_client_packets(app.world_mut());
        let _ = collect_player_state_payloads(&mut azure_helper);
        let _ = collect_player_state_payloads(&mut bob_helper);

        {
            let mut query = app.world_mut().query::<&mut PlayerState>();
            let mut azure_state = query
                .get_mut(app.world_mut(), azure_entity)
                .expect("azure state should be mutable");
            azure_state.spirit_qi = 81.0;
        }

        app.update();
        flush_all_client_packets(app.world_mut());

        let azure_payloads = collect_player_state_payloads(&mut azure_helper);
        let bob_payloads = collect_player_state_payloads(&mut bob_helper);

        assert_eq!(
            azure_payloads.len(),
            1,
            "changed target should receive one payload"
        );
        assert!(
            bob_payloads.is_empty(),
            "missing-route/fallthrough must not broadcast player_state to other clients"
        );
    }

    #[test]
    fn player_state_periodic_emission_happens_without_component_change() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, emit_player_state_payloads_for_changes);

        let (mut azure_bundle, mut azure_helper) = create_mock_client("Azure");
        azure_bundle.player.position = Position::new(crate::world::DEFAULT_SPAWN_POSITION);
        app.world_mut().spawn((
            azure_bundle,
            PlayerState {
                realm: "qi_refining_3".to_string(),
                spirit_qi: 78.0,
                spirit_qi_max: 100.0,
                karma: 0.0,
                experience: 0,
                inventory_score: 0.0,
            },
        ));

        app.update();
        flush_all_client_packets(app.world_mut());
        let _ = collect_player_state_payloads(&mut azure_helper);

        for _ in 0..(WORLD_STATE_PUBLISH_INTERVAL_TICKS - 1) {
            app.update();
        }
        flush_all_client_packets(app.world_mut());

        let periodic_payloads = collect_player_state_payloads(&mut azure_helper);
        assert_eq!(
            periodic_payloads.len(),
            1,
            "periodic cadence should emit one player_state payload without Changed<PlayerState>"
        );
    }

    #[test]
    fn narration_payload_serializes_as_typed_server_data() {
        let payload = build_narration_payload(&Narration {
            scope: NarrationScope::Broadcast,
            target: None,
            text: "天道测试叙事".to_string(),
            style: NarrationStyle::SystemWarning,
        })
        .expect("narration payload should serialize");

        let decoded: ClientPayloadV1 =
            serde_json::from_slice(&payload).expect("serialized payload should decode");

        match decoded {
            ClientPayloadV1::Narration { v, narrations } => {
                assert_eq!(v, 1);
                assert_eq!(narrations.len(), 1);
                assert_eq!(narrations[0].text, "天道测试叙事");
                assert_eq!(narrations[0].style, NarrationStyle::SystemWarning);
                assert_eq!(narrations[0].scope, NarrationScope::Broadcast);
            }
            other => panic!("expected narration payload, got {other:?}"),
        }
    }

    #[test]
    fn narration_payload_always_contains_single_item() {
        let payload = build_narration_payload(&Narration {
            scope: NarrationScope::Player,
            target: Some("offline:player_1".to_string()),
            text: "只应单条发送".to_string(),
            style: NarrationStyle::Narration,
        })
        .expect("narration payload should serialize");

        let decoded: ClientPayloadV1 =
            serde_json::from_slice(&payload).expect("serialized payload should decode");

        match decoded {
            ClientPayloadV1::Narration { narrations, .. } => {
                assert_eq!(narrations.len(), 1);
                assert_eq!(narrations[0].text, "只应单条发送");
                assert_eq!(narrations[0].target.as_deref(), Some("offline:player_1"));
            }
            other => panic!("expected narration payload, got {other:?}"),
        }
    }
}
