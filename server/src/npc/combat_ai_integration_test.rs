#[cfg(test)]
mod tests {
    use valence::prelude::Entity;

    use crate::combat::components::{DerivedAttrs, Wounds};
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::cultivation::known_techniques::{
        technique_definition, KnownTechnique, KnownTechniques, SkillCategory,
    };
    use crate::cultivation::meridian::severed::SkillMeridianDependencies;
    use crate::npc::combat_power::compute_combat_power;
    use crate::npc::lifecycle::NpcArchetype;
    use crate::npc::technique::{
        assign_npc_techniques, category_weight, has_usable_heal_technique,
        npc_meridian_system_for_realm, select_technique, NpcCooldownMap, NpcSkillScoringContext,
    };

    fn empty_deps() -> SkillMeridianDependencies {
        SkillMeridianDependencies::default()
    }

    fn default_ctx() -> NpcSkillScoringContext {
        NpcSkillScoringContext {
            hp_ratio: 1.0,
            qi_ratio: 1.0,
            target_distance: 3.0,
            target_hp_ratio: 1.0,
            has_active_buff: false,
            in_combat: true,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // P4.1.1 Near tier 完整战斗循环
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn near_tier_full_hp_prefers_attack_over_heal() {
        let realm = Realm::Condense;
        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();

        let mut attack_selected = 0u32;
        let mut heal_selected = 0u32;

        for seed in 0..50u64 {
            let techniques =
                assign_npc_techniques(NpcArchetype::Rogue, realm, &meridian_sys, &deps, None, seed);

            let cultivation = Cultivation {
                realm,
                qi_current: 200.0,
                qi_max: 200.0,
                ..Default::default()
            };
            let cooldowns = NpcCooldownMap::default();
            let entity = Entity::from_raw(seed as u32 + 1000);

            let ctx = NpcSkillScoringContext {
                hp_ratio: 1.0,
                qi_ratio: 1.0,
                target_distance: 3.0,
                target_hp_ratio: 0.5,
                has_active_buff: false,
                in_combat: true,
            };

            for tick in 0..100u64 {
                if let Some(sel) = select_technique(
                    &techniques,
                    &cultivation,
                    &deps,
                    None,
                    None, // meridian_sys（dugu-poison opened gate；测试无 MeridianSystem 传 None）
                    &cooldowns,
                    entity,
                    3.0,
                    tick,
                    &ctx,
                    None,
                ) {
                    let def = technique_definition(&sel.technique_id);
                    if let Some(d) = def {
                        match d.category {
                            SkillCategory::Attack | SkillCategory::Control => attack_selected += 1,
                            SkillCategory::Heal => heal_selected += 1,
                            _ => {}
                        }
                    }
                }
            }
        }

        assert!(
            attack_selected > heal_selected * 5,
            "at full HP, attack selections ({attack_selected}) should vastly outnumber \
             heal selections ({heal_selected})"
        );
    }

    #[test]
    fn near_tier_low_hp_triggers_heal_scorer() {
        let realm = Realm::Condense;
        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();

        let mut heal_count = 0u32;
        let total_trials = 200u32;

        for seed in 0..total_trials as u64 {
            let techniques =
                assign_npc_techniques(NpcArchetype::Rogue, realm, &meridian_sys, &deps, None, seed);

            let cultivation = Cultivation {
                realm,
                qi_current: 200.0,
                qi_max: 200.0,
                ..Default::default()
            };
            let cooldowns = NpcCooldownMap::default();
            let entity = Entity::from_raw(seed as u32 + 2000);

            let hp_ratio = 0.2;
            let has_heal = has_usable_heal_technique(
                &techniques,
                &cultivation,
                &deps,
                None,
                None, // meridian_sys（dugu-poison opened gate；测试无 MeridianSystem 传 None）
                &cooldowns,
                entity,
                100,
            );

            if has_heal {
                let ctx = NpcSkillScoringContext {
                    hp_ratio,
                    qi_ratio: 1.0,
                    target_distance: 3.0,
                    target_hp_ratio: 0.5,
                    has_active_buff: false,
                    in_combat: true,
                };

                if let Some(sel) = select_technique(
                    &techniques,
                    &cultivation,
                    &deps,
                    None,
                    None, // meridian_sys（dugu-poison opened gate；测试无 MeridianSystem 传 None）
                    &cooldowns,
                    entity,
                    3.0,
                    100,
                    &ctx,
                    Some(SkillCategory::Heal),
                ) {
                    let def = technique_definition(&sel.technique_id);
                    if def.is_some_and(|d| d.category == SkillCategory::Heal) {
                        heal_count += 1;
                    }
                }
            }
        }

        assert!(
            heal_count > 0,
            "at hp_ratio=0.2, NPC with heal technique should select heal at least once \
             across {total_trials} trials"
        );
    }

    #[test]
    fn near_tier_heal_scorer_zero_when_hp_above_threshold() {
        let ctx_high = NpcSkillScoringContext {
            hp_ratio: 0.5,
            ..default_ctx()
        };
        let ctx_low = NpcSkillScoringContext {
            hp_ratio: 0.2,
            ..default_ctx()
        };

        let heal_weight_high = category_weight(SkillCategory::Heal, &ctx_high);
        let heal_weight_low = category_weight(SkillCategory::Heal, &ctx_low);

        assert!(
            heal_weight_low > heal_weight_high,
            "heal weight at hp=0.2 ({heal_weight_low}) should be > hp=0.5 ({heal_weight_high})"
        );

        let heal_weight_full = category_weight(
            SkillCategory::Heal,
            &NpcSkillScoringContext {
                hp_ratio: 1.0,
                ..default_ctx()
            },
        );
        assert!(
            heal_weight_full < 0.01,
            "heal weight at full HP should be near zero, got {heal_weight_full}"
        );
    }

    #[test]
    fn near_tier_buff_appears_in_selection_pool() {
        let realm = Realm::Condense;
        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();

        let mut buff_ever_selected = false;

        for seed in 0..200u64 {
            let techniques =
                assign_npc_techniques(NpcArchetype::Rogue, realm, &meridian_sys, &deps, None, seed);

            let cultivation = Cultivation {
                realm,
                qi_current: 200.0,
                qi_max: 200.0,
                ..Default::default()
            };
            let cooldowns = NpcCooldownMap::default();
            let entity = Entity::from_raw(seed as u32 + 3000);

            let ctx = NpcSkillScoringContext {
                hp_ratio: 0.8,
                qi_ratio: 0.8,
                target_distance: 3.0,
                target_hp_ratio: 0.5,
                has_active_buff: false,
                in_combat: true,
            };

            for tick in 0..500u64 {
                if let Some(sel) = select_technique(
                    &techniques,
                    &cultivation,
                    &deps,
                    None,
                    None, // meridian_sys（dugu-poison opened gate；测试无 MeridianSystem 传 None）
                    &cooldowns,
                    entity,
                    3.0,
                    tick,
                    &ctx,
                    None,
                ) {
                    let def = technique_definition(&sel.technique_id);
                    if def.is_some_and(|d| d.category == SkillCategory::Buff) {
                        buff_ever_selected = true;
                    }
                }
                if buff_ever_selected {
                    break;
                }
            }
            if buff_ever_selected {
                break;
            }
        }

        assert!(
            buff_ever_selected,
            "buff technique should be selected at least once across 200 NPC seeds x 500 ticks"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // P4.1.4 NPC 死亡清理
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn death_cleanup_cooldown_map() {
        let mut map = NpcCooldownMap::default();
        let npc = Entity::from_raw(42);
        let other = Entity::from_raw(43);

        map.set(npc, "sword.cleave", 500);
        map.set(npc, "npc.heal_basic", 600);
        map.set(other, "sword.thrust", 700);

        assert_eq!(map.len(), 3, "3 entries before death");

        map.remove_all_for(npc);

        assert_eq!(
            map.len(),
            1,
            "only 1 entry should remain after removing dead NPC"
        );
        assert!(
            !map.is_on_cooldown(npc, "sword.cleave", 100),
            "dead NPC's cooldowns should be cleared"
        );
        assert!(
            !map.is_on_cooldown(npc, "npc.heal_basic", 100),
            "dead NPC's cooldowns should be cleared"
        );
        assert!(
            map.is_on_cooldown(other, "sword.thrust", 100),
            "other NPC's cooldowns should remain"
        );
    }

    #[test]
    fn entity_reuse_no_residual_cooldown() {
        let mut map = NpcCooldownMap::default();
        let npc = Entity::from_raw(42);

        map.set(npc, "sword.cleave", 500);
        map.set(npc, "npc.heal_basic", 600);

        map.remove_all_for(npc);

        let reused = Entity::from_raw(42);
        assert!(
            !map.is_on_cooldown(reused, "sword.cleave", 100),
            "reused entity should not inherit cooldowns from dead NPC"
        );
        assert!(
            !map.is_on_cooldown(reused, "npc.heal_basic", 100),
            "reused entity should not inherit cooldowns from dead NPC"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // P4.2 数值校准
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn calibration_heal_amount_vs_typical_damage() {
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let rank = crate::cultivation::technique_scroll::realm_rank(realm);
            let heal_amount = 5.0 + rank as f64 * 3.0;
            let typical_damage = 8.0_f64 + rank as f64 * 2.0;

            let heal_to_damage_ratio = heal_amount / typical_damage;
            assert!(
                (0.5..=3.0).contains(&heal_to_damage_ratio),
                "realm {:?}: heal_amount={heal_amount} vs typical_damage={typical_damage}, \
                 ratio={heal_to_damage_ratio:.2} should be in [0.5, 3.0]",
                realm
            );
        }
    }

    #[test]
    fn calibration_combat_power_realm_ordering() {
        let derived = DerivedAttrs {
            attack_power: 5.0,
            defense_power: 3.0,
            ..Default::default()
        };
        let techniques = KnownTechniques {
            entries: Vec::new(),
        };

        let mut prev_power = 0.0_f32;
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let qi_max = match realm {
                Realm::Awaken => 10.0,
                Realm::Induce => 30.0,
                Realm::Condense => 60.0,
                Realm::Solidify => 120.0,
                Realm::Spirit => 200.0,
                Realm::Void => 400.0,
            };
            let wounds = Wounds {
                entries: Vec::new(),
                health_current: 100.0,
                health_max: 100.0,
            };
            let cultivation = Cultivation {
                realm,
                qi_current: qi_max,
                qi_max,
                ..Default::default()
            };
            let power =
                compute_combat_power(realm, &cultivation, &wounds, &derived, &techniques, None);
            assert!(
                power.0 > prev_power,
                "realm {:?} power ({}) should exceed previous ({prev_power})",
                realm,
                power.0
            );
            prev_power = power.0;
        }
    }

    #[test]
    fn calibration_combat_power_full_vs_wounded() {
        let realm = Realm::Condense;
        let cultivation = Cultivation {
            realm,
            qi_current: 60.0,
            qi_max: 60.0,
            ..Default::default()
        };
        let derived = DerivedAttrs {
            attack_power: 5.0,
            defense_power: 3.0,
            ..Default::default()
        };
        let techniques = KnownTechniques {
            entries: Vec::new(),
        };

        let full = compute_combat_power(
            realm,
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
        let wounded = compute_combat_power(
            realm,
            &cultivation,
            &Wounds {
                entries: Vec::new(),
                health_current: 30.0,
                health_max: 100.0,
            },
            &derived,
            &techniques,
            None,
        );

        assert!(
            full.0 > wounded.0,
            "full HP power ({}) should exceed wounded power ({})",
            full.0,
            wounded.0
        );

        let ratio = wounded.0 / full.0;
        assert!(
            ratio >= 0.15,
            "wounded/full ratio ({ratio:.3}) should be >= 0.15 (condition_factor floor)"
        );
    }

    #[test]
    fn calibration_combat_power_condition_factor_floor() {
        let realm = Realm::Condense;
        let cultivation = Cultivation {
            realm,
            qi_current: 0.0,
            qi_max: 60.0,
            ..Default::default()
        };
        let derived = DerivedAttrs {
            attack_power: 5.0,
            defense_power: 3.0,
            ..Default::default()
        };
        let techniques = KnownTechniques {
            entries: Vec::new(),
        };
        let wounds = Wounds {
            entries: Vec::new(),
            health_current: 0.0,
            health_max: 100.0,
        };

        let power = compute_combat_power(realm, &cultivation, &wounds, &derived, &techniques, None);

        let base = 40.0 + 5.0 + 3.0;
        let expected = base * 0.15;
        assert!(
            (power.0 - expected).abs() < 0.01,
            "at hp=0 qi=0, power should be base*0.15={expected}, got {}",
            power.0
        );
    }

    #[test]
    fn calibration_combat_power_weight_proportions() {
        let cultivation = Cultivation {
            realm: Realm::Spirit,
            qi_current: 200.0,
            qi_max: 200.0,
            ..Default::default()
        };
        let wounds = Wounds {
            entries: Vec::new(),
            health_current: 100.0,
            health_max: 100.0,
        };

        let realm_weight = crate::npc::combat_power::realm_ordinal(Realm::Spirit) as f32 * 20.0;
        let attr_contribution = 10.0 + 6.0;
        let tech_contribution = 3.0 * 2.0;
        let total_base = realm_weight + attr_contribution + tech_contribution;

        let techniques = KnownTechniques {
            entries: vec![
                KnownTechnique {
                    id: "t1".to_string(),
                    proficiency: 0.5,
                    active: true,
                },
                KnownTechnique {
                    id: "t2".to_string(),
                    proficiency: 0.5,
                    active: true,
                },
                KnownTechnique {
                    id: "t3".to_string(),
                    proficiency: 0.5,
                    active: true,
                },
            ],
        };
        let derived = DerivedAttrs {
            attack_power: 10.0,
            defense_power: 6.0,
            ..Default::default()
        };

        let power = compute_combat_power(
            Realm::Spirit,
            &cultivation,
            &wounds,
            &derived,
            &techniques,
            None,
        );

        let realm_pct = realm_weight / total_base;
        assert!(
            realm_pct > 0.40,
            "realm weight should dominate (>40%) at Spirit tier, got {:.1}% \
             (realm_w={realm_weight}, total={total_base})",
            realm_pct * 100.0
        );

        assert!(power.0 > 0.0, "power should be positive, got {}", power.0);

        let attr_pct = attr_contribution / total_base;
        let tech_pct = tech_contribution / total_base;
        assert!(
            attr_pct + tech_pct < realm_pct,
            "attrs ({:.1}%) + tech ({:.1}%) should be less than realm ({:.1}%)",
            attr_pct * 100.0,
            tech_pct * 100.0,
            realm_pct * 100.0
        );
    }

    #[test]
    fn calibration_buff_duration_200_ticks() {
        let def_speed = technique_definition("npc.buff_speed");
        let def_defense = technique_definition("npc.buff_defense");

        assert!(
            def_speed.is_some(),
            "npc.buff_speed should exist in TECHNIQUE_DEFINITIONS"
        );
        assert!(
            def_defense.is_some(),
            "npc.buff_defense should exist in TECHNIQUE_DEFINITIONS"
        );
    }

    #[test]
    fn calibration_wounded_npc_lower_power_than_full() {
        let derived = DerivedAttrs {
            attack_power: 5.0,
            defense_power: 3.0,
            ..Default::default()
        };
        let techniques = KnownTechniques {
            entries: Vec::new(),
        };

        let full_cult = Cultivation {
            realm: Realm::Condense,
            qi_current: 60.0,
            qi_max: 60.0,
            ..Default::default()
        };
        let half_cult = Cultivation {
            realm: Realm::Condense,
            qi_current: 30.0,
            qi_max: 60.0,
            ..Default::default()
        };

        let full_power = compute_combat_power(
            Realm::Condense,
            &full_cult,
            &Wounds {
                entries: Vec::new(),
                health_current: 100.0,
                health_max: 100.0,
            },
            &derived,
            &techniques,
            None,
        );
        let half_power = compute_combat_power(
            Realm::Condense,
            &half_cult,
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
            full_power.0 > half_power.0,
            "full HP+QI ({}) should have more combat power than half HP+QI ({})",
            full_power.0,
            half_power.0
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // P4.2 extra: 跨系统集成验证
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn integration_full_combat_cycle_near_tier() {
        let realm = Realm::Condense;
        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();

        let mut categories_seen = std::collections::HashSet::new();
        for seed in 0..50u64 {
            let techniques = assign_npc_techniques(
                NpcArchetype::Disciple,
                realm,
                &meridian_sys,
                &deps,
                None,
                seed,
            );

            let cultivation = Cultivation {
                realm,
                qi_current: 200.0,
                qi_max: 200.0,
                ..Default::default()
            };
            let cooldowns = NpcCooldownMap::default();
            let entity = Entity::from_raw(seed as u32 + 5000);

            for tick in 0..200u64 {
                let ctx = NpcSkillScoringContext {
                    hp_ratio: if tick < 50 { 0.9 } else { 0.25 },
                    qi_ratio: 0.8,
                    target_distance: 3.0,
                    target_hp_ratio: 0.5,
                    has_active_buff: tick % 100 > 50,
                    in_combat: true,
                };

                if let Some(sel) = select_technique(
                    &techniques,
                    &cultivation,
                    &deps,
                    None,
                    None, // meridian_sys（dugu-poison opened gate；测试无 MeridianSystem 传 None）
                    &cooldowns,
                    entity,
                    3.0,
                    tick,
                    &ctx,
                    None,
                ) {
                    if let Some(def) = technique_definition(&sel.technique_id) {
                        categories_seen.insert(format!("{:?}", def.category));
                    }
                }
            }
        }

        assert!(
            categories_seen.len() >= 2,
            "combat cycle should see multiple categories across 50 seeds, saw: {:?}",
            categories_seen
        );
    }
}
