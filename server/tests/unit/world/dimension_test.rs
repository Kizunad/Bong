use bong_server::world::dimension::{
    register_tsy_dimension, CurrentDimension, DimensionKind, DimensionLayers,
    TSY_DIMENSION_IDENT,
};
use valence::prelude::{DimensionTypeRegistry, Entity};
use valence::registry::dimension_type::DimensionEffects;

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
