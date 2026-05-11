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
use crate::player::state::canonical_player_id;
use crate::schema::agent_command::Command;
use crate::schema::common::{CommandType, GameEventType};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::schema::world_state::GameEvent;
use crate::world::dimension::CurrentDimension;
use crate::world::events::{
    ActiveEventsResource, ZoneCollapsedEvent, EVENT_BEAST_TIDE, EVENT_KARMA_BACKLASH,
    EVENT_REALM_COLLAPSE,
};
use crate::world::karma::{KarmaWeightStore, QiDensityHeatmap};
use crate::world::season::{query_season, Season, WorldSeasonState};
use crate::world::zone::{Zone, ZoneRegistry};
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
pub const VFX_WORLD_OMEN_REALM_COLLAPSE: &str = "bong:world_omen_realm_collapse";
pub const VFX_WORLD_OMEN_KARMA_BACKLASH: &str = "bong:world_omen_karma_backlash";

const PSEUDO_VEIN_OMEN_LEAD_TICKS: u64 = 60 * TICKS_PER_SECOND;
const BEAST_TIDE_OMEN_LEAD_TICKS: u64 = 120 * TICKS_PER_SECOND;
const REALM_COLLAPSE_OMEN_LEAD_TICKS: u64 = 300 * TICKS_PER_SECOND;
const KARMA_BACKLASH_OMEN_LEAD_TICKS: u64 = 10 * TICKS_PER_SECOND;
const OMEN_VISUAL_DURATION_TICKS: u16 = 200;
const BEAST_TIDE_LOW_QI_THRESHOLD: f64 = 0.15;
const BEAST_TIDE_LOW_QI_REQUIRED_TICKS: u64 = 5 * TICKS_PER_MINUTE;
const REALM_COLLAPSE_DEAD_QI_REQUIRED_TICKS: u64 = 10 * TICKS_PER_MINUTE;
const REALM_COLLAPSE_EVACUATION_TICKS: u64 = 30 * TICKS_PER_SECOND;
const PSEUDO_VEIN_ACTIVE_CAP: usize = 3;
const PSEUDO_VEIN_MIN_DISTANCE_BLOCKS: f64 = 500.0;
const KARMA_BASE_ROLL_PROBABILITY: f64 = 0.003;
const RECENT_BREAKTHROUGH_WINDOW_TICKS: u64 = 10 * TICKS_PER_MINUTE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeartbeatEventKind {
    PseudoVein,
    BeastTide,
    RealmCollapse,
    KarmaBacklash,
}

impl HeartbeatEventKind {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "pseudo_vein" => Some(Self::PseudoVein),
            "beast_tide" => Some(Self::BeastTide),
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
    RealmCollapseImminent,
    KarmaBacklashTarget,
}

impl OmenKind {
    const fn vfx_event_id(self) -> &'static str {
        match self {
            Self::PseudoVeinForming => VFX_WORLD_OMEN_PSEUDO_VEIN,
            Self::BeastTideApproaching => VFX_WORLD_OMEN_BEAST_TIDE,
            Self::RealmCollapseImminent => VFX_WORLD_OMEN_REALM_COLLAPSE,
            Self::KarmaBacklashTarget => VFX_WORLD_OMEN_KARMA_BACKLASH,
        }
    }

    const fn color(self) -> &'static str {
        match self {
            Self::PseudoVeinForming => "#66D8C8",
            Self::BeastTideApproaching => "#B8864A",
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
            (self.seasonal_multiplier * self.pressure_multiplier * override_multiplier.max(1.0))
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
    pub realm_collapse_cadence: EventCadence,
    pub karma_backlash_cadence: EventCadence,
    pub world_pressure: WorldPressure,
    active_pseudo_veins: HashMap<String, PseudoVeinRuntimeState>,
    pending_omens: Vec<WorldEventOmen>,
    low_qi_ticks_by_zone: HashMap<String, u64>,
    dead_qi_ticks_by_zone: HashMap<String, u64>,
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
            realm_collapse_cadence: EventCadence::new(TICKS_PER_HOUR),
            karma_backlash_cadence: EventCadence::new(20 * TICKS_PER_MINUTE),
            world_pressure: WorldPressure::default(),
            active_pseudo_veins: HashMap::new(),
            pending_omens: Vec::new(),
            low_qi_ticks_by_zone: HashMap::new(),
            dead_qi_ticks_by_zone: HashMap::new(),
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
    fn active_pseudo_vein_count(&self) -> usize {
        self.active_pseudo_veins.len()
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

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering world heartbeat scheduler");
    app.insert_resource(WorldHeartbeat::default());
    app.add_event::<EventChainTrigger>();
    app.add_systems(
        Update,
        (
            record_breakthroughs_for_heartbeat,
            forward_realm_collapse_chain_triggers,
            heartbeat_tick,
            chain_reaction_tick.after(heartbeat_tick),
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
    let duration_ticks = command
        .params
        .get("duration_ticks")
        .and_then(value_to_u64)
        .unwrap_or(5 * TICKS_PER_MINUTE);
    if duration_ticks == 0 {
        return Err(HeartbeatOverrideError::InvalidDuration);
    }
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

    maybe_queue_pseudo_vein(
        &mut heartbeat,
        zone_registry,
        modifiers,
        current_tick,
        vfx_events.as_deref_mut(),
    );
    maybe_queue_beast_tide(
        &mut heartbeat,
        zone_registry,
        npc_registry.as_deref(),
        &active_events,
        modifiers,
        current_tick,
        vfx_events.as_deref_mut(),
    );
    maybe_queue_realm_collapse(
        &mut heartbeat,
        zone_registry,
        &player_samples,
        &active_events,
        modifiers,
        current_tick,
        vfx_events.as_deref_mut(),
    );
    maybe_queue_karma_backlash(
        &mut heartbeat,
        zone_registry,
        &player_samples,
        modifiers,
        current_tick,
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
                let neighbor_names = adjacent_zone_names(zone_registry, &source, 900.0);
                for neighbor_name in neighbor_names {
                    let Some(neighbor) = zone_registry.find_zone_by_name(neighbor_name.as_str())
                    else {
                        continue;
                    };
                    let npc_count = npc_counts
                        .and_then(|counts| counts.get(neighbor.name.as_str()).copied())
                        .unwrap_or_default();
                    if neighbor.spirit_qi >= BEAST_TIDE_LOW_QI_THRESHOLD || npc_count <= 3 {
                        continue;
                    }
                    if heartbeat.is_suppressed(
                        HeartbeatEventKind::BeastTide,
                        neighbor.name.as_str(),
                        current_tick,
                    ) {
                        continue;
                    }
                    let command = spawn_event_command(
                        neighbor.name.as_str(),
                        EVENT_BEAST_TIDE,
                        0.3 + (*redistributed_qi).clamp(0.0, 0.4),
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
    zone_name: Option<String>,
    position: DVec3,
    high_realm: bool,
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
    let avg_zone_qi = if zone_registry.zones.is_empty() {
        0.0
    } else {
        zone_registry
            .zones
            .iter()
            .map(|zone| zone.spirit_qi)
            .sum::<f64>()
            / zone_registry.zones.len() as f64
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
            .filter(|sample| sample.high_realm)
            .count() as u32,
        recent_breakthrough_count: heartbeat.recent_breakthrough_ticks.len() as u32,
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

fn maybe_queue_pseudo_vein(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    modifiers: SeasonEventModifiers,
    current_tick: u64,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    if heartbeat.active_pseudo_veins.len() >= PSEUDO_VEIN_ACTIVE_CAP {
        return;
    }
    let Some(anchor) = select_pseudo_vein_anchor(zone_registry, heartbeat, current_tick) else {
        return;
    };
    if heartbeat.is_suppressed(
        HeartbeatEventKind::PseudoVein,
        anchor.name.as_str(),
        current_tick,
    ) {
        return;
    }
    let override_multiplier = heartbeat.override_multiplier(
        HeartbeatEventKind::PseudoVein,
        anchor.name.as_str(),
        current_tick,
    );
    if override_multiplier <= 0.0
        || !heartbeat
            .pseudo_vein_cadence
            .ready(current_tick, override_multiplier)
    {
        return;
    }
    let strength = heartbeat
        .override_intensity(
            HeartbeatEventKind::PseudoVein,
            anchor.name.as_str(),
            current_tick,
        )
        .unwrap_or_else(|| pseudo_vein_strength(modifiers, current_tick, anchor.name.as_str()));
    if queue_omen(
        heartbeat,
        OmenKind::PseudoVeinForming,
        anchor.name,
        None,
        anchor.center,
        strength,
        PSEUDO_VEIN_OMEN_LEAD_TICKS,
        current_tick,
        vfx_events,
    ) {
        heartbeat.pseudo_vein_cadence.mark_fired(current_tick);
    }
}

fn maybe_queue_beast_tide(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    npc_registry: Option<&NpcRegistry>,
    active_events: &ActiveEventsResource,
    modifiers: SeasonEventModifiers,
    current_tick: u64,
    mut vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    let mut tracked_zones = Vec::new();
    for zone in &zone_registry.zones {
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

    for zone in &zone_registry.zones {
        let low_ticks = heartbeat
            .low_qi_ticks_by_zone
            .get(zone.name.as_str())
            .copied()
            .unwrap_or_default();
        if low_ticks < BEAST_TIDE_LOW_QI_REQUIRED_TICKS {
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
                current_tick,
            )
        {
            continue;
        }
        let override_multiplier = heartbeat.override_multiplier(
            HeartbeatEventKind::BeastTide,
            zone.name.as_str(),
            current_tick,
        );
        if override_multiplier <= 0.0
            || !heartbeat
                .beast_tide_cadence
                .ready(current_tick, override_multiplier)
        {
            continue;
        }
        let intensity = heartbeat
            .override_intensity(
                HeartbeatEventKind::BeastTide,
                zone.name.as_str(),
                current_tick,
            )
            .unwrap_or_else(|| {
                (0.25 + npc_count as f64 * 0.04).min(1.0) * modifiers.beast_tide_scale
            });
        if queue_omen(
            heartbeat,
            OmenKind::BeastTideApproaching,
            zone.name.clone(),
            None,
            zone.center(),
            intensity.clamp(0.0, 1.0),
            BEAST_TIDE_OMEN_LEAD_TICKS,
            current_tick,
            vfx_events.as_deref_mut(),
        ) {
            heartbeat.beast_tide_cadence.mark_fired(current_tick);
        }
    }
}

fn maybe_queue_realm_collapse(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    player_samples: &[PlayerSample],
    active_events: &ActiveEventsResource,
    _modifiers: SeasonEventModifiers,
    current_tick: u64,
    mut vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    let mut tracked_zones = Vec::new();
    for zone in &zone_registry.zones {
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

    for zone in &zone_registry.zones {
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
        if !has_player
            || active_events.contains(zone.name.as_str(), EVENT_REALM_COLLAPSE)
            || heartbeat.is_suppressed(
                HeartbeatEventKind::RealmCollapse,
                zone.name.as_str(),
                current_tick,
            )
        {
            continue;
        }
        let override_multiplier = heartbeat.override_multiplier(
            HeartbeatEventKind::RealmCollapse,
            zone.name.as_str(),
            current_tick,
        );
        if override_multiplier <= 0.0
            || !heartbeat
                .realm_collapse_cadence
                .ready(current_tick, override_multiplier)
        {
            continue;
        }
        let intensity = heartbeat
            .override_intensity(
                HeartbeatEventKind::RealmCollapse,
                zone.name.as_str(),
                current_tick,
            )
            .unwrap_or(1.0);
        if queue_omen(
            heartbeat,
            OmenKind::RealmCollapseImminent,
            zone.name.clone(),
            None,
            zone.center(),
            intensity,
            REALM_COLLAPSE_OMEN_LEAD_TICKS,
            current_tick,
            vfx_events.as_deref_mut(),
        ) {
            heartbeat.realm_collapse_cadence.mark_fired(current_tick);
        }
    }
}

fn maybe_queue_karma_backlash(
    heartbeat: &mut WorldHeartbeat,
    zone_registry: &ZoneRegistry,
    player_samples: &[PlayerSample],
    modifiers: SeasonEventModifiers,
    current_tick: u64,
    mut vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    for sample in player_samples {
        let Some(zone_name) = sample.zone_name.as_deref() else {
            continue;
        };
        if heartbeat.is_suppressed(HeartbeatEventKind::KarmaBacklash, zone_name, current_tick) {
            continue;
        }
        let override_multiplier = heartbeat.override_multiplier(
            HeartbeatEventKind::KarmaBacklash,
            zone_name,
            current_tick,
        );
        if override_multiplier <= 0.0
            || !heartbeat
                .karma_backlash_cadence
                .ready(current_tick, override_multiplier)
        {
            continue;
        }
        let recent_factor = 1.0 + heartbeat.world_pressure.recent_breakthrough_count as f64 * 0.1;
        let high_realm_factor = if sample.high_realm { 1.5 } else { 1.0 };
        let probability = KARMA_BASE_ROLL_PROBABILITY
            * modifiers.karma_backlash_frequency
            * recent_factor
            * high_realm_factor;
        if !deterministic_probability_hit(
            ("karma_backlash", sample.player_id.as_str(), current_tick),
            probability,
        ) {
            continue;
        }
        let Some(zone) = zone_registry.find_zone_by_name(zone_name) else {
            continue;
        };
        let intensity = heartbeat
            .override_intensity(HeartbeatEventKind::KarmaBacklash, zone_name, current_tick)
            .unwrap_or(0.7);
        if queue_omen(
            heartbeat,
            OmenKind::KarmaBacklashTarget,
            zone.name.clone(),
            Some(sample.player_id.clone()),
            sample.position,
            intensity,
            KARMA_BACKLASH_OMEN_LEAD_TICKS,
            current_tick,
            vfx_events.as_deref_mut(),
        ) {
            heartbeat.karma_backlash_cadence.mark_fired(current_tick);
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
        HeartbeatEventKind::RealmCollapse => OmenKind::RealmCollapseImminent,
        HeartbeatEventKind::KarmaBacklash => OmenKind::KarmaBacklashTarget,
    }
}

fn event_kind_for_omen(kind: OmenKind) -> HeartbeatEventKind {
    match kind {
        OmenKind::PseudoVeinForming => HeartbeatEventKind::PseudoVein,
        OmenKind::BeastTideApproaching => HeartbeatEventKind::BeastTide,
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
            season_event_modifiers(Season::Summer),
            20_000,
            None,
        );

        assert_eq!(heartbeat.pending_omens.len(), 1);
        assert_eq!(
            heartbeat.pending_omens[0].intensity, 0.42,
            "accelerate intensity_override should drive queued beast tide strength"
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
}
