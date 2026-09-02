//! R6 P1 independent contract tests for the declared/unwired S2C emit builder.

use bong_server::network::agent_bridge::{serialize_server_data_payload, SERVER_DATA_CHANNEL};
use bong_server::network::emit::{
    emit_server_data, EmitReport, EmitScope, JoinSnapshotKey, ReplayPolicy, ServerDataEmission,
};
use bong_server::schema::server_data::ServerDataV1;
use bong_server::world::dimension::{CurrentDimension, DimensionKind};
use bong_server::world::zone::ZoneRegistry;
use valence::prelude::{
    App, Client, DVec3, Entity, Position, Query, Res, ResMut, Resource, Update,
};
use valence::protocol::packets::play::CustomPayloadS2c;
use valence::testing::{create_mock_client, MockClientHelper};

fn emission(scope: EmitScope) -> ServerDataEmission {
    ServerDataEmission::new(ServerDataV1::welcome("scope contract"), scope)
}

struct PendingEmission(ServerDataEmission);

impl Resource for PendingEmission {}

#[derive(Default)]
struct CapturedReport(Option<EmitReport>);

impl Resource for CapturedReport {}

fn call_builder_system(
    emission: Res<PendingEmission>,
    mut recipients: Query<(
        Entity,
        &mut Client,
        Option<&CurrentDimension>,
        Option<&Position>,
    )>,
    zones: Option<Res<ZoneRegistry>>,
    mut report: ResMut<CapturedReport>,
) {
    report.0 = Some(emit_server_data(
        &emission.0,
        &mut recipients,
        zones.as_deref(),
    ));
}

fn new_contract_app() -> App {
    let mut app = App::new();
    app.add_systems(Update, call_builder_system);
    app
}

fn run_emit(
    app: &mut App,
    emission: ServerDataEmission,
    zones: Option<ZoneRegistry>,
) -> EmitReport {
    app.insert_resource(PendingEmission(emission));
    app.insert_resource(CapturedReport::default());
    if let Some(zones) = zones {
        app.insert_resource(zones);
    } else {
        app.world_mut().remove_resource::<ZoneRegistry>();
    }
    app.update();

    app.world_mut().remove_resource::<PendingEmission>();
    app.world_mut()
        .remove_resource::<CapturedReport>()
        .expect("builder system must capture one report")
        .0
        .expect("builder system must write a report")
}

fn spawn_client(
    app: &mut App,
    name: &str,
    dimension: Option<DimensionKind>,
    position: Option<[f64; 3]>,
) -> (Entity, MockClientHelper) {
    let (mut bundle, helper) = create_mock_client(name);
    if let Some(position) = position {
        bundle.player.position = Position::new(position);
    }
    let entity = app.world_mut().spawn(bundle).id();
    if let Some(dimension) = dimension {
        app.world_mut()
            .entity_mut(entity)
            .insert(CurrentDimension(dimension));
    }
    if position.is_none() {
        app.world_mut().entity_mut(entity).remove::<Position>();
    }
    (entity, helper)
}

fn flush_client_packets(app: &mut App) {
    let world = app.world_mut();
    let mut clients = world.query::<&mut Client>();
    for mut client in clients.iter_mut(world) {
        client
            .flush_packets()
            .expect("contract mock client packets should flush");
    }
}

fn server_data_packets(helper: &mut MockClientHelper) -> Vec<Vec<u8>> {
    helper
        .collect_received()
        .0
        .into_iter()
        .filter_map(|frame| {
            let packet = frame.decode::<CustomPayloadS2c>().ok()?;
            (packet.channel.as_str() == SERVER_DATA_CHANNEL).then(|| packet.data.0 .0.to_vec())
        })
        .collect()
}

fn assert_report(report: EmitReport, matched: usize, sent: usize, serialization_failed: usize) {
    assert_eq!(
        report,
        EmitReport {
            matched,
            sent,
            serialization_failed,
        },
        "emit report must count scope matches, queued sends, and emission-level serialization failures"
    );
}

fn test_zone_registry() -> ZoneRegistry {
    let mut overworld = ZoneRegistry::fallback().zones[0].clone();
    overworld.name = "qingyun_test".to_string();
    overworld.bounds = (DVec3::new(-10.0, 64.0, -10.0), DVec3::new(10.0, 80.0, 10.0));

    let mut tsy = overworld.clone();
    tsy.name = "tsy_test".to_string();
    tsy.dimension = DimensionKind::Tsy;

    ZoneRegistry {
        zones: vec![overworld, tsy],
        spatial_revision: 0,
    }
}

#[test]
fn global_scope_matches_every_client_without_metadata_and_reuses_wire_bytes() {
    let mut app = new_contract_app();
    let (_, mut first) = spawn_client(&mut app, "GlobalOne", None, None);
    let (_, mut second) = spawn_client(
        &mut app,
        "GlobalTwo",
        Some(DimensionKind::Tsy),
        Some([0.0, 70.0, 0.0]),
    );

    let report = run_emit(&mut app, emission(EmitScope::Global), None);
    assert_report(report, 2, 2, 0);

    flush_client_packets(&mut app);
    let expected = serialize_server_data_payload(&ServerDataV1::welcome("scope contract"))
        .expect("the contract payload should serialize");
    assert_eq!(server_data_packets(&mut first), vec![expected.clone()]);
    assert_eq!(server_data_packets(&mut second), vec![expected]);
}

#[test]
fn dimension_scope_requires_metadata_and_never_crosses_dimensions() {
    let mut app = new_contract_app();
    let (_, mut same) = spawn_client(
        &mut app,
        "DimensionSame",
        Some(DimensionKind::Overworld),
        Some([0.0, 70.0, 0.0]),
    );
    let (_, mut other) = spawn_client(
        &mut app,
        "DimensionOther",
        Some(DimensionKind::Tsy),
        Some([0.0, 70.0, 0.0]),
    );
    let (_, mut missing) = spawn_client(&mut app, "DimensionMissing", None, Some([0.0, 70.0, 0.0]));

    let report = run_emit(
        &mut app,
        emission(EmitScope::Dimension(DimensionKind::Overworld)),
        None,
    );
    assert_report(report, 1, 1, 0);

    flush_client_packets(&mut app);
    assert_eq!(server_data_packets(&mut same).len(), 1);
    assert!(
        server_data_packets(&mut other).is_empty(),
        "different dimensions must not receive a dimension-scoped payload"
    );
    assert!(
        server_data_packets(&mut missing).is_empty(),
        "missing CurrentDimension must fail closed"
    );
}

#[test]
fn zone_scope_uses_authoritative_canonical_lookup_and_fails_closed() {
    let mut app = new_contract_app();
    let (_, mut same) = spawn_client(
        &mut app,
        "ZoneSame",
        Some(DimensionKind::Overworld),
        Some([0.0, 70.0, 0.0]),
    );
    let (_, mut other_zone) = spawn_client(
        &mut app,
        "ZoneOther",
        Some(DimensionKind::Overworld),
        Some([20.0, 70.0, 0.0]),
    );
    let (_, mut other_dimension) = spawn_client(
        &mut app,
        "ZoneOtherDimension",
        Some(DimensionKind::Tsy),
        Some([0.0, 70.0, 0.0]),
    );
    let (_, mut missing_dimension) = spawn_client(
        &mut app,
        "ZoneMissingDimension",
        None,
        Some([0.0, 70.0, 0.0]),
    );
    let (_, mut missing_position) = spawn_client(
        &mut app,
        "ZoneMissingPosition",
        Some(DimensionKind::Overworld),
        None,
    );
    let (_, mut registryless) = spawn_client(
        &mut app,
        "ZoneRegistryless",
        Some(DimensionKind::Overworld),
        Some([0.0, 70.0, 0.0]),
    );
    let registry = test_zone_registry();

    let report = run_emit(
        &mut app,
        emission(EmitScope::Zone {
            dimension: DimensionKind::Overworld,
            zone: "qingyun_test".to_string(),
        }),
        Some(registry),
    );
    assert_report(report, 2, 2, 0);

    flush_client_packets(&mut app);
    assert_eq!(server_data_packets(&mut same).len(), 1);
    assert_eq!(
        server_data_packets(&mut registryless).len(),
        1,
        "a client at a canonical zone must match while the authoritative registry is present"
    );
    assert!(
        server_data_packets(&mut other_zone).is_empty(),
        "Zone scope must compare the canonical zone resolved from Position"
    );
    assert!(
        server_data_packets(&mut other_dimension).is_empty(),
        "matching XYZ in another dimension must not match"
    );
    assert!(
        server_data_packets(&mut missing_dimension).is_empty(),
        "missing CurrentDimension must fail closed for Zone"
    );
    assert!(
        server_data_packets(&mut missing_position).is_empty(),
        "missing Position must fail closed for Zone"
    );

    let report = run_emit(
        &mut app,
        emission(EmitScope::Zone {
            dimension: DimensionKind::Overworld,
            zone: "qingyun_test".to_string(),
        }),
        None,
    );
    assert_report(report, 0, 0, 0);
    flush_client_packets(&mut app);
    assert!(
        server_data_packets(&mut registryless).is_empty(),
        "Zone scope without an authoritative registry must have zero recipients"
    );
}

#[test]
fn player_scope_is_exact_and_does_not_broadcast_when_target_is_invalid() {
    let mut app = new_contract_app();
    let (target, mut target_helper) = spawn_client(&mut app, "PlayerTarget", None, None);
    let (_, mut other_helper) = spawn_client(
        &mut app,
        "PlayerOther",
        Some(DimensionKind::Tsy),
        Some([0.0, 70.0, 0.0]),
    );

    let report = run_emit(&mut app, emission(EmitScope::Player(target)), None);
    assert_report(report, 1, 1, 0);
    flush_client_packets(&mut app);
    assert_eq!(
        server_data_packets(&mut target_helper).len(),
        1,
        "Player scope must reach the exact target even without metadata"
    );
    assert!(
        server_data_packets(&mut other_helper).is_empty(),
        "Player scope must not reach other clients"
    );

    let non_client = app.world_mut().spawn_empty().id();
    let report = run_emit(&mut app, emission(EmitScope::Player(non_client)), None);
    assert_report(report, 0, 0, 0);
    flush_client_packets(&mut app);
    assert!(
        server_data_packets(&mut target_helper).is_empty(),
        "a non-client target must not fall back to broadcast"
    );
    assert!(
        server_data_packets(&mut other_helper).is_empty(),
        "an absent/non-client target must produce zero recipients"
    );
}

#[test]
fn empty_recipient_set_is_reported_without_send() {
    let mut app = new_contract_app();
    let report = run_emit(&mut app, emission(EmitScope::Global), None);
    assert_report(report, 0, 0, 0);
}

#[test]
fn serialization_failure_is_counted_once_and_sends_nothing() {
    let mut app = new_contract_app();
    let (_, mut helper) = spawn_client(
        &mut app,
        "SerializationFailure",
        Some(DimensionKind::Overworld),
        Some([0.0, 70.0, 0.0]),
    );
    let oversized = ServerDataV1::welcome("x".repeat(2 * 1024 * 1024));
    let report = run_emit(
        &mut app,
        ServerDataEmission {
            payload: oversized,
            scope: EmitScope::Global,
            replay: ReplayPolicy::None,
        },
        None,
    );
    assert_report(report, 0, 0, 1);
    flush_client_packets(&mut app);
    assert!(
        server_data_packets(&mut helper).is_empty(),
        "a serialization failure must not queue a partial or fallback packet"
    );
}

#[test]
fn replay_policy_is_metadata_only_and_join_key_roundtrips() {
    let key = JoinSnapshotKey::new("player_state");
    let with_replay = emission(EmitScope::Global).with_replay(key.clone());
    assert_eq!(key.as_str(), "player_state");
    assert_eq!(with_replay.replay, ReplayPolicy::JoinSnapshot(key));

    let mut app = new_contract_app();
    let (_, mut helper) = spawn_client(&mut app, "ReplayMetadata", None, None);
    let report = run_emit(&mut app, with_replay, None);
    assert_report(report, 1, 1, 0);
    flush_client_packets(&mut app);
    let expected = serialize_server_data_payload(&ServerDataV1::welcome("scope contract"))
        .expect("replay metadata must not change payload serialization");
    assert_eq!(
        server_data_packets(&mut helper),
        vec![expected],
        "JoinSnapshot metadata must not make the builder synthesize or append a business snapshot"
    );

    let none = emission(EmitScope::Global);
    assert_eq!(none.replay, ReplayPolicy::None);
}
