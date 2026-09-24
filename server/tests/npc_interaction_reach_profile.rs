use bong_server::network::gate::{DistanceMetric, DistanceRule, NEARBY_INTERACT_MAX_BLOCKS};
use std::fs;

fn next_f64(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

#[test]
fn npc_interaction_uses_the_named_euclidean_nearby_profile() {
    let profile = DistanceRule::nearby_interact();

    assert_eq!(
        profile.profile_parts(),
        Some((DistanceMetric::Euclidean3dSquared, NEARBY_INTERACT_MAX_BLOCKS)),
        "NPC interaction must use the named 6.0-block Euclidean profile, not a local metric or radius"
    );
    assert_eq!(
        profile,
        DistanceRule::NEARBY_INTERACT,
        "the constructor and frozen profile constant must describe the same NPC interaction policy"
    );
}

#[test]
fn npc_interaction_accepts_exact_boundary_and_rejects_one_ulp_beyond() {
    let profile = DistanceRule::nearby_interact();
    let radius = NEARBY_INTERACT_MAX_BLOCKS;
    let one_ulp_beyond = next_f64(radius);

    assert!(
        profile.allows([0.0, 0.0, 0.0], [radius, 0.0, 0.0]),
        "exactly 6.0 Euclidean blocks must remain inclusive for NPC interaction"
    );
    assert!(
        profile.allows([0.0, 0.0, 0.0], [4.0, 4.0, 0.0]),
        "an in-range Euclidean diagonal must remain reachable"
    );
    assert!(
        !profile.allows([0.0, 0.0, 0.0], [one_ulp_beyond, 0.0, 0.0]),
        "one representable ULP beyond the 6.0-block Euclidean boundary must fail closed"
    );
}

#[test]
fn npc_interaction_preserves_negative_coordinate_distance_semantics() {
    let profile = DistanceRule::nearby_interact();

    assert!(
        profile.allows([-10.0, -20.0, -30.0], [-4.0, -20.0, -30.0]),
        "negative-coordinate positions exactly 6.0 blocks apart must remain reachable"
    );
    assert!(
        !profile.allows([-10.0, -20.0, -30.0], [-3.0, -15.0, -26.0]),
        "negative-coordinate positions beyond Euclidean reach must be rejected"
    );
}

#[test]
fn npc_interaction_fails_closed_for_non_finite_requester_or_target_coordinates() {
    let profile = DistanceRule::nearby_interact();

    for (label, coordinate) in [
        ("NaN", f64::NAN),
        ("positive infinity", f64::INFINITY),
        ("negative infinity", f64::NEG_INFINITY),
    ] {
        for axis in 0..3 {
            let mut requester = [0.0, 0.0, 0.0];
            requester[axis] = coordinate;
            assert!(
                !profile.allows(requester, [0.0, 0.0, 0.0]),
                "{label} requester coordinate on axis {axis} must fail closed"
            );

            let mut target = [0.0, 0.0, 0.0];
            target[axis] = coordinate;
            assert!(
                !profile.allows([0.0, 0.0, 0.0], target),
                "{label} target coordinate on axis {axis} must fail closed"
            );
        }
    }
}

#[test]
fn npc_resolver_is_wired_to_the_shared_reach_profile() {
    let source_path = format!(
        "{}/src/network/client_request/npc.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
        panic!("read NPC interaction consumer source {source_path}: {error}")
    });
    let resolver_start = source
        .find("pub(crate) fn resolve_npc_engagement_target")
        .expect("NPC engagement resolver must remain present");
    let resolver_end = source[resolver_start..]
        .find("\nfn dimension_kind_for")
        .map(|offset| resolver_start + offset)
        .expect("NPC engagement resolver must retain its existing helper boundary");
    let resolver = &source[resolver_start..resolver_end];

    assert!(
        source.contains("use crate::reach::DistanceRule;"),
        "NPC interaction consumer must import the shared reach policy"
    );
    assert!(
        resolver.contains("DistanceRule::nearby_interact().allows(player_position, npc_position)"),
        "resolve_npc_engagement_target must evaluate the shared NearbyInteract predicate"
    );
    assert!(
        !source.contains("NPC_INTERACTION_MAX_DISTANCE"),
        "NPC interaction consumer must not retain a duplicate local distance constant"
    );
}
