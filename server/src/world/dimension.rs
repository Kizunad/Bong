//! TSY (坍缩渊) dimension registration and per-player dimension tracking.
//!
//! See `docs/plan-tsy-dimension-v1.md` §1 for the design rationale.

use valence::prelude::{
    bevy_ecs, ident, App, Component, DimensionType, DimensionTypeRegistry, Entity, PreStartup,
    ResMut, Resource,
};
use valence::registry::dimension_type::{DimensionEffects, MonsterSpawnLightLevel};

/// Identifier of the TSY dimension in `DimensionTypeRegistry`.
pub const TSY_DIMENSION_IDENT: &str = "bong:tsy";

/// Logical dimension a player or NPC is currently inhabiting.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DimensionKind {
    Overworld,
    Tsy,
}

impl Default for DimensionKind {
    fn default() -> Self {
        Self::Overworld
    }
}

/// Resource holding the spawned `LayerBundle` entities for each dimension.
///
/// Inserted by `world::setup_world` after both layers are spawned. Cross-dimension
/// transfers (`apply_dimension_transfers`) consult this map to look up the target
/// layer entity.
#[derive(Resource, Debug, Clone, Copy)]
pub struct DimensionLayers {
    pub overworld: Entity,
    pub tsy: Entity,
}

impl DimensionLayers {
    pub fn entity_for(&self, kind: DimensionKind) -> Entity {
        match kind {
            DimensionKind::Overworld => self.overworld,
            DimensionKind::Tsy => self.tsy,
        }
    }
}

/// Component tracking which dimension a client (or relevant entity) currently inhabits.
///
/// - Initialised on `Added<Client>` to `DimensionKind::Overworld` (see `player::apply_spawn_defaults`).
/// - Mutated by `apply_dimension_transfers` after switching `VisibleChunkLayer`.
/// - Read by gameplay systems that need to scope queries to the current dimension
///   (e.g. zone lookups, terrain narration).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_kind_default_is_overworld() {
        assert_eq!(DimensionKind::default(), DimensionKind::Overworld);
    }

    #[test]
    fn current_dimension_default_is_overworld() {
        assert_eq!(CurrentDimension::default(), CurrentDimension(DimensionKind::Overworld));
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
        assert!(found, "registry should contain bong:tsy entry after register_tsy_dimension");
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
        assert_eq!(layers.entity_for(DimensionKind::Overworld), Entity::from_raw(1));
        assert_eq!(layers.entity_for(DimensionKind::Tsy), Entity::from_raw(2));
    }
}
