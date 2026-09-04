use bong_server::lingtian::range_gate::{
    is_lingtian_position_in_scope, validate_lingtian_interaction, LingtianInteractionDenial,
};
use bong_server::reach::{DistanceMetric, DistanceRule, LINGTIAN_INTERACT_MAX_BLOCKS};
use bong_server::world::dimension::{CurrentDimension, DimensionKind};
use std::fs;
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

fn target_center(target: BlockPos) -> DVec3 {
    DVec3::new(
        f64::from(target.x) + 0.5,
        f64::from(target.y) + 0.5,
        f64::from(target.z) + 0.5,
    )
}

fn next_f64(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

#[test]
fn lingtian_range_gate_uses_the_named_euclidean_profile() {
    assert_eq!(
        DistanceRule::LINGTIAN_INTERACT.profile_parts(),
        Some((
            DistanceMetric::Euclidean3dSquared,
            LINGTIAN_INTERACT_MAX_BLOCKS,
        )),
        "灵田交互必须使用共享的 4.5 格欧氏 profile，而不是域内重复距离规则"
    );
    assert_eq!(
        DistanceRule::LINGTIAN_INTERACT,
        DistanceRule::lingtian_interact(),
        "灵田交互 profile 常量与构造器必须保持同一命名契约"
    );

    let source_path = format!("{}/src/lingtian/range_gate.rs", env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("读取灵田 range adapter 源码失败 {source_path}: {error}"));
    let scope_start = source
        .find("pub fn is_lingtian_position_in_scope(")
        .expect("灵田 range adapter 必须保留公开位置范围入口");
    let scope_end = source[scope_start..]
        .find("\npub fn validate_lingtian_interaction")
        .map(|offset| scope_start + offset)
        .expect("灵田 range adapter 与 validator 的边界必须保持明确");
    let scope = &source[scope_start..scope_end];
    let normalized: String = scope.split_whitespace().collect();

    assert!(
        normalized.contains("DistanceRule::LINGTIAN_INTERACT.allows(actor_position,target_center)"),
        "灵田 adapter 必须直接调用命名 LINGTIAN_INTERACT profile"
    );
    assert!(
        !scope.contains("LINGTIAN_INTERACT_MAX_DISTANCE")
            && !scope.contains("LINGTIAN_INTERACT_TOLERANCE"),
        "灵田 adapter 不得保留已删除的本地距离/容差常量"
    );
    let dimension_check = scope
        .find("if actor_dimension != DimensionKind::Overworld")
        .expect("灵田 adapter 必须保留主世界维度门");
    let center_conversion = scope
        .find("let target_center = DVec3::new(")
        .expect("灵田 adapter 必须保留方块中心换算");
    let profile_check = scope
        .find("DistanceRule::LINGTIAN_INTERACT.allows")
        .expect("灵田 adapter 必须有共享 profile 判定");
    assert!(
        dimension_check < center_conversion && center_conversion < profile_check,
        "灵田 adapter 必须先检查维度、再换算方块中心、最后执行 reach 判定"
    );
}

#[test]
fn lingtian_reach_includes_exact_four_point_five_boundary_and_rejects_one_ulp_beyond() {
    let target = target();
    let center = target_center(target);
    let radius = LINGTIAN_INTERACT_MAX_BLOCKS;
    let one_ulp_beyond = next_f64(radius);
    assert!(
        one_ulp_beyond > radius,
        "边界外 witness 必须是半径之上的下一个可表示 f64"
    );

    let exact_boundary = center + DVec3::new(radius, 0.0, 0.0);
    let just_outside = center + DVec3::new(one_ulp_beyond, 0.0, 0.0);
    assert!(
        is_lingtian_position_in_scope(exact_boundary, DimensionKind::Overworld, target),
        "灵田交互恰好 4.5 格的轴向欧氏边界必须 inclusive 放行"
    );
    assert!(
        DistanceRule::LINGTIAN_INTERACT.allows(center, exact_boundary),
        "共享灵田 profile 必须接受恰好 4.5 格的轴向边界"
    );
    assert!(
        !is_lingtian_position_in_scope(just_outside, DimensionKind::Overworld, target),
        "灵田交互超过 4.5 格一个 ULP 必须拒绝"
    );
    assert!(
        !DistanceRule::LINGTIAN_INTERACT.allows(center, just_outside),
        "共享灵田 profile 必须拒绝超过 4.5 格一个 ULP 的轴向距离"
    );
}

#[test]
fn lingtian_reach_preserves_euclidean_diagonal_boundary() {
    let target = target();
    let center = target_center(target);
    let exact_diagonal = center + DVec3::new(3.0, 3.0, 1.5);
    let outside_diagonal = center + DVec3::new(3.0, 3.0, 1.500_001);

    assert!(
        is_lingtian_position_in_scope(exact_diagonal, DimensionKind::Overworld, target),
        "(3,3,1.5) 的欧氏距离恰为 4.5 格，必须保持放行"
    );
    assert!(
        !is_lingtian_position_in_scope(outside_diagonal, DimensionKind::Overworld, target),
        "欧氏对角线超过 4.5 格后必须拒绝"
    );
}

#[test]
fn lingtian_reach_preserves_negative_coordinate_semantics() {
    let target = BlockPos::new(-1, -2, -3);
    let center = target_center(target);
    let exact_boundary = center + DVec3::new(-LINGTIAN_INTERACT_MAX_BLOCKS, 0.0, 0.0);
    let just_outside = center + DVec3::new(-next_f64(LINGTIAN_INTERACT_MAX_BLOCKS), 0.0, 0.0);

    assert!(
        is_lingtian_position_in_scope(exact_boundary, DimensionKind::Overworld, target),
        "负坐标目标的方块中心到 4.5 格轴向边界必须放行"
    );
    assert!(
        !is_lingtian_position_in_scope(just_outside, DimensionKind::Overworld, target),
        "负坐标目标超过 4.5 格一个 ULP 必须拒绝"
    );
}

#[test]
fn lingtian_reach_fails_closed_for_non_finite_actor_coordinates() {
    let center = target_center(target());

    for (label, coordinate) in [
        ("NaN", f64::NAN),
        ("positive infinity", f64::INFINITY),
        ("negative infinity", f64::NEG_INFINITY),
    ] {
        for axis in 0..3 {
            let mut coordinates = [center.x, center.y, center.z];
            coordinates[axis] = coordinate;
            assert!(
                !is_lingtian_position_in_scope(
                    DVec3::from_array(coordinates),
                    DimensionKind::Overworld,
                    target(),
                ),
                "{label} actor coordinate on axis {axis} must fail closed"
            );
        }
    }
}

#[test]
fn rejected_lingtian_interaction_does_not_mutate_world_state() {
    let mut app = App::new();
    let position = DVec3::new(5.001, 64.5, 0.5);
    let dimension = DimensionKind::Overworld;
    let actor = app
        .world_mut()
        .spawn((Position(position), CurrentDimension(dimension)))
        .id();
    let entity_count = app.world().entities().len();

    assert_eq!(
        validate_from_world(&mut app, actor),
        Err(LingtianInteractionDenial::OutOfRange),
        "超出范围的灵田交互必须在任何业务 mutation 前拒绝"
    );
    assert_eq!(
        app.world().entities().len(),
        entity_count,
        "灵田交互拒绝不得创建或删除实体"
    );
    assert_eq!(
        app.world().get::<Position>(actor).map(|value| value.0),
        Some(position),
        "灵田交互拒绝不得改写请求者位置"
    );
    assert_eq!(
        app.world()
            .get::<CurrentDimension>(actor)
            .map(|value| value.0),
        Some(dimension),
        "灵田交互拒绝不得改写请求者维度"
    );
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
