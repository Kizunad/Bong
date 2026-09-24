use bong_server::coffin::is_coffin_target_in_range;
use bong_server::reach::{DistanceMetric, DistanceRule, NEARBY_INTERACT_MAX_BLOCKS};
use valence::prelude::{BlockPos, DVec3};

const TARGET: BlockPos = BlockPos::new(0, 0, 0);
const TARGET_CENTER: DVec3 = DVec3::new(0.5, 0.5, 0.5);

fn next_f64(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

fn profile_allows(player_pos: DVec3, target: BlockPos) -> bool {
    let target_center = DVec3::new(
        f64::from(target.x) + 0.5,
        f64::from(target.y) + 0.5,
        f64::from(target.z) + 0.5,
    );
    DistanceRule::NEARBY_INTERACT.allows(player_pos, target_center)
}

fn assert_adapter_matches_profile(
    label: &str,
    player_pos: DVec3,
    target: BlockPos,
    expected: bool,
) {
    let profile_result = profile_allows(player_pos, target);
    let adapter_result = is_coffin_target_in_range(player_pos, target);

    assert_eq!(
        profile_result, expected,
        "{label}: DistanceRule::NEARBY_INTERACT returned an unexpected reach result"
    );
    assert_eq!(
        adapter_result, profile_result,
        "{label}: coffin adapter must preserve the authoritative reach profile"
    );
}

#[test]
fn coffin_adapter_uses_nearby_interact_at_face_and_exact_boundary() {
    assert_eq!(
        DistanceRule::NEARBY_INTERACT.profile_parts(),
        Some((
            DistanceMetric::Euclidean3dSquared,
            NEARBY_INTERACT_MAX_BLOCKS,
        )),
        "延寿棺应使用共享 NearbyInteract 的三维欧氏平方距离与 6.0 格半径"
    );
    assert_adapter_matches_profile("same target center", TARGET_CENTER, TARGET, true);
    assert_adapter_matches_profile(
        "face-to-face adjacent position",
        TARGET_CENTER + DVec3::new(1.0, 0.0, 0.0),
        TARGET,
        true,
    );
    assert_adapter_matches_profile(
        "exact six-block axial boundary",
        TARGET_CENTER + DVec3::new(NEARBY_INTERACT_MAX_BLOCKS, 0.0, 0.0),
        TARGET,
        true,
    );
    assert_adapter_matches_profile(
        "inside Euclidean diagonal",
        TARGET_CENTER + DVec3::new(3.0, 4.0, 0.0),
        TARGET,
        true,
    );
}

#[test]
fn coffin_adapter_rejects_one_ulp_beyond_and_outside_diagonal() {
    let one_ulp_beyond = next_f64(NEARBY_INTERACT_MAX_BLOCKS);
    assert!(
        one_ulp_beyond > NEARBY_INTERACT_MAX_BLOCKS,
        "the boundary witness must be the next representable value above six blocks"
    );

    assert_adapter_matches_profile(
        "one ULP beyond the axial boundary",
        TARGET_CENTER + DVec3::new(one_ulp_beyond, 0.0, 0.0),
        TARGET,
        false,
    );
    assert_adapter_matches_profile(
        "outside Euclidean diagonal",
        TARGET_CENTER + DVec3::new(4.5, 4.5, 0.0),
        TARGET,
        false,
    );
}

#[test]
fn coffin_adapter_preserves_block_centering_for_negative_coordinates() {
    let target = BlockPos::new(-10, -20, -30);
    let target_center = DVec3::new(-9.5, -19.5, -29.5);
    let exact_boundary_z = target_center.z + NEARBY_INTERACT_MAX_BLOCKS;
    let one_ulp_outside_z = f64::from_bits(exact_boundary_z.to_bits() - 1);

    assert!(
        one_ulp_outside_z > exact_boundary_z,
        "negative-coordinate boundary witness must move toward positive infinity"
    );
    assert!(
        one_ulp_outside_z - target_center.z > NEARBY_INTERACT_MAX_BLOCKS,
        "negative-coordinate boundary witness must remain strictly beyond six blocks after subtraction"
    );

    assert_adapter_matches_profile(
        "negative-coordinate target center",
        target_center,
        target,
        true,
    );
    assert_adapter_matches_profile(
        "negative-coordinate exact boundary",
        target_center + DVec3::new(0.0, 0.0, NEARBY_INTERACT_MAX_BLOCKS),
        target,
        true,
    );
    assert_adapter_matches_profile(
        "negative-coordinate outside boundary",
        DVec3::new(target_center.x, target_center.y, one_ulp_outside_z),
        target,
        false,
    );
}

#[test]
fn coffin_adapter_fails_closed_for_non_finite_player_coordinates() {
    for (label, coordinate) in [
        ("NaN", f64::NAN),
        ("positive infinity", f64::INFINITY),
        ("negative infinity", f64::NEG_INFINITY),
    ] {
        for axis in 0..3 {
            let mut player_pos = TARGET_CENTER;
            player_pos[axis] = coordinate;
            assert_adapter_matches_profile(label, player_pos, TARGET, false);
        }
    }
}
