use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker};
use serde_json::{json, Value};
use std::collections::HashMap;
use valence::entity::lightning::LightningEntityBundle;
use valence::entity::zombie::ZombieEntityBundle;
use valence::prelude::{
    bevy_ecs, App, Client, Commands, DVec3, Despawned, Entity, EntityKind, EntityLayerId, Event,
    EventWriter, IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Update, Username, With,
};

use super::zone::ZoneRegistry;
use crate::combat::events::DeathEvent;
use crate::npc::brain::{FleeAction, PlayerProximityScorer, PROXIMITY_THRESHOLD};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype, NpcRegistry};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::persistence::{
    load_zone_overlays, persist_zone_overlays, PersistenceSettings, ZoneOverlayRecord,
};
use crate::player::state::canonical_player_id;
use crate::schema::agent_command::Command;
use crate::schema::common::GameEventType;
use crate::schema::tribulation::{TribulationEventV1, TribulationPhaseV1};
use crate::schema::world_state::GameEvent;
use crate::world::zone::Zone;

pub const EVENT_THUNDER_TRIBULATION: &str = "thunder_tribulation";
pub const EVENT_BEAST_TIDE: &str = "beast_tide";
pub const EVENT_REALM_COLLAPSE: &str = "realm_collapse";
pub const EVENT_KARMA_BACKLASH: &str = "karma_backlash";

const DEFAULT_EVENT_DURATION_TICKS: u64 = 200;
const MIN_EVENT_DURATION_TICKS: u64 = 1;
const RECENT_GAME_EVENTS_LIMIT: usize = 16;
const THUNDER_INTERVAL_TICKS: u64 = 40;
const DEFAULT_EVENT_INTENSITY: f64 = 0.5;
const THUNDER_TARGET_BIAS_RADIUS: f64 = 5.0;
const THUNDER_DEFAULT_Y_OFFSET: f64 = 1.0;
const BEAST_TIDE_BEASTS_PER_INTENSITY: f64 = 10.0;
const KARMA_BACKLASH_EVENT_DURATION_TICKS: u64 = 1;
const COLLAPSED_ZONE_DANGER_LEVEL: u8 = 5;
pub(crate) const REALM_COLLAPSE_EVACUATION_WINDOW_TICKS: u64 = 10 * 60 * 20;

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveEvent {
    pub event_name: String,
    pub zone_name: String,
    pub elapsed_ticks: u64,
    pub duration_ticks: u64,
    intensity: f64,
    target_player: Option<String>,
    thunder: ThunderRuntimeState,
    beast_tide: BeastTideRuntimeState,
    collapse: RealmCollapseRuntimeState,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct ThunderRuntimeState {
    emitted_strikes: Vec<DVec3>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct BeastTideRuntimeState {
    spawned_beasts: Vec<Entity>,
    spawn_points: Vec<DVec3>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct RealmCollapseRuntimeState {
    completed: bool,
    evacuation_warning_emitted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MajorEventAlert {
    pub event_name: String,
    pub zone_name: String,
    pub duration_ticks: u64,
    pub message: Option<String>,
}

impl ActiveEvent {
    fn from_spawn_command(command: &Command) -> Option<Self> {
        let event_name = command.params.get("event")?.as_str()?;
        if !matches!(
            event_name,
            EVENT_THUNDER_TRIBULATION
                | EVENT_BEAST_TIDE
                | EVENT_REALM_COLLAPSE
                | EVENT_KARMA_BACKLASH
        ) {
            return None;
        }

        let duration_ticks = value_to_u64(command.params.get("duration_ticks"))
            .unwrap_or(DEFAULT_EVENT_DURATION_TICKS)
            .max(MIN_EVENT_DURATION_TICKS);

        Some(Self {
            event_name: event_name.to_string(),
            zone_name: command.target.clone(),
            elapsed_ticks: 0,
            duration_ticks,
            intensity: value_to_f64(command.params.get("intensity"))
                .unwrap_or(DEFAULT_EVENT_INTENSITY)
                .clamp(0.0, 1.0),
            target_player: command
                .params
                .get("target_player")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
            thunder: ThunderRuntimeState::default(),
            beast_tide: BeastTideRuntimeState::default(),
            collapse: RealmCollapseRuntimeState::default(),
        })
    }

    fn is_expired(&self) -> bool {
        self.elapsed_ticks >= self.duration_ticks
    }
}

#[derive(Default)]
pub struct ActiveEventsResource {
    active_events: Vec<ActiveEvent>,
    pending_major_alerts: Vec<MajorEventAlert>,
    pending_tribulation_events: Vec<TribulationEventV1>,
    recent_game_events: Vec<GameEvent>,
}

#[derive(Debug, Clone, Event)]
pub struct ZoneCollapsedEvent {
    pub zone_name: String,
}

impl Resource for ActiveEventsResource {}

impl ActiveEventsResource {
    pub fn enqueue_from_spawn_command(
        &mut self,
        command: &Command,
        zone_registry: Option<&mut ZoneRegistry>,
    ) -> bool {
        let Some(event) = ActiveEvent::from_spawn_command(command) else {
            let event_name = command
                .params
                .get("event")
                .and_then(Value::as_str)
                .unwrap_or("<missing>");

            tracing::warn!(
                "[bong][world] spawn_event `{event_name}` is not implemented in M1 scheduler"
            );
            return false;
        };

        let Some(zone_registry) = zone_registry else {
            tracing::warn!(
                "[bong][world] cannot enqueue {} for `{}` because ZoneRegistry resource is missing",
                event.event_name,
                event.zone_name
            );
            return false;
        };

        let Some(zone) = zone_registry.find_zone_mut(event.zone_name.as_str()) else {
            tracing::warn!(
                "[bong][world] {} target zone `{}` was not found",
                event.event_name,
                event.zone_name
            );
            return false;
        };

        if self.contains(event.zone_name.as_str(), event.event_name.as_str()) {
            tracing::info!(
                "[bong][world] ignored duplicate schedule for {} in zone `{}`",
                event.event_name,
                event.zone_name
            );
            return false;
        }

        if event.event_name == EVENT_KARMA_BACKLASH {
            tracing::info!(
                "[bong][world] hidden schedule accepted for {} in zone `{}`",
                event.event_name,
                event.zone_name
            );

            let duration_ticks = value_to_u64(command.params.get("duration_ticks"))
                .unwrap_or(KARMA_BACKLASH_EVENT_DURATION_TICKS)
                .max(MIN_EVENT_DURATION_TICKS);
            let center = zone.center();

            self.record_recent_event(GameEvent {
                event_type: GameEventType::EventTriggered,
                tick: 0,
                player: None,
                target: Some(event.event_name.clone()),
                zone: Some(event.zone_name.clone()),
                details: Some(HashMap::from([
                    ("hidden".to_string(), Value::Bool(true)),
                    (
                        "duration_ticks".to_string(),
                        Value::Number(duration_ticks.into()),
                    ),
                    ("karma_weight".to_string(), json!(event.intensity)),
                ])),
            });
            self.pending_tribulation_events
                .push(TribulationEventV1::targeted(
                    TribulationPhaseV1::Omen,
                    Some(event.zone_name.clone()),
                    Some([center.x, center.y, center.z]),
                ));

            return true;
        }

        if !zone
            .active_events
            .iter()
            .any(|name| name == &event.event_name)
        {
            zone.active_events.push(event.event_name.clone());
        }

        tracing::info!(
            "[bong][world] scheduled {} for zone `{}` (duration_ticks={})",
            event.event_name,
            event.zone_name,
            event.duration_ticks
        );

        self.pending_major_alerts.push(MajorEventAlert {
            event_name: event.event_name.clone(),
            zone_name: event.zone_name.clone(),
            duration_ticks: event.duration_ticks,
            message: None,
        });

        if event.event_name == EVENT_REALM_COLLAPSE {
            let center = zone.center();
            self.pending_tribulation_events
                .push(TribulationEventV1::zone_collapse(
                    TribulationPhaseV1::Omen,
                    Some(event.zone_name.clone()),
                    Some([center.x, center.y, center.z]),
                ));
        }

        self.active_events.push(event);
        true
    }

    pub fn drain_major_event_alerts(&mut self) -> Vec<MajorEventAlert> {
        std::mem::take(&mut self.pending_major_alerts)
    }

    pub fn drain_tribulation_events(&mut self) -> Vec<TribulationEventV1> {
        std::mem::take(&mut self.pending_tribulation_events)
    }

    pub fn record_recent_event(&mut self, event: GameEvent) {
        self.recent_game_events.push(event);

        if self.recent_game_events.len() > RECENT_GAME_EVENTS_LIMIT {
            let overflow = self.recent_game_events.len() - RECENT_GAME_EVENTS_LIMIT;
            self.recent_game_events.drain(0..overflow);
        }
    }

    pub fn recent_events_snapshot(&self) -> Vec<GameEvent> {
        self.recent_game_events.clone()
    }

    fn tick_metadata_only(&mut self, zone_registry: Option<&mut ZoneRegistry>) {
        for event in &mut self.active_events {
            event.elapsed_ticks = event.elapsed_ticks.saturating_add(1);
        }

        let Some(zone_registry) = zone_registry else {
            return;
        };

        self.active_events.retain(|event| {
            if !event.is_expired() {
                return true;
            }

            if let Some(zone) = zone_registry.find_zone_mut(event.zone_name.as_str()) {
                zone.active_events.retain(|name| name != &event.event_name);
            }

            tracing::info!(
                "[bong][world] expired {} for zone `{}` after {} ticks",
                event.event_name,
                event.zone_name,
                event.elapsed_ticks
            );

            false
        });
    }

    /// 推进事件。返回剩余未消耗的 `npc_spawn_budget`（若入参为 `None` 则返回
    /// `None`）。调用方负责把剩余额度通过 `NpcRegistry::release_spawn_batch`
    /// 回滚，以避免预留但未消费的配额把 `spawn_paused` 误触发（P2-5）。
    #[must_use = "leftover budget should be released back to NpcRegistry"]
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        zone_registry: Option<&mut ZoneRegistry>,
        layer_entity: Option<Entity>,
        mut commands: Option<&mut Commands>,
        player_positions: Option<&[(String, DVec3)]>,
        collapse_targets: Option<&[(Entity, DVec3)]>,
        mut death_events: Option<&mut EventWriter<DeathEvent>>,
        mut collapsed_events: Option<&mut EventWriter<ZoneCollapsedEvent>>,
        mut npc_spawn_budget: Option<usize>,
    ) -> Option<usize> {
        let Some(zone_registry) = zone_registry else {
            self.tick_metadata_only(None);
            return npc_spawn_budget;
        };
        let mut recent_events = Vec::new();

        for event in &mut self.active_events {
            if event.is_expired() {
                continue;
            }

            let Some(zone) = zone_registry.find_zone_by_name(event.zone_name.as_str()) else {
                continue;
            };

            let zone = zone.clone();
            match event.event_name.as_str() {
                EVENT_THUNDER_TRIBULATION => {
                    if event
                        .elapsed_ticks
                        .saturating_add(1)
                        .is_multiple_of(THUNDER_INTERVAL_TICKS)
                    {
                        let Some(layer_entity) = layer_entity else {
                            tracing::warn!(
                                "[bong][world] thunder runtime for zone `{}` skipped: missing entity layer",
                                event.zone_name
                            );
                            continue;
                        };

                        let Some(commands) = commands.as_deref_mut() else {
                            tracing::warn!(
                                "[bong][world] thunder runtime for zone `{}` skipped: missing Commands",
                                event.zone_name
                            );
                            continue;
                        };

                        let target_player_position =
                            event.target_player.as_deref().and_then(|target| {
                                resolve_target_player_position(player_positions, target)
                            });
                        let strike_count = thunder_strike_count_for_intensity(event.intensity);

                        for strike_index in 0..strike_count {
                            let strike_position = thunder_strike_position(
                                &zone,
                                target_player_position,
                                strike_index,
                                strike_count,
                            );

                            spawn_lightning(commands, layer_entity, strike_position);
                            event.thunder.emitted_strikes.push(strike_position);
                        }
                    }
                }
                EVENT_BEAST_TIDE => {
                    if event.elapsed_ticks == 0 && event.beast_tide.spawned_beasts.is_empty() {
                        let Some(layer_entity) = layer_entity else {
                            tracing::warn!(
                                "[bong][world] beast_tide runtime for zone `{}` skipped: missing entity layer",
                                event.zone_name
                            );
                            continue;
                        };

                        let Some(commands) = commands.as_deref_mut() else {
                            tracing::warn!(
                                "[bong][world] beast_tide runtime for zone `{}` skipped: missing Commands",
                                event.zone_name
                            );
                            continue;
                        };

                        let desired_beast_count = beast_count_for_intensity(event.intensity);
                        let beast_count = npc_spawn_budget
                            .map(|budget| desired_beast_count.min(budget))
                            .unwrap_or(desired_beast_count);
                        if beast_count == 0 {
                            tracing::info!(
                                "[bong][world] beast_tide runtime for zone `{}` skipped: npc registry budget exhausted",
                                event.zone_name
                            );
                            continue;
                        }
                        if let Some(budget) = npc_spawn_budget.as_mut() {
                            *budget = budget.saturating_sub(beast_count);
                        }
                        for beast_index in 0..beast_count {
                            let spawn_position =
                                beast_spawn_position_on_zone_edge(&zone, beast_index, beast_count);
                            let beast = spawn_beast_tide_zombie(
                                commands,
                                layer_entity,
                                event.zone_name.as_str(),
                                spawn_position,
                                zone.center(),
                            );
                            event.beast_tide.spawned_beasts.push(beast);
                            event.beast_tide.spawn_points.push(spawn_position);
                        }
                    }
                }
                EVENT_REALM_COLLAPSE => {
                    let next_elapsed = event.elapsed_ticks.saturating_add(1);
                    if !event.collapse.evacuation_warning_emitted
                        && !event.collapse.completed
                        && next_elapsed < event.duration_ticks
                    {
                        let remaining_ticks = event.duration_ticks.saturating_sub(next_elapsed);
                        if remaining_ticks <= REALM_COLLAPSE_EVACUATION_WINDOW_TICKS {
                            event.collapse.evacuation_warning_emitted = true;
                            self.pending_major_alerts.push(MajorEventAlert {
                                event_name: event.event_name.clone(),
                                zone_name: event.zone_name.clone(),
                                duration_ticks: remaining_ticks,
                                message: Some(realm_collapse_evacuation_alert_message(
                                    event.zone_name.as_str(),
                                    remaining_ticks,
                                )),
                            });
                            self.pending_tribulation_events.push(
                                TribulationEventV1::zone_collapse(
                                    TribulationPhaseV1::Lock,
                                    Some(event.zone_name.clone()),
                                    Some([zone.center().x, zone.center().y, zone.center().z]),
                                ),
                            );
                        }
                    }

                    if next_elapsed >= event.duration_ticks && !event.collapse.completed {
                        let Some(death_events) = death_events.as_deref_mut() else {
                            tracing::warn!(
                                "[bong][world] realm_collapse runtime for zone `{}` skipped: missing DeathEvent writer",
                                event.zone_name
                            );
                            continue;
                        };

                        let killed = collapse_zone(
                            zone_registry,
                            &zone,
                            next_elapsed,
                            collapse_targets.unwrap_or(&[]),
                            death_events,
                        );
                        if let Some(collapsed_events) = collapsed_events.as_deref_mut() {
                            collapsed_events.send(ZoneCollapsedEvent {
                                zone_name: event.zone_name.clone(),
                            });
                        }
                        event.collapse.completed = true;
                        self.pending_tribulation_events
                            .push(TribulationEventV1::zone_collapse(
                                TribulationPhaseV1::Settle,
                                Some(event.zone_name.clone()),
                                Some([zone.center().x, zone.center().y, zone.center().z]),
                            ));
                        recent_events.push(GameEvent {
                            event_type: GameEventType::EventTriggered,
                            tick: next_elapsed,
                            player: None,
                            target: Some(EVENT_REALM_COLLAPSE.to_string()),
                            zone: Some(event.zone_name.clone()),
                            details: Some(HashMap::from([(
                                "killed_entities".to_string(),
                                Value::Number((killed as u64).into()),
                            )])),
                        });
                    }
                }
                _ => {}
            }
        }

        for event in recent_events {
            self.record_recent_event(event);
        }

        for event in &mut self.active_events {
            event.elapsed_ticks = event.elapsed_ticks.saturating_add(1);
        }

        let mut expired_beasts = Vec::new();
        self.active_events.retain(|event| {
            if !event.is_expired() {
                return true;
            }

            if let Some(zone) = zone_registry.find_zone_mut(event.zone_name.as_str()) {
                if event.event_name != EVENT_REALM_COLLAPSE {
                    zone.active_events.retain(|name| name != &event.event_name);
                }
            }

            if event.event_name == EVENT_BEAST_TIDE {
                expired_beasts.extend(event.beast_tide.spawned_beasts.iter().copied());
            }

            tracing::info!(
                "[bong][world] expired {} for zone `{}` after {} ticks",
                event.event_name,
                event.zone_name,
                event.elapsed_ticks
            );

            false
        });

        if let Some(commands) = commands {
            for beast in expired_beasts {
                if let Some(mut entity_commands) = commands.get_entity(beast) {
                    entity_commands.insert(Despawned);
                }
            }
        }

        npc_spawn_budget
    }

    pub fn contains(&self, zone_name: &str, event_name: &str) -> bool {
        self.active_events
            .iter()
            .any(|event| event.zone_name == zone_name && event.event_name == event_name)
    }

    #[cfg(test)]
    pub fn count_by_zone_and_event(&self, zone_name: &str, event_name: &str) -> usize {
        self.active_events
            .iter()
            .filter(|event| event.zone_name == zone_name && event.event_name == event_name)
            .count()
    }

    #[cfg(test)]
    pub fn elapsed_for_first(&self, zone_name: &str, event_name: &str) -> Option<u64> {
        self.active_events
            .iter()
            .find(|event| event.zone_name == zone_name && event.event_name == event_name)
            .map(|event| event.elapsed_ticks)
    }

    #[cfg(test)]
    pub fn thunder_strikes_for_zone(&self, zone_name: &str) -> Vec<DVec3> {
        self.active_events
            .iter()
            .filter(|event| {
                event.zone_name == zone_name && event.event_name == EVENT_THUNDER_TRIBULATION
            })
            .flat_map(|event| event.thunder.emitted_strikes.iter().copied())
            .collect()
    }

    #[cfg(test)]
    pub fn thunder_target_for_zone(&self, zone_name: &str) -> Option<String> {
        self.active_events
            .iter()
            .find(|event| {
                event.zone_name == zone_name && event.event_name == EVENT_THUNDER_TRIBULATION
            })
            .and_then(|event| event.target_player.clone())
    }

    #[cfg(test)]
    pub fn beast_spawned_entities_for_zone(&self, zone_name: &str) -> Vec<Entity> {
        self.active_events
            .iter()
            .filter(|event| event.zone_name == zone_name && event.event_name == EVENT_BEAST_TIDE)
            .flat_map(|event| event.beast_tide.spawned_beasts.iter().copied())
            .collect()
    }

    #[cfg(test)]
    pub fn beast_spawn_points_for_zone(&self, zone_name: &str) -> Vec<DVec3> {
        self.active_events
            .iter()
            .filter(|event| event.zone_name == zone_name && event.event_name == EVENT_BEAST_TIDE)
            .flat_map(|event| event.beast_tide.spawn_points.iter().copied())
            .collect()
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering active events scheduler");
    app.insert_resource(ActiveEventsResource::default());
    app.add_event::<ZoneCollapsedEvent>();
    app.add_systems(
        Update,
        (
            tick_active_events,
            persist_zone_collapsed_overlays.after(tick_active_events),
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn tick_active_events(
    mut commands: Commands,
    mut active_events: ResMut<ActiveEventsResource>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    mut npc_registry: Option<ResMut<NpcRegistry>>,
    redis: Option<Res<crate::network::RedisBridgeResource>>,
    mut death_events: EventWriter<DeathEvent>,
    mut collapsed_events: EventWriter<ZoneCollapsedEvent>,
    layers: Query<Entity, With<crate::world::dimension::OverworldLayer>>,
    players: Query<(Entity, &Username, &Position), With<Client>>,
    npcs: Query<(Entity, &Position), With<NpcMarker>>,
) {
    let layer_entity = layers.iter().next();
    let mut player_positions = Vec::new();
    let mut collapse_targets = Vec::new();
    for (entity, username, position) in &players {
        let pos = position.get();
        player_positions.push((canonical_player_id(username.0.as_str()), pos));
        collapse_targets.push((entity, pos));
    }
    for (entity, position) in &npcs {
        collapse_targets.push((entity, position.get()));
    }

    let npc_spawn_budget = if let Some(registry) = npc_registry.as_deref_mut() {
        let desired = active_events
            .active_events
            .iter()
            .filter(|event| event.event_name == EVENT_BEAST_TIDE && event.elapsed_ticks == 0)
            .map(|event| beast_count_for_intensity(event.intensity))
            .sum::<usize>();
        let reserved = registry.reserve_spawn_batch(desired);
        if reserved < desired {
            tracing::info!(
                "[bong][world] beast_tide spawn clamped by npc registry: desired={} reserved={} live_npc_count={} max_npc_count={}",
                desired,
                reserved,
                registry.live_npc_count,
                registry.max_npc_count
            );
        }
        Some(reserved)
    } else {
        None
    };

    let leftover = active_events.tick(
        zone_registry.as_deref_mut(),
        layer_entity,
        Some(&mut commands),
        Some(player_positions.as_slice()),
        Some(collapse_targets.as_slice()),
        Some(&mut death_events),
        Some(&mut collapsed_events),
        npc_spawn_budget,
    );

    let tribulation_events = active_events.drain_tribulation_events();
    if let Some(redis) = redis.as_deref() {
        for event in tribulation_events {
            let _ = redis
                .tx_outbound
                .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(event));
        }
    }

    // P2-5: 把 reserve 了但没消费掉（eg. beast_tide 因 missing layer/commands
    // 提前 continue）的额度归还给 registry，防止 1-tick 暂态 `spawn_paused`。
    if let (Some(registry), Some(remaining)) = (npc_registry.as_deref_mut(), leftover) {
        if remaining > 0 {
            registry.release_spawn_batch(remaining);
        }
    }
}

fn value_to_u64(value: Option<&Value>) -> Option<u64> {
    let value = value?;

    if let Some(v) = value.as_u64() {
        return Some(v);
    }

    let v = value.as_i64()?;
    if v < 0 {
        return None;
    }

    Some(v as u64)
}

fn value_to_f64(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    if let Some(v) = value.as_f64() {
        return v.is_finite().then_some(v);
    }

    value.as_i64().map(|v| v as f64)
}

fn realm_collapse_evacuation_alert_message(zone_name: &str, remaining_ticks: u64) -> String {
    format!("域崩撤离窗口已在区域 {zone_name} 开启，剩余 {remaining_ticks} tick；未撤者横死。")
}

pub(crate) fn persist_zone_collapsed_overlays(
    settings: Option<Res<PersistenceSettings>>,
    mut events: valence::prelude::EventReader<ZoneCollapsedEvent>,
) {
    let Some(settings) = settings else {
        return;
    };
    for event in events.read() {
        let mut overlays = match load_zone_overlays(&settings) {
            Ok(overlays) => overlays,
            Err(error) => {
                tracing::warn!(
                    "[bong][persistence] failed to load zone overlays before realm_collapse persist: {error}"
                );
                Vec::new()
            }
        };
        if !overlays.iter().any(|overlay| {
            overlay.zone_id == event.zone_name && overlay.overlay_kind == "collapsed"
        }) {
            overlays.push(ZoneOverlayRecord {
                zone_id: event.zone_name.clone(),
                overlay_kind: "collapsed".to_string(),
                payload_json: json!({
                    "danger_level": COLLAPSED_ZONE_DANGER_LEVEL,
                    "active_events": [EVENT_REALM_COLLAPSE],
                    "blocked_tiles": [],
                })
                .to_string(),
                payload_version: 1,
                since_wall: current_unix_seconds_for_overlay(),
            });
        }
        if let Err(error) = persist_zone_overlays(&settings, &overlays) {
            tracing::warn!(
                "[bong][persistence] failed to persist collapsed overlay for zone `{}`: {error}",
                event.zone_name
            );
        }
    }
}

fn current_unix_seconds_for_overlay() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}

fn resolve_target_player_position(
    player_positions: Option<&[(String, DVec3)]>,
    target_player: &str,
) -> Option<DVec3> {
    let player_positions = player_positions?;
    let normalized_target = canonical_player_id(
        target_player
            .trim()
            .trim_start_matches("offline:")
            .trim_start_matches("OFFLINE:"),
    )
    .to_ascii_lowercase();

    player_positions
        .iter()
        .find(|(player_id, _)| player_id.to_ascii_lowercase() == normalized_target)
        .map(|(_, position)| *position)
}

fn thunder_strike_count_for_intensity(intensity: f64) -> usize {
    let normalized = intensity.clamp(0.0, 1.0);
    if normalized < 0.34 {
        1
    } else if normalized < 0.67 {
        2
    } else {
        3
    }
}

fn beast_count_for_intensity(intensity: f64) -> usize {
    let normalized = intensity.clamp(0.0, 1.0);
    (normalized
        .mul_add(BEAST_TIDE_BEASTS_PER_INTENSITY, 1.0)
        .round() as usize)
        .max(1)
}

fn thunder_strike_position(
    zone: &Zone,
    target_player_position: Option<DVec3>,
    strike_index: usize,
    strike_count: usize,
) -> DVec3 {
    let strike_count = strike_count.max(1);
    let normalized = (strike_index as f64 + 0.5) / strike_count as f64;

    let (min, max) = zone.bounds;
    let zone_edge_margin = 0.5;

    let fallback_x = min.x + (max.x - min.x) * normalized;
    let fallback_z = max.z - (max.z - min.z) * normalized;
    let fallback_y = max.y.min(min.y + THUNDER_DEFAULT_Y_OFFSET.max(0.0));

    let fallback = zone.clamp_position(DVec3::new(fallback_x, fallback_y, fallback_z));

    let Some(target) = target_player_position else {
        return fallback;
    };

    let angle = normalized * std::f64::consts::TAU;
    let offset = DVec3::new(
        angle.cos() * THUNDER_TARGET_BIAS_RADIUS,
        THUNDER_DEFAULT_Y_OFFSET,
        angle.sin() * THUNDER_TARGET_BIAS_RADIUS,
    );
    let biased = zone.clamp_position(target + offset);

    let bounded = DVec3::new(
        biased
            .x
            .clamp(min.x + zone_edge_margin, max.x - zone_edge_margin),
        biased.y.clamp(min.y, max.y),
        biased
            .z
            .clamp(min.z + zone_edge_margin, max.z - zone_edge_margin),
    );

    zone.clamp_position(bounded)
}

fn beast_spawn_position_on_zone_edge(zone: &Zone, beast_index: usize, beast_count: usize) -> DVec3 {
    let beast_count = beast_count.max(1);
    let ratio = (beast_index as f64 + 0.5) / beast_count as f64;
    let (min, max) = zone.bounds;

    let perimeter = ((max.x - min.x) * 2.0 + (max.z - min.z) * 2.0).max(1.0);
    let mut distance = perimeter * ratio;

    let (x, z) = if distance <= (max.x - min.x) {
        (min.x + distance, min.z)
    } else {
        distance -= max.x - min.x;
        if distance <= (max.z - min.z) {
            (max.x, min.z + distance)
        } else {
            distance -= max.z - min.z;
            if distance <= (max.x - min.x) {
                (max.x - distance, max.z)
            } else {
                distance -= max.x - min.x;
                (min.x, max.z - distance)
            }
        }
    };

    let y = zone.center().y;
    zone.clamp_position(DVec3::new(x, y, z))
}

fn spawn_lightning(commands: &mut Commands, layer_entity: Entity, position: DVec3) -> Entity {
    commands
        .spawn(LightningEntityBundle {
            kind: EntityKind::LIGHTNING,
            layer: EntityLayerId(layer_entity),
            position: Position::new([position.x, position.y, position.z]),
            ..Default::default()
        })
        .id()
}

fn spawn_beast_tide_zombie(
    commands: &mut Commands,
    layer_entity: Entity,
    zone_name: &str,
    spawn_position: DVec3,
    zone_center: DVec3,
) -> Entity {
    let entity = commands
        .spawn((
            ZombieEntityBundle {
                kind: EntityKind::ZOMBIE,
                layer: EntityLayerId(layer_entity),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            },
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            NpcArchetype::Beast,
            NpcPatrol::new(zone_name, zone_center),
            Thinker::build()
                .picker(FirstToScore {
                    threshold: PROXIMITY_THRESHOLD,
                })
                .when(PlayerProximityScorer, FleeAction),
        ))
        .id();

    commands
        .entity(entity)
        .insert(npc_runtime_bundle(entity, NpcArchetype::Beast));

    entity
}

fn collapse_zone(
    zone_registry: &mut ZoneRegistry,
    zone: &Zone,
    collapse_tick: u64,
    collapse_targets: &[(Entity, DVec3)],
    death_events: &mut EventWriter<DeathEvent>,
) -> usize {
    let Some(active_zone) = zone_registry.find_zone_mut(zone.name.as_str()) else {
        return 0;
    };

    active_zone.spirit_qi = 0.0;
    active_zone.danger_level = COLLAPSED_ZONE_DANGER_LEVEL;
    if !active_zone
        .active_events
        .iter()
        .any(|name| name == EVENT_REALM_COLLAPSE)
    {
        active_zone
            .active_events
            .push(EVENT_REALM_COLLAPSE.to_string());
    }

    let mut killed = 0usize;
    for (entity, position) in collapse_targets
        .iter()
        .copied()
        .filter(|(_, position)| zone.contains(*position))
    {
        death_events.send(DeathEvent {
            target: entity,
            cause: "realm_collapse".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: collapse_tick,
        });
        killed += 1;
        tracing::info!(
            "[bong][world] realm_collapse killed entity={:?} at ({:.1},{:.1},{:.1}) in zone `{}`",
            entity,
            position.x,
            position.y,
            position.z,
            zone.name
        );
    }

    killed
}

#[cfg(test)]
mod events_tests {
    use std::collections::HashMap;

    use serde_json::json;
    use serde_json::Value;
    use valence::entity::lightning::LightningEntity;
    use valence::prelude::{
        bevy_ecs, App, DVec3, Entity, EntityKind, Events, IntoSystemConfigs, Position, Update, With,
    };
    use valence::testing::{create_mock_client, ScenarioSingleClient};

    use super::{
        persist_zone_collapsed_overlays, tick_active_events, ActiveEventsResource,
        ZoneCollapsedEvent, COLLAPSED_ZONE_DANGER_LEVEL, EVENT_BEAST_TIDE, EVENT_KARMA_BACKLASH,
        EVENT_REALM_COLLAPSE, EVENT_THUNDER_TRIBULATION, REALM_COLLAPSE_EVACUATION_WINDOW_TICKS,
    };
    use crate::combat::events::DeathEvent;
    use crate::npc::lifecycle::NpcRegistry;
    use crate::npc::patrol::NpcPatrol;
    use crate::npc::spawn::NpcMarker;
    use crate::persistence::{bootstrap_sqlite, load_zone_overlays, PersistenceSettings};
    use crate::schema::agent_command::Command;
    use crate::schema::common::CommandType;
    use crate::schema::tribulation::{TribulationKindV1, TribulationPhaseV1};
    use crate::world::zone::Zone;
    use crate::world::zone::ZoneRegistry;
    use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

    fn spawn_event_command(target: &str, event: &str, duration_ticks: u64) -> Command {
        let mut params = HashMap::new();
        params.insert("event".to_string(), json!(event));
        params.insert("duration_ticks".to_string(), json!(duration_ticks));

        Command {
            command_type: CommandType::SpawnEvent,
            target: target.to_string(),
            params,
        }
    }

    fn spawn_event_command_with_params(
        target: &str,
        event: &str,
        duration_ticks: u64,
        intensity: f64,
        target_player: Option<&str>,
    ) -> Command {
        let mut params = HashMap::new();
        params.insert("event".to_string(), json!(event));
        params.insert("duration_ticks".to_string(), json!(duration_ticks));
        params.insert("intensity".to_string(), json!(intensity));
        if let Some(target_player) = target_player {
            params.insert("target_player".to_string(), json!(target_player));
        }

        Command {
            command_type: CommandType::SpawnEvent,
            target: target.to_string(),
            params,
        }
    }

    fn setup_events_app() -> (App, Entity) {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        app.world_mut()
            .entity_mut(layer)
            .insert(crate::world::dimension::OverworldLayer);
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(ActiveEventsResource::default());
        app.add_event::<DeathEvent>();
        app.add_event::<ZoneCollapsedEvent>();
        app.add_systems(Update, tick_active_events);
        (app, layer)
    }

    fn spawn_mock_player(
        app: &mut App,
        layer: Entity,
        username: &str,
        position: [f64; 3],
    ) -> Entity {
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);
        client_bundle.player.layer.0 = layer;
        client_bundle.visible_chunk_layer.0 = layer;
        client_bundle.visible_entity_layers.0.insert(layer);

        app.world_mut().spawn(client_bundle).id()
    }

    fn query_npc_entities(world: &mut bevy_ecs::world::World) -> Vec<Entity> {
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).collect::<Vec<_>>()
    }

    fn query_lightning_entities(world: &mut bevy_ecs::world::World) -> Vec<Entity> {
        let mut query = world.query_filtered::<Entity, With<LightningEntity>>();
        query.iter(world).collect::<Vec<_>>()
    }

    fn is_on_zone_edge(zone: &Zone, position: DVec3) -> bool {
        let (min, max) = zone.bounds;
        let epsilon = 1e-6;

        (position.x - min.x).abs() <= epsilon
            || (position.x - max.x).abs() <= epsilon
            || (position.z - min.z).abs() <= epsilon
            || (position.z - max.z).abs() <= epsilon
    }

    #[test]
    fn thunder_event_ticks_until_expiry() {
        let (mut app, layer) = setup_events_app();
        let _target_player = spawn_mock_player(&mut app, layer, "Steve", [8.0, 66.0, 8.0]);

        {
            let world = app.world_mut();
            let command = spawn_event_command_with_params(
                "spawn",
                EVENT_THUNDER_TRIBULATION,
                82,
                0.8,
                Some("offline:Steve"),
            );
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                events.enqueue_from_spawn_command(&command, Some(&mut zones));
            });
        }

        {
            let world = app.world();
            let zone = world
                .resource::<ZoneRegistry>()
                .find_zone(
                    crate::world::dimension::DimensionKind::Overworld,
                    DVec3::new(8.0, 66.0, 8.0),
                )
                .expect("spawn zone should exist");
            assert!(zone
                .active_events
                .iter()
                .any(|event| event == EVENT_THUNDER_TRIBULATION));
            assert!(world
                .resource::<ActiveEventsResource>()
                .contains("spawn", EVENT_THUNDER_TRIBULATION));
            assert_eq!(
                world
                    .resource::<ActiveEventsResource>()
                    .thunder_target_for_zone("spawn")
                    .as_deref(),
                Some("offline:Steve")
            );
        }

        for _ in 0..40 {
            app.update();
        }
        {
            let world = app.world_mut();
            let events = world.resource::<ActiveEventsResource>();
            assert_eq!(
                events.elapsed_for_first("spawn", EVENT_THUNDER_TRIBULATION),
                Some(40)
            );

            let strikes = events.thunder_strikes_for_zone("spawn");
            assert_eq!(
                strikes.len(),
                3,
                "intensity=0.8 should emit 3 strikes per 40-tick cadence"
            );

            assert!(
                strikes
                    .iter()
                    .all(|strike| strike.distance_squared(DVec3::new(8.0, 66.0, 8.0)) <= 64.0),
                "target_player bias should place strikes near target player"
            );

            let lightning_entities = query_lightning_entities(world);
            assert_eq!(
                lightning_entities.len(),
                3,
                "thunder runtime should spawn concrete lightning entities"
            );
        }

        for _ in 0..42 {
            app.update();
        }
        {
            let world = app.world();
            let zone = world
                .resource::<ZoneRegistry>()
                .find_zone(
                    crate::world::dimension::DimensionKind::Overworld,
                    DVec3::new(8.0, 66.0, 8.0),
                )
                .expect("spawn zone should exist");
            assert!(
                !zone
                    .active_events
                    .iter()
                    .any(|event| event == EVENT_THUNDER_TRIBULATION),
                "thunder event should be removed from zone after expiry"
            );
            assert!(
                !world
                    .resource::<ActiveEventsResource>()
                    .contains("spawn", EVENT_THUNDER_TRIBULATION),
                "thunder event should be removed from scheduler after expiry"
            );
        }
    }

    #[test]
    fn thunder_intensity_scales_runtime_strike_density() {
        let (mut low_app, _low_layer) = setup_events_app();
        let (mut high_app, _high_layer) = setup_events_app();

        {
            let world = low_app.world_mut();
            let low =
                spawn_event_command_with_params("spawn", EVENT_THUNDER_TRIBULATION, 45, 0.1, None);
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                world
                    .resource_mut::<ActiveEventsResource>()
                    .enqueue_from_spawn_command(&low, Some(&mut zones));
            });
        }

        {
            let world = high_app.world_mut();
            let high =
                spawn_event_command_with_params("spawn", EVENT_THUNDER_TRIBULATION, 45, 0.95, None);
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                world
                    .resource_mut::<ActiveEventsResource>()
                    .enqueue_from_spawn_command(&high, Some(&mut zones));
            });
        }

        for _ in 0..40 {
            low_app.update();
            high_app.update();
        }

        let low_count = low_app
            .world()
            .resource::<ActiveEventsResource>()
            .thunder_strikes_for_zone("spawn")
            .len();
        let high_count = high_app
            .world()
            .resource::<ActiveEventsResource>()
            .thunder_strikes_for_zone("spawn")
            .len();

        assert_eq!(
            low_count, 1,
            "low intensity should emit one strike per cadence"
        );
        assert_eq!(
            high_count, 3,
            "high intensity should emit denser strikes per cadence"
        );

        let low_lightning = {
            let world = low_app.world_mut();
            query_lightning_entities(world).len()
        };
        let high_lightning = {
            let world = high_app.world_mut();
            query_lightning_entities(world).len()
        };

        assert_eq!(low_lightning, 1);
        assert_eq!(high_lightning, 3);
    }

    #[test]
    fn beast_tide_event_spawns_and_cleans_up() {
        let (mut app, _layer) = setup_events_app();

        {
            let world = app.world_mut();
            let command = spawn_event_command_with_params("spawn", EVENT_BEAST_TIDE, 3, 0.6, None);
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                events.enqueue_from_spawn_command(&command, Some(&mut zones));
            });
        }

        {
            let world = app.world();
            let zone = world
                .resource::<ZoneRegistry>()
                .find_zone(
                    crate::world::dimension::DimensionKind::Overworld,
                    DVec3::new(8.0, 66.0, 8.0),
                )
                .expect("spawn zone should exist");
            assert!(zone
                .active_events
                .iter()
                .any(|event| event == EVENT_BEAST_TIDE));
            assert!(world
                .resource::<ActiveEventsResource>()
                .contains("spawn", EVENT_BEAST_TIDE));
        }

        app.update();
        {
            let world = app.world_mut();
            let spawned_beasts = world
                .resource::<ActiveEventsResource>()
                .beast_spawned_entities_for_zone("spawn");
            let spawn_points = world
                .resource::<ActiveEventsResource>()
                .beast_spawn_points_for_zone("spawn");

            assert!(
                !spawned_beasts.is_empty(),
                "beast_tide should spawn runtime beasts"
            );
            assert_eq!(
                spawned_beasts.len(),
                spawn_points.len(),
                "tracked beast entities and spawn points should align"
            );

            let zone = world
                .resource::<ZoneRegistry>()
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist")
                .clone();

            for entity in &spawned_beasts {
                assert!(
                    world.get::<NpcMarker>(*entity).is_some(),
                    "spawned beast should include NpcMarker"
                );
                assert_eq!(
                    *world
                        .get::<EntityKind>(*entity)
                        .expect("spawned beast should have EntityKind"),
                    EntityKind::ZOMBIE,
                    "beast_tide runtime should spawn zombie entities"
                );
                let patrol = world
                    .get::<NpcPatrol>(*entity)
                    .expect("spawned beast should include NpcPatrol");
                assert_eq!(patrol.home_zone, DEFAULT_SPAWN_ZONE_NAME);
                assert!(
                    patrol.current_target.distance_squared(zone.center()) < 1e-9,
                    "beast patrol target should be zone center"
                );
            }

            assert!(
                spawn_points.iter().all(|pos| zone.contains(*pos)),
                "beast spawns should stay inside authoritative zone bounds"
            );
            assert!(
                spawn_points.iter().all(|pos| is_on_zone_edge(&zone, *pos)),
                "beast_tide should spawn beasts on zone edge"
            );
            assert!(
                query_npc_entities(world).len() >= spawned_beasts.len(),
                "live world should contain spawned beasts"
            );
        }

        app.update();
        app.update();
        {
            let world = app.world_mut();
            let zone = world
                .resource::<ZoneRegistry>()
                .find_zone(
                    crate::world::dimension::DimensionKind::Overworld,
                    DVec3::new(8.0, 66.0, 8.0),
                )
                .expect("spawn zone should exist");
            assert!(
                !zone
                    .active_events
                    .iter()
                    .any(|event| event == EVENT_BEAST_TIDE),
                "beast_tide should be removed from zone when duration elapses"
            );
            assert!(
                !world
                    .resource::<ActiveEventsResource>()
                    .contains("spawn", EVENT_BEAST_TIDE),
                "beast_tide should be removed from scheduler when duration elapses"
            );

            let lingering = query_npc_entities(world);
            assert!(
                lingering.is_empty(),
                "beast_tide-spawned NPCs should be despawned after expiry"
            );
        }
    }

    #[test]
    fn spawn_event_only_enters_scheduler_once() {
        let (mut app, _layer) = setup_events_app();
        let command = spawn_event_command("spawn", EVENT_THUNDER_TRIBULATION, 3);

        {
            let world = app.world_mut();
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                events.enqueue_from_spawn_command(&command, Some(&mut zones));
                events.enqueue_from_spawn_command(&command, Some(&mut zones));
            });
        }

        let world = app.world();
        let events = world.resource::<ActiveEventsResource>();
        let zone = world
            .resource::<ZoneRegistry>()
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("spawn zone should exist");

        assert_eq!(
            events.count_by_zone_and_event("spawn", EVENT_THUNDER_TRIBULATION),
            1,
            "repeated spawn_event should only register one scheduled thunder event"
        );
        assert_eq!(
            zone.active_events
                .iter()
                .filter(|name| name.as_str() == EVENT_THUNDER_TRIBULATION)
                .count(),
            1,
            "spawn zone should expose thunder exactly once in stable active_events"
        );

        app.update();
        app.update();
        app.update();

        let world = app.world();
        let events = world.resource::<ActiveEventsResource>();
        let zone = world
            .resource::<ZoneRegistry>()
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("spawn zone should exist");

        assert!(
            !events.contains("spawn", EVENT_THUNDER_TRIBULATION),
            "expired thunder event should be removed from scheduler after cleanup"
        );
        assert!(
            !zone
                .active_events
                .iter()
                .any(|event| event == EVENT_THUNDER_TRIBULATION),
            "expired thunder event should be removed from zone after cleanup"
        );
    }

    #[test]
    fn realm_collapse_collapses_zone_and_kills_occupants() {
        let (mut app, layer) = setup_events_app();
        let player = spawn_mock_player(&mut app, layer, "Azure", [8.0, 66.0, 8.0]);
        let npc = app
            .world_mut()
            .spawn((NpcMarker, Position::new([10.0, 66.0, 10.0])))
            .id();
        let outsider = spawn_mock_player(&mut app, layer, "Far", [300.0, 66.0, 300.0]);
        let command = spawn_event_command("spawn", EVENT_REALM_COLLAPSE, 2);

        {
            let world = app.world_mut();
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                let accepted = events.enqueue_from_spawn_command(&command, Some(&mut zones));
                assert!(accepted, "realm_collapse event should be accepted");
            });
        }

        assert!(app
            .world()
            .resource::<ActiveEventsResource>()
            .contains("spawn", EVENT_REALM_COLLAPSE));

        app.update();
        app.update();

        let world = app.world();
        let zone = world
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist");
        assert_eq!(zone.spirit_qi, 0.0);
        assert_eq!(zone.danger_level, COLLAPSED_ZONE_DANGER_LEVEL);
        assert!(zone
            .active_events
            .iter()
            .any(|event| event == EVENT_REALM_COLLAPSE));

        assert!(
            !world
                .resource::<ActiveEventsResource>()
                .contains("spawn", EVENT_REALM_COLLAPSE),
            "realm_collapse should leave scheduler after collapse while zone keeps collapsed marker"
        );

        let deaths = world.resource::<Events<DeathEvent>>();
        let collected: Vec<_> = deaths.get_reader().read(deaths).cloned().collect();
        assert!(collected
            .iter()
            .any(|event| event.target == player && event.cause == "realm_collapse"));
        assert!(collected
            .iter()
            .any(|event| event.target == npc && event.cause == "realm_collapse"));
        assert!(
            !collected.iter().any(|event| event.target == outsider),
            "realm_collapse must not kill entities outside zone bounds"
        );
    }

    #[test]
    fn realm_collapse_emits_evacuation_warning_before_collapse() {
        let mut zones = ZoneRegistry::fallback();
        let mut events = ActiveEventsResource::default();
        let duration_ticks = REALM_COLLAPSE_EVACUATION_WINDOW_TICKS + 2;
        let command = spawn_event_command(
            DEFAULT_SPAWN_ZONE_NAME,
            EVENT_REALM_COLLAPSE,
            duration_ticks,
        );

        assert!(events.enqueue_from_spawn_command(&command, Some(&mut zones)));
        let initial_alerts = events.drain_major_event_alerts();
        assert_eq!(initial_alerts.len(), 1);
        assert_eq!(initial_alerts[0].duration_ticks, duration_ticks);
        assert!(initial_alerts[0].message.is_none());
        let initial_events = events.drain_tribulation_events();
        assert_eq!(initial_events.len(), 1);
        assert_eq!(initial_events[0].kind, TribulationKindV1::ZoneCollapse);
        assert_eq!(initial_events[0].phase, TribulationPhaseV1::Omen);

        let _ = events.tick(Some(&mut zones), None, None, None, None, None, None, None);
        assert!(events.drain_major_event_alerts().is_empty());
        assert!(events.drain_tribulation_events().is_empty());

        let _ = events.tick(Some(&mut zones), None, None, None, None, None, None, None);
        let evacuation_alerts = events.drain_major_event_alerts();
        assert_eq!(evacuation_alerts.len(), 1);
        assert_eq!(
            evacuation_alerts[0].duration_ticks,
            REALM_COLLAPSE_EVACUATION_WINDOW_TICKS
        );
        assert!(evacuation_alerts[0]
            .message
            .as_deref()
            .is_some_and(|message| message.contains("撤离窗口") && message.contains("横死")));
        let lock_events = events.drain_tribulation_events();
        assert_eq!(lock_events.len(), 1);
        assert_eq!(lock_events[0].kind, TribulationKindV1::ZoneCollapse);
        assert_eq!(lock_events[0].phase, TribulationPhaseV1::Lock);

        let _ = events.tick(Some(&mut zones), None, None, None, None, None, None, None);
        assert!(events.drain_major_event_alerts().is_empty());
        assert!(events.drain_tribulation_events().is_empty());
    }

    #[test]
    fn realm_collapse_persists_collapsed_overlay() {
        let (mut app, layer) = setup_events_app();
        let db_path = unique_test_db("realm-collapse-persists-overlay");
        std::fs::create_dir_all(db_path.parent().expect("test db should have parent"))
            .expect("test db parent should be creatable");
        let settings = PersistenceSettings::with_paths(
            db_path.clone(),
            db_path.with_extension("deceased"),
            "realm-collapse-test",
        );
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("test sqlite should bootstrap");
        app.insert_resource(settings.clone());
        app.add_systems(
            Update,
            persist_zone_collapsed_overlays.after(tick_active_events),
        );
        let _player = spawn_mock_player(&mut app, layer, "Azure", [8.0, 66.0, 8.0]);

        {
            let world = app.world_mut();
            let command = spawn_event_command("spawn", EVENT_REALM_COLLAPSE, 1);
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                world
                    .resource_mut::<ActiveEventsResource>()
                    .enqueue_from_spawn_command(&command, Some(&mut zones));
            });
        }

        app.update();

        let overlays = load_zone_overlays(&settings).expect("collapsed overlay should load");
        assert!(overlays.iter().any(|overlay| {
            overlay.zone_id == DEFAULT_SPAWN_ZONE_NAME
                && overlay.overlay_kind == "collapsed"
                && overlay.payload_version == 1
                && overlay.payload_json.contains(EVENT_REALM_COLLAPSE)
        }));
    }

    #[test]
    fn concurrent_beast_tides_share_npc_registry_budget() {
        let (mut app, _layer) = setup_events_app();
        app.insert_resource(NpcRegistry {
            max_npc_count: 10,
            resume_npc_count: 8,
            ..NpcRegistry::default()
        });
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .zones
            .push(Zone {
                name: "forest".to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
                bounds: (
                    DVec3::new(100.0, 60.0, 100.0),
                    DVec3::new(200.0, 80.0, 200.0),
                ),
                spirit_qi: 0.5,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: vec![DVec3::new(150.0, 70.0, 150.0)],
                blocked_tiles: Vec::new(),
            });

        {
            let world = app.world_mut();
            let cmd_spawn =
                spawn_event_command_with_params("spawn", EVENT_BEAST_TIDE, 6, 0.7, None);
            let cmd_forest =
                spawn_event_command_with_params("forest", EVENT_BEAST_TIDE, 6, 0.7, None);
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                assert!(events.enqueue_from_spawn_command(&cmd_spawn, Some(&mut zones)));
                assert!(events.enqueue_from_spawn_command(&cmd_forest, Some(&mut zones)));
            });
        }

        app.update();

        let live = query_npc_entities(app.world_mut()).len();
        let cap = app.world().resource::<NpcRegistry>().max_npc_count;
        assert!(
            live <= cap,
            "concurrent beast_tides must share the reserved npc budget: live={live} cap={cap}"
        );
    }

    #[test]
    fn beast_tide_releases_leftover_budget_when_no_layer_available() {
        // P2-5: 当 beast_tide 因 missing layer 提前 continue 时，
        // 事先 reserve 的 npc 配额必须回流到 NpcRegistry —— 否则
        // 同 tick 内 `live_npc_count >= resume_npc_count` 可能误触
        // `spawn_paused=true`，击杀后续 spawn。
        let mut registry = NpcRegistry::default();
        let reserved = registry.reserve_spawn_batch(5);
        assert_eq!(reserved, 5);
        assert_eq!(registry.live_npc_count, 5);

        let mut zones = ZoneRegistry::fallback();
        let mut events = ActiveEventsResource::default();
        let cmd = spawn_event_command_with_params(
            DEFAULT_SPAWN_ZONE_NAME,
            EVENT_BEAST_TIDE,
            6,
            0.7,
            None,
        );
        assert!(events.enqueue_from_spawn_command(&cmd, Some(&mut zones)));

        // 不传 layer / commands，模拟"事件已 enqueue 但 chunk layer 尚未就位"。
        let leftover = events.tick(
            Some(&mut zones),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(reserved),
        );
        assert_eq!(
            leftover,
            Some(reserved),
            "tick must return the full reserved budget when spawn could not occur"
        );

        registry.release_spawn_batch(leftover.unwrap_or(0));
        assert_eq!(
            registry.live_npc_count, 0,
            "leftover budget must be released back to NpcRegistry"
        );
        assert!(
            !registry.spawn_paused,
            "release must un-pause registry if live_npc_count drops below resume threshold"
        );
    }

    #[test]
    fn hidden_karma_backlash_records_internal_marker() {
        let (mut app, _layer) = setup_events_app();
        let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);

        {
            let world = app.world_mut();
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                let accepted = events.enqueue_from_spawn_command(&command, Some(&mut zones));
                assert!(accepted, "hidden event should be accepted");
            });
        }

        let world = app.world();
        let events = world.resource::<ActiveEventsResource>();

        assert!(
            !events.contains("spawn", EVENT_KARMA_BACKLASH),
            "hidden event should not remain in active scheduler queue"
        );

        let recent = events.recent_events_snapshot();
        assert!(
            recent.iter().any(
                |event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH)
                    && event.zone.as_deref() == Some("spawn")
                    && event
                        .details
                        .as_ref()
                        .and_then(|details| details.get("hidden"))
                        .is_some_and(|flag| flag == &Value::Bool(true))
            ),
            "karma_backlash should append an internal hidden marker"
        );
    }

    fn unique_test_db(test_name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("bong-world-events-{test_name}-{nanos}"))
            .join("bong.db")
    }
}
