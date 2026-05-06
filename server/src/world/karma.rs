//! 定向天罚内部权重（plan-tribulation-v1 §5）。
//!
//! 本模块只维护 server 内部数据契约：`KarmaWeightStore` 记录玩家/区域劫气，
//! `QiDensityHeatmap` 记录区域灵物密度热度。它们不注册任何查询接口，避免把
//! 定向天罚明牌化；后续负面事件 roll 只需读取这些 resource 叠权重。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, BlockPos, Position, Query, Res, ResMut, Resource, Username};

use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::tick::CultivationClock;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::season::Season;
use crate::world::zone::ZoneRegistry;

pub const KARMA_WEIGHT_MIN: f32 = 0.0;
pub const KARMA_WEIGHT_MAX: f32 = 1.0;
#[allow(dead_code)] // 后续负面事件 roll 接入前，仅作为内部契约锚点。
pub const KARMA_WEIGHT_DECAY_PER_TICK: f32 = 1.0 / (30.0 * 24.0 * 60.0 * 60.0 * 20.0);
pub const QI_DENSITY_HEAT_MAX: f32 = 1.0;
pub const QI_DENSITY_CELL_SIZE: i32 = 16;
pub const TARGETED_CALAMITY_BASE_PROBABILITY: f32 = 0.05;
pub const TARGETED_CALAMITY_MAX_PROBABILITY: f32 = 0.30;
pub const TARGETED_QI_NULLIFICATION_HEAT_THRESHOLD: f32 = 0.80;
pub const VOID_REALM_KARMA_WEIGHT_FLOOR: f32 = 0.20;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetedCalamityRoll {
    pub base_probability: f32,
    pub karma_weight: f32,
    pub qi_density_heat: f32,
    pub effective_probability: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KarmaWeightEntry {
    pub player_id: String,
    pub zone: Option<String>,
    pub weight: f32,
    pub last_position: [i32; 3],
    pub last_tick: u64,
    pub decay_checkpoint_tick: u64,
}

#[derive(Debug, Default, Resource)]
pub struct KarmaWeightStore {
    by_player: HashMap<String, KarmaWeightEntry>,
}

impl KarmaWeightStore {
    pub fn mark_player(
        &mut self,
        player_id: impl Into<String>,
        zone: Option<String>,
        position: BlockPos,
        weight_delta: f32,
        tick: u64,
    ) {
        let player_id = player_id.into();
        let normalized_delta = weight_delta.clamp(KARMA_WEIGHT_MIN, KARMA_WEIGHT_MAX);
        let entry = self
            .by_player
            .entry(player_id.clone())
            .or_insert_with(|| KarmaWeightEntry {
                player_id,
                zone: zone.clone(),
                weight: KARMA_WEIGHT_MIN,
                last_position: [position.x, position.y, position.z],
                last_tick: tick,
                decay_checkpoint_tick: tick,
            });
        decay_entry_to(entry, tick);
        entry.zone = zone;
        entry.last_position = [position.x, position.y, position.z];
        entry.last_tick = tick;
        entry.weight = (entry.weight + normalized_delta).clamp(KARMA_WEIGHT_MIN, KARMA_WEIGHT_MAX);
    }

    pub fn mark_player_floor(
        &mut self,
        player_id: impl Into<String>,
        zone: Option<String>,
        position: BlockPos,
        weight_floor: f32,
        tick: u64,
    ) {
        let player_id = player_id.into();
        let normalized_floor = weight_floor.clamp(KARMA_WEIGHT_MIN, KARMA_WEIGHT_MAX);
        let entry = self
            .by_player
            .entry(player_id.clone())
            .or_insert_with(|| KarmaWeightEntry {
                player_id,
                zone: zone.clone(),
                weight: KARMA_WEIGHT_MIN,
                last_position: [position.x, position.y, position.z],
                last_tick: tick,
                decay_checkpoint_tick: tick,
            });
        decay_entry_to(entry, tick);
        entry.zone = zone;
        entry.last_position = [position.x, position.y, position.z];
        entry.last_tick = tick;
        entry.weight = entry.weight.max(normalized_floor);
    }

    pub fn weight_for_player(&self, player_id: &str) -> f32 {
        self.by_player
            .get(player_id)
            .map(|entry| entry.weight)
            .unwrap_or(KARMA_WEIGHT_MIN)
    }

    pub fn weight_for_zone(&self, zone: &str) -> f32 {
        self.by_player
            .values()
            .filter(|entry| entry.zone.as_deref() == Some(zone))
            .map(|entry| entry.weight)
            .fold(KARMA_WEIGHT_MIN, f32::max)
    }

    pub fn strongest_entry_for_zone(&self, zone: &str) -> Option<&KarmaWeightEntry> {
        self.by_player
            .values()
            .filter(|entry| entry.zone.as_deref() == Some(zone))
            .max_by(|left, right| left.weight.total_cmp(&right.weight))
    }

    pub fn entry_for_player(&self, player_id: &str) -> Option<&KarmaWeightEntry> {
        self.by_player.get(player_id)
    }

    pub fn decay_to(&mut self, tick: u64) {
        self.by_player.retain(|_, entry| {
            decay_entry_to(entry, tick);
            entry.weight > KARMA_WEIGHT_MIN
        });
    }
}

fn decay_entry_to(entry: &mut KarmaWeightEntry, tick: u64) {
    let ticks = tick.saturating_sub(entry.decay_checkpoint_tick);
    if ticks == 0 {
        return;
    }
    let amount = KARMA_WEIGHT_DECAY_PER_TICK * ticks as f32;
    entry.weight = (entry.weight - amount).max(KARMA_WEIGHT_MIN);
    entry.decay_checkpoint_tick = tick;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QiDensityCell {
    pub dimension: DimensionKind,
    pub x: i32,
    pub z: i32,
}

#[derive(Debug, Default, Resource)]
pub struct QiDensityHeatmap {
    by_cell: HashMap<QiDensityCell, f32>,
}

impl QiDensityHeatmap {
    pub fn add_heat(&mut self, dimension: DimensionKind, position: BlockPos, heat_delta: f32) {
        let cell = QiDensityCell::from_position(dimension, position);
        let entry = self.by_cell.entry(cell).or_insert(0.0);
        *entry = (*entry + heat_delta.max(0.0)).min(QI_DENSITY_HEAT_MAX);
    }

    pub fn heat_at(&self, dimension: DimensionKind, position: BlockPos) -> f32 {
        self.by_cell
            .get(&QiDensityCell::from_position(dimension, position))
            .copied()
            .unwrap_or(0.0)
    }

    pub fn max_heat_in_rect(
        &self,
        dimension: DimensionKind,
        min_x: i32,
        max_x: i32,
        min_z: i32,
        max_z: i32,
    ) -> f32 {
        let (min_x, max_x) = if min_x <= max_x {
            (min_x, max_x)
        } else {
            (max_x, min_x)
        };
        let (min_z, max_z) = if min_z <= max_z {
            (min_z, max_z)
        } else {
            (max_z, min_z)
        };
        let min_cell_x = min_x.div_euclid(QI_DENSITY_CELL_SIZE);
        let max_cell_x = max_x.div_euclid(QI_DENSITY_CELL_SIZE);
        let min_cell_z = min_z.div_euclid(QI_DENSITY_CELL_SIZE);
        let max_cell_z = max_z.div_euclid(QI_DENSITY_CELL_SIZE);

        self.by_cell
            .iter()
            .filter(|(cell, _)| {
                cell.dimension == dimension
                    && cell.x >= min_cell_x
                    && cell.x <= max_cell_x
                    && cell.z >= min_cell_z
                    && cell.z <= max_cell_z
            })
            .map(|(_, heat)| *heat)
            .fold(0.0, f32::max)
    }
}

impl QiDensityCell {
    pub fn from_position(dimension: DimensionKind, position: BlockPos) -> Self {
        Self {
            dimension,
            x: position.x.div_euclid(QI_DENSITY_CELL_SIZE),
            z: position.z.div_euclid(QI_DENSITY_CELL_SIZE),
        }
    }
}

pub fn targeted_calamity_roll(
    base_probability: f32,
    karma_weight: f32,
    qi_density_heat: f32,
) -> TargetedCalamityRoll {
    let base_probability = base_probability.clamp(0.0, TARGETED_CALAMITY_MAX_PROBABILITY);
    let karma_weight = karma_weight.clamp(KARMA_WEIGHT_MIN, KARMA_WEIGHT_MAX);
    let qi_density_heat = qi_density_heat.clamp(0.0, QI_DENSITY_HEAT_MAX);
    let pressure = karma_weight.max(qi_density_heat);
    let effective_probability = (base_probability
        + (TARGETED_CALAMITY_MAX_PROBABILITY - base_probability) * pressure)
        .clamp(0.0, TARGETED_CALAMITY_MAX_PROBABILITY);

    TargetedCalamityRoll {
        base_probability,
        karma_weight,
        qi_density_heat,
        effective_probability,
    }
}

pub fn season_calamity_multiplier(season: Season) -> f32 {
    if season.is_xizhuan() {
        2.0
    } else {
        1.0
    }
}

pub fn targeted_calamity_roll_with_season(
    base_probability: f32,
    karma_weight: f32,
    qi_density_heat: f32,
    season: Season,
) -> TargetedCalamityRoll {
    let adjusted_base = (base_probability * season_calamity_multiplier(season))
        .clamp(0.0, TARGETED_CALAMITY_MAX_PROBABILITY);
    targeted_calamity_roll(adjusted_base, karma_weight, qi_density_heat)
}

pub fn targeted_calamity_event_hit(effective_probability: f32, seed: u64) -> (f32, bool) {
    let probability = effective_probability.clamp(0.0, 1.0);
    let roll_value = targeted_calamity_roll_value(seed);
    (roll_value, roll_value < probability)
}

pub fn karma_weight_decay_tick(
    clock: Res<CultivationClock>,
    mut store: Option<ResMut<KarmaWeightStore>>,
) {
    let Some(store) = store.as_deref_mut() else {
        return;
    };
    store.decay_to(clock.tick);
}

#[allow(clippy::type_complexity)]
pub fn void_realm_karma_pressure_tick(
    clock: Res<CultivationClock>,
    mut store: Option<ResMut<KarmaWeightStore>>,
    cultivators: Query<(
        &Cultivation,
        Option<&Username>,
        Option<&LifeRecord>,
        &Position,
        Option<&CurrentDimension>,
    )>,
    zones: Option<Res<ZoneRegistry>>,
) {
    let Some(store) = store.as_deref_mut() else {
        return;
    };

    for (cultivation, username, life_record, position, current_dimension) in &cultivators {
        if cultivation.realm != Realm::Void {
            continue;
        }
        let Some(player_id) = void_pressure_player_id(username, life_record) else {
            continue;
        };
        let dimension = current_dimension
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        let position_vec = position.get();
        let zone_name = zones.as_deref().and_then(|registry| {
            registry
                .find_zone(dimension, position_vec)
                .map(|zone| zone.name.clone())
        });

        store.mark_player_floor(
            player_id,
            zone_name,
            block_pos_from_position(position),
            VOID_REALM_KARMA_WEIGHT_FLOOR,
            clock.tick,
        );
    }
}

fn void_pressure_player_id(
    username: Option<&Username>,
    life_record: Option<&LifeRecord>,
) -> Option<String> {
    username
        .map(|name| name.0.clone())
        .or_else(|| life_record.map(|life| life.character_id.clone()))
        .filter(|id| !id.trim().is_empty())
}

fn block_pos_from_position(position: &Position) -> BlockPos {
    let p = position.get();
    BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
}

fn targeted_calamity_roll_value(seed: u64) -> f32 {
    const ROLL_BUCKETS: u64 = 10_000;
    (splitmix64(seed) % ROLL_BUCKETS) as f32 / ROLL_BUCKETS as f32
}

fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Update};

    #[test]
    fn karma_weight_accumulates_and_clamps_per_player() {
        let mut store = KarmaWeightStore::default();
        let pos = BlockPos::new(1, 64, 2);

        store.mark_player("Azure", Some("spawn".to_string()), pos, 0.30, 10);
        store.mark_player("Azure", Some("spawn".to_string()), pos, 0.80, 11);

        let entry = store.entry_for_player("Azure").expect("entry should exist");
        assert_eq!(entry.weight, KARMA_WEIGHT_MAX);
        assert_eq!(entry.zone.as_deref(), Some("spawn"));
        assert_eq!(entry.last_position, [1, 64, 2]);
        assert_eq!(entry.last_tick, 11);
        assert_eq!(entry.decay_checkpoint_tick, 11);
        assert_eq!(store.weight_for_zone("spawn"), KARMA_WEIGHT_MAX);
    }

    #[test]
    fn karma_weight_floor_refreshes_without_lowering_existing_pressure() {
        let mut store = KarmaWeightStore::default();
        let pos = BlockPos::new(1, 64, 2);

        store.mark_player("Azure", Some("spawn".to_string()), pos, 0.80, 10);
        store.mark_player_floor("Azure", Some("spawn".to_string()), pos, 0.20, 11);

        assert!(store.weight_for_player("Azure") > 0.79);
    }

    #[test]
    fn karma_weight_decays_to_zero_and_expires() {
        let mut store = KarmaWeightStore::default();
        store.mark_player("Azure", None, BlockPos::new(0, 64, 0), 0.01, 1);

        store.decay_to(1 + 30 * 24 * 60 * 60 * 20);

        assert_eq!(store.weight_for_player("Azure"), 0.0);
        assert!(store.entry_for_player("Azure").is_none());
    }

    #[test]
    fn karma_weight_decay_to_only_applies_elapsed_ticks_once() {
        let mut store = KarmaWeightStore::default();
        store.mark_player("Azure", None, BlockPos::new(0, 64, 0), 0.50, 1);

        store.decay_to(10);
        let after_first = store.weight_for_player("Azure");
        store.decay_to(10);

        assert_eq!(store.weight_for_player("Azure"), after_first);
    }

    fn setup_void_pressure_app(now: u64) -> App {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: now });
        app.insert_resource(KarmaWeightStore::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, void_realm_karma_pressure_tick);
        app
    }

    #[test]
    fn void_realm_cultivator_refreshes_hidden_karma_pressure_floor() {
        let mut app = setup_void_pressure_app(42);
        app.world_mut().spawn((
            Cultivation {
                realm: Realm::Void,
                ..Default::default()
            },
            Username("Azure".to_string()),
            Position::new([8.8, 66.2, 8.1]),
        ));

        app.update();

        let weights = app.world().resource::<KarmaWeightStore>();
        let entry = weights
            .entry_for_player("Azure")
            .expect("Void cultivator should refresh hidden karma pressure");
        assert_eq!(entry.weight, VOID_REALM_KARMA_WEIGHT_FLOOR);
        assert_eq!(entry.zone.as_deref(), Some("spawn"));
        assert_eq!(entry.last_position, [8, 66, 8]);
        assert_eq!(entry.last_tick, 42);
    }

    #[test]
    fn non_void_cultivator_does_not_refresh_karma_pressure() {
        let mut app = setup_void_pressure_app(42);
        app.world_mut().spawn((
            Cultivation {
                realm: Realm::Spirit,
                ..Default::default()
            },
            Username("Azure".to_string()),
            Position::new([8.0, 66.0, 8.0]),
        ));

        app.update();

        let weights = app.world().resource::<KarmaWeightStore>();
        assert!(weights.entry_for_player("Azure").is_none());
    }

    #[test]
    fn void_realm_pressure_floor_does_not_accumulate_each_tick() {
        let mut app = setup_void_pressure_app(42);
        app.world_mut().spawn((
            Cultivation {
                realm: Realm::Void,
                ..Default::default()
            },
            Username("Azure".to_string()),
            Position::new([8.0, 66.0, 8.0]),
        ));

        app.update();
        app.world_mut().resource_mut::<CultivationClock>().tick = 43;
        app.update();

        let weights = app.world().resource::<KarmaWeightStore>();
        assert_eq!(
            weights.weight_for_player("Azure"),
            VOID_REALM_KARMA_WEIGHT_FLOOR
        );
    }

    #[test]
    fn qi_density_heatmap_buckets_by_dimension_and_chunk_cell() {
        let mut heatmap = QiDensityHeatmap::default();

        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(31, 64, -1), 0.25);
        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(16, 70, -16), 0.85);
        heatmap.add_heat(DimensionKind::Tsy, BlockPos::new(31, 64, -1), 0.40);

        assert_eq!(
            heatmap.heat_at(DimensionKind::Overworld, BlockPos::new(20, 64, -8)),
            QI_DENSITY_HEAT_MAX
        );
        assert_eq!(
            heatmap.heat_at(DimensionKind::Tsy, BlockPos::new(20, 64, -8)),
            0.40
        );
    }

    #[test]
    fn qi_density_heatmap_reports_max_heat_in_rect() {
        let mut heatmap = QiDensityHeatmap::default();

        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(8, 64, 8), 0.30);
        heatmap.add_heat(DimensionKind::Overworld, BlockPos::new(40, 64, 40), 0.75);
        heatmap.add_heat(DimensionKind::Tsy, BlockPos::new(8, 64, 8), 1.0);

        assert_eq!(
            heatmap.max_heat_in_rect(DimensionKind::Overworld, 0, 48, 0, 48),
            0.75
        );
        assert_eq!(
            heatmap.max_heat_in_rect(DimensionKind::Overworld, 48, 0, 48, 0),
            0.75,
            "reversed bounds should be normalized"
        );
        assert_eq!(
            heatmap.max_heat_in_rect(DimensionKind::Overworld, 64, 80, 64, 80),
            0.0
        );
    }

    #[test]
    fn targeted_calamity_roll_scales_base_probability_to_hidden_max() {
        let low = targeted_calamity_roll(TARGETED_CALAMITY_BASE_PROBABILITY, 0.0, 0.0);
        assert_eq!(
            low.effective_probability,
            TARGETED_CALAMITY_BASE_PROBABILITY
        );

        let high = targeted_calamity_roll(TARGETED_CALAMITY_BASE_PROBABILITY, 1.0, 0.2);
        assert_eq!(
            high.effective_probability,
            TARGETED_CALAMITY_MAX_PROBABILITY
        );

        let heat_driven = targeted_calamity_roll(TARGETED_CALAMITY_BASE_PROBABILITY, 0.1, 0.6);
        assert!(heat_driven.effective_probability > TARGETED_CALAMITY_BASE_PROBABILITY);
        assert!(heat_driven.effective_probability < TARGETED_CALAMITY_MAX_PROBABILITY);
    }

    #[test]
    fn targeted_calamity_in_xizhuan_doubles_probability() {
        let normal = targeted_calamity_roll_with_season(
            TARGETED_CALAMITY_BASE_PROBABILITY,
            0.0,
            0.0,
            Season::Summer,
        );
        let xizhuan = targeted_calamity_roll_with_season(
            TARGETED_CALAMITY_BASE_PROBABILITY,
            0.0,
            0.0,
            Season::WinterToSummer,
        );

        assert_eq!(
            normal.effective_probability,
            TARGETED_CALAMITY_BASE_PROBABILITY
        );
        assert_eq!(
            xizhuan.effective_probability,
            TARGETED_CALAMITY_BASE_PROBABILITY * 2.0
        );
        assert_eq!(season_calamity_multiplier(Season::SummerToWinter), 2.0);
    }

    #[test]
    fn targeted_calamity_season_multiplier_clamps_to_max() {
        let xizhuan = targeted_calamity_roll_with_season(
            TARGETED_CALAMITY_MAX_PROBABILITY,
            1.0,
            1.0,
            Season::SummerToWinter,
        );

        assert_eq!(
            xizhuan.effective_probability,
            TARGETED_CALAMITY_MAX_PROBABILITY
        );
        assert_eq!(xizhuan.base_probability, TARGETED_CALAMITY_MAX_PROBABILITY);
    }

    #[test]
    fn targeted_calamity_event_hit_uses_effective_probability() {
        let seed = 42;
        let (roll_value, miss) = targeted_calamity_event_hit(0.0, seed);
        assert!((0.0..1.0).contains(&roll_value));
        assert!(!miss);

        let (same_roll_value, hit) = targeted_calamity_event_hit(1.0, seed);
        assert_eq!(roll_value, same_roll_value);
        assert!(hit);
    }
}
