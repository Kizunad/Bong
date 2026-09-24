use bong_server::craft::{is_within_workbench_range, WORKBENCH_INTERACT_RANGE};
use bong_server::network::gate::{DistanceRule, WORKBENCH_MAX_BLOCKS};

fn gate_allows(player_pos: [f64; 3], workbench_pos: [i32; 3]) -> bool {
    DistanceRule::WORKBENCH.allows(player_pos, workbench_pos.map(f64::from))
}

fn assert_adapter_matches_profile(
    label: &str,
    player_pos: [f64; 3],
    workbench_pos: [i32; 3],
    expected: bool,
) {
    let profile_result = gate_allows(player_pos, workbench_pos);
    let adapter_result = is_within_workbench_range(player_pos, workbench_pos);

    assert_eq!(
        profile_result, expected,
        "{label}: DistanceRule::WORKBENCH returned an unexpected reach result"
    );
    assert_eq!(
        adapter_result, profile_result,
        "{label}: craft adapter must preserve the authoritative Workbench profile"
    );
}

#[test]
fn workbench_adapter_uses_the_gate_profile_at_origin_and_boundaries() {
    assert_eq!(
        WORKBENCH_INTERACT_RANGE, WORKBENCH_MAX_BLOCKS,
        "the legacy craft constant must remain an alias of the gate-owned radius"
    );

    assert_adapter_matches_profile("same origin", [0.0, 0.0, 0.0], [0, 0, 0], true);
    assert_adapter_matches_profile("axial boundary", [3.0, 0.0, 0.0], [0, 0, 0], true);
    assert_adapter_matches_profile(
        "Chebyshev diagonal boundary",
        [3.0, 3.0, 3.0],
        [0, 0, 0],
        true,
    );
}

#[test]
fn workbench_adapter_rejects_just_beyond_the_profile() {
    let just_outside = f64::from_bits(WORKBENCH_MAX_BLOCKS.to_bits() + 1);
    assert!(
        just_outside > WORKBENCH_MAX_BLOCKS,
        "the boundary witness must be the next representable value above the profile radius"
    );

    assert_adapter_matches_profile(
        "one ULP beyond the axial boundary",
        [just_outside, 0.0, 0.0],
        [0, 0, 0],
        false,
    );
    assert_adapter_matches_profile(
        "one ULP beyond a diagonal component",
        [3.0, just_outside, 3.0],
        [0, 0, 0],
        false,
    );
}

#[test]
fn workbench_adapter_preserves_negative_coordinate_reach() {
    assert_adapter_matches_profile(
        "negative-coordinate diagonal boundary",
        [-7.0, -8.0, -9.0],
        [-10, -5, -6],
        true,
    );
    assert_adapter_matches_profile(
        "negative-coordinate out of range",
        [-13.0, -5.0, -6.0],
        [-9, -5, -6],
        false,
    );
}

#[test]
fn workbench_adapter_fails_closed_for_non_finite_player_coordinates() {
    for (label, coordinate) in [
        ("NaN", f64::NAN),
        ("positive infinity", f64::INFINITY),
        ("negative infinity", f64::NEG_INFINITY),
    ] {
        for axis in 0..3 {
            let mut player_pos = [0.0, 0.0, 0.0];
            player_pos[axis] = coordinate;
            assert_adapter_matches_profile(label, player_pos, [0, 0, 0], false);
        }
    }
}
