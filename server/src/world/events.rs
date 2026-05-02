use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use valence::entity::lightning::LightningEntityBundle;
use valence::entity::zombie::ZombieEntityBundle;
use valence::prelude::{
    bevy_ecs, App, Client, Commands, DVec3, Despawned, Entity, EntityKind, EntityLayerId, Event,
    EventWriter, Events, IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Update,
    Username, With,
};

use super::zone::ZoneRegistry;
use crate::combat::events::DeathEvent;
use crate::fauna::components::{fauna_spawn_seed, fauna_tag_for_beast_spawn};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::brain::{FleeAction, PlayerProximityScorer, PROXIMITY_THRESHOLD};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype, NpcRegistry};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::persistence::{
    load_zone_overlays, persist_zone_overlays, PersistenceSettings, ZoneOverlayRecord,
    ZONE_OVERLAY_PAYLOAD_VERSION,
};
use crate::player::state::canonical_player_id;
use crate::schema::agent_command::Command;
use crate::schema::common::{CommandType, GameEventType};
use crate::schema::tribulation::{TribulationEventV1, TribulationPhaseV1};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::schema::world_state::GameEvent;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::karma::{
    targeted_calamity_event_hit, targeted_calamity_roll, KarmaWeightStore, QiDensityHeatmap,
    TARGETED_CALAMITY_BASE_PROBABILITY, TARGETED_CALAMITY_MAX_PROBABILITY,
    TARGETED_QI_NULLIFICATION_HEAT_THRESHOLD,
};
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
const TARGETED_LIGHTNING_VFX_EVENT_ID: &str = "bong:tribulation_lightning";
const TARGETED_LIGHTNING_VFX_COLOR: &str = "#D0C8FF";
const TARGETED_LIGHTNING_VFX_COUNT: u16 = 3;
const TARGETED_LIGHTNING_VFX_DURATION_TICKS: u16 = 14;
const COLLAPSED_ZONE_DANGER_LEVEL: u8 = 5;
pub(crate) const REALM_COLLAPSE_LOW_QI_THRESHOLD: f64 = 0.1;
pub(crate) const REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS: u64 = 60 * 60 * 20;
pub(crate) const REALM_COLLAPSE_MONITOR_EVENT_DURATION_TICKS: u64 =
    REALM_COLLAPSE_EVACUATION_WINDOW_TICKS;
pub(crate) const REALM_COLLAPSE_EVACUATION_WINDOW_TICKS: u64 = 10 * 60 * 20;
pub(crate) const REALM_COLLAPSE_EVACUATION_REMINDER_INTERVAL_TICKS: u64 = 60 * 20;
pub(crate) const REALM_COLLAPSE_BOUNDARY_VFX_EVENT_ID: &str = "bong:realm_collapse_boundary";
const REALM_COLLAPSE_BOUNDARY_VFX_COLOR: &str = "#2B2B31";
const REALM_COLLAPSE_BOUNDARY_VFX_COUNT: u16 = 64;
const REALM_COLLAPSE_BOUNDARY_VFX_DURATION_TICKS: u16 = 160;
const REALM_COLLAPSE_BOUNDARY_VFX_OMEN_STRENGTH: f32 = 0.35;
const REALM_COLLAPSE_BOUNDARY_VFX_LOCK_STRENGTH: f32 = 0.70;
const REALM_COLLAPSE_BOUNDARY_VFX_SETTLE_STRENGTH: f32 = 1.0;

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
    last_evacuation_reminder_bucket: Option<u64>,
    evacuee_entities: HashSet<Entity>,
}

#[derive(Debug, Clone, PartialEq)]
struct TargetedDaoxiangSpawn {
    zone_name: String,
    target_player: Option<String>,
    position: DVec3,
    qi_density_heat: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ZoneOccupantPosition {
    dimension: DimensionKind,
    position: DVec3,
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
    pending_vfx_events: Vec<VfxEventRequest>,
    pending_lightning_strikes: Vec<DVec3>,
    pending_daoxiang_spawns: Vec<TargetedDaoxiangSpawn>,
    recent_game_events: Vec<GameEvent>,
}

#[derive(Default)]
pub struct RealmCollapseLowQiMonitor {
    low_qi_ticks_by_zone: HashMap<String, u64>,
}

#[derive(Debug, Clone, Event)]
pub struct ZoneCollapsedEvent {
    pub zone_name: String,
}

impl Resource for ActiveEventsResource {}
impl Resource for RealmCollapseLowQiMonitor {}

impl RealmCollapseLowQiMonitor {
    fn tick(
        &mut self,
        zone_registry: &mut ZoneRegistry,
        active_events: &mut ActiveEventsResource,
        occupant_positions: &[ZoneOccupantPosition],
    ) {
        let mut zones_to_collapse = Vec::new();

        for zone in &zone_registry.zones {
            if zone
                .active_events
                .iter()
                .any(|name| name == EVENT_REALM_COLLAPSE)
                || active_events.contains(zone.name.as_str(), EVENT_REALM_COLLAPSE)
            {
                self.low_qi_ticks_by_zone.remove(&zone.name);
                continue;
            }

            if zone.spirit_qi >= REALM_COLLAPSE_LOW_QI_THRESHOLD {
                self.low_qi_ticks_by_zone.remove(&zone.name);
                continue;
            }

            let low_qi_ticks = self
                .low_qi_ticks_by_zone
                .entry(zone.name.clone())
                .or_default();
            *low_qi_ticks = low_qi_ticks.saturating_add(1);

            if *low_qi_ticks >= REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS
                && zone_has_occupant(zone, occupant_positions)
            {
                zones_to_collapse.push(zone.name.clone());
            }
        }

        self.low_qi_ticks_by_zone
            .retain(|zone_name, _| zone_registry.find_zone_by_name(zone_name).is_some());

        for zone_name in zones_to_collapse {
            let command = realm_collapse_monitor_command(zone_name.as_str());
            if active_events.enqueue_from_spawn_command_with_karma(
                &command,
                Some(&mut *zone_registry),
                None,
                None,
            ) {
                self.low_qi_ticks_by_zone.remove(&zone_name);
            }
        }
    }

    #[cfg(test)]
    pub fn low_qi_ticks_for_zone(&self, zone_name: &str) -> Option<u64> {
        self.low_qi_ticks_by_zone.get(zone_name).copied()
    }
}

impl ActiveEventsResource {
    #[cfg(test)]
    pub fn enqueue_from_spawn_command(
        &mut self,
        command: &Command,
        zone_registry: Option<&mut ZoneRegistry>,
    ) -> bool {
        self.enqueue_from_spawn_command_with_karma(command, zone_registry, None, None)
    }

    pub fn enqueue_from_spawn_command_with_karma(
        &mut self,
        command: &Command,
        zone_registry: Option<&mut ZoneRegistry>,
        karma_weights: Option<&KarmaWeightStore>,
        qi_heatmap: Option<&QiDensityHeatmap>,
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
            let karma_weight = karma_weights
                .map(|weights| weights.weight_for_zone(event.zone_name.as_str()))
                .unwrap_or_default();
            let qi_density_heat = targeted_qi_density_heat(qi_heatmap, zone);
            let roll = targeted_calamity_roll(
                TARGETED_CALAMITY_BASE_PROBABILITY,
                karma_weight,
                qi_density_heat,
            );
            let roll_seed = targeted_calamity_event_seed(
                event.zone_name.as_str(),
                event.target_player.as_deref(),
                event.duration_ticks,
                event.intensity,
                self.recent_game_events.len() as u64,
            );
            let (roll_value, negative_event_triggered) =
                targeted_calamity_event_hit(roll.effective_probability, roll_seed);

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
                    ("command_intensity".to_string(), json!(event.intensity)),
                    ("karma_weight".to_string(), json!(roll.karma_weight)),
                    ("base_probability".to_string(), json!(roll.base_probability)),
                    (
                        "effective_probability".to_string(),
                        json!(roll.effective_probability),
                    ),
                    ("zone_karma_weight".to_string(), json!(roll.karma_weight)),
                    ("qi_density_heat".to_string(), json!(roll.qi_density_heat)),
                    ("roll_value".to_string(), json!(roll_value)),
                    (
                        "negative_event_triggered".to_string(),
                        json!(negative_event_triggered),
                    ),
                ])),
            });
            if negative_event_triggered {
                let strike_position = targeted_calamity_strike_position(
                    zone,
                    event.target_player.as_deref(),
                    karma_weights,
                );
                let qi_nullified = maybe_nullify_targeted_zone_qi(zone, roll.qi_density_heat);
                let daoxiang_spawn = maybe_targeted_daoxiang_spawn(
                    zone,
                    event.target_player.clone(),
                    strike_position,
                    roll.qi_density_heat,
                    qi_nullified.is_some(),
                );
                self.record_recent_event(GameEvent {
                    event_type: GameEventType::EventTriggered,
                    tick: 0,
                    player: event.target_player.clone(),
                    target: Some("targeted_negative_event".to_string()),
                    zone: Some(event.zone_name.clone()),
                    details: Some(HashMap::from([
                        ("event".to_string(), Value::String("运道折耗".to_string())),
                        (
                            "effective_probability".to_string(),
                            json!(roll.effective_probability),
                        ),
                        ("roll_value".to_string(), json!(roll_value)),
                        (
                            "localized_lightning".to_string(),
                            json!([strike_position.x, strike_position.y, strike_position.z]),
                        ),
                        ("qi_nullified".to_string(), json!(qi_nullified.is_some())),
                        (
                            "daoxiang_spawn_queued".to_string(),
                            json!(daoxiang_spawn.is_some()),
                        ),
                    ])),
                });
                if let Some(previous_spirit_qi) = qi_nullified {
                    self.record_recent_event(GameEvent {
                        event_type: GameEventType::EventTriggered,
                        tick: 0,
                        player: event.target_player.clone(),
                        target: Some("targeted_qi_nullified".to_string()),
                        zone: Some(event.zone_name.clone()),
                        details: Some(HashMap::from([
                            ("event".to_string(), Value::String("灵气归零".to_string())),
                            ("previous_spirit_qi".to_string(), json!(previous_spirit_qi)),
                            ("spirit_qi".to_string(), json!(0.0)),
                            ("qi_density_heat".to_string(), json!(roll.qi_density_heat)),
                        ])),
                    });
                }
                if let Some(spawn) = daoxiang_spawn {
                    self.pending_daoxiang_spawns.push(spawn);
                }
                self.record_recent_event(GameEvent {
                    event_type: GameEventType::EventTriggered,
                    tick: 0,
                    player: event.target_player.clone(),
                    target: Some("targeted_local_lightning".to_string()),
                    zone: Some(event.zone_name.clone()),
                    details: Some(HashMap::from([
                        ("event".to_string(), Value::String("局部落雷".to_string())),
                        (
                            "position".to_string(),
                            json!([strike_position.x, strike_position.y, strike_position.z]),
                        ),
                        (
                            "effective_probability".to_string(),
                            json!(roll.effective_probability),
                        ),
                    ])),
                });
                self.pending_lightning_strikes.push(strike_position);
                self.pending_vfx_events.push(targeted_lightning_vfx(
                    strike_position,
                    roll.effective_probability,
                ));
                self.pending_tribulation_events
                    .push(TribulationEventV1::targeted(
                        TribulationPhaseV1::Omen,
                        Some(event.zone_name.clone()),
                        Some([strike_position.x, strike_position.y, strike_position.z]),
                    ));
            }

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
            self.pending_vfx_events.push(realm_collapse_boundary_vfx(
                zone,
                REALM_COLLAPSE_BOUNDARY_VFX_OMEN_STRENGTH,
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

    pub fn drain_vfx_events(&mut self) -> Vec<VfxEventRequest> {
        std::mem::take(&mut self.pending_vfx_events)
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

    /// 推进事件。返回剩余未消耗的 zone 预算（若入参为 `None` 则返回
    /// `None`）。调用方负责把剩余额度通过 `NpcRegistry::release_zone_batch`
    /// 回滚，以避免预留但未消费的配额把 `spawn_paused` 误触发（P2-5）。
    #[must_use = "leftover budget should be released back to NpcRegistry"]
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        zone_registry: Option<&mut ZoneRegistry>,
        layer_entity: Option<Entity>,
        mut commands: Option<&mut Commands>,
        player_positions: Option<&[(String, DVec3)]>,
        collapse_targets: Option<&[(Entity, DimensionKind, DVec3)]>,
        mut death_events: Option<&mut EventWriter<DeathEvent>>,
        mut collapsed_events: Option<&mut EventWriter<ZoneCollapsedEvent>>,
        mut npc_spawn_budget_by_zone: Option<HashMap<String, usize>>,
    ) -> Option<HashMap<String, usize>> {
        let Some(zone_registry) = zone_registry else {
            self.tick_metadata_only(None);
            return npc_spawn_budget_by_zone;
        };
        let mut recent_events = Vec::new();

        if !self.pending_lightning_strikes.is_empty() {
            if let (Some(layer_entity), Some(commands)) = (layer_entity, commands.as_deref_mut()) {
                for strike_position in std::mem::take(&mut self.pending_lightning_strikes) {
                    spawn_lightning(commands, layer_entity, strike_position);
                }
            } else {
                tracing::warn!(
                    "[bong][world] targeted local lightning skipped this tick: missing entity layer or Commands"
                );
            }
        }

        if !self.pending_daoxiang_spawns.is_empty() {
            if let (Some(layer_entity), Some(commands)) = (layer_entity, commands.as_deref_mut()) {
                let mut deferred_spawns = Vec::new();
                for spawn in std::mem::take(&mut self.pending_daoxiang_spawns) {
                    if let Some(budget) = npc_spawn_budget_by_zone
                        .as_mut()
                        .and_then(|budgets| budgets.get_mut(spawn.zone_name.as_str()))
                    {
                        if *budget == 0 {
                            deferred_spawns.push(spawn);
                            continue;
                        }
                        *budget = budget.saturating_sub(1);
                    }

                    let entity = spawn_targeted_daoxiang(
                        commands,
                        layer_entity,
                        spawn.zone_name.as_str(),
                        spawn.position,
                    );
                    self.record_recent_event(GameEvent {
                        event_type: GameEventType::EventTriggered,
                        tick: 0,
                        player: spawn.target_player.clone(),
                        target: Some("targeted_daoxiang_spawned".to_string()),
                        zone: Some(spawn.zone_name.clone()),
                        details: Some(HashMap::from([
                            ("event".to_string(), Value::String("道伥刷新".to_string())),
                            (
                                "position".to_string(),
                                json!([spawn.position.x, spawn.position.y, spawn.position.z]),
                            ),
                            ("qi_density_heat".to_string(), json!(spawn.qi_density_heat)),
                            ("entity".to_string(), json!(format!("{entity:?}"))),
                        ])),
                    });
                }
                self.pending_daoxiang_spawns.extend(deferred_spawns);
            } else {
                tracing::warn!(
                    "[bong][world] targeted daoxiang spawn skipped this tick: missing entity layer or Commands"
                );
            }
        }

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
                        let beast_count = npc_spawn_budget_by_zone
                            .as_ref()
                            .and_then(|budget| budget.get(event.zone_name.as_str()).copied())
                            .map(|budget| desired_beast_count.min(budget))
                            .unwrap_or(desired_beast_count);
                        if beast_count == 0 {
                            tracing::info!(
                                "[bong][world] beast_tide runtime for zone `{}` skipped: npc registry budget exhausted",
                                event.zone_name
                            );
                            continue;
                        }
                        if let Some(budget_by_zone) = npc_spawn_budget_by_zone.as_mut() {
                            if let Some(budget) = budget_by_zone.get_mut(event.zone_name.as_str()) {
                                *budget = budget.saturating_sub(beast_count);
                            }
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
                    if !event.collapse.completed && next_elapsed < event.duration_ticks {
                        let remaining_ticks = event.duration_ticks.saturating_sub(next_elapsed);
                        if remaining_ticks <= REALM_COLLAPSE_EVACUATION_WINDOW_TICKS {
                            if !event.collapse.evacuation_warning_emitted {
                                event.collapse.evacuation_warning_emitted = true;
                                event.collapse.last_evacuation_reminder_bucket = Some(
                                    realm_collapse_evacuation_reminder_bucket(remaining_ticks),
                                );
                                event.collapse.evacuee_entities = realm_collapse_entities_in_zone(
                                    &zone,
                                    collapse_targets.unwrap_or(&[]),
                                );
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
                                self.pending_vfx_events.push(realm_collapse_boundary_vfx(
                                    &zone,
                                    REALM_COLLAPSE_BOUNDARY_VFX_LOCK_STRENGTH,
                                ));
                            } else {
                                maybe_emit_realm_collapse_evacuation_reminder(
                                    event,
                                    remaining_ticks,
                                    &mut self.pending_major_alerts,
                                );
                            }
                            maybe_kill_new_realm_collapse_intruders(
                                event,
                                &zone,
                                next_elapsed,
                                collapse_targets.unwrap_or(&[]),
                                death_events.as_deref_mut(),
                                &mut recent_events,
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
                        self.pending_vfx_events.push(realm_collapse_boundary_vfx(
                            &zone,
                            REALM_COLLAPSE_BOUNDARY_VFX_SETTLE_STRENGTH,
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

        npc_spawn_budget_by_zone
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
    app.insert_resource(RealmCollapseLowQiMonitor::default());
    app.add_event::<ZoneCollapsedEvent>();
    app.add_systems(
        Update,
        (
            tick_realm_collapse_low_qi_monitor.before(tick_active_events),
            tick_active_events,
            persist_zone_collapsed_overlays.after(tick_active_events),
        ),
    );
}

fn tick_realm_collapse_low_qi_monitor(
    mut monitor: ResMut<RealmCollapseLowQiMonitor>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    mut active_events: ResMut<ActiveEventsResource>,
    players: Query<(&Position, Option<&CurrentDimension>), With<Client>>,
    npcs: Query<(&Position, Option<&CurrentDimension>), With<NpcMarker>>,
) {
    let Some(zone_registry) = zone_registry.as_deref_mut() else {
        return;
    };

    let mut occupants = Vec::new();
    occupants.extend(
        players
            .iter()
            .map(|(position, dimension)| ZoneOccupantPosition {
                dimension: dimension.map(|dim| dim.0).unwrap_or_default(),
                position: position.get(),
            }),
    );
    occupants.extend(
        npcs.iter()
            .map(|(position, dimension)| ZoneOccupantPosition {
                dimension: dimension.map(|dim| dim.0).unwrap_or_default(),
                position: position.get(),
            }),
    );

    monitor.tick(zone_registry, &mut active_events, occupants.as_slice());
}

#[allow(clippy::too_many_arguments)]
fn tick_active_events(
    mut commands: Commands,
    mut active_events: ResMut<ActiveEventsResource>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    mut npc_registry: Option<ResMut<NpcRegistry>>,
    redis: Option<Res<crate::network::RedisBridgeResource>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut death_events: EventWriter<DeathEvent>,
    mut collapsed_events: EventWriter<ZoneCollapsedEvent>,
    layers: Query<Entity, With<crate::world::dimension::OverworldLayer>>,
    players: Query<(Entity, &Username, &Position, Option<&CurrentDimension>), With<Client>>,
    npcs: Query<(Entity, &Position, Option<&CurrentDimension>), With<NpcMarker>>,
) {
    let layer_entity = layers.iter().next();
    let mut player_positions = Vec::new();
    let mut collapse_targets = Vec::new();
    for (entity, username, position, dimension) in &players {
        let pos = position.get();
        player_positions.push((canonical_player_id(username.0.as_str()), pos));
        collapse_targets.push((entity, dimension.map(|dim| dim.0).unwrap_or_default(), pos));
    }
    for (entity, position, dimension) in &npcs {
        collapse_targets.push((
            entity,
            dimension.map(|dim| dim.0).unwrap_or_default(),
            position.get(),
        ));
    }

    let npc_spawn_budget = if let Some(registry) = npc_registry.as_deref_mut() {
        let mut reserved_by_zone = HashMap::new();
        for event in active_events
            .active_events
            .iter()
            .filter(|event| event.event_name == EVENT_BEAST_TIDE && event.elapsed_ticks == 0)
        {
            let desired = beast_count_for_intensity(event.intensity);
            let reserved = registry.reserve_zone_batch(event.zone_name.as_str(), desired);
            if reserved < desired {
                tracing::info!(
                    "[bong][world] beast_tide spawn clamped by npc registry: zone={} desired={} reserved={} live_npc_count={} max_npc_count={}",
                    event.zone_name,
                    desired,
                    reserved,
                    registry.live_npc_count,
                    registry.max_npc_count
                );
            }
            reserved_by_zone.insert(event.zone_name.clone(), reserved);
        }
        for spawn in &active_events.pending_daoxiang_spawns {
            let reserved = registry.reserve_zone_batch(spawn.zone_name.as_str(), 1);
            *reserved_by_zone.entry(spawn.zone_name.clone()).or_insert(0) += reserved;
        }
        Some(reserved_by_zone)
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

    let pending_vfx_events = active_events.drain_vfx_events();
    if let Some(vfx_events) = vfx_events.as_deref_mut() {
        for event in pending_vfx_events {
            vfx_events.send(event);
        }
    }

    // P2-5: 把 reserve 了但没消费掉（eg. beast_tide 因 missing layer/commands
    // 提前 continue）的额度归还给 registry，防止 1-tick 暂态 `spawn_paused`。
    if let (Some(registry), Some(remaining_by_zone)) = (npc_registry.as_deref_mut(), leftover) {
        for (zone, remaining) in remaining_by_zone {
            if remaining > 0 {
                registry.release_zone_batch(zone.as_str(), remaining);
            }
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

fn realm_collapse_monitor_command(zone_name: &str) -> Command {
    Command {
        command_type: CommandType::SpawnEvent,
        target: zone_name.to_string(),
        params: HashMap::from([
            ("event".to_string(), json!(EVENT_REALM_COLLAPSE)),
            (
                "duration_ticks".to_string(),
                json!(REALM_COLLAPSE_MONITOR_EVENT_DURATION_TICKS),
            ),
            ("intensity".to_string(), json!(1.0)),
        ]),
    }
}

fn zone_has_occupant(zone: &Zone, occupant_positions: &[ZoneOccupantPosition]) -> bool {
    occupant_positions
        .iter()
        .any(|occupant| occupant.dimension == zone.dimension && zone.contains(occupant.position))
}

fn targeted_calamity_event_seed(
    zone_name: &str,
    target_player: Option<&str>,
    duration_ticks: u64,
    intensity: f64,
    nonce: u64,
) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    zone_name.hash(&mut hasher);
    target_player.hash(&mut hasher);
    duration_ticks.hash(&mut hasher);
    intensity.to_bits().hash(&mut hasher);
    nonce.hash(&mut hasher);
    hasher.finish()
}

fn targeted_qi_density_heat(qi_heatmap: Option<&QiDensityHeatmap>, zone: &Zone) -> f32 {
    let Some(heatmap) = qi_heatmap else {
        return 0.0;
    };
    let (min, max) = zone.bounds;
    let center = zone.center();
    let center_block = valence::prelude::BlockPos::new(
        center.x.floor() as i32,
        center.y.floor() as i32,
        center.z.floor() as i32,
    );
    let center_heat = heatmap.heat_at(zone.dimension, center_block);
    let zone_heat = heatmap.max_heat_in_rect(
        zone.dimension,
        min.x.floor() as i32,
        max.x.ceil() as i32,
        min.z.floor() as i32,
        max.z.ceil() as i32,
    );
    center_heat.max(zone_heat)
}

fn maybe_nullify_targeted_zone_qi(zone: &mut Zone, qi_density_heat: f32) -> Option<f64> {
    if qi_density_heat < TARGETED_QI_NULLIFICATION_HEAT_THRESHOLD || zone.spirit_qi <= 0.0 {
        return None;
    }

    let previous_spirit_qi = zone.spirit_qi;
    zone.spirit_qi = 0.0;
    Some(previous_spirit_qi)
}

fn maybe_targeted_daoxiang_spawn(
    zone: &Zone,
    target_player: Option<String>,
    position: DVec3,
    qi_density_heat: f32,
    qi_was_nullified: bool,
) -> Option<TargetedDaoxiangSpawn> {
    if qi_was_nullified || qi_density_heat < TARGETED_QI_NULLIFICATION_HEAT_THRESHOLD {
        return None;
    }

    Some(TargetedDaoxiangSpawn {
        zone_name: zone.name.clone(),
        target_player,
        position: zone.clamp_position(position),
        qi_density_heat,
    })
}

fn targeted_calamity_strike_position(
    zone: &Zone,
    target_player: Option<&str>,
    karma_weights: Option<&KarmaWeightStore>,
) -> DVec3 {
    if let Some(weights) = karma_weights {
        if let Some(target_player) = target_player {
            let target_player = target_player.trim();
            let stripped = target_player
                .trim_start_matches("offline:")
                .trim_start_matches("OFFLINE:");
            if let Some(entry) = weights
                .entry_for_player(target_player)
                .or_else(|| weights.entry_for_player(stripped))
            {
                return zone.clamp_position(DVec3::new(
                    entry.last_position[0] as f64,
                    entry.last_position[1] as f64,
                    entry.last_position[2] as f64,
                ));
            }
        }

        if let Some(entry) = weights.strongest_entry_for_zone(zone.name.as_str()) {
            return zone.clamp_position(DVec3::new(
                entry.last_position[0] as f64,
                entry.last_position[1] as f64,
                entry.last_position[2] as f64,
            ));
        }
    }

    zone.center()
}

fn targeted_lightning_vfx(position: DVec3, effective_probability: f32) -> VfxEventRequest {
    let strength = (effective_probability / TARGETED_CALAMITY_MAX_PROBABILITY).clamp(0.0, 1.0);
    VfxEventRequest::new(
        position,
        VfxEventPayloadV1::SpawnParticle {
            event_id: TARGETED_LIGHTNING_VFX_EVENT_ID.to_string(),
            origin: [position.x, position.y, position.z],
            direction: None,
            color: Some(TARGETED_LIGHTNING_VFX_COLOR.to_string()),
            strength: Some(strength),
            count: Some(TARGETED_LIGHTNING_VFX_COUNT),
            duration_ticks: Some(TARGETED_LIGHTNING_VFX_DURATION_TICKS),
        },
    )
}

fn realm_collapse_evacuation_alert_message(zone_name: &str, remaining_ticks: u64) -> String {
    format!("域崩撤离窗口已在区域 {zone_name} 开启，剩余 {remaining_ticks} tick；未撤者横死。")
}

fn realm_collapse_evacuation_reminder_bucket(remaining_ticks: u64) -> u64 {
    remaining_ticks.div_ceil(REALM_COLLAPSE_EVACUATION_REMINDER_INTERVAL_TICKS)
}

fn maybe_emit_realm_collapse_evacuation_reminder(
    event: &mut ActiveEvent,
    remaining_ticks: u64,
    pending_alerts: &mut Vec<MajorEventAlert>,
) {
    let bucket = realm_collapse_evacuation_reminder_bucket(remaining_ticks);
    if event.collapse.last_evacuation_reminder_bucket == Some(bucket) {
        return;
    }

    event.collapse.last_evacuation_reminder_bucket = Some(bucket);
    pending_alerts.push(MajorEventAlert {
        event_name: event.event_name.clone(),
        zone_name: event.zone_name.clone(),
        duration_ticks: remaining_ticks,
        message: Some(realm_collapse_evacuation_reminder_message(
            event.zone_name.as_str(),
            remaining_ticks,
        )),
    });
}

fn realm_collapse_evacuation_reminder_message(zone_name: &str, remaining_ticks: u64) -> String {
    let remaining_minutes = remaining_ticks.div_ceil(60 * 20);
    format!("区域 {zone_name} 域崩撤离倒计时：约 {remaining_minutes} 分钟；请立即离开边界。")
}

fn realm_collapse_entities_in_zone(
    zone: &Zone,
    collapse_targets: &[(Entity, DimensionKind, DVec3)],
) -> HashSet<Entity> {
    collapse_targets
        .iter()
        .filter_map(|(entity, dimension, position)| {
            (*dimension == zone.dimension && zone.contains(*position)).then_some(*entity)
        })
        .collect()
}

fn maybe_kill_new_realm_collapse_intruders(
    event: &mut ActiveEvent,
    zone: &Zone,
    tick: u64,
    collapse_targets: &[(Entity, DimensionKind, DVec3)],
    death_events: Option<&mut EventWriter<DeathEvent>>,
    recent_events: &mut Vec<GameEvent>,
) {
    let intruders = collapse_targets
        .iter()
        .copied()
        .filter(|(entity, dimension, position)| {
            *dimension == zone.dimension
                && zone.contains(*position)
                && !event.collapse.evacuee_entities.contains(entity)
        })
        .collect::<Vec<_>>();

    if intruders.is_empty() {
        return;
    }

    let Some(death_events) = death_events else {
        tracing::warn!(
            "[bong][world] realm_collapse lock for zone `{}` could not reject {} new entrant(s): missing DeathEvent writer",
            event.zone_name,
            intruders.len()
        );
        return;
    };

    for (entity, _, position) in intruders {
        death_events.send(DeathEvent {
            target: entity,
            cause: "realm_collapse_entry_lock".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: tick,
        });
        event.collapse.evacuee_entities.insert(entity);
        recent_events.push(GameEvent {
            event_type: GameEventType::EventTriggered,
            tick,
            player: None,
            target: Some("realm_collapse_entry_lock".to_string()),
            zone: Some(event.zone_name.clone()),
            details: Some(HashMap::from([
                ("entity".to_string(), json!(format!("{entity:?}"))),
                (
                    "position".to_string(),
                    json!([position.x, position.y, position.z]),
                ),
            ])),
        });
    }
}

fn realm_collapse_boundary_vfx(zone: &Zone, strength: f32) -> VfxEventRequest {
    let (min, max) = zone.bounds;
    let center = zone.center();
    let origin = [center.x, min.y + 1.0, center.z];
    let half_extent = [
        ((max.x - min.x) * 0.5).max(1.0),
        0.0,
        ((max.z - min.z) * 0.5).max(1.0),
    ];

    VfxEventRequest::new(
        DVec3::new(origin[0], origin[1], origin[2]),
        VfxEventPayloadV1::SpawnParticle {
            event_id: REALM_COLLAPSE_BOUNDARY_VFX_EVENT_ID.to_string(),
            origin,
            direction: Some(half_extent),
            color: Some(REALM_COLLAPSE_BOUNDARY_VFX_COLOR.to_string()),
            strength: Some(strength),
            count: Some(REALM_COLLAPSE_BOUNDARY_VFX_COUNT),
            duration_ticks: Some(REALM_COLLAPSE_BOUNDARY_VFX_DURATION_TICKS),
        },
    )
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
                    "zone_status": "collapsed",
                    "danger_level": COLLAPSED_ZONE_DANGER_LEVEL,
                    "active_events": [EVENT_REALM_COLLAPSE],
                    "blocked_tiles": [],
                })
                .to_string(),
                payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
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
    let fauna_seed = fauna_spawn_seed(zone_name, spawn_position.x, spawn_position.z);
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
            fauna_tag_for_beast_spawn(zone_name, fauna_seed),
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

fn spawn_targeted_daoxiang(
    commands: &mut Commands,
    layer_entity: Entity,
    zone_name: &str,
    spawn_position: DVec3,
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
            NpcArchetype::Daoxiang,
            NpcPatrol::new(zone_name, spawn_position),
            Thinker::build()
                .picker(FirstToScore {
                    threshold: PROXIMITY_THRESHOLD,
                })
                .when(PlayerProximityScorer, FleeAction),
        ))
        .id();

    commands
        .entity(entity)
        .insert(npc_runtime_bundle(entity, NpcArchetype::Daoxiang));

    entity
}

fn collapse_zone(
    zone_registry: &mut ZoneRegistry,
    zone: &Zone,
    collapse_tick: u64,
    collapse_targets: &[(Entity, DimensionKind, DVec3)],
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
    for (entity, _, position) in collapse_targets
        .iter()
        .copied()
        .filter(|(_, dimension, position)| *dimension == zone.dimension && zone.contains(*position))
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
        RealmCollapseLowQiMonitor, ZoneCollapsedEvent, ZoneOccupantPosition,
        COLLAPSED_ZONE_DANGER_LEVEL, EVENT_BEAST_TIDE, EVENT_KARMA_BACKLASH, EVENT_REALM_COLLAPSE,
        EVENT_THUNDER_TRIBULATION, REALM_COLLAPSE_BOUNDARY_VFX_EVENT_ID,
        REALM_COLLAPSE_EVACUATION_REMINDER_INTERVAL_TICKS, REALM_COLLAPSE_EVACUATION_WINDOW_TICKS,
        REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS, REALM_COLLAPSE_LOW_QI_THRESHOLD,
        TARGETED_LIGHTNING_VFX_EVENT_ID,
    };
    use crate::combat::events::DeathEvent;
    use crate::npc::lifecycle::{NpcArchetype, NpcRegistry};
    use crate::npc::patrol::NpcPatrol;
    use crate::npc::spawn::NpcMarker;
    use crate::persistence::{
        bootstrap_sqlite, load_zone_overlays, PersistenceSettings, ZONE_OVERLAY_PAYLOAD_VERSION,
    };
    use crate::schema::agent_command::Command;
    use crate::schema::common::CommandType;
    use crate::schema::tribulation::{TribulationKindV1, TribulationPhaseV1};
    use crate::schema::vfx_event::VfxEventPayloadV1;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::karma::{KarmaWeightStore, QiDensityHeatmap};
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

    fn overworld_occupant(position: [f64; 3]) -> ZoneOccupantPosition {
        ZoneOccupantPosition {
            dimension: DimensionKind::Overworld,
            position: DVec3::new(position[0], position[1], position[2]),
        }
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
                assert!(
                    world
                        .get::<crate::fauna::components::FaunaTag>(*entity)
                        .is_some(),
                    "beast_tide runtime should tag fauna drops"
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
        let other_dimension_npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Tsy),
            ))
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
            !collected
                .iter()
                .any(|event| event.target == other_dimension_npc),
            "realm_collapse must not kill entities at the same coordinates in another dimension"
        );
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
        let initial_vfx = events.drain_vfx_events();
        assert_eq!(initial_vfx.len(), 1);
        match &initial_vfx[0].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                origin,
                direction,
                color,
                strength,
                count,
                duration_ticks,
            } => {
                assert_eq!(event_id, REALM_COLLAPSE_BOUNDARY_VFX_EVENT_ID);
                assert_eq!(*origin, [128.0, 65.0, 128.0]);
                assert_eq!(*direction, Some([128.0, 0.0, 128.0]));
                assert_eq!(color.as_deref(), Some("#2B2B31"));
                assert_eq!(*strength, Some(0.35));
                assert_eq!(*count, Some(64));
                assert_eq!(*duration_ticks, Some(160));
            }
            other => panic!("unexpected realm collapse vfx payload: {other:?}"),
        }

        let _ = events.tick(Some(&mut zones), None, None, None, None, None, None, None);
        assert!(events.drain_major_event_alerts().is_empty());
        assert!(events.drain_tribulation_events().is_empty());
        assert!(events.drain_vfx_events().is_empty());

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
        let lock_vfx = events.drain_vfx_events();
        assert_eq!(lock_vfx.len(), 1);
        match &lock_vfx[0].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id, strength, ..
            } => {
                assert_eq!(event_id, REALM_COLLAPSE_BOUNDARY_VFX_EVENT_ID);
                assert_eq!(*strength, Some(0.70));
            }
            other => panic!("unexpected realm collapse lock vfx payload: {other:?}"),
        }

        let _ = events.tick(Some(&mut zones), None, None, None, None, None, None, None);
        assert!(events.drain_major_event_alerts().is_empty());
        assert!(events.drain_tribulation_events().is_empty());
        assert!(events.drain_vfx_events().is_empty());
    }

    #[test]
    fn realm_collapse_emits_minute_evacuation_reminders() {
        let mut zones = ZoneRegistry::fallback();
        let mut events = ActiveEventsResource::default();
        let duration_ticks = REALM_COLLAPSE_EVACUATION_WINDOW_TICKS + 1;
        let command = spawn_event_command(
            DEFAULT_SPAWN_ZONE_NAME,
            EVENT_REALM_COLLAPSE,
            duration_ticks,
        );

        assert!(events.enqueue_from_spawn_command(&command, Some(&mut zones)));
        let _ = events.drain_major_event_alerts();
        let _ = events.drain_tribulation_events();
        let _ = events.drain_vfx_events();

        let _ = events.tick(Some(&mut zones), None, None, None, None, None, None, None);
        let evacuation_alerts = events.drain_major_event_alerts();
        assert_eq!(evacuation_alerts.len(), 1);
        assert!(evacuation_alerts[0]
            .message
            .as_deref()
            .is_some_and(|message| message.contains("撤离窗口") && message.contains("横死")));
        assert_eq!(
            evacuation_alerts[0].duration_ticks,
            REALM_COLLAPSE_EVACUATION_WINDOW_TICKS
        );
        let _ = events.drain_tribulation_events();
        let _ = events.drain_vfx_events();

        for _ in 0..REALM_COLLAPSE_EVACUATION_REMINDER_INTERVAL_TICKS - 1 {
            let _ = events.tick(Some(&mut zones), None, None, None, None, None, None, None);
            assert!(
                events.drain_major_event_alerts().is_empty(),
                "same minute bucket should not spam evacuation reminders"
            );
        }

        let _ = events.tick(Some(&mut zones), None, None, None, None, None, None, None);
        let reminders = events.drain_major_event_alerts();
        assert_eq!(reminders.len(), 1);
        assert_eq!(
            reminders[0].duration_ticks,
            REALM_COLLAPSE_EVACUATION_WINDOW_TICKS
                - REALM_COLLAPSE_EVACUATION_REMINDER_INTERVAL_TICKS
        );
        assert!(reminders[0]
            .message
            .as_deref()
            .is_some_and(|message| message.contains("倒计时") && message.contains("约 9 分钟")));
        assert!(
            events.drain_tribulation_events().is_empty(),
            "periodic reminders should not re-emit tribulation lock events"
        );
        assert!(
            events.drain_vfx_events().is_empty(),
            "periodic reminders should not replay boundary VFX"
        );
    }

    #[test]
    fn realm_collapse_lock_rejects_new_entrants_after_evacuation_window_opens() {
        let (mut app, layer) = setup_events_app();
        let evacuee = spawn_mock_player(&mut app, layer, "Azure", [8.0, 66.0, 8.0]);
        let command = spawn_event_command(
            DEFAULT_SPAWN_ZONE_NAME,
            EVENT_REALM_COLLAPSE,
            REALM_COLLAPSE_EVACUATION_WINDOW_TICKS + 2,
        );

        {
            let world = app.world_mut();
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                world
                    .resource_mut::<ActiveEventsResource>()
                    .enqueue_from_spawn_command(&command, Some(&mut zones));
            });
        }

        app.update();
        app.update();

        let intruder = spawn_mock_player(&mut app, layer, "Intruder", [9.0, 66.0, 9.0]);
        let other_dimension_intruder =
            spawn_mock_player(&mut app, layer, "TsyIntruder", [9.0, 66.0, 9.0]);
        app.world_mut()
            .entity_mut(other_dimension_intruder)
            .insert(CurrentDimension(DimensionKind::Tsy));
        app.update();

        let world = app.world();
        let deaths = world.resource::<Events<DeathEvent>>();
        let collected: Vec<_> = deaths.get_reader().read(deaths).cloned().collect();
        assert!(collected.iter().any(|event| {
            event.target == intruder && event.cause == "realm_collapse_entry_lock"
        }));
        assert!(
            !collected
                .iter()
                .any(|event| event.target == other_dimension_intruder),
            "realm_collapse entry lock must not kill same-coordinate players in another dimension"
        );
        assert!(
            !collected.iter().any(|event| {
                event.target == evacuee && event.cause == "realm_collapse_entry_lock"
            }),
            "players already in the zone when evacuation opens are allowed to evacuate"
        );

        assert!(
            world
                .resource::<ActiveEventsResource>()
                .recent_events_snapshot()
                .iter()
                .any(|event| {
                    event.target.as_deref() == Some("realm_collapse_entry_lock")
                        && event.zone.as_deref() == Some(DEFAULT_SPAWN_ZONE_NAME)
                }),
            "entry lock kills should be visible in internal recent events"
        );
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
                && overlay.payload_version == ZONE_OVERLAY_PAYLOAD_VERSION
                && overlay
                    .payload_json
                    .contains("\"zone_status\":\"collapsed\"")
                && overlay.payload_json.contains(EVENT_REALM_COLLAPSE)
        }));
    }

    #[test]
    fn low_qi_monitor_waits_until_threshold_and_occupant_before_realm_collapse() {
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .spirit_qi = REALM_COLLAPSE_LOW_QI_THRESHOLD - 0.01;
        let mut events = ActiveEventsResource::default();
        let mut monitor = RealmCollapseLowQiMonitor::default();
        let occupants = [overworld_occupant([8.0, 66.0, 8.0])];

        for _ in 0..REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS - 1 {
            monitor.tick(&mut zones, &mut events, occupants.as_slice());
        }

        assert!(!events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE));
        assert_eq!(
            monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME),
            Some(REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS - 1)
        );

        monitor.tick(&mut zones, &mut events, occupants.as_slice());

        assert!(events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE));
        assert_eq!(
            events.elapsed_for_first(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE),
            Some(0)
        );
        assert_eq!(
            monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME),
            None,
            "successful schedule should clear low-qi accumulation"
        );
        assert_eq!(
            zones
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist")
                .active_events
                .iter()
                .filter(|event| event.as_str() == EVENT_REALM_COLLAPSE)
                .count(),
            1
        );

        let alerts = events.drain_major_event_alerts();
        assert_eq!(alerts.len(), 1);
        assert_eq!(
            alerts[0].duration_ticks,
            REALM_COLLAPSE_EVACUATION_WINDOW_TICKS,
            "low-qi monitor should schedule the 10-minute evacuation window after the one-hour threshold"
        );
        let tribulation_events = events.drain_tribulation_events();
        assert_eq!(tribulation_events.len(), 1);
        assert_eq!(tribulation_events[0].kind, TribulationKindV1::ZoneCollapse);
        assert_eq!(tribulation_events[0].phase, TribulationPhaseV1::Omen);
        let vfx = events.drain_vfx_events();
        assert_eq!(vfx.len(), 1);
        match &vfx[0].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id, strength, ..
            } => {
                assert_eq!(event_id, REALM_COLLAPSE_BOUNDARY_VFX_EVENT_ID);
                assert_eq!(*strength, Some(0.35));
            }
            other => panic!("unexpected realm collapse monitor vfx payload: {other:?}"),
        }
    }

    #[test]
    fn low_qi_monitor_requires_occupant_at_threshold() {
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .spirit_qi = REALM_COLLAPSE_LOW_QI_THRESHOLD - 0.01;
        let mut events = ActiveEventsResource::default();
        let mut monitor = RealmCollapseLowQiMonitor::default();

        for _ in 0..REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS {
            monitor.tick(&mut zones, &mut events, &[]);
        }

        assert!(!events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE));
        assert_eq!(
            monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME),
            Some(REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS)
        );

        monitor.tick(
            &mut zones,
            &mut events,
            &[ZoneOccupantPosition {
                dimension: DimensionKind::Tsy,
                position: DVec3::new(8.0, 66.0, 8.0),
            }],
        );

        assert!(
            !events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE),
            "occupants in another dimension must not trigger overworld zone collapse"
        );

        monitor.tick(
            &mut zones,
            &mut events,
            &[overworld_occupant([300.0, 66.0, 300.0])],
        );

        assert!(
            !events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE),
            "occupants outside the zone must not trigger collapse"
        );

        monitor.tick(
            &mut zones,
            &mut events,
            &[overworld_occupant([8.0, 66.0, 8.0])],
        );

        assert!(events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE));
    }

    #[test]
    fn low_qi_monitor_resets_when_qi_recovers() {
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .spirit_qi = REALM_COLLAPSE_LOW_QI_THRESHOLD - 0.01;
        let mut events = ActiveEventsResource::default();
        let mut monitor = RealmCollapseLowQiMonitor::default();
        let occupants = [overworld_occupant([8.0, 66.0, 8.0])];

        monitor.tick(&mut zones, &mut events, occupants.as_slice());
        assert_eq!(
            monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME),
            Some(1)
        );

        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .spirit_qi = REALM_COLLAPSE_LOW_QI_THRESHOLD;
        monitor.tick(&mut zones, &mut events, occupants.as_slice());

        assert_eq!(monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME), None);
        assert!(!events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE));
    }

    #[test]
    fn low_qi_monitor_does_not_duplicate_active_realm_collapse() {
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .spirit_qi = REALM_COLLAPSE_LOW_QI_THRESHOLD - 0.01;
        let mut events = ActiveEventsResource::default();
        let mut monitor = RealmCollapseLowQiMonitor::default();
        let occupants = [overworld_occupant([8.0, 66.0, 8.0])];

        for _ in 0..REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS {
            monitor.tick(&mut zones, &mut events, occupants.as_slice());
        }
        monitor.tick(&mut zones, &mut events, occupants.as_slice());

        assert_eq!(
            events.count_by_zone_and_event(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE),
            1
        );
        assert_eq!(monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME), None);
        assert_eq!(
            zones
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist")
                .active_events
                .iter()
                .filter(|event| event.as_str() == EVENT_REALM_COLLAPSE)
                .count(),
            1
        );
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
        let reserved = registry.reserve_zone_batch(DEFAULT_SPAWN_ZONE_NAME, 5);
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
            Some(HashMap::from([(
                DEFAULT_SPAWN_ZONE_NAME.to_string(),
                reserved,
            )])),
        );
        assert_eq!(
            leftover
                .as_ref()
                .and_then(|budgets| budgets.get(DEFAULT_SPAWN_ZONE_NAME))
                .copied(),
            Some(reserved),
            "tick must return the full reserved budget when spawn could not occur"
        );

        for (zone, remaining) in leftover.unwrap_or_default() {
            registry.release_zone_batch(zone.as_str(), remaining);
        }
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

    #[test]
    fn hidden_karma_backlash_misses_negative_event_at_base_probability() {
        let (mut app, _layer) = setup_events_app();
        let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);

        {
            let world = app.world_mut();
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                assert!(events.enqueue_from_spawn_command_with_karma(
                    &command,
                    Some(&mut zones),
                    None,
                    None,
                ));

                let recent = events.recent_events_snapshot();
                let marker = recent
                    .iter()
                    .find(|event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH))
                    .expect("hidden marker should be recorded");
                let details = marker.details.as_ref().expect("marker details");
                assert_eq!(
                    details
                        .get("negative_event_triggered")
                        .and_then(Value::as_bool),
                    Some(false)
                );
                assert!(recent
                    .iter()
                    .all(|event| event.target.as_deref() != Some("targeted_negative_event")));
                assert!(events.drain_tribulation_events().is_empty());
            });
        }
    }

    #[test]
    fn hidden_karma_backlash_emits_targeted_hint_when_weighted_roll_hits() {
        let (mut app, _layer) = setup_events_app();
        let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);
        let mut karma = KarmaWeightStore::default();
        karma.mark_player(
            "Azure",
            Some("spawn".to_string()),
            valence::prelude::BlockPos::new(8, 66, 8),
            1.0,
            1,
        );

        {
            let world = app.world_mut();
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                assert!(events.enqueue_from_spawn_command_with_karma(
                    &command,
                    Some(&mut zones),
                    Some(&karma),
                    None,
                ));

                let recent = events.recent_events_snapshot();
                let marker = recent
                    .iter()
                    .find(|event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH))
                    .expect("hidden marker should be recorded");
                let details = marker.details.as_ref().expect("marker details");
                assert_eq!(
                    details
                        .get("negative_event_triggered")
                        .and_then(Value::as_bool),
                    Some(true)
                );
                assert!(recent.iter().any(|event| {
                    event.target.as_deref() == Some("targeted_negative_event")
                        && event.zone.as_deref() == Some("spawn")
                }));
                let lightning_marker = recent
                    .iter()
                    .find(|event| event.target.as_deref() == Some("targeted_local_lightning"))
                    .expect("weighted negative event should record local lightning");
                let lightning_position = lightning_marker
                    .details
                    .as_ref()
                    .and_then(|details| details.get("position"))
                    .and_then(Value::as_array)
                    .expect("local lightning marker should record position");
                assert_eq!(lightning_position[0].as_f64(), Some(8.0));
                assert_eq!(lightning_position[1].as_f64(), Some(66.0));
                assert_eq!(lightning_position[2].as_f64(), Some(8.0));

                let vfx = events.drain_vfx_events();
                assert_eq!(vfx.len(), 1);
                match &vfx[0].payload {
                    VfxEventPayloadV1::SpawnParticle {
                        event_id,
                        origin,
                        color,
                        strength,
                        count,
                        duration_ticks,
                        ..
                    } => {
                        assert_eq!(event_id, TARGETED_LIGHTNING_VFX_EVENT_ID);
                        assert_eq!(*origin, [8.0, 66.0, 8.0]);
                        assert_eq!(color.as_deref(), Some("#D0C8FF"));
                        assert_eq!(*strength, Some(1.0));
                        assert_eq!(*count, Some(3));
                        assert_eq!(*duration_ticks, Some(14));
                    }
                    other => panic!("unexpected targeted lightning vfx payload: {other:?}"),
                }

                let tribulation_events = events.drain_tribulation_events();
                assert_eq!(tribulation_events.len(), 1);
                assert_eq!(tribulation_events[0].kind, TribulationKindV1::Targeted);
                assert_eq!(tribulation_events[0].phase, TribulationPhaseV1::Omen);
                assert_eq!(tribulation_events[0].zone.as_deref(), Some("spawn"));
                assert_eq!(tribulation_events[0].epicenter, Some([8.0, 66.0, 8.0]));
            });
        }

        app.update();
        assert_eq!(
            query_lightning_entities(app.world_mut()).len(),
            1,
            "targeted local lightning should spawn a concrete lightning entity on the next world tick"
        );
    }

    #[test]
    fn hidden_karma_backlash_nullifies_zone_qi_when_density_heat_is_high() {
        let (mut app, _layer) = setup_events_app();
        let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);
        let mut heatmap = QiDensityHeatmap::default();
        heatmap.add_heat(
            DimensionKind::Overworld,
            valence::prelude::BlockPos::new(8, 66, 8),
            1.0,
        );

        {
            let world = app.world_mut();
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                assert!(events.enqueue_from_spawn_command_with_karma(
                    &command,
                    Some(&mut zones),
                    None,
                    Some(&heatmap),
                ));

                let zone = zones
                    .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                    .expect("spawn zone should exist");
                assert_eq!(zone.spirit_qi, 0.0);

                let recent = events.recent_events_snapshot();
                let marker = recent
                    .iter()
                    .find(|event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH))
                    .expect("hidden marker should be recorded");
                let marker_details = marker.details.as_ref().expect("marker details");
                assert_eq!(
                    marker_details
                        .get("qi_density_heat")
                        .and_then(Value::as_f64),
                    Some(1.0)
                );
                assert_eq!(
                    marker_details
                        .get("negative_event_triggered")
                        .and_then(Value::as_bool),
                    Some(true)
                );

                let nullified = recent
                    .iter()
                    .find(|event| event.target.as_deref() == Some("targeted_qi_nullified"))
                    .expect("high heat hit should record qi nullification");
                let details = nullified.details.as_ref().expect("nullification details");
                assert_eq!(
                    details.get("previous_spirit_qi").and_then(Value::as_f64),
                    Some(0.9)
                );
                assert_eq!(details.get("spirit_qi").and_then(Value::as_f64), Some(0.0));
                assert_eq!(
                    details.get("qi_density_heat").and_then(Value::as_f64),
                    Some(1.0)
                );
            });
        }
    }

    #[test]
    fn hidden_karma_backlash_spawns_daoxiang_after_qi_was_already_nullified() {
        let (mut app, _layer) = setup_events_app();
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .spirit_qi = 0.0;

        let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);
        let mut heatmap = QiDensityHeatmap::default();
        heatmap.add_heat(
            DimensionKind::Overworld,
            valence::prelude::BlockPos::new(8, 66, 8),
            1.0,
        );

        {
            let world = app.world_mut();
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                assert!(events.enqueue_from_spawn_command_with_karma(
                    &command,
                    Some(&mut zones),
                    None,
                    Some(&heatmap),
                ));

                let recent = events.recent_events_snapshot();
                let negative = recent
                    .iter()
                    .find(|event| event.target.as_deref() == Some("targeted_negative_event"))
                    .expect("weighted hit should record negative event");
                assert_eq!(
                    negative
                        .details
                        .as_ref()
                        .and_then(|details| details.get("qi_nullified"))
                        .and_then(Value::as_bool),
                    Some(false)
                );
                assert_eq!(
                    negative
                        .details
                        .as_ref()
                        .and_then(|details| details.get("daoxiang_spawn_queued"))
                        .and_then(Value::as_bool),
                    Some(true)
                );
            });
        }

        app.update();

        let world = app.world_mut();
        let mut archetypes = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
        assert!(
            archetypes
                .iter(world)
                .any(|archetype| *archetype == NpcArchetype::Daoxiang),
            "high heat targeted backlash should spawn one daoxiang when qi was already nullified"
        );
        let events = world.resource::<ActiveEventsResource>();
        assert!(events.recent_events_snapshot().iter().any(|event| {
            event.target.as_deref() == Some("targeted_daoxiang_spawned")
                && event.zone.as_deref() == Some(DEFAULT_SPAWN_ZONE_NAME)
        }));
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
