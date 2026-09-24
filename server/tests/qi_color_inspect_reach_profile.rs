use bong_server::reach::{DistanceMetric, DistanceRule, NEARBY_INTERACT_MAX_BLOCKS};
use std::fs;

fn next_f64(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

#[test]
fn qi_color_inspect_uses_the_named_euclidean_nearby_profile() {
    let profile = DistanceRule::NEARBY_INTERACT;

    assert_eq!(
        profile.profile_parts(),
        Some((
            DistanceMetric::Euclidean3dSquared,
            NEARBY_INTERACT_MAX_BLOCKS,
        )),
        "QiColorInspect must use the shared 6.0-block Euclidean profile, not a local distance rule"
    );
    assert_eq!(
        profile,
        DistanceRule::nearby_interact(),
        "the frozen NearbyInteract constant and constructor must describe one QiColorInspect policy"
    );
}

#[test]
fn qi_color_inspect_reach_includes_exact_boundary_and_rejects_one_ulp_beyond() {
    let profile = DistanceRule::NEARBY_INTERACT;
    let radius = NEARBY_INTERACT_MAX_BLOCKS;
    let one_ulp_beyond = next_f64(radius);

    assert!(
        profile.allows([0.0, 0.0, 0.0], [radius, 0.0, 0.0]),
        "exactly 6.0 Euclidean blocks must remain inclusive for QiColorInspect"
    );
    assert!(
        !profile.allows([0.0, 0.0, 0.0], [one_ulp_beyond, 0.0, 0.0]),
        "one representable ULP beyond the 6.0-block boundary must be rejected"
    );
}

#[test]
fn qi_color_inspect_reach_preserves_euclidean_diagonal_boundary() {
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
fn qi_color_inspect_reach_preserves_negative_coordinate_semantics() {
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
fn qi_color_inspect_reach_fails_closed_for_non_finite_coordinates() {
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
fn qi_color_inspect_handler_keeps_resolution_rejection_order_and_shared_wiring() {
    let source_path = format!(
        "{}/src/network/client_request_handler.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
        panic!("read QiColorInspect consumer source {source_path}: {error}")
    });

    assert!(
        !source.contains("QI_COLOR_INSPECT_MAX_DISTANCE"),
        "QiColorInspect must not retain a duplicate local distance constant"
    );

    let scope_start = source
        .find("fn is_qi_color_inspect_target_in_scope(")
        .expect("QiColorInspect scope helper must remain present");
    let scope_end = source[scope_start..]
        .find("\nfn is_qi_color_inspect_position_in_scope")
        .map(|offset| scope_start + offset)
        .expect("QiColorInspect scope helper boundary must remain explicit");
    let scope = &source[scope_start..scope_end];

    let self_target = scope
        .find("if observer == observed")
        .expect("self-target rejection must remain in the QiColorInspect resolver");
    let observer_position = scope
        .find("positions.get(observer)")
        .expect("observer position must be resolved after self-target rejection");
    let observed_position = scope
        .find("positions.get(observed)")
        .expect("observed position must be resolved after observer position");
    let observer_dimension = scope
        .find("dimension_kind_for(dimensions, observer)")
        .expect("observer dimension must remain part of scope validation");
    let observed_dimension = scope
        .find("dimension_kind_for(dimensions, observed)")
        .expect("observed dimension must remain part of scope validation");
    let position_check = scope
        .find("is_qi_color_inspect_position_in_scope(")
        .expect("position scope adapter must remain the final resolver check");

    assert!(
        self_target < observer_position
            && observer_position < observed_position
            && observed_position < observer_dimension
            && observer_dimension < observed_dimension
            && observed_dimension < position_check,
        "QiColorInspect must reject self targets, then resolve positions, then compare dimensions before reach"
    );

    let position_start = source
        .find("fn is_qi_color_inspect_position_in_scope(")
        .expect("QiColorInspect position helper must remain present");
    let position_end = source[position_start..]
        .find("\nfn dimension_kind_for")
        .map(|offset| position_start + offset)
        .expect("QiColorInspect position helper boundary must remain explicit");
    let position_helper = &source[position_start..position_end];
    let normalized_position: String = position_helper.split_whitespace().collect();
    assert!(
        normalized_position.contains(
            "same_dimension&&crate::reach::DistanceRule::NEARBY_INTERACT.allows(observer_position,observed_position)"
        ),
        "QiColorInspect position validation must call the shared NearbyInteract predicate"
    );

    let branch_start = source
        .find("            ClientRequestV1::QiColorInspect { observed, .. } => {")
        .expect("QiColorInspect dispatch branch must remain present");
    let branch_end = source[branch_start..]
        .find("\n            ClientRequestV1::UseLifeCore")
        .map(|offset| branch_start + offset)
        .expect("QiColorInspect dispatch branch boundary must remain explicit");
    let branch = &source[branch_start..branch_end];
    let resolve = branch
        .find("resolve_qi_color_inspect_target(")
        .expect("QiColorInspect dispatch must resolve the entity before emitting an event");
    let reject = branch
        .find("continue;")
        .expect("QiColorInspect rejection must continue before dispatch");
    let emit = branch
        .find("qi_color_inspect_tx.send(")
        .expect("QiColorInspect success path must retain its event dispatch");
    assert!(
        resolve < reject && reject < emit,
        "QiColorInspect must resolve, fail closed with no event, and only then emit QiColorInspectRequest"
    );
    assert!(
        !branch[..reject].contains("qi_color_inspect_tx.send("),
        "a rejected QiColorInspect request must not emit QiColorInspectRequest before returning"
    );
    assert!(
        branch[..reject].matches(".send(").count() == 0,
        "the rejection path must not perform another event-like side effect before it continues"
    );
}
