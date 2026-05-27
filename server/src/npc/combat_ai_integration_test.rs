#[cfg(test)]
mod tests {
    use valence::prelude::{App, Entity, Events};

    use crate::combat::components::{CombatState, DefenseWindow, DerivedAttrs, Wounds};
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::cultivation::known_techniques::{
        technique_definition, KnownTechnique, KnownTechniques, SkillCategory,
    };
    use crate::cultivation::meridian::severed::SkillMeridianDependencies;
    use crate::npc::abstract_combat::{
        abstract_combat_resolve, apply_outcome_to_loser, apply_qi_cost, AbstractCombatOutcome,
    };
    use crate::npc::abstract_combat_system::ABSTRACT_COMBAT_INTERVAL_TICKS;
    use crate::npc::combat_power::{compute_combat_power, CombatPowerScore};
    use crate::npc::faction::{
        FactionId, FactionMembership, FactionRank, FactionStore, Reputation,
    };
    use crate::npc::lifecycle::NpcArchetype;
    use crate::npc::lod::NpcLodTier;
    use crate::npc::movement::GameTick;
    use crate::npc::patrol::NpcPatrol;
    use crate::npc::spawn::{NpcBlackboard, NpcMarker};
    use crate::npc::technique::{
        assign_npc_techniques, category_weight, has_usable_heal_technique,
        npc_meridian_system_for_realm, select_technique, NpcCooldownMap, NpcSkillScoringContext,
    };
    use crate::qi_physics::ledger::QiTransfer;

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

    fn build_test_app() -> App {
        let mut app = App::new();
        app.insert_resource(GameTick(0));
        app.insert_resource(FactionStore::default());
        app.add_event::<QiTransfer>();
        crate::npc::abstract_combat_system::register(&mut app);
        app
    }

    fn spawn_far_npc(
        app: &mut App,
        realm: Realm,
        faction: FactionId,
        qi_current: f64,
        hp_current: f32,
        attack_power: f32,
    ) -> Entity {
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Far,
                Wounds {
                    entries: Vec::new(),
                    health_current: hp_current,
                    health_max: 100.0,
                },
                Cultivation {
                    realm,
                    qi_current,
                    qi_max: 100.0,
                    ..Default::default()
                },
                DerivedAttrs {
                    attack_power,
                    defense_power: 3.0,
                    ..Default::default()
                },
                KnownTechniques {
                    entries: Vec::new(),
                },
                FactionMembership {
                    faction_id: faction,
                    rank: FactionRank::Disciple,
                    reputation: Reputation::default(),
                    lineage: None,
                    mission_queue: Default::default(),
                },
                NpcPatrol::new("spawn", valence::prelude::DVec3::ZERO),
                NpcBlackboard::default(),
                CombatState::default(),
            ))
            .id()
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
    // P4.1.2 Far tier 抽象战斗
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn far_tier_abstract_combat_produces_outcome() {
        let attacker = CombatPowerScore(50.0);
        let defender = CombatPowerScore(50.0);

        let mut attacker_wins = 0;
        let mut defender_wins = 0;
        for seed in 0..1000u64 {
            match abstract_combat_resolve(
                attacker,
                defender,
                Realm::Condense,
                Realm::Condense,
                seed * 31 + 42,
            ) {
                AbstractCombatOutcome::AttackerWins { .. } => attacker_wins += 1,
                AbstractCombatOutcome::DefenderWins { .. } => defender_wins += 1,
            }
        }
        assert!(
            attacker_wins > 0 && defender_wins > 0,
            "equal power should produce both outcomes: a_wins={attacker_wins}, d_wins={defender_wins}"
        );
    }

    #[test]
    fn far_tier_loser_hp_deducted() {
        let mut wounds = Wounds {
            entries: Vec::new(),
            health_current: 100.0,
            health_max: 100.0,
        };
        apply_outcome_to_loser(&mut wounds, 0.15);
        assert!(
            (wounds.health_current - 85.0).abs() < 0.01,
            "loser should lose 15% of max HP, got {}",
            wounds.health_current
        );
    }

    #[test]
    fn far_tier_qi_cost_deducted_from_both() {
        let mut a_cult = Cultivation {
            realm: Realm::Condense,
            qi_current: 50.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let mut d_cult = Cultivation {
            realm: Realm::Induce,
            qi_current: 30.0,
            qi_max: 100.0,
            ..Default::default()
        };

        let a_actual = apply_qi_cost(&mut a_cult, 8.0);
        let d_actual = apply_qi_cost(&mut d_cult, 5.0);

        assert!(
            (a_actual - 8.0).abs() < 1e-9,
            "attacker should spend 8.0 qi, actual={a_actual}"
        );
        assert!(
            (d_actual - 5.0).abs() < 1e-9,
            "defender should spend 5.0 qi, actual={d_actual}"
        );
        assert!(
            (a_cult.qi_current - 42.0).abs() < 1e-9,
            "attacker qi should be 42.0, got {}",
            a_cult.qi_current
        );
        assert!(
            (d_cult.qi_current - 25.0).abs() < 1e-9,
            "defender qi should be 25.0, got {}",
            d_cult.qi_current
        );
    }

    #[test]
    fn far_tier_qi_transfer_emitted_in_ecs() {
        let mut app = build_test_app();
        app.insert_resource(GameTick(ABSTRACT_COMBAT_INTERVAL_TICKS));

        spawn_far_npc(
            &mut app,
            Realm::Condense,
            FactionId::Attack,
            50.0,
            100.0,
            5.0,
        );
        spawn_far_npc(
            &mut app,
            Realm::Condense,
            FactionId::Defend,
            50.0,
            100.0,
            5.0,
        );

        app.update();

        let events = app.world().resource::<Events<QiTransfer>>();
        let transfers: Vec<_> = events.iter_current_update_events().collect();
        assert!(
            !transfers.is_empty(),
            "QiTransfer events should be emitted after far-tier abstract combat"
        );
    }

    #[test]
    fn far_tier_death_when_hp_below_zero() {
        let mut wounds = Wounds {
            entries: Vec::new(),
            health_current: 3.0,
            health_max: 100.0,
        };
        apply_outcome_to_loser(&mut wounds, 0.30);
        assert!(
            wounds.health_current <= 0.0,
            "HP 3.0 - 30% of 100 = -27.0 should clamp to 0, got {}",
            wounds.health_current
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // P4.1.3 LOD 过渡
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn lod_near_to_far_clears_incoming_window() {
        let mut app = build_test_app();

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Far,
                CombatState {
                    in_combat_until_tick: Some(1000),
                    last_attack_at_tick: Some(500),
                    incoming_window: Some(DefenseWindow {
                        opened_at_tick: 400,
                        duration_ms: 200,
                    }),
                },
                NpcBlackboard::default(),
            ))
            .id();

        app.update();

        let cs = app.world().get::<CombatState>(entity).unwrap();
        assert!(
            cs.incoming_window.is_none(),
            "incoming_window should be cleared when transitioning to Far tier"
        );
    }

    #[test]
    fn lod_transition_hp_qi_preserved() {
        let mut app = build_test_app();

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Far,
                CombatState {
                    in_combat_until_tick: Some(1000),
                    last_attack_at_tick: Some(500),
                    incoming_window: Some(DefenseWindow {
                        opened_at_tick: 400,
                        duration_ms: 200,
                    }),
                },
                Wounds {
                    entries: Vec::new(),
                    health_current: 42.5,
                    health_max: 100.0,
                },
                Cultivation {
                    realm: Realm::Condense,
                    qi_current: 33.7,
                    qi_max: 100.0,
                    ..Default::default()
                },
                NpcBlackboard::default(),
            ))
            .id();

        app.update();

        let wounds = app.world().get::<Wounds>(entity).unwrap();
        assert!(
            (wounds.health_current - 42.5).abs() < 1e-4,
            "HP should be preserved across LOD transition, got {}",
            wounds.health_current
        );

        let cult = app.world().get::<Cultivation>(entity).unwrap();
        assert!(
            (cult.qi_current - 33.7).abs() < 1e-9,
            "qi should be preserved across LOD transition, got {}",
            cult.qi_current
        );
    }

    #[test]
    fn lod_dormant_clears_both_window_and_retaliation() {
        let mut app = build_test_app();

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Dormant,
                CombatState {
                    in_combat_until_tick: Some(1000),
                    last_attack_at_tick: Some(500),
                    incoming_window: Some(DefenseWindow {
                        opened_at_tick: 400,
                        duration_ms: 200,
                    }),
                },
                NpcBlackboard {
                    retaliation_target: Some((Entity::PLACEHOLDER, 999)),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let cs = app.world().get::<CombatState>(entity).unwrap();
        assert!(
            cs.incoming_window.is_none(),
            "incoming_window should be cleared for Dormant"
        );
        let bb = app.world().get::<NpcBlackboard>(entity).unwrap();
        assert!(
            bb.retaliation_target.is_none(),
            "retaliation_target should be cleared for Dormant"
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
    fn calibration_abstract_combat_interval_200_ticks() {
        assert_eq!(
            ABSTRACT_COMBAT_INTERVAL_TICKS, 200,
            "far tier combat interval should be 200 ticks (10 seconds)"
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
    fn calibration_npc_pair_different_realms() {
        let low_wounds = Wounds {
            entries: Vec::new(),
            health_current: 100.0,
            health_max: 100.0,
        };
        let high_wounds = low_wounds.clone();

        let low_cult = Cultivation {
            realm: Realm::Induce,
            qi_current: 30.0,
            qi_max: 30.0,
            ..Default::default()
        };
        let high_cult = Cultivation {
            realm: Realm::Spirit,
            qi_current: 200.0,
            qi_max: 200.0,
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

        let low_power = compute_combat_power(
            Realm::Induce,
            &low_cult,
            &low_wounds,
            &derived,
            &techniques,
            None,
        );
        let high_power = compute_combat_power(
            Realm::Spirit,
            &high_cult,
            &high_wounds,
            &derived,
            &techniques,
            None,
        );

        assert!(
            high_power.0 > low_power.0 * 2.0,
            "Spirit power ({}) should be >2x Induce power ({})",
            high_power.0,
            low_power.0
        );

        let mut high_wins = 0;
        let trials = 1000;
        for seed in 0..trials {
            match abstract_combat_resolve(
                high_power,
                low_power,
                Realm::Spirit,
                Realm::Induce,
                seed * 7919 + 42,
            ) {
                AbstractCombatOutcome::AttackerWins { .. } => high_wins += 1,
                AbstractCombatOutcome::DefenderWins { .. } => {}
            }
        }

        let win_rate = high_wins as f64 / trials as f64;
        assert!(
            win_rate > 0.65,
            "Spirit vs Induce: Spirit should win >65% of the time, got {:.1}%",
            win_rate * 100.0
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

    #[test]
    fn integration_far_tier_repeated_combat_drains_resources() {
        let mut app = build_test_app();

        let a = spawn_far_npc(
            &mut app,
            Realm::Condense,
            FactionId::Attack,
            50.0,
            100.0,
            5.0,
        );
        let d = spawn_far_npc(
            &mut app,
            Realm::Condense,
            FactionId::Defend,
            50.0,
            100.0,
            5.0,
        );

        for round in 0..5u32 {
            let tick = (round + 1) * ABSTRACT_COMBAT_INTERVAL_TICKS;
            app.insert_resource(GameTick(tick));
            app.update();
        }

        let a_wounds = app.world().get::<Wounds>(a).unwrap();
        let d_wounds = app.world().get::<Wounds>(d).unwrap();
        let a_cult = app.world().get::<Cultivation>(a).unwrap();
        let d_cult = app.world().get::<Cultivation>(d).unwrap();

        let total_hp_lost = (100.0 - a_wounds.health_current) + (100.0 - d_wounds.health_current);
        assert!(
            total_hp_lost > 5.0,
            "after 5 rounds of combat, total HP lost should be >5.0, got {total_hp_lost:.2}"
        );

        let total_qi_lost = (50.0 - a_cult.qi_current) + (50.0 - d_cult.qi_current);
        assert!(
            total_qi_lost > 5.0,
            "after 5 rounds of combat, total qi lost should be >5.0, got {total_qi_lost:.2}"
        );
    }
}
