//! TSY (坍缩渊) dimension registration and per-player dimension tracking.
//!
//! See `docs/plan-tsy-dimension-v1.md` §1 for the design rationale.

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, ident, App, Component, DimensionType, DimensionTypeRegistry, Entity, PreStartup,
    ResMut, Resource,
};
use valence::registry::dimension_type::{DimensionEffects, MonsterSpawnLightLevel};

/// Identifier of the TSY dimension in `DimensionTypeRegistry`.
#[allow(dead_code)] // Consumed by P0 / worldgen plans; kept for symmetry with `bong:tsy` ident usage.
pub const TSY_DIMENSION_IDENT: &str = "bong:tsy";

/// Logical dimension a player or NPC is currently inhabiting.
#[derive(Resource, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DimensionKind {
    #[default]
    Overworld,
    Tsy,
}

impl DimensionKind {
    /// Identifier 字符串（与 `valence::ident!` 注册值对齐）。
    /// 用作 wire schema `dimension` 字段的字面量，server / agent 双端必须一致。
    pub fn ident_str(self) -> &'static str {
        match self {
            DimensionKind::Overworld => "minecraft:overworld",
            DimensionKind::Tsy => TSY_DIMENSION_IDENT,
        }
    }
}

/// Resource holding the spawned `LayerBundle` entities for each dimension.
///
/// Inserted by `world::setup_world` after both layers are spawned. Cross-dimension
/// transfers (`apply_dimension_transfers`) consult this map to look up the target
/// layer entity.
#[derive(Resource, Debug, Clone, Copy)]
#[allow(dead_code)] // `tsy` consumed by `apply_dimension_transfers` (next commit).
pub struct DimensionLayers {
    pub overworld: Entity,
    pub tsy: Entity,
}

impl DimensionLayers {
    #[allow(dead_code)] // Used by cross-dim transfer in next commit.
    pub fn entity_for(&self, kind: DimensionKind) -> Entity {
        match kind {
            DimensionKind::Overworld => self.overworld,
            DimensionKind::Tsy => self.tsy,
        }
    }
}

/// Marker component on the overworld `LayerBundle` entity.
///
/// Existing single-layer queries (`Query<&mut ChunkLayer>` etc.) are scoped to the
/// overworld via `With<OverworldLayer>` filter so they keep finding exactly one
/// match after the TSY layer is also spawned.
#[derive(Component, Debug, Clone, Copy)]
pub struct OverworldLayer;

/// Marker component on the TSY `LayerBundle` entity.
#[allow(dead_code)] // Filter consumed by P0 / worldgen plan (TSY-scoped systems).
#[derive(Component, Debug, Clone, Copy)]
pub struct TsyLayer;

/// Component tracking which dimension a client (or relevant entity) currently inhabits.
///
/// - Initialised on `Added<Client>` to `DimensionKind::Overworld` (see `player::apply_spawn_defaults`).
/// - Mutated by `apply_dimension_transfers` after switching `VisibleChunkLayer`.
/// - Read by gameplay systems that need to scope queries to the current dimension
///   (e.g. zone lookups, terrain narration).
#[allow(dead_code)] // Wired up by next commit (`apply_dimension_transfers` + player init).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CurrentDimension(pub DimensionKind);

impl Default for CurrentDimension {
    fn default() -> Self {
        Self(DimensionKind::Overworld)
    }
}

/// Register the bespoke `bong:tsy` `DimensionType`. Must run before any
/// `LayerBundle::new(ident!("bong:tsy"), …)` call (Valence requires registry
/// stability before client spawn — see `valence_registry::dimension_type` notes).
pub fn register_tsy_dimension(registry: &mut DimensionTypeRegistry) {
    registry.insert(
        ident!("bong:tsy"),
        DimensionType {
            ambient_light: 0.08,
            bed_works: false,
            coordinate_scale: 1.0,
            effects: DimensionEffects::TheNether,
            fixed_time: Some(18000),
            has_ceiling: true,
            has_raids: false,
            has_skylight: false,
            height: 256,
            infiniburn: "#minecraft:infiniburn_nether".into(),
            logical_height: 256,
            min_y: -64,
            monster_spawn_block_light_limit: 0,
            monster_spawn_light_level: MonsterSpawnLightLevel::Int(0),
            natural: false,
            piglin_safe: false,
            respawn_anchor_works: false,
            ultrawarm: false,
        },
    );
}

fn register_tsy_dimension_system(mut registry: ResMut<DimensionTypeRegistry>) {
    register_tsy_dimension(&mut registry);
}

pub fn register(app: &mut App) {
    app.add_systems(PreStartup, register_tsy_dimension_system);
}

/// Test helper: tag the single layer spawned by `valence::testing::ScenarioSingleClient::new`
/// with the [`OverworldLayer`] marker so production systems filtered by that marker still
/// see the test layer.
///
/// Production setup gets the marker from `world::setup_world` directly; tests that bypass
/// that setup need to opt in explicitly.
#[cfg(test)]
pub fn mark_test_layer_as_overworld(app: &mut App) {
    use valence::prelude::{ChunkLayer, EntityLayer, With};
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, (With<ChunkLayer>, With<EntityLayer>)>();
    let layer_entity = query
        .iter(world)
        .next()
        .expect("test scenario should have spawned a layer entity");
    world.entity_mut(layer_entity).insert(OverworldLayer);
}
