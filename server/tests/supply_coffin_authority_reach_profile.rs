use bong_server::cultivation::components::Cultivation;
use bong_server::inventory::external_container::{
    ExternalContainer, ExternalContainerKind, ExternalContainerRegistry,
};
use bong_server::inventory::{
    ContainerState, InventoryInstanceIdAllocator, InventoryRevision, ItemRegistry, PlayerInventory,
};
use bong_server::network::audio_event_emit::PlaySoundRecipeRequest;
use bong_server::network::vfx_event_emit::VfxEventRequest;
use bong_server::player::state::PlayerState;
use bong_server::supply_coffin::{
    current_wall_clock_secs, external_container_lifecycle_tick, handle_supply_coffin_interact,
    ActiveSupplyCoffin, SupplyCoffinGrade, SupplyCoffinOpenRequest, SupplyCoffinOpened,
    SupplyCoffinRegistry,
};
use bong_server::world::dimension::{CurrentDimension, DimensionKind};
use valence::prelude::{App, DVec3, Entity, Position, Update};
use valence::testing::{create_mock_client, MockClientHelper};

const COFFIN_POS: DVec3 = DVec3::ZERO;

fn next_f64(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

fn empty_inventory() -> PlayerInventory {
    PlayerInventory {
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            id: "main_pack".to_string(),
            name: "main_pack".to_string(),
            rows: 5,
            cols: 7,
            items: Vec::new(),
            owner_instance_id: None,
            quick_access: false,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
        triggered_treasures: Vec::new(),
    }
}

fn setup_open_app(
    player_pos: DVec3,
    player_dimension: Option<DimensionKind>,
    source: Option<ActiveSupplyCoffin>,
) -> (App, Entity, Entity, MockClientHelper) {
    let mut app = App::new();
    app.insert_resource(ExternalContainerRegistry::default());
    app.insert_resource(ItemRegistry::default());
    app.insert_resource(InventoryInstanceIdAllocator::default());
    app.add_event::<SupplyCoffinOpenRequest>();
    app.add_event::<SupplyCoffinOpened>();
    app.add_event::<PlaySoundRecipeRequest>();

    let (client_bundle, helper) = create_mock_client("Azure");
    let player = app
        .world_mut()
        .spawn((
            client_bundle,
            empty_inventory(),
            PlayerState::default(),
            Cultivation::default(),
        ))
        .id();
    app.world_mut()
        .entity_mut(player)
        .insert(Position::new(player_pos));
    if let Some(dimension) = player_dimension {
        app.world_mut()
            .entity_mut(player)
            .insert(CurrentDimension(dimension));
    }

    let target = app.world_mut().spawn_empty().id();
    let mut registry = SupplyCoffinRegistry::new(
        (
            DVec3::new(-100.0, -100.0, -100.0),
            DVec3::new(100.0, 100.0, 100.0),
        ),
        0.0,
        0x1234,
    );
    if let Some(active) = source {
        registry.active.insert(target, active);
    }
    app.insert_resource(registry);
    app.add_systems(Update, handle_supply_coffin_interact);
    (app, player, target, helper)
}

fn active(pos: DVec3) -> ActiveSupplyCoffin {
    ActiveSupplyCoffin {
        grade: SupplyCoffinGrade::Common,
        pos,
        dimension: DimensionKind::Overworld,
        spawned_at_wall_secs: current_wall_clock_secs(),
    }
}

fn send_open(app: &mut App, player: Entity, target: Entity) {
    app.world_mut()
        .resource_mut::<valence::prelude::bevy_ecs::event::Events<SupplyCoffinOpenRequest>>()
        .send(SupplyCoffinOpenRequest {
            client: player,
            target,
        });
    app.update();
}

fn assert_opened(app: &App, target: Entity, expected: bool, label: &str) {
    assert_eq!(
        app.world().get::<ExternalContainer>(target).is_some(),
        expected,
        "{label}: authorization result must match the supply-coffin open profile"
    );
}

#[test]
fn open_profile_matches_boundary_diagonal_ulp_and_negative_coordinates() {
    let cases = [
        ("贴脸", DVec3::new(1.0, 0.0, 0.0), true),
        ("4.5 轴向边界", DVec3::new(4.5, 0.0, 0.0), true),
        ("4.5 欧氏对角线内点", DVec3::splat(2.25), true),
        ("4.5 超出一 ULP", DVec3::new(next_f64(4.5), 0.0, 0.0), false),
        ("负坐标边界", DVec3::new(-4.5, 0.0, 0.0), true),
    ];

    for (label, player_pos, expected) in cases {
        let (mut app, player, target, _helper) = setup_open_app(
            player_pos,
            Some(DimensionKind::Overworld),
            Some(active(COFFIN_POS)),
        );
        send_open(&mut app, player, target);
        assert_opened(&app, target, expected, label);
    }
}

#[test]
fn open_profile_rejects_non_finite_coordinates_and_missing_or_wrong_dimension() {
    for (label, coordinate) in [
        ("NaN", f64::NAN),
        ("正无穷", f64::INFINITY),
        ("负无穷", f64::NEG_INFINITY),
    ] {
        let (mut app, player, target, _helper) = setup_open_app(
            DVec3::new(coordinate, 0.0, 0.0),
            Some(DimensionKind::Overworld),
            Some(active(COFFIN_POS)),
        );
        send_open(&mut app, player, target);
        assert_opened(&app, target, false, label);
    }

    let (mut missing_dimension, player, target, _helper) =
        setup_open_app(COFFIN_POS, None, Some(active(COFFIN_POS)));
    send_open(&mut missing_dimension, player, target);
    assert_opened(&missing_dimension, target, false, "缺失玩家位面");

    let (mut wrong_dimension, player, target, _helper) = setup_open_app(
        COFFIN_POS,
        Some(DimensionKind::Tsy),
        Some(active(COFFIN_POS)),
    );
    send_open(&mut wrong_dimension, player, target);
    assert_opened(&wrong_dimension, target, false, "同 XYZ 跨维");

    let (mut missing_source, player, target, _helper) =
        setup_open_app(COFFIN_POS, Some(DimensionKind::Overworld), None);
    send_open(&mut missing_source, player, target);
    assert_opened(&missing_source, target, false, "缺失 source");

    let (mut non_finite_source, player, target, _helper) = setup_open_app(
        COFFIN_POS,
        Some(DimensionKind::Overworld),
        Some(active(DVec3::new(f64::INFINITY, 0.0, 0.0))),
    );
    send_open(&mut non_finite_source, player, target);
    assert_opened(&non_finite_source, target, false, "source 正无穷");
}

fn setup_session_app(player_pos: DVec3) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, external_container_lifecycle_tick);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app
        .world_mut()
        .spawn((
            client_bundle,
            empty_inventory(),
            PlayerState::default(),
            Cultivation::default(),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();
    app.world_mut()
        .entity_mut(player)
        .insert(Position::new(player_pos));
    let coffin = app
        .world_mut()
        .spawn(ExternalContainer {
            session_id: 91,
            container: ContainerState {
                id: ExternalContainer::container_id(91),
                name: "supply_coffin_common".to_string(),
                rows: 3,
                cols: 4,
                items: Vec::new(),
                owner_instance_id: None,
                quick_access: false,
            },
            opened_by: Some(player),
            timeout_wall_secs: u64::MAX,
            source_kind: ExternalContainerKind::SupplyCoffin {
                grade: SupplyCoffinGrade::Common,
            },
        })
        .id();
    let mut ext_registry = ExternalContainerRegistry::default();
    ext_registry.sessions.insert(91, coffin);
    app.insert_resource(ext_registry);
    let mut registry = SupplyCoffinRegistry::new(
        (
            DVec3::new(-100.0, -100.0, -100.0),
            DVec3::new(100.0, 100.0, 100.0),
        ),
        0.0,
        0x4321,
    );
    registry.active.insert(coffin, active(COFFIN_POS));
    app.insert_resource(registry);
    (app, player, coffin)
}

#[test]
fn session_profile_is_euclidean_inclusive_and_rejects_one_ulp_beyond() {
    for (label, player_pos, expected) in [
        ("6.5 轴向边界", DVec3::new(6.5, 0.0, 0.0), true),
        ("6.5 欧氏对角线内点", DVec3::splat(3.25), true),
        ("6.5 超出一 ULP", DVec3::new(next_f64(6.5), 0.0, 0.0), false),
        ("负坐标边界", DVec3::new(-6.5, 0.0, 0.0), true),
    ] {
        let (mut app, _player, coffin) = setup_session_app(player_pos);
        app.update();
        if expected {
            assert!(
                app.world()
                    .get::<ExternalContainer>(coffin)
                    .unwrap()
                    .opened_by
                    .is_some(),
                "{label}: exact session boundary must remain authorized"
            );
        } else {
            assert_eq!(
                app.world()
                    .get::<ExternalContainer>(coffin)
                    .unwrap()
                    .opened_by,
                None,
                "{label}: one ULP beyond session boundary must close the session"
            );
        }
    }
}

#[test]
fn session_profile_fails_closed_for_non_finite_and_missing_authority_context() {
    for (label, coordinate) in [
        ("NaN", f64::NAN),
        ("正无穷", f64::INFINITY),
        ("负无穷", f64::NEG_INFINITY),
    ] {
        let (mut app, _player, coffin) = setup_session_app(DVec3::new(coordinate, 0.0, 0.0));
        app.update();
        assert_eq!(
            app.world()
                .get::<ExternalContainer>(coffin)
                .unwrap()
                .opened_by,
            None,
            "{label}: non-finite session coordinates must fail closed"
        );
    }

    let (mut missing_source, _player, coffin) = setup_session_app(COFFIN_POS);
    missing_source
        .world_mut()
        .resource_mut::<SupplyCoffinRegistry>()
        .active
        .clear();
    missing_source.update();
    assert_eq!(
        missing_source
            .world()
            .get::<ExternalContainer>(coffin)
            .unwrap()
            .opened_by,
        None,
        "missing session source must close the session before mutation"
    );

    let (mut missing_dimension, player, coffin) = setup_session_app(COFFIN_POS);
    missing_dimension
        .world_mut()
        .entity_mut(player)
        .remove::<CurrentDimension>();
    missing_dimension.update();
    assert_eq!(
        missing_dimension
            .world()
            .get::<ExternalContainer>(coffin)
            .unwrap()
            .opened_by,
        None,
        "missing session dimension must close the session"
    );

    let (mut wrong_dimension, player, coffin) = setup_session_app(COFFIN_POS);
    wrong_dimension
        .world_mut()
        .entity_mut(player)
        .insert(CurrentDimension(DimensionKind::Tsy));
    wrong_dimension.update();
    assert_eq!(
        wrong_dimension
            .world()
            .get::<ExternalContainer>(coffin)
            .unwrap()
            .opened_by,
        None,
        "same XYZ across dimensions must close the session"
    );
}
