#![allow(dead_code)]

use crate::combat::components::{DerivedAttrs, Wounds};
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::npc::equipment::NpcEquipment;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatPowerScore(pub f32);

pub fn realm_ordinal(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

pub fn compute_combat_power(
    realm: Realm,
    cultivation: &Cultivation,
    wounds: &Wounds,
    derived: &DerivedAttrs,
    techniques: &KnownTechniques,
    equipment: Option<&NpcEquipment>,
) -> CombatPowerScore {
    let realm_weight = realm_ordinal(realm) as f32 * 20.0;
    let hp_ratio = if wounds.health_max > 0.0 {
        (wounds.health_current / wounds.health_max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let qi_ratio = if cultivation.qi_max > 0.0 {
        (cultivation.qi_current / cultivation.qi_max).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let condition_factor = (hp_ratio * 0.6 + qi_ratio * 0.4).max(0.15);
    let combat_attrs = derived.attack_power + derived.defense_power;
    let tech_count = techniques.entries.len() as f32 * 2.0;
    let equip_quality = equipment.map(|e| e.total_quality_score()).unwrap_or(0.0);
    CombatPowerScore((realm_weight + combat_attrs + tech_count + equip_quality) * condition_factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_wounds() -> Wounds {
        Wounds {
            entries: Vec::new(),
            health_current: 100.0,
            health_max: 100.0,
        }
    }

    fn default_cultivation(realm: Realm) -> Cultivation {
        let qi_max = match realm {
            Realm::Awaken => 10.0,
            Realm::Induce => 30.0,
            Realm::Condense => 60.0,
            Realm::Solidify => 120.0,
            Realm::Spirit => 200.0,
            Realm::Void => 400.0,
        };
        Cultivation {
            realm,
            qi_current: qi_max,
            qi_max,
            ..Default::default()
        }
    }

    fn default_derived() -> DerivedAttrs {
        DerivedAttrs {
            attack_power: 5.0,
            defense_power: 3.0,
            ..Default::default()
        }
    }

    fn empty_techniques() -> KnownTechniques {
        KnownTechniques {
            entries: Vec::new(),
        }
    }

    fn techniques_with_n(n: usize) -> KnownTechniques {
        KnownTechniques {
            entries: (0..n)
                .map(|i| crate::cultivation::known_techniques::KnownTechnique {
                    id: format!("tech_{i}"),
                    proficiency: 0.5,
                    active: true,
                })
                .collect(),
        }
    }

    #[test]
    fn realm_ordinal_ascending() {
        let realms = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        for (i, realm) in realms.iter().enumerate() {
            assert_eq!(
                realm_ordinal(*realm),
                i as u8,
                "realm_ordinal({realm:?}) should be {i}"
            );
        }
    }

    #[test]
    fn higher_realm_higher_power_same_conditions() {
        let wounds = default_wounds();
        let derived = default_derived();
        let techniques = empty_techniques();

        let awaken_power = compute_combat_power(
            Realm::Awaken,
            &default_cultivation(Realm::Awaken),
            &wounds,
            &derived,
            &techniques,
            None,
        );
        let induce_power = compute_combat_power(
            Realm::Induce,
            &default_cultivation(Realm::Induce),
            &wounds,
            &derived,
            &techniques,
            None,
        );
        let void_power = compute_combat_power(
            Realm::Void,
            &default_cultivation(Realm::Void),
            &wounds,
            &derived,
            &techniques,
            None,
        );
        assert!(
            induce_power.0 > awaken_power.0,
            "Induce ({}) should be > Awaken ({})",
            induce_power.0,
            awaken_power.0
        );
        assert!(
            void_power.0 > induce_power.0,
            "Void ({}) should be > Induce ({})",
            void_power.0,
            induce_power.0
        );
    }

    #[test]
    fn full_health_higher_than_half_health() {
        let cultivation = default_cultivation(Realm::Induce);
        let derived = default_derived();
        let techniques = empty_techniques();

        let full = compute_combat_power(
            Realm::Induce,
            &cultivation,
            &Wounds {
                entries: Vec::new(),
                health_current: 100.0,
                health_max: 100.0,
            },
            &derived,
            &techniques,
            None,
        );
        let half = compute_combat_power(
            Realm::Induce,
            &cultivation,
            &Wounds {
                entries: Vec::new(),
                health_current: 50.0,
                health_max: 100.0,
            },
            &derived,
            &techniques,
            None,
        );
        assert!(
            full.0 > half.0,
            "full health ({}) should be > half health ({})",
            full.0,
            half.0
        );
    }

    #[test]
    fn condition_factor_floor_at_015() {
        let cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 0.0,
            qi_max: 30.0,
            ..Default::default()
        };
        let wounds = Wounds {
            entries: Vec::new(),
            health_current: 0.0,
            health_max: 100.0,
        };
        let derived = default_derived();
        let techniques = empty_techniques();

        let power = compute_combat_power(
            Realm::Induce,
            &cultivation,
            &wounds,
            &derived,
            &techniques,
            None,
        );
        let base = 20.0 + 5.0 + 3.0;
        let expected = base * 0.15;
        assert!(
            (power.0 - expected).abs() < 1e-4,
            "at hp=0, qi=0: power should be {} (base {} * 0.15), got {}",
            expected,
            base,
            power.0
        );
    }

    #[test]
    fn condition_factor_at_half_hp_full_qi() {
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 10.0,
            qi_max: 10.0,
            ..Default::default()
        };
        let wounds = Wounds {
            entries: Vec::new(),
            health_current: 50.0,
            health_max: 100.0,
        };
        let derived = default_derived();
        let techniques = empty_techniques();

        let power = compute_combat_power(
            Realm::Awaken,
            &cultivation,
            &wounds,
            &derived,
            &techniques,
            None,
        );
        let hp_ratio = 0.5_f32;
        let qi_ratio = 1.0_f32;
        let expected_factor = hp_ratio * 0.6 + qi_ratio * 0.4;
        let base = 0.0 + 5.0 + 3.0;
        let expected = base * expected_factor;
        assert!(
            (power.0 - expected).abs() < 1e-4,
            "half_hp+full_qi: expected {expected}, got {}",
            power.0
        );
    }

    #[test]
    fn techniques_increase_power() {
        let cultivation = default_cultivation(Realm::Awaken);
        let wounds = default_wounds();
        let derived = default_derived();

        let no_tech = compute_combat_power(
            Realm::Awaken,
            &cultivation,
            &wounds,
            &derived,
            &empty_techniques(),
            None,
        );
        let with_tech = compute_combat_power(
            Realm::Awaken,
            &cultivation,
            &wounds,
            &derived,
            &techniques_with_n(5),
            None,
        );
        assert!(
            with_tech.0 > no_tech.0,
            "5 techniques ({}) should be > 0 techniques ({})",
            with_tech.0,
            no_tech.0
        );
    }

    #[test]
    fn equipment_increases_power() {
        let cultivation = default_cultivation(Realm::Awaken);
        let wounds = default_wounds();
        let derived = default_derived();
        let techniques = empty_techniques();

        let no_equip = compute_combat_power(
            Realm::Awaken,
            &cultivation,
            &wounds,
            &derived,
            &techniques,
            None,
        );

        let eq = NpcEquipment {
            main_hand: Some(crate::npc::equipment::NpcEquipSlot {
                template_id: "iron_sword".to_string(),
                display_name: "iron sword".to_string(),
                item_kind: valence::prelude::ItemKind::IronSword,
                quality_tier: 1,
                base_attack: 10.0,
                weapon_kind: Some(crate::combat::weapon::WeaponKind::Sword),
                armor_profile_id: None,
                durability_ratio: 0.9,
            }),
            head: Some(crate::npc::equipment::NpcEquipSlot {
                template_id: "iron_helmet".to_string(),
                display_name: "iron helmet".to_string(),
                item_kind: valence::prelude::ItemKind::IronHelmet,
                quality_tier: 1,
                base_attack: 0.0,
                weapon_kind: None,
                armor_profile_id: Some("armor_iron_helmet".to_string()),
                durability_ratio: 0.8,
            }),
            ..Default::default()
        };
        let with_equip = compute_combat_power(
            Realm::Awaken,
            &cultivation,
            &wounds,
            &derived,
            &techniques,
            Some(&eq),
        );
        assert!(
            with_equip.0 > no_equip.0,
            "equipped ({}) should be > unarmed ({})",
            with_equip.0,
            no_equip.0
        );
    }

    #[test]
    fn zero_health_max_does_not_panic() {
        let cultivation = default_cultivation(Realm::Awaken);
        let wounds = Wounds {
            entries: Vec::new(),
            health_current: 0.0,
            health_max: 0.0,
        };
        let derived = default_derived();
        let techniques = empty_techniques();

        let power = compute_combat_power(
            Realm::Awaken,
            &cultivation,
            &wounds,
            &derived,
            &techniques,
            None,
        );
        assert!(
            power.0.is_finite(),
            "power should be finite, got {}",
            power.0
        );
    }

    #[test]
    fn zero_qi_max_does_not_panic() {
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 0.0,
            qi_max: 0.0,
            ..Default::default()
        };
        let wounds = default_wounds();
        let derived = default_derived();
        let techniques = empty_techniques();

        let power = compute_combat_power(
            Realm::Awaken,
            &cultivation,
            &wounds,
            &derived,
            &techniques,
            None,
        );
        assert!(
            power.0.is_finite(),
            "power should be finite, got {}",
            power.0
        );
    }

    #[test]
    fn qi_ratio_multiplier_at_various_levels() {
        let wounds = default_wounds();
        let derived = default_derived();
        let techniques = empty_techniques();

        let full_qi = Cultivation {
            realm: Realm::Induce,
            qi_current: 30.0,
            qi_max: 30.0,
            ..Default::default()
        };
        let half_qi = Cultivation {
            realm: Realm::Induce,
            qi_current: 15.0,
            qi_max: 30.0,
            ..Default::default()
        };
        let zero_qi = Cultivation {
            realm: Realm::Induce,
            qi_current: 0.0,
            qi_max: 30.0,
            ..Default::default()
        };

        let p_full = compute_combat_power(
            Realm::Induce,
            &full_qi,
            &wounds,
            &derived,
            &techniques,
            None,
        );
        let p_half = compute_combat_power(
            Realm::Induce,
            &half_qi,
            &wounds,
            &derived,
            &techniques,
            None,
        );
        let p_zero = compute_combat_power(
            Realm::Induce,
            &zero_qi,
            &wounds,
            &derived,
            &techniques,
            None,
        );

        assert!(
            p_full.0 > p_half.0,
            "full qi ({}) should be > half qi ({})",
            p_full.0,
            p_half.0
        );
        assert!(
            p_half.0 > p_zero.0,
            "half qi ({}) should be > zero qi ({})",
            p_half.0,
            p_zero.0
        );
    }

    #[test]
    fn empty_equipment_quality_score_is_zero() {
        let eq = NpcEquipment::default();
        assert!(
            (eq.total_quality_score() - 0.0).abs() < 1e-5,
            "empty equipment quality score should be 0"
        );
    }

    #[test]
    fn power_always_positive() {
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let cultivation = default_cultivation(realm);
            let wounds = Wounds {
                entries: Vec::new(),
                health_current: 1.0,
                health_max: 100.0,
            };
            let derived = DerivedAttrs {
                attack_power: 0.0,
                defense_power: 0.0,
                ..Default::default()
            };
            let techniques = empty_techniques();
            let power =
                compute_combat_power(realm, &cultivation, &wounds, &derived, &techniques, None);
            assert!(
                power.0 >= 0.0,
                "power should always be >= 0 for {realm:?}, got {}",
                power.0
            );
        }
    }
}
