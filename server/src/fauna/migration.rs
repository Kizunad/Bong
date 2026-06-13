use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::json;
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, Event, EventReader, EventWriter, Position, Query,
    Res, ResMut, Resource, With, Without,
};

use crate::cultivation::tick::CultivationClock;
use crate::fauna::components::FaunaTag;
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::lod::NpcLodTier;
use crate::npc::navigator::Navigator;
use crate::npc::spawn::NpcMarker;
use crate::schema::agent_command::Command;
use crate::schema::common::CommandType;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::events::{ActiveEventsResource, EVENT_BEAST_TIDE};
use crate::world::zone::{Zone, ZoneRegistry};

pub const MIGRATION_THRESHOLD: f64 = 0.05;
pub const HORDE_TRIGGER_THRESHOLD: f64 = MIGRATION_THRESHOLD;
pub const MIGRATION_SUSTAIN_TICKS: u64 = 600;
pub const MIGRATION_MIN_DURATION_TICKS: u32 = 6_000;
pub const MIGRATION_MAX_DURATION_TICKS: u32 = 12_000;
pub const MIGRATION_VISUAL_EVENT_ID: &str = "bong:migration_visual";
pub const MIGRATION_RUMBLE_RECIPE_ID: &str = "beast_migration_rumble";
pub const MIGRATION_BEAST_TIDE_THRESHOLD: usize = 10;
const MIGRATION_VFX_DURATION_TICKS: u16 = 200;
const MIGRATION_RUMBLE_RADIUS_BLOCKS: f64 = 100.0;
const MIGRATION_NEAR_STEP_BLOCKS: f64 = 0.6;
const MIGRATION_FAR_STEP_BLOCKS: f64 = 5.0;
const MIGRATION_REACH_DISTANCE: f64 = 2.0;
const FLOW_FIELD_CELL_SIZE_BLOCKS: f64 = 1.0;
const FLOW_FIELD_MAX_DIMENSION: usize = 256;

#[derive(Debug, Clone, PartialEq, Event)]
pub struct ZoneDepletionEvent {
    pub zone: String,
    pub spirit_qi: f64,
    pub spirit_qi_rate_of_change: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub struct ZoneQiCriticalEvent {
    pub zone_id: String,
    pub spirit_qi: f64,
    pub neighbors: Vec<(String, f64)>,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub struct MigrationEvent {
    pub zone_id: String,
    pub target_zone: String,
    pub direction: [f64; 3],
    pub duration_ticks: u32,
    pub started_at_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HordePhase {
    Gathering,
    Migrating,
    Dispersed,
    Annihilated,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub struct BeastHordeEvent {
    pub source_zone: String,
    pub target_zone: String,
    pub beast_count: u32,
    pub phase: HordePhase,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub struct FlowFieldPrototype {
    pub source_zone: String,
    pub target_zone: String,
    pub direction: [f64; 3],
    pub computed_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub struct FlowFieldComputeTask {
    pub source_zone: String,
    pub target_zone: String,
    pub computed_tick: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowField {
    pub id: String,
    pub source_zone: String,
    pub target_zone: String,
    pub grid_origin_x: f64,
    pub grid_origin_z: f64,
    pub cell_size: f64,
    pub width: usize,
    pub depth: usize,
    pub vectors: Vec<[f64; 2]>,
    pub computed_tick: u64,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct FlowFields {
    fields_by_id: HashMap<String, FlowField>,
}

impl FlowFields {
    pub fn insert(&mut self, field: FlowField) {
        self.fields_by_id.insert(field.id.clone(), field);
    }

    pub fn get(&self, id: &str) -> Option<&FlowField> {
        self.fields_by_id.get(id)
    }

    fn contains(&self, id: &str) -> bool {
        self.fields_by_id.contains_key(id)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.fields_by_id.len()
    }
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct MigrationTarget {
    pub origin_zone: String,
    pub target_zone: String,
    pub target_pos: DVec3,
    pub speed_multiplier: f64,
    pub started_at_tick: u64,
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct HordeMigrationComponent {
    pub target_zone: String,
    pub assigned_flow_field: Option<String>,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct FaunaMigrationState {
    critical_ticks_by_zone: HashMap<String, u64>,
    active_until_by_zone: HashMap<String, u64>,
    last_spirit_qi_by_zone: HashMap<String, f64>,
    last_tick: Option<u64>,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct BeastHordeState {
    phase_by_source_zone: HashMap<String, HordePhase>,
}

impl BeastHordeState {
    fn is_active(&self, source_zone: &str) -> bool {
        matches!(
            self.phase_by_source_zone.get(source_zone),
            Some(HordePhase::Gathering | HordePhase::Migrating)
        )
    }

    fn mark_active(&mut self, source_zone: String, phase: HordePhase) {
        self.phase_by_source_zone.insert(source_zone, phase);
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct ZoneGraph {
    adjacency_by_zone: HashMap<String, Vec<String>>,
}

impl ZoneGraph {
    pub fn from_edges<I, L, R>(edges: I) -> Self
    where
        I: IntoIterator<Item = (L, R)>,
        L: Into<String>,
        R: Into<String>,
    {
        let mut graph = Self::default();
        for (left, right) in edges {
            graph.add_undirected_edge(left, right);
        }
        graph
    }

    pub fn add_undirected_edge(&mut self, left: impl Into<String>, right: impl Into<String>) {
        let left = left.into();
        let right = right.into();
        self.adjacency_by_zone
            .entry(left.clone())
            .or_default()
            .push(right.clone());
        self.adjacency_by_zone.entry(right).or_default().push(left);
    }

    fn is_empty(&self) -> bool {
        self.adjacency_by_zone.is_empty()
    }

    fn neighbors<'a>(&self, source_zone: &str, zones: &'a [Zone]) -> Vec<&'a Zone> {
        let Some(neighbor_names) = self.adjacency_by_zone.get(source_zone) else {
            return Vec::new();
        };
        zones
            .iter()
            .filter(|zone| neighbor_names.iter().any(|name| name == &zone.name))
            .collect()
    }
}

type MigrationTriggerNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static FaunaTag>,
        Option<&'static NpcArchetype>,
    ),
    With<NpcMarker>,
>;

type BeastHordeNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        Option<&'static FaunaTag>,
        Option<&'static NpcArchetype>,
    ),
    With<NpcMarker>,
>;

type MigrationMoveQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Position,
        &'static MigrationTarget,
        Option<&'static NpcLodTier>,
        Option<&'static mut Navigator>,
    ),
    (With<NpcMarker>, Without<HordeMigrationComponent>),
>;

type HordeMigrationAssignQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static FaunaTag>,
        Option<&'static NpcArchetype>,
    ),
    With<NpcMarker>,
>;

type HordeMigrationMoveQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Position,
        &'static HordeMigrationComponent,
        &'static MigrationTarget,
        Option<&'static NpcLodTier>,
        Option<&'static mut Navigator>,
    ),
    With<NpcMarker>,
>;

#[derive(bevy_ecs::system::SystemParam)]
pub struct FaunaMigrationEventWriters<'w> {
    depletion_events: EventWriter<'w, ZoneDepletionEvent>,
    critical_events: EventWriter<'w, ZoneQiCriticalEvent>,
    migration_events: EventWriter<'w, MigrationEvent>,
    vfx_events: EventWriter<'w, VfxEventRequest>,
    audio_events: EventWriter<'w, PlaySoundRecipeRequest>,
}

#[derive(bevy_ecs::system::SystemParam)]
pub struct BeastHordeEventWriters<'w> {
    horde_events: EventWriter<'w, BeastHordeEvent>,
    flow_field_events: EventWriter<'w, FlowFieldPrototype>,
    flow_field_tasks: EventWriter<'w, FlowFieldComputeTask>,
}

pub fn fauna_migration_system(
    zones: Option<Res<ZoneRegistry>>,
    graph: Option<Res<ZoneGraph>>,
    clock: Option<Res<CultivationClock>>,
    mut state: ResMut<FaunaMigrationState>,
    mut events: FaunaMigrationEventWriters,
) {
    let Some(zones) = zones else {
        state.critical_ticks_by_zone.clear();
        state.last_spirit_qi_by_zone.clear();
        state.last_tick = None;
        return;
    };
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let elapsed = state.elapsed_ticks(now);

    for zone in &zones.zones {
        let previous_spirit_qi = state
            .last_spirit_qi_by_zone
            .insert(zone.name.clone(), zone.spirit_qi);
        let spirit_qi_rate_of_change = previous_spirit_qi
            .map(|previous| (zone.spirit_qi - previous) / elapsed.max(1) as f64)
            .unwrap_or(0.0);

        if zone.spirit_qi >= HORDE_TRIGGER_THRESHOLD {
            state.critical_ticks_by_zone.remove(zone.name.as_str());
            continue;
        }
        if spirit_qi_rate_of_change > 0.0 {
            state.critical_ticks_by_zone.remove(zone.name.as_str());
            continue;
        }

        let active_until = state
            .active_until_by_zone
            .get(zone.name.as_str())
            .copied()
            .unwrap_or_default();
        if now < active_until {
            continue;
        }

        let low_ticks = state
            .critical_ticks_by_zone
            .entry(zone.name.clone())
            .or_default();
        *low_ticks = low_ticks.saturating_add(elapsed);
        if *low_ticks < MIGRATION_SUSTAIN_TICKS {
            continue;
        }

        let Some(target_zone) = select_migration_target_zone(zone, &zones.zones, graph.as_deref())
        else {
            continue;
        };
        let duration = migration_duration_ticks(zone);
        state
            .active_until_by_zone
            .insert(zone.name.clone(), now.saturating_add(duration as u64));
        state.critical_ticks_by_zone.remove(zone.name.as_str());

        let critical_event = ZoneQiCriticalEvent {
            zone_id: zone.name.clone(),
            spirit_qi: zone.spirit_qi,
            neighbors: migration_neighbors(zone, &zones.zones, graph.as_deref()),
        };
        let migration_event = MigrationEvent {
            zone_id: zone.name.clone(),
            target_zone: target_zone.name.clone(),
            direction: refuge_direction(zone, target_zone),
            duration_ticks: duration,
            started_at_tick: now,
        };
        let depletion_event = ZoneDepletionEvent {
            zone: zone.name.clone(),
            spirit_qi: zone.spirit_qi,
            spirit_qi_rate_of_change,
            tick: now,
        };

        events.depletion_events.send(depletion_event);
        events.critical_events.send(critical_event);
        events.migration_events.send(migration_event.clone());
        events
            .vfx_events
            .send(migration_vfx_request(zone, &migration_event));
        events.audio_events.send(migration_rumble_request(zone));
    }
}

pub fn beast_horde_detect_system(
    mut depletion_events: EventReader<ZoneDepletionEvent>,
    zones: Option<Res<ZoneRegistry>>,
    graph: Option<Res<ZoneGraph>>,
    mut state: ResMut<BeastHordeState>,
    npcs: BeastHordeNpcQuery<'_, '_>,
    mut writers: BeastHordeEventWriters,
) {
    let Some(zones) = zones else {
        return;
    };

    for event in depletion_events.read() {
        if state.is_active(event.zone.as_str()) {
            continue;
        }
        if event.spirit_qi > HORDE_TRIGGER_THRESHOLD || event.spirit_qi_rate_of_change > 0.0 {
            continue;
        }

        let Some(source_zone) = zones.find_zone_by_name(event.zone.as_str()) else {
            continue;
        };
        let Some(target_zone) =
            select_migration_target_zone(source_zone, &zones.zones, graph.as_deref())
        else {
            continue;
        };
        let beast_count = count_horde_beasts_in_zone(source_zone, &npcs);
        if beast_count == 0 {
            continue;
        }

        state.mark_active(source_zone.name.clone(), HordePhase::Gathering);
        let direction = refuge_direction(source_zone, target_zone);
        writers.horde_events.send(BeastHordeEvent {
            source_zone: source_zone.name.clone(),
            target_zone: target_zone.name.clone(),
            beast_count,
            phase: HordePhase::Gathering,
            tick: event.tick,
        });
        writers.flow_field_events.send(FlowFieldPrototype {
            source_zone: source_zone.name.clone(),
            target_zone: target_zone.name.clone(),
            direction,
            computed_tick: event.tick,
        });
        writers.flow_field_tasks.send(FlowFieldComputeTask {
            source_zone: source_zone.name.clone(),
            target_zone: target_zone.name.clone(),
            computed_tick: event.tick,
        });
    }
}

pub fn flow_field_compute_system(
    mut tasks: EventReader<FlowFieldComputeTask>,
    zones: Option<Res<ZoneRegistry>>,
    mut flow_fields: ResMut<FlowFields>,
) {
    let Some(zones) = zones else {
        return;
    };

    for task in tasks.read() {
        let Some(source_zone) = zones.find_zone_by_name(task.source_zone.as_str()) else {
            continue;
        };
        let Some(target_zone) = zones.find_zone_by_name(task.target_zone.as_str()) else {
            continue;
        };
        let field_id = flow_field_id(
            source_zone.name.as_str(),
            target_zone.name.as_str(),
            task.computed_tick,
        );
        if flow_fields.contains(field_id.as_str()) {
            continue;
        }
        flow_fields.insert(FlowField::from_zones(
            source_zone,
            target_zone,
            task.computed_tick,
        ));
    }
}

pub fn horde_migration_assignment_system(
    mut commands: Commands,
    mut horde_events: EventReader<BeastHordeEvent>,
    zones: Option<Res<ZoneRegistry>>,
    flow_fields: Res<FlowFields>,
    npcs: HordeMigrationAssignQuery<'_, '_>,
) {
    let Some(zones) = zones else {
        return;
    };

    for event in horde_events.read() {
        if event.phase != HordePhase::Gathering {
            continue;
        }
        let Some(source_zone) = zones.find_zone_by_name(event.source_zone.as_str()) else {
            continue;
        };
        let Some(target_zone) = zones.find_zone_by_name(event.target_zone.as_str()) else {
            continue;
        };
        let field_id = flow_field_id(
            source_zone.name.as_str(),
            target_zone.name.as_str(),
            event.tick,
        );
        let assigned_flow_field = flow_fields
            .contains(field_id.as_str())
            .then_some(field_id.clone());
        for (entity, position, fauna_tag, archetype) in &npcs {
            if !source_zone.contains(position.get()) || !is_horde_beast(fauna_tag, archetype) {
                continue;
            }
            let speed_multiplier = migration_speed_multiplier(fauna_tag, archetype)
                .expect("horde beasts must have a migration speed");
            commands.entity(entity).insert((
                HordeMigrationComponent {
                    target_zone: target_zone.name.clone(),
                    assigned_flow_field: assigned_flow_field.clone(),
                },
                MigrationTarget {
                    origin_zone: source_zone.name.clone(),
                    target_zone: target_zone.name.clone(),
                    target_pos: target_zone.center(),
                    speed_multiplier,
                    started_at_tick: event.tick,
                },
            ));
        }
    }
}

pub fn horde_migration_system(
    mut commands: Commands,
    clock: Option<Res<CultivationClock>>,
    zones: Option<Res<ZoneRegistry>>,
    flow_fields: Res<FlowFields>,
    mut migrating: HordeMigrationMoveQuery<'_, '_>,
) {
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let Some(zones) = zones else {
        return;
    };

    for (entity, mut position, horde, target, lod_tier, navigator) in &mut migrating {
        let Some(target_zone) = zones.find_zone_by_name(horde.target_zone.as_str()) else {
            continue;
        };
        let current = position.get();
        if target_zone.contains(current)
            || current.distance(target.target_pos) <= MIGRATION_REACH_DISTANCE
        {
            commands
                .entity(entity)
                .remove::<HordeMigrationComponent>()
                .remove::<MigrationTarget>();
            continue;
        }

        let direction = horde
            .assigned_flow_field
            .as_deref()
            .and_then(|field_id| flow_fields.get(field_id))
            .map(|field| field.direction_at(current))
            .unwrap_or_else(|| direction_toward_xz(current, target.target_pos));

        match lod_tier.copied().unwrap_or_default() {
            NpcLodTier::Dormant => {
                position.set(target.target_pos);
            }
            NpcLodTier::Far => {
                if now % 1_200 == 0 {
                    position.set(step_by_direction_preserving_y(
                        current,
                        direction,
                        MIGRATION_FAR_STEP_BLOCKS,
                    ));
                }
            }
            NpcLodTier::Mid => {
                if now % 600 == 0 {
                    position.set(step_by_direction_preserving_y(
                        current,
                        direction,
                        MIGRATION_FAR_STEP_BLOCKS,
                    ));
                }
            }
            NpcLodTier::Near => {
                if let Some(mut navigator) = navigator {
                    let next_waypoint = step_by_direction_preserving_y(
                        current,
                        direction,
                        MIGRATION_NEAR_STEP_BLOCKS,
                    );
                    navigator.set_goal(next_waypoint, target.speed_multiplier);
                }
            }
        }
    }
}

pub fn migration_trigger_system(
    mut commands: Commands,
    clock: Option<Res<CultivationClock>>,
    mut critical_events: EventReader<ZoneQiCriticalEvent>,
    zone_registry: Option<Res<ZoneRegistry>>,
    npcs: MigrationTriggerNpcQuery<'_, '_>,
) {
    let Some(zone_registry) = zone_registry else {
        return;
    };

    for event in critical_events.read() {
        let Some(source_zone) = zone_registry.find_zone_by_name(event.zone_id.as_str()) else {
            continue;
        };
        let Some(target_zone) = event
            .neighbors
            .iter()
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .and_then(|(zone_id, _)| zone_registry.find_zone_by_name(zone_id.as_str()))
        else {
            continue;
        };
        let started_at_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
        for (entity, position, fauna_tag, archetype) in &npcs {
            if !source_zone.contains(position.get()) {
                continue;
            }
            let Some(speed_multiplier) = migration_speed_multiplier(fauna_tag, archetype) else {
                continue;
            };
            commands.entity(entity).insert(MigrationTarget {
                origin_zone: event.zone_id.clone(),
                target_zone: target_zone.name.clone(),
                target_pos: target_zone.center(),
                speed_multiplier,
                started_at_tick,
            });
        }
    }
}

pub fn migration_move_system(
    mut commands: Commands,
    clock: Option<Res<CultivationClock>>,
    mut migrating: MigrationMoveQuery<'_, '_>,
) {
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (entity, mut position, target, lod_tier, navigator) in &mut migrating {
        let current = position.get();
        if current.distance(target.target_pos) <= MIGRATION_REACH_DISTANCE {
            commands.entity(entity).remove::<MigrationTarget>();
            continue;
        }

        match lod_tier.copied().unwrap_or_default() {
            NpcLodTier::Dormant => {
                position.set(target.target_pos);
            }
            NpcLodTier::Far => {
                if now % 1_200 == 0 {
                    position.set(step_toward_xz_preserving_y(
                        current,
                        target.target_pos,
                        MIGRATION_FAR_STEP_BLOCKS,
                    ));
                }
            }
            // Mid（Drowsy）：hydrated live entity，降频步进（同 Far 语义，稍快）
            NpcLodTier::Mid => {
                if now % 600 == 0 {
                    position.set(step_toward_xz_preserving_y(
                        current,
                        target.target_pos,
                        MIGRATION_FAR_STEP_BLOCKS,
                    ));
                }
            }
            NpcLodTier::Near => {
                if let Some(mut navigator) = navigator {
                    navigator.set_goal(target.target_pos, target.speed_multiplier);
                }
            }
        }
    }
}

pub fn migration_to_beast_tide_system(
    mut active_events: ResMut<ActiveEventsResource>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    migrating_fauna: Query<(&Position, &MigrationTarget, &FaunaTag), With<NpcMarker>>,
) {
    let Some(zone_registry) = zone_registry.as_deref_mut() else {
        return;
    };

    let mut arrivals_by_target: HashMap<String, usize> = HashMap::new();
    for (position, target, _tag) in &migrating_fauna {
        let Some(target_zone) = zone_registry.find_zone_by_name(target.target_zone.as_str()) else {
            continue;
        };
        if target_zone.contains(position.get()) {
            *arrivals_by_target
                .entry(target.target_zone.clone())
                .or_default() += 1;
        }
    }

    for (target_zone, count) in arrivals_by_target {
        if count < MIGRATION_BEAST_TIDE_THRESHOLD
            || active_events.contains(target_zone.as_str(), EVENT_BEAST_TIDE)
        {
            continue;
        }
        let command = migration_beast_tide_command(target_zone.as_str(), count);
        active_events.enqueue_from_spawn_command_with_karma(
            &command,
            Some(&mut *zone_registry),
            None,
            None,
        );
    }
}

impl FaunaMigrationState {
    fn elapsed_ticks(&mut self, now: u64) -> u64 {
        let elapsed = self
            .last_tick
            .map(|last_tick| now.saturating_sub(last_tick).max(1))
            .unwrap_or(1);
        self.last_tick = Some(now);
        elapsed
    }
}

fn migration_speed_multiplier(
    fauna_tag: Option<&FaunaTag>,
    archetype: Option<&NpcArchetype>,
) -> Option<f64> {
    if is_horde_beast(fauna_tag, archetype) {
        Some(1.5)
    } else if archetype.is_some() {
        Some(1.2)
    } else {
        None
    }
}

fn is_horde_beast(fauna_tag: Option<&FaunaTag>, archetype: Option<&NpcArchetype>) -> bool {
    fauna_tag.is_some() || archetype == Some(&NpcArchetype::Beast)
}

fn count_horde_beasts_in_zone(source_zone: &Zone, npcs: &BeastHordeNpcQuery<'_, '_>) -> u32 {
    npcs.iter()
        .filter(|(position, fauna_tag, archetype)| {
            source_zone.contains(position.get()) && is_horde_beast(*fauna_tag, *archetype)
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

fn migration_beast_tide_command(target_zone: &str, beast_count: usize) -> Command {
    Command {
        command_type: CommandType::SpawnEvent,
        target: target_zone.to_string(),
        params: HashMap::from([
            ("event".to_string(), json!(EVENT_BEAST_TIDE)),
            ("tide_kind".to_string(), json!("wandering")),
            (
                "intensity".to_string(),
                json!((beast_count as f64 / 20.0).clamp(0.5, 1.0)),
            ),
            ("duration_ticks".to_string(), json!(24_000_u64)),
            ("reason".to_string(), json!("migration_arrival")),
        ]),
    }
}

fn migration_vfx_request(zone: &Zone, event: &MigrationEvent) -> VfxEventRequest {
    let center = zone.center();
    VfxEventRequest::new(
        center,
        VfxEventPayloadV1::SpawnParticle {
            event_id: MIGRATION_VISUAL_EVENT_ID.to_string(),
            origin: [center.x, center.y, center.z],
            direction: Some(event.direction),
            color: Some("#B08A5A".to_string()),
            strength: Some((1.0 - zone.spirit_qi).clamp(0.20, 1.0) as f32),
            count: Some(migration_visual_count(zone)),
            duration_ticks: Some(MIGRATION_VFX_DURATION_TICKS),
        },
    )
}

fn migration_rumble_request(zone: &Zone) -> PlaySoundRecipeRequest {
    let center = zone.center();
    PlaySoundRecipeRequest {
        recipe_id: MIGRATION_RUMBLE_RECIPE_ID.to_string(),
        instance_id: 0,
        pos: Some([
            center.x.floor() as i32,
            center.y.floor() as i32,
            center.z.floor() as i32,
        ]),
        flag: None,
        volume_mul: (0.75 + (1.0 - zone.spirit_qi).clamp(0.0, 1.0) * 0.35) as f32,
        pitch_shift: -0.15,
        recipient: AudioRecipient::Radius {
            origin: center,
            radius: MIGRATION_RUMBLE_RADIUS_BLOCKS,
        },
    }
}

fn migration_visual_count(zone: &Zone) -> u16 {
    let signal = ((1.0 - zone.spirit_qi).clamp(0.0, 1.0) * 64.0).round() as u16;
    signal.clamp(8, crate::schema::vfx_event::VFX_PARTICLE_COUNT_MAX)
}

fn migration_duration_ticks(zone: &Zone) -> u32 {
    let (min, max) = zone.bounds;
    let area = ((max.x - min.x).abs() * (max.z - min.z).abs()).max(1.0);
    let scaled = MIGRATION_MIN_DURATION_TICKS as f64 + area.sqrt() * 20.0;
    scaled.round().clamp(
        MIGRATION_MIN_DURATION_TICKS as f64,
        MIGRATION_MAX_DURATION_TICKS as f64,
    ) as u32
}

fn select_migration_target_zone<'a>(
    source: &Zone,
    zones: &'a [Zone],
    graph: Option<&ZoneGraph>,
) -> Option<&'a Zone> {
    let graph_neighbors = graph
        .filter(|graph| !graph.is_empty())
        .map(|graph| graph.neighbors(source.name.as_str(), zones));
    let fallback_neighbors: Vec<&Zone>;
    let candidates: &[&Zone] = match graph_neighbors.as_deref() {
        Some(neighbors) => neighbors,
        None => {
            fallback_neighbors = zones.iter().collect::<Vec<_>>();
            fallback_neighbors.as_slice()
        }
    };

    candidates
        .iter()
        .copied()
        .filter(|zone| zone.name != source.name && zone.spirit_qi > source.spirit_qi)
        .max_by(|left, right| left.spirit_qi.total_cmp(&right.spirit_qi))
}

fn migration_neighbors(
    source: &Zone,
    zones: &[Zone],
    graph: Option<&ZoneGraph>,
) -> Vec<(String, f64)> {
    let graph_neighbors = graph
        .filter(|graph| !graph.is_empty())
        .map(|graph| graph.neighbors(source.name.as_str(), zones));
    let fallback_neighbors: Vec<&Zone>;
    let candidates: &[&Zone] = match graph_neighbors.as_deref() {
        Some(neighbors) => neighbors,
        None => {
            fallback_neighbors = zones
                .iter()
                .filter(|zone| zone.name != source.name)
                .collect();
            fallback_neighbors.as_slice()
        }
    };

    candidates
        .iter()
        .copied()
        .filter(|zone| zone.name != source.name)
        .map(|zone| (zone.name.clone(), zone.spirit_qi))
        .collect()
}

fn refuge_direction(source: &Zone, target: &Zone) -> [f64; 3] {
    let source_center = source.center();
    let target_center = target.center();
    let vector = target_center - source_center;
    let horizontal_len = (vector.x * vector.x + vector.z * vector.z).sqrt();
    if horizontal_len <= 1e-6 {
        return [1.0, 0.0, 0.0];
    }
    [vector.x / horizontal_len, 0.0, vector.z / horizontal_len]
}

fn step_toward(current: DVec3, target: DVec3, max_step: f64) -> DVec3 {
    let delta = target - current;
    let distance = delta.length();
    if distance <= max_step || distance <= f64::EPSILON {
        return target;
    }
    current + delta / distance * max_step
}

fn step_toward_xz_preserving_y(current: DVec3, target: DVec3, max_step: f64) -> DVec3 {
    let horizontal_target = DVec3::new(target.x, current.y, target.z);
    step_toward(current, horizontal_target, max_step)
}

fn step_by_direction_preserving_y(current: DVec3, direction: [f64; 3], max_step: f64) -> DVec3 {
    let horizontal = DVec3::new(direction[0], 0.0, direction[2]);
    let len = horizontal.length();
    if len <= f64::EPSILON {
        return current;
    }
    current + horizontal / len * max_step
}

fn direction_toward_xz(current: DVec3, target: DVec3) -> [f64; 3] {
    let delta = DVec3::new(target.x - current.x, 0.0, target.z - current.z);
    let len = delta.length();
    if len <= f64::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [delta.x / len, 0.0, delta.z / len]
    }
}

fn flow_field_id(source_zone: &str, target_zone: &str, computed_tick: u64) -> String {
    format!("{source_zone}->{target_zone}@{computed_tick}")
}

impl FlowField {
    pub fn from_zones(source: &Zone, target: &Zone, computed_tick: u64) -> Self {
        let (min, max) = source.bounds;
        let width = grid_dimension((max.x - min.x).abs());
        let depth = grid_dimension((max.z - min.z).abs());
        let target_cell = closest_cell_to_point(
            min.x,
            min.z,
            FLOW_FIELD_CELL_SIZE_BLOCKS,
            width,
            depth,
            target.center(),
        );
        Self::compute_for_grid(
            source.name.as_str(),
            target.name.as_str(),
            min.x,
            min.z,
            FLOW_FIELD_CELL_SIZE_BLOCKS,
            width,
            depth,
            target_cell,
            computed_tick,
            |_, _| false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_for_grid<F>(
        source_zone: &str,
        target_zone: &str,
        grid_origin_x: f64,
        grid_origin_z: f64,
        cell_size: f64,
        width: usize,
        depth: usize,
        target_cell: (usize, usize),
        computed_tick: u64,
        is_blocked: F,
    ) -> Self
    where
        F: Fn(usize, usize) -> bool,
    {
        let cell_count = width.saturating_mul(depth);
        let mut distances = vec![u32::MAX; cell_count];
        let target_cell =
            nearest_passable_cell(width, depth, target_cell, &is_blocked).unwrap_or((0, 0));
        let target_index = grid_index(width, target_cell.0, target_cell.1);
        let mut queue = VecDeque::new();
        distances[target_index] = 0;
        queue.push_back(target_cell);

        while let Some((x, z)) = queue.pop_front() {
            let next_distance = distances[grid_index(width, x, z)].saturating_add(1);
            for (nx, nz) in grid_neighbors(width, depth, x, z) {
                if is_blocked(nx, nz) {
                    continue;
                }
                let index = grid_index(width, nx, nz);
                if distances[index] <= next_distance {
                    continue;
                }
                distances[index] = next_distance;
                queue.push_back((nx, nz));
            }
        }

        let mut vectors = vec![[0.0, 0.0]; cell_count];
        for z in 0..depth {
            for x in 0..width {
                let index = grid_index(width, x, z);
                if is_blocked(x, z) {
                    continue;
                }
                vectors[index] = best_flow_vector(width, depth, x, z, &distances)
                    .unwrap_or_else(|| fallback_grid_vector(x, z, target_cell));
            }
        }

        Self {
            id: flow_field_id(source_zone, target_zone, computed_tick),
            source_zone: source_zone.to_string(),
            target_zone: target_zone.to_string(),
            grid_origin_x,
            grid_origin_z,
            cell_size,
            width,
            depth,
            vectors,
            computed_tick,
        }
    }

    pub fn direction_at(&self, position: DVec3) -> [f64; 3] {
        let x = cell_coord(position.x, self.grid_origin_x, self.cell_size, self.width);
        let z = cell_coord(position.z, self.grid_origin_z, self.cell_size, self.depth);
        let vector = self.vectors[grid_index(self.width, x, z)];
        [vector[0], 0.0, vector[1]]
    }
}

fn grid_dimension(length: f64) -> usize {
    ((length / FLOW_FIELD_CELL_SIZE_BLOCKS).ceil() as usize).clamp(1, FLOW_FIELD_MAX_DIMENSION)
}

fn closest_cell_to_point(
    origin_x: f64,
    origin_z: f64,
    cell_size: f64,
    width: usize,
    depth: usize,
    point: DVec3,
) -> (usize, usize) {
    (
        cell_coord(point.x, origin_x, cell_size, width),
        cell_coord(point.z, origin_z, cell_size, depth),
    )
}

fn cell_coord(value: f64, origin: f64, cell_size: f64, max: usize) -> usize {
    ((value - origin) / cell_size)
        .floor()
        .clamp(0.0, (max - 1) as f64) as usize
}

fn grid_index(width: usize, x: usize, z: usize) -> usize {
    z * width + x
}

fn grid_neighbors(
    width: usize,
    depth: usize,
    x: usize,
    z: usize,
) -> impl Iterator<Item = (usize, usize)> {
    let mut neighbors = [(usize::MAX, usize::MAX); 4];
    let mut count = 0;
    if x > 0 {
        neighbors[count] = (x - 1, z);
        count += 1;
    }
    if x + 1 < width {
        neighbors[count] = (x + 1, z);
        count += 1;
    }
    if z > 0 {
        neighbors[count] = (x, z - 1);
        count += 1;
    }
    if z + 1 < depth {
        neighbors[count] = (x, z + 1);
        count += 1;
    }
    neighbors.into_iter().take(count)
}

fn nearest_passable_cell<F>(
    width: usize,
    depth: usize,
    target: (usize, usize),
    is_blocked: &F,
) -> Option<(usize, usize)>
where
    F: Fn(usize, usize) -> bool,
{
    if !is_blocked(target.0, target.1) {
        return Some(target);
    }
    (0..depth)
        .flat_map(|z| (0..width).map(move |x| (x, z)))
        .filter(|(x, z)| !is_blocked(*x, *z))
        .min_by_key(|(x, z)| x.abs_diff(target.0) + z.abs_diff(target.1))
}

fn best_flow_vector(
    width: usize,
    depth: usize,
    x: usize,
    z: usize,
    distances: &[u32],
) -> Option<[f64; 2]> {
    let current_distance = distances[grid_index(width, x, z)];
    if current_distance == 0 || current_distance == u32::MAX {
        return None;
    }
    grid_neighbors(width, depth, x, z)
        .min_by_key(|(nx, nz)| distances[grid_index(width, *nx, *nz)])
        .and_then(|(nx, nz)| {
            let neighbor_distance = distances[grid_index(width, nx, nz)];
            (neighbor_distance < current_distance)
                .then(|| normalize_grid_delta(nx as isize - x as isize, nz as isize - z as isize))
        })
}

fn fallback_grid_vector(x: usize, z: usize, target: (usize, usize)) -> [f64; 2] {
    normalize_grid_delta(
        target.0 as isize - x as isize,
        target.1 as isize - z as isize,
    )
}

fn normalize_grid_delta(dx: isize, dz: isize) -> [f64; 2] {
    let dx = dx as f64;
    let dz = dz as f64;
    let len = (dx * dx + dz * dz).sqrt();
    if len <= f64::EPSILON {
        [0.0, 0.0]
    } else {
        [dx / len, dz / len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fauna::components::{BeastKind, FaunaTag};
    use crate::world::dimension::DimensionKind;
    use valence::prelude::{App, Events, IntoSystemConfigs, Update};

    #[test]
    fn critical_qi_triggers_migration() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 0 });
        app.insert_resource(FaunaMigrationState::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("draining", 0.52, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.add_event::<ZoneDepletionEvent>();
        app.add_event::<ZoneQiCriticalEvent>();
        app.add_event::<MigrationEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, fauna_migration_system);

        app.update();
        {
            let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
            zones.zones[0].spirit_qi = 0.04;
        }
        app.world_mut().resource_mut::<CultivationClock>().tick = MIGRATION_SUSTAIN_TICKS;
        app.update();

        let critical = drain_events::<ZoneQiCriticalEvent>(&app);
        assert_eq!(critical.len(), 1);
        assert_eq!(critical[0].zone_id, "draining");
        assert_eq!(critical[0].spirit_qi, 0.04);

        let migrations = drain_events::<MigrationEvent>(&app);
        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].target_zone, "refuge");
        assert!(migrations[0].direction[0] > 0.9);

        let vfx_events = drain_events::<VfxEventRequest>(&app);
        assert_eq!(vfx_events.len(), 1);
        match &vfx_events[0].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                direction,
                duration_ticks,
                ..
            } => {
                assert_eq!(event_id, MIGRATION_VISUAL_EVENT_ID);
                assert_eq!(*direction, Some(migrations[0].direction));
                assert_eq!(*duration_ticks, Some(MIGRATION_VFX_DURATION_TICKS));
            }
            other => panic!("expected migration SpawnParticle VFX, got {other:?}"),
        }

        let audio_events = drain_events::<PlaySoundRecipeRequest>(&app);
        assert_eq!(audio_events.len(), 1);
        assert_eq!(audio_events[0].recipe_id, MIGRATION_RUMBLE_RECIPE_ID);
        assert_eq!(audio_events[0].pos, Some([8, 72, 8]));
    }

    #[test]
    fn depletion_event_carries_rate_without_mutating_zone_qi() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 0 });
        app.insert_resource(FaunaMigrationState::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("draining", 0.06, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.add_event::<ZoneDepletionEvent>();
        app.add_event::<ZoneQiCriticalEvent>();
        app.add_event::<MigrationEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, fauna_migration_system);

        app.update();
        {
            let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
            zones.zones[0].spirit_qi = 0.04;
        }
        app.world_mut().resource_mut::<CultivationClock>().tick = MIGRATION_SUSTAIN_TICKS;
        app.update();

        let depletion = drain_events::<ZoneDepletionEvent>(&app);
        assert_eq!(depletion.len(), 1);
        assert_eq!(depletion[0].zone, "draining");
        assert_eq!(depletion[0].spirit_qi, 0.04);
        assert!(
            depletion[0].spirit_qi_rate_of_change < 0.0,
            "ZoneDepletionEvent 必须携带下降速率，供兽潮检测做守恒链路审计"
        );
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("draining")
                .unwrap()
                .spirit_qi,
            0.04,
            "检测事件只能传递 qi 状态，不得修改 zone.spirit_qi"
        );
    }

    #[test]
    fn improving_low_qi_resets_horde_trigger_window() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 0 });
        app.insert_resource(FaunaMigrationState::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("recovering", 0.04, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.add_event::<ZoneDepletionEvent>();
        app.add_event::<ZoneQiCriticalEvent>();
        app.add_event::<MigrationEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, fauna_migration_system);

        app.update();
        {
            let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
            zones.zones[0].spirit_qi = 0.045;
        }
        app.world_mut().resource_mut::<CultivationClock>().tick = MIGRATION_SUSTAIN_TICKS;
        app.update();

        assert!(
            drain_events::<ZoneDepletionEvent>(&app).is_empty(),
            "低灵气但正在回升时不应触发 ZoneDepletionEvent"
        );
        assert!(
            drain_events::<MigrationEvent>(&app).is_empty(),
            "低灵气回升不能误报迁徙"
        );
    }

    #[test]
    fn migration_target_is_highest_qi_neighbor() {
        let zones = vec![
            zone("source", 0.02, 0.0),
            zone("weak", 0.20, 32.0),
            zone("rich", 0.90, 64.0),
        ];

        let target = select_migration_target_zone(&zones[0], &zones, None)
            .expect("migration should find richest neighboring zone");

        assert_eq!(target.name, "rich");
    }

    #[test]
    fn zone_graph_limits_target_selection_to_declared_neighbors() {
        let zones = vec![
            zone("source", 0.02, 0.0),
            zone("adjacent", 0.60, 32.0),
            zone("far_rich", 0.95, 96.0),
        ];
        let graph = ZoneGraph::from_edges([("source", "adjacent")]);

        let target = select_migration_target_zone(&zones[0], &zones, Some(&graph))
            .expect("source should choose the highest qi declared neighbor");

        assert_eq!(
            target.name, "adjacent",
            "ZoneGraph 存在时不得跨过邻接表直接涌向更远高灵气区"
        );
    }

    #[test]
    fn beast_horde_detect_emits_event_and_flow_field_prototype() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: vec![
                zone("source", 0.02, 0.0),
                zone("adjacent", 0.60, 32.0),
                zone("far_rich", 0.95, 96.0),
            ],
        });
        app.insert_resource(ZoneGraph::from_edges([("source", "adjacent")]));
        app.insert_resource(BeastHordeState::default());
        app.add_event::<ZoneDepletionEvent>();
        app.add_event::<BeastHordeEvent>();
        app.add_event::<FlowFieldPrototype>();
        app.add_event::<FlowFieldComputeTask>();
        app.add_systems(Update, beast_horde_detect_system);

        for x in [4.0, 8.0] {
            app.world_mut().spawn((
                NpcMarker,
                FaunaTag::new(BeastKind::Rat),
                Position::new([x, 66.0, 8.0]),
            ));
        }
        app.world_mut().spawn((
            NpcMarker,
            NpcArchetype::Rogue,
            Position::new([12.0, 66.0, 8.0]),
        ));
        app.world_mut()
            .resource_mut::<Events<ZoneDepletionEvent>>()
            .send(ZoneDepletionEvent {
                zone: "source".to_string(),
                spirit_qi: 0.02,
                spirit_qi_rate_of_change: -0.001,
                tick: 42,
            });

        app.update();

        let hordes = drain_events::<BeastHordeEvent>(&app);
        assert_eq!(hordes.len(), 1);
        assert_eq!(hordes[0].source_zone, "source");
        assert_eq!(hordes[0].target_zone, "adjacent");
        assert_eq!(hordes[0].beast_count, 2);
        assert_eq!(hordes[0].phase, HordePhase::Gathering);
        assert_eq!(hordes[0].tick, 42);

        let flow_fields = drain_events::<FlowFieldPrototype>(&app);
        assert_eq!(flow_fields.len(), 1);
        assert_eq!(flow_fields[0].source_zone, "source");
        assert_eq!(flow_fields[0].target_zone, "adjacent");
        assert!(
            flow_fields[0].direction[0] > 0.9,
            "P0 flow field 原型至少要给出朝目标 zone 的单位方向"
        );
        assert_eq!(flow_fields[0].computed_tick, 42);

        let tasks = drain_events::<FlowFieldComputeTask>(&app);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].source_zone, "source");
        assert_eq!(tasks[0].target_zone, "adjacent");
        assert_eq!(tasks[0].computed_tick, 42);
    }

    #[test]
    fn active_beast_horde_does_not_duplicate() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.insert_resource(BeastHordeState::default());
        app.add_event::<ZoneDepletionEvent>();
        app.add_event::<BeastHordeEvent>();
        app.add_event::<FlowFieldPrototype>();
        app.add_event::<FlowFieldComputeTask>();
        app.add_systems(Update, beast_horde_detect_system);
        app.world_mut().spawn((
            NpcMarker,
            FaunaTag::new(BeastKind::Rat),
            Position::new([8.0, 66.0, 8.0]),
        ));

        for tick in [1, 2] {
            app.world_mut()
                .resource_mut::<Events<ZoneDepletionEvent>>()
                .send(ZoneDepletionEvent {
                    zone: "source".to_string(),
                    spirit_qi: 0.02,
                    spirit_qi_rate_of_change: -0.001,
                    tick,
                });
            app.update();
        }

        let hordes = drain_events::<BeastHordeEvent>(&app);
        assert_eq!(
            hordes.len(),
            1,
            "同一 source zone 已有 Gathering/Migrating 兽潮时不得重复触发"
        );
    }

    #[test]
    fn flow_field_compute_system_builds_shared_vectors_toward_target_zone() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.insert_resource(FlowFields::default());
        app.add_event::<FlowFieldComputeTask>();
        app.add_systems(Update, flow_field_compute_system);
        app.world_mut()
            .resource_mut::<Events<FlowFieldComputeTask>>()
            .send(FlowFieldComputeTask {
                source_zone: "source".to_string(),
                target_zone: "refuge".to_string(),
                computed_tick: 77,
            });

        app.update();

        let flow_fields = app.world().resource::<FlowFields>();
        let field = flow_fields
            .get("source->refuge@77")
            .expect("FlowFieldComputeTask 应生成可复用 FlowField");
        assert_eq!(field.source_zone, "source");
        assert_eq!(field.target_zone, "refuge");
        assert_eq!(field.computed_tick, 77);
        assert!(
            field.direction_at(DVec3::new(4.0, 66.0, 8.0))[0] > 0.9,
            "source 内任意兽群应沿共享流场朝 refuge 边界移动"
        );
    }

    #[test]
    fn flow_field_bfs_routes_around_blocked_cells() {
        let field = FlowField::compute_for_grid(
            "source",
            "refuge",
            0.0,
            0.0,
            1.0,
            5,
            3,
            (4, 1),
            9,
            |x, z| x == 2 && z != 0,
        );

        let vector = field.direction_at(DVec3::new(1.0, 66.0, 1.0));

        assert!(
            vector[2] < -0.9,
            "BFS 流场必须绕开阻塞列，而不是把野兽直接推向 x=2 的阻塞格"
        );
    }

    #[test]
    fn horde_assignment_attaches_same_flow_field_to_beasts_only() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        let mut flow_fields = FlowFields::default();
        flow_fields.insert(FlowField::from_zones(
            &zone("source", 0.02, 0.0),
            &zone("refuge", 0.90, 64.0),
            5,
        ));
        app.insert_resource(flow_fields);
        app.add_event::<BeastHordeEvent>();
        app.add_systems(Update, horde_migration_assignment_system);
        let rat_a = app
            .world_mut()
            .spawn((
                NpcMarker,
                FaunaTag::new(BeastKind::Rat),
                Position::new([4.0, 66.0, 8.0]),
            ))
            .id();
        let rat_b = app
            .world_mut()
            .spawn((
                NpcMarker,
                FaunaTag::new(BeastKind::Spider),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();
        let rogue = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Rogue,
                Position::new([12.0, 66.0, 8.0]),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Events<BeastHordeEvent>>()
            .send(BeastHordeEvent {
                source_zone: "source".to_string(),
                target_zone: "refuge".to_string(),
                beast_count: 2,
                phase: HordePhase::Gathering,
                tick: 5,
            });

        app.update();

        for entity in [rat_a, rat_b] {
            let horde = app
                .world()
                .get::<HordeMigrationComponent>(entity)
                .expect("兽潮野兽必须挂接 HordeMigrationComponent");
            assert_eq!(horde.target_zone, "refuge");
            assert_eq!(
                horde.assigned_flow_field.as_deref(),
                Some("source->refuge@5"),
                "同一股兽潮的野兽必须共享同一个 FlowField"
            );
        }
        assert!(
            app.world().get::<HordeMigrationComponent>(rogue).is_none(),
            "普通 NPC 仍由既有 MigrationTarget 逃离系统处理，不应被兽潮流场接管"
        );
    }

    #[test]
    fn horde_migration_system_moves_far_entities_by_shared_flow_vector() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1_200 });
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        let mut flow_fields = FlowFields::default();
        flow_fields.insert(FlowField::from_zones(
            &zone("source", 0.02, 0.0),
            &zone("refuge", 0.90, 64.0),
            5,
        ));
        app.insert_resource(flow_fields);
        app.add_systems(Update, horde_migration_system);
        let start_pos = DVec3::new(4.0, 96.0, 8.0);
        let entity = spawn_horde_entity(&mut app, start_pos, NpcLodTier::Far, 5);

        app.update();

        let moved = app.world().get::<Position>(entity).unwrap().get();
        assert!(
            moved.x > start_pos.x,
            "Far 兽潮应按共享 FlowField 朝目标推进"
        );
        assert_eq!(moved.y, start_pos.y, "FlowField 只管 XZ 方向，不能制造飞行");
    }

    #[test]
    fn near_horde_entities_delegate_to_navigator_waypoint() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1 });
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        let mut flow_fields = FlowFields::default();
        flow_fields.insert(FlowField::from_zones(
            &zone("source", 0.02, 0.0),
            &zone("refuge", 0.90, 64.0),
            5,
        ));
        app.insert_resource(flow_fields);
        app.add_systems(Update, horde_migration_system);
        let start_pos = DVec3::new(4.0, 96.0, 8.0);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Near,
                Navigator::new(),
                Position::new([start_pos.x, start_pos.y, start_pos.z]),
                HordeMigrationComponent {
                    target_zone: "refuge".to_string(),
                    assigned_flow_field: Some("source->refuge@5".to_string()),
                },
                MigrationTarget {
                    origin_zone: "source".to_string(),
                    target_zone: "refuge".to_string(),
                    target_pos: DVec3::new(72.0, 72.0, 8.0),
                    speed_multiplier: 1.5,
                    started_at_tick: 5,
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Position>(entity).unwrap().get(),
            start_pos,
            "Near 兽潮实体同 tick 不应裸写 Position"
        );
        assert!(
            !app.world().get::<Navigator>(entity).unwrap().is_idle(),
            "Near 兽潮实体应把流场下一步交给 Navigator"
        );
    }

    #[test]
    fn horde_migration_excludes_generic_migration_position_writer() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1_200 });
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        let mut flow_fields = FlowFields::default();
        flow_fields.insert(FlowField::from_zones(
            &zone("source", 0.02, 0.0),
            &zone("refuge", 0.90, 64.0),
            5,
        ));
        app.insert_resource(flow_fields);
        app.add_systems(Update, (migration_move_system, horde_migration_system));
        let start_pos = DVec3::new(4.0, 96.0, 8.0);
        let entity = spawn_horde_entity(&mut app, start_pos, NpcLodTier::Far, 5);

        app.update();

        let moved = app.world().get::<Position>(entity).unwrap().get();
        assert!(
            moved.distance(start_pos) <= MIGRATION_FAR_STEP_BLOCKS + f64::EPSILON,
            "HordeMigrationComponent 必须让通用 migration_move_system 让权，避免同 tick 双写 Position"
        );
    }

    #[test]
    fn horde_migration_system_moves_200_far_beasts_under_five_ms_budget() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1_199 });
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        let mut flow_fields = FlowFields::default();
        flow_fields.insert(FlowField::from_zones(
            &zone("source", 0.02, 0.0),
            &zone("refuge", 0.90, 64.0),
            5,
        ));
        app.insert_resource(flow_fields);
        app.add_systems(Update, horde_migration_system);
        let entities = (0..200)
            .map(|index| {
                spawn_horde_entity(
                    &mut app,
                    DVec3::new(2.0 + (index % 10) as f64, 96.0, 2.0 + (index / 10) as f64),
                    NpcLodTier::Far,
                    5,
                )
            })
            .collect::<Vec<_>>();

        app.update();
        app.world_mut().resource_mut::<CultivationClock>().tick = 1_200;
        let started_at = std::time::Instant::now();
        app.update();
        let elapsed = started_at.elapsed();

        assert!(
            elapsed <= std::time::Duration::from_millis(5),
            "200 兽 FlowField 迁移单 tick 应控制在 5ms 内，实际耗时 {elapsed:?}"
        );
        let moved_count = entities
            .iter()
            .filter(|entity| {
                app.world()
                    .get::<Position>(**entity)
                    .expect("测试实体应仍有 Position")
                    .get()
                    .x
                    > 2.0
            })
            .count();
        assert_eq!(
            moved_count, 200,
            "性能验收 tick 不能只空跑，200 只 Far 兽必须全部按流场推进"
        );
    }

    #[test]
    fn multiple_hordes_keep_independent_flow_fields() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: vec![
                zone("source_a", 0.02, 0.0),
                zone("source_b", 0.02, 32.0),
                zone("refuge", 0.90, 64.0),
            ],
        });
        app.insert_resource(FlowFields::default());
        app.add_event::<FlowFieldComputeTask>();
        app.add_systems(Update, flow_field_compute_system);
        for source_zone in ["source_a", "source_b"] {
            app.world_mut()
                .resource_mut::<Events<FlowFieldComputeTask>>()
                .send(FlowFieldComputeTask {
                    source_zone: source_zone.to_string(),
                    target_zone: "refuge".to_string(),
                    computed_tick: 12,
                });
        }

        app.update();

        let flow_fields = app.world().resource::<FlowFields>();
        assert_eq!(flow_fields.len(), 2);
        assert!(flow_fields.get("source_a->refuge@12").is_some());
        assert!(flow_fields.get("source_b->refuge@12").is_some());
    }

    #[test]
    fn mass_arrival_triggers_beast_tide() {
        let mut app = App::new();
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.add_systems(Update, migration_to_beast_tide_system);

        for _ in 0..MIGRATION_BEAST_TIDE_THRESHOLD {
            app.world_mut().spawn((
                NpcMarker,
                FaunaTag::new(BeastKind::Rat),
                Position::new([70.0, 66.0, 8.0]),
                MigrationTarget {
                    origin_zone: "source".to_string(),
                    target_zone: "refuge".to_string(),
                    target_pos: DVec3::new(72.0, 72.0, 8.0),
                    speed_multiplier: 1.5,
                    started_at_tick: 0,
                },
            ));
        }

        app.update();

        assert!(
            app.world()
                .resource::<ActiveEventsResource>()
                .contains("refuge", EVENT_BEAST_TIDE),
            "10+ 迁徙兽群抵达应升级为既有兽潮状态机"
        );
    }

    #[test]
    fn npc_also_flees() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.insert_resource(CultivationClock { tick: 77 });
        app.add_event::<ZoneQiCriticalEvent>();
        app.add_systems(Update, migration_trigger_system);
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Rogue,
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<ZoneQiCriticalEvent>>()
            .send(ZoneQiCriticalEvent {
                zone_id: "source".to_string(),
                spirit_qi: 0.02,
                neighbors: vec![("refuge".to_string(), 0.90)],
            });
        app.update();

        let target = app
            .world()
            .get::<MigrationTarget>(npc)
            .expect("NPC should receive MigrationTarget");
        assert_eq!(target.target_zone, "refuge");
        assert_eq!(target.speed_multiplier, 1.2);
        assert_eq!(target.started_at_tick, 77);
    }

    #[test]
    fn dormant_entities_teleport() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1 });
        app.add_systems(Update, migration_move_system);
        let target_pos = DVec3::new(72.0, 66.0, 8.0);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Dormant,
                Position::new([8.0, 66.0, 8.0]),
                MigrationTarget {
                    origin_zone: "source".to_string(),
                    target_zone: "refuge".to_string(),
                    target_pos,
                    speed_multiplier: 1.5,
                    started_at_tick: 0,
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Position>(entity).unwrap().get(),
            target_pos,
            "Dormant 层不真实寻路，直接落到目标 zone 边缘"
        );
    }

    #[test]
    fn near_entities_delegate_migration_to_navigator_without_direct_position_step() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1 });
        app.add_systems(Update, migration_move_system);
        let start_pos = DVec3::new(8.0, 80.0, 8.0);
        let target_pos = DVec3::new(72.0, 66.0, 8.0);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Near,
                Position::new([start_pos.x, start_pos.y, start_pos.z]),
                Navigator::new(),
                MigrationTarget {
                    origin_zone: "source".to_string(),
                    target_zone: "refuge".to_string(),
                    target_pos,
                    speed_multiplier: 1.5,
                    started_at_tick: 0,
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Position>(entity).unwrap().get(),
            start_pos,
            "Near 迁徙实体必须交给 Navigator，不能同 tick 裸写 Position 穿墙/飞行"
        );
        assert!(
            !app.world().get::<Navigator>(entity).unwrap().is_idle(),
            "Near 迁徙实体应设置 Navigator 目标"
        );
    }

    #[test]
    fn far_entities_preserve_altitude_during_coarse_migration_step() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1_200 });
        app.add_systems(Update, migration_move_system);
        let start_pos = DVec3::new(8.0, 96.0, 8.0);
        let target_pos = DVec3::new(72.0, 66.0, 8.0);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Far,
                Position::new([start_pos.x, start_pos.y, start_pos.z]),
                MigrationTarget {
                    origin_zone: "source".to_string(),
                    target_zone: "refuge".to_string(),
                    target_pos,
                    speed_multiplier: 1.5,
                    started_at_tick: 0,
                },
            ))
            .id();

        app.update();

        let moved = app.world().get::<Position>(entity).unwrap().get();
        assert!(moved.x > start_pos.x, "Far 迁徙仍应低频向目标 XZ 推进");
        assert_eq!(
            moved.y, start_pos.y,
            "Far 迁徙的粗粒度模拟不能把实体沿 3D 直线拉成飞行"
        );
    }

    #[test]
    fn world_ecology_feedback_loop_low_qi_escalates_to_beast_tide() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 0 });
        app.insert_resource(FaunaMigrationState::default());
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("draining", 0.04, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.add_event::<ZoneDepletionEvent>();
        app.add_event::<ZoneQiCriticalEvent>();
        app.add_event::<MigrationEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(
            Update,
            (
                fauna_migration_system,
                migration_trigger_system.after(fauna_migration_system),
                migration_move_system.after(migration_trigger_system),
                migration_to_beast_tide_system.after(migration_move_system),
            ),
        );

        for _ in 0..MIGRATION_BEAST_TIDE_THRESHOLD {
            app.world_mut().spawn((
                NpcMarker,
                NpcLodTier::Dormant,
                FaunaTag::new(BeastKind::Rat),
                Position::new([8.0, 66.0, 8.0]),
            ));
        }

        app.update();
        app.world_mut().resource_mut::<CultivationClock>().tick = MIGRATION_SUSTAIN_TICKS;
        app.update();
        app.world_mut().resource_mut::<CultivationClock>().tick = MIGRATION_SUSTAIN_TICKS + 1;
        app.update();

        let events = app.world().resource::<ActiveEventsResource>();
        assert!(
            events.contains("refuge", EVENT_BEAST_TIDE),
            "低灵气持续阈值应触发迁徙，Dormant 兽群到达邻区后升级为兽潮"
        );
        assert_eq!(events.beast_tide_kind_for_zone("refuge"), Some("wandering"));
    }

    fn drain_events<T: Event + Clone>(app: &App) -> Vec<T> {
        let events = app.world().resource::<Events<T>>();
        events
            .get_reader()
            .read(events)
            .cloned()
            .collect::<Vec<_>>()
    }

    fn spawn_horde_entity(
        app: &mut App,
        start_pos: DVec3,
        lod_tier: NpcLodTier,
        computed_tick: u64,
    ) -> Entity {
        app.world_mut()
            .spawn((
                NpcMarker,
                lod_tier,
                Position::new([start_pos.x, start_pos.y, start_pos.z]),
                HordeMigrationComponent {
                    target_zone: "refuge".to_string(),
                    assigned_flow_field: Some(flow_field_id("source", "refuge", computed_tick)),
                },
                MigrationTarget {
                    origin_zone: "source".to_string(),
                    target_zone: "refuge".to_string(),
                    target_pos: DVec3::new(72.0, 72.0, 8.0),
                    speed_multiplier: 1.5,
                    started_at_tick: computed_tick,
                },
            ))
            .id()
    }

    fn zone(name: &str, spirit_qi: f64, x: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(x, 64.0, 0.0), DVec3::new(x + 16.0, 80.0, 16.0)),
            spirit_qi,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }
}
