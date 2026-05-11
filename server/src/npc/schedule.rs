//! NPC daily schedule layer.
//!
//! v1 keeps the "daily life" contract deliberately local to server NPC logic:
//! Near NPCs expose weighted activities to existing scorers/actions, while Far
//! and Dormant NPCs use coarse state ticks instead of full brain execution.

use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, App, BlockPos, Commands, Component, DVec3, Entity, Event, EventWriter,
    IntoSystemConfigs, Position, Query, Res, Update, With,
};

use crate::cultivation::components::{recover_current_qi, Cultivation};
use crate::cultivation::tick::CultivationClock;
use crate::npc::hunger::Hunger;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::lod::NpcLodTier;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::NpcMarker;
use crate::world::poi_novice::{PoiNoviceKind, PoiNoviceRegistry};
use crate::world::zone::ZoneRegistry;

pub const DAY_TICKS: u64 = 20 * 60 * 20;
pub const PHASE_OFFSET_LIMIT_TICKS: i32 = 200;
pub const FAR_SCHEDULE_TICK_INTERVAL: u64 = 20 * 60;
pub const DORMANT_SCHEDULE_TICK_INTERVAL: u64 = 20 * 60 * 10;
pub const DAILY_POI_SEARCH_RADIUS: f64 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DayPhase {
    Dawn,
    Day,
    Dusk,
    Night,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScheduleActivity {
    Forage,
    Cultivate,
    Trade,
    Patrol,
    Rest,
    Socialize,
    Wander,
}

#[derive(Clone, Debug, Component)]
pub struct NpcDailySchedule {
    pub phase_weights: HashMap<DayPhase, Vec<(ScheduleActivity, f32)>>,
    pub phase_offset_ticks: i32,
}

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct NpcScheduleState {
    pub last_phase: Option<DayPhase>,
    pub current_activity: Option<ScheduleActivity>,
}

#[derive(Clone, Copy, Debug, Component, PartialEq)]
pub struct NpcHomeBase {
    pub pos: BlockPos,
    pub quality: f32,
}

#[derive(Clone, Copy, Debug, Event, PartialEq, Eq)]
pub struct NpcScheduleChangedEvent {
    pub entity: Entity,
    pub previous_phase: Option<DayPhase>,
    pub next_phase: DayPhase,
    pub tick: u64,
}

pub fn register(app: &mut App) {
    app.add_event::<NpcScheduleChangedEvent>().add_systems(
        Update,
        (
            schedule_phase_event_system,
            far_npc_schedule_tick_system.after(schedule_phase_event_system),
        ),
    );
}

pub fn day_phase(tick: u64, offset: i32) -> DayPhase {
    let bounded_offset = offset.clamp(-PHASE_OFFSET_LIMIT_TICKS, PHASE_OFFSET_LIMIT_TICKS);
    let day_tick = ((tick as i64 + bounded_offset as i64).rem_euclid(DAY_TICKS as i64)) as u64;
    match day_tick {
        0..=5999 => DayPhase::Dawn,
        6000..=14399 => DayPhase::Day,
        14400..=19199 => DayPhase::Dusk,
        _ => DayPhase::Night,
    }
}

impl NpcDailySchedule {
    pub fn for_archetype(archetype: NpcArchetype, seed: u64) -> Self {
        let phase_weights = match archetype {
            NpcArchetype::Rogue | NpcArchetype::Disciple => rogue_schedule_weights(),
            NpcArchetype::Commoner => commoner_schedule_weights(),
            NpcArchetype::Beast => beast_schedule_weights(),
            _ => fallback_schedule_weights(),
        };
        Self {
            phase_weights,
            phase_offset_ticks: deterministic_phase_offset(seed),
        }
    }

    pub fn phase(&self, tick: u64) -> DayPhase {
        day_phase(tick, self.phase_offset_ticks)
    }

    pub fn weight(&self, phase: DayPhase, activity: ScheduleActivity) -> f32 {
        self.phase_weights
            .get(&phase)
            .and_then(|weights| {
                weights
                    .iter()
                    .find_map(|(candidate, weight)| (*candidate == activity).then_some(*weight))
            })
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    }

    pub fn activity_for(&self, tick: u64, salt: u64) -> ScheduleActivity {
        let phase = self.phase(tick);
        let Some(weights) = self.phase_weights.get(&phase) else {
            return ScheduleActivity::Wander;
        };
        weighted_activity(weights, deterministic_unit(salt, tick))
            .unwrap_or(ScheduleActivity::Wander)
    }
}

impl NpcHomeBase {
    pub fn new(pos: BlockPos, quality: f32) -> Self {
        Self {
            pos,
            quality: quality.clamp(0.0, 1.0),
        }
    }

    pub fn from_world_pos(pos: DVec3, quality: f32) -> Self {
        Self::new(
            BlockPos::new(
                pos.x.round() as i32,
                pos.y.round() as i32,
                pos.z.round() as i32,
            ),
            quality,
        )
    }

    pub fn center(self) -> DVec3 {
        DVec3::new(
            f64::from(self.pos.x) + 0.5,
            f64::from(self.pos.y),
            f64::from(self.pos.z) + 0.5,
        )
    }
}

pub fn schedule_multiplier(
    schedule: Option<&NpcDailySchedule>,
    tier: Option<&NpcLodTier>,
    tick: u64,
    activity: ScheduleActivity,
) -> Option<f32> {
    let Some(schedule) = schedule else {
        return Some(1.0);
    };
    if !matches!(tier.copied().unwrap_or(NpcLodTier::Near), NpcLodTier::Near) {
        return Some(0.0);
    }
    Some(schedule.weight(schedule.phase(tick), activity))
}

pub fn scheduled_wander_score(
    schedule: Option<&NpcDailySchedule>,
    tier: Option<&NpcLodTier>,
    tick: u64,
    salt: u64,
    baseline: f32,
) -> Option<f32> {
    let Some(schedule) = schedule else {
        return Some(baseline);
    };
    if !matches!(tier.copied().unwrap_or(NpcLodTier::Near), NpcLodTier::Near) {
        return Some(0.0);
    }
    let phase = schedule.phase(tick);
    if schedule.activity_for(tick, salt) == ScheduleActivity::Wander {
        Some(baseline * schedule.weight(phase, ScheduleActivity::Wander))
    } else {
        Some(0.02)
    }
}

pub fn activity_poi_kinds(activity: ScheduleActivity) -> &'static [PoiNoviceKind] {
    match activity {
        ScheduleActivity::Forage => &[PoiNoviceKind::HerbPatch, PoiNoviceKind::SpiritHerbValley],
        ScheduleActivity::Cultivate => &[PoiNoviceKind::QiSpring],
        ScheduleActivity::Trade => &[PoiNoviceKind::TradeSpot, PoiNoviceKind::RogueVillage],
        ScheduleActivity::Rest => &[PoiNoviceKind::ShelterSpot],
        ScheduleActivity::Patrol => &[PoiNoviceKind::WaterSource, PoiNoviceKind::RogueVillage],
        ScheduleActivity::Socialize => &[PoiNoviceKind::RogueVillage, PoiNoviceKind::TradeSpot],
        ScheduleActivity::Wander => &[],
    }
}

pub fn nearest_poi_for_activity(
    registry: Option<&PoiNoviceRegistry>,
    origin: DVec3,
    activity: ScheduleActivity,
    radius: f64,
) -> Option<DVec3> {
    let kinds = activity_poi_kinds(activity);
    if kinds.is_empty() {
        return None;
    }
    registry?
        .nearest_by_kinds(origin, kinds, radius)
        .map(|site| site.position_vec())
}

pub fn rest_tick(
    hunger: Option<&mut Hunger>,
    cultivation: Option<&mut Cultivation>,
    home_quality: f32,
    base_hunger_restore: f64,
) {
    let quality = f64::from(home_quality.clamp(0.0, 1.0));
    if let Some(hunger) = hunger {
        hunger.replenish(base_hunger_restore.max(0.0) * 2.0 * (0.5 + quality));
    }
    if let Some(cultivation) = cultivation {
        let qi_restore = cultivation.qi_max.max(0.0) * 0.0015 * (0.5 + quality);
        recover_current_qi(cultivation, qi_restore);
    }
}

pub fn far_activity_tick(
    activity: ScheduleActivity,
    hunger: Option<&mut Hunger>,
    cultivation: Option<&mut Cultivation>,
    zone_qi: f64,
) {
    match activity {
        ScheduleActivity::Forage => {
            if let Some(hunger) = hunger {
                hunger.replenish(0.05);
            }
        }
        ScheduleActivity::Cultivate => {
            if let Some(cultivation) = cultivation {
                recover_current_qi(cultivation, zone_qi.max(0.0) * 0.01);
            }
        }
        ScheduleActivity::Rest => {
            if let Some(hunger) = hunger {
                hunger.replenish(0.02);
            }
        }
        ScheduleActivity::Trade
        | ScheduleActivity::Patrol
        | ScheduleActivity::Socialize
        | ScheduleActivity::Wander => {}
    }
}

#[cfg(test)]
pub fn dormant_minimal_tick(
    hunger: Option<&mut Hunger>,
    lifespan: &mut crate::npc::lifecycle::NpcLifespan,
    elapsed_ticks: u64,
) {
    if let Some(hunger) = hunger {
        hunger.consume(0.1);
        if hunger.value <= 0.0 {
            hunger.set(0.3);
        }
    }
    lifespan.age_ticks += elapsed_ticks as f64;
}

pub fn hydrate_position_for(
    schedule: &NpcDailySchedule,
    home: Option<NpcHomeBase>,
    current_pos: DVec3,
    tick: u64,
    salt: u64,
    registry: Option<&PoiNoviceRegistry>,
) -> DVec3 {
    match schedule.activity_for(tick, salt) {
        ScheduleActivity::Rest if schedule.phase(tick) == DayPhase::Night => {
            home.map(NpcHomeBase::center).unwrap_or(current_pos)
        }
        ScheduleActivity::Forage if schedule.phase(tick) == DayPhase::Dawn => {
            nearest_poi_for_activity(
                registry,
                current_pos,
                ScheduleActivity::Forage,
                DAILY_POI_SEARCH_RADIUS,
            )
            .unwrap_or(current_pos)
        }
        _ => current_pos,
    }
}

#[allow(clippy::type_complexity)]
fn schedule_phase_event_system(
    mut commands: Commands,
    clock: Option<Res<CultivationClock>>,
    mut events: EventWriter<NpcScheduleChangedEvent>,
    mut npcs: Query<
        (
            Entity,
            &NpcDailySchedule,
            Option<&NpcLodTier>,
            Option<&mut NpcScheduleState>,
        ),
        With<NpcMarker>,
    >,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (entity, schedule, tier, state) in &mut npcs {
        if !matches!(tier.copied().unwrap_or(NpcLodTier::Near), NpcLodTier::Near) {
            continue;
        }
        let next_phase = schedule.phase(tick);
        let activity = schedule.activity_for(tick, u64::from(entity.index()));
        match state {
            Some(mut state) => {
                if state.last_phase != Some(next_phase) {
                    events.send(NpcScheduleChangedEvent {
                        entity,
                        previous_phase: state.last_phase,
                        next_phase,
                        tick,
                    });
                    state.last_phase = Some(next_phase);
                }
                state.current_activity = Some(activity);
            }
            None => {
                commands.entity(entity).insert(NpcScheduleState {
                    last_phase: Some(next_phase),
                    current_activity: Some(activity),
                });
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn far_npc_schedule_tick_system(
    clock: Option<Res<CultivationClock>>,
    zones: Option<Res<ZoneRegistry>>,
    pois: Option<Res<PoiNoviceRegistry>>,
    mut npcs: Query<
        (
            Entity,
            &NpcDailySchedule,
            Option<&NpcLodTier>,
            Option<&NpcHomeBase>,
            Option<&NpcPatrol>,
            Option<&mut Hunger>,
            Option<&mut Cultivation>,
            Option<&mut Position>,
        ),
        With<NpcMarker>,
    >,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    if tick % FAR_SCHEDULE_TICK_INTERVAL != 0 {
        return;
    }

    for (entity, schedule, tier, home, patrol, hunger, cultivation, position) in &mut npcs {
        if !matches!(tier.copied().unwrap_or(NpcLodTier::Near), NpcLodTier::Far) {
            continue;
        }

        let activity = schedule.activity_for(tick, u64::from(entity.index()));
        let zone_qi = patrol
            .and_then(|patrol| {
                zones
                    .as_deref()
                    .and_then(|zones| zones.find_zone_by_name(&patrol.home_zone))
            })
            .map(|zone| zone.spirit_qi)
            .unwrap_or_default();

        let mut hunger = hunger;
        let mut cultivation = cultivation;
        far_activity_tick(
            activity,
            hunger.as_deref_mut(),
            cultivation.as_deref_mut(),
            zone_qi,
        );

        if let Some(mut position) = position {
            let current = position.get();
            let target = match activity {
                ScheduleActivity::Rest => home.map(|home| home.center()),
                _ => nearest_poi_for_activity(
                    pois.as_deref(),
                    current,
                    activity,
                    DAILY_POI_SEARCH_RADIUS,
                ),
            };
            if let Some(target) = target {
                let next = drift_toward(current, target, 5.0);
                position.set([next.x, next.y, next.z]);
            }
        }
    }
}

fn drift_toward(current: DVec3, target: DVec3, max_step: f64) -> DVec3 {
    let dx = target.x - current.x;
    let dz = target.z - current.z;
    let distance = (dx * dx + dz * dz).sqrt();
    if distance <= f64::EPSILON || distance <= max_step {
        return DVec3::new(target.x, current.y, target.z);
    }
    DVec3::new(
        current.x + dx / distance * max_step,
        current.y,
        current.z + dz / distance * max_step,
    )
}

fn rogue_schedule_weights() -> HashMap<DayPhase, Vec<(ScheduleActivity, f32)>> {
    HashMap::from([
        (
            DayPhase::Dawn,
            vec![
                (ScheduleActivity::Forage, 0.5),
                (ScheduleActivity::Cultivate, 0.3),
                (ScheduleActivity::Wander, 0.2),
            ],
        ),
        (
            DayPhase::Day,
            vec![
                (ScheduleActivity::Trade, 0.3),
                (ScheduleActivity::Forage, 0.3),
                (ScheduleActivity::Cultivate, 0.2),
                (ScheduleActivity::Socialize, 0.1),
                (ScheduleActivity::Wander, 0.1),
            ],
        ),
        (
            DayPhase::Dusk,
            vec![
                (ScheduleActivity::Rest, 0.4),
                (ScheduleActivity::Forage, 0.3),
                (ScheduleActivity::Cultivate, 0.2),
                (ScheduleActivity::Wander, 0.1),
            ],
        ),
        (
            DayPhase::Night,
            vec![
                (ScheduleActivity::Rest, 0.6),
                (ScheduleActivity::Cultivate, 0.3),
                (ScheduleActivity::Patrol, 0.1),
            ],
        ),
    ])
}

fn commoner_schedule_weights() -> HashMap<DayPhase, Vec<(ScheduleActivity, f32)>> {
    HashMap::from([
        (
            DayPhase::Dawn,
            vec![
                (ScheduleActivity::Forage, 0.4),
                (ScheduleActivity::Trade, 0.2),
                (ScheduleActivity::Wander, 0.4),
            ],
        ),
        (
            DayPhase::Day,
            vec![
                (ScheduleActivity::Trade, 0.4),
                (ScheduleActivity::Socialize, 0.2),
                (ScheduleActivity::Forage, 0.2),
                (ScheduleActivity::Wander, 0.2),
            ],
        ),
        (
            DayPhase::Dusk,
            vec![
                (ScheduleActivity::Rest, 0.5),
                (ScheduleActivity::Socialize, 0.2),
                (ScheduleActivity::Wander, 0.3),
            ],
        ),
        (
            DayPhase::Night,
            vec![
                (ScheduleActivity::Rest, 0.8),
                (ScheduleActivity::Wander, 0.2),
            ],
        ),
    ])
}

fn beast_schedule_weights() -> HashMap<DayPhase, Vec<(ScheduleActivity, f32)>> {
    HashMap::from([
        (
            DayPhase::Dawn,
            vec![
                (ScheduleActivity::Patrol, 0.6),
                (ScheduleActivity::Forage, 0.4),
            ],
        ),
        (
            DayPhase::Day,
            vec![
                (ScheduleActivity::Patrol, 0.6),
                (ScheduleActivity::Wander, 0.4),
            ],
        ),
        (
            DayPhase::Dusk,
            vec![
                (ScheduleActivity::Rest, 0.4),
                (ScheduleActivity::Patrol, 0.6),
            ],
        ),
        (
            DayPhase::Night,
            vec![
                (ScheduleActivity::Rest, 0.5),
                (ScheduleActivity::Patrol, 0.5),
            ],
        ),
    ])
}

fn fallback_schedule_weights() -> HashMap<DayPhase, Vec<(ScheduleActivity, f32)>> {
    HashMap::from([
        (DayPhase::Dawn, vec![(ScheduleActivity::Wander, 1.0)]),
        (DayPhase::Day, vec![(ScheduleActivity::Wander, 1.0)]),
        (DayPhase::Dusk, vec![(ScheduleActivity::Wander, 1.0)]),
        (DayPhase::Night, vec![(ScheduleActivity::Rest, 1.0)]),
    ])
}

fn deterministic_phase_offset(seed: u64) -> i32 {
    let span = i64::from(PHASE_OFFSET_LIMIT_TICKS) * 2 + 1;
    ((deterministic_hash(seed, 0x64_61_79) % span as u64) as i64
        - i64::from(PHASE_OFFSET_LIMIT_TICKS)) as i32
}

fn weighted_activity(weights: &[(ScheduleActivity, f32)], unit: f64) -> Option<ScheduleActivity> {
    let total = weights
        .iter()
        .map(|(_, weight)| weight.max(0.0))
        .sum::<f32>();
    if total <= f32::EPSILON {
        return None;
    }
    let mut cursor = (unit.clamp(0.0, 0.999_999) as f32) * total;
    for (activity, weight) in weights {
        cursor -= weight.max(0.0);
        if cursor <= 0.0 {
            return Some(*activity);
        }
    }
    weights.last().map(|(activity, _)| *activity)
}

fn deterministic_unit(seed: u64, tick: u64) -> f64 {
    (deterministic_hash(seed, tick) & 0xffff) as f64 / 65_535.0
}

fn deterministic_hash(seed: u64, salt: u64) -> u64 {
    seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt.rotate_left(17))
        .wrapping_mul(0xbf58_476d_1ce4_e5b9)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::lifecycle::NpcLifespan;

    #[test]
    fn day_phase_boundaries() {
        assert_eq!(day_phase(0, 0), DayPhase::Dawn);
        assert_eq!(day_phase(5999, 0), DayPhase::Dawn);
        assert_eq!(day_phase(6000, 0), DayPhase::Day);
        assert_eq!(day_phase(14399, 0), DayPhase::Day);
        assert_eq!(day_phase(14400, 0), DayPhase::Dusk);
        assert_eq!(day_phase(19199, 0), DayPhase::Dusk);
        assert_eq!(day_phase(19200, 0), DayPhase::Night);
        assert_eq!(day_phase(23999, 0), DayPhase::Night);
    }

    #[test]
    fn phase_offset_shifts_boundary() {
        assert_eq!(day_phase(5990, 20), DayPhase::Day);
        assert_eq!(day_phase(10, -20), DayPhase::Night);
    }

    #[test]
    fn wander_score_modulated_by_phase() {
        let mut schedule = NpcDailySchedule::for_archetype(NpcArchetype::Rogue, 0);
        schedule.phase_offset_ticks = 0;
        let score =
            scheduled_wander_score(Some(&schedule), Some(&NpcLodTier::Near), 20_000, 1, 0.08)
                .expect("scheduled NPC should produce a score");
        assert!(
            score < 0.08,
            "Night wander score should be lower, got {score}"
        );
    }

    #[test]
    fn cultivate_score_high_at_night() {
        let mut schedule = NpcDailySchedule::for_archetype(NpcArchetype::Rogue, 0);
        schedule.phase_offset_ticks = 0;
        assert_eq!(
            schedule.weight(DayPhase::Night, ScheduleActivity::Cultivate),
            0.3
        );
    }

    #[test]
    fn far_tick_updates_hunger() {
        let mut hunger = Hunger::new(0.2);
        far_activity_tick(ScheduleActivity::Forage, Some(&mut hunger), None, 0.0);
        assert!((hunger.value - 0.25).abs() < 1e-6);
    }

    #[test]
    fn dormant_tick_advances_lifespan() {
        let mut hunger = Hunger::new(0.05);
        let mut lifespan = NpcLifespan::new(10.0, 1_000.0);
        dormant_minimal_tick(
            Some(&mut hunger),
            &mut lifespan,
            DORMANT_SCHEDULE_TICK_INTERVAL,
        );
        assert_eq!(hunger.value, 0.3);
        assert_eq!(
            lifespan.age_ticks,
            10.0 + DORMANT_SCHEDULE_TICK_INTERVAL as f64
        );
    }

    #[test]
    fn hydrate_at_night_spawns_near_home() {
        let mut schedule = NpcDailySchedule::for_archetype(NpcArchetype::Rogue, 0);
        schedule.phase_offset_ticks = 0;
        schedule
            .phase_weights
            .insert(DayPhase::Night, vec![(ScheduleActivity::Rest, 1.0)]);
        let home = NpcHomeBase::from_world_pos(DVec3::new(20.0, 66.0, 30.0), 0.8);
        let pos = hydrate_position_for(
            &schedule,
            Some(home),
            DVec3::new(0.0, 66.0, 0.0),
            20_000,
            0,
            None,
        );
        assert_eq!(pos, home.center());
    }

    #[test]
    fn hydrate_at_dawn_spawns_near_herb_poi() {
        let mut registry = PoiNoviceRegistry::default();
        registry.replace_all(vec![crate::world::poi_novice::PoiNoviceSite {
            id: "spawn:herb_patch".to_string(),
            kind: PoiNoviceKind::HerbPatch,
            zone: "spawn".to_string(),
            name: "晨露灵草".to_string(),
            pos_xyz: [12.0, 66.0, 8.0],
            selection_strategy: "test".to_string(),
            qi_affinity: 0.4,
            danger_bias: 0,
            tags: Vec::new(),
        }]);
        let mut schedule = NpcDailySchedule::for_archetype(NpcArchetype::Rogue, 0);
        schedule.phase_offset_ticks = 0;
        schedule
            .phase_weights
            .insert(DayPhase::Dawn, vec![(ScheduleActivity::Forage, 1.0)]);
        let pos = hydrate_position_for(
            &schedule,
            None,
            DVec3::new(0.0, 66.0, 0.0),
            1,
            0,
            Some(&registry),
        );
        assert_eq!(pos, DVec3::new(12.0, 66.0, 8.0));
    }

    #[test]
    fn schedule_matrix_pins_all_phase_activity_archetype_combinations() {
        let archetypes = [
            NpcArchetype::Rogue,
            NpcArchetype::Commoner,
            NpcArchetype::Beast,
        ];
        let phases = [
            DayPhase::Dawn,
            DayPhase::Day,
            DayPhase::Dusk,
            DayPhase::Night,
        ];
        let activities = [
            ScheduleActivity::Forage,
            ScheduleActivity::Cultivate,
            ScheduleActivity::Trade,
            ScheduleActivity::Patrol,
            ScheduleActivity::Rest,
            ScheduleActivity::Socialize,
            ScheduleActivity::Wander,
        ];
        let mut checked = 0;
        for archetype in archetypes {
            let schedule = NpcDailySchedule::for_archetype(archetype, 0);
            for phase in phases {
                for activity in activities {
                    assert!(schedule.weight(phase, activity).is_finite());
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 84);
    }
}
