use bong_server::mineral::probe::is_probe_target_in_range;
use valence::prelude::{BlockPos, DVec3};

const TARGET: BlockPos = BlockPos::new(0, 0, 0);
const TARGET_CENTER: DVec3 = DVec3::new(0.5, 0.5, 0.5);

fn next_f64(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

#[test]
fn mineral_probe_reach_allows_face_to_face_and_block_center() {
    assert!(
        is_probe_target_in_range(TARGET_CENTER + DVec3::new(1.0, 0.0, 0.0), TARGET),
        "贴脸的一格相邻位置应通过 NearbyInteract 欧氏距离门"
    );
    assert!(
        is_probe_target_in_range(DVec3::new(0.5, 0.5, 0.5), TARGET),
        "站在方块中心时目标中心应通过 6.0 格门"
    );
}

#[test]
fn mineral_probe_reach_includes_exact_six_block_boundary_but_rejects_one_ulp_beyond() {
    let exact_boundary = TARGET_CENTER + DVec3::new(6.0, 0.0, 0.0);
    let one_ulp_beyond = TARGET_CENTER + DVec3::new(next_f64(6.0), 0.0, 0.0);

    assert!(
        is_probe_target_in_range(exact_boundary, TARGET),
        "恰好 6.0 格的欧氏边界必须 inclusive 放行"
    );
    assert!(
        !is_probe_target_in_range(one_ulp_beyond, TARGET),
        "超过 6.0 格一个 ULP 必须拒绝"
    );
}

#[test]
fn mineral_probe_reach_preserves_negative_coordinate_centering() {
    let target = BlockPos::new(-10, -20, -30);
    let center = DVec3::new(-9.5, -19.5, -29.5);

    assert!(
        is_probe_target_in_range(center, target),
        "负坐标方块中心应按半格偏移后通过"
    );
    assert!(
        is_probe_target_in_range(center + DVec3::new(0.0, 0.0, 6.0), target),
        "负坐标目标的 6.0 格边界应保持 inclusive"
    );
}

#[test]
fn mineral_probe_reach_fails_closed_for_non_finite_coordinates() {
    for (label, coordinate) in [
        ("NaN", f64::NAN),
        ("positive infinity", f64::INFINITY),
        ("negative infinity", f64::NEG_INFINITY),
    ] {
        for axis in 0..3 {
            let mut player_pos = TARGET_CENTER;
            player_pos[axis] = coordinate;
            assert!(
                !is_probe_target_in_range(player_pos, TARGET),
                "{label} requester coordinate on axis {axis} must fail closed"
            );
        }
    }
}
