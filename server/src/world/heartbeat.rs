use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use serde_json::{json, Value};
use valence::prelude::{
    bevy_ecs, App, Client, Component, DVec3, Event, EventReader, EventWriter, Events,
    IntoSystemConfigs, Position, PostUpdate, Query, Res, ResMut, Resource, Update, Username, With,
};

use crate::cultivation::breakthrough::BreakthroughOutcome;
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::tick::CultivationClock;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::lifecycle::NpcRegistry;
use crate::npc::spawn::ambient_scheduler::{danger_tide_required_ticks_scale, danger_tide_weight};
use crate::persistence::HeartbeatPseudoVeinRecord;
use crate::player::state::canonical_player_id;
#[cfg(test)]
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
#[cfg(test)]
use crate::qi_physics::QiAccountId;
use crate::qi_physics::{
    pending_inflow_account, transfer_ledger_qi_to_zone, zone_equilibrium_inflow, QiTransfer,
    QiTransferReason, WorldQiAccount,
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
use crate::world::pseudo_vein_runtime::{
    inject_zone_for_pseudo_vein_target, settle_ephemeral_pseudo_vein_zone,
    settle_ephemeral_pseudo_vein_zone_to_target,
};
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
pub(crate) const HEARTBEAT_PSEUDO_VEIN_ZONE_PREFIX: &str = "pseudo_vein_heartbeat_";
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
    restored_eval_elapsed_ticks: u64,
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
            restored_eval_elapsed_ticks: 0,
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

    /// `zones_runtime` 是跨重启的物理余额权威；生命周期记录只保存时钟与阶段元数据。
    /// 两张表恢复完毕后，用 zone 余额校准 state，避免旧版本遗留的不一致继续传播。
    pub(crate) fn sync_active_pseudo_vein_qi_from_zones(&mut self, zones: &ZoneRegistry) {
        for (zone_id, state) in &mut self.active_pseudo_veins {
            let Some(zone) = zones.find_zone_by_name(zone_id.as_str()) else {
                continue;
            };
            state.qi_current = zone.spirit_qi.clamp(0.0, 1.0);
        }
    }

    pub(crate) fn active_pseudo_vein_records(
        &self,
        zone_registry: &ZoneRegistry,
    ) -> Vec<HeartbeatPseudoVeinRecord> {
        let current_tick = self
            .active_pseudo_veins
            .values()
            .map(PseudoVeinRuntimeState::last_observed_raw_tick)
            .max()
            .unwrap_or(self.last_eval_tick);
        self.active_pseudo_vein_records_at_tick(zone_registry, current_tick)
    }

    pub(crate) fn active_pseudo_vein_records_at_tick(
        &self,
        zone_registry: &ZoneRegistry,
        current_tick: u64,
    ) -> Vec<HeartbeatPseudoVeinRecord> {
        let eval_elapsed_ticks = self.eval_elapsed_ticks(current_tick);
        let mut records = self
            .active_pseudo_veins
            .iter()
            .filter_map(|(zone_id, state)| {
                if state.dissipated {
                    return None;
                }
                let zone = zone_registry.find_zone_by_name(zone_id.as_str())?;
                let (min, max) = zone.bounds;
                let timing = state.persistence_timing(current_tick);
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
                    // zone 是物理余额权威；即使外部系统在两个 heartbeat eval 之间改变
                    // spirit_qi，原子快照里的 lifecycle 与 zones_runtime 也必须写同一值。
                    qi_current: zone.spirit_qi.clamp(0.0, 1.0),
                    total_qi_consumed: state.total_qi_consumed,
                    warning_sent: state.warning_sent,
                    dissipated: state.dissipated,
                    season_at_spawn: state.season_at_spawn,
                    observed_age_ticks: timing.observed_age_ticks,
                    pending_runtime_ticks: timing.pending_runtime_ticks,
                    pending_offline_ticks: timing.pending_offline_ticks,
                    occupant_count: timing.occupant_count,
                    eval_elapsed_ticks,
                    snapshot_wall: 0,
                })
            })
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.zone_id.cmp(&right.zone_id));
        records
    }

    pub(crate) fn eval_elapsed_ticks(&self, current_tick: u64) -> u64 {
        current_tick
            .saturating_sub(self.last_eval_tick)
            .saturating_add(self.restored_eval_elapsed_ticks)
    }

    fn restore_eval_elapsed_ticks(&mut self, current_tick: u64, elapsed_ticks: u64) {
        self.last_eval_tick = current_tick;
        self.restored_eval_elapsed_ticks = self.restored_eval_elapsed_ticks.max(elapsed_ticks);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn restore_pseudo_vein_records(
        &mut self,
        zone_registry: &mut ZoneRegistry,
        records: &[HeartbeatPseudoVeinRecord],
        current_tick: u64,
    ) -> usize {
        self.restore_pseudo_vein_records_at_wall(zone_registry, records, current_tick, 0)
    }

    pub(crate) fn restore_pseudo_vein_records_at_wall(
        &mut self,
        zone_registry: &mut ZoneRegistry,
        records: &[HeartbeatPseudoVeinRecord],
        current_tick: u64,
        current_wall: i64,
    ) -> usize {
        let mut restored = 0;
        for record in records {
            if let Err(error) = validate_persisted_pseudo_vein_record(record) {
                tracing::warn!(
                    "[bong][heartbeat] skipped invalid persisted pseudo-vein `{}`: {error}",
                    record.zone_id
                );
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
                    // 真元余额由同一事务里的 zones_runtime 行在随后 hydration 覆盖；这里
                    // 只先重建动态 zone 拓扑，不能把 lifecycle qi_current 当新余额创生。
                    spirit_qi: 0.0,
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

            let offline_ticks = wall_elapsed_ticks(record.snapshot_wall, current_wall);
            let mut state = PseudoVeinRuntimeState::restored_with_pending_elapsed(
                record.zone_id.clone(),
                record.center_xz,
                current_tick,
                record.observed_age_ticks,
                record.pending_runtime_ticks,
                record.pending_offline_ticks,
                offline_ticks,
                record.occupant_count,
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
            self.restore_eval_elapsed_ticks(
                current_tick,
                record.eval_elapsed_ticks.saturating_add(offline_ticks),
            );
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

pub(crate) fn is_heartbeat_pseudo_vein_zone_id(zone_id: &str) -> bool {
    heartbeat_pseudo_vein_index(zone_id).is_some()
}

pub(crate) fn is_heartbeat_pseudo_vein_zone_namespace(zone_id: &str) -> bool {
    zone_id.starts_with(HEARTBEAT_PSEUDO_VEIN_ZONE_PREFIX)
}

pub(crate) fn validate_persisted_pseudo_vein_record(
    record: &HeartbeatPseudoVeinRecord,
) -> Result<(), String> {
    if !is_heartbeat_pseudo_vein_zone_id(record.zone_id.as_str()) {
        return Err(format!(
            "zone_id must match {HEARTBEAT_PSEUDO_VEIN_ZONE_PREFIX}<u64>"
        ));
    }
    if record.dissipated {
        return Err("dissipated lifecycle rows must not remain persisted".to_string());
    }
    if !finite_array3(record.bounds_min) || !finite_array3(record.bounds_max) {
        return Err("bounds must contain only finite coordinates".to_string());
    }
    if record.bounds_min[0] > record.bounds_max[0]
        || record.bounds_min[1] > record.bounds_max[1]
        || record.bounds_min[2] > record.bounds_max[2]
    {
        return Err("bounds min must not exceed bounds max".to_string());
    }
    if !finite_array2(record.center_xz)
        || !(record.bounds_min[0]..=record.bounds_max[0]).contains(&record.center_xz[0])
        || !(record.bounds_min[2]..=record.bounds_max[2]).contains(&record.center_xz[1])
    {
        return Err("center_xz must be finite and lie within persisted bounds".to_string());
    }
    if record
        .patrol_anchors
        .iter()
        .any(|anchor| !finite_array3(*anchor))
    {
        return Err("patrol anchors must contain only finite coordinates".to_string());
    }
    if !record.qi_current.is_finite() || !(0.0..=1.0).contains(&record.qi_current) {
        return Err(format!(
            "qi_current must be finite within [0, 1], actual {}",
            record.qi_current
        ));
    }
    if !record.total_qi_consumed.is_finite() || record.total_qi_consumed < 0.0 {
        return Err(format!(
            "total_qi_consumed must be finite and non-negative, actual {}",
            record.total_qi_consumed
        ));
    }
    if record.last_tick < record.spawned_at_tick {
        return Err(format!(
            "last_tick {} precedes spawned_at_tick {}",
            record.last_tick, record.spawned_at_tick
        ));
    }
    if record.snapshot_wall < 0 {
        return Err(format!(
            "snapshot_wall must be non-negative, actual {}",
            record.snapshot_wall
        ));
    }
    Ok(())
}

fn wall_elapsed_ticks(snapshot_wall: i64, current_wall: i64) -> u64 {
    let elapsed_seconds = current_wall.saturating_sub(snapshot_wall).max(0);
    u64::try_from(elapsed_seconds)
        .unwrap_or(u64::MAX)
        .saturating_mul(TICKS_PER_SECOND)
}

fn heartbeat_pseudo_vein_index(zone_id: &str) -> Option<u64> {
    zone_id
        .strip_prefix(HEARTBEAT_PSEUDO_VEIN_ZONE_PREFIX)
        .and_then(|suffix| suffix.parse::<u64>().ok())
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering world heartbeat scheduler");
    app.insert_resource(WorldHeartbeat::default());
    app.init_resource::<ZoneQiInflowClock>();
    app.init_resource::<WorldQiAccount>();
    app.add_event::<EventChainTrigger>();
    // qi_physics::register（生产路径）负责唯一一次 add_event/update；这里只预置资源，
    // 让单独注册 heartbeat 的 headless App 也能满足 EventWriter<QiTransfer> 参数。
    app.init_resource::<Events<QiTransfer>>();
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
    // heartbeat 事件评估每 200 tick 一次，但修炼/释放等系统会逐 tick 改 zone。
    // PostUpdate 只把物理 zone 同步进 lifecycle；账本必须由真实 QiTransfer 路径维护，
    // 不能在帧末用 set_balance 掩盖未记账流动。持久化本身直接读 zone，双表仍同值。
    app.add_systems(PostUpdate, sync_active_pseudo_vein_state_system);
}

pub(crate) fn sync_active_pseudo_vein_state_system(
    mut heartbeat: ResMut<WorldHeartbeat>,
    zones: Option<Res<ZoneRegistry>>,
) {
    let Some(zones) = zones.as_deref() else {
        return;
    };
    heartbeat.sync_active_pseudo_vein_qi_from_zones(zones);
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
    mut qi_ledger: ResMut<WorldQiAccount>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    players: Query<PlayerSampleQueryItem, With<Client>>,
    mut chain_triggers: EventWriter<EventChainTrigger>,
    mut qi_transfers: EventWriter<QiTransfer>,
) {
    let current_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_else(|| {
        heartbeat
            .last_eval_tick
            .saturating_add(heartbeat.eval_interval_ticks)
    });
    if heartbeat.eval_elapsed_ticks(current_tick) < heartbeat.eval_interval_ticks {
        return;
    }
    heartbeat.last_eval_tick = current_tick;
    heartbeat.restored_eval_elapsed_ticks = 0;
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
        &mut qi_ledger,
        &mut qi_transfers,
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
        &mut qi_ledger,
        &mut qi_transfers,
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

#[allow(clippy::too_many_arguments)]
fn advance_active_pseudo_veins(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &mut ZoneRegistry,
    player_samples: &[PlayerSample],
    current_tick: u64,
    chain_triggers: &mut EventWriter<EventChainTrigger>,
    qi_ledger: &mut WorldQiAccount,
    qi_transfers: &mut EventWriter<QiTransfer>,
    mut vfx_events: Option<&mut Events<VfxEventRequest>>,
    active_events: &mut ActiveEventsResource,
) {
    let mut dissipated = Vec::new();
    for (zone_name, state) in &mut heartbeat.active_pseudo_veins {
        let Some(zone) = zone_registry.find_zone_mut(zone_name.as_str()) else {
            tracing::warn!(
                "[bong][heartbeat] dynamic pseudo-vein state has no zone {}; retaining runtime",
                zone_name
            );
            continue;
        };
        // zone 是玩家吸收、释放及持久化共同读取的物理余额权威。每次推进前先校准
        // lifecycle，再把本 tick 的衰减通过 PseudoVeinSettle 真实归还 pending pool。
        state.qi_current = zone.spirit_qi.clamp(0.0, 1.0);
        let state_before_advance = state.clone();
        let occupants = player_samples
            .iter()
            .filter(|sample| sample.zone_name.as_deref() == Some(zone_name.as_str()))
            .map(|sample| sample.player_id.clone())
            .collect::<Vec<_>>();
        let advance = state.advance(current_tick, occupants);
        match settle_ephemeral_pseudo_vein_zone_to_target(zone, qi_ledger, state.qi_current) {
            Ok(Some(transfer)) => {
                qi_transfers.send(transfer);
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    "[bong][heartbeat] failed to apply dynamic pseudo-vein decay zone={}: {error}",
                    zone_name
                );
                // 账本未落地时回滚 lifecycle，避免 warning/dissipated 先走而物理余额未动。
                *state = state_before_advance;
                continue;
            }
        }
        state.qi_current = zone.spirit_qi.clamp(0.0, 1.0);
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
        if state.dissipated {
            dissipated.push(zone_name.clone());
        }
    }

    for zone_name in dissipated {
        if let Some(zone) = zone_registry.find_zone_mut(zone_name.as_str()) {
            match settle_ephemeral_pseudo_vein_zone(zone, qi_ledger) {
                Ok(Some(transfer)) => {
                    qi_transfers.send(transfer);
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        "[bong][heartbeat] failed to settle dynamic pseudo-vein zone {} before removal: {error}",
                        zone_name
                    );
                    // 保留 runtime，下一 heartbeat tick 重试；绝不能删 zone 吞掉余额。
                    continue;
                }
            }
        }
        heartbeat.active_pseudo_veins.remove(&zone_name);
        if let Some(zone) = zone_registry.find_zone_mut(zone_name.as_str()) {
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

#[allow(clippy::too_many_arguments)]
fn fire_due_omens(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &mut ZoneRegistry,
    active_events: &mut ActiveEventsResource,
    chain_triggers: &mut EventWriter<EventChainTrigger>,
    sources: HeartbeatEventSources<'_>,
    qi_ledger: &mut WorldQiAccount,
    qi_transfers: &mut EventWriter<QiTransfer>,
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
                if let Some(transfer) = spawn_pseudo_vein_from_omen(
                    heartbeat,
                    zone_registry,
                    active_events,
                    qi_ledger,
                    &omen,
                    season,
                    current_tick,
                ) {
                    qi_transfers.send(transfer);
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
    qi_ledger: &mut WorldQiAccount,
    omen: &WorldEventOmen,
    season: Season,
    current_tick: u64,
) -> Option<QiTransfer> {
    if heartbeat.active_pseudo_veins.len() >= PSEUDO_VEIN_ACTIVE_CAP {
        return None;
    }
    let anchor_zone = zone_registry
        .find_zone_by_name(omen.zone_name.as_str())
        .cloned()?;
    if anchor_zone.dimension != DimensionKind::Overworld {
        return None;
    }
    let id = format!("heartbeat_{}", heartbeat.next_pseudo_vein_index);
    heartbeat.next_pseudo_vein_index = heartbeat.next_pseudo_vein_index.saturating_add(1);
    let zone_name = pseudo_vein_zone_name(id.as_str()).ok()?;
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
        spirit_qi: 0.0,
        danger_level: PSEUDO_VEIN_DANGER_LEVEL,
        active_events: vec![EVENT_PSEUDO_VEIN.to_string()],
        patrol_anchors: vec![center],
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    };
    if zone_registry.register_runtime_zone(zone).is_err() {
        return None;
    }
    let transfer = zone_registry
        .find_zone_mut(zone_name.as_str())
        .and_then(|zone| inject_zone_for_pseudo_vein_target(zone, qi_ledger, omen.intensity));
    if transfer.is_none() {
        remove_runtime_pseudo_vein_zone(zone_registry, zone_name.as_str());
    }
    let transfer = transfer?;
    let actual_spirit_qi = zone_registry
        .find_zone_by_name(zone_name.as_str())
        .map(|zone| zone.spirit_qi)
        .unwrap_or_default();
    let mut state = PseudoVeinRuntimeState::new(
        zone_name.clone(),
        [center.x, center.z],
        current_tick,
        pseudo_vein_season(season),
    );
    state.qi_current = actual_spirit_qi;
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
            ("spirit_qi".to_string(), json!(actual_spirit_qi)),
            ("autonomous".to_string(), Value::Bool(true)),
        ])),
    });
    Some(transfer)
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
    if before != zone_registry.zones.len() {
        // fix-spec-1901-v2 §7.1 — 空间变化（删除 zone）递增 revision。
        zone_registry.spatial_revision = zone_registry.spatial_revision.wrapping_add(1);
    }
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
/// 事务入口直接借记稳定待分配池、增加外部 Zone owner 并追加 audit；不创建长期
/// `zone:*` 镜像余额，避免 `summarize_world_qi` 把同一份区域真元重复计算。
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

        if transfer_ledger_qi_to_zone(
            ledger,
            pool,
            zone.name.as_str(),
            &mut zone.spirit_qi,
            actual_absolute,
            zone.qi_equilibrium,
            QiTransferReason::ZoneInflow,
        )
        .is_err()
        {
            continue;
        }
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
#[path = "heartbeat_tests.rs"]
mod tests;
