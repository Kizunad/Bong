use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::entity::entity::{CustomName, NameVisible};
use valence::prelude::{
    bevy_ecs, BlockPos, Changed, ChunkPos, Color, Component, DVec3, Entity, Event, EventReader,
    EventWriter, IntoText, ParamSet, Position, Query, Res, ResMut, Resource, Text, With,
};

use crate::combat::events::DeathEvent;
use crate::combat::CombatClock;
use crate::npc::spawn::NpcMarker;
use crate::npc::spawn_rat::RatBlackboard;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::karma::{QiDensityHeatmap, QI_DENSITY_CELL_SIZE};
use crate::world::zone::ZoneRegistry;

pub const RAT_PHASE_DENSITY_THRESHOLD: f32 = 8.0;
pub const RAT_PHASE_QI_GRADIENT_THRESHOLD: f32 = 0.20;
pub const SURGE_TRIGGER_THRESHOLD: f32 = 1.0;
pub const TRANSITION_DURATION_TICKS: u16 = 600;
pub const RAT_DRAINED_CHUNK_WINDOW: usize = 8;

type RatPhaseReadQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static RatGroupId,
        &'static RatBlackboard,
        &'static RatPhase,
    ),
    With<NpcMarker>,
>;

type RatPhaseUpdateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static RatGroupId,
        &'static RatBlackboard,
        &'static mut PressureSensor,
        &'static RatPhase,
    ),
    With<NpcMarker>,
>;

type RatPhaseVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RatPhase,
        &'static mut CustomName,
        &'static mut NameVisible,
    ),
    (With<NpcMarker>, Changed<RatPhase>),
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Component)]
#[serde(rename_all = "snake_case")]
pub enum RatPhase {
    Solitary,
    Transitioning { progress: u16 },
    Gregarious,
}

impl Default for RatPhase {
    fn default() -> Self {
        Self::Solitary
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
pub struct RatGroupId(pub u64);

impl RatGroupId {
    pub fn for_zone_chunk(zone_name: &str, chunk: ChunkPos) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in zone_name.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        hash ^= (chunk.x as u64).rotate_left(17);
        hash ^= (chunk.z as u64).rotate_right(11);
        Self(hash)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct PressureSensor {
    pub local_density: f32,
    pub qi_pressure_grad: f32,
    pub surge_intensity: f32,
    pub negative_pressure_avoidance: f32,
}

impl Default for PressureSensor {
    fn default() -> Self {
        Self {
            local_density: 0.0,
            qi_pressure_grad: 0.0,
            surge_intensity: 0.0,
            negative_pressure_avoidance: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct MeditatingState {
    pub since_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct RatPhaseChangeEvent {
    pub chunk: [i32; 2],
    pub zone: String,
    pub group_id: u64,
    pub from: RatPhase,
    pub to: RatPhase,
    pub rat_count: u32,
    pub local_qi: f32,
    pub qi_gradient: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RatDensitySnapshot {
    pub total: u32,
    pub solitary: u32,
    pub transitioning: u32,
    pub gregarious: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RatDensityHeatmapV1 {
    pub zones: HashMap<String, RatDensitySnapshot>,
}

#[derive(Debug, Clone, Resource)]
pub struct LocustSwarmCooldownStore {
    pub last_swarm_by_zone: HashMap<String, u64>,
    pub cooldown_ticks: u64,
}

impl LocustSwarmCooldownStore {
    pub const DEFAULT_COOLDOWN_TICKS: u64 = 24 * 3600 * 20;

    pub fn ready_at(&self, zone: &str, tick: u64) -> bool {
        self.last_swarm_by_zone
            .get(zone)
            .is_none_or(|last| tick.saturating_sub(*last) >= self.cooldown_ticks)
    }

    pub fn mark(&mut self, zone: impl Into<String>, tick: u64) {
        self.last_swarm_by_zone.insert(zone.into(), tick);
    }
}

impl Default for LocustSwarmCooldownStore {
    fn default() -> Self {
        Self {
            last_swarm_by_zone: HashMap::new(),
            cooldown_ticks: Self::DEFAULT_COOLDOWN_TICKS,
        }
    }
}

pub fn chunk_pos_from_world(position: DVec3) -> ChunkPos {
    ChunkPos::new(
        (position.x.floor() as i32).div_euclid(16),
        (position.z.floor() as i32).div_euclid(16),
    )
}

pub fn rat_phase_wire_name(phase: RatPhase) -> &'static str {
    match phase {
        RatPhase::Solitary => "solitary",
        RatPhase::Transitioning { .. } => "transitioning",
        RatPhase::Gregarious => "gregarious",
    }
}

pub fn rat_phase_display_name(phase: &RatPhase) -> Text {
    match phase {
        RatPhase::Solitary => "噬元鼠".color(Color::GRAY),
        RatPhase::Transitioning { .. } => "赤噬元鼠".color(Color::DARK_RED),
        RatPhase::Gregarious => "灵蝗噬元鼠".color(Color::RED),
    }
}

pub fn is_drained_chunk(blackboard: &RatBlackboard, chunk: ChunkPos) -> bool {
    blackboard.recently_drained.contains(&chunk)
}

pub fn remember_drained_chunk(blackboard: &mut RatBlackboard, chunk: ChunkPos) {
    if is_drained_chunk(blackboard, chunk) {
        return;
    }
    blackboard.recently_drained.push(chunk);
    if blackboard.recently_drained.len() > RAT_DRAINED_CHUNK_WINDOW {
        let overflow = blackboard.recently_drained.len() - RAT_DRAINED_CHUNK_WINDOW;
        blackboard.recently_drained.drain(0..overflow);
    }
}

pub fn pressure_sensor_tick_system(
    clock: Option<Res<CombatClock>>,
    heatmap: Option<Res<QiDensityHeatmap>>,
    mut phase_events: EventWriter<RatPhaseChangeEvent>,
    mut rats: ParamSet<(RatPhaseReadQuery<'_, '_>, RatPhaseUpdateQuery<'_, '_>)>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let mut counts: HashMap<(u64, ChunkPos), u32> = HashMap::new();
    for (_, position, _, group_id, _, _) in rats.p0().iter() {
        *counts
            .entry((group_id.0, chunk_pos_from_world(position.get())))
            .or_default() += 1;
    }

    for (_, position, dimension, group_id, blackboard, mut sensor, phase) in rats.p1().iter_mut() {
        let chunk = chunk_pos_from_world(position.get());
        let rat_count = counts.get(&(group_id.0, chunk)).copied().unwrap_or(0);
        sensor.local_density = rat_count as f32 / RAT_PHASE_DENSITY_THRESHOLD;
        sensor.qi_pressure_grad = qi_gradient_for_chunk(
            heatmap.as_deref(),
            dimension.map(|dim| dim.0).unwrap_or_default(),
            chunk,
        );
        if sensor.local_density >= 1.0 && sensor.qi_pressure_grad >= RAT_PHASE_QI_GRADIENT_THRESHOLD
        {
            sensor.surge_intensity += sensor.local_density * sensor.qi_pressure_grad;
        } else {
            sensor.surge_intensity *= 0.5;
        }

        if matches!(phase, RatPhase::Solitary)
            && sensor.surge_intensity >= SURGE_TRIGGER_THRESHOLD
            && !is_drained_chunk(blackboard, chunk)
        {
            phase_events.send(RatPhaseChangeEvent {
                chunk: [chunk.x, chunk.z],
                zone: blackboard.home_zone.clone(),
                group_id: group_id.0,
                from: RatPhase::Solitary,
                to: RatPhase::Transitioning { progress: 0 },
                rat_count,
                local_qi: heatmap
                    .as_deref()
                    .map(|heatmap| {
                        heatmap.heat_at(
                            dimension.map(|dim| dim.0).unwrap_or_default(),
                            chunk_block_pos(chunk),
                        )
                    })
                    .unwrap_or_default(),
                qi_gradient: sensor.qi_pressure_grad,
                tick,
            });
        }
    }
}

pub fn apply_rat_phase_change_system(
    mut events: EventReader<RatPhaseChangeEvent>,
    mut rats: Query<(&Position, &RatGroupId, &mut RatPhase), With<NpcMarker>>,
) {
    for event in events.read() {
        let chunk = ChunkPos::new(event.chunk[0], event.chunk[1]);
        for (position, group_id, mut phase) in &mut rats {
            if group_id.0 == event.group_id && chunk_pos_from_world(position.get()) == chunk {
                *phase = event.to;
            }
        }
    }
}

pub fn advance_transitioning_phase_system(mut rats: Query<&mut RatPhase, With<NpcMarker>>) {
    for mut phase in &mut rats {
        if let RatPhase::Transitioning { progress } = *phase {
            let next = progress.saturating_add(1);
            *phase = if next >= TRANSITION_DURATION_TICKS {
                RatPhase::Gregarious
            } else {
                RatPhase::Transitioning { progress: next }
            };
        }
    }
}

pub fn apply_rat_phase_visual_system(mut rats: RatPhaseVisualQuery<'_, '_>) {
    for (phase, mut custom_name, mut name_visible) in &mut rats {
        custom_name.0 = Some(rat_phase_display_name(phase));
        name_visible.0 = true;
    }
}

pub fn release_drained_qi_on_death_system(
    mut deaths: EventReader<DeathEvent>,
    rats: Query<(&Position, Option<&CurrentDimension>, &RatBlackboard), With<NpcMarker>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
) {
    let Some(zones) = zones.as_deref_mut() else {
        for _ in deaths.read() {}
        return;
    };

    for death in deaths.read() {
        let Ok((position, dimension, rat)) = rats.get(death.target) else {
            continue;
        };
        if rat.drained_qi <= 0.0 {
            continue;
        }
        let dim = dimension.map(|dim| dim.0).unwrap_or_default();
        let Some(zone_name) = zones
            .find_zone(dim, position.get())
            .map(|zone| zone.name.clone())
        else {
            continue;
        };
        if let Some(zone) = zones.find_zone_mut(zone_name.as_str()) {
            zone.spirit_qi = (zone.spirit_qi + rat.drained_qi * 0.01).clamp(-1.0, 1.0);
        }
    }
}

pub fn collect_rat_density_heatmap<'a, I>(rats: I) -> RatDensityHeatmapV1
where
    I: IntoIterator<Item = (&'a RatBlackboard, &'a RatPhase)>,
{
    let mut heatmap = RatDensityHeatmapV1::default();
    for (blackboard, phase) in rats {
        let entry = heatmap
            .zones
            .entry(blackboard.home_zone.clone())
            .or_default();
        entry.total = entry.total.saturating_add(1);
        match phase {
            RatPhase::Solitary => entry.solitary = entry.solitary.saturating_add(1),
            RatPhase::Transitioning { .. } => {
                entry.transitioning = entry.transitioning.saturating_add(1)
            }
            RatPhase::Gregarious => entry.gregarious = entry.gregarious.saturating_add(1),
        }
    }
    heatmap
}

fn qi_gradient_for_chunk(
    heatmap: Option<&QiDensityHeatmap>,
    dimension: DimensionKind,
    chunk: ChunkPos,
) -> f32 {
    let Some(heatmap) = heatmap else {
        return 0.0;
    };
    let mut min_heat = f32::MAX;
    let mut max_heat = f32::MIN;
    for dx in -1..=1 {
        for dz in -1..=1 {
            let sample = ChunkPos::new(chunk.x + dx, chunk.z + dz);
            let heat = heatmap.heat_at(dimension, chunk_block_pos(sample));
            min_heat = min_heat.min(heat);
            max_heat = max_heat.max(heat);
        }
    }
    if min_heat == f32::MAX {
        0.0
    } else {
        (max_heat - min_heat).max(0.0)
    }
}

fn chunk_block_pos(chunk: ChunkPos) -> BlockPos {
    BlockPos::new(
        chunk.x * QI_DENSITY_CELL_SIZE,
        64,
        chunk.z * QI_DENSITY_CELL_SIZE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::entity::entity::{CustomName, NameVisible};
    use valence::prelude::{App, Events, Update};

    use crate::world::karma::QiDensityHeatmap;

    fn rat_blackboard(zone: &str, chunk: ChunkPos) -> RatBlackboard {
        RatBlackboard {
            home_chunk: chunk,
            home_zone: zone.to_string(),
            group_id: RatGroupId(7),
            last_pressure_target: None,
            recently_drained: Vec::new(),
            drained_qi: 0.0,
        }
    }

    #[test]
    fn rat_phase_default_is_solitary() {
        assert_eq!(RatPhase::default(), RatPhase::Solitary);
    }

    #[test]
    fn pressure_sensor_density_threshold_triggers_transition() {
        let mut app = App::new();
        app.add_event::<RatPhaseChangeEvent>();
        app.insert_resource(CombatClock { tick: 12345 });
        let mut heatmap = QiDensityHeatmap::default();
        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 0.0);
        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(16, 64, 0), 1.0);
        app.insert_resource(heatmap);
        app.add_systems(Update, pressure_sensor_tick_system);

        let chunk = ChunkPos::new(0, 0);
        for i in 0..8 {
            app.world_mut().spawn((
                NpcMarker,
                Position::new([f64::from(i), 64.0, 0.0]),
                RatGroupId(7),
                rat_blackboard("spawn", chunk),
                PressureSensor::default(),
                RatPhase::Solitary,
            ));
        }

        app.update();

        let events = app.world().resource::<Events<RatPhaseChangeEvent>>();
        let event = events
            .iter_current_update_events()
            .next()
            .expect("dense rat chunk with qi gradient should emit transition");
        assert_eq!(event.chunk, [0, 0]);
        assert_eq!(event.from, RatPhase::Solitary);
        assert_eq!(event.to, RatPhase::Transitioning { progress: 0 });
        assert_eq!(event.rat_count, 8);
    }

    #[test]
    fn pressure_sensor_low_qi_gradient_does_not_transition_alone() {
        let mut app = App::new();
        app.add_event::<RatPhaseChangeEvent>();
        app.insert_resource(QiDensityHeatmap::default());
        app.add_systems(Update, pressure_sensor_tick_system);
        let chunk = ChunkPos::new(0, 0);
        for i in 0..8 {
            app.world_mut().spawn((
                NpcMarker,
                Position::new([f64::from(i), 64.0, 0.0]),
                RatGroupId(7),
                rat_blackboard("spawn", chunk),
                PressureSensor::default(),
                RatPhase::Solitary,
            ));
        }

        app.update();

        assert!(
            app.world()
                .resource::<Events<RatPhaseChangeEvent>>()
                .is_empty(),
            "density without qi gradient must not trigger phase transition"
        );
    }

    #[test]
    fn pressure_sensor_high_qi_gradient_alone_does_not_transition() {
        let mut app = App::new();
        app.add_event::<RatPhaseChangeEvent>();
        let mut heatmap = QiDensityHeatmap::default();
        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 0.0);
        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(16, 64, 0), 1.0);
        app.insert_resource(heatmap);
        app.add_systems(Update, pressure_sensor_tick_system);

        let chunk = ChunkPos::new(0, 0);
        app.world_mut().spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            RatGroupId(7),
            rat_blackboard("spawn", chunk),
            PressureSensor::default(),
            RatPhase::Solitary,
        ));

        app.update();

        assert!(
            app.world()
                .resource::<Events<RatPhaseChangeEvent>>()
                .is_empty(),
            "qi gradient without enough rats must not trigger phase transition"
        );
    }

    #[test]
    fn transitioning_phase_promotes_to_gregarious_after_duration() {
        let mut app = App::new();
        app.add_systems(Update, advance_transitioning_phase_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                RatPhase::Transitioning {
                    progress: TRANSITION_DURATION_TICKS - 1,
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<RatPhase>(rat),
            Some(&RatPhase::Gregarious)
        );
    }

    #[test]
    fn rat_phase_visual_name_tracks_phase() {
        let mut app = App::new();
        app.add_systems(Update, apply_rat_phase_visual_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                RatPhase::Transitioning { progress: 1 },
                CustomName(None),
                NameVisible(false),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .get::<CustomName>(rat)
                .and_then(|name| name.0.clone()),
            Some(rat_phase_display_name(&RatPhase::Transitioning {
                progress: 1
            }))
        );
        assert_eq!(
            app.world().get::<NameVisible>(rat),
            Some(&NameVisible(true))
        );
    }

    #[test]
    fn apply_rat_phase_change_synchronizes_full_chunk_group() {
        let mut app = App::new();
        app.add_event::<RatPhaseChangeEvent>();
        app.add_systems(Update, apply_rat_phase_change_system);
        let same_chunk_a = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([1.0, 64.0, 1.0]),
                RatGroupId(7),
                RatPhase::Solitary,
            ))
            .id();
        let same_chunk_b = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([2.0, 64.0, 2.0]),
                RatGroupId(7),
                RatPhase::Solitary,
            ))
            .id();
        let other_group = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([3.0, 64.0, 3.0]),
                RatGroupId(8),
                RatPhase::Solitary,
            ))
            .id();

        app.world_mut().send_event(RatPhaseChangeEvent {
            chunk: [0, 0],
            zone: "spawn".to_string(),
            group_id: 7,
            from: RatPhase::Solitary,
            to: RatPhase::Gregarious,
            rat_count: 2,
            local_qi: 0.6,
            qi_gradient: 0.4,
            tick: 9,
        });
        app.update();

        assert_eq!(
            app.world().get::<RatPhase>(same_chunk_a),
            Some(&RatPhase::Gregarious)
        );
        assert_eq!(
            app.world().get::<RatPhase>(same_chunk_b),
            Some(&RatPhase::Gregarious)
        );
        assert_eq!(
            app.world().get::<RatPhase>(other_group),
            Some(&RatPhase::Solitary)
        );
    }

    #[test]
    fn drained_chunk_avoid_still_works_in_gregarious_phase() {
        let chunk = ChunkPos::new(1, -1);
        let mut blackboard = rat_blackboard("spawn", chunk);
        remember_drained_chunk(&mut blackboard, chunk);

        assert!(is_drained_chunk(&blackboard, chunk));
    }
}
