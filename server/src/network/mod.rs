pub mod agent_bridge;
pub mod chat_collector;
pub mod combat_bridge;
pub mod command_executor;
pub mod client_request_handler;
pub mod cultivation_bridge;
pub mod cultivation_detail_emit;
pub mod redis_bridge;
pub mod vfx_event_emit;

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_bridge::{
    payload_type_label, route_recipient_indices, serialize_server_data_payload, AgentCommand,
    GameEvent, NetworkBridgeResource, PayloadBuildError, RecipientMetadata, RecipientSelector,
    SERVER_DATA_CHANNEL,
};
use big_brain::prelude::{ActionState, Actor};
use chat_collector::{collect_player_chat, ChatCollectorRateLimit};
use command_executor::{execute_agent_commands, CommandExecutorResource};
use redis_bridge::{RedisInbound, RedisOutbound};
use valence::prelude::{
    ident, Added, App, Changed, Client, Commands, Entity, EntityKind, EventWriter,
    IntoSystemConfigs, Or, Position, Query, Res, Resource, Update, Username, With,
};

use crate::cultivation::components::{Cultivation, MeridianSystem, QiColor};
use crate::cultivation::life_record::LifeRecord;
use crate::npc::brain::{canonical_npc_id, ChaseAction, DashAction, FleeAction, MeleeAttackAction};
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::player::state::{canonical_player_id, PlayerState};
use crate::schema::common::{EventKind, NpcStateKind, PlayerTrend};
use crate::schema::cultivation::{CultivationSnapshotV1, LifeRecordSnapshotV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::world_state::{NpcSnapshot, PlayerProfile, WorldStateV1, ZoneSnapshot};
use crate::world::events::ActiveEventsResource;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const REDIS_URL_ENV_KEY: &str = "REDIS_URL";
const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379";
const WORLD_STATE_PUBLISH_INTERVAL_TICKS: u64 = 200; // ~10 seconds at 20 TPS
const REDIS_INBOUND_DRAIN_BUDGET: usize = 16;
const DEFAULT_PLAYER_ACTIVE_HOURS: f64 = 0.0;
const DEFAULT_PLAYER_RECENT_KILLS: u32 = 0;
const DEFAULT_PLAYER_RECENT_DEATHS: u32 = 0;
const NARRATION_DEDUPE_WINDOW_SECS: u64 = 15;
const NARRATION_DEDUPE_CAPACITY: usize = 512;

/// Resource holding the Redis bridge channels
pub struct RedisBridgeResource {
    pub tx_outbound: crossbeam_channel::Sender<RedisOutbound>,
    pub rx_inbound: crossbeam_channel::Receiver<RedisInbound>,
}

impl Resource for RedisBridgeResource {}

/// Tick counter for world state publishing
#[derive(Default)]
pub struct WorldStateTimer {
    ticks: u64,
}

impl Resource for WorldStateTimer {}

#[derive(Default)]
struct ZoneTransitionTracker {
    last_zone_by_entity: HashMap<Entity, String>,
}

impl Resource for ZoneTransitionTracker {}

#[derive(Default)]
struct NarrationDedupeResource {
    recent_payload_keys: VecDeque<(String, u64)>,
}

impl Resource for NarrationDedupeResource {}

impl NarrationDedupeResource {
    fn should_drop(&mut self, payload_key: &str, now_secs: u64) -> bool {
        self.prune(now_secs);

        if self
            .recent_payload_keys
            .iter()
            .any(|(key, _)| key == payload_key)
        {
            return true;
        }

        self.recent_payload_keys
            .push_back((payload_key.to_string(), now_secs));
        while self.recent_payload_keys.len() > NARRATION_DEDUPE_CAPACITY {
            self.recent_payload_keys.pop_front();
        }

        false
    }

    fn prune(&mut self, now_secs: u64) {
        while let Some((_, seen_at_secs)) = self.recent_payload_keys.front() {
            let age_secs = now_secs.saturating_sub(*seen_at_secs);
            if age_secs > NARRATION_DEDUPE_WINDOW_SECS {
                self.recent_payload_keys.pop_front();
                continue;
            }
            break;
        }

        while self.recent_payload_keys.len() > NARRATION_DEDUPE_CAPACITY {
            self.recent_payload_keys.pop_front();
        }
    }
}

pub fn register(app: &mut App) {
    // Legacy mock bridge systems
    app.add_systems(
        Update,
        (send_welcome_payload_on_join, process_bridge_messages),
    );

    // Redis bridge
    let redis_url = redis_url_from_env();
    tracing::info!(
        "[bong][redis] configured redis endpoint: {}",
        redact_redis_url_for_log(redis_url.as_str())
    );
    let (handle, tx_outbound, rx_inbound) = redis_bridge::spawn_redis_bridge(redis_url.as_str());
    std::mem::drop(handle); // detach thread

    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.insert_resource(WorldStateTimer::default());
    app.insert_resource(ZoneTransitionTracker::default());
    app.insert_resource(ChatCollectorRateLimit::default());
    app.insert_resource(CommandExecutorResource::default());
    app.insert_resource(NarrationDedupeResource::default());
    app.insert_resource(combat_bridge::CombatSummaryAccumulator::default());

    app.add_systems(
        Update,
        (
            publish_world_state_to_redis,
            collect_player_chat,
            process_redis_inbound,
            execute_agent_commands.after(process_redis_inbound),
            emit_gameplay_narrations.after(crate::player::gameplay::apply_queued_gameplay_actions),
            emit_player_state_payloads
                .after(crate::player::attach_player_state_to_joined_clients)
                .after(crate::player::gameplay::apply_queued_gameplay_actions),
            emit_zone_info_on_zone_transition,
            emit_event_alerts_on_major_event_creation.after(execute_agent_commands),
            combat_bridge::publish_combat_realtime_events
                .after(crate::combat::resolve::resolve_attack_intents),
            combat_bridge::publish_combat_summary_on_interval.after(publish_world_state_to_redis),
            cultivation_bridge::publish_breakthrough_events,
            cultivation_bridge::publish_forge_events,
            cultivation_bridge::publish_cultivation_death_events,
            cultivation_bridge::publish_insight_requests,
            client_request_handler::handle_client_request_payloads,
            cultivation_detail_emit::emit_cultivation_detail_payloads,
            vfx_event_emit::handle_vfx_debug_commands,
            vfx_event_emit::emit_vfx_event_payloads
                .after(vfx_event_emit::handle_vfx_debug_commands),
        ),
    );
    app.init_resource::<cultivation_detail_emit::CultivationDetailEmitState>();
    app.add_event::<vfx_event_emit::VfxEventRequest>();
}

fn redis_url_from_env() -> String {
    resolve_redis_url(std::env::var(REDIS_URL_ENV_KEY).ok())
}

fn resolve_redis_url(env_value: Option<String>) -> String {
    env_value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_REDIS_URL.to_string())
}

fn redact_redis_url_for_log(redis_url: &str) -> String {
    let Some(scheme_index) = redis_url.find("://") else {
        return "[redacted redis endpoint]".to_string();
    };

    let authority_and_path = &redis_url[(scheme_index + 3)..];
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let endpoint = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority)
        .trim();

    if endpoint.is_empty() {
        "[redacted redis endpoint]".to_string()
    } else {
        endpoint.to_string()
    }
}

/// Periodically publish world state snapshot to Redis
#[allow(clippy::too_many_arguments)]
fn publish_world_state_to_redis(
    redis: Res<RedisBridgeResource>,
    mut timer: valence::prelude::ResMut<WorldStateTimer>,
    clients: Query<(Entity, &Position, &Username, Option<&PlayerState>), With<Client>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    active_events: Option<Res<ActiveEventsResource>>,
    npcs: Query<(Entity, &Position, &NpcBlackboard, &EntityKind), With<NpcMarker>>,
    flee_actions: Query<(&Actor, &ActionState), With<FleeAction>>,
    chase_actions: Query<(&Actor, &ActionState), With<ChaseAction>>,
    melee_actions: Query<(&Actor, &ActionState), With<MeleeAttackAction>>,
    dash_actions: Query<(&Actor, &ActionState), With<DashAction>>,
    cultivation_q: Query<
        (Entity, &Cultivation, &MeridianSystem, &QiColor, &LifeRecord),
        With<Client>,
    >,
) {
    timer.ticks += 1;
    if !timer
        .ticks
        .is_multiple_of(WORLD_STATE_PUBLISH_INTERVAL_TICKS)
    {
        return;
    }

    let npc_action_states =
        collect_npc_action_states(&flee_actions, &chase_actions, &melee_actions, &dash_actions);

    let cultivation_by_entity = collect_cultivation_snapshots(&cultivation_q);

    let state = build_world_state_snapshot(
        current_unix_timestamp_secs(),
        timer.ticks,
        &clients,
        zone_registry.as_deref(),
        active_events.as_deref(),
        &npcs,
        &npc_action_states,
        &cultivation_by_entity,
    );

    let _ = redis.tx_outbound.send(RedisOutbound::WorldState(state));
}

fn collect_cultivation_snapshots(
    q: &Query<(Entity, &Cultivation, &MeridianSystem, &QiColor, &LifeRecord), With<Client>>,
) -> HashMap<Entity, (CultivationSnapshotV1, LifeRecordSnapshotV1)> {
    const RECENT_BIO_N: usize = 12;
    q.iter()
        .map(|(entity, c, m, q, life)| {
            let snap = CultivationSnapshotV1::from_components(c, m, q);
            let life_snap = LifeRecordSnapshotV1 {
                recent_biography_summary: life.recent_summary_text(RECENT_BIO_N),
            };
            (entity, (snap, life_snap))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_world_state_snapshot(
    ts: u64,
    tick: u64,
    clients: &Query<(Entity, &Position, &Username, Option<&PlayerState>), With<Client>>,
    zone_registry: Option<&ZoneRegistry>,
    active_events: Option<&ActiveEventsResource>,
    npcs: &Query<(Entity, &Position, &NpcBlackboard, &EntityKind), With<NpcMarker>>,
    npc_action_states: &HashMap<Entity, NpcStateKind>,
    cultivation_by_entity: &HashMap<Entity, (CultivationSnapshotV1, LifeRecordSnapshotV1)>,
) -> WorldStateV1 {
    let zone_registry = effective_zone_registry(zone_registry);
    let (players, player_ids_by_entity, player_counts_by_zone) =
        collect_player_snapshots(clients, &zone_registry, cultivation_by_entity);

    WorldStateV1 {
        v: 1,
        ts,
        tick,
        players,
        npcs: collect_npc_snapshots(npcs, npc_action_states, &player_ids_by_entity),
        zones: collect_zone_snapshots(&zone_registry, &player_counts_by_zone),
        recent_events: active_events
            .map(ActiveEventsResource::recent_events_snapshot)
            .unwrap_or_default(),
    }
}

#[cfg(test)]
pub(crate) fn build_player_state_payload(
    player_state: &PlayerState,
    zone: impl Into<String>,
) -> Result<Vec<u8>, PayloadBuildError> {
    let payload = player_state.server_payload(None, zone.into());
    serialize_server_data_payload(&payload)
}

#[cfg(test)]
pub(crate) fn collect_players_for_world_state<'a, I>(
    clients: I,
    zone_registry: &ZoneRegistry,
) -> (Vec<PlayerProfile>, HashMap<String, u32>)
where
    I: IntoIterator<
        Item = (
            &'a str,
            valence::prelude::Uuid,
            valence::prelude::DVec3,
            Option<&'a PlayerState>,
        ),
    >,
{
    let mut player_counts_by_zone = HashMap::new();
    let mut players = clients
        .into_iter()
        .map(|(name, _uuid, position, player_state)| {
            let zone_name = zone_name_for_position(zone_registry, position);
            let (realm, composite_power, breakdown) = player_state
                .map(|state| {
                    let normalized = state.normalized();
                    (
                        normalized.realm.clone(),
                        normalized.composite_power(),
                        normalized.power_breakdown(),
                    )
                })
                .unwrap_or_else(|| {
                    let default_state = PlayerState::default();
                    (
                        default_state.realm.clone(),
                        default_state.composite_power(),
                        default_state.power_breakdown(),
                    )
                });

            *player_counts_by_zone.entry(zone_name.clone()).or_default() += 1;

            PlayerProfile {
                uuid: canonical_player_id(name),
                name: name.to_string(),
                realm,
                composite_power,
                breakdown,
                trend: PlayerTrend::Stable,
                active_hours: DEFAULT_PLAYER_ACTIVE_HOURS,
                zone: zone_name,
                pos: vec3_to_array(position),
                recent_kills: DEFAULT_PLAYER_RECENT_KILLS,
                recent_deaths: DEFAULT_PLAYER_RECENT_DEATHS,
                cultivation: None,
                life_record: None,
            }
        })
        .collect::<Vec<_>>();

    players.sort_by(|left, right| left.uuid.cmp(&right.uuid));

    (players, player_counts_by_zone)
}

fn emit_gameplay_narrations(
    zone_registry: Option<Res<ZoneRegistry>>,
    gameplay_narrations: Option<valence::prelude::ResMut<PendingGameplayNarrations>>,
    mut clients: Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
) {
    let Some(mut gameplay_narrations) = gameplay_narrations else {
        return;
    };

    let narrations = gameplay_narrations.drain();
    if narrations.is_empty() {
        return;
    }

    process_agent_narrations(
        &mut clients,
        zone_registry.as_deref(),
        narrations.as_slice(),
    );
}

fn effective_zone_registry(zone_registry: Option<&ZoneRegistry>) -> ZoneRegistry {
    match zone_registry {
        Some(zone_registry) if !zone_registry.zones.is_empty() => zone_registry.clone(),
        _ => ZoneRegistry::fallback(),
    }
}

fn collect_player_snapshots(
    clients: &Query<(Entity, &Position, &Username, Option<&PlayerState>), With<Client>>,
    zone_registry: &ZoneRegistry,
    cultivation_by_entity: &HashMap<Entity, (CultivationSnapshotV1, LifeRecordSnapshotV1)>,
) -> (
    Vec<PlayerProfile>,
    HashMap<Entity, String>,
    HashMap<String, u32>,
) {
    let mut player_ids_by_entity = HashMap::new();
    let mut player_counts_by_zone = HashMap::new();

    let mut players = clients
        .iter()
        .map(|(entity, position, username, player_state)| {
            let name = username.0.clone();
            let zone_name = zone_name_for_position(zone_registry, position.get());
            let canonical_id = canonical_player_id(&name);
            let (realm, composite_power, breakdown) = player_state
                .map(|state| {
                    let normalized = state.normalized();
                    (
                        normalized.realm.clone(),
                        normalized.composite_power(),
                        normalized.power_breakdown(),
                    )
                })
                .unwrap_or_else(|| {
                    let default_state = PlayerState::default();
                    (
                        default_state.realm.clone(),
                        default_state.composite_power(),
                        default_state.power_breakdown(),
                    )
                });

            player_ids_by_entity.insert(entity, canonical_id.clone());
            *player_counts_by_zone.entry(zone_name.clone()).or_default() += 1;

            let (cultivation, life_record) = cultivation_by_entity
                .get(&entity)
                .cloned()
                .map(|(c, l)| (Some(c), Some(l)))
                .unwrap_or((None, None));

            PlayerProfile {
                uuid: canonical_id,
                name,
                realm,
                composite_power,
                breakdown,
                trend: PlayerTrend::Stable,
                active_hours: DEFAULT_PLAYER_ACTIVE_HOURS,
                zone: zone_name,
                pos: vec3_to_array(position.get()),
                recent_kills: DEFAULT_PLAYER_RECENT_KILLS,
                recent_deaths: DEFAULT_PLAYER_RECENT_DEATHS,
                cultivation,
                life_record,
            }
        })
        .collect::<Vec<_>>();

    players.sort_by(|left, right| left.uuid.cmp(&right.uuid));

    (players, player_ids_by_entity, player_counts_by_zone)
}

fn collect_npc_snapshots(
    npcs: &Query<(Entity, &Position, &NpcBlackboard, &EntityKind), With<NpcMarker>>,
    npc_action_states: &HashMap<Entity, NpcStateKind>,
    player_ids_by_entity: &HashMap<Entity, String>,
) -> Vec<NpcSnapshot> {
    let mut npc_snapshots = npcs
        .iter()
        .map(|(entity, position, blackboard, kind)| NpcSnapshot {
            id: canonical_npc_id(entity),
            kind: format!("{kind:?}"),
            pos: vec3_to_array(position.get()),
            state: npc_action_states
                .get(&entity)
                .cloned()
                .unwrap_or(NpcStateKind::Idle),
            blackboard: build_npc_blackboard(blackboard, player_ids_by_entity),
        })
        .collect::<Vec<_>>();

    npc_snapshots.sort_by(|left, right| left.id.cmp(&right.id));

    npc_snapshots
}

fn collect_zone_snapshots(
    zone_registry: &ZoneRegistry,
    player_counts_by_zone: &HashMap<String, u32>,
) -> Vec<ZoneSnapshot> {
    let mut zones = zone_registry
        .zones
        .iter()
        .map(|zone| ZoneSnapshot {
            name: zone.name.clone(),
            spirit_qi: zone.spirit_qi,
            danger_level: zone.danger_level,
            active_events: zone.active_events.clone(),
            player_count: player_counts_by_zone
                .get(&zone.name)
                .copied()
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    zones.sort_by(|left, right| left.name.cmp(&right.name));

    zones
}

fn collect_npc_action_states(
    flee_actions: &Query<(&Actor, &ActionState), With<FleeAction>>,
    chase_actions: &Query<(&Actor, &ActionState), With<ChaseAction>>,
    melee_actions: &Query<(&Actor, &ActionState), With<MeleeAttackAction>>,
    dash_actions: &Query<(&Actor, &ActionState), With<DashAction>>,
) -> HashMap<Entity, NpcStateKind> {
    let mut states = HashMap::new();

    // Lower priority first, higher priority overwrites.
    for (Actor(entity), action_state) in chase_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Patrolling);
        }
    }
    for (Actor(entity), action_state) in flee_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Fleeing);
        }
    }
    for (Actor(entity), action_state) in dash_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Attacking);
        }
    }
    for (Actor(entity), action_state) in melee_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Attacking);
        }
    }

    states
}

fn build_npc_blackboard(
    blackboard: &NpcBlackboard,
    player_ids_by_entity: &HashMap<Entity, String>,
) -> HashMap<String, serde_json::Value> {
    let mut snapshot = HashMap::new();

    if let Some(nearest_player) = blackboard.nearest_player {
        if let Some(player_id) = player_ids_by_entity.get(&nearest_player) {
            snapshot.insert(
                "nearest_player".to_string(),
                serde_json::Value::String(player_id.clone()),
            );
        }
    }

    snapshot
}

fn zone_name_for_position(
    zone_registry: &ZoneRegistry,
    position: valence::prelude::DVec3,
) -> String {
    zone_registry
        .find_zone(position)
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

fn current_unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn vec3_to_array(position: valence::prelude::DVec3) -> [f64; 3] {
    [position.x, position.y, position.z]
}

type PlayerStateEmitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a Position,
    &'a PlayerState,
);

type PlayerStateEmitQueryFilter = (With<Client>, Or<(Added<PlayerState>, Changed<PlayerState>)>);

fn emit_player_state_payloads(
    zone_registry: Option<Res<ZoneRegistry>>,
    mut clients: Query<PlayerStateEmitQueryItem<'_>, PlayerStateEmitQueryFilter>,
) {
    let zone_registry = effective_zone_registry(zone_registry.as_deref());

    for (entity, mut client, username, position, player_state) in &mut clients {
        let zone_name = zone_name_for_position(&zone_registry, position.get());
        let payload =
            player_state.server_payload(Some(canonical_player_id(username.0.as_str())), zone_name);
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(payload) => payload,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };

        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracing::info!(
            "[bong][network] sent {} {} payload to client entity {entity:?} for `{}`",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0,
        );
    }
}

fn emit_zone_info_on_zone_transition(
    zone_registry: Option<Res<ZoneRegistry>>,
    mut tracker: valence::prelude::ResMut<ZoneTransitionTracker>,
    mut clients: Query<(Entity, &mut Client, &Position), With<Client>>,
) {
    let zone_registry = effective_zone_registry(zone_registry.as_deref());
    let mut live_entities = HashSet::new();

    for (entity, mut client, position) in &mut clients {
        live_entities.insert(entity);

        let zone_name = zone_name_for_position(&zone_registry, position.get());
        let previous_zone = tracker.last_zone_by_entity.get(&entity);
        let transitioned = previous_zone
            .map(|last_zone| !last_zone.eq_ignore_ascii_case(zone_name.as_str()))
            .unwrap_or(true);

        if !transitioned {
            continue;
        }

        let Some(zone) = zone_registry.find_zone_by_name(zone_name.as_str()) else {
            tracing::warn!(
                "[bong][network] zone transition for entity {entity:?} resolved unknown zone `{}`",
                zone_name
            );
            tracker.last_zone_by_entity.insert(entity, zone_name);
            continue;
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::ZoneInfo {
            zone: zone.name.clone(),
            spirit_qi: zone.spirit_qi,
            danger_level: zone.danger_level,
            active_events: (!zone.active_events.is_empty()).then(|| zone.active_events.clone()),
        });
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(payload) => payload,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };

        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracker.last_zone_by_entity.insert(entity, zone_name);
    }

    tracker
        .last_zone_by_entity
        .retain(|entity, _| live_entities.contains(entity));
}

fn emit_event_alerts_on_major_event_creation(
    mut active_events: Option<valence::prelude::ResMut<ActiveEventsResource>>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    let Some(active_events) = active_events.as_deref_mut() else {
        return;
    };

    for pending_alert in active_events.drain_major_event_alerts() {
        let Some(event_kind) = event_kind_from_name(pending_alert.event_name.as_str()) else {
            tracing::warn!(
                "[bong][network] skipping unsupported major event alert `{}` for zone `{}`",
                pending_alert.event_name,
                pending_alert.zone_name
            );
            continue;
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::EventAlert {
            event: event_kind,
            message: major_event_alert_message(
                pending_alert.event_name.as_str(),
                pending_alert.zone_name.as_str(),
                pending_alert.duration_ticks,
            ),
            zone: Some(pending_alert.zone_name.clone()),
            duration_ticks: Some(pending_alert.duration_ticks),
        });
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(payload) => payload,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };

        for mut client in &mut clients {
            send_server_data_payload(&mut client, payload_bytes.as_slice());
        }
    }
}

fn event_kind_from_name(event_name: &str) -> Option<EventKind> {
    match event_name {
        crate::world::events::EVENT_THUNDER_TRIBULATION => Some(EventKind::ThunderTribulation),
        crate::world::events::EVENT_BEAST_TIDE => Some(EventKind::BeastTide),
        crate::world::events::EVENT_REALM_COLLAPSE => Some(EventKind::RealmCollapse),
        crate::world::events::EVENT_KARMA_BACKLASH => Some(EventKind::KarmaBacklash),
        _ => None,
    }
}

fn major_event_alert_message(event_name: &str, zone_name: &str, duration_ticks: u64) -> String {
    let event_label = match event_name {
        crate::world::events::EVENT_THUNDER_TRIBULATION => "天劫",
        crate::world::events::EVENT_BEAST_TIDE => "兽潮",
        crate::world::events::EVENT_REALM_COLLAPSE => "境界坍塌",
        crate::world::events::EVENT_KARMA_BACKLASH => "因果反噬",
        _ => "异变",
    };

    format!("{event_label}已在区域 {zone_name} 触发，预计持续 {duration_ticks} tick。")
}

/// Process inbound messages from Redis (agent commands + narrations)
#[allow(clippy::too_many_arguments)]
fn process_redis_inbound(
    redis: Res<RedisBridgeResource>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut clients: Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    mut command_executor: valence::prelude::ResMut<CommandExecutorResource>,
    mut narration_dedupe: valence::prelude::ResMut<NarrationDedupeResource>,
    mut commands: Commands,
    mut insight_offers: EventWriter<crate::cultivation::insight::InsightOffer>,
) {
    let mut drained_messages = 0;

    while drained_messages < REDIS_INBOUND_DRAIN_BUDGET {
        let Ok(msg) = redis.rx_inbound.try_recv() else {
            break;
        };

        drained_messages += 1;

        match msg {
            RedisInbound::AgentCommand(cmd) => {
                let command_count = cmd.commands.len();
                let batch_id = cmd.id.clone();
                let source = cmd.source.clone().unwrap_or_else(|| "unknown".to_string());
                let enqueue_outcome = command_executor.enqueue_batch(cmd);

                if enqueue_outcome.dedupe_drop {
                    tracing::info!(
                        "[bong][network] dedupe_drop batch_id={} source={} type=command_batch target=- result=dropped_duplicate_batch command_count={}",
                        batch_id,
                        source.as_str(),
                        command_count
                    );
                    continue;
                }

                tracing::info!(
                    "[bong][network] command_batch_ingress batch_id={} source={} type=command_batch target=- result=queued command_count={}",
                    batch_id,
                    source.as_str(),
                    command_count
                );
            }
            RedisInbound::AgentNarration(narr) => {
                process_agent_narrations_with_dedupe(
                    &mut clients,
                    zone_registry.as_deref(),
                    &mut narration_dedupe,
                    narr.narrations.as_slice(),
                );
            }
            RedisInbound::InsightOffer(offer) => {
                tracing::info!(
                    "[bong][network] insight_offer_received character_id={} trigger_id={} choices={}",
                    offer.character_id,
                    offer.trigger_id,
                    offer.choices.len()
                );
                let Some((entity, _, _, _)) = clients
                    .iter_mut()
                    .find(|(_, _, name, _)| name.0 == offer.character_id)
                else {
                    tracing::warn!(
                        "[bong][network] insight offer character_id={:?} not connected; dropping",
                        offer.character_id
                    );
                    continue;
                };
                let Some(choices) = crate::cultivation::insight_flow::ingest_agent_insight_offer(
                    &offer.trigger_id,
                    &offer.choices,
                ) else {
                    continue;
                };
                commands.entity(entity).insert(
                    crate::cultivation::insight_flow::PendingInsightOffer {
                        trigger_id: offer.trigger_id.clone(),
                        choices: choices.clone(),
                    },
                );
                insight_offers.send(crate::cultivation::insight::InsightOffer {
                    entity,
                    trigger_id: offer.trigger_id.clone(),
                    choices,
                });
            }
        }
    }

    if drained_messages == REDIS_INBOUND_DRAIN_BUDGET {
        tracing::debug!(
            "[bong][network] redis inbound drain hit budget {REDIS_INBOUND_DRAIN_BUDGET}; remaining messages will be handled next tick"
        );
    }
}

fn process_agent_narrations(
    clients: &mut Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    zone_registry: Option<&ZoneRegistry>,
    narrations: &[crate::schema::narration::Narration],
) {
    for narration in narrations {
        process_single_narration(clients, zone_registry, narration);
    }
}

fn process_agent_narrations_with_dedupe(
    clients: &mut Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    zone_registry: Option<&ZoneRegistry>,
    narration_dedupe: &mut NarrationDedupeResource,
    narrations: &[crate::schema::narration::Narration],
) {
    for narration in narrations {
        let dedupe_key = narration_dedupe_key(narration);
        if narration_dedupe.should_drop(dedupe_key.as_str(), current_unix_timestamp_secs()) {
            tracing::info!(
                "[bong][network] dedupe_drop batch_id=- source=agent type=narration target={:?} result=dropped_duplicate_payload scope={:?}",
                narration.target,
                narration.scope
            );
            continue;
        }

        process_single_narration(clients, zone_registry, narration);
    }
}

fn narration_dedupe_key(narration: &crate::schema::narration::Narration) -> String {
    format!(
        "scope={:?}|target={}|style={:?}|text={}",
        narration.scope,
        narration.target.as_deref().unwrap_or_default(),
        narration.style,
        narration.text
    )
}

fn process_single_narration(
    clients: &mut Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    zone_registry: Option<&ZoneRegistry>,
    narration: &crate::schema::narration::Narration,
) {
    let selector = match narration_selector(narration) {
        Some(selector) => selector,
        None => {
            tracing::warn!(
                "[bong][network] dropped narration with missing/invalid target for scope {:?}",
                narration.scope
            );
            return;
        }
    };

    let routed_targets = collect_routed_targets(clients, zone_registry, &selector);
    if routed_targets.is_empty() {
        tracing::debug!(
            "[bong][network] narration scope {:?} target {:?} matched zero recipients",
            narration.scope,
            narration.target
        );
        return;
    }

    let payload = ServerDataV1::new(ServerDataPayloadV1::Narration {
        narrations: vec![narration.clone()],
    });
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    for entity in routed_targets.iter().copied() {
        if let Ok((_, mut client, _, _)) = clients.get_mut(entity) {
            send_server_data_payload(&mut client, payload_bytes.as_slice());
        }
    }

    tracing::info!(
        "[bong][network] sent {} {} narration payload to {} recipient(s) for scope {:?} target {:?}",
        SERVER_DATA_CHANNEL,
        payload_type,
        routed_targets.len(),
        narration.scope,
        narration.target
    );
}

fn narration_selector(
    narration: &crate::schema::narration::Narration,
) -> Option<RecipientSelector> {
    match narration.scope {
        crate::schema::common::NarrationScope::Broadcast => Some(RecipientSelector::Broadcast),
        crate::schema::common::NarrationScope::Zone => narration
            .target
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .map(RecipientSelector::zone),
        crate::schema::common::NarrationScope::Player => narration
            .target
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .map(RecipientSelector::player),
    }
}

fn collect_routed_targets(
    clients: &mut Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    zone_registry: Option<&ZoneRegistry>,
    selector: &RecipientSelector,
) -> Vec<Entity> {
    let zone_registry = effective_zone_registry(zone_registry);

    let recipient_rows = clients
        .iter_mut()
        .map(|(entity, _, username, position)| {
            let computed_zone = Some(zone_name_for_position(&zone_registry, position.get()));

            (
                entity,
                RecipientMetadata {
                    username: Some(username.0.clone()),
                    zone: computed_zone,
                },
            )
        })
        .collect::<Vec<_>>();

    let recipient_metadata = recipient_rows
        .iter()
        .map(|(_, metadata)| metadata.clone())
        .collect::<Vec<_>>();

    let matched_indices = route_recipient_indices(
        selector,
        recipient_metadata.as_slice(),
        Some(&|zone_name, recipient| {
            recipient
                .zone
                .as_deref()
                .is_some_and(|zone| zone.eq_ignore_ascii_case(zone_name))
        }),
    );

    matched_indices
        .into_iter()
        .filter_map(|index| recipient_rows.get(index).map(|(entity, _)| *entity))
        .collect()
}

// ─── Legacy mock bridge systems (unchanged) ──────────────

fn send_welcome_payload_on_join(mut joined_clients: Query<(Entity, &mut Client), Added<Client>>) {
    let payload = ServerDataV1::welcome(crate::schema::server_data::WELCOME_MESSAGE);
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    for (entity, mut client) in &mut joined_clients {
        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracing::info!(
            "[bong][network] sent {} {} payload to client entity {entity:?}",
            SERVER_DATA_CHANNEL,
            payload_type,
        );
    }
}

fn process_bridge_messages(bridge: Res<NetworkBridgeResource>, mut clients: Query<&mut Client>) {
    let payload = ServerDataV1::heartbeat(crate::schema::server_data::HEARTBEAT_MESSAGE);
    let payload_type = payload_type_label(payload.payload_type());
    let heartbeat_payload = match serialize_server_data_payload(&payload) {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
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
        PayloadBuildError::Oversize { size, max } => tracing::error!(
            "[bong][network] {payload_type} payload for {} rejected as oversize: {size} > {max}",
            SERVER_DATA_CHANNEL
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{bounded, unbounded, Receiver};
    use std::time::Duration;
    use valence::testing::create_mock_client;

    fn assert_approx_eq(left: f64, right: f64) {
        assert!(
            (left - right).abs() < 1e-9,
            "expected {left} to be approximately equal to {right}"
        );
    }

    #[test]
    fn resolve_redis_url_prefers_non_empty_env_value() {
        let value = resolve_redis_url(Some("redis://10.0.0.8:6380".to_string()));
        assert_eq!(value, "redis://10.0.0.8:6380");
    }

    #[test]
    fn resolve_redis_url_falls_back_to_default_when_missing_or_blank() {
        assert_eq!(resolve_redis_url(None), DEFAULT_REDIS_URL.to_string());
        assert_eq!(
            resolve_redis_url(Some("   \t\n ".to_string())),
            DEFAULT_REDIS_URL.to_string()
        );
    }

    #[test]
    fn redact_redis_url_for_log_strips_credentials_and_paths() {
        assert_eq!(
            redact_redis_url_for_log("redis://:password@cache.internal:6380/4"),
            "cache.internal:6380"
        );
        assert_eq!(
            redact_redis_url_for_log("rediss://user:password@[::1]:6390/0?tls=true"),
            "[::1]:6390"
        );
    }

    #[test]
    fn redact_redis_url_for_log_falls_back_for_invalid_values() {
        assert_eq!(
            redact_redis_url_for_log("not-a-redis-url"),
            "[redacted redis endpoint]"
        );
    }

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

    mod world_state_tests {
        use super::*;
        use crate::player::state::PlayerState;

        fn setup_publish_app(with_zone_registry: bool) -> (App, Receiver<RedisOutbound>) {
            let (tx_outbound, rx_outbound) = unbounded();
            let (_tx_inbound, rx_inbound) = unbounded();
            let mut app = App::new();

            app.insert_resource(RedisBridgeResource {
                tx_outbound,
                rx_inbound,
            });
            app.insert_resource(WorldStateTimer {
                ticks: WORLD_STATE_PUBLISH_INTERVAL_TICKS - 1,
            });

            if with_zone_registry {
                app.insert_resource(ZoneRegistry::fallback());
            }

            app.add_systems(Update, publish_world_state_to_redis);

            (app, rx_outbound)
        }

        fn spawn_test_client(app: &mut App, username: &str, position: [f64; 3]) -> Entity {
            let (mut client_bundle, _helper) = create_mock_client(username);
            client_bundle.player.position = Position::new(position);

            app.world_mut().spawn(client_bundle).id()
        }

        fn publish_once(app: &mut App, rx_outbound: &Receiver<RedisOutbound>) -> WorldStateV1 {
            app.update();

            match rx_outbound
                .try_recv()
                .expect("world state publish should enqueue a Redis outbound message")
            {
                RedisOutbound::WorldState(state) => state,
                other => panic!("expected a world-state publish, got {other:?}"),
            }
        }

        #[test]
        fn uses_real_player_names_and_positions() {
            let (mut app, rx_outbound) = setup_publish_app(true);
            spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
            spawn_test_client(&mut app, "Bob", [12.5, 66.0, 9.25]);

            let state = publish_once(&mut app, &rx_outbound);
            let alice = state
                .players
                .iter()
                .find(|player| player.name == "Alice")
                .expect("Alice should be present in the world snapshot");
            let bob = state
                .players
                .iter()
                .find(|player| player.name == "Bob")
                .expect("Bob should be present in the world snapshot");

            assert_eq!(alice.pos, [8.0, 66.0, 8.0]);
            assert_eq!(bob.pos, [12.5, 66.0, 9.25]);
            assert!(
                state
                    .players
                    .iter()
                    .all(|player| !player.name.starts_with("Player")),
                "placeholder Player{{i}} names should not be emitted once real usernames exist"
            );
        }

        #[test]
        fn emits_spawn_zone_without_players() {
            let (mut app, rx_outbound) = setup_publish_app(true);

            let state = publish_once(&mut app, &rx_outbound);
            let spawn_zone = state
                .zones
                .iter()
                .find(|zone| zone.name == DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn fallback zone should still be emitted with zero players");

            assert!(state.players.is_empty());
            assert_eq!(spawn_zone.player_count, 0);
            assert!(
                state.recent_events.is_empty(),
                "recent_events should be an explicit empty array when no event buffer exists"
            );
        }

        #[test]
        fn uses_generation_aware_canonical_ids() {
            let (mut app, rx_outbound) = setup_publish_app(false);
            let player_entity = spawn_test_client(&mut app, "Azure", [8.0, 66.0, 8.0]);
            let npc_entity = app
                .world_mut()
                .spawn((
                    NpcMarker,
                    NpcBlackboard {
                        nearest_player: Some(player_entity),
                        ..Default::default()
                    },
                    Position::new([14.0, 66.0, 14.0]),
                    EntityKind::ZOMBIE,
                ))
                .id();
            let expected_npc_id = format!("npc_{}v{}", npc_entity.index(), npc_entity.generation());

            let state = publish_once(&mut app, &rx_outbound);
            let player = state
                .players
                .iter()
                .find(|player| player.name == "Azure")
                .expect("Azure should be present in the world snapshot");
            let npc = state
                .npcs
                .iter()
                .find(|npc| npc.id == expected_npc_id)
                .expect("NPC snapshot should use the generation-aware canonical id");

            assert_eq!(player.uuid, "offline:Azure");
            assert_eq!(player.name, "Azure");
            assert_eq!(player.zone, DEFAULT_SPAWN_ZONE_NAME);
            assert_eq!(npc.id, canonical_npc_id(npc_entity));
            assert_eq!(npc.id, expected_npc_id);
            assert!(
                npc.id.contains('v'),
                "NPC canonical ids must include entity generation"
            );
            assert_eq!(
                npc.blackboard.get("nearest_player"),
                Some(&serde_json::Value::String("offline:Azure".to_string()))
            );
            assert!(
                state
                    .players
                    .iter()
                    .all(|player| !player.uuid.contains("player_")),
                "canonical player ids must be offline:{{username}}, not offline:player_{{i}}"
            );
        }

        #[test]
        fn uses_attached_player_state_when_present() {
            let (mut app, rx_outbound) = setup_publish_app(true);
            let player_entity = spawn_test_client(&mut app, "Azure", [8.0, 66.0, 8.0]);

            app.world_mut()
                .entity_mut(player_entity)
                .insert(PlayerState {
                    realm: "qi_refining_3".to_string(),
                    spirit_qi: 78.0,
                    spirit_qi_max: 100.0,
                    karma: 0.2,
                    experience: 1_200,
                    inventory_score: 0.4,
                });

            let state = publish_once(&mut app, &rx_outbound);
            let player = state
                .players
                .iter()
                .find(|player| player.name == "Azure")
                .expect("Azure should be present in the world snapshot");

            assert_eq!(player.realm, "qi_refining_3");
            assert_eq!(player.zone, DEFAULT_SPAWN_ZONE_NAME);
            assert!(
                player.composite_power > 0.0,
                "attached PlayerState should replace placeholder composite power"
            );
            assert!(
                player.breakdown.combat > 0.0,
                "attached PlayerState should replace placeholder power breakdown"
            );
        }
    }

    mod narration_tests {
        use super::*;
        use crate::schema::common::{NarrationScope, NarrationStyle};
        use crate::schema::narration::{Narration, NarrationV1};
        use crate::world::zone::Zone;
        use crossbeam_channel::Sender;
        use valence::prelude::DVec3;
        use valence::protocol::packets::play::{CustomPayloadS2c, GameMessageS2c};
        use valence::testing::MockClientHelper;

        fn setup_narration_app(zone_registry: Option<ZoneRegistry>) -> (App, Sender<RedisInbound>) {
            let (tx_outbound, _rx_outbound) = unbounded();
            let (tx_inbound, rx_inbound) = unbounded();
            let mut app = App::new();

            app.insert_resource(RedisBridgeResource {
                tx_outbound,
                rx_inbound,
            });
            app.insert_resource(CommandExecutorResource::default());
            app.insert_resource(NarrationDedupeResource::default());

            if let Some(zone_registry) = zone_registry {
                app.insert_resource(zone_registry);
            }

            app.add_event::<crate::cultivation::insight::InsightOffer>();
            app.add_systems(Update, process_redis_inbound);

            (app, tx_inbound)
        }

        fn spawn_test_client_with_helper(
            app: &mut App,
            username: &str,
            position: [f64; 3],
        ) -> (Entity, MockClientHelper) {
            let (mut client_bundle, helper) = create_mock_client(username);
            client_bundle.player.position = Position::new(position);

            let entity = app.world_mut().spawn(client_bundle).id();
            (entity, helper)
        }

        fn enqueue_single_narration(tx_inbound: &Sender<RedisInbound>, narration: Narration) {
            tx_inbound
                .send(RedisInbound::AgentNarration(NarrationV1 {
                    v: 1,
                    narrations: vec![narration],
                }))
                .expect("narration message should enqueue into inbound channel");
        }

        fn flush_all_client_packets(app: &mut App) {
            let world = app.world_mut();
            let mut query = world.query::<&mut Client>();

            for mut client in query.iter_mut(world) {
                client
                    .flush_packets()
                    .expect("mock client packets should flush successfully");
            }
        }

        fn collect_typed_narration_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
            let mut payloads = Vec::new();

            for frame in helper.collect_received().0 {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    continue;
                };

                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    continue;
                }

                let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                    .expect("typed custom payload should decode as ServerDataV1 JSON");

                if matches!(payload.payload, ServerDataPayloadV1::Narration { .. }) {
                    payloads.push(payload);
                }
            }

            payloads
        }

        fn collect_game_message_packets(helper: &mut MockClientHelper) -> usize {
            helper
                .collect_received()
                .0
                .into_iter()
                .filter(|frame| frame.decode::<GameMessageS2c>().is_ok())
                .count()
        }

        fn assert_single_narration_payload(payloads: &[ServerDataV1], expected_text: &str) {
            assert_eq!(
                payloads.len(),
                1,
                "expected exactly one typed narration payload"
            );

            match &payloads[0].payload {
                ServerDataPayloadV1::Narration { narrations } => {
                    assert_eq!(narrations.len(), 1, "expected exactly one narration entry");
                    assert_eq!(narrations[0].text, expected_text);
                }
                other => panic!("expected narration payload, got {other:?}"),
            }
        }

        #[test]
        fn broadcast_emits_only_typed_narration_payload() {
            let (mut app, tx_inbound) = setup_narration_app(None);
            let (_alice, mut alice_helper) =
                spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

            enqueue_single_narration(
                &tx_inbound,
                Narration {
                    scope: NarrationScope::Broadcast,
                    target: None,
                    text: "天地震荡，灵气翻涌。".to_string(),
                    style: NarrationStyle::Narration,
                },
            );

            app.update();
            flush_all_client_packets(&mut app);

            let alice_payloads = collect_typed_narration_payloads(&mut alice_helper);
            let alice_chat_packets = collect_game_message_packets(&mut alice_helper);

            assert_single_narration_payload(alice_payloads.as_slice(), "天地震荡，灵气翻涌。");
            assert_eq!(
                alice_chat_packets, 0,
                "narration path should not emit mirrored GameMessageS2c chat packets"
            );
        }

        #[test]
        fn zone_scope_filters_by_zone() {
            let spawn_zone = Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                bounds: (DVec3::new(0.0, 64.0, 0.0), DVec3::new(128.0, 128.0, 128.0)),
                spirit_qi: 0.9,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: vec![DVec3::new(14.0, 66.0, 14.0)],
                blocked_tiles: Vec::new(),
            };
            let blood_valley = Zone {
                name: "blood_valley".to_string(),
                bounds: (
                    DVec3::new(1000.0, 64.0, 1000.0),
                    DVec3::new(1200.0, 128.0, 1200.0),
                ),
                spirit_qi: 0.4,
                danger_level: 4,
                active_events: Vec::new(),
                patrol_anchors: vec![DVec3::new(1004.0, 66.0, 1004.0)],
                blocked_tiles: Vec::new(),
            };

            let zone_registry = ZoneRegistry {
                zones: vec![spawn_zone, blood_valley],
            };

            let (mut app, tx_inbound) = setup_narration_app(Some(zone_registry));
            let (_alice, mut alice_helper) =
                spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);
            let (_bob, mut bob_helper) =
                spawn_test_client_with_helper(&mut app, "Bob", [1005.0, 66.0, 1005.0]);

            enqueue_single_narration(
                &tx_inbound,
                Narration {
                    scope: NarrationScope::Zone,
                    target: Some("blood_valley".to_string()),
                    text: "血谷雷云聚集。".to_string(),
                    style: NarrationStyle::SystemWarning,
                },
            );

            app.update();
            flush_all_client_packets(&mut app);

            let alice_payloads = collect_typed_narration_payloads(&mut alice_helper);
            let bob_payloads = collect_typed_narration_payloads(&mut bob_helper);
            let alice_chat_packets = collect_game_message_packets(&mut alice_helper);
            let bob_chat_packets = collect_game_message_packets(&mut bob_helper);

            assert!(
                alice_payloads.is_empty(),
                "spawn zone player should not receive blood_valley scoped narration"
            );
            assert_eq!(
                alice_chat_packets, 0,
                "zone-scoped narration should not mirror chat packets"
            );
            assert_single_narration_payload(bob_payloads.as_slice(), "血谷雷云聚集。");
            assert_eq!(
                bob_chat_packets, 0,
                "zone-scoped narration should not mirror chat packets"
            );
        }

        #[test]
        fn player_scope_matches_username_and_offline_id() {
            let (mut app, tx_inbound) = setup_narration_app(None);
            let (_steve, mut steve_helper) =
                spawn_test_client_with_helper(&mut app, "Steve", [8.0, 66.0, 8.0]);
            let (_alex, mut alex_helper) =
                spawn_test_client_with_helper(&mut app, "Alex", [18.0, 66.0, 18.0]);

            enqueue_single_narration(
                &tx_inbound,
                Narration {
                    scope: NarrationScope::Player,
                    target: Some("Steve".to_string()),
                    text: "第一段单人叙事。".to_string(),
                    style: NarrationStyle::Perception,
                },
            );

            app.update();
            flush_all_client_packets(&mut app);

            let steve_plain = collect_typed_narration_payloads(&mut steve_helper);
            let alex_plain = collect_typed_narration_payloads(&mut alex_helper);
            let steve_chat_packets = collect_game_message_packets(&mut steve_helper);
            let alex_chat_packets = collect_game_message_packets(&mut alex_helper);

            assert_single_narration_payload(steve_plain.as_slice(), "第一段单人叙事。");
            assert!(
                alex_plain.is_empty(),
                "non-targeted player must not receive payload"
            );
            assert_eq!(
                steve_chat_packets, 0,
                "player-scoped narration should not mirror chat packets"
            );
            assert_eq!(
                alex_chat_packets, 0,
                "non-targeted player must not receive chat packets"
            );

            enqueue_single_narration(
                &tx_inbound,
                Narration {
                    scope: NarrationScope::Player,
                    target: Some("offline:Steve".to_string()),
                    text: "第二段单人叙事。".to_string(),
                    style: NarrationStyle::Perception,
                },
            );

            app.update();
            flush_all_client_packets(&mut app);

            let steve_alias = collect_typed_narration_payloads(&mut steve_helper);
            let alex_alias = collect_typed_narration_payloads(&mut alex_helper);
            let steve_alias_chat_packets = collect_game_message_packets(&mut steve_helper);
            let alex_alias_chat_packets = collect_game_message_packets(&mut alex_helper);

            assert_single_narration_payload(steve_alias.as_slice(), "第二段单人叙事。");
            assert!(
                alex_alias.is_empty(),
                "non-targeted player must not receive payload"
            );
            assert_eq!(
                steve_alias_chat_packets, 0,
                "player-scoped narration should not mirror chat packets"
            );
            assert_eq!(
                alex_alias_chat_packets, 0,
                "non-targeted player must not receive chat packets"
            );
        }

        #[test]
        fn missing_player_target_is_ignored() {
            let (mut app, tx_inbound) = setup_narration_app(None);
            let (_alice, mut alice_helper) =
                spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);
            let (_bob, mut bob_helper) =
                spawn_test_client_with_helper(&mut app, "Bob", [20.0, 66.0, 20.0]);

            enqueue_single_narration(
                &tx_inbound,
                Narration {
                    scope: NarrationScope::Player,
                    target: Some("offline:Ghost".to_string()),
                    text: "不存在目标，不应泄露。".to_string(),
                    style: NarrationStyle::Narration,
                },
            );

            app.update();
            flush_all_client_packets(&mut app);

            let alice_payloads = collect_typed_narration_payloads(&mut alice_helper);
            let bob_payloads = collect_typed_narration_payloads(&mut bob_helper);
            let alice_chat_packets = collect_game_message_packets(&mut alice_helper);
            let bob_chat_packets = collect_game_message_packets(&mut bob_helper);

            assert!(
                alice_payloads.is_empty(),
                "missing player target should not leak payload to Alice"
            );
            assert_eq!(
                alice_chat_packets, 0,
                "missing player target should not leak chat packets to Alice"
            );
            assert!(
                bob_payloads.is_empty(),
                "missing player target should not leak payload to Bob"
            );
            assert_eq!(
                bob_chat_packets, 0,
                "missing player target should not leak chat packets to Bob"
            );
        }

        #[test]
        fn duplicate_narration_payload_is_deduped_within_window() {
            let (mut app, tx_inbound) = setup_narration_app(None);
            let (_alice, mut alice_helper) =
                spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

            let narration = Narration {
                scope: NarrationScope::Broadcast,
                target: None,
                text: "重复叙事只应投递一次。".to_string(),
                style: NarrationStyle::Narration,
            };

            enqueue_single_narration(&tx_inbound, narration.clone());
            enqueue_single_narration(&tx_inbound, narration);

            app.update();
            flush_all_client_packets(&mut app);

            let payloads = collect_typed_narration_payloads(&mut alice_helper);
            assert_eq!(
                payloads.len(),
                1,
                "duplicate narration payload should be dropped by short-window dedupe"
            );
        }
    }

    mod zone_payload_tests {
        use super::*;
        use crate::world::zone::Zone;
        use valence::prelude::DVec3;
        use valence::protocol::packets::play::CustomPayloadS2c;
        use valence::testing::MockClientHelper;

        fn setup_zone_transition_app(zone_registry: ZoneRegistry) -> App {
            let mut app = App::new();
            app.insert_resource(ZoneTransitionTracker::default());
            app.insert_resource(zone_registry);
            app.add_systems(Update, emit_zone_info_on_zone_transition);
            app
        }

        fn spawn_test_client_with_helper(
            app: &mut App,
            username: &str,
            position: [f64; 3],
        ) -> (Entity, MockClientHelper) {
            let (mut client_bundle, helper) = create_mock_client(username);
            client_bundle.player.position = Position::new(position);
            let entity = app.world_mut().spawn(client_bundle).id();
            (entity, helper)
        }

        fn flush_all_client_packets(app: &mut App) {
            let world = app.world_mut();
            let mut query = world.query::<&mut Client>();
            for mut client in query.iter_mut(world) {
                client
                    .flush_packets()
                    .expect("mock client packets should flush successfully");
            }
        }

        fn collect_zone_info_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
            let mut payloads = Vec::new();

            for frame in helper.collect_received().0 {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    continue;
                };
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    continue;
                }

                let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                    .expect("typed payload should decode as ServerDataV1");

                if matches!(payload.payload, ServerDataPayloadV1::ZoneInfo { .. }) {
                    payloads.push(payload);
                }
            }

            payloads
        }

        #[test]
        fn emits_zone_info_on_transition() {
            let zone_registry = ZoneRegistry {
                zones: vec![
                    Zone {
                        name: "spawn".to_string(),
                        bounds: (DVec3::new(0.0, 64.0, 0.0), DVec3::new(128.0, 128.0, 128.0)),
                        spirit_qi: 0.9,
                        danger_level: 0,
                        active_events: vec![],
                        patrol_anchors: vec![DVec3::new(14.0, 66.0, 14.0)],
                        blocked_tiles: vec![],
                    },
                    Zone {
                        name: "blood_valley".to_string(),
                        bounds: (
                            DVec3::new(1000.0, 64.0, 1000.0),
                            DVec3::new(1200.0, 128.0, 1200.0),
                        ),
                        spirit_qi: 0.42,
                        danger_level: 4,
                        active_events: vec!["beast_tide".to_string()],
                        patrol_anchors: vec![DVec3::new(1004.0, 66.0, 1004.0)],
                        blocked_tiles: vec![],
                    },
                ],
            };

            let mut app = setup_zone_transition_app(zone_registry);
            let (entity, mut helper) =
                spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

            app.update();
            flush_all_client_packets(&mut app);

            let first_payloads = collect_zone_info_payloads(&mut helper);
            assert_eq!(
                first_payloads.len(),
                1,
                "first zone snapshot should be sent on initial track"
            );

            match &first_payloads[0].payload {
                ServerDataPayloadV1::ZoneInfo {
                    zone,
                    spirit_qi,
                    danger_level,
                    active_events,
                } => {
                    assert_eq!(zone, "spawn");
                    assert_eq!(*spirit_qi, 0.9);
                    assert_eq!(*danger_level, 0);
                    assert_eq!(active_events, &None);
                }
                other => panic!("expected zone_info payload, got {other:?}"),
            }

            {
                let mut query = app.world_mut().query::<&mut Position>();
                let mut position = query
                    .get_mut(app.world_mut(), entity)
                    .expect("test client position should be mutable");
                position.set([1005.0, 66.0, 1005.0]);
            }

            app.update();
            flush_all_client_packets(&mut app);

            let second_payloads = collect_zone_info_payloads(&mut helper);
            assert_eq!(
                second_payloads.len(),
                1,
                "transition should emit exactly one zone_info payload"
            );

            match &second_payloads[0].payload {
                ServerDataPayloadV1::ZoneInfo {
                    zone,
                    spirit_qi,
                    danger_level,
                    active_events,
                } => {
                    assert_eq!(zone, "blood_valley");
                    assert_eq!(*spirit_qi, 0.42);
                    assert_eq!(*danger_level, 4);
                    assert_eq!(active_events, &Some(vec!["beast_tide".to_string()]));
                }
                other => panic!("expected zone_info payload, got {other:?}"),
            }

            app.update();
            flush_all_client_packets(&mut app);
            let third_payloads = collect_zone_info_payloads(&mut helper);
            assert!(
                third_payloads.is_empty(),
                "no additional payload should be emitted without a new transition"
            );
        }
    }

    mod player_state_payload_tests {
        use super::*;
        use crate::player::state::PlayerState;
        use crate::world::zone::ZoneRegistry;
        use valence::protocol::packets::play::CustomPayloadS2c;
        use valence::testing::MockClientHelper;

        fn emit_player_state_payloads_periodically_without_change(
            zone_registry: Option<Res<ZoneRegistry>>,
            mut tick_counter: valence::prelude::Local<u64>,
            mut clients: Query<
                (Entity, &mut Client, &Username, &Position, &PlayerState),
                With<Client>,
            >,
        ) {
            *tick_counter += 1;
            if !tick_counter.is_multiple_of(WORLD_STATE_PUBLISH_INTERVAL_TICKS) {
                return;
            }

            let zone_registry = effective_zone_registry(zone_registry.as_deref());

            for (entity, mut client, username, position, player_state) in &mut clients {
                let zone_name = zone_name_for_position(&zone_registry, position.get());
                let payload = player_state
                    .server_payload(Some(canonical_player_id(username.0.as_str())), zone_name);
                let payload_type = payload_type_label(payload.payload_type());
                let payload_bytes = match serialize_server_data_payload(&payload) {
                    Ok(payload) => payload,
                    Err(error) => {
                        log_payload_build_error(payload_type, &error);
                        continue;
                    }
                };

                send_server_data_payload(&mut client, payload_bytes.as_slice());
                tracing::info!(
                    "[bong][network] sent {} {} payload to client entity {entity:?} for `{}` (periodic test seam)",
                    SERVER_DATA_CHANNEL,
                    payload_type,
                    username.0,
                );
            }
        }

        fn setup_player_state_payload_app() -> App {
            let mut app = App::new();
            app.insert_resource(ZoneRegistry::fallback());
            app.add_systems(Update, emit_player_state_payloads);
            app
        }

        fn spawn_test_client_with_helper(
            app: &mut App,
            username: &str,
            position: [f64; 3],
        ) -> (Entity, MockClientHelper) {
            let (mut client_bundle, helper) = create_mock_client(username);
            client_bundle.player.position = Position::new(position);
            let entity = app.world_mut().spawn(client_bundle).id();
            (entity, helper)
        }

        fn flush_all_client_packets(app: &mut App) {
            let world = app.world_mut();
            let mut query = world.query::<&mut Client>();
            for mut client in query.iter_mut(world) {
                client
                    .flush_packets()
                    .expect("mock client packets should flush successfully");
            }
        }

        fn collect_player_state_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
            let mut payloads = Vec::new();

            for frame in helper.collect_received().0 {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    continue;
                };

                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    continue;
                }

                let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                    .expect("typed payload should decode as ServerDataV1");

                if matches!(payload.payload, ServerDataPayloadV1::PlayerState { .. }) {
                    payloads.push(payload);
                }
            }

            payloads
        }

        #[test]
        fn emits_player_state_on_join_and_change() {
            let mut app = setup_player_state_payload_app();
            let (entity, mut helper) =
                spawn_test_client_with_helper(&mut app, "Azure", [8.0, 66.0, 8.0]);

            app.world_mut().entity_mut(entity).insert(PlayerState {
                realm: "qi_refining_3".to_string(),
                spirit_qi: 78.0,
                spirit_qi_max: 100.0,
                karma: 0.2,
                experience: 1_200,
                inventory_score: 0.4,
            });

            app.update();
            flush_all_client_packets(&mut app);

            let first_payloads = collect_player_state_payloads(&mut helper);
            assert_eq!(
                first_payloads.len(),
                1,
                "join/attach should emit one player_state payload"
            );

            match &first_payloads[0].payload {
                ServerDataPayloadV1::PlayerState {
                    player,
                    realm,
                    spirit_qi,
                    zone,
                    ..
                } => {
                    assert_eq!(player.as_deref(), Some("offline:Azure"));
                    assert_eq!(realm, "qi_refining_3");
                    assert_eq!(*spirit_qi, 78.0);
                    assert_eq!(zone, DEFAULT_SPAWN_ZONE_NAME);
                }
                other => panic!("expected player_state payload, got {other:?}"),
            }

            {
                let mut query = app.world_mut().query::<&mut PlayerState>();
                let mut player_state = query
                    .get_mut(app.world_mut(), entity)
                    .expect("test client PlayerState should be mutable");
                player_state.spirit_qi = 81.0;
            }

            app.update();
            flush_all_client_packets(&mut app);

            let second_payloads = collect_player_state_payloads(&mut helper);
            assert_eq!(
                second_payloads.len(),
                1,
                "PlayerState change should emit exactly one payload"
            );

            match &second_payloads[0].payload {
                ServerDataPayloadV1::PlayerState { spirit_qi, .. } => {
                    assert_eq!(*spirit_qi, 81.0);
                }
                other => panic!("expected player_state payload, got {other:?}"),
            }
        }

        #[test]
        fn missing_target_route_player_state_does_not_broadcast_to_all_clients() {
            let mut app = setup_player_state_payload_app();
            let (azure_entity, mut azure_helper) =
                spawn_test_client_with_helper(&mut app, "Azure", [8.0, 66.0, 8.0]);
            let (_bob_entity, mut bob_helper) =
                spawn_test_client_with_helper(&mut app, "Bob", [20.0, 66.0, 20.0]);

            app.world_mut()
                .entity_mut(azure_entity)
                .insert(PlayerState {
                    realm: "qi_refining_3".to_string(),
                    spirit_qi: 78.0,
                    spirit_qi_max: 100.0,
                    karma: 0.2,
                    experience: 1_200,
                    inventory_score: 0.4,
                });
            app.world_mut().entity_mut(_bob_entity).insert(PlayerState {
                realm: "mortal".to_string(),
                spirit_qi: 0.0,
                spirit_qi_max: 100.0,
                karma: 0.0,
                experience: 0,
                inventory_score: 0.0,
            });

            app.update();
            flush_all_client_packets(&mut app);
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
            flush_all_client_packets(&mut app);

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
            app.add_systems(
                Update,
                emit_player_state_payloads_periodically_without_change,
            );

            let (entity, mut helper) =
                spawn_test_client_with_helper(&mut app, "Azure", [8.0, 66.0, 8.0]);
            app.world_mut().entity_mut(entity).insert(PlayerState {
                realm: "qi_refining_3".to_string(),
                spirit_qi: 78.0,
                spirit_qi_max: 100.0,
                karma: 0.0,
                experience: 0,
                inventory_score: 0.0,
            });

            app.update();
            flush_all_client_packets(&mut app);
            let _ = collect_player_state_payloads(&mut helper);

            for _ in 0..(WORLD_STATE_PUBLISH_INTERVAL_TICKS - 1) {
                app.update();
            }
            flush_all_client_packets(&mut app);

            let periodic_payloads = collect_player_state_payloads(&mut helper);
            assert_eq!(
                periodic_payloads.len(),
                1,
                "periodic cadence should emit one player_state payload without Changed<PlayerState>"
            );
        }
    }

    mod event_payload_tests {
        use super::*;
        use crate::world::events::{ActiveEventsResource, EVENT_THUNDER_TRIBULATION};
        use crate::world::zone::ZoneRegistry;
        use std::collections::HashMap;
        use valence::protocol::packets::play::CustomPayloadS2c;
        use valence::testing::MockClientHelper;

        fn spawn_event_command(
            target: &str,
            event: &str,
            duration_ticks: u64,
        ) -> crate::schema::agent_command::Command {
            let mut params = HashMap::new();
            params.insert("event".to_string(), serde_json::json!(event));
            params.insert(
                "duration_ticks".to_string(),
                serde_json::json!(duration_ticks),
            );

            crate::schema::agent_command::Command {
                command_type: crate::schema::common::CommandType::SpawnEvent,
                target: target.to_string(),
                params,
            }
        }

        fn setup_event_alert_app() -> App {
            let mut app = App::new();
            app.insert_resource(ActiveEventsResource::default());
            app.insert_resource(ZoneRegistry::fallback());
            app.add_systems(Update, emit_event_alerts_on_major_event_creation);
            app
        }

        fn spawn_test_client_with_helper(
            app: &mut App,
            username: &str,
            position: [f64; 3],
        ) -> (Entity, MockClientHelper) {
            let (mut client_bundle, helper) = create_mock_client(username);
            client_bundle.player.position = Position::new(position);
            let entity = app.world_mut().spawn(client_bundle).id();
            (entity, helper)
        }

        fn flush_all_client_packets(app: &mut App) {
            let world = app.world_mut();
            let mut query = world.query::<&mut Client>();
            for mut client in query.iter_mut(world) {
                client
                    .flush_packets()
                    .expect("mock client packets should flush successfully");
            }
        }

        fn collect_event_alert_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
            let mut payloads = Vec::new();

            for frame in helper.collect_received().0 {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    continue;
                };
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    continue;
                }

                let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                    .expect("typed payload should decode as ServerDataV1");

                if matches!(payload.payload, ServerDataPayloadV1::EventAlert { .. }) {
                    payloads.push(payload);
                }
            }

            payloads
        }

        #[test]
        fn emits_event_alert_on_major_event() {
            let mut app = setup_event_alert_app();
            let (_entity, mut helper) =
                spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

            {
                let world = app.world_mut();
                let command = spawn_event_command("spawn", EVENT_THUNDER_TRIBULATION, 180);
                world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                    let mut events = world.resource_mut::<ActiveEventsResource>();
                    let accepted = events.enqueue_from_spawn_command(&command, Some(&mut zones));
                    assert!(
                        accepted,
                        "thunder major event should be accepted into scheduler"
                    );
                });
            }

            app.update();
            flush_all_client_packets(&mut app);

            let payloads = collect_event_alert_payloads(&mut helper);
            assert_eq!(
                payloads.len(),
                1,
                "major event enqueue should emit one event_alert payload"
            );

            match &payloads[0].payload {
                ServerDataPayloadV1::EventAlert {
                    event,
                    message,
                    zone,
                    duration_ticks,
                } => {
                    assert_eq!(*event, EventKind::ThunderTribulation);
                    assert!(message.contains("天劫"));
                    assert_eq!(zone.as_deref(), Some("spawn"));
                    assert_eq!(*duration_ticks, Some(180));
                }
                other => panic!("expected event_alert payload, got {other:?}"),
            }

            app.update();
            flush_all_client_packets(&mut app);
            let second = collect_event_alert_payloads(&mut helper);
            assert!(
                second.is_empty(),
                "drained major-event alerts must not be resent on subsequent ticks"
            );
        }
    }

    mod gameplay_tests {
        use super::*;
        use crate::combat::{
            components::{CombatState, DerivedAttrs, Lifecycle, Stamina, StatusEffects, Wounds},
            events::{ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathEvent},
            CombatClock,
        };
        use crate::cultivation::components::{
            Contamination, Cultivation, MeridianId, MeridianSystem,
        };
        use crate::cultivation::life_record::LifeRecord;
        use crate::player::gameplay::{
            CombatAction, GameplayAction, GameplayActionQueue, GameplayTick, GatherAction,
            PendingGameplayNarrations,
        };
        use crate::world::events::ActiveEventsResource;
        use crossbeam_channel::{unbounded, Receiver};
        use valence::prelude::Events;
        use valence::protocol::packets::play::CustomPayloadS2c;
        use valence::testing::MockClientHelper;

        fn setup_gameplay_app() -> (App, Receiver<RedisOutbound>) {
            let (tx_outbound, rx_outbound) = unbounded();
            let (_tx_inbound, rx_inbound) = unbounded();
            let mut app = App::new();

            app.insert_resource(RedisBridgeResource {
                tx_outbound,
                rx_inbound,
            });
            app.insert_resource(WorldStateTimer {
                ticks: WORLD_STATE_PUBLISH_INTERVAL_TICKS - 1,
            });
            app.insert_resource(ZoneRegistry::fallback());
            app.insert_resource(ActiveEventsResource::default());
            app.insert_resource(GameplayActionQueue::default());
            app.insert_resource(PendingGameplayNarrations::default());
            app.insert_resource(GameplayTick::default());
            app.insert_resource(CombatClock::default());
            app.add_event::<AttackIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<CombatEvent>();
            app.add_event::<DeathEvent>();
            app.add_systems(
                Update,
                (
                    crate::combat::debug::tick_combat_clock,
                    crate::player::gameplay::apply_queued_gameplay_actions
                        .after(crate::combat::debug::tick_combat_clock),
                    crate::combat::status::status_effect_apply_tick
                        .after(crate::player::gameplay::apply_queued_gameplay_actions),
                    crate::combat::status::attribute_aggregate_tick
                        .after(crate::combat::status::status_effect_apply_tick),
                    crate::combat::resolve::resolve_attack_intents
                        .after(crate::player::gameplay::apply_queued_gameplay_actions),
                    emit_gameplay_narrations
                        .after(crate::combat::resolve::resolve_attack_intents),
                    emit_player_state_payloads
                        .after(crate::player::gameplay::apply_queued_gameplay_actions),
                    publish_world_state_to_redis
                        .after(crate::combat::resolve::resolve_attack_intents),
                ),
            );

            (app, rx_outbound)
        }

        fn spawn_test_client_with_state(
            app: &mut App,
            username: &str,
            position: [f64; 3],
            player_state: PlayerState,
        ) -> (Entity, MockClientHelper) {
            let (mut client_bundle, helper) = create_mock_client(username);
            client_bundle.player.position = Position::new(position);
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    Cultivation {
                        qi_current: player_state.spirit_qi,
                        qi_max: player_state.spirit_qi_max,
                        ..Cultivation::default()
                    },
                    player_state,
                    Wounds::default(),
                    Stamina::default(),
                    CombatState::default(),
                    StatusEffects::default(),
                    DerivedAttrs::default(),
                    Lifecycle {
                        character_id: canonical_player_id(username),
                        ..Default::default()
                    },
                    Contamination::default(),
                    MeridianSystem::default(),
                    LifeRecord::new(canonical_player_id(username)),
                ))
                .id();
            (entity, helper)
        }

        fn flush_all_client_packets(app: &mut App) {
            let world = app.world_mut();
            let mut query = world.query::<&mut Client>();

            for mut client in query.iter_mut(world) {
                client
                    .flush_packets()
                    .expect("mock client packets should flush successfully");
            }
        }

        fn collect_server_data_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
            let mut payloads = Vec::new();

            for frame in helper.collect_received().0 {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    continue;
                };
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    continue;
                }

                payloads.push(
                    serde_json::from_slice(packet.data.0 .0)
                        .expect("typed payload should decode as ServerDataV1"),
                );
            }

            payloads
        }

        fn extract_player_state_payloads(payloads: &[ServerDataV1]) -> Vec<&ServerDataV1> {
            payloads
                .iter()
                .filter(|payload| {
                    matches!(payload.payload, ServerDataPayloadV1::PlayerState { .. })
                })
                .collect()
        }

        fn extract_narration_payloads(payloads: &[ServerDataV1]) -> Vec<&ServerDataV1> {
            payloads
                .iter()
                .filter(|payload| matches!(payload.payload, ServerDataPayloadV1::Narration { .. }))
                .collect()
        }

        fn dequeue_world_state(rx_outbound: &Receiver<RedisOutbound>) -> WorldStateV1 {
            match rx_outbound
                .try_recv()
                .expect("world state publish should enqueue a Redis outbound message")
            {
                RedisOutbound::WorldState(state) => state,
                other => panic!("expected world-state publish, got {other:?}"),
            }
        }

        #[test]
        fn combat_routes_debug_attack_through_resolver() {
            let (mut app, rx_outbound) = setup_gameplay_app();
            let (attacker, _attacker_helper) = spawn_test_client_with_state(
                &mut app,
                "Azure",
                [8.0, 66.0, 8.0],
                PlayerState {
                    realm: "qi_refining_1".to_string(),
                    spirit_qi: 70.0,
                    spirit_qi_max: 100.0,
                    karma: 0.05,
                    experience: 200,
                    inventory_score: 0.10,
                },
            );
            let (target, _target_helper) = spawn_test_client_with_state(
                &mut app,
                "Crimson",
                [9.0, 66.0, 8.0],
                PlayerState {
                    realm: "qi_refining_1".to_string(),
                    spirit_qi: 65.0,
                    spirit_qi_max: 100.0,
                    karma: 0.0,
                    experience: 80,
                    inventory_score: 0.05,
                },
            );

            let mut target_meridians = MeridianSystem::default();
            target_meridians.get_mut(MeridianId::Lung).opened = true;
            app.world_mut().entity_mut(target).insert((
                Wounds {
                    entries: Vec::new(),
                    health_current: 8.0,
                    health_max: 100.0,
                },
                target_meridians,
            ));

            app.world_mut()
                .resource_mut::<GameplayActionQueue>()
                .enqueue(
                    "Azure",
                    GameplayAction::Combat(CombatAction {
                        target: "Crimson".to_string(),
                        qi_invest: 40.0,
                    }),
                );

            app.update();
            flush_all_client_packets(&mut app);

            let world_state = dequeue_world_state(&rx_outbound);
            assert_eq!(world_state.recent_events.len(), 1);
            assert_eq!(
                world_state.recent_events[0].event_type,
                crate::schema::common::GameEventType::EventTriggered
            );

            let expected_target_id = canonical_player_id("Crimson");
            assert_eq!(
                world_state.recent_events[0].target.as_deref(),
                Some(expected_target_id.as_str())
            );

            {
                let world = app.world_mut();
                let wounds = world
                    .entity(target)
                    .get::<Wounds>()
                    .expect("target should keep combat wounds after resolver");
                let stamina = world
                    .entity(target)
                    .get::<Stamina>()
                    .expect("target should keep stamina after resolver");
                let contamination = world
                    .entity(target)
                    .get::<Contamination>()
                    .expect("target should keep contamination after resolver");
                let meridians = world
                    .entity(target)
                    .get::<MeridianSystem>()
                    .expect("target should keep meridians after resolver");

                assert!(wounds.health_current <= 0.0);
                assert_eq!(wounds.entries.len(), 1);
                assert!(stamina.current < stamina.max);
                let expected_attacker_id = canonical_player_id("Azure");
                assert_eq!(
                    contamination.entries[0].attacker_id.as_deref(),
                    Some(expected_attacker_id.as_str())
                );
                assert!(meridians.get(MeridianId::Lung).throughput_current > 0.0);
            }

            let combat_events = app.world().resource::<Events<CombatEvent>>();
            let death_events = app.world().resource::<Events<DeathEvent>>();
            assert!(!combat_events.is_empty(), "combat should emit CombatEvent via resolver");
            assert!(!death_events.is_empty(), "lethal debug combat should emit DeathEvent");

            let attacker_state = app
                .world()
                .entity(attacker)
                .get::<PlayerState>()
                .expect("attacker player state should remain attached");
            assert_eq!(attacker_state.spirit_qi, 70.0, "attacker PlayerState should not be fake-mutated");

            let attacker_cultivation = app
                .world()
                .entity(attacker)
                .get::<crate::cultivation::components::Cultivation>()
                .expect("attacker cultivation should be present for qi-backed combat");
            assert_eq!(attacker_cultivation.qi_current, 30.0);
        }

        #[test]
        fn gathering_grants_experience() {
            let (mut app, rx_outbound) = setup_gameplay_app();
            let (entity, mut helper) = spawn_test_client_with_state(
                &mut app,
                "Gatherer",
                [8.0, 66.0, 8.0],
                PlayerState {
                    realm: "mortal".to_string(),
                    spirit_qi: 20.0,
                    spirit_qi_max: 100.0,
                    karma: 0.0,
                    experience: 10,
                    inventory_score: 0.0,
                },
            );

            app.world_mut()
                .resource_mut::<GameplayActionQueue>()
                .enqueue(
                    "Gatherer",
                    GameplayAction::Gather(GatherAction {
                        resource: "spirit_herb".to_string(),
                    }),
                );

            app.update();
            flush_all_client_packets(&mut app);

            let payloads = collect_server_data_payloads(&mut helper);
            let player_state_payloads = extract_player_state_payloads(payloads.as_slice());
            let narration_payloads = extract_narration_payloads(payloads.as_slice());
            assert_eq!(
                player_state_payloads.len(),
                1,
                "gathering should emit one player_state payload"
            );
            assert_eq!(
                narration_payloads.len(),
                1,
                "gathering should emit one narration payload"
            );

            match &player_state_payloads[0].payload {
                ServerDataPayloadV1::PlayerState {
                    spirit_qi,
                    karma,
                    zone,
                    ..
                } => {
                    assert_eq!(*spirit_qi, 34.0);
                    assert_eq!(*karma, 0.06);
                    assert_eq!(zone, DEFAULT_SPAWN_ZONE_NAME);
                }
                other => panic!("expected player_state payload, got {other:?}"),
            }

            let world_state = dequeue_world_state(&rx_outbound);
            assert_eq!(world_state.recent_events.len(), 1);
            assert_eq!(
                world_state.recent_events[0].event_type,
                crate::schema::common::GameEventType::ZoneQiChange
            );
            assert_eq!(
                world_state.recent_events[0].target.as_deref(),
                Some("spirit_herb")
            );

            {
                let world = app.world_mut();
                let player_state = world
                    .entity(entity)
                    .get::<PlayerState>()
                    .expect("player state should remain attached after gathering");
                assert_eq!(player_state.spirit_qi, 34.0);
                assert_eq!(player_state.experience, 100);
                assert_approx_eq(player_state.inventory_score, 0.12);
                assert_approx_eq(player_state.karma, 0.06);
            }
        }

        #[test]
        fn realm_breakthrough_updates_payloads() {
            let (mut app, rx_outbound) = setup_gameplay_app();
            let (entity, mut helper) = spawn_test_client_with_state(
                &mut app,
                "Seeker",
                [8.0, 66.0, 8.0],
                PlayerState {
                    realm: "mortal".to_string(),
                    spirit_qi: 80.0,
                    spirit_qi_max: 100.0,
                    karma: 0.1,
                    experience: 150,
                    inventory_score: 0.05,
                },
            );

            app.world_mut()
                .resource_mut::<GameplayActionQueue>()
                .enqueue("Seeker", GameplayAction::AttemptBreakthrough);

            app.update();
            flush_all_client_packets(&mut app);

            let payloads = collect_server_data_payloads(&mut helper);
            let player_state_payloads = extract_player_state_payloads(payloads.as_slice());
            let narration_payloads = extract_narration_payloads(payloads.as_slice());
            assert_eq!(
                player_state_payloads.len(),
                1,
                "breakthrough should emit one player_state payload"
            );
            assert_eq!(
                narration_payloads.len(),
                1,
                "breakthrough should emit one narration payload"
            );

            match &player_state_payloads[0].payload {
                ServerDataPayloadV1::PlayerState {
                    realm,
                    spirit_qi,
                    player,
                    ..
                } => {
                    assert_eq!(player.as_deref(), Some("offline:Seeker"));
                    assert_eq!(realm, "qi_refining_1");
                    assert_eq!(*spirit_qi, 120.0);
                }
                other => panic!("expected player_state payload, got {other:?}"),
            }

            match &narration_payloads[0].payload {
                ServerDataPayloadV1::Narration { narrations } => {
                    assert_eq!(
                        narrations[0].style,
                        crate::schema::common::NarrationStyle::SystemWarning
                    );
                    assert!(narrations[0].text.contains("炼气一层"));
                }
                other => panic!("expected narration payload, got {other:?}"),
            }

            let world_state = dequeue_world_state(&rx_outbound);
            assert_eq!(world_state.recent_events.len(), 1);
            assert_eq!(
                world_state.recent_events[0].event_type,
                crate::schema::common::GameEventType::EventTriggered
            );
            assert_eq!(
                world_state.recent_events[0].target.as_deref(),
                Some("qi_refining_1")
            );

            {
                let world = app.world_mut();
                let player_state = world
                    .entity(entity)
                    .get::<PlayerState>()
                    .expect("player state should remain attached after breakthrough");
                assert_eq!(player_state.realm, "qi_refining_1");
                assert_eq!(player_state.spirit_qi, 120.0);
                assert_eq!(player_state.spirit_qi_max, 120.0);
            }

            app.world_mut()
                .resource_mut::<GameplayActionQueue>()
                .enqueue("offline:Seeker", GameplayAction::AttemptBreakthrough);

            app.update();
            flush_all_client_packets(&mut app);

            let invalid_payloads = collect_server_data_payloads(&mut helper);
            assert!(
                extract_player_state_payloads(invalid_payloads.as_slice()).is_empty(),
                "insufficient experience should not emit a new player_state payload"
            );
            let invalid_narrations = extract_narration_payloads(invalid_payloads.as_slice());
            assert_eq!(invalid_narrations.len(), 1);

            match &invalid_narrations[0].payload {
                ServerDataPayloadV1::Narration { narrations } => {
                    assert!(narrations[0].text.contains("经验"));
                }
                other => panic!("expected narration payload, got {other:?}"),
            }

            let recent_events = app
                .world()
                .resource::<ActiveEventsResource>()
                .recent_events_snapshot();
            assert_eq!(
                recent_events.len(),
                1,
                "failed breakthrough should not append a new recent event"
            );
        }

        #[test]
        fn realm_breakthrough_rejects_invalid_karma_without_side_effects() {
            let (mut app, rx_outbound) = setup_gameplay_app();
            let (entity, mut helper) = spawn_test_client_with_state(
                &mut app,
                "Ascetic",
                [8.0, 66.0, 8.0],
                PlayerState {
                    realm: "qi_refining_2".to_string(),
                    spirit_qi: 130.0,
                    spirit_qi_max: 140.0,
                    karma: -0.2,
                    experience: 700,
                    inventory_score: 0.2,
                },
            );

            app.update();
            flush_all_client_packets(&mut app);

            let baseline_payloads = collect_server_data_payloads(&mut helper);
            assert_eq!(
                extract_player_state_payloads(baseline_payloads.as_slice()).len(),
                1,
                "freshly spawned player state should emit one baseline payload before rejection assertions"
            );
            while rx_outbound.try_recv().is_ok() {}

            app.world_mut()
                .resource_mut::<GameplayActionQueue>()
                .enqueue("offline:Ascetic", GameplayAction::AttemptBreakthrough);

            app.update();
            flush_all_client_packets(&mut app);

            let payloads = collect_server_data_payloads(&mut helper);
            assert!(
                extract_player_state_payloads(payloads.as_slice()).is_empty(),
                "invalid karma should not emit a player_state payload"
            );

            let narration_payloads = extract_narration_payloads(payloads.as_slice());
            assert_eq!(
                narration_payloads.len(),
                1,
                "invalid karma rejection should still emit warning narration"
            );

            match &narration_payloads[0].payload {
                ServerDataPayloadV1::Narration { narrations } => {
                    assert_eq!(narrations.len(), 1);
                    assert_eq!(
                        narrations[0].style,
                        crate::schema::common::NarrationStyle::SystemWarning
                    );
                    assert!(
                        narrations[0].text.contains("心境") || narrations[0].text.contains("因果"),
                        "karma rejection text should mention karma/心境 semantics"
                    );
                }
                other => panic!("expected narration payload, got {other:?}"),
            }

            {
                let world = app.world_mut();
                let player_state = world
                    .entity(entity)
                    .get::<PlayerState>()
                    .expect("player state should remain attached after rejected breakthrough");
                assert_eq!(player_state.realm, "qi_refining_2");
                assert_eq!(player_state.spirit_qi, 130.0);
                assert_eq!(player_state.spirit_qi_max, 140.0);
                assert_eq!(player_state.karma, -0.2);
                assert_eq!(player_state.experience, 700);
                assert_approx_eq(player_state.inventory_score, 0.2);
            }

            let recent_events = app
                .world()
                .resource::<ActiveEventsResource>()
                .recent_events_snapshot();
            assert!(
                recent_events.is_empty(),
                "invalid karma rejection must not append an internal recent event either"
            );
        }
    }
}
