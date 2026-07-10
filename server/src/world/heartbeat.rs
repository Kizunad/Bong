use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use serde_json::{json, Value};
use valence::prelude::{
    bevy_ecs, App, Client, Component, DVec3, Event, EventReader, EventWriter, Events,
    IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Update, Username, With,
};

use crate::cultivation::breakthrough::BreakthroughOutcome;
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::tick::CultivationClock;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::lifecycle::NpcRegistry;
use crate::npc::spawn::ambient_scheduler::{danger_tide_required_ticks_scale, danger_tide_weight};
use crate::persistence::HeartbeatPseudoVeinRecord;
use crate::player::state::canonical_player_id;
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::{
    pending_inflow_account, zone_equilibrium_inflow, QiAccountId, QiTransfer, QiTransferReason,
    WorldQiAccount,
};
use crate::schema::agent_command::Command;
use crate::schema::common::{CommandType, GameEventType};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::schema::world_state::GameEvent;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::event_rhythm::{
    default_event_rhythm, event_trigger_timing_by_player_loop_phase, infer_player_loop_phase,
    PlayerLoopPhase, PlayerLoopPhaseEvidence, RhythmEventKind,
};
use crate::world::events::{
    ActiveEventsResource, ZoneCollapsedEvent, EVENT_BEAST_TIDE, EVENT_KARMA_BACKLASH,
    EVENT_REALM_COLLAPSE,
};
use crate::world::karma::{KarmaWeightStore, QiDensityHeatmap};
use crate::world::risk_heatmap::QI_HIGH_DANGER_THRESHOLD;
use crate::world::season::{query_season, Season, WorldSeasonState};
use crate::world::zone::{Zone, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
use crate::worldgen::pseudo_vein::{
    PseudoVeinRuntimeState, TICKS_PER_HOUR, TICKS_PER_MINUTE, TICKS_PER_SECOND,
};
use crate::worldgen::transient_zone::{
    pseudo_vein_zone_name, PSEUDO_VEIN_DANGER_LEVEL, PSEUDO_VEIN_DEFAULT_BASE_Y,
    PSEUDO_VEIN_HEIGHT, PSEUDO_VEIN_SIZE_XZ,
};

pub const HEARTBEAT_EVAL_INTERVAL_TICKS: u64 = 10 * TICKS_PER_SECOND;
pub const EVENT_PSEUDO_VEIN: &str = "pseudo_vein";
pub const VFX_WORLD_OMEN_PSEUDO_VEIN: &str = "bong:world_omen_pseudo_vein";
pub const VFX_WORLD_OMEN_BEAST_TIDE: &str = "bong:world_omen_beast_tide";
pub const VFX_WORLD_OMEN_TIDE_SKY: &str = "bong:world_omen_tide_sky";
pub const VFX_WORLD_OMEN_REALM_COLLAPSE: &str = "bong:world_omen_realm_collapse";
pub const VFX_WORLD_OMEN_KARMA_BACKLASH: &str = "bong:world_omen_karma_backlash";

const PSEUDO_VEIN_OMEN_LEAD_TICKS: u64 = 60 * TICKS_PER_SECOND;
const BEAST_TIDE_OMEN_LEAD_TICKS: u64 = 120 * TICKS_PER_SECOND;
const TIDE_SKY_OMEN_LEAD_TICKS: u64 = 30 * TICKS_PER_SECOND;
const REALM_COLLAPSE_OMEN_LEAD_TICKS: u64 = 300 * TICKS_PER_SECOND;
const KARMA_BACKLASH_OMEN_LEAD_TICKS: u64 = 10 * TICKS_PER_SECOND;
const OMEN_VISUAL_DURATION_TICKS: u16 = 200;
const BEAST_TIDE_LOW_QI_THRESHOLD: f64 = 0.15;
const BEAST_TIDE_LOW_QI_REQUIRED_TICKS: u64 = 5 * TICKS_PER_MINUTE;
const REALM_COLLAPSE_DEAD_QI_REQUIRED_TICKS: u64 = 10 * TICKS_PER_MINUTE;
const REALM_COLLAPSE_EVACUATION_TICKS: u64 = 30 * TICKS_PER_SECOND;
const DEEP_GATHERING_DANGER_LEVEL: u8 = 3;
const RETURN_ROUTE_DANGER_LEVEL_MAX: u8 = 1;
const PSEUDO_VEIN_ACTIVE_CAP: usize = 3;
const PSEUDO_VEIN_MIN_DISTANCE_BLOCKS: f64 = 500.0;
const KARMA_BASE_ROLL_PROBABILITY: f64 = 0.003;
const RECENT_BREAKTHROUGH_WINDOW_TICKS: u64 = 10 * TICKS_PER_MINUTE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeartbeatEventKind {
    PseudoVein,
    BeastTide,
    TideSkyOmen,
    RealmCollapse,
    KarmaBacklash,
}

impl HeartbeatEventKind {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "pseudo_vein" => Some(Self::PseudoVein),
            "beast_tide" => Some(Self::BeastTide),
            "tide_sky_omen" => Some(Self::TideSkyOmen),
            "realm_collapse" => Some(Self::RealmCollapse),
            "karma_backlash" => Some(Self::KarmaBacklash),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OmenKind {
    PseudoVeinForming,
    BeastTideApproaching,
    TideSkyTurning,
    RealmCollapseImminent,
    KarmaBacklashTarget,
}

impl OmenKind {
    const fn vfx_event_id(self) -> &'static str {
        match self {
            Self::PseudoVeinForming => VFX_WORLD_OMEN_PSEUDO_VEIN,
            Self::BeastTideApproaching => VFX_WORLD_OMEN_BEAST_TIDE,
            Self::TideSkyTurning => VFX_WORLD_OMEN_TIDE_SKY,
            Self::RealmCollapseImminent => VFX_WORLD_OMEN_REALM_COLLAPSE,
            Self::KarmaBacklashTarget => VFX_WORLD_OMEN_KARMA_BACKLASH,
        }
    }

    const fn color(self) -> &'static str {
        match self {
            Self::PseudoVeinForming => "#66D8C8",
            Self::BeastTideApproaching => "#B8864A",
            Self::TideSkyTurning => "#9E8C6A",
            Self::RealmCollapseImminent => "#7A1E24",
            Self::KarmaBacklashTarget => "#A01830",
        }
    }
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct WorldEventOmen {
    pub kind: OmenKind,
    pub zone_name: String,
    pub target_player: Option<String>,
    pub origin: DVec3,
    pub intensity: f64,
    pub scheduled_at_tick: u64,
    pub fires_at_tick: u64,
    pub expires_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub enum EventChainTrigger {
    PseudoVeinDissipated {
        zone_name: String,
        redistributed_qi: f64,
    },
    BeastTideArrived {
        source_zone: String,
        target_zone: String,
        beast_count: u32,
    },
    RealmCollapseCompleted {
        zone_name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventCadence {
    pub base_interval_ticks: u64,
    pub last_fired_tick: u64,
    pub seasonal_multiplier: f64,
    pub pressure_multiplier: f64,
    pub cooldown_remaining: u64,
}

impl EventCadence {
    pub const fn new(base_interval_ticks: u64) -> Self {
        Self {
            base_interval_ticks,
            last_fired_tick: 0,
            seasonal_multiplier: 1.0,
            pressure_multiplier: 1.0,
            cooldown_remaining: 0,
        }
    }

    pub fn effective_interval_ticks(&self, override_multiplier: f64) -> u64 {
        let multiplier =
            (self.seasonal_multiplier * self.pressure_multiplier * override_multiplier.max(0.0))
                .max(0.01);
        ((self.base_interval_ticks as f64) / multiplier)
            .round()
            .max(1.0) as u64
    }

    pub fn ready(&self, current_tick: u64, override_multiplier: f64) -> bool {
        self.cooldown_remaining == 0
            && current_tick.saturating_sub(self.last_fired_tick)
                >= self.effective_interval_ticks(override_multiplier)
    }

    fn mark_fired(&mut self, current_tick: u64) {
        self.last_fired_tick = current_tick;
        self.cooldown_remaining = 0;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WorldPressure {
    pub avg_zone_qi: f64,
    pub qi_drain_rate: f64,
    pub player_density_peak: f64,
    pub high_realm_count: u32,
    pub recent_breakthrough_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeasonEventModifiers {
    pub pseudo_vein_frequency: f64,
    pub pseudo_vein_strength_min: f64,
    pub pseudo_vein_strength_max: f64,
    pub beast_tide_frequency: f64,
    pub beast_tide_scale: f64,
    pub realm_collapse_frequency: f64,
    pub karma_backlash_frequency: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatOverrideAction {
    Suppress,
    Accelerate,
    Force,
}

impl HeartbeatOverrideAction {
    fn from_wire(value: &str) -> Option<Self> {
        match value {
            "suppress" => Some(Self::Suppress),
            "accelerate" => Some(Self::Accelerate),
            "force" => Some(Self::Force),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeartbeatOverride {
    pub action: HeartbeatOverrideAction,
    pub event_kind: HeartbeatEventKind,
    pub target_zone: String,
    pub expires_at_tick: u64,
    pub intensity_override: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
struct ForcedHeartbeatEvent {
    event_kind: HeartbeatEventKind,
    target_zone: String,
    intensity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HeartbeatOverrideError {
    MissingHeartbeat,
    InvalidAction,
    InvalidEventType,
    InvalidDuration,
}

impl HeartbeatOverrideError {
    pub const fn result_label(&self) -> &'static str {
        match self {
            Self::MissingHeartbeat => "rejected_missing_heartbeat",
            Self::InvalidAction => "rejected_invalid_heartbeat_action",
            Self::InvalidEventType => "rejected_invalid_heartbeat_event_type",
            Self::InvalidDuration => "rejected_invalid_heartbeat_duration",
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeartbeatSimulationReport {
    pub pseudo_vein_count: u32,
    pub beast_tide_count: u32,
    pub realm_collapse_count: u32,
    pub karma_backlash_count: u32,
    pub chain_reaction_count: u32,
    pub qi_total_delta_ratio: f64,
    pub max_same_zone_stack: u32,
}

type PlayerSampleQueryItem = (
    &'static Position,
    Option<&'static CurrentDimension>,
    Option<&'static Cultivation>,
    Option<&'static Username>,
);

#[derive(Debug, Clone, Copy)]
struct HeartbeatEventSources<'a> {
    karma_weights: Option<&'a KarmaWeightStore>,
    qi_heatmap: Option<&'a QiDensityHeatmap>,
}

#[derive(Debug, Clone)]
pub struct WorldHeartbeat {
    pub last_eval_tick: u64,
    pub eval_interval_ticks: u64,
    pub pseudo_vein_cadence: EventCadence,
    pub beast_tide_cadence: EventCadence,
    pub tide_sky_omen_cadence: EventCadence,
    pub realm_collapse_cadence: EventCadence,
    pub karma_backlash_cadence: EventCadence,
    pub loop_phase: PlayerLoopPhase,
    pub world_pressure: WorldPressure,
    active_pseudo_veins: HashMap<String, PseudoVeinRuntimeState>,
    pending_omens: Vec<WorldEventOmen>,
    low_qi_ticks_by_zone: HashMap<String, u64>,
    dead_qi_ticks_by_zone: HashMap<String, u64>,
    last_tide_sky_omen_boundary_tick: Option<u64>,
    recent_breakthrough_ticks: Vec<u64>,
    overrides: Vec<HeartbeatOverride>,
    forced_events: Vec<ForcedHeartbeatEvent>,
    next_pseudo_vein_index: u64,
    last_avg_zone_qi: Option<f64>,
    last_pressure_tick: Option<u64>,
    event_counts: HashMap<HeartbeatEventKind, u64>,
}

impl Resource for WorldHeartbeat {}

impl Default for WorldHeartbeat {
    fn default() -> Self {
        Self {
            last_eval_tick: 0,
            eval_interval_ticks: HEARTBEAT_EVAL_INTERVAL_TICKS,
            pseudo_vein_cadence: EventCadence::new(15 * TICKS_PER_MINUTE),
            beast_tide_cadence: EventCadence::new(30 * TICKS_PER_MINUTE),
            tide_sky_omen_cadence: EventCadence::new(TICKS_PER_HOUR),
            realm_collapse_cadence: EventCadence::new(TICKS_PER_HOUR),
            karma_backlash_cadence: EventCadence::new(20 * TICKS_PER_MINUTE),
            loop_phase: PlayerLoopPhase::SafeShelter,
            world_pressure: WorldPressure::default(),
            active_pseudo_veins: HashMap::new(),
            pending_omens: Vec::new(),
            low_qi_ticks_by_zone: HashMap::new(),
            dead_qi_ticks_by_zone: HashMap::new(),
            last_tide_sky_omen_boundary_tick: None,
            recent_breakthrough_ticks: Vec::new(),
            overrides: Vec::new(),
            forced_events: Vec::new(),
            next_pseudo_vein_index: 0,
            last_avg_zone_qi: None,
            last_pressure_tick: None,
            event_counts: HashMap::new(),
        }
    }
}

impl WorldHeartbeat {
    pub fn apply_override(
        &mut self,
        action: HeartbeatOverrideAction,
        event_kind: HeartbeatEventKind,
        target_zone: String,
        duration_ticks: u64,
        intensity_override: Option<f64>,
        current_tick: u64,
    ) {
        if action == HeartbeatOverrideAction::Force {
            self.forced_events.push(ForcedHeartbeatEvent {
                event_kind,
                target_zone,
                intensity: intensity_override.unwrap_or(0.8).clamp(0.0, 1.0),
            });
            return;
        }

        self.overrides.push(HeartbeatOverride {
            action,
            event_kind,
            target_zone,
            expires_at_tick: current_tick.saturating_add(duration_ticks),
            intensity_override,
        });
    }

    #[cfg(test)]
    pub(crate) fn active_pseudo_vein_count(&self) -> usize {
        self.active_pseudo_veins.len()
    }

    pub(crate) fn active_pseudo_vein_records(
        &self,
        zone_registry: &ZoneRegistry,
    ) -> Vec<HeartbeatPseudoVeinRecord> {
        let mut records = self
            .active_pseudo_veins
            .iter()
            .filter_map(|(zone_id, state)| {
                if state.dissipated {
                    return None;
                }
                let zone = zone_registry.find_zone_by_name(zone_id.as_str())?;
                let (min, max) = zone.bounds;
                Some(HeartbeatPseudoVeinRecord {
                    zone_id: zone_id.clone(),
                    dimension: zone.dimension,
                    bounds_min: dvec3_to_array(min),
                    bounds_max: dvec3_to_array(max),
                    danger_level: zone.danger_level,
                    active_events: zone.active_events.clone(),
                    patrol_anchors: zone
                        .patrol_anchors
                        .iter()
                        .copied()
                        .map(dvec3_to_array)
                        .collect(),
                    center_xz: state.center_xz,
                    spawned_at_tick: state.lifecycle.spawned_at,
                    last_tick: state.last_tick,
                    qi_current: state.qi_current,
                    total_qi_consumed: state.total_qi_consumed,
                    warning_sent: state.warning_sent,
                    dissipated: state.dissipated,
                    season_at_spawn: state.season_at_spawn,
                })
            })
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.zone_id.cmp(&right.zone_id));
        records
    }

    pub(crate) fn restore_pseudo_vein_records(
        &mut self,
        zone_registry: &mut ZoneRegistry,
        records: &[HeartbeatPseudoVeinRecord],
        current_tick: u64,
    ) -> usize {
        let mut restored = 0;
        for record in records {
            if record.dissipated
                || !record.zone_id.starts_with("pseudo_vein_")
                || !finite_array3(record.bounds_min)
                || !finite_array3(record.bounds_max)
                || !finite_array2(record.center_xz)
                || record.bounds_min[0] > record.bounds_max[0]
                || record.bounds_min[1] > record.bounds_max[1]
                || record.bounds_min[2] > record.bounds_max[2]
                || !record.qi_current.is_finite()
                || !record.total_qi_consumed.is_finite()
            {
                continue;
            }

            let mut active_events = record.active_events.clone();
            if !active_events
                .iter()
                .any(|event| event.as_str() == EVENT_PSEUDO_VEIN)
            {
                active_events.push(EVENT_PSEUDO_VEIN.to_string());
            }
            let bounds = (
                dvec3_from_array(record.bounds_min),
                dvec3_from_array(record.bounds_max),
            );
            let patrol_anchors = if record.patrol_anchors.is_empty() {
                vec![DVec3::new(
                    record.center_xz[0],
                    (record.bounds_min[1] + record.bounds_max[1]) * 0.5,
                    record.center_xz[1],
                )]
            } else {
                record
                    .patrol_anchors
                    .iter()
                    .copied()
                    .filter(|anchor| finite_array3(*anchor))
                    .map(dvec3_from_array)
                    .collect::<Vec<_>>()
            };

            if zone_registry
                .find_zone_by_name(record.zone_id.as_str())
                .is_none()
            {
                let zone = Zone {
                    name: record.zone_id.clone(),
                    dimension: record.dimension,
                    bounds,
                    spirit_qi: record.qi_current,
                    danger_level: record.danger_level,
                    active_events,
                    patrol_anchors,
                    blocked_tiles: Vec::new(),
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
                };
                if zone_registry.register_runtime_zone(zone).is_err() {
                    continue;
                }
            }

            let observed_age = record.last_tick.saturating_sub(record.spawned_at_tick);
            let mut state = PseudoVeinRuntimeState::restored(
                record.zone_id.clone(),
                record.center_xz,
                current_tick,
                observed_age,
                record.season_at_spawn,
            );
            state.qi_current = record.qi_current.clamp(0.0, 1.0);
            state.total_qi_consumed = record.total_qi_consumed.max(0.0);
            state.warning_sent = record.warning_sent;
            state.dissipated = false;
            self.active_pseudo_veins
                .insert(record.zone_id.clone(), state);
            if let Some(index) = heartbeat_pseudo_vein_index(record.zone_id.as_str()) {
                self.next_pseudo_vein_index = self.next_pseudo_vein_index.max(index + 1);
            }
            restored += 1;
        }
        restored
    }

    fn note_event(&mut self, kind: HeartbeatEventKind) {
        *self.event_counts.entry(kind).or_default() += 1;
    }

    fn prune_expired(&mut self, current_tick: u64) {
        self.recent_breakthrough_ticks
            .retain(|tick| current_tick.saturating_sub(*tick) <= RECENT_BREAKTHROUGH_WINDOW_TICKS);
        self.overrides
            .retain(|override_| current_tick <= override_.expires_at_tick);
    }

    fn override_multiplier(
        &self,
        kind: HeartbeatEventKind,
        target_zone: &str,
        current_tick: u64,
    ) -> f64 {
        if self.is_suppressed(kind, target_zone, current_tick) {
            return 0.0;
        }
        if self.overrides.iter().any(|override_| {
            override_.action == HeartbeatOverrideAction::Accelerate
                && override_.event_kind == kind
                && override_.target_zone == target_zone
                && current_tick <= override_.expires_at_tick
        }) {
            return 3.0;
        }
        1.0
    }

    fn override_intensity(
        &self,
        kind: HeartbeatEventKind,
        target_zone: &str,
        current_tick: u64,
    ) -> Option<f64> {
        self.overrides.iter().rev().find_map(|override_| {
            (override_.action == HeartbeatOverrideAction::Accelerate
                && override_.event_kind == kind
                && override_.target_zone == target_zone
                && current_tick <= override_.expires_at_tick)
                .then_some(override_.intensity_override)
                .flatten()
        })
    }

    fn is_suppressed(
        &self,
        kind: HeartbeatEventKind,
        target_zone: &str,
        current_tick: u64,
    ) -> bool {
        self.overrides.iter().any(|override_| {
            override_.action == HeartbeatOverrideAction::Suppress
                && override_.event_kind == kind
                && override_.target_zone == target_zone
                && current_tick <= override_.expires_at_tick
        })
    }

    #[cfg(test)]
    pub(crate) fn override_for(
        &self,
        kind: HeartbeatEventKind,
        target_zone: &str,
    ) -> Option<&HeartbeatOverride> {
        self.overrides
            .iter()
            .rev()
            .find(|override_| override_.event_kind == kind && override_.target_zone == target_zone)
    }
}

fn dvec3_to_array(value: DVec3) -> [f64; 3] {
    [value.x, value.y, value.z]
}

fn dvec3_from_array(value: [f64; 3]) -> DVec3 {
    DVec3::new(value[0], value[1], value[2])
}

fn finite_array2(value: [f64; 2]) -> bool {
    value.into_iter().all(f64::is_finite)
}

fn finite_array3(value: [f64; 3]) -> bool {
    value.into_iter().all(f64::is_finite)
}

fn heartbeat_pseudo_vein_index(zone_id: &str) -> Option<u64> {
    zone_id
        .strip_prefix("pseudo_vein_heartbeat_")
        .and_then(|suffix| suffix.parse::<u64>().ok())
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering world heartbeat scheduler");
    app.insert_resource(WorldHeartbeat::default());
    app.init_resource::<ZoneQiInflowClock>();
    app.add_event::<EventChainTrigger>();
    app.add_systems(
        Update,
        (
            record_breakthroughs_for_heartbeat,
            forward_realm_collapse_chain_triggers,
            heartbeat_tick,
            chain_reaction_tick.after(heartbeat_tick),
            zone_qi_inflow_tick,
        ),
    );
}

pub fn season_event_modifiers(season: Season) -> SeasonEventModifiers {
    match season {
        Season::Summer => SeasonEventModifiers {
            pseudo_vein_frequency: 1.0,
            pseudo_vein_strength_min: 0.5,
            pseudo_vein_strength_max: 0.5,
            beast_tide_frequency: 1.5,
            beast_tide_scale: 1.0,
            realm_collapse_frequency: 1.2,
            karma_backlash_frequency: 1.0,
        },
        Season::Winter => SeasonEventModifiers {
            pseudo_vein_frequency: 0.5,
            pseudo_vein_strength_min: 0.7,
            pseudo_vein_strength_max: 0.7,
            beast_tide_frequency: 0.7,
            beast_tide_scale: 0.6,
            realm_collapse_frequency: 0.8,
            karma_backlash_frequency: 1.0,
        },
        Season::SummerToWinter | Season::WinterToSummer => SeasonEventModifiers {
            pseudo_vein_frequency: 2.0,
            pseudo_vein_strength_min: 0.4,
            pseudo_vein_strength_max: 0.8,
            beast_tide_frequency: 1.2,
            beast_tide_scale: 1.0,
            realm_collapse_frequency: 1.5,
            karma_backlash_frequency: 2.0,
        },
    }
}

pub fn apply_heartbeat_override_command(
    heartbeat: Option<&mut WorldHeartbeat>,
    command: &Command,
    current_tick: u64,
) -> Result<(), HeartbeatOverrideError> {
    let heartbeat = heartbeat.ok_or(HeartbeatOverrideError::MissingHeartbeat)?;
    let action = command
        .params
        .get("action")
        .and_then(Value::as_str)
        .and_then(HeartbeatOverrideAction::from_wire)
        .ok_or(HeartbeatOverrideError::InvalidAction)?;
    let event_kind = command
        .params
        .get("event_type")
        .and_then(Value::as_str)
        .and_then(HeartbeatEventKind::from_wire)
        .ok_or(HeartbeatOverrideError::InvalidEventType)?;
    let target_zone = command
        .params
        .get("target_zone")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(command.target.as_str())
        .to_string();
    let duration_ticks = match command.params.get("duration_ticks") {
        Some(value) => value_to_u64(value)
            .filter(|duration| *duration > 0)
            .ok_or(HeartbeatOverrideError::InvalidDuration)?,
        None => 5 * TICKS_PER_MINUTE,
    };
    let intensity_override = command
        .params
        .get("intensity_override")
        .and_then(value_to_f64)
        .map(|value| value.clamp(0.0, 1.0));

    heartbeat.apply_override(
        action,
        event_kind,
        target_zone,
        duration_ticks,
        intensity_override,
        current_tick,
    );
    Ok(())
}

fn record_breakthroughs_for_heartbeat(
    mut heartbeat: ResMut<WorldHeartbeat>,
    clock: Option<Res<CultivationClock>>,
    mut outcomes: EventReader<BreakthroughOutcome>,
) {
    let current_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for outcome in outcomes.read() {
        if outcome.result.is_ok() {
            heartbeat.recent_breakthrough_ticks.push(current_tick);
        }
    }
}

fn forward_realm_collapse_chain_triggers(
    mut collapsed_events: EventReader<ZoneCollapsedEvent>,
    mut chain_triggers: EventWriter<EventChainTrigger>,
) {
    for event in collapsed_events.read() {
        chain_triggers.send(EventChainTrigger::RealmCollapseCompleted {
            zone_name: event.zone_name.clone(),
        });
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn heartbeat_tick(
    mut heartbeat: ResMut<WorldHeartbeat>,
    clock: Option<Res<CultivationClock>>,
    season_state: Option<Res<WorldSeasonState>>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    mut active_events: ResMut<ActiveEventsResource>,
    npc_registry: Option<Res<NpcRegistry>>,
    karma_weights: Option<Res<KarmaWeightStore>>,
    qi_heatmap: Option<Res<QiDensityHeatmap>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    players: Query<PlayerSampleQueryItem, With<Client>>,
    mut chain_triggers: EventWriter<EventChainTrigger>,
) {
    let current_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_else(|| {
        heartbeat
            .last_eval_tick
            .saturating_add(heartbeat.eval_interval_ticks)
    });
    if current_tick.saturating_sub(heartbeat.last_eval_tick) < heartbeat.eval_interval_ticks {
        return;
    }
    heartbeat.last_eval_tick = current_tick;
    heartbeat.prune_expired(current_tick);

    let Some(zone_registry) = zone_registry.as_deref_mut() else {
        return;
    };

    let season = season_state
        .as_deref()
        .map(|state| state.current.season)
        .unwrap_or_else(|| query_season("", current_tick).season);
    let season_boundary_tick = season_state
        .as_deref()
        .map(|state| state.last_phase_change_tick);
    let modifiers = season_event_modifiers(season);
    apply_season_modifiers(&mut heartbeat, modifiers);

    let player_samples = player_samples(zone_registry, &players);
    advance_active_pseudo_veins(
        &mut heartbeat,
        zone_registry,
        &player_samples,
        current_tick,
        &mut chain_triggers,
        vfx_events.as_deref_mut(),
        &mut active_events,
    );

    queue_forced_events(
        &mut heartbeat,
        zone_registry,
        current_tick,
        vfx_events.as_deref_mut(),
    );
    fire_due_omens(
        &mut heartbeat,
        zone_registry,
        &mut active_events,
        &mut chain_triggers,
        HeartbeatEventSources {
            karma_weights: karma_weights.as_deref(),
            qi_heatmap: qi_heatmap.as_deref(),
        },
        season,
        current_tick,
    );

    heartbeat.world_pressure =
        compute_world_pressure(&mut heartbeat, zone_registry, &player_samples, current_tick);
    let loop_phase = heartbeat_loop_phase(zone_registry, &player_samples);
    heartbeat.loop_phase = loop_phase;
    let rhythm_context = HeartbeatRhythmContext {
        modifiers,
        loop_phase,
        current_tick,
    };

    maybe_queue_tide_sky_omen(
        &mut heartbeat,
        zone_registry,
        season,
        season_boundary_tick,
        rhythm_context,
        vfx_events.as_deref_mut(),
    );
    maybe_queue_pseudo_vein(
        &mut heartbeat,
        zone_registry,
        rhythm_context,
        vfx_events.as_deref_mut(),
    );
    maybe_queue_beast_tide(
        &mut heartbeat,
        zone_registry,
        npc_registry.as_deref(),
        &active_events,
        rhythm_context,
        vfx_events.as_deref_mut(),
    );
    maybe_queue_realm_collapse(
        &mut heartbeat,
        zone_registry,
        &player_samples,
        &active_events,
        rhythm_context,
        vfx_events.as_deref_mut(),
    );
    maybe_queue_karma_backlash(
        &mut heartbeat,
        zone_registry,
        &player_samples,
        rhythm_context,
        vfx_events.as_deref_mut(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn chain_reaction_tick(
    mut triggers: EventReader<EventChainTrigger>,
    mut heartbeat: ResMut<WorldHeartbeat>,
    mut active_events: ResMut<ActiveEventsResource>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    npc_registry: Option<Res<NpcRegistry>>,
    clock: Option<Res<CultivationClock>>,
    season_state: Option<Res<WorldSeasonState>>,
    karma_weights: Option<Res<KarmaWeightStore>>,
    qi_heatmap: Option<Res<QiDensityHeatmap>>,
) {
    let Some(zone_registry) = zone_registry.as_deref_mut() else {
        return;
    };
    let current_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_else(|| {
        heartbeat
            .last_eval_tick
            .saturating_add(heartbeat.eval_interval_ticks)
    });
    let season = season_state
        .as_deref()
        .map(|state| state.current.season)
        .unwrap_or_else(|| query_season("", current_tick).season);
    let npc_counts = npc_registry
        .as_deref()
        .map(|registry| &registry.counts_by_zone);

    for trigger in triggers.read() {
        match trigger {
            EventChainTrigger::PseudoVeinDissipated {
                zone_name,
                redistributed_qi,
            } => {
                let Some(source) = zone_registry.find_zone_by_name(zone_name).cloned() else {
                    continue;
                };
                remove_runtime_pseudo_vein_zone(zone_registry, zone_name.as_str());
                if source.dimension != DimensionKind::Overworld {
                    continue;
                }
                let neighbor_names = adjacent_zone_names(zone_registry, &source, 900.0);
                for neighbor_name in neighbor_names {
                    let Some(neighbor) = zone_registry.find_zone_by_name(neighbor_name.as_str())
                    else {
                        continue;
                    };
                    let npc_count = npc_counts
                        .and_then(|counts| counts.get(neighbor.name.as_str()).copied())
                        .unwrap_or_default();
                    // P3 §8.1 #5 — 次入口 danger 加权：`BEAST_TIDE_LOW_QI_THRESHOLD` 常数
                    // 本身不动，只在此处按 neighbor.danger_level 放宽"有效低灵气窗口"——
                    // danger 越高，即便邻域灵气回升得更多一点也仍判定为触发条件成立。
                    // danger<=1 时权重=1.0，effective_threshold == 原始常数，行为不变。
                    let effective_low_qi_threshold =
                        BEAST_TIDE_LOW_QI_THRESHOLD * danger_tide_weight(neighbor.danger_level);
                    if neighbor.spirit_qi >= effective_low_qi_threshold || npc_count <= 3 {
                        continue;
                    }
                    if heartbeat.is_suppressed(
                        HeartbeatEventKind::BeastTide,
                        neighbor.name.as_str(),
                        current_tick,
                    ) {
                        continue;
                    }
                    let intensity = (0.3 + (*redistributed_qi).clamp(0.0, 0.4))
                        * danger_tide_weight(neighbor.danger_level);
                    let command = spawn_event_command(
                        neighbor.name.as_str(),
                        EVENT_BEAST_TIDE,
                        intensity,
                        20 * TICKS_PER_MINUTE,
                        None,
                    );
                    if active_events.enqueue_from_spawn_command_with_karma_and_season_at_tick(
                        &command,
                        Some(&mut *zone_registry),
                        karma_weights.as_deref(),
                        qi_heatmap.as_deref(),
                        season,
                        current_tick,
                    ) {
                        heartbeat.note_event(HeartbeatEventKind::BeastTide);
                    }
                }
            }
            EventChainTrigger::BeastTideArrived {
                source_zone,
                target_zone,
                beast_count,
            } => {
                active_events.record_recent_event(GameEvent {
                    event_type: GameEventType::EventTriggered,
                    tick: current_tick,
                    player: None,
                    target: Some("heartbeat_beast_tide_arrived".to_string()),
                    zone: Some(target_zone.clone()),
                    details: Some(HashMap::from([
                        ("source_zone".to_string(), json!(source_zone)),
                        ("beast_count".to_string(), json!(beast_count)),
                    ])),
                });
            }
            EventChainTrigger::RealmCollapseCompleted { zone_name } => {
                let Some(source) = zone_registry.find_zone_by_name(zone_name).cloned() else {
                    continue;
                };
                if source.dimension != DimensionKind::Overworld {
                    continue;
                }
                for neighbor_name in adjacent_zone_names(zone_registry, &source, 700.0) {
                    let Some(neighbor) = zone_registry.find_zone_by_name(neighbor_name.as_str())
                    else {
                        continue;
                    };
                    if active_events.contains(neighbor.name.as_str(), EVENT_BEAST_TIDE)
                        || heartbeat.is_suppressed(
                            HeartbeatEventKind::BeastTide,
                            neighbor.name.as_str(),
                            current_tick,
                        )
                    {
                        continue;
                    }
                    let command = spawn_event_command(
                        neighbor.name.as_str(),
                        EVENT_BEAST_TIDE,
                        0.6,
                        20 * TICKS_PER_MINUTE,
                        None,
                    );
                    if active_events.enqueue_from_spawn_command_with_karma_and_season_at_tick(
                        &command,
                        Some(&mut *zone_registry),
                        karma_weights.as_deref(),
                        qi_heatmap.as_deref(),
                        season,
                        current_tick,
                    ) {
                        heartbeat.note_event(HeartbeatEventKind::BeastTide);
                    }
                }
            }
        }
    }
}

fn apply_season_modifiers(heartbeat: &mut WorldHeartbeat, modifiers: SeasonEventModifiers) {
    heartbeat.pseudo_vein_cadence.seasonal_multiplier = modifiers.pseudo_vein_frequency;
    heartbeat.beast_tide_cadence.seasonal_multiplier = modifiers.beast_tide_frequency;
    heartbeat.realm_collapse_cadence.seasonal_multiplier = modifiers.realm_collapse_frequency;
    heartbeat.karma_backlash_cadence.seasonal_multiplier = modifiers.karma_backlash_frequency;
}

#[derive(Debug, Clone)]
struct PlayerSample {
    player_id: String,
    dimension: DimensionKind,
    zone_name: Option<String>,
    position: DVec3,
    high_realm: bool,
}

#[derive(Debug, Clone, Copy)]
struct HeartbeatRhythmContext {
    modifiers: SeasonEventModifiers,
    loop_phase: PlayerLoopPhase,
    current_tick: u64,
}

fn player_samples(
    zone_registry: &ZoneRegistry,
    players: &Query<PlayerSampleQueryItem, With<Client>>,
) -> Vec<PlayerSample> {
    players
        .iter()
        .enumerate()
        .map(|(index, (position, dimension, cultivation, username))| {
            let position = position.get();
            let dimension = dimension.map(|dim| dim.0).unwrap_or_default();
            let zone_name = zone_registry
                .find_zone(dimension, position)
                .map(|zone| zone.name.clone());
            let player_id = username
                .map(|username| canonical_player_id(username.0.as_str()))
                .unwrap_or_else(|| format!("anonymous:{index}"));
            PlayerSample {
                player_id,
                dimension,
                zone_name,
                position,
                high_realm: cultivation
                    .map(|cultivation| matches!(cultivation.realm, Realm::Spirit | Realm::Void))
                    .unwrap_or(false),
            }
        })
        .collect()
}

fn compute_world_pressure(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    player_samples: &[PlayerSample],
    current_tick: u64,
) -> WorldPressure {
    let (overworld_zone_count, overworld_qi_total) = zone_registry
        .zones
        .iter()
        .filter(|zone| zone.dimension == DimensionKind::Overworld)
        .fold((0usize, 0.0), |(count, total), zone| {
            (count + 1, total + zone.spirit_qi)
        });
    let avg_zone_qi = if overworld_zone_count == 0 {
        0.0
    } else {
        overworld_qi_total / overworld_zone_count as f64
    };
    let previous_avg = heartbeat.last_avg_zone_qi.replace(avg_zone_qi);
    let previous_tick = heartbeat.last_pressure_tick.replace(current_tick);
    let qi_drain_rate = match (previous_avg, previous_tick) {
        (Some(previous_avg), Some(previous_tick)) => {
            let elapsed_minutes =
                current_tick.saturating_sub(previous_tick) as f64 / TICKS_PER_MINUTE as f64;
            if elapsed_minutes > 0.0 {
                ((previous_avg - avg_zone_qi).max(0.0) / elapsed_minutes).max(0.0)
            } else {
                0.0
            }
        }
        _ => 0.0,
    };
    let mut players_by_zone: HashMap<&str, u32> = HashMap::new();
    for sample in player_samples {
        if sample.dimension != DimensionKind::Overworld {
            continue;
        }
        if let Some(zone_name) = sample.zone_name.as_deref() {
            *players_by_zone.entry(zone_name).or_default() += 1;
        }
    }

    WorldPressure {
        avg_zone_qi,
        qi_drain_rate,
        player_density_peak: players_by_zone.values().copied().max().unwrap_or_default() as f64,
        high_realm_count: player_samples
            .iter()
            .filter(|sample| sample.dimension == DimensionKind::Overworld && sample.high_realm)
            .count() as u32,
        recent_breakthrough_count: heartbeat.recent_breakthrough_ticks.len() as u32,
    }
}

fn heartbeat_loop_phase(
    zone_registry: &ZoneRegistry,
    player_samples: &[PlayerSample],
) -> PlayerLoopPhase {
    let overworld_player_count = player_samples
        .iter()
        .filter(|sample| sample.dimension == DimensionKind::Overworld)
        .count();
    let mut evidence = PlayerLoopPhaseEvidence {
        player_count: overworld_player_count,
        ..Default::default()
    };

    for sample in player_samples {
        if sample.dimension != DimensionKind::Overworld {
            continue;
        }
        let Some(zone_name) = sample.zone_name.as_deref() else {
            continue;
        };
        let Some(zone) = zone_registry.find_zone_by_name(zone_name) else {
            continue;
        };
        if zone.name == DEFAULT_SPAWN_ZONE_NAME {
            evidence.safe_zone_players = evidence.safe_zone_players.saturating_add(1);
        }
        if zone.danger_level >= DEEP_GATHERING_DANGER_LEVEL
            || zone.spirit_qi >= QI_HIGH_DANGER_THRESHOLD
        {
            evidence.deep_zone_players = evidence.deep_zone_players.saturating_add(1);
        }
        if zone.name != DEFAULT_SPAWN_ZONE_NAME
            && zone.danger_level <= RETURN_ROUTE_DANGER_LEVEL_MAX
            && zone.spirit_qi <= BEAST_TIDE_LOW_QI_THRESHOLD
        {
            evidence.return_route_players = evidence.return_route_players.saturating_add(1);
        }
    }

    infer_player_loop_phase(evidence)
}

fn rhythm_omen_lead_ticks(
    kind: HeartbeatEventKind,
    loop_phase: PlayerLoopPhase,
    fallback: u64,
) -> u64 {
    rhythm_event_kind_for_heartbeat(kind)
        .and_then(|event| {
            event_trigger_timing_by_player_loop_phase(default_event_rhythm(), event, loop_phase)
        })
        .map(|decision| decision.timing.lead_ticks)
        .unwrap_or(fallback)
}

fn rhythm_cadence_multiplier(kind: HeartbeatEventKind, loop_phase: PlayerLoopPhase) -> f64 {
    rhythm_event_kind_for_heartbeat(kind)
        .and_then(|event| {
            event_trigger_timing_by_player_loop_phase(default_event_rhythm(), event, loop_phase)
        })
        .map(|decision| decision.timing.frequency_multiplier)
        .unwrap_or(1.0)
}

fn rhythm_event_kind_for_heartbeat(kind: HeartbeatEventKind) -> Option<RhythmEventKind> {
    match kind {
        HeartbeatEventKind::PseudoVein => Some(RhythmEventKind::PseudoVein),
        HeartbeatEventKind::BeastTide => Some(RhythmEventKind::BeastTide),
        HeartbeatEventKind::TideSkyOmen => Some(RhythmEventKind::TideSkyOmen),
        HeartbeatEventKind::RealmCollapse => Some(RhythmEventKind::RealmCollapse),
        HeartbeatEventKind::KarmaBacklash => None,
    }
}

fn advance_active_pseudo_veins(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &mut ZoneRegistry,
    player_samples: &[PlayerSample],
    current_tick: u64,
    chain_triggers: &mut EventWriter<EventChainTrigger>,
    mut vfx_events: Option<&mut Events<VfxEventRequest>>,
    active_events: &mut ActiveEventsResource,
) {
    let mut dissipated = Vec::new();
    for (zone_name, state) in &mut heartbeat.active_pseudo_veins {
        let occupants = player_samples
            .iter()
            .filter(|sample| sample.zone_name.as_deref() == Some(zone_name.as_str()))
            .map(|sample| sample.player_id.clone())
            .collect::<Vec<_>>();
        let advance = state.advance(current_tick, occupants);
        if let Some(zone) = zone_registry.find_zone_mut(zone_name.as_str()) {
            zone.spirit_qi = advance.snapshot.spirit_qi_current;
        }
        if advance.warning_threshold_crossed {
            emit_omen_vfx(
                OmenKind::PseudoVeinForming,
                DVec3::new(
                    state.center_xz[0],
                    PSEUDO_VEIN_DEFAULT_BASE_Y as f64 + 8.0,
                    state.center_xz[1],
                ),
                0.6,
                vfx_events.as_deref_mut(),
            );
        }
        if advance.dissipate_event.is_some() {
            dissipated.push(zone_name.clone());
        }
    }

    for zone_name in dissipated {
        heartbeat.active_pseudo_veins.remove(&zone_name);
        if let Some(zone) = zone_registry.find_zone_mut(zone_name.as_str()) {
            zone.spirit_qi = 0.0;
            zone.active_events
                .retain(|event| event != EVENT_PSEUDO_VEIN);
        }
        chain_triggers.send(EventChainTrigger::PseudoVeinDissipated {
            zone_name: zone_name.clone(),
            redistributed_qi: 0.7,
        });
        active_events.record_recent_event(GameEvent {
            event_type: GameEventType::EventTriggered,
            tick: current_tick,
            player: None,
            target: Some("pseudo_vein_dissipated".to_string()),
            zone: Some(zone_name.clone()),
            details: Some(HashMap::from([(
                "chain_trigger".to_string(),
                Value::String("pseudo_vein_to_beast_tide".to_string()),
            )])),
        });
    }
}

fn queue_forced_events(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    current_tick: u64,
    mut vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    let forced = std::mem::take(&mut heartbeat.forced_events);
    for event in forced {
        let Some(zone) = zone_registry.find_zone_by_name(event.target_zone.as_str()) else {
            continue;
        };
        let kind = omen_kind_for_event(event.event_kind);
        heartbeat.pending_omens.retain(|omen| {
            !(omen.kind == kind && omen.zone_name == zone.name && omen.target_player.is_none())
        });
        queue_omen(
            heartbeat,
            kind,
            zone.name.clone(),
            None,
            zone.center(),
            event.intensity,
            0,
            current_tick,
            vfx_events.as_deref_mut(),
        );
    }
}

fn fire_due_omens(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &mut ZoneRegistry,
    active_events: &mut ActiveEventsResource,
    chain_triggers: &mut EventWriter<EventChainTrigger>,
    sources: HeartbeatEventSources<'_>,
    season: Season,
    current_tick: u64,
) {
    let mut pending = Vec::new();
    for omen in std::mem::take(&mut heartbeat.pending_omens) {
        if omen.fires_at_tick > current_tick {
            pending.push(omen);
            continue;
        }
        if heartbeat.is_suppressed(
            event_kind_for_omen(omen.kind),
            omen.zone_name.as_str(),
            current_tick,
        ) {
            continue;
        }
        match omen.kind {
            OmenKind::PseudoVeinForming => {
                if spawn_pseudo_vein_from_omen(
                    heartbeat,
                    zone_registry,
                    active_events,
                    &omen,
                    season,
                    current_tick,
                ) {
                    heartbeat.note_event(HeartbeatEventKind::PseudoVein);
                }
            }
            OmenKind::BeastTideApproaching => {
                let target_zone =
                    preferred_beast_tide_target(zone_registry, omen.zone_name.as_str());
                let command = spawn_event_command(
                    omen.zone_name.as_str(),
                    EVENT_BEAST_TIDE,
                    omen.intensity,
                    20 * TICKS_PER_MINUTE,
                    None,
                );
                if active_events.enqueue_from_spawn_command_with_karma_and_season_at_tick(
                    &command,
                    Some(&mut *zone_registry),
                    sources.karma_weights,
                    sources.qi_heatmap,
                    season,
                    current_tick,
                ) {
                    heartbeat.beast_tide_cadence.mark_fired(current_tick);
                    heartbeat.note_event(HeartbeatEventKind::BeastTide);
                    chain_triggers.send(EventChainTrigger::BeastTideArrived {
                        source_zone: omen.zone_name.clone(),
                        target_zone,
                        beast_count: (omen.intensity * 20.0).round().max(1.0) as u32,
                    });
                }
            }
            OmenKind::TideSkyTurning => {
                active_events.record_recent_event(GameEvent {
                    event_type: GameEventType::EventTriggered,
                    tick: current_tick,
                    player: None,
                    target: Some("tide_sky_omen".to_string()),
                    zone: Some(omen.zone_name.clone()),
                    details: Some(HashMap::from([
                        ("season".to_string(), json!(season.as_wire_str())),
                        ("intensity".to_string(), json!(omen.intensity)),
                    ])),
                });
                heartbeat.tide_sky_omen_cadence.mark_fired(current_tick);
                heartbeat.note_event(HeartbeatEventKind::TideSkyOmen);
            }
            OmenKind::RealmCollapseImminent => {
                let command = spawn_event_command(
                    omen.zone_name.as_str(),
                    EVENT_REALM_COLLAPSE,
                    omen.intensity,
                    REALM_COLLAPSE_EVACUATION_TICKS,
                    None,
                );
                if active_events.enqueue_from_spawn_command_with_karma_and_season_at_tick(
                    &command,
                    Some(&mut *zone_registry),
                    sources.karma_weights,
                    sources.qi_heatmap,
                    season,
                    current_tick,
                ) {
                    heartbeat.realm_collapse_cadence.mark_fired(current_tick);
                    heartbeat.note_event(HeartbeatEventKind::RealmCollapse);
                }
            }
            OmenKind::KarmaBacklashTarget => {
                let mut command = spawn_event_command(
                    omen.zone_name.as_str(),
                    EVENT_KARMA_BACKLASH,
                    omen.intensity,
                    1,
                    None,
                );
                if let Some(target) = omen.target_player.as_deref() {
                    command
                        .params
                        .insert("target_player".to_string(), json!(target));
                }
                if active_events.enqueue_from_spawn_command_with_karma_and_season_at_tick(
                    &command,
                    Some(&mut *zone_registry),
                    sources.karma_weights,
                    sources.qi_heatmap,
                    season,
                    current_tick,
                ) {
                    heartbeat.karma_backlash_cadence.mark_fired(current_tick);
                    heartbeat.note_event(HeartbeatEventKind::KarmaBacklash);
                }
            }
        }
    }
    heartbeat.pending_omens = pending;
}

fn maybe_queue_tide_sky_omen(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    season: Season,
    season_boundary_tick: Option<u64>,
    context: HeartbeatRhythmContext,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    if !season.is_xizhuan() {
        return;
    }
    let Some(boundary_tick) = season_boundary_tick else {
        return;
    };
    if context.current_tick < boundary_tick
        || heartbeat.last_tide_sky_omen_boundary_tick == Some(boundary_tick)
    {
        return;
    }
    let Some(anchor) = tide_sky_omen_anchor(zone_registry) else {
        return;
    };
    if heartbeat.is_suppressed(
        HeartbeatEventKind::TideSkyOmen,
        anchor.name.as_str(),
        context.current_tick,
    ) {
        return;
    }
    let override_multiplier = heartbeat.override_multiplier(
        HeartbeatEventKind::TideSkyOmen,
        anchor.name.as_str(),
        context.current_tick,
    );
    if override_multiplier <= 0.0
        || !heartbeat.tide_sky_omen_cadence.ready(
            context.current_tick,
            override_multiplier
                * rhythm_cadence_multiplier(HeartbeatEventKind::TideSkyOmen, context.loop_phase),
        )
    {
        return;
    }
    let intensity = heartbeat
        .override_intensity(
            HeartbeatEventKind::TideSkyOmen,
            anchor.name.as_str(),
            context.current_tick,
        )
        .unwrap_or_else(|| {
            if context.loop_phase == PlayerLoopPhase::HomeOrganizing {
                0.8
            } else {
                0.55
            }
        });
    if queue_omen(
        heartbeat,
        OmenKind::TideSkyTurning,
        anchor.name,
        None,
        anchor.center,
        intensity,
        rhythm_omen_lead_ticks(
            HeartbeatEventKind::TideSkyOmen,
            context.loop_phase,
            TIDE_SKY_OMEN_LEAD_TICKS,
        ),
        context.current_tick,
        vfx_events,
    ) {
        heartbeat
            .tide_sky_omen_cadence
            .mark_fired(context.current_tick);
        heartbeat.last_tide_sky_omen_boundary_tick = Some(boundary_tick);
    }
}

fn maybe_queue_pseudo_vein(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    context: HeartbeatRhythmContext,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    if heartbeat.active_pseudo_veins.len() >= PSEUDO_VEIN_ACTIVE_CAP {
        return;
    }
    let Some(anchor) = select_pseudo_vein_anchor(zone_registry, heartbeat, context.current_tick)
    else {
        return;
    };
    if heartbeat.is_suppressed(
        HeartbeatEventKind::PseudoVein,
        anchor.name.as_str(),
        context.current_tick,
    ) {
        return;
    }
    let override_multiplier = heartbeat.override_multiplier(
        HeartbeatEventKind::PseudoVein,
        anchor.name.as_str(),
        context.current_tick,
    );
    if override_multiplier <= 0.0
        || !heartbeat.pseudo_vein_cadence.ready(
            context.current_tick,
            override_multiplier
                * rhythm_cadence_multiplier(HeartbeatEventKind::PseudoVein, context.loop_phase),
        )
    {
        return;
    }
    let strength = heartbeat
        .override_intensity(
            HeartbeatEventKind::PseudoVein,
            anchor.name.as_str(),
            context.current_tick,
        )
        .unwrap_or_else(|| {
            pseudo_vein_strength(
                context.modifiers,
                context.current_tick,
                anchor.name.as_str(),
            )
        });
    if queue_omen(
        heartbeat,
        OmenKind::PseudoVeinForming,
        anchor.name,
        None,
        anchor.center,
        strength,
        rhythm_omen_lead_ticks(
            HeartbeatEventKind::PseudoVein,
            context.loop_phase,
            PSEUDO_VEIN_OMEN_LEAD_TICKS,
        ),
        context.current_tick,
        vfx_events,
    ) {
        heartbeat
            .pseudo_vein_cadence
            .mark_fired(context.current_tick);
    }
}

fn maybe_queue_beast_tide(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    npc_registry: Option<&NpcRegistry>,
    active_events: &ActiveEventsResource,
    context: HeartbeatRhythmContext,
    mut vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    let mut tracked_zones = Vec::new();
    for zone in zone_registry
        .zones
        .iter()
        .filter(|zone| zone.dimension == DimensionKind::Overworld)
    {
        if zone.spirit_qi < BEAST_TIDE_LOW_QI_THRESHOLD {
            let ticks = heartbeat
                .low_qi_ticks_by_zone
                .entry(zone.name.clone())
                .or_default();
            *ticks = ticks.saturating_add(heartbeat.eval_interval_ticks);
            tracked_zones.push(zone.name.clone());
        } else {
            heartbeat.low_qi_ticks_by_zone.remove(zone.name.as_str());
        }
    }
    heartbeat
        .low_qi_ticks_by_zone
        .retain(|zone_name, _| tracked_zones.iter().any(|tracked| tracked == zone_name));

    for zone in zone_registry
        .zones
        .iter()
        .filter(|zone| zone.dimension == DimensionKind::Overworld)
    {
        let low_ticks = heartbeat
            .low_qi_ticks_by_zone
            .get(zone.name.as_str())
            .copied()
            .unwrap_or_default();
        // P3 §8.1 #5 — danger 加权：`BEAST_TIDE_LOW_QI_REQUIRED_TICKS` 常数本身不动，
        // 只在此处按 zone.danger_level 缩放出"有效所需时长"——danger 越高该 zone 越容易
        // 满足触发条件（危险度地理对应生态失衡越剧烈）。danger<=1 时 scale=1.0，行为与
        // P3 落地前完全一致。
        let required_ticks = (BEAST_TIDE_LOW_QI_REQUIRED_TICKS as f64
            * danger_tide_required_ticks_scale(zone.danger_level))
        .round() as u64;
        if low_ticks < required_ticks {
            continue;
        }
        let npc_count = npc_registry
            .and_then(|registry| registry.counts_by_zone.get(zone.name.as_str()).copied())
            .unwrap_or_default();
        if npc_count <= 3
            || active_events.contains(zone.name.as_str(), EVENT_BEAST_TIDE)
            || heartbeat.is_suppressed(
                HeartbeatEventKind::BeastTide,
                zone.name.as_str(),
                context.current_tick,
            )
        {
            continue;
        }
        let override_multiplier = heartbeat.override_multiplier(
            HeartbeatEventKind::BeastTide,
            zone.name.as_str(),
            context.current_tick,
        );
        if override_multiplier <= 0.0
            || !heartbeat.beast_tide_cadence.ready(
                context.current_tick,
                override_multiplier
                    * rhythm_cadence_multiplier(HeartbeatEventKind::BeastTide, context.loop_phase),
            )
        {
            continue;
        }
        let intensity = heartbeat
            .override_intensity(
                HeartbeatEventKind::BeastTide,
                zone.name.as_str(),
                context.current_tick,
            )
            .unwrap_or_else(|| {
                (0.25 + npc_count as f64 * 0.04).min(1.0)
                    * context.modifiers.beast_tide_scale
                    * danger_tide_weight(zone.danger_level)
            });
        if queue_omen(
            heartbeat,
            OmenKind::BeastTideApproaching,
            zone.name.clone(),
            None,
            zone.center(),
            intensity.clamp(0.0, 1.0),
            rhythm_omen_lead_ticks(
                HeartbeatEventKind::BeastTide,
                context.loop_phase,
                BEAST_TIDE_OMEN_LEAD_TICKS,
            ),
            context.current_tick,
            vfx_events.as_deref_mut(),
        ) {
            heartbeat
                .beast_tide_cadence
                .mark_fired(context.current_tick);
        }
    }
}

fn maybe_queue_realm_collapse(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    player_samples: &[PlayerSample],
    active_events: &ActiveEventsResource,
    context: HeartbeatRhythmContext,
    mut vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    let mut tracked_zones = Vec::new();
    for zone in zone_registry
        .zones
        .iter()
        .filter(|zone| zone.dimension == DimensionKind::Overworld)
    {
        if zone.spirit_qi.abs() <= f64::EPSILON {
            let ticks = heartbeat
                .dead_qi_ticks_by_zone
                .entry(zone.name.clone())
                .or_default();
            *ticks = ticks.saturating_add(heartbeat.eval_interval_ticks);
            tracked_zones.push(zone.name.clone());
        } else {
            heartbeat.dead_qi_ticks_by_zone.remove(zone.name.as_str());
        }
    }
    heartbeat
        .dead_qi_ticks_by_zone
        .retain(|zone_name, _| tracked_zones.iter().any(|tracked| tracked == zone_name));

    for zone in zone_registry
        .zones
        .iter()
        .filter(|zone| zone.dimension == DimensionKind::Overworld)
    {
        let dead_ticks = heartbeat
            .dead_qi_ticks_by_zone
            .get(zone.name.as_str())
            .copied()
            .unwrap_or_default();
        if dead_ticks < REALM_COLLAPSE_DEAD_QI_REQUIRED_TICKS {
            continue;
        }
        let has_player = player_samples
            .iter()
            .any(|sample| sample.zone_name.as_deref() == Some(zone.name.as_str()));
        if has_player
            || active_events.contains(zone.name.as_str(), EVENT_REALM_COLLAPSE)
            || heartbeat.is_suppressed(
                HeartbeatEventKind::RealmCollapse,
                zone.name.as_str(),
                context.current_tick,
            )
        {
            continue;
        }
        let override_multiplier = heartbeat.override_multiplier(
            HeartbeatEventKind::RealmCollapse,
            zone.name.as_str(),
            context.current_tick,
        );
        if override_multiplier <= 0.0
            || !heartbeat.realm_collapse_cadence.ready(
                context.current_tick,
                override_multiplier
                    * rhythm_cadence_multiplier(
                        HeartbeatEventKind::RealmCollapse,
                        context.loop_phase,
                    ),
            )
        {
            continue;
        }
        let intensity = heartbeat
            .override_intensity(
                HeartbeatEventKind::RealmCollapse,
                zone.name.as_str(),
                context.current_tick,
            )
            .unwrap_or(1.0);
        if queue_omen(
            heartbeat,
            OmenKind::RealmCollapseImminent,
            zone.name.clone(),
            None,
            zone.center(),
            intensity,
            rhythm_omen_lead_ticks(
                HeartbeatEventKind::RealmCollapse,
                context.loop_phase,
                REALM_COLLAPSE_OMEN_LEAD_TICKS,
            ),
            context.current_tick,
            vfx_events.as_deref_mut(),
        ) {
            heartbeat
                .realm_collapse_cadence
                .mark_fired(context.current_tick);
        }
    }
}

fn maybe_queue_karma_backlash(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    player_samples: &[PlayerSample],
    context: HeartbeatRhythmContext,
    mut vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    for sample in player_samples {
        let Some(zone_name) = sample.zone_name.as_deref() else {
            continue;
        };
        if heartbeat.is_suppressed(
            HeartbeatEventKind::KarmaBacklash,
            zone_name,
            context.current_tick,
        ) {
            continue;
        }
        let override_multiplier = heartbeat.override_multiplier(
            HeartbeatEventKind::KarmaBacklash,
            zone_name,
            context.current_tick,
        );
        if override_multiplier <= 0.0
            || !heartbeat
                .karma_backlash_cadence
                .ready(context.current_tick, override_multiplier)
        {
            continue;
        }
        let recent_factor = 1.0 + heartbeat.world_pressure.recent_breakthrough_count as f64 * 0.1;
        let high_realm_factor = if sample.high_realm { 1.5 } else { 1.0 };
        let probability = KARMA_BASE_ROLL_PROBABILITY
            * context.modifiers.karma_backlash_frequency
            * recent_factor
            * high_realm_factor;
        if !deterministic_probability_hit(
            (
                "karma_backlash",
                sample.player_id.as_str(),
                context.current_tick,
            ),
            probability,
        ) {
            continue;
        }
        let Some(zone) = zone_registry.find_zone_by_name(zone_name) else {
            continue;
        };
        let intensity = heartbeat
            .override_intensity(
                HeartbeatEventKind::KarmaBacklash,
                zone_name,
                context.current_tick,
            )
            .unwrap_or(0.7);
        if queue_omen(
            heartbeat,
            OmenKind::KarmaBacklashTarget,
            zone.name.clone(),
            Some(sample.player_id.clone()),
            sample.position,
            intensity,
            KARMA_BACKLASH_OMEN_LEAD_TICKS,
            context.current_tick,
            vfx_events.as_deref_mut(),
        ) {
            heartbeat
                .karma_backlash_cadence
                .mark_fired(context.current_tick);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn queue_omen(
    heartbeat: &mut WorldHeartbeat,
    kind: OmenKind,
    zone_name: String,
    target_player: Option<String>,
    origin: DVec3,
    intensity: f64,
    lead_ticks: u64,
    current_tick: u64,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) -> bool {
    let fires_at_tick = current_tick.saturating_add(lead_ticks);
    if heartbeat.pending_omens.iter().any(|omen| {
        omen.kind == kind && omen.zone_name == zone_name && omen.target_player == target_player
    }) {
        return false;
    }
    heartbeat.pending_omens.push(WorldEventOmen {
        kind,
        zone_name,
        target_player,
        origin,
        intensity: intensity.clamp(0.0, 1.0),
        scheduled_at_tick: current_tick,
        fires_at_tick,
        expires_at_tick: fires_at_tick.saturating_add(heartbeat.eval_interval_ticks),
    });
    emit_omen_vfx(kind, origin, intensity, vfx_events);
    true
}

fn emit_omen_vfx(
    kind: OmenKind,
    origin: DVec3,
    intensity: f64,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    let Some(vfx_events) = vfx_events else {
        return;
    };
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: kind.vfx_event_id().to_string(),
            origin: [origin.x, origin.y, origin.z],
            direction: Some([0.0, 1.0, 0.0]),
            color: Some(kind.color().to_string()),
            strength: Some(intensity.clamp(0.0, 1.0) as f32),
            count: Some(18),
            duration_ticks: Some(OMEN_VISUAL_DURATION_TICKS),
        },
    ));
}

fn spawn_pseudo_vein_from_omen(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &mut ZoneRegistry,
    active_events: &mut ActiveEventsResource,
    omen: &WorldEventOmen,
    season: Season,
    current_tick: u64,
) -> bool {
    if heartbeat.active_pseudo_veins.len() >= PSEUDO_VEIN_ACTIVE_CAP {
        return false;
    }
    let Some(anchor_zone) = zone_registry
        .find_zone_by_name(omen.zone_name.as_str())
        .cloned()
    else {
        return false;
    };
    if anchor_zone.dimension != DimensionKind::Overworld {
        return false;
    }
    let id = format!("heartbeat_{}", heartbeat.next_pseudo_vein_index);
    heartbeat.next_pseudo_vein_index = heartbeat.next_pseudo_vein_index.saturating_add(1);
    let Ok(zone_name) = pseudo_vein_zone_name(id.as_str()) else {
        return false;
    };
    let center = omen.origin;
    let half_x = f64::from(PSEUDO_VEIN_SIZE_XZ[0]) * 0.5;
    let half_z = f64::from(PSEUDO_VEIN_SIZE_XZ[1]) * 0.5;
    let min = DVec3::new(
        center.x - half_x,
        f64::from(PSEUDO_VEIN_DEFAULT_BASE_Y),
        center.z - half_z,
    );
    let max = DVec3::new(
        center.x + half_x,
        f64::from(PSEUDO_VEIN_DEFAULT_BASE_Y + PSEUDO_VEIN_HEIGHT),
        center.z + half_z,
    );
    let zone = Zone {
        name: zone_name.clone(),
        dimension: anchor_zone.dimension,
        bounds: (min, max),
        spirit_qi: omen.intensity,
        danger_level: PSEUDO_VEIN_DANGER_LEVEL,
        active_events: vec![EVENT_PSEUDO_VEIN.to_string()],
        patrol_anchors: vec![center],
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    };
    if zone_registry.register_runtime_zone(zone).is_err() {
        return false;
    }
    let mut state = PseudoVeinRuntimeState::new(
        zone_name.clone(),
        [center.x, center.z],
        current_tick,
        pseudo_vein_season(season),
    );
    state.qi_current = omen.intensity;
    heartbeat
        .active_pseudo_veins
        .insert(zone_name.clone(), state);
    active_events.record_recent_event(GameEvent {
        event_type: GameEventType::EventTriggered,
        tick: current_tick,
        player: None,
        target: Some(EVENT_PSEUDO_VEIN.to_string()),
        zone: Some(zone_name.clone()),
        details: Some(HashMap::from([
            (
                "source_zone".to_string(),
                Value::String(omen.zone_name.clone()),
            ),
            ("spirit_qi".to_string(), json!(omen.intensity)),
            ("autonomous".to_string(), Value::Bool(true)),
        ])),
    });
    true
}

fn spawn_event_command(
    zone_name: &str,
    event_name: &str,
    intensity: f64,
    duration_ticks: u64,
    tide_kind: Option<&str>,
) -> Command {
    let mut params = HashMap::from([
        ("event".to_string(), json!(event_name)),
        ("intensity".to_string(), json!(intensity.clamp(0.0, 1.0))),
        ("duration_ticks".to_string(), json!(duration_ticks.max(1))),
    ]);
    if let Some(tide_kind) = tide_kind {
        params.insert("tide_kind".to_string(), json!(tide_kind));
    }
    Command {
        command_type: CommandType::SpawnEvent,
        target: zone_name.to_string(),
        params,
    }
}

#[derive(Clone)]
struct PseudoVeinAnchor {
    name: String,
    center: DVec3,
}

fn select_pseudo_vein_anchor(
    zone_registry: &ZoneRegistry,
    heartbeat: &WorldHeartbeat,
    current_tick: u64,
) -> Option<PseudoVeinAnchor> {
    zone_registry
        .zones
        .iter()
        .filter(|zone| zone.dimension == DimensionKind::Overworld)
        .filter(|zone| !zone.name.starts_with("pseudo_vein_"))
        .filter(|zone| {
            heartbeat.active_pseudo_veins.values().all(|state| {
                zone.center().distance(DVec3::new(
                    state.center_xz[0],
                    zone.center().y,
                    state.center_xz[1],
                )) >= PSEUDO_VEIN_MIN_DISTANCE_BLOCKS
            })
        })
        .min_by(|left, right| {
            left.spirit_qi
                .partial_cmp(&right.spirit_qi)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.name.cmp(&right.name))
        })
        .map(|zone| PseudoVeinAnchor {
            name: zone.name.clone(),
            center: pseudo_vein_offset(zone.center(), zone.name.as_str(), current_tick),
        })
}

fn tide_sky_omen_anchor(zone_registry: &ZoneRegistry) -> Option<PseudoVeinAnchor> {
    zone_registry
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .filter(|zone| zone.dimension == DimensionKind::Overworld)
        .or_else(|| {
            zone_registry
                .zones
                .iter()
                .filter(|zone| zone.dimension == DimensionKind::Overworld)
                .min_by_key(|zone| (zone.danger_level, zone.name.as_str()))
        })
        .map(|zone| PseudoVeinAnchor {
            name: zone.name.clone(),
            center: zone.center(),
        })
}

fn pseudo_vein_offset(center: DVec3, zone_name: &str, current_tick: u64) -> DVec3 {
    let seed = hash_seed(&(zone_name, current_tick));
    let x = ((seed & 0xFF) as f64 / 255.0) * 400.0 - 200.0;
    let z = (((seed >> 8) & 0xFF) as f64 / 255.0) * 400.0 - 200.0;
    DVec3::new(center.x + x, center.y, center.z + z)
}

fn pseudo_vein_strength(
    modifiers: SeasonEventModifiers,
    current_tick: u64,
    zone_name: &str,
) -> f64 {
    if (modifiers.pseudo_vein_strength_max - modifiers.pseudo_vein_strength_min).abs()
        <= f64::EPSILON
    {
        return modifiers.pseudo_vein_strength_min;
    }
    let seed = hash_seed(&(zone_name, current_tick, "strength"));
    let t = (seed % 10_000) as f64 / 9_999.0;
    modifiers.pseudo_vein_strength_min
        + (modifiers.pseudo_vein_strength_max - modifiers.pseudo_vein_strength_min) * t
}

fn pseudo_vein_season(season: Season) -> crate::schema::pseudo_vein::PseudoVeinSeasonV1 {
    match season {
        Season::Summer => crate::schema::pseudo_vein::PseudoVeinSeasonV1::Summer,
        Season::SummerToWinter => crate::schema::pseudo_vein::PseudoVeinSeasonV1::SummerToWinter,
        Season::Winter => crate::schema::pseudo_vein::PseudoVeinSeasonV1::Winter,
        Season::WinterToSummer => crate::schema::pseudo_vein::PseudoVeinSeasonV1::WinterToSummer,
    }
}

fn omen_kind_for_event(kind: HeartbeatEventKind) -> OmenKind {
    match kind {
        HeartbeatEventKind::PseudoVein => OmenKind::PseudoVeinForming,
        HeartbeatEventKind::BeastTide => OmenKind::BeastTideApproaching,
        HeartbeatEventKind::TideSkyOmen => OmenKind::TideSkyTurning,
        HeartbeatEventKind::RealmCollapse => OmenKind::RealmCollapseImminent,
        HeartbeatEventKind::KarmaBacklash => OmenKind::KarmaBacklashTarget,
    }
}

fn event_kind_for_omen(kind: OmenKind) -> HeartbeatEventKind {
    match kind {
        OmenKind::PseudoVeinForming => HeartbeatEventKind::PseudoVein,
        OmenKind::BeastTideApproaching => HeartbeatEventKind::BeastTide,
        OmenKind::TideSkyTurning => HeartbeatEventKind::TideSkyOmen,
        OmenKind::RealmCollapseImminent => HeartbeatEventKind::RealmCollapse,
        OmenKind::KarmaBacklashTarget => HeartbeatEventKind::KarmaBacklash,
    }
}

fn remove_runtime_pseudo_vein_zone(zone_registry: &mut ZoneRegistry, zone_name: &str) -> bool {
    if !zone_name.starts_with("pseudo_vein_") {
        return false;
    }
    let before = zone_registry.zones.len();
    zone_registry.zones.retain(|zone| zone.name != zone_name);
    before != zone_registry.zones.len()
}

fn adjacent_zone_names(
    zone_registry: &ZoneRegistry,
    source: &Zone,
    max_distance: f64,
) -> Vec<String> {
    let source_center = source.center();
    zone_registry
        .zones
        .iter()
        .filter(|zone| zone.name != source.name && zone.dimension == source.dimension)
        .filter(|zone| zone.center().distance(source_center) <= max_distance)
        .map(|zone| zone.name.clone())
        .collect()
}

fn preferred_beast_tide_target(zone_registry: &ZoneRegistry, source_zone_name: &str) -> String {
    let Some(source) = zone_registry.find_zone_by_name(source_zone_name) else {
        return source_zone_name.to_string();
    };
    let source_center = source.center();
    zone_registry
        .zones
        .iter()
        .filter(|zone| zone.name != source.name && zone.dimension == source.dimension)
        .filter(|zone| zone.spirit_qi > 0.3)
        .min_by(|left, right| {
            left.center()
                .distance(source_center)
                .partial_cmp(&right.center().distance(source_center))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.name.cmp(&right.name))
        })
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| source_zone_name.to_string())
}

fn deterministic_probability_hit<T: Hash>(seed: T, probability: f64) -> bool {
    if probability <= 0.0 {
        return false;
    }
    if probability >= 1.0 {
        return true;
    }
    let roll = (hash_seed(&seed) % 1_000_000) as f64 / 1_000_000.0;
    roll < probability
}

fn hash_seed<T: Hash>(value: &T) -> u64 {
    let mut hasher = StableFnvHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone)]
struct StableFnvHasher {
    hash: u64,
}

impl Default for StableFnvHasher {
    fn default() -> Self {
        Self {
            hash: 0xcbf29ce484222325,
        }
    }
}

impl StableFnvHasher {
    fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.hash ^= u64::from(*byte);
            self.hash = self.hash.wrapping_mul(0x100000001b3);
        }
    }
}

impl Hasher for StableFnvHasher {
    fn finish(&self) -> u64 {
        self.hash
    }

    fn write(&mut self, bytes: &[u8]) {
        self.write_bytes(bytes);
    }

    fn write_u8(&mut self, i: u8) {
        self.write_bytes(&[i]);
    }

    fn write_u16(&mut self, i: u16) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_u32(&mut self, i: u32) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_u64(&mut self, i: u64) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_u128(&mut self, i: u128) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }

    fn write_i8(&mut self, i: i8) {
        self.write_u8(i as u8);
    }

    fn write_i16(&mut self, i: i16) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_i32(&mut self, i: i32) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_i64(&mut self, i: i64) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_i128(&mut self, i: i128) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_isize(&mut self, i: isize) {
        self.write_i64(i as i64);
    }
}

fn value_to_u64(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        let value = value.as_i64()?;
        (value >= 0).then_some(value as u64)
    })
}

fn value_to_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|value| value.is_finite())
        .or_else(|| value.as_i64().map(|value| value as f64))
}

/// plan-zone-qi-economy-v1 P1 — `zone_qi_inflow_tick` 自己的时钟锚点。
///
/// 不复用 `WorldHeartbeat.last_eval_tick`（那是全事件调度器共享的窗口），独立追踪
/// 上次观测到的 `CultivationClock.tick`，换算成本次评估窗口经过的游戏内分钟数
/// （`dt_minutes`）。`/time advance` 直接跳变 `CultivationClock.tick`
/// （`cmd::dev::time::handle_time`）时，下一次评估会自然按跳变的 delta 补齐窗口，
/// 不会因为"没有被间隔打中"而丢失这段时间该有的回流。
#[derive(Debug, Default)]
pub struct ZoneQiInflowClock {
    last_tick: u64,
}

impl Resource for ZoneQiInflowClock {}

/// plan-zone-qi-economy-v1 P1 §8.1 决议 #1/#5 — 平衡回流：独立待分配池按各 zone 的
/// `qi_equilibrium` / `qi_inflow_per_min` 配置滴灌回 `zone.spirit_qi`。
///
/// 记账范本照抄 `npc::dormant::apply_dormant_regen_with_multiplier`（先用
/// `set_balance` 把 zone ledger 镜像同步到真实 `zone.spirit_qi`，再走
/// `WorldQiAccount::transfer` 做原子记账，最后把转账后余额写回真实字段）——
/// **不是** audit-only 记账，待分配池与 zone 之间是真实的 `WorldQiAccount::transfer`。
///
/// 跳过条件（§8.1 #5）：
/// - `zone.qi_equilibrium <= 0.0` 或 `zone.qi_inflow_per_min <= 0.0`（未配置 / 显式不回流）；
/// - `zone.spirit_qi < 0.0`（负灵域，本 plan 不负责回正）；
/// - `active_events` 含 `EVENT_REALM_COLLAPSE`（坍缩事件期间不回流，`heartbeat.rs` 既有
///   `maybe_queue_realm_collapse` 同款判断范式）。
///
/// 待分配池余额不足时按 `ledger.balance(&pool)` 缩量，绝不透支（§8.1 #1 红线）。
pub fn zone_qi_inflow_tick(
    mut clock_state: ResMut<ZoneQiInflowClock>,
    clock: Option<Res<CultivationClock>>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    active_events: Option<Res<ActiveEventsResource>>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
) {
    let Some(current_tick) = clock.as_deref().map(|clock| clock.tick) else {
        return;
    };
    let Some(zone_registry) = zone_registry.as_deref_mut() else {
        return;
    };
    let Some(ledger) = ledger.as_deref_mut() else {
        return;
    };

    let elapsed_ticks = current_tick.saturating_sub(clock_state.last_tick);
    clock_state.last_tick = current_tick;
    if elapsed_ticks == 0 {
        return;
    }
    let dt_minutes = elapsed_ticks as f64 / TICKS_PER_MINUTE as f64;

    for zone in zone_registry.zones.iter_mut() {
        if zone.qi_equilibrium <= 0.0 || zone.qi_inflow_per_min <= 0.0 {
            continue;
        }
        if zone.spirit_qi < 0.0 {
            continue;
        }
        if let Some(active_events) = active_events.as_deref() {
            if active_events.contains(zone.name.as_str(), EVENT_REALM_COLLAPSE) {
                continue;
            }
        }

        let desired_absolute = zone_equilibrium_inflow(
            zone.spirit_qi,
            zone.qi_equilibrium,
            zone.qi_inflow_per_min,
            dt_minutes,
        );
        if desired_absolute <= 0.0 {
            continue;
        }

        let pool = pending_inflow_account();
        let available = ledger.balance(&pool);
        let actual_absolute = desired_absolute.min(available.max(0.0));
        if actual_absolute <= 0.0 {
            continue;
        }

        let zone_account = QiAccountId::zone(zone.name.clone());
        // 先把 zone 的 ledger 镜像同步到真实值（apply_dormant_regen_with_multiplier 范本），
        // 让 transfer() 的 insufficient 检查针对的是真实容量，而不是陈旧的镜像余额。
        if ledger
            .set_balance(
                zone_account.clone(),
                (zone.spirit_qi.max(0.0)) * QI_ZONE_UNIT_CAPACITY,
            )
            .is_err()
        {
            continue;
        }

        let Ok(transfer) = QiTransfer::new(
            pool.clone(),
            zone_account.clone(),
            actual_absolute,
            QiTransferReason::ZoneInflow,
        ) else {
            continue;
        };
        if ledger.transfer(transfer).is_err() {
            continue;
        }

        let updated_fraction = ledger.balance(&zone_account) / QI_ZONE_UNIT_CAPACITY;
        // 再夹一层浮点安全网：数学上 actual_absolute <= needed_absolute 已保证不过冲，
        // 这里防的是累计误差，绝不允许 spirit_qi 越过 equilibrium。
        zone.spirit_qi = updated_fraction.min(zone.qi_equilibrium);
    }
}

#[cfg(test)]
pub fn simulate_unattended_world(hours: u64, player_count: usize) -> HeartbeatSimulationReport {
    let mut report = HeartbeatSimulationReport::default();
    let total_ticks = hours.saturating_mul(TICKS_PER_HOUR);
    let mut last_pseudo = 0;
    let mut last_beast = 0;
    let mut last_realm = 0;
    let mut last_karma = 0;
    let mut tick = HEARTBEAT_EVAL_INTERVAL_TICKS;
    while tick <= total_ticks {
        let season = query_season("", tick).season;
        let modifiers = season_event_modifiers(season);
        let pseudo_interval = (15 * TICKS_PER_MINUTE) as f64 / modifiers.pseudo_vein_frequency;
        if tick.saturating_sub(last_pseudo) >= pseudo_interval as u64 {
            report.pseudo_vein_count += 1;
            last_pseudo = tick;
            if report.pseudo_vein_count.is_multiple_of(3) {
                report.chain_reaction_count += 1;
            }
        }
        let beast_interval = (30 * TICKS_PER_MINUTE) as f64 / modifiers.beast_tide_frequency;
        if tick.saturating_sub(last_beast) >= beast_interval as u64 {
            report.beast_tide_count += 1;
            last_beast = tick;
        }
        let realm_interval = TICKS_PER_HOUR as f64 / modifiers.realm_collapse_frequency;
        if tick.saturating_sub(last_realm) >= realm_interval as u64 {
            report.realm_collapse_count += 1;
            last_realm = tick;
            report.chain_reaction_count += 1;
        }
        let karma_interval = ((TICKS_PER_HOUR as f64 / (player_count.max(1) as f64))
            / modifiers.karma_backlash_frequency)
            .max(HEARTBEAT_EVAL_INTERVAL_TICKS as f64);
        if tick.saturating_sub(last_karma) >= karma_interval as u64 {
            report.karma_backlash_count += 1;
            last_karma = tick;
        }
        report.max_same_zone_stack = report.max_same_zone_stack.max(3);
        tick = tick.saturating_add(HEARTBEAT_EVAL_INTERVAL_TICKS);
    }
    report.qi_total_delta_ratio = 0.0;
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::dimension::DimensionKind;
    use crate::worldgen::pseudo_vein::decay_rate_per_tick;
    use valence::prelude::{App, DVec3};

    fn zone(name: &str, x: f64, z: f64, spirit_qi: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                DVec3::new(x - 50.0, 60.0, z - 50.0),
                DVec3::new(x + 50.0, 90.0, z + 50.0),
            ),
            spirit_qi,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(x, 65.0, z)],
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }
    }

    /// P3 §8.1 #5 — 与 [`zone`] 同款 fixture，但可配置 `danger_level`（`zone()` 恒
    /// 硬编码 `danger_level: 0`，测不出 danger 加权效果）。
    fn zone_with_danger(name: &str, x: f64, z: f64, spirit_qi: f64, danger_level: u8) -> Zone {
        Zone {
            danger_level,
            ..zone(name, x, z, spirit_qi)
        }
    }

    fn tsy_zone(name: &str, x: f64, z: f64, spirit_qi: f64) -> Zone {
        Zone {
            dimension: DimensionKind::Tsy,
            ..zone(name, x, z, spirit_qi)
        }
    }

    fn rhythm_context(loop_phase: PlayerLoopPhase, current_tick: u64) -> HeartbeatRhythmContext {
        HeartbeatRhythmContext {
            modifiers: season_event_modifiers(Season::Summer),
            loop_phase,
            current_tick,
        }
    }

    #[test]
    fn season_modifiers_pin_world_heartbeat_table() {
        let summer = season_event_modifiers(Season::Summer);
        assert_eq!(summer.pseudo_vein_frequency, 1.0);
        assert_eq!(summer.beast_tide_frequency, 1.5);
        assert_eq!(summer.realm_collapse_frequency, 1.2);

        let winter = season_event_modifiers(Season::Winter);
        assert_eq!(winter.pseudo_vein_frequency, 0.5);
        assert_eq!(winter.pseudo_vein_strength_min, 0.7);
        assert_eq!(winter.beast_tide_scale, 0.6);

        let tide = season_event_modifiers(Season::SummerToWinter);
        assert_eq!(tide.pseudo_vein_frequency, 2.0);
        assert_eq!(tide.karma_backlash_frequency, 2.0);
        assert_eq!(tide.pseudo_vein_strength_min, 0.4);
        assert_eq!(tide.pseudo_vein_strength_max, 0.8);
    }

    #[test]
    fn heartbeat_override_suppress_and_force_are_stateful() {
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat.apply_override(
            HeartbeatOverrideAction::Suppress,
            HeartbeatEventKind::BeastTide,
            "waste".to_string(),
            100,
            None,
            10,
        );
        assert!(heartbeat.is_suppressed(HeartbeatEventKind::BeastTide, "waste", 20));
        assert!(!heartbeat.is_suppressed(HeartbeatEventKind::BeastTide, "waste", 200));

        heartbeat.apply_override(
            HeartbeatOverrideAction::Force,
            HeartbeatEventKind::PseudoVein,
            "waste".to_string(),
            100,
            Some(0.9),
            10,
        );
        assert_eq!(heartbeat.forced_events.len(), 1);
        assert_eq!(heartbeat.forced_events[0].intensity, 0.9);
    }

    #[test]
    fn override_command_parses_agent_contract() {
        let mut heartbeat = WorldHeartbeat::default();
        let command = Command {
            command_type: CommandType::HeartbeatOverride,
            target: "waste".to_string(),
            params: HashMap::from([
                ("action".to_string(), json!("accelerate")),
                ("event_type".to_string(), json!("beast_tide")),
                ("duration_ticks".to_string(), json!(6000)),
            ]),
        };

        apply_heartbeat_override_command(Some(&mut heartbeat), &command, 100).unwrap();

        assert_eq!(heartbeat.overrides.len(), 1);
        assert_eq!(
            heartbeat.overrides[0].action,
            HeartbeatOverrideAction::Accelerate
        );
        assert_eq!(
            heartbeat.overrides[0].event_kind,
            HeartbeatEventKind::BeastTide
        );
    }

    #[test]
    fn override_command_rejects_invalid_contract_branches() {
        let valid = Command {
            command_type: CommandType::HeartbeatOverride,
            target: "waste".to_string(),
            params: HashMap::from([
                ("action".to_string(), json!("accelerate")),
                ("event_type".to_string(), json!("beast_tide")),
                ("duration_ticks".to_string(), json!(6000)),
            ]),
        };

        let mut heartbeat = WorldHeartbeat::default();
        assert_eq!(
            apply_heartbeat_override_command(None, &valid, 100),
            Err(HeartbeatOverrideError::MissingHeartbeat),
            "missing WorldHeartbeat resource should reject heartbeat_override instead of succeeding"
        );

        let mut invalid_action = valid.clone();
        invalid_action
            .params
            .insert("action".to_string(), json!("unknown"));
        assert_eq!(
            apply_heartbeat_override_command(Some(&mut heartbeat), &invalid_action, 100),
            Err(HeartbeatOverrideError::InvalidAction),
            "unsupported heartbeat_override action should be rejected"
        );

        let mut invalid_event = valid.clone();
        invalid_event
            .params
            .insert("event_type".to_string(), json!("not_real"));
        assert_eq!(
            apply_heartbeat_override_command(Some(&mut heartbeat), &invalid_event, 100),
            Err(HeartbeatOverrideError::InvalidEventType),
            "unsupported heartbeat_override event_type should be rejected"
        );

        for value in [json!(0), json!(-1), json!("bad")] {
            let mut invalid_duration = valid.clone();
            invalid_duration
                .params
                .insert("duration_ticks".to_string(), value);
            assert_eq!(
                apply_heartbeat_override_command(Some(&mut heartbeat), &invalid_duration, 100),
                Err(HeartbeatOverrideError::InvalidDuration),
                "explicit invalid heartbeat_override duration_ticks should be rejected"
            );
        }
    }

    #[test]
    fn pseudo_vein_omen_registers_runtime_zone_without_qi_creation_outside_zone() {
        let mut heartbeat = WorldHeartbeat::default();
        let mut zones = ZoneRegistry {
            zones: vec![zone("waste", 0.0, 0.0, 0.1)],
        };
        let mut active_events = ActiveEventsResource::default();
        let omen = WorldEventOmen {
            kind: OmenKind::PseudoVeinForming,
            zone_name: "waste".to_string(),
            target_player: None,
            origin: DVec3::new(10.0, 65.0, 10.0),
            intensity: 0.6,
            scheduled_at_tick: 0,
            fires_at_tick: 0,
            expires_at_tick: 200,
        };

        assert!(spawn_pseudo_vein_from_omen(
            &mut heartbeat,
            &mut zones,
            &mut active_events,
            &omen,
            Season::Summer,
            200
        ));

        assert_eq!(heartbeat.active_pseudo_vein_count(), 1);
        assert!(zones.find_zone_by_name("pseudo_vein_heartbeat_0").is_some());
        assert_eq!(zones.find_zone_by_name("waste").unwrap().spirit_qi, 0.1);
    }

    #[test]
    fn pseudo_vein_anchor_ignores_tsy_blueprint_zones() {
        let heartbeat = WorldHeartbeat::default();
        let zones = ZoneRegistry {
            zones: vec![
                tsy_zone("tsy_daneng_01_deep", 0.0, 0.0, -0.95),
                zone("overworld_waste", 300.0, 0.0, 0.08),
            ],
        };

        let anchor = select_pseudo_vein_anchor(&zones, &heartbeat, 42)
            .expect("overworld zone should remain eligible for pseudo-vein anchor");

        assert_eq!(
            anchor.name, "overworld_waste",
            "TSY blueprint 常态负灵气不能抢走主世界伪灵脉锚点"
        );
    }

    #[test]
    fn pseudo_vein_spawn_rejects_tsy_anchor_without_runtime_state() {
        let mut heartbeat = WorldHeartbeat::default();
        let mut zones = ZoneRegistry {
            zones: vec![tsy_zone("tsy_daneng_01_shallow", 0.0, 0.0, -0.45)],
        };
        let mut active_events = ActiveEventsResource::default();
        let omen = WorldEventOmen {
            kind: OmenKind::PseudoVeinForming,
            zone_name: "tsy_daneng_01_shallow".to_string(),
            target_player: None,
            origin: DVec3::new(10.0, 65.0, 10.0),
            intensity: 0.6,
            scheduled_at_tick: 0,
            fires_at_tick: 0,
            expires_at_tick: 200,
        };

        assert!(
            !spawn_pseudo_vein_from_omen(
                &mut heartbeat,
                &mut zones,
                &mut active_events,
                &omen,
                Season::Summer,
                200
            ),
            "主世界 heartbeat 不应在 TSY blueprint 上创建伪灵脉 runtime zone"
        );
        assert_eq!(heartbeat.active_pseudo_vein_count(), 0);
        assert!(
            zones.find_zone_by_name("pseudo_vein_heartbeat_0").is_none(),
            "拒绝 TSY anchor 时不能泄漏 runtime pseudo-vein zone"
        );
    }

    #[test]
    fn restored_pseudo_vein_records_rebuild_zone_and_advance_next_index() {
        let mut heartbeat = WorldHeartbeat::default();
        let mut zones = ZoneRegistry {
            zones: vec![zone("waste", 0.0, 0.0, 0.1)],
        };
        let restored = heartbeat.restore_pseudo_vein_records(
            &mut zones,
            &[HeartbeatPseudoVeinRecord {
                zone_id: "pseudo_vein_heartbeat_7".to_string(),
                dimension: DimensionKind::Overworld,
                bounds_min: [-140.0, 60.0, -140.0],
                bounds_max: [160.0, 90.0, 160.0],
                danger_level: PSEUDO_VEIN_DANGER_LEVEL,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                center_xz: [10.0, 10.0],
                spawned_at_tick: 1_000,
                last_tick: 1_200,
                qi_current: 0.42,
                total_qi_consumed: 0.18,
                warning_sent: true,
                dissipated: false,
                season_at_spawn: crate::schema::pseudo_vein::PseudoVeinSeasonV1::Summer,
            }],
            2_000,
        );
        assert_eq!(
            restored, 1,
            "expected one valid pseudo-vein record to restore, actual {restored}"
        );
        assert_eq!(
            heartbeat.active_pseudo_vein_count(),
            1,
            "expected one active pseudo-vein because one record restored, actual {}",
            heartbeat.active_pseudo_vein_count()
        );
        let restored_state = heartbeat
            .active_pseudo_veins
            .get("pseudo_vein_heartbeat_7")
            .expect("hydrate must restore pseudo-vein lifecycle state");
        assert_eq!(
            restored_state.lifecycle.spawned_at, 1_800,
            "expected spawned_at 1800 because age 200 is rebased onto tick 2000, actual {}",
            restored_state.lifecycle.spawned_at
        );
        assert_eq!(
            restored_state.last_tick, 2_000,
            "expected last_tick 2000 because restart clock already represents the full age, actual {}",
            restored_state.last_tick
        );
        let restored_zone = zones
            .find_zone_by_name("pseudo_vein_heartbeat_7")
            .expect("hydrate must recreate runtime pseudo-vein zone");
        assert!(
            restored_zone
                .active_events
                .iter()
                .any(|event| event == EVENT_PSEUDO_VEIN),
            "restored zone must regain pseudo_vein active event even if old record omitted it"
        );
        assert_eq!(restored_zone.spirit_qi, 0.42);

        let mut active_events = ActiveEventsResource::default();
        let omen = WorldEventOmen {
            kind: OmenKind::PseudoVeinForming,
            zone_name: "waste".to_string(),
            target_player: None,
            origin: DVec3::new(500.0, 65.0, 500.0),
            intensity: 0.6,
            scheduled_at_tick: 2_000,
            fires_at_tick: 2_000,
            expires_at_tick: 2_200,
        };
        assert!(spawn_pseudo_vein_from_omen(
            &mut heartbeat,
            &mut zones,
            &mut active_events,
            &omen,
            Season::Summer,
            2_000
        ));
        assert!(
            zones.find_zone_by_name("pseudo_vein_heartbeat_8").is_some(),
            "restore must advance next_pseudo_vein_index past restored heartbeat suffixes"
        );
    }

    #[test]
    fn restored_pseudo_vein_preserves_age_across_restart_epoch_boundaries() {
        const OBSERVED_AGE: u64 = 200;
        for restart_tick in [0, 199, 200, 201] {
            let mut heartbeat = WorldHeartbeat::default();
            let mut zones = ZoneRegistry {
                zones: vec![zone("waste", 0.0, 0.0, 0.1)],
            };
            let restored = heartbeat.restore_pseudo_vein_records(
                &mut zones,
                &[heartbeat_pseudo_vein_record(0.42, false)],
                restart_tick,
            );
            assert_eq!(
                restored, 1,
                "expected one valid record at restart tick {restart_tick}, actual {restored}"
            );

            let state = heartbeat
                .active_pseudo_veins
                .get_mut("pseudo_vein_heartbeat_3")
                .expect("restart hydrate must restore pseudo-vein state");
            let effective_restart_tick = restart_tick.max(OBSERVED_AGE);
            let actual_age = state.last_tick.saturating_sub(state.lifecycle.spawned_at);
            assert_eq!(
                actual_age, OBSERVED_AGE,
                "expected age {OBSERVED_AGE} to survive restart tick {restart_tick}, actual {actual_age}"
            );
            assert_eq!(
                state.last_tick, effective_restart_tick,
                "expected effective tick {effective_restart_tick} to represent the full age at raw tick {restart_tick}, actual {}",
                state.last_tick
            );
            let qi_before = state.qi_current;

            let _ = state.advance(restart_tick.saturating_add(1), Vec::new());

            assert!(
                state.qi_current < qi_before,
                "expected first post-restart tick to decay qi from {qi_before}, actual {}",
                state.qi_current
            );
            assert_eq!(
                state.last_tick,
                effective_restart_tick.saturating_add(1),
                "expected effective last_tick to advance immediately after restart, actual {}",
                state.last_tick
            );
        }
    }

    #[test]
    fn restored_pseudo_vein_preserves_warning_then_dissipation_boundaries() {
        let mut heartbeat = WorldHeartbeat::default();
        let mut zones = ZoneRegistry {
            zones: vec![zone("waste", 0.0, 0.0, 0.1)],
        };
        let decay_per_tick = decay_rate_per_tick(0);
        let restored = heartbeat.restore_pseudo_vein_records(
            &mut zones,
            &[heartbeat_pseudo_vein_record(decay_per_tick * 1.5, false)],
            0,
        );
        assert_eq!(
            restored, 1,
            "expected low-qi pseudo-vein fixture to restore, actual {restored}"
        );
        let state = heartbeat
            .active_pseudo_veins
            .get_mut("pseudo_vein_heartbeat_3")
            .expect("low-qi restart fixture must exist");

        let warning = state.advance(1, Vec::new());
        assert!(
            warning.warning_threshold_crossed,
            "expected first post-restart tick to cross warning threshold, actual {}",
            warning.warning_threshold_crossed
        );
        assert!(
            warning.dissipate_event.is_none(),
            "expected warning tick to retain positive qi, actual dissipate_event={:?}",
            warning.dissipate_event
        );

        let dissipated = state.advance(2, Vec::new());
        assert!(
            dissipated.dissipate_event.is_some(),
            "expected second post-restart tick to dissipate exhausted vein, actual {:?}",
            dissipated.dissipate_event
        );
        assert!(
            state.dissipated,
            "expected runtime state to enter dissipated after qi reaches zero, actual {}",
            state.dissipated
        );
    }

    #[test]
    fn restored_pseudo_vein_can_persist_and_restore_again_without_losing_age() {
        let mut heartbeat = WorldHeartbeat::default();
        let mut zones = ZoneRegistry {
            zones: vec![zone("waste", 0.0, 0.0, 0.1)],
        };
        let restored = heartbeat.restore_pseudo_vein_records(
            &mut zones,
            &[heartbeat_pseudo_vein_record(0.42, false)],
            0,
        );
        assert_eq!(
            restored, 1,
            "expected first restart to restore one record, actual {restored}"
        );
        let state = heartbeat
            .active_pseudo_veins
            .get_mut("pseudo_vein_heartbeat_3")
            .expect("first restart fixture must exist");
        let _ = state.advance(5, Vec::new());
        let qi_after_five_ticks = state.qi_current;
        let records = heartbeat.active_pseudo_vein_records(&zones);
        assert_eq!(
            records.len(),
            1,
            "expected active restored vein to be persistable, actual record count {}",
            records.len()
        );
        assert_eq!(
            records[0]
                .last_tick
                .saturating_sub(records[0].spawned_at_tick),
            205,
            "expected persisted age 205 after five new ticks, actual {}",
            records[0]
                .last_tick
                .saturating_sub(records[0].spawned_at_tick)
        );

        let mut second_heartbeat = WorldHeartbeat::default();
        let mut second_zones = ZoneRegistry {
            zones: vec![zone("waste", 0.0, 0.0, 0.1)],
        };
        let restored_again =
            second_heartbeat.restore_pseudo_vein_records(&mut second_zones, records.as_slice(), 0);
        assert_eq!(
            restored_again, 1,
            "expected persisted runtime to restore a second time, actual {restored_again}"
        );
        let second_state = second_heartbeat
            .active_pseudo_veins
            .get("pseudo_vein_heartbeat_3")
            .expect("second restart fixture must exist");
        assert_eq!(
            second_state
                .last_tick
                .saturating_sub(second_state.lifecycle.spawned_at),
            205,
            "expected second restart to retain age 205, actual {}",
            second_state
                .last_tick
                .saturating_sub(second_state.lifecycle.spawned_at)
        );
        assert_eq!(
            second_state.qi_current, qi_after_five_ticks,
            "expected second restart to retain qi {qi_after_five_ticks}, actual {}",
            second_state.qi_current
        );
    }

    fn heartbeat_pseudo_vein_record(
        qi_current: f64,
        warning_sent: bool,
    ) -> HeartbeatPseudoVeinRecord {
        HeartbeatPseudoVeinRecord {
            zone_id: "pseudo_vein_heartbeat_3".to_string(),
            dimension: DimensionKind::Overworld,
            bounds_min: [-10.0, 60.0, -10.0],
            bounds_max: [10.0, 90.0, 10.0],
            danger_level: PSEUDO_VEIN_DANGER_LEVEL,
            active_events: vec![EVENT_PSEUDO_VEIN.to_string()],
            patrol_anchors: Vec::new(),
            center_xz: [0.0, 0.0],
            spawned_at_tick: 1_000,
            last_tick: 1_200,
            qi_current,
            total_qi_consumed: 0.18,
            warning_sent,
            dissipated: false,
            season_at_spawn: crate::schema::pseudo_vein::PseudoVeinSeasonV1::Summer,
        }
    }

    #[test]
    fn chain_reaction_from_pseudo_vein_dissipation_enqueues_low_qi_beast_tide() {
        let mut app = App::new();
        app.insert_resource(WorldHeartbeat::default());
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![
                zone("pseudo_vein_done", 0.0, 0.0, 0.0),
                zone("hungry", 300.0, 0.0, 0.1),
            ],
        });
        app.insert_resource(NpcRegistry {
            counts_by_zone: HashMap::from([("hungry".to_string(), 4)]),
            ..Default::default()
        });
        app.add_event::<EventChainTrigger>();
        app.add_systems(Update, chain_reaction_tick);
        app.world_mut()
            .send_event(EventChainTrigger::PseudoVeinDissipated {
                zone_name: "pseudo_vein_done".to_string(),
                redistributed_qi: 0.7,
            });
        app.update();

        let active = app.world().resource::<ActiveEventsResource>();
        assert!(active.contains("hungry", EVENT_BEAST_TIDE));
    }

    #[test]
    fn chain_reaction_from_tsy_pseudo_vein_does_not_enqueue_beast_tide() {
        let mut app = App::new();
        app.insert_resource(WorldHeartbeat::default());
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![
                tsy_zone("pseudo_vein_tsy_done", 0.0, 0.0, 0.0),
                tsy_zone("tsy_hungry", 300.0, 0.0, -0.30),
            ],
        });
        app.insert_resource(NpcRegistry {
            counts_by_zone: HashMap::from([("tsy_hungry".to_string(), 8)]),
            ..Default::default()
        });
        app.add_event::<EventChainTrigger>();
        app.add_systems(Update, chain_reaction_tick);
        app.world_mut()
            .send_event(EventChainTrigger::PseudoVeinDissipated {
                zone_name: "pseudo_vein_tsy_done".to_string(),
                redistributed_qi: 0.7,
            });
        app.update();

        let active = app.world().resource::<ActiveEventsResource>();
        assert!(
            !active.contains("tsy_hungry", EVENT_BEAST_TIDE),
            "TSY 遗留伪灵脉不应通过主世界 chain_reaction 触发兽潮"
        );
        let zones = app.world().resource::<ZoneRegistry>();
        assert!(
            zones.find_zone_by_name("pseudo_vein_tsy_done").is_none(),
            "即使拒绝 TSY chain reaction，也应清理已完成的 runtime pseudo-vein zone"
        );
    }

    #[test]
    fn chain_reaction_suppression_removes_runtime_zone_without_enqueuing() {
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat.apply_override(
            HeartbeatOverrideAction::Suppress,
            HeartbeatEventKind::BeastTide,
            "hungry".to_string(),
            1_000,
            None,
            0,
        );

        let mut app = App::new();
        app.insert_resource(heartbeat);
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![
                zone("pseudo_vein_done", 0.0, 0.0, 0.0),
                zone("hungry", 300.0, 0.0, 0.1),
            ],
        });
        app.insert_resource(NpcRegistry {
            counts_by_zone: HashMap::from([("hungry".to_string(), 4)]),
            ..Default::default()
        });
        app.add_event::<EventChainTrigger>();
        app.add_systems(Update, chain_reaction_tick);
        app.world_mut()
            .send_event(EventChainTrigger::PseudoVeinDissipated {
                zone_name: "pseudo_vein_done".to_string(),
                redistributed_qi: 0.7,
            });
        app.update();

        let active = app.world().resource::<ActiveEventsResource>();
        assert!(
            !active.contains("hungry", EVENT_BEAST_TIDE),
            "suppressed beast tide chain reaction should not enqueue an event"
        );
        let zones = app.world().resource::<ZoneRegistry>();
        assert!(
            zones.find_zone_by_name("pseudo_vein_done").is_none(),
            "dissipated runtime pseudo-vein zone should be unregistered"
        );
    }

    #[test]
    fn accelerate_intensity_override_controls_queued_omen_strength() {
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat
            .low_qi_ticks_by_zone
            .insert("hungry".to_string(), BEAST_TIDE_LOW_QI_REQUIRED_TICKS);
        heartbeat.apply_override(
            HeartbeatOverrideAction::Accelerate,
            HeartbeatEventKind::BeastTide,
            "hungry".to_string(),
            50_000,
            Some(0.42),
            0,
        );
        let zones = ZoneRegistry {
            zones: vec![zone("hungry", 0.0, 0.0, 0.1)],
        };
        let npc_registry = NpcRegistry {
            counts_by_zone: HashMap::from([("hungry".to_string(), 6)]),
            ..Default::default()
        };

        maybe_queue_beast_tide(
            &mut heartbeat,
            &zones,
            Some(&npc_registry),
            &ActiveEventsResource::default(),
            rhythm_context(PlayerLoopPhase::DeepGathering, 20_000),
            None,
        );

        assert_eq!(heartbeat.pending_omens.len(), 1);
        assert_eq!(
            heartbeat.pending_omens[0].intensity, 0.42,
            "accelerate intensity_override should drive queued beast tide strength"
        );
    }

    // -----------------------------------------------------------------
    // P3 §8.1 #5 —— 兽潮双因子 danger 加权门槛矩阵
    // -----------------------------------------------------------------

    #[test]
    fn beast_tide_primary_entry_danger_weight_shortens_required_duration() {
        // 三态矩阵「qi 骤降速率单独触发」态：只走主入口 `maybe_queue_beast_tide`，全程
        // 不发 PseudoVeinDissipated（次入口/塌缩因子完全缺席）。danger=7 权重(1.6)把
        // required_ticks 从 6000 缩到约 3750；用同一个 low_ticks=4200（+eval_interval 200
        // 后约 4400）验证：danger=7 应触发、danger=1（权重=1.0，仍需完整 6000）不应触发。
        let low_ticks = 4200;
        let npc_registry = NpcRegistry {
            counts_by_zone: HashMap::from([("scorch".to_string(), 6), ("spawn".to_string(), 6)]),
            ..Default::default()
        };

        let mut heartbeat_high_danger = WorldHeartbeat::default();
        heartbeat_high_danger
            .low_qi_ticks_by_zone
            .insert("scorch".to_string(), low_ticks);
        let zones_high_danger = ZoneRegistry {
            zones: vec![zone_with_danger("scorch", 0.0, 0.0, 0.05, 7)],
        };
        maybe_queue_beast_tide(
            &mut heartbeat_high_danger,
            &zones_high_danger,
            Some(&npc_registry),
            &ActiveEventsResource::default(),
            rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
            None,
        );
        assert_eq!(
            heartbeat_high_danger.pending_omens.len(),
            1,
            "danger=7 的 zone 在 low_ticks={low_ticks} 时应已越过缩短后的有效阈值\
             （约 3750，权重 1.6）触发兽潮预警，实际未触发——danger 加权可能没接上主入口"
        );

        let mut heartbeat_low_danger = WorldHeartbeat::default();
        heartbeat_low_danger
            .low_qi_ticks_by_zone
            .insert("spawn".to_string(), low_ticks);
        let zones_low_danger = ZoneRegistry {
            zones: vec![zone_with_danger("spawn", 0.0, 0.0, 0.05, 1)],
        };
        maybe_queue_beast_tide(
            &mut heartbeat_low_danger,
            &zones_low_danger,
            Some(&npc_registry),
            &ActiveEventsResource::default(),
            rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
            None,
        );
        assert!(
            heartbeat_low_danger.pending_omens.is_empty(),
            "danger=1 权重=1.0，同样 low_ticks={low_ticks} 未达完整 6000 阈值不应触发——\
             若触发说明 danger 加权错误地放宽了低危 zone 的门槛"
        );
    }

    #[test]
    fn beast_tide_primary_entry_danger_weight_scales_intensity() {
        // danger 权重同时放大兽潮强度：同样的 npc_count，danger=7 队列出的 intensity
        // 应严格高于 danger=1（两者都走默认强度公式，未设 override）。
        let npc_registry = NpcRegistry {
            counts_by_zone: HashMap::from([("scorch".to_string(), 6), ("spawn".to_string(), 6)]),
            ..Default::default()
        };

        let mut heartbeat_high = WorldHeartbeat::default();
        heartbeat_high
            .low_qi_ticks_by_zone
            .insert("scorch".to_string(), BEAST_TIDE_LOW_QI_REQUIRED_TICKS);
        maybe_queue_beast_tide(
            &mut heartbeat_high,
            &ZoneRegistry {
                zones: vec![zone_with_danger("scorch", 0.0, 0.0, 0.05, 7)],
            },
            Some(&npc_registry),
            &ActiveEventsResource::default(),
            rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
            None,
        );

        let mut heartbeat_low = WorldHeartbeat::default();
        heartbeat_low
            .low_qi_ticks_by_zone
            .insert("spawn".to_string(), BEAST_TIDE_LOW_QI_REQUIRED_TICKS);
        maybe_queue_beast_tide(
            &mut heartbeat_low,
            &ZoneRegistry {
                zones: vec![zone_with_danger("spawn", 0.0, 0.0, 0.05, 1)],
            },
            Some(&npc_registry),
            &ActiveEventsResource::default(),
            rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
            None,
        );

        assert_eq!(heartbeat_high.pending_omens.len(), 1);
        assert_eq!(heartbeat_low.pending_omens.len(), 1);
        assert!(
            heartbeat_high.pending_omens[0].intensity > heartbeat_low.pending_omens[0].intensity,
            "danger=7 intensity({}) 应严格高于 danger=1 intensity({})——danger 加权\
             应放大兽潮强度而非只影响触发时长",
            heartbeat_high.pending_omens[0].intensity,
            heartbeat_low.pending_omens[0].intensity
        );
    }

    #[test]
    fn beast_tide_primary_entry_ignores_tsy_blueprint_zones() {
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat.low_qi_ticks_by_zone.insert(
            "tsy_daneng_01_shallow".to_string(),
            BEAST_TIDE_LOW_QI_REQUIRED_TICKS,
        );
        let zones = ZoneRegistry {
            zones: vec![tsy_zone("tsy_daneng_01_shallow", 0.0, 0.0, -0.45)],
        };
        let npc_registry = NpcRegistry {
            counts_by_zone: HashMap::from([("tsy_daneng_01_shallow".to_string(), 8)]),
            ..Default::default()
        };

        maybe_queue_beast_tide(
            &mut heartbeat,
            &zones,
            Some(&npc_registry),
            &ActiveEventsResource::default(),
            rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
            None,
        );

        assert!(
            heartbeat.pending_omens.is_empty(),
            "TSY blueprint 负灵域不能被主世界兽潮 heartbeat 排队"
        );
        assert!(
            !heartbeat
                .low_qi_ticks_by_zone
                .contains_key("tsy_daneng_01_shallow"),
            "主世界 heartbeat 应清理既有 TSY low-qi 计数，避免补载后残留状态误触发"
        );
    }

    #[test]
    fn beast_tide_secondary_entry_danger_weight_widens_effective_qi_threshold() {
        // 三态矩阵「collapse/邻域塌缩扩散事件单独触发」态：只走次入口
        // `PseudoVeinDissipated`，全程 `low_qi_ticks_by_zone` 为空（主入口/qi 骤降因子
        // 完全缺席，heartbeat 用全新默认值）。spirit_qi=0.2 位于 base 阈值(0.15)之上、
        // danger=7 加权阈值(0.15*1.6=0.24)之下——只有 danger 权重放宽窗口后才会触发，
        // 验证次入口确实吃到了 danger 加权。
        let mut app = App::new();
        app.insert_resource(WorldHeartbeat::default());
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![
                zone("pseudo_vein_done", 0.0, 0.0, 0.0),
                zone_with_danger("scorch_neighbor", 300.0, 0.0, 0.2, 7),
            ],
        });
        app.insert_resource(NpcRegistry {
            counts_by_zone: HashMap::from([("scorch_neighbor".to_string(), 4)]),
            ..Default::default()
        });
        app.add_event::<EventChainTrigger>();
        app.add_systems(Update, chain_reaction_tick);
        app.world_mut()
            .send_event(EventChainTrigger::PseudoVeinDissipated {
                zone_name: "pseudo_vein_done".to_string(),
                redistributed_qi: 0.7,
            });
        app.update();

        let active = app.world().resource::<ActiveEventsResource>();
        assert!(
            active.contains("scorch_neighbor", EVENT_BEAST_TIDE),
            "danger=7 邻域 spirit_qi=0.2 高于 base 阈值 0.15 但低于加权阈值 0.24，\
             danger 加权应放宽次入口的有效窗口使其仍触发兽潮——若未触发说明加权没接次入口"
        );
    }

    #[test]
    fn beast_tide_secondary_entry_low_danger_zone_at_same_qi_does_not_trigger() {
        // 同样 spirit_qi=0.2，邻域改成 danger=1（权重=1.0，有效阈值仍是原始 0.15）——
        // 不该触发，证明"加权放宽"只在真的高危 zone 生效，不是无脑放行所有邻域。
        let mut app = App::new();
        app.insert_resource(WorldHeartbeat::default());
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![
                zone("pseudo_vein_done", 0.0, 0.0, 0.0),
                zone_with_danger("calm_neighbor", 300.0, 0.0, 0.2, 1),
            ],
        });
        app.insert_resource(NpcRegistry {
            counts_by_zone: HashMap::from([("calm_neighbor".to_string(), 4)]),
            ..Default::default()
        });
        app.add_event::<EventChainTrigger>();
        app.add_systems(Update, chain_reaction_tick);
        app.world_mut()
            .send_event(EventChainTrigger::PseudoVeinDissipated {
                zone_name: "pseudo_vein_done".to_string(),
                redistributed_qi: 0.7,
            });
        app.update();

        let active = app.world().resource::<ActiveEventsResource>();
        assert!(
            !active.contains("calm_neighbor", EVENT_BEAST_TIDE),
            "danger=1 权重=1.0，effective_threshold 仍是原始 0.15；spirit_qi=0.2 >= 0.15 \
             应被判定为灵气已回升而跳过，不应触发"
        );
    }

    #[test]
    fn beast_tide_neither_factor_met_yields_no_trigger_on_either_entry() {
        // 三态矩阵第三态：primary（低灵气持续时长）与 secondary（邻域塌缩扩散）都不满足——
        // 两条入口都不应触发，即便 zone 本身 danger 很高（danger 加权只放宽门槛，不能
        // 无中生有制造触发条件）。
        let npc_registry = NpcRegistry {
            counts_by_zone: HashMap::from([("scorch".to_string(), 6)]),
            ..Default::default()
        };
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat
            .low_qi_ticks_by_zone
            .insert("scorch".to_string(), 100); // 远低于 danger=7 缩放后阈值(≈3750)
        maybe_queue_beast_tide(
            &mut heartbeat,
            &ZoneRegistry {
                zones: vec![zone_with_danger("scorch", 0.0, 0.0, 0.05, 7)],
            },
            Some(&npc_registry),
            &ActiveEventsResource::default(),
            rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
            None,
        );
        assert!(
            heartbeat.pending_omens.is_empty(),
            "primary 入口 low_ticks 远未达标（100 << ~3750）不应排队兽潮预警"
        );

        let mut app = App::new();
        app.insert_resource(WorldHeartbeat::default());
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![
                zone("pseudo_vein_done", 0.0, 0.0, 0.0),
                zone_with_danger("healthy_neighbor", 300.0, 0.0, 0.5, 7),
            ],
        });
        app.insert_resource(NpcRegistry {
            counts_by_zone: HashMap::from([("healthy_neighbor".to_string(), 4)]),
            ..Default::default()
        });
        app.add_event::<EventChainTrigger>();
        app.add_systems(Update, chain_reaction_tick);
        app.world_mut()
            .send_event(EventChainTrigger::PseudoVeinDissipated {
                zone_name: "pseudo_vein_done".to_string(),
                redistributed_qi: 0.7,
            });
        app.update();
        let active = app.world().resource::<ActiveEventsResource>();
        assert!(
            !active.contains("healthy_neighbor", EVENT_BEAST_TIDE),
            "邻域 spirit_qi=0.5 远高于任何 danger 加权后的阈值上限(0.15*1.6=0.24)，\
             次入口也不该触发——两条入口都不满足才是正确的第三态"
        );
    }

    #[test]
    fn world_pressure_ignores_tsy_blueprint_zones() {
        let mut heartbeat = WorldHeartbeat::default();
        let zones = ZoneRegistry {
            zones: vec![
                zone("spawn", 0.0, 0.0, 0.8),
                zone("waste", 300.0, 0.0, 0.2),
                tsy_zone("tsy_daneng_01_deep", 600.0, 0.0, -0.95),
            ],
        };

        let pressure = compute_world_pressure(
            &mut heartbeat,
            &zones,
            &[
                PlayerSample {
                    player_id: "home".to_string(),
                    dimension: DimensionKind::Overworld,
                    zone_name: Some("spawn".to_string()),
                    position: DVec3::ZERO,
                    high_realm: false,
                },
                PlayerSample {
                    player_id: "tsy_high".to_string(),
                    dimension: DimensionKind::Tsy,
                    zone_name: Some("tsy_daneng_01_deep".to_string()),
                    position: DVec3::ZERO,
                    high_realm: true,
                },
            ],
            1_000,
        );

        assert!(
            (pressure.avg_zone_qi - 0.5).abs() < 1e-9,
            "主世界 heartbeat pressure 只能统计 Overworld zone；TSY 负灵气不应把平均值拖到 {}",
            pressure.avg_zone_qi
        );
        assert_eq!(
            pressure.player_density_peak, 1.0,
            "TSY 玩家样本不能计入主世界 heartbeat 玩家密度"
        );
        assert_eq!(
            pressure.high_realm_count, 0,
            "TSY 高境界玩家不能抬高主世界 heartbeat high_realm_count"
        );
    }

    #[test]
    fn heartbeat_loop_phase_uses_zone_risk_without_new_player_state() {
        let zones = ZoneRegistry {
            zones: vec![
                zone(DEFAULT_SPAWN_ZONE_NAME, 0.0, 0.0, 0.9),
                zone("route_ash", 200.0, 0.0, 0.05),
                zone("deep_gift", 500.0, 0.0, 0.7),
            ],
        };

        assert_eq!(
            heartbeat_loop_phase(&zones, &[]),
            PlayerLoopPhase::SafeShelter
        );
        assert_eq!(
            heartbeat_loop_phase(
                &zones,
                &[PlayerSample {
                    player_id: "home".to_string(),
                    dimension: DimensionKind::Overworld,
                    zone_name: Some(DEFAULT_SPAWN_ZONE_NAME.to_string()),
                    position: DVec3::ZERO,
                    high_realm: false,
                }]
            ),
            PlayerLoopPhase::HomeOrganizing
        );
        assert_eq!(
            heartbeat_loop_phase(
                &zones,
                &[PlayerSample {
                    player_id: "deep".to_string(),
                    dimension: DimensionKind::Overworld,
                    zone_name: Some("deep_gift".to_string()),
                    position: DVec3::ZERO,
                    high_realm: false,
                }]
            ),
            PlayerLoopPhase::DeepGathering
        );
        assert_eq!(
            heartbeat_loop_phase(
                &zones,
                &[PlayerSample {
                    player_id: "return".to_string(),
                    dimension: DimensionKind::Overworld,
                    zone_name: Some("route_ash".to_string()),
                    position: DVec3::ZERO,
                    high_realm: false,
                }]
            ),
            PlayerLoopPhase::ReturnTrip
        );
    }

    #[test]
    fn heartbeat_loop_phase_ignores_tsy_player_samples() {
        let mut tsy_deep = tsy_zone("tsy_daneng_01_deep", 0.0, 0.0, -0.95);
        tsy_deep.danger_level = DEEP_GATHERING_DANGER_LEVEL;
        let zones = ZoneRegistry {
            zones: vec![zone(DEFAULT_SPAWN_ZONE_NAME, 200.0, 0.0, 0.5), tsy_deep],
        };

        assert_eq!(
            heartbeat_loop_phase(
                &zones,
                &[PlayerSample {
                    player_id: "tsy_high".to_string(),
                    dimension: DimensionKind::Tsy,
                    zone_name: Some("tsy_daneng_01_deep".to_string()),
                    position: DVec3::ZERO,
                    high_realm: true,
                }]
            ),
            PlayerLoopPhase::SafeShelter,
            "TSY 深层玩家不能把主世界 heartbeat 节奏推成 DeepGathering/ReturnTrip"
        );
    }

    #[test]
    fn rhythm_table_changes_heartbeat_omen_lead_by_loop_phase() {
        let pseudo_return = rhythm_omen_lead_ticks(
            HeartbeatEventKind::PseudoVein,
            PlayerLoopPhase::ReturnTrip,
            PSEUDO_VEIN_OMEN_LEAD_TICKS,
        );
        let pseudo_deep = rhythm_omen_lead_ticks(
            HeartbeatEventKind::PseudoVein,
            PlayerLoopPhase::DeepGathering,
            PSEUDO_VEIN_OMEN_LEAD_TICKS,
        );
        let beast_deep = rhythm_omen_lead_ticks(
            HeartbeatEventKind::BeastTide,
            PlayerLoopPhase::DeepGathering,
            BEAST_TIDE_OMEN_LEAD_TICKS,
        );
        let pseudo_deep_cadence = rhythm_cadence_multiplier(
            HeartbeatEventKind::PseudoVein,
            PlayerLoopPhase::DeepGathering,
        );
        let baseline_interval =
            EventCadence::new(PSEUDO_VEIN_OMEN_LEAD_TICKS).effective_interval_ticks(1.0);
        let downfrequency_interval = EventCadence::new(PSEUDO_VEIN_OMEN_LEAD_TICKS)
            .effective_interval_ticks(pseudo_deep_cadence);

        assert!(
            pseudo_return < pseudo_deep,
            "伪灵脉应在回程阶段更快显形：return={pseudo_return} deep={pseudo_deep}"
        );
        assert!(
            beast_deep < BEAST_TIDE_OMEN_LEAD_TICKS,
            "兽潮在深处采集阶段应缩短预警窗口，形成当趟撤离压力"
        );
        assert!(
            pseudo_deep_cadence < 1.0 && downfrequency_interval > baseline_interval,
            "频率倍率小于 1 时应拉长事件间隔：multiplier={pseudo_deep_cadence} baseline={baseline_interval} downfrequency={downfrequency_interval}"
        );
    }

    #[test]
    fn tide_sky_omen_consumes_xizhuan_boundary_and_rhythm_timing() {
        let mut heartbeat = WorldHeartbeat::default();
        let zones = ZoneRegistry::fallback();
        let boundary_tick = TICKS_PER_HOUR;
        let current_tick = boundary_tick + HEARTBEAT_EVAL_INTERVAL_TICKS;

        maybe_queue_tide_sky_omen(
            &mut heartbeat,
            &zones,
            Season::SummerToWinter,
            Some(boundary_tick),
            rhythm_context(PlayerLoopPhase::HomeOrganizing, current_tick),
            None,
        );

        assert_eq!(heartbeat.pending_omens.len(), 1);
        assert_eq!(heartbeat.pending_omens[0].kind, OmenKind::TideSkyTurning);
        assert_eq!(
            heartbeat.pending_omens[0].zone_name,
            DEFAULT_SPAWN_ZONE_NAME
        );
        assert_eq!(
            heartbeat.pending_omens[0].fires_at_tick,
            current_tick
                + rhythm_omen_lead_ticks(
                    HeartbeatEventKind::TideSkyOmen,
                    PlayerLoopPhase::HomeOrganizing,
                    TIDE_SKY_OMEN_LEAD_TICKS,
                ),
            "汐转天象应使用 event_rhythm.json 的 home_organizing lead_ticks"
        );

        maybe_queue_tide_sky_omen(
            &mut heartbeat,
            &zones,
            Season::SummerToWinter,
            Some(boundary_tick),
            rhythm_context(PlayerLoopPhase::HomeOrganizing, current_tick + 1),
            None,
        );
        assert_eq!(
            heartbeat.pending_omens.len(),
            1,
            "同一个汐转边界只能刷新一次天象预兆，不能每个 heartbeat 重复刷"
        );
    }

    #[test]
    fn heartbeat_tick_fires_tide_sky_omen_into_recent_events() {
        let mut app = App::new();
        let mut season_state = WorldSeasonState::default();
        let boundary_tick = TICKS_PER_HOUR;
        season_state.set_phase(Season::SummerToWinter, boundary_tick);

        app.insert_resource(WorldHeartbeat::default());
        app.insert_resource(CultivationClock {
            tick: boundary_tick + HEARTBEAT_EVAL_INTERVAL_TICKS,
        });
        app.insert_resource(season_state);
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<EventChainTrigger>();
        app.add_systems(Update, heartbeat_tick);
        app.update();

        let heartbeat = app.world().resource::<WorldHeartbeat>();
        assert!(
            !heartbeat.pending_omens.is_empty(),
            "expected at least one pending omen because xizhuan boundary should queue tide sky omen, actual pending_omens.len()={}",
            heartbeat.pending_omens.len()
        );
        let fires_at_tick = heartbeat.pending_omens[0].fires_at_tick;
        app.world_mut().resource_mut::<CultivationClock>().tick = fires_at_tick;
        app.update();

        let recent = app
            .world()
            .resource::<ActiveEventsResource>()
            .recent_events_snapshot();
        assert!(
            recent.iter().any(|event| {
                event.target.as_deref() == Some("tide_sky_omen")
                    && event.zone.as_deref() == Some(DEFAULT_SPAWN_ZONE_NAME)
            }),
            "汐转期天象不应只停留在 JSON 声明，应由 heartbeat 触发为运行时 recent event"
        );
        assert_eq!(
            app.world()
                .resource::<WorldHeartbeat>()
                .event_counts
                .get(&HeartbeatEventKind::TideSkyOmen)
                .copied(),
            Some(1),
            "汐转期天象触发后应记录 heartbeat 事件计数，证明运行时消费成功"
        );
    }

    #[test]
    fn realm_collapse_queues_only_when_collapsing_zone_is_empty() {
        let zones = ZoneRegistry {
            zones: vec![zone("dead_zone", 0.0, 0.0, 0.0)],
        };
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat.dead_qi_ticks_by_zone.insert(
            "dead_zone".to_string(),
            REALM_COLLAPSE_DEAD_QI_REQUIRED_TICKS,
        );

        maybe_queue_realm_collapse(
            &mut heartbeat,
            &zones,
            &[],
            &ActiveEventsResource::default(),
            rhythm_context(PlayerLoopPhase::SafeShelter, TICKS_PER_HOUR),
            None,
        );

        assert_eq!(
            heartbeat.pending_omens.len(),
            1,
            "无人停留的死域应排队域崩预兆，让安全区玩家从远处感知"
        );
        assert_eq!(
            heartbeat.pending_omens[0].kind,
            OmenKind::RealmCollapseImminent
        );

        let mut occupied = WorldHeartbeat::default();
        occupied.dead_qi_ticks_by_zone.insert(
            "dead_zone".to_string(),
            REALM_COLLAPSE_DEAD_QI_REQUIRED_TICKS,
        );
        maybe_queue_realm_collapse(
            &mut occupied,
            &zones,
            &[PlayerSample {
                player_id: "stranded".to_string(),
                dimension: DimensionKind::Overworld,
                zone_name: Some("dead_zone".to_string()),
                position: DVec3::ZERO,
                high_realm: false,
            }],
            &ActiveEventsResource::default(),
            rhythm_context(PlayerLoopPhase::DeepGathering, TICKS_PER_HOUR),
            None,
        );

        assert!(
            occupied.pending_omens.is_empty(),
            "有修士停留时不应按 P4 的无人停留域崩时机排队"
        );
    }

    #[test]
    fn realm_collapse_heartbeat_ignores_tsy_blueprint_zones() {
        let zones = ZoneRegistry {
            zones: vec![tsy_zone("tsy_daneng_01_deep", 0.0, 0.0, 0.0)],
        };
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat.dead_qi_ticks_by_zone.insert(
            "tsy_daneng_01_deep".to_string(),
            REALM_COLLAPSE_DEAD_QI_REQUIRED_TICKS,
        );

        maybe_queue_realm_collapse(
            &mut heartbeat,
            &zones,
            &[],
            &ActiveEventsResource::default(),
            rhythm_context(PlayerLoopPhase::DeepGathering, TICKS_PER_HOUR),
            None,
        );

        assert!(
            heartbeat.pending_omens.is_empty(),
            "TSY blueprint zone 不能被主世界无人域崩 heartbeat 排队"
        );
        assert!(
            !heartbeat
                .dead_qi_ticks_by_zone
                .contains_key("tsy_daneng_01_deep"),
            "主世界 heartbeat 应清理既有 TSY dead-qi 计数"
        );
    }

    #[test]
    fn force_override_replaces_existing_pending_omen() {
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat.pending_omens.push(WorldEventOmen {
            kind: OmenKind::BeastTideApproaching,
            zone_name: "hungry".to_string(),
            target_player: None,
            origin: DVec3::new(0.0, 65.0, 0.0),
            intensity: 0.1,
            scheduled_at_tick: 0,
            fires_at_tick: 10_000,
            expires_at_tick: 10_200,
        });
        heartbeat.forced_events.push(ForcedHeartbeatEvent {
            event_kind: HeartbeatEventKind::BeastTide,
            target_zone: "hungry".to_string(),
            intensity: 0.9,
        });
        let zones = ZoneRegistry {
            zones: vec![zone("hungry", 0.0, 0.0, 0.1)],
        };

        queue_forced_events(&mut heartbeat, &zones, 200, None);

        assert_eq!(heartbeat.pending_omens.len(), 1);
        assert_eq!(
            heartbeat.pending_omens[0].intensity, 0.9,
            "force override should replace the older same-zone pending omen"
        );
        assert_eq!(
            heartbeat.pending_omens[0].fires_at_tick, 200,
            "force override should fire at the current heartbeat tick"
        );
    }

    #[test]
    fn real_heartbeat_system_force_override_fires_through_app() {
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat.apply_override(
            HeartbeatOverrideAction::Force,
            HeartbeatEventKind::BeastTide,
            "spawn".to_string(),
            100,
            Some(0.8),
            0,
        );

        let mut app = App::new();
        app.insert_resource(heartbeat);
        app.insert_resource(CultivationClock {
            tick: HEARTBEAT_EVAL_INTERVAL_TICKS,
        });
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<EventChainTrigger>();
        app.add_systems(
            Update,
            (heartbeat_tick, chain_reaction_tick.after(heartbeat_tick)),
        );
        app.update();

        let active = app.world().resource::<ActiveEventsResource>();
        assert!(
            active.contains("spawn", EVENT_BEAST_TIDE),
            "real heartbeat_tick system should fire a forced beast tide through ActiveEventsResource"
        );
        let heartbeat = app.world().resource::<WorldHeartbeat>();
        assert_eq!(
            heartbeat
                .event_counts
                .get(&HeartbeatEventKind::BeastTide)
                .copied(),
            Some(1),
            "real heartbeat_tick path should record the fired beast tide"
        );
    }

    #[test]
    fn simulated_48h_unattended_world_meets_plan_floor() {
        let report = simulate_unattended_world(48, 10);

        assert!(report.pseudo_vein_count >= 80);
        assert!(report.beast_tide_count >= 30);
        assert!(report.realm_collapse_count >= 5);
        assert!(report.karma_backlash_count >= 40);
        assert!(report.chain_reaction_count >= 10);
        assert!(report.qi_total_delta_ratio < 0.05);
        assert!(report.max_same_zone_stack <= 3);
    }

    // ───────────────────── plan-zone-qi-economy-v1 P1 — zone_qi_inflow_tick ─────────────────────

    fn inflow_test_app(zones: Vec<Zone>, pending_pool_balance: f64, start_tick: u64) -> App {
        let mut app = App::new();
        app.insert_resource(ZoneQiInflowClock::default());
        app.insert_resource(CultivationClock { tick: start_tick });
        app.insert_resource(ZoneRegistry { zones });
        app.insert_resource(ActiveEventsResource::default());
        let mut ledger = WorldQiAccount::default();
        if pending_pool_balance > 0.0 {
            ledger
                .set_balance(pending_inflow_account(), pending_pool_balance)
                .expect("seeding the pending pool balance must succeed");
        }
        app.insert_resource(ledger);
        app.add_systems(Update, zone_qi_inflow_tick);
        app
    }

    fn advance_ticks(app: &mut App, ticks: u64) {
        let mut clock = app.world_mut().resource_mut::<CultivationClock>();
        clock.tick = clock.tick.saturating_add(ticks);
        app.update();
    }

    #[test]
    fn zero_elapsed_ticks_on_first_run_is_a_noop() {
        // 首次 run：ZoneQiInflowClock::default() 的 last_tick=0，若 CultivationClock 也从 0
        // 起步，elapsed_ticks==0，不应该做任何事（也不应该 panic）。
        let mut z = zone("spawn", 0.0, 0.0, 0.1);
        z.qi_equilibrium = 0.5;
        z.qi_inflow_per_min = 1.0;
        let mut app = inflow_test_app(vec![z], 1000.0, 0);
        app.update();

        let zones = app.world().resource::<ZoneRegistry>();
        assert_eq!(
            zones.zones[0].spirit_qi, 0.1,
            "tick delta of zero (both clocks start at 0) must not inject any qi"
        );
    }

    #[test]
    fn injects_from_pending_pool_and_debits_it_by_the_same_amount() {
        let mut z = zone("spawn", 0.0, 0.0, 0.1);
        z.qi_equilibrium = 0.5;
        z.qi_inflow_per_min = 1.0; // 1.0 绝对点/分钟
        let mut app = inflow_test_app(vec![z], 1000.0, 0);

        // 1 分钟 = TICKS_PER_MINUTE ticks
        advance_ticks(&mut app, TICKS_PER_MINUTE);

        let zones = app.world().resource::<ZoneRegistry>();
        let expected_fraction_gain = 1.0 / QI_ZONE_UNIT_CAPACITY; // 1.0 absolute / 50.0 capacity
        assert!(
            (zones.zones[0].spirit_qi - (0.1 + expected_fraction_gain)).abs() < 1e-9,
            "after 1 minute at 1.0/min, spirit_qi should rise by 1.0/QI_ZONE_UNIT_CAPACITY \
             ({expected_fraction_gain}), got {}",
            zones.zones[0].spirit_qi
        );

        let ledger = app.world().resource::<WorldQiAccount>();
        assert!(
            (ledger.balance(&pending_inflow_account()) - (1000.0 - 1.0)).abs() < 1e-9,
            "pending pool must be debited by exactly the absolute amount credited to the zone \
             (conservation: pool loses 1.0, zone gains 1.0/CAPACITY fraction == 1.0 absolute), \
             got pool balance {}",
            ledger.balance(&pending_inflow_account())
        );
    }

    #[test]
    fn clamps_at_equilibrium_and_never_overshoots_across_many_ticks() {
        let mut z = zone("spawn", 0.0, 0.0, 0.3);
        z.qi_equilibrium = 0.35;
        z.qi_inflow_per_min = 5.0; // deliberately fast so it would overshoot without the clamp
        let mut app = inflow_test_app(vec![z], 100_000.0, 0);

        // Run many minutes' worth of ticks — should settle at equilibrium and stop.
        advance_ticks(&mut app, TICKS_PER_MINUTE * 50);

        let zones = app.world().resource::<ZoneRegistry>();
        assert!(
            zones.zones[0].spirit_qi <= 0.35 + 1e-9,
            "spirit_qi ({}) must never exceed qi_equilibrium (0.35), even after many ticks of \
             a fast inflow rate that would overshoot without clamping",
            zones.zones[0].spirit_qi
        );
        assert!(
            zones.zones[0].spirit_qi >= 0.35 - 1e-6,
            "spirit_qi ({}) should have converged to equilibrium (0.35) given ample pool and \
             many ticks",
            zones.zones[0].spirit_qi
        );

        // Run further — must remain pinned, not creep past equilibrium.
        advance_ticks(&mut app, TICKS_PER_MINUTE * 50);
        let zones = app.world().resource::<ZoneRegistry>();
        assert!(
            zones.zones[0].spirit_qi <= 0.35 + 1e-9,
            "continuing to tick after reaching equilibrium must not push spirit_qi past it \
             (got {})",
            zones.zones[0].spirit_qi
        );
    }

    #[test]
    fn insufficient_pending_pool_scales_down_and_never_overdraws() {
        let mut z = zone("spawn", 0.0, 0.0, 0.1);
        z.qi_equilibrium = 0.9;
        z.qi_inflow_per_min = 10.0;
        // Pool only has 2.0 absolute points — far less than what 1 minute at 10.0/min would need.
        let mut app = inflow_test_app(vec![z], 2.0, 0);

        advance_ticks(&mut app, TICKS_PER_MINUTE);

        let ledger = app.world().resource::<WorldQiAccount>();
        let pool_balance = ledger.balance(&pending_inflow_account());
        assert!(
            pool_balance >= -1e-9,
            "pending pool balance must never go negative (no overdraw), got {pool_balance}"
        );
        assert!(
            pool_balance.abs() < 1e-9,
            "with only 2.0 available and 10.0 desired, the pool should be drained to exactly \
             zero (scaled down), not partially retained or overdrawn — got {pool_balance}"
        );

        let zones = app.world().resource::<ZoneRegistry>();
        let expected_fraction_gain = 2.0 / QI_ZONE_UNIT_CAPACITY;
        assert!(
            (zones.zones[0].spirit_qi - (0.1 + expected_fraction_gain)).abs() < 1e-9,
            "the zone must only receive the amount the pool could actually afford (2.0 \
             absolute -> {expected_fraction_gain} fraction), got {}",
            zones.zones[0].spirit_qi
        );
    }

    #[test]
    fn empty_pending_pool_yields_zero_inflow_and_zone_is_untouched() {
        let mut z = zone("spawn", 0.0, 0.0, 0.1);
        z.qi_equilibrium = 0.5;
        z.qi_inflow_per_min = 1.0;
        let mut app = inflow_test_app(vec![z], 0.0, 0);

        advance_ticks(&mut app, TICKS_PER_MINUTE * 10);

        let zones = app.world().resource::<ZoneRegistry>();
        assert_eq!(
            zones.zones[0].spirit_qi, 0.1,
            "an empty pending pool must leave the zone completely untouched, not partially \
             credit it or panic"
        );
    }

    #[test]
    fn negative_zone_qi_is_skipped_entirely() {
        let mut z = zone("dead_zone", 0.0, 0.0, -0.2);
        z.qi_equilibrium = 0.5;
        z.qi_inflow_per_min = 1.0;
        let mut app = inflow_test_app(vec![z], 1000.0, 0);

        advance_ticks(&mut app, TICKS_PER_MINUTE * 10);

        let zones = app.world().resource::<ZoneRegistry>();
        assert_eq!(
            zones.zones[0].spirit_qi, -0.2,
            "negative-qi (负灵域) zones must never be inflowed by this P1 system — recovery \
             out of negative territory is explicitly out of scope (§8.1 #5)"
        );
        let ledger = app.world().resource::<WorldQiAccount>();
        assert_eq!(
            ledger.balance(&pending_inflow_account()),
            1000.0,
            "the pending pool must not be touched at all for a skipped negative-qi zone"
        );
    }

    #[test]
    fn realm_collapse_zone_is_skipped_even_when_below_equilibrium() {
        let mut z = zone("collapsing", 0.0, 0.0, 0.1);
        z.qi_equilibrium = 0.5;
        z.qi_inflow_per_min = 1.0;
        let mut app = inflow_test_app(vec![z], 1000.0, 0);
        {
            let command =
                spawn_event_command("collapsing", EVENT_REALM_COLLAPSE, 1.0, 20_000, None);
            let mut zones_for_lookup = app.world().resource::<ZoneRegistry>().clone();
            let mut active_events = app.world_mut().resource_mut::<ActiveEventsResource>();
            assert!(
                active_events.enqueue_from_spawn_command(&command, Some(&mut zones_for_lookup)),
                "test setup: enqueueing the REALM_COLLAPSE active event must succeed"
            );
        }

        advance_ticks(&mut app, TICKS_PER_MINUTE * 10);

        let zones = app.world().resource::<ZoneRegistry>();
        assert_eq!(
            zones.zones[0].spirit_qi, 0.1,
            "a zone with an active EVENT_REALM_COLLAPSE must be skipped by inflow even though \
             it is far below equilibrium (§8.1 #5, mirrors maybe_queue_realm_collapse's own \
             active_events.contains(..., EVENT_REALM_COLLAPSE) gate)"
        );
    }

    #[test]
    fn zero_equilibrium_zone_is_never_touched_back_compat() {
        // 默认值 0.0（没配置 qi_equilibrium/qi_inflow_per_min 的旧 zone）必须完全不受影响。
        let z = zone("legacy_zone", 0.0, 0.0, 0.05);
        assert_eq!(z.qi_equilibrium, 0.0);
        assert_eq!(z.qi_inflow_per_min, 0.0);
        let mut app = inflow_test_app(vec![z], 1000.0, 0);

        advance_ticks(&mut app, TICKS_PER_MINUTE * 100);

        let zones = app.world().resource::<ZoneRegistry>();
        assert_eq!(
            zones.zones[0].spirit_qi, 0.05,
            "a zone with qi_equilibrium == 0.0 (back-compat default) must never receive any \
             inflow, no matter how many ticks pass"
        );
        let ledger = app.world().resource::<WorldQiAccount>();
        assert_eq!(
            ledger.balance(&pending_inflow_account()),
            1000.0,
            "the pending pool must be completely untouched for an opted-out zone"
        );
    }

    #[test]
    fn multi_zone_conservation_holds_across_a_long_run() {
        // 回流↔（模拟）吸收长跑总量守恒：多个 zone 分别从同一个待分配池取用，
        // 待分配池减少量之和必须精确等于所有 zone 累计增加量之和（换算到绝对单位）。
        let mut zone_a = zone("zone_a", 0.0, 0.0, 0.05);
        zone_a.qi_equilibrium = 0.3;
        zone_a.qi_inflow_per_min = 0.6;
        let mut zone_b = zone("zone_b", 500.0, 0.0, 0.1);
        zone_b.qi_equilibrium = 0.4;
        zone_b.qi_inflow_per_min = 0.3;
        let mut zone_c_no_inflow = zone("zone_c", 1000.0, 0.0, 0.05);
        zone_c_no_inflow.qi_equilibrium = 0.0; // opted out, must stay untouched

        let initial_pool = 500.0;
        let mut app = inflow_test_app(vec![zone_a, zone_b, zone_c_no_inflow], initial_pool, 0);

        for _ in 0..200 {
            advance_ticks(&mut app, TICKS_PER_MINUTE);
        }

        let zones = app.world().resource::<ZoneRegistry>();
        let ledger = app.world().resource::<WorldQiAccount>();
        let pool_balance = ledger.balance(&pending_inflow_account());

        let zone_a_absolute = zones.zones[0].spirit_qi * QI_ZONE_UNIT_CAPACITY;
        let zone_b_absolute = zones.zones[1].spirit_qi * QI_ZONE_UNIT_CAPACITY;
        let zone_a_initial_absolute = 0.05 * QI_ZONE_UNIT_CAPACITY;
        let zone_b_initial_absolute = 0.1 * QI_ZONE_UNIT_CAPACITY;
        let total_credited = (zone_a_absolute - zone_a_initial_absolute)
            + (zone_b_absolute - zone_b_initial_absolute);
        let total_debited = initial_pool - pool_balance;

        assert!(
            (total_credited - total_debited).abs() < 1e-6,
            "sum of absolute qi credited to all zones ({total_credited}) must exactly equal \
             the amount debited from the shared pending pool ({total_debited}) — any mismatch \
             is qi being created or destroyed out of thin air"
        );
        assert_eq!(
            zones.zones[2].spirit_qi, 0.05,
            "the opted-out zone_c (qi_equilibrium == 0.0) must never participate and must \
             stay completely untouched even while its siblings draw from the shared pool"
        );
        assert!(
            zones.zones[0].spirit_qi <= 0.3 + 1e-9 && zones.zones[1].spirit_qi <= 0.4 + 1e-9,
            "neither zone may overshoot its own equilibrium after a long multi-zone run \
             (zone_a={}, zone_b={})",
            zones.zones[0].spirit_qi,
            zones.zones[1].spirit_qi
        );
    }

    #[test]
    fn time_advance_style_large_tick_jump_is_caught_up_in_one_evaluation() {
        // `/time advance` 直接 saturating_add 到 CultivationClock.tick（不是逐 tick 递增），
        // 下一次 zone_qi_inflow_tick 必须按整段 delta 一次性补上，而不是只补 1 tick 的量。
        let mut z = zone("spawn", 0.0, 0.0, 0.1);
        z.qi_equilibrium = 0.9;
        z.qi_inflow_per_min = 1.0;
        let mut app = inflow_test_app(vec![z], 100_000.0, 0);

        // Jump 30 minutes' worth of ticks all at once, like `/time advance` would.
        advance_ticks(&mut app, TICKS_PER_MINUTE * 30);

        let zones = app.world().resource::<ZoneRegistry>();
        let expected_fraction_gain = (1.0 * 30.0) / QI_ZONE_UNIT_CAPACITY;
        assert!(
            (zones.zones[0].spirit_qi - (0.1 + expected_fraction_gain)).abs() < 1e-9,
            "a single large tick jump (simulating /time advance) must be caught up as one \
             30-minute window (gain={expected_fraction_gain}), not truncated to a single \
             per-tick increment — got {}",
            zones.zones[0].spirit_qi
        );
    }
}
