#![allow(dead_code)]

use crate::combat::components::Wounds;
use crate::cultivation::components::{Cultivation, Realm};
use crate::npc::combat_power::{realm_ordinal, CombatPowerScore};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AbstractCombatOutcome {
    AttackerWins {
        damage_ratio: f32,
        attacker_qi_cost: f64,
        defender_qi_cost: f64,
    },
    DefenderWins {
        damage_ratio: f32,
        attacker_qi_cost: f64,
        defender_qi_cost: f64,
    },
}

pub fn avg_qi_cost_for_realm(realm: Realm) -> f64 {
    2.0 + realm_ordinal(realm) as f64 * 3.0
}

fn splitmix64(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn splitmix64_unit(seed: u64) -> f32 {
    let bits = ((splitmix64(seed) >> 40) & 0x00FF_FFFF) as u32;
    bits as f32 / (1u32 << 24) as f32
}

pub fn abstract_combat_resolve(
    attacker_power: CombatPowerScore,
    defender_power: CombatPowerScore,
    attacker_realm: Realm,
    defender_realm: Realm,
    seed: u64,
) -> AbstractCombatOutcome {
    let ratio = attacker_power.0 / (attacker_power.0 + defender_power.0 + f32::EPSILON);
    let roll = splitmix64_unit(seed);
    if roll < ratio {
        AbstractCombatOutcome::AttackerWins {
            damage_ratio: ratio * 0.3,
            attacker_qi_cost: avg_qi_cost_for_realm(attacker_realm),
            defender_qi_cost: avg_qi_cost_for_realm(defender_realm) * 0.5,
        }
    } else {
        AbstractCombatOutcome::DefenderWins {
            damage_ratio: (1.0 - ratio) * 0.3,
            attacker_qi_cost: avg_qi_cost_for_realm(attacker_realm),
            defender_qi_cost: avg_qi_cost_for_realm(defender_realm),
        }
    }
}

pub fn apply_outcome_to_loser(wounds: &mut Wounds, damage_ratio: f32) {
    let damage = wounds.health_max * damage_ratio;
    wounds.health_current = (wounds.health_current - damage).max(0.0);
}

pub fn apply_qi_cost(cultivation: &mut Cultivation, qi_cost: f64) -> f64 {
    let actual = qi_cost.min(cultivation.qi_current.max(0.0));
    cultivation.qi_current = (cultivation.qi_current - actual).max(0.0);
    actual
}

#[cfg(test)]
mod tests {
    use super::*;

    fn power(v: f32) -> CombatPowerScore {
        CombatPowerScore(v)
    }

    #[test]
    fn avg_qi_cost_scales_with_realm() {
        let awaken = avg_qi_cost_for_realm(Realm::Awaken);
        let induce = avg_qi_cost_for_realm(Realm::Induce);
        let void = avg_qi_cost_for_realm(Realm::Void);
        assert!(
            induce > awaken,
            "Induce qi cost ({induce}) should > Awaken ({awaken})"
        );
        assert!(
            void > induce,
            "Void qi cost ({void}) should > Induce ({induce})"
        );
        assert!(
            (awaken - 2.0).abs() < 1e-9,
            "Awaken qi cost should be 2.0, got {awaken}"
        );
        assert!(
            (void - 17.0).abs() < 1e-9,
            "Void qi cost should be 17.0, got {void}"
        );
    }

    #[test]
    fn avg_qi_cost_all_realms() {
        let expected = [2.0, 5.0, 8.0, 11.0, 14.0, 17.0];
        let realms = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        for (realm, exp) in realms.iter().zip(expected.iter()) {
            let cost = avg_qi_cost_for_realm(*realm);
            assert!(
                (cost - exp).abs() < 1e-9,
                "{realm:?} qi cost expected {exp}, got {cost}"
            );
        }
    }

    #[test]
    fn probability_distribution_matches_power_ratio() {
        let attacker = power(70.0);
        let defender = power(30.0);

        let mut attacker_wins = 0;
        let trials = 10_000;
        for seed in 0..trials {
            match abstract_combat_resolve(
                attacker,
                defender,
                Realm::Induce,
                Realm::Awaken,
                seed * 7919 + 42,
            ) {
                AbstractCombatOutcome::AttackerWins { .. } => attacker_wins += 1,
                AbstractCombatOutcome::DefenderWins { .. } => {}
            }
        }
        let win_rate = attacker_wins as f64 / trials as f64;
        assert!(
            (win_rate - 0.7).abs() < 0.05,
            "attacker with 70/100 power should win ~70% of the time, got {:.1}%",
            win_rate * 100.0
        );
    }

    #[test]
    fn equal_power_roughly_50_50() {
        let p = power(50.0);

        let mut attacker_wins = 0;
        let trials = 10_000;
        for seed in 0..trials {
            match abstract_combat_resolve(p, p, Realm::Induce, Realm::Induce, seed * 6271 + 123) {
                AbstractCombatOutcome::AttackerWins { .. } => attacker_wins += 1,
                AbstractCombatOutcome::DefenderWins { .. } => {}
            }
        }
        let win_rate = attacker_wins as f64 / trials as f64;
        assert!(
            (win_rate - 0.5).abs() < 0.05,
            "equal power should be ~50/50, got {:.1}%",
            win_rate * 100.0
        );
    }

    #[test]
    fn attacker_wins_deals_damage_to_defender() {
        let mut wounds = Wounds {
            entries: Vec::new(),
            health_current: 100.0,
            health_max: 100.0,
        };
        apply_outcome_to_loser(&mut wounds, 0.21);
        assert!(
            (wounds.health_current - 79.0).abs() < 1e-4,
            "defender hp should be 79.0, got {}",
            wounds.health_current
        );
    }

    #[test]
    fn defender_wins_deals_damage_to_attacker() {
        let mut wounds = Wounds {
            entries: Vec::new(),
            health_current: 100.0,
            health_max: 100.0,
        };
        apply_outcome_to_loser(&mut wounds, 0.09);
        assert!(
            (wounds.health_current - 91.0).abs() < 1e-4,
            "attacker hp should be 91.0, got {}",
            wounds.health_current
        );
    }

    #[test]
    fn apply_outcome_clamps_hp_to_zero() {
        let mut wounds = Wounds {
            entries: Vec::new(),
            health_current: 5.0,
            health_max: 100.0,
        };
        apply_outcome_to_loser(&mut wounds, 0.3);
        assert!(
            (wounds.health_current - 0.0).abs() < 1e-4,
            "hp should be clamped to 0, got {}",
            wounds.health_current
        );
    }

    #[test]
    fn apply_qi_cost_deducts_and_returns_actual() {
        let mut cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 10.0,
            qi_max: 30.0,
            ..Default::default()
        };
        let actual = apply_qi_cost(&mut cultivation, 5.0);
        assert!(
            (actual - 5.0).abs() < 1e-9,
            "should deduct 5.0, got {actual}"
        );
        assert!(
            (cultivation.qi_current - 5.0).abs() < 1e-9,
            "qi_current should be 5.0, got {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn apply_qi_cost_clamps_to_available() {
        let mut cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 3.0,
            qi_max: 10.0,
            ..Default::default()
        };
        let actual = apply_qi_cost(&mut cultivation, 10.0);
        assert!(
            (actual - 3.0).abs() < 1e-9,
            "should only deduct available 3.0, got {actual}"
        );
        assert!(
            (cultivation.qi_current - 0.0).abs() < 1e-9,
            "qi_current should be 0.0, got {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn apply_qi_cost_zero_qi_returns_zero() {
        let mut cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 0.0,
            qi_max: 10.0,
            ..Default::default()
        };
        let actual = apply_qi_cost(&mut cultivation, 5.0);
        assert!(
            (actual - 0.0).abs() < 1e-9,
            "zero qi should return 0.0, got {actual}"
        );
    }

    #[test]
    fn damage_ratio_proportional_to_power_ratio() {
        let strong = power(80.0);
        let weak = power(20.0);
        let expected_ratio = 80.0 / (80.0 + 20.0 + f32::EPSILON);
        let expected_attacker_damage = expected_ratio * 0.3;
        let expected_defender_damage = (1.0 - expected_ratio) * 0.3;

        let mut saw_attacker_wins = false;
        let mut saw_defender_wins = false;
        for seed in 0..1000_u64 {
            match abstract_combat_resolve(strong, weak, Realm::Void, Realm::Awaken, seed * 31) {
                AbstractCombatOutcome::AttackerWins { damage_ratio, .. } => {
                    assert!(
                        (damage_ratio - expected_attacker_damage).abs() < 1e-4,
                        "attacker_wins damage_ratio should be {expected_attacker_damage}, got {damage_ratio}"
                    );
                    saw_attacker_wins = true;
                }
                AbstractCombatOutcome::DefenderWins { damage_ratio, .. } => {
                    assert!(
                        (damage_ratio - expected_defender_damage).abs() < 1e-4,
                        "defender_wins damage_ratio should be {expected_defender_damage}, got {damage_ratio}"
                    );
                    saw_defender_wins = true;
                }
            }
            if saw_attacker_wins && saw_defender_wins {
                break;
            }
        }
        assert!(saw_attacker_wins, "should see attacker wins");
        assert!(saw_defender_wins, "should see defender wins");
    }

    #[test]
    fn qi_cost_uses_correct_realm_per_outcome() {
        let p = power(50.0);
        let mut saw_attacker_wins = false;
        let mut saw_defender_wins = false;
        for seed in 0..1000_u64 {
            match abstract_combat_resolve(p, p, Realm::Condense, Realm::Awaken, seed * 13) {
                AbstractCombatOutcome::AttackerWins {
                    attacker_qi_cost,
                    defender_qi_cost,
                    ..
                } => {
                    assert!(
                        (attacker_qi_cost - avg_qi_cost_for_realm(Realm::Condense)).abs() < 1e-9,
                        "attacker qi cost should match Condense realm"
                    );
                    assert!(
                        (defender_qi_cost - avg_qi_cost_for_realm(Realm::Awaken) * 0.5).abs()
                            < 1e-9,
                        "defender qi cost in AttackerWins should be half of Awaken realm cost"
                    );
                    saw_attacker_wins = true;
                }
                AbstractCombatOutcome::DefenderWins {
                    attacker_qi_cost,
                    defender_qi_cost,
                    ..
                } => {
                    assert!(
                        (attacker_qi_cost - avg_qi_cost_for_realm(Realm::Condense)).abs() < 1e-9,
                        "attacker qi cost should match Condense realm"
                    );
                    assert!(
                        (defender_qi_cost - avg_qi_cost_for_realm(Realm::Awaken)).abs() < 1e-9,
                        "defender qi cost in DefenderWins should be full Awaken realm cost"
                    );
                    saw_defender_wins = true;
                }
            }
            if saw_attacker_wins && saw_defender_wins {
                break;
            }
        }
        assert!(saw_attacker_wins, "should see attacker wins");
        assert!(saw_defender_wins, "should see defender wins");
    }

    #[test]
    fn zero_power_both_sides_does_not_panic() {
        let zero = power(0.0);
        let outcome = abstract_combat_resolve(zero, zero, Realm::Awaken, Realm::Awaken, 42);
        match outcome {
            AbstractCombatOutcome::AttackerWins { damage_ratio, .. }
            | AbstractCombatOutcome::DefenderWins { damage_ratio, .. } => {
                assert!(
                    damage_ratio.is_finite(),
                    "damage_ratio should be finite even with 0 power"
                );
            }
        }
    }

    #[test]
    fn splitmix64_unit_stays_in_range() {
        for seed in 0..10_000_u64 {
            let v = splitmix64_unit(seed);
            assert!(
                (0.0..1.0).contains(&v),
                "splitmix64_unit({seed}) = {v} outside [0,1)"
            );
        }
    }
}
