//! 越级原则 - 池子差距矩阵。
//!
//! 只给"全力一击"这类一次性灌注使用；常规战斗仍走流派、武器、护甲和位置结算。

use crate::cultivation::components::Realm;

/// 6x6 池子差距矩阵。行 = 攻击者境界，列 = 防御者境界。
pub const REALM_GAP_MATRIX: [[f32; 6]; 6] = [
    [1.0, 0.25, 0.067, 0.019, 0.0048, 0.00093],
    [4.0, 1.0, 0.267, 0.074, 0.019, 0.0037],
    [15.0, 3.75, 1.0, 0.278, 0.071, 0.014],
    [54.0, 13.5, 3.6, 1.0, 0.257, 0.051],
    [210.0, 52.0, 14.0, 3.89, 1.0, 0.196],
    [1070.0, 268.0, 71.0, 19.8, 5.1, 1.0],
];

pub fn realm_gap_multiplier(attacker: Realm, defender: Realm) -> f32 {
    REALM_GAP_MATRIX[realm_index(attacker)][realm_index(defender)]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealmGapTier {
    Lower,
    Equal,
    OneStepUp,
    TwoStepUp,
    ThreeStepUp,
}

pub fn classify_gap(ratio: f32) -> RealmGapTier {
    match ratio {
        r if r < 0.95 => RealmGapTier::Lower,
        r if r < 1.5 => RealmGapTier::Equal,
        r if r < 6.0 => RealmGapTier::OneStepUp,
        r if r < 100.0 => RealmGapTier::TwoStepUp,
        _ => RealmGapTier::ThreeStepUp,
    }
}

pub fn realm_index(realm: Realm) -> usize {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REALMS: [Realm; 6] = [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ];

    #[test]
    fn realm_gap_matrix_diagonal_is_one() {
        for realm in REALMS {
            assert_eq!(realm_gap_multiplier(realm, realm), 1.0);
        }
    }

    #[test]
    fn realm_gap_matrix_inverse_relation() {
        for attacker in REALMS {
            for defender in REALMS {
                if attacker == defender {
                    continue;
                }
                let forward = realm_gap_multiplier(attacker, defender);
                let reverse = realm_gap_multiplier(defender, attacker);
                let product = forward * reverse;
                assert!(
                    (product - 1.0).abs() <= 0.08,
                    "{attacker:?}->{defender:?} inverse drift: {forward} * {reverse} = {product}"
                );
            }
        }
    }

    #[test]
    fn realm_gap_matrix_matches_worldview_table_for_all_pairs() {
        for (i, attacker) in REALMS.into_iter().enumerate() {
            for (j, defender) in REALMS.into_iter().enumerate() {
                let actual = realm_gap_multiplier(attacker, defender);
                let expected = REALM_GAP_MATRIX[i][j];
                assert!(
                    (actual - expected).abs() <= 0.05,
                    "unexpected multiplier for {attacker:?}->{defender:?}: {actual} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn classify_gap_boundaries_match_worldview_tiers() {
        assert_eq!(classify_gap(0.93), RealmGapTier::Lower);
        assert_eq!(classify_gap(1.0), RealmGapTier::Equal);
        assert_eq!(classify_gap(3.6), RealmGapTier::OneStepUp);
        assert_eq!(classify_gap(5.1), RealmGapTier::OneStepUp);
        assert_eq!(classify_gap(13.0), RealmGapTier::TwoStepUp);
        assert_eq!(classify_gap(71.0), RealmGapTier::TwoStepUp);
        assert_eq!(classify_gap(107.0), RealmGapTier::ThreeStepUp);
    }
}
