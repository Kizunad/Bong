use bong_server::lingtian::range_gate::{
    is_lingtian_position_in_scope, validate_lingtian_interaction, LingtianInteractionDenial,
};
use bong_server::world::dimension::{CurrentDimension, DimensionKind};
use valence::prelude::{bevy_ecs, App, BlockPos, DVec3, Entity, Position, Query};

fn validate_from_world(app: &mut App, actor: Entity) -> Result<(), LingtianInteractionDenial> {
    let world = app.world_mut();
    let mut state =
        bevy_ecs::system::SystemState::<(Query<&Position>, Query<&CurrentDimension>)>::new(world);
    let (positions, dimensions) = state.get(world);
    validate_lingtian_interaction(actor, target(), &positions, &dimensions)
}

fn target() -> BlockPos {
    BlockPos::new(0, 64, 0)
}

#[test]
fn overworld_target_inside_scope_is_allowed() {
    assert!(is_lingtian_position_in_scope(
        DVec3::new(0.5, 64.5, 0.5),
        DimensionKind::Overworld,
        target(),
    ));
}

#[test]
fn canonical_distance_plus_tolerance_is_allowed() {
    assert!(is_lingtian_position_in_scope(
        DVec3::new(5.0, 64.5, 0.5),
        DimensionKind::Overworld,
        target(),
    ));
}

#[test]
fn distance_just_past_tolerance_is_denied() {
    assert!(!is_lingtian_position_in_scope(
        DVec3::new(5.000_001, 64.5, 0.5),
        DimensionKind::Overworld,
        target(),
    ));
}

#[test]
fn tsy_is_denied_even_at_same_coordinates() {
    assert!(!is_lingtian_position_in_scope(
        DVec3::new(0.5, 64.5, 0.5),
        DimensionKind::Tsy,
        target(),
    ));
}

#[test]
fn validate_rejects_missing_position() {
    let mut app = App::new();
    let actor = Entity::from_raw(1);
    assert_eq!(
        validate_from_world(&mut app, actor),
        Err(LingtianInteractionDenial::MissingPosition)
    );
}

#[test]
fn validate_rejects_missing_dimension() {
    let mut app = App::new();
    let actor = app
        .world_mut()
        .spawn(Position(DVec3::new(0.5, 64.5, 0.5)))
        .id();
    assert_eq!(
        validate_from_world(&mut app, actor),
        Err(LingtianInteractionDenial::MissingDimension)
    );
}

#[test]
fn validate_rejects_wrong_dimension() {
    let mut app = App::new();
    let actor = app
        .world_mut()
        .spawn((
            Position(DVec3::new(0.5, 64.5, 0.5)),
            CurrentDimension(DimensionKind::Tsy),
        ))
        .id();
    assert_eq!(
        validate_from_world(&mut app, actor),
        Err(LingtianInteractionDenial::WrongDimension)
    );
}

#[test]
fn validate_accepts_block_center_boundary() {
    let mut app = App::new();
    let actor = app
        .world_mut()
        .spawn((
            Position(DVec3::new(5.0, 64.5, 0.5)),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();
    assert_eq!(validate_from_world(&mut app, actor), Ok(()));
}

#[test]
fn validate_rejects_out_of_range() {
    let mut app = App::new();
    let actor = app
        .world_mut()
        .spawn((
            Position(DVec3::new(5.001, 64.5, 0.5)),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();
    assert_eq!(
        validate_from_world(&mut app, actor),
        Err(LingtianInteractionDenial::OutOfRange)
    );
}
