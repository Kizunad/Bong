use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker};
use serde_json::Value;
use std::collections::HashMap;
use valence::entity::lightning::LightningEntityBundle;
use valence::entity::zombie::ZombieEntityBundle;
use valence::prelude::{
    App, ChunkLayer, Client, Commands, DVec3, Despawned, Entity, EntityKind, EntityLayer,
    EntityLayerId, Position, Query, ResMut, Resource, Update, Username, With,
};

use super::zone::ZoneRegistry;
use crate::npc::brain::{FleeAction, PlayerProximityScorer, PROXIMITY_THRESHOLD};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::player::state::canonical_player_id;
use crate::schema::agent_command::Command;
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
const PLACEHOLDER_EVENT_DURATION_TICKS: u64 = 1;

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

#[derive(Debug, Clone, PartialEq)]
pub struct MajorEventAlert {
    pub event_name: String,
    pub zone_name: String,
    pub duration_ticks: u64,
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
    recent_game_events: Vec<GameEvent>,
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

        let is_placeholder = matches!(
            event.event_name.as_str(),
            EVENT_REALM_COLLAPSE | EVENT_KARMA_BACKLASH
        );

        if is_placeholder {
            tracing::info!(
                "[bong][world] placeholder schedule accepted for {} in zone `{}`",
                event.event_name,
                event.zone_name
            );

            let placeholder_duration = value_to_u64(command.params.get("duration_ticks"))
                .unwrap_or(PLACEHOLDER_EVENT_DURATION_TICKS)
                .max(MIN_EVENT_DURATION_TICKS);

            self.pending_major_alerts.push(MajorEventAlert {
                event_name: event.event_name.clone(),
                zone_name: event.zone_name.clone(),
                duration_ticks: placeholder_duration,
            });

            self.record_recent_event(GameEvent {
                event_type: crate::schema::common::GameEventType::EventTriggered,
                tick: 0,
                player: None,
                target: Some(event.event_name.clone()),
                zone: Some(event.zone_name.clone()),
                details: Some(HashMap::from([(
                    "placeholder".to_string(),
                    Value::Bool(true),
                )])),
            });

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
        });

        self.active_events.push(event);
        true
    }

    pub fn drain_major_event_alerts(&mut self) -> Vec<MajorEventAlert> {
        std::mem::take(&mut self.pending_major_alerts)
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

    pub fn tick(
        &mut self,
        zone_registry: Option<&mut ZoneRegistry>,
        layer_entity: Option<Entity>,
        mut commands: Option<&mut Commands>,
        player_positions: Option<&[(String, DVec3)]>,
    ) {
        let Some(zone_registry) = zone_registry else {
            self.tick_metadata_only(None);
            return;
        };

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

                        let beast_count = beast_count_for_intensity(event.intensity);
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
                _ => {}
            }
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
                zone.active_events.retain(|name| name != &event.event_name);
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
    app.add_systems(Update, tick_active_events);
}

fn tick_active_events(
    mut commands: Commands,
    mut active_events: ResMut<ActiveEventsResource>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
    players: Query<(&Username, &Position), With<Client>>,
) {
    let layer_entity = layers.iter().next();
    let player_positions = players
        .iter()
        .map(|(username, position)| (canonical_player_id(username.0.as_str()), position.get()))
        .collect::<Vec<_>>();

    active_events.tick(
        zone_registry.as_deref_mut(),
        layer_entity,
        Some(&mut commands),
        Some(player_positions.as_slice()),
    );
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
    commands
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
            NpcPatrol::new(zone_name, zone_center),
            Thinker::build()
                .picker(FirstToScore {
                    threshold: PROXIMITY_THRESHOLD,
                })
                .when(PlayerProximityScorer, FleeAction),
        ))
        .id()
}

#[cfg(test)]
mod events_tests {
    use std::collections::HashMap;

    use serde_json::json;
    use serde_json::Value;
    use valence::entity::lightning::LightningEntity;
    use valence::prelude::{bevy_ecs, App, DVec3, Entity, EntityKind, Position, Update, With};
    use valence::testing::{create_mock_client, ScenarioSingleClient};

    use super::{
        tick_active_events, ActiveEventsResource, EVENT_BEAST_TIDE, EVENT_KARMA_BACKLASH,
        EVENT_REALM_COLLAPSE, EVENT_THUNDER_TRIBULATION,
    };
    use crate::npc::patrol::NpcPatrol;
    use crate::npc::spawn::NpcMarker;
    use crate::schema::agent_command::Command;
    use crate::schema::common::CommandType;
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
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(ActiveEventsResource::default());
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
                .find_zone(DVec3::new(8.0, 66.0, 8.0))
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
                .find_zone(DVec3::new(8.0, 66.0, 8.0))
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
                .find_zone(DVec3::new(8.0, 66.0, 8.0))
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
                .find_zone(DVec3::new(8.0, 66.0, 8.0))
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
            .find_zone(DVec3::new(8.0, 66.0, 8.0))
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
            .find_zone(DVec3::new(8.0, 66.0, 8.0))
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
    fn placeholder_realm_collapse_produces_alert_and_recent_event() {
        let (mut app, _layer) = setup_events_app();
        let command = spawn_event_command("spawn", EVENT_REALM_COLLAPSE, 2);

        {
            let world = app.world_mut();
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                let accepted = events.enqueue_from_spawn_command(&command, Some(&mut zones));
                assert!(accepted, "placeholder event should be accepted");
            });
        }

        let world = app.world();
        let events = world.resource::<ActiveEventsResource>();

        assert!(
            !events.contains("spawn", EVENT_REALM_COLLAPSE),
            "placeholder event should not remain in active scheduler queue"
        );

        let recent = events.recent_events_snapshot();
        assert!(
            recent.iter().any(
                |event| event.target.as_deref() == Some(EVENT_REALM_COLLAPSE)
                    && event.zone.as_deref() == Some("spawn")
                    && event
                        .details
                        .as_ref()
                        .and_then(|details| details.get("placeholder"))
                        .is_some_and(|flag| flag == &Value::Bool(true))
            ),
            "realm_collapse placeholder should append verifiable recent event"
        );
    }

    #[test]
    fn placeholder_karma_backlash_produces_alert_and_recent_event() {
        let (mut app, _layer) = setup_events_app();
        let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);

        {
            let world = app.world_mut();
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                let accepted = events.enqueue_from_spawn_command(&command, Some(&mut zones));
                assert!(accepted, "placeholder event should be accepted");
            });
        }

        let world = app.world();
        let events = world.resource::<ActiveEventsResource>();

        assert!(
            !events.contains("spawn", EVENT_KARMA_BACKLASH),
            "placeholder event should not remain in active scheduler queue"
        );

        let recent = events.recent_events_snapshot();
        assert!(
            recent.iter().any(
                |event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH)
                    && event.zone.as_deref() == Some("spawn")
                    && event
                        .details
                        .as_ref()
                        .and_then(|details| details.get("placeholder"))
                        .is_some_and(|flag| flag == &Value::Bool(true))
            ),
            "karma_backlash placeholder should append verifiable recent event"
        );
    }
}
