//! P0 MeridianAdjacency 测试。

use crate::combat::baomai_v4::adjacency::{adjacent_pairs_for, are_adjacent, MERIDIAN_ADJACENCY};
use crate::cultivation::components::MeridianId;

#[test]
fn adjacency_same_limb() {
    // 左臂三阴：LU↔HT, LU↔PC, HT↔PC
    assert!(
        are_adjacent(MeridianId::Lung, MeridianId::Heart),
        "Lung and Heart are same-limb (left arm), should be adjacent"
    );
    assert!(
        are_adjacent(MeridianId::Lung, MeridianId::Pericardium),
        "Lung and Pericardium are same-limb (left arm), should be adjacent"
    );
    // 右臂三阳：LI↔SI, SI↔TE, LI↔TE
    assert!(
        are_adjacent(MeridianId::LargeIntestine, MeridianId::SmallIntestine),
        "LI and SI are same-limb (right arm), should be adjacent"
    );
    // 左腿三阴：SP↔KI, KI↔LR, SP↔LR
    assert!(
        are_adjacent(MeridianId::Spleen, MeridianId::Kidney),
        "SP and KI are same-limb (left leg), should be adjacent"
    );
    // 右腿三阳：ST↔BL, BL↔GB, ST↔GB
    assert!(
        are_adjacent(MeridianId::Stomach, MeridianId::Bladder),
        "ST and BL are same-limb (right leg), should be adjacent"
    );
}

#[test]
fn adjacency_cross_limb() {
    // Cross-limb: Lung (left arm) and LargeIntestine (right arm) should NOT be adjacent.
    assert!(
        !are_adjacent(MeridianId::Lung, MeridianId::LargeIntestine),
        "Lung (left arm) and LargeIntestine (right arm) should not be adjacent"
    );
    // Cross-limb: Heart (left arm) and Stomach (right leg).
    assert!(
        !are_adjacent(MeridianId::Heart, MeridianId::Stomach),
        "Heart (left arm) and Stomach (right leg) should not be adjacent"
    );
    // Cross-limb: Kidney (left leg) and Bladder (right leg).
    assert!(
        !are_adjacent(MeridianId::Kidney, MeridianId::Bladder),
        "Kidney (left leg) and Bladder (right leg) should not be adjacent"
    );
}

#[test]
fn adjacency_ren_du() {
    assert!(
        are_adjacent(MeridianId::Ren, MeridianId::Du),
        "Ren and Du (任督桥) should be adjacent"
    );
    // Ren should NOT be adjacent to regular meridians.
    assert!(
        !are_adjacent(MeridianId::Ren, MeridianId::Lung),
        "Ren should not be adjacent to Lung"
    );
    assert!(
        !are_adjacent(MeridianId::Du, MeridianId::Heart),
        "Du should not be adjacent to Heart"
    );
}

#[test]
fn adjacency_symmetric() {
    // are_adjacent(a, b) == are_adjacent(b, a) for ALL pairs.
    for &(a, b) in MERIDIAN_ADJACENCY {
        assert!(
            are_adjacent(a, b) && are_adjacent(b, a),
            "Adjacency should be symmetric: {:?} ↔ {:?}",
            a,
            b
        );
    }
    // Also check a non-adjacent pair is symmetric in its negation.
    assert!(
        !are_adjacent(MeridianId::Lung, MeridianId::Stomach)
            && !are_adjacent(MeridianId::Stomach, MeridianId::Lung),
        "Non-adjacency should also be symmetric"
    );
}

#[test]
fn adjacency_self_is_false() {
    for &id in MeridianId::ALL.iter() {
        assert!(
            !are_adjacent(id, id),
            "A meridian should not be adjacent to itself: {:?}",
            id
        );
    }
}

#[test]
fn adjacent_pairs_for_returns_correct_neighbors() {
    let neighbors = adjacent_pairs_for(MeridianId::Kidney);
    assert!(
        neighbors.contains(&MeridianId::Spleen),
        "Kidney neighbors should include Spleen, got {:?}",
        neighbors
    );
    assert!(
        neighbors.contains(&MeridianId::Liver),
        "Kidney neighbors should include Liver, got {:?}",
        neighbors
    );
    assert_eq!(
        neighbors.len(),
        2,
        "Kidney should have exactly 2 neighbors (Spleen, Liver), got {}",
        neighbors.len()
    );

    let ren_neighbors = adjacent_pairs_for(MeridianId::Ren);
    assert_eq!(
        ren_neighbors,
        vec![MeridianId::Du],
        "Ren should only be adjacent to Du"
    );
}
