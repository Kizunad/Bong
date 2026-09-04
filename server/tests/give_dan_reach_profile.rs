use bong_server::reach::{DistanceMetric, DistanceRule, NEARBY_INTERACT_MAX_BLOCKS};
use std::fs;

fn next_f64(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

#[test]
fn give_dan_uses_the_named_euclidean_nearby_profile() {
    let profile = DistanceRule::NEARBY_INTERACT;

    assert_eq!(
        profile.profile_parts(),
        Some((
            DistanceMetric::Euclidean3dSquared,
            NEARBY_INTERACT_MAX_BLOCKS,
        )),
        "give_dan must use the shared 6.0-block Euclidean profile, not a local distance rule"
    );
    assert_eq!(
        profile,
        DistanceRule::nearby_interact(),
        "the frozen NearbyInteract constant and constructor must describe one give_dan policy"
    );
}

#[test]
fn give_dan_reach_includes_exact_boundary_and_rejects_one_ulp_beyond() {
    let profile = DistanceRule::NEARBY_INTERACT;
    let radius = NEARBY_INTERACT_MAX_BLOCKS;
    let one_ulp_beyond = next_f64(radius);

    assert!(
        profile.allows([0.0, 0.0, 0.0], [radius, 0.0, 0.0]),
        "exactly 6.0 Euclidean blocks must remain inclusive for give_dan"
    );
    assert!(
        !profile.allows([0.0, 0.0, 0.0], [one_ulp_beyond, 0.0, 0.0]),
        "one representable ULP beyond the 6.0-block Euclidean boundary must be rejected"
    );
}

#[test]
fn give_dan_reach_preserves_euclidean_diagonal_boundary() {
    let profile = DistanceRule::NEARBY_INTERACT;

    assert!(
        profile.allows([0.0, 0.0, 0.0], [4.0, 4.0, 2.0]),
        "the Euclidean diagonal (4,4,2) has distance exactly 6.0 and must remain reachable"
    );
    assert!(
        !profile.allows([0.0, 0.0, 0.0], [4.0, 4.0, 2.01]),
        "moving clearly beyond the Euclidean diagonal boundary must be rejected"
    );
}

#[test]
fn give_dan_reach_preserves_negative_coordinate_semantics() {
    let profile = DistanceRule::NEARBY_INTERACT;

    assert!(
        profile.allows([-10.0, -20.0, -30.0], [-4.0, -20.0, -30.0]),
        "negative-coordinate positions exactly 6.0 blocks apart must remain reachable"
    );
    assert!(
        !profile.allows([-10.0, -20.0, -30.0], [-4.0, -15.0, -26.0]),
        "negative-coordinate positions beyond Euclidean reach must be rejected"
    );
}

#[test]
fn give_dan_reach_fails_closed_for_non_finite_coordinates() {
    let profile = DistanceRule::NEARBY_INTERACT;

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
fn give_dan_handler_keeps_dimension_order_and_rejects_before_emitting() {
    let source_path = format!(
        "{}/src/network/client_request_handler.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("read give_dan consumer source {source_path}: {error}"));

    assert!(
        !source.contains("GIVE_DAN_MAX_DISTANCE"),
        "give_dan must not retain a duplicate local distance constant"
    );

    let scope_start = source
        .find("fn is_give_dan_target_in_scope(")
        .expect("give_dan scope adapter must remain present");
    let scope_end = source[scope_start..]
        .find("\nfn reject_give_dan_target")
        .map(|offset| scope_start + offset)
        .expect("give_dan scope adapter boundary must remain explicit");
    let scope = &source[scope_start..scope_end];
    let normalized_scope: String = scope.split_whitespace().collect();

    assert!(
        normalized_scope.contains(
            "player_dimension==elder_dimension&&crate::reach::DistanceRule::NEARBY_INTERACT.allows(player_position,elder_position)"
        ),
        "give_dan must compare dimensions first, then call the shared NearbyInteract predicate"
    );

    let handler_start = source
        .find("fn handle_give_dan_to_elder(")
        .expect("give_dan handler must remain present");
    let handler_end = source[handler_start..]
        .find("\n#[cfg(test)]")
        .map(|offset| handler_start + offset)
        .expect("give_dan handler boundary must remain explicit");
    let handler = &source[handler_start..handler_end];
    let scope_check = handler
        .find("if !is_give_dan_target_in_scope(")
        .expect("give_dan handler must retain its reach rejection branch");
    let reject = handler[scope_check..]
        .find("reject_give_dan_target(")
        .map(|offset| scope_check + offset)
        .expect("out-of-scope give_dan requests must use the existing rejection path");
    let reject_return = handler[reject..]
        .find("return;")
        .map(|offset| reject + offset)
        .expect("out-of-scope give_dan requests must return before dispatch");
    let emit = handler
        .find("tx.send(GiveDanToElderIntent {")
        .expect("give_dan success path must retain its intent dispatch");

    assert!(
        scope_check < reject && reject < reject_return && reject_return < emit,
        "give_dan must reject out-of-scope targets before emitting the intent"
    );
    assert!(
        !handler[scope_check..emit].contains("tx.send("),
        "the give_dan reach rejection path must not emit an intent or mutate downstream state"
    );
}
