//! TSY (坍缩渊) dimension registration and per-player dimension tracking.
//!
//! See `docs/plan-tsy-dimension-v1.md` §1 for the design rationale.

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, ident, App, Commands, Component, DimensionType, DimensionTypeRegistry, Entity,
    EntityLayerId, Mut, PreStartup, ResMut, Resource, VisibleChunkLayer, VisibleEntityLayers,
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

/// How a lifecycle transition should publish the Overworld visibility set.
///
/// Formal revival and explicit new-character creation replace the entire runtime view, while
/// join-time reincarnation preserves visibility layers unrelated to the dimension being left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverworldVisibilityPolicy {
    ReplaceAll,
    PreserveUnrelatedLayers,
}

/// Publishes an already-committed transition to the Overworld runtime layer.
///
/// Missing components are inserted through `Commands`; callers must establish `DimensionLayers`
/// as a durable-transaction prerequisite before invoking this runtime-only publication helper.
#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_overworld_runtime(
    entity: Entity,
    commands: &mut Commands,
    layer_id: Option<Mut<'_, EntityLayerId>>,
    visible_chunk_layer: Option<Mut<'_, VisibleChunkLayer>>,
    visible_entity_layers: Option<Mut<'_, VisibleEntityLayers>>,
    current_dimension: Option<Mut<'_, CurrentDimension>>,
    layers: &DimensionLayers,
    visibility_policy: OverworldVisibilityPolicy,
) {
    let overworld = layers.overworld;
    let previous_layer = layer_id.as_ref().map(|layer_id| layer_id.0);

    if let Some(mut layer_id) = layer_id {
        layer_id.0 = overworld;
    } else {
        commands.entity(entity).insert(EntityLayerId(overworld));
    }

    if let Some(mut visible_layers) = visible_entity_layers {
        match visibility_policy {
            OverworldVisibilityPolicy::ReplaceAll => visible_layers.0.clear(),
            OverworldVisibilityPolicy::PreserveUnrelatedLayers => {
                if let Some(previous_layer) = previous_layer {
                    visible_layers.0.remove(&previous_layer);
                } else {
                    visible_layers.0.clear();
                }
                visible_layers.0.remove(&layers.tsy);
            }
        }
        visible_layers.0.insert(overworld);
    } else {
        let mut visible_layers = VisibleEntityLayers::default();
        visible_layers.0.insert(overworld);
        commands.entity(entity).insert(visible_layers);
    }

    if let Some(mut visible_chunk_layer) = visible_chunk_layer {
        visible_chunk_layer.0 = overworld;
    } else {
        commands.entity(entity).insert(VisibleChunkLayer(overworld));
    }
    if let Some(mut current_dimension) = current_dimension {
        current_dimension.0 = DimensionKind::Overworld;
    } else {
        commands
            .entity(entity)
            .insert(CurrentDimension(DimensionKind::Overworld));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_kind_default_is_overworld() {
        assert_eq!(DimensionKind::default(), DimensionKind::Overworld);
    }

    #[test]
    fn current_dimension_default_is_overworld() {
        assert_eq!(
            CurrentDimension::default(),
            CurrentDimension(DimensionKind::Overworld)
        );
    }

    #[test]
    fn tsy_ident_constant_matches_register() {
        assert_eq!(TSY_DIMENSION_IDENT, "bong:tsy");
    }

    #[test]
    fn register_tsy_dimension_inserts_bong_tsy() {
        let mut registry = DimensionTypeRegistry::default();
        register_tsy_dimension(&mut registry);
        let found = registry
            .iter()
            .any(|(_, name, _)| name.as_str() == "bong:tsy");
        assert!(
            found,
            "registry should contain bong:tsy entry after register_tsy_dimension"
        );
    }

    #[test]
    fn register_tsy_dimension_uses_nether_visuals() {
        let mut registry = DimensionTypeRegistry::default();
        register_tsy_dimension(&mut registry);
        let (_, _, dim) = registry
            .iter()
            .find(|(_, name, _)| name.as_str() == "bong:tsy")
            .expect("bong:tsy should be registered");
        assert_eq!(dim.effects, DimensionEffects::TheNether);
        assert!(!dim.has_skylight);
        assert!(dim.has_ceiling);
        assert_eq!(dim.fixed_time, Some(18000));
        assert_eq!(dim.height, 256);
        assert_eq!(dim.logical_height, 256);
        assert_eq!(dim.min_y, -64);
    }

    #[test]
    fn dimension_layers_entity_for_routes_correctly() {
        // Use Entity::PLACEHOLDER values (any constants since we just compare).
        let layers = DimensionLayers {
            overworld: Entity::from_raw(1),
            tsy: Entity::from_raw(2),
        };
        assert_eq!(
            layers.entity_for(DimensionKind::Overworld),
            Entity::from_raw(1)
        );
        assert_eq!(layers.entity_for(DimensionKind::Tsy), Entity::from_raw(2));
    }
}
