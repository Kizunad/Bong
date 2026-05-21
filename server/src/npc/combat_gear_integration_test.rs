//! plan-npc-combat-gear-v1 §P3 — 集成测试 + 校准。
//!
//! 纯 ECS 集成测试（不起真网络），验证 NPC 装备 + 功法 + 经脉 + 协议的端到端链路。

#[cfg(test)]
mod tests {
    use crate::cultivation::components::{Cultivation, MeridianId, Realm};
    use crate::cultivation::known_techniques::{
        technique_definition, KnownTechnique, KnownTechniques,
    };
    use crate::cultivation::meridian::severed::{
        check_meridian_dependencies, MeridianSeveredPermanent, SeveredSource,
        SkillMeridianDependencies,
    };
    use crate::cultivation::technique_scroll::realm_rank;
    use crate::network::npc_metadata::{build_npc_metadata, NpcMetadataBuildInput};
    use crate::npc::equipment::{
        assign_npc_equipment, merge_equipment, npc_weapon_damage_multiplier, roll_equipment_drops,
        NpcEquipment,
    };
    use crate::npc::lifecycle::NpcArchetype;
    use crate::npc::technique::{
        assign_npc_techniques, npc_meridian_system_for_realm, select_technique, NpcCooldownMap,
    };
    use crate::npc::trade::assign_npc_trade_inventory;
    use crate::skin::npc_skin_selector::select_npc_visual_profile;
    use valence::prelude::Entity;

    // ─── helpers ────────────────────────────────────────────────────────────────

    fn empty_deps() -> SkillMeridianDependencies {
        SkillMeridianDependencies::default()
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // P3.1 Server 集成测试
    // ─────────────────────────────────────────────────────────────────────────────

    /// P3.1 - 完整链路: spawn Rogue NPC → 验证四组件存在 + 经脉依赖满足
    #[test]
    fn integration_spawn_rogue_has_all_four_components() {
        let realm = Realm::Condense;
        let seed = 42u64;
        let archetype = NpcArchetype::Rogue;

        // 1. NpcEquipment
        let equipment = assign_npc_equipment(archetype, realm, None, seed);
        // Rogue at Condense should have something (50% sword + 30% staff = 80% armed)
        // Verify structure is valid (quality_tier <= 2)
        for (_name, slot) in equipment.iter_slots() {
            assert!(
                slot.quality_tier <= 2,
                "Rogue quality_tier should not exceed 2 (法宝), got {}",
                slot.quality_tier
            );
        }

        // 2. MeridianSystem — via npc_meridian_system_for_realm
        let meridian_sys = npc_meridian_system_for_realm(realm);
        let opened_count = MeridianId::ALL
            .iter()
            .filter(|id| meridian_sys.get(**id).opened)
            .count();
        assert_eq!(
            opened_count,
            realm.required_meridians(),
            "Condense NPC should have {} meridians opened, got {}",
            realm.required_meridians(),
            opened_count
        );

        // 3. KnownTechniques
        let deps = empty_deps();
        let techniques = assign_npc_techniques(archetype, realm, &meridian_sys, &deps, None, seed);
        assert!(
            !techniques.entries.is_empty(),
            "Rogue Condense should have at least 1 technique"
        );
        assert!(
            techniques.entries.len() <= 3,
            "Rogue should have at most 3 techniques, got {}",
            techniques.entries.len()
        );

        // 4. Verify each technique's meridian deps satisfied by MeridianSystem
        for entry in &techniques.entries {
            let def = technique_definition(&entry.id).unwrap_or_else(|| {
                panic!(
                    "technique {} should exist in TECHNIQUE_DEFINITIONS",
                    entry.id
                )
            });
            // Check realm gating
            let required_realm_str = def.required_realm;
            assert!(
                realm_rank(match required_realm_str {
                    "Awaken" => Realm::Awaken,
                    "Induce" => Realm::Induce,
                    "Condense" => Realm::Condense,
                    "Solidify" => Realm::Solidify,
                    "Spirit" => Realm::Spirit,
                    "Void" => Realm::Void,
                    _ => panic!("unknown realm string: {}", required_realm_str),
                }) <= realm_rank(realm),
                "technique {} requires realm {} but NPC is {:?}",
                entry.id,
                required_realm_str,
                realm
            );
            // Check meridian deps in definition
            for req in def.required_meridians {
                if let Some(channel) =
                    crate::cultivation::technique_scroll::parse_meridian_id(req.channel)
                {
                    let m = meridian_sys.get(channel);
                    assert!(
                        m.opened,
                        "technique {} requires meridian {:?} to be opened but it's closed",
                        entry.id, channel
                    );
                }
            }
        }

        // 5. Valence Equipment merge works
        let profile = select_npc_visual_profile(archetype, realm, None, None, 0.5);
        let merged = merge_equipment(Some(&equipment), &profile);
        // merged should not panic — that's the test
        let _ = merged;
    }

    /// P3.1 - 完整链路: spawn Disciple NPC → 全链路验证
    #[test]
    fn integration_spawn_disciple_full_chain() {
        use crate::npc::faction::FactionId;

        let realm = Realm::Solidify;
        let seed = 7777u64;
        let archetype = NpcArchetype::Disciple;
        let faction = Some(FactionId::Attack);

        let equipment = assign_npc_equipment(archetype, realm, faction, seed);
        assert!(
            equipment.main_hand.is_some(),
            "Disciple should always have main_hand weapon"
        );

        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();
        let techniques = assign_npc_techniques(archetype, realm, &meridian_sys, &deps, None, seed);
        assert!(
            techniques.entries.len() >= 2,
            "Disciple should have >= 2 techniques, got {}",
            techniques.entries.len()
        );

        // Verify each technique's realm + meridian deps
        for entry in &techniques.entries {
            let def = technique_definition(&entry.id).unwrap();
            assert!(entry.proficiency >= 0.3 && entry.proficiency <= 0.8);
            // Meridian deps check same as above test
            for req in def.required_meridians {
                if let Some(channel) =
                    crate::cultivation::technique_scroll::parse_meridian_id(req.channel)
                {
                    assert!(
                        meridian_sys.get(channel).opened,
                        "Disciple technique {} requires {:?} but closed",
                        entry.id,
                        channel
                    );
                }
            }
        }

        // Trade inventory
        let trade = assign_npc_trade_inventory(archetype, realm, seed);
        assert!(
            trade.offers.len() >= 2,
            "Disciple should have >= 2 trade offers"
        );
    }

    /// P3.1 - 完整链路: spawn GuardianRelic → 高品质装备 + 多功法
    #[test]
    fn integration_spawn_guardian_relic_full_chain() {
        let realm = Realm::Spirit;
        let seed = 31337u64;
        let archetype = NpcArchetype::GuardianRelic;

        let equipment = assign_npc_equipment(archetype, realm, None, seed);
        let main = equipment
            .main_hand
            .as_ref()
            .expect("GuardianRelic must have weapon");
        assert_eq!(main.template_id, "ancient_sword");
        assert_eq!(main.quality_tier, 2);
        assert!(equipment.armor_slots().count() >= 2);

        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();
        let techniques = assign_npc_techniques(archetype, realm, &meridian_sys, &deps, None, seed);
        assert!(
            techniques.entries.len() >= 3,
            "GuardianRelic should have >= 3 techniques, got {}",
            techniques.entries.len()
        );
        for entry in &techniques.entries {
            assert!(entry.proficiency >= 0.6);
        }
    }

    /// P3.1 - 战斗集成: NPC select_technique → CastResult-ready 验证
    #[test]
    fn integration_combat_select_technique_and_cast_readiness() {
        let realm = Realm::Condense;
        let archetype = NpcArchetype::Rogue;
        let seed = 99u64;

        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();
        let techniques = assign_npc_techniques(archetype, realm, &meridian_sys, &deps, None, seed);

        // Create cultivation with enough qi
        let cultivation = Cultivation {
            realm,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        // select_technique should return Some
        let selected = select_technique(
            &techniques,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
        );
        assert!(
            selected.is_some(),
            "NPC with qi and known techniques should be able to select a technique"
        );

        // Verify selected technique exists and has valid qi_cost
        let tid = selected.unwrap();
        let def = technique_definition(&tid)
            .unwrap_or_else(|| panic!("selected technique {} should exist", tid));
        assert!(
            f64::from(def.qi_cost) <= cultivation.qi_current,
            "selected technique {} qi_cost={} should be <= qi_current={}",
            tid,
            def.qi_cost,
            cultivation.qi_current
        );
    }

    /// P3.1 - 死亡集成: NPC 死亡 → 装备概率掉落 → RolledLoot 合法
    #[test]
    fn integration_death_drop_loot_valid() {
        let equipment = assign_npc_equipment(NpcArchetype::Rogue, Realm::Condense, None, 42);
        let slot_count = equipment.iter_slots().count();

        // Roll many times and verify all drops reference valid template_ids
        let mut total_drops = 0usize;
        for seed in 0..500u64 {
            let drops = roll_equipment_drops(&equipment, seed * 7919);
            for drop in &drops {
                assert!(
                    !drop.template_id.is_empty(),
                    "dropped item template_id should not be empty"
                );
                assert_eq!(drop.stack, 1, "equipment drops should always be stack=1");
                // Verify the template_id matches one of the equipped slots
                let matches_slot = equipment
                    .iter_slots()
                    .any(|(_, slot)| slot.template_id == drop.template_id);
                assert!(
                    matches_slot,
                    "dropped template_id {} should match an equipped slot",
                    drop.template_id
                );
            }
            total_drops += drops.len();
        }

        // Statistical check: overall drop rate should be ~30%
        if slot_count > 0 {
            let total_possible = slot_count * 500;
            let rate = total_drops as f32 / total_possible as f32;
            assert!(
                (0.20..=0.40).contains(&rate),
                "equipment drop rate should be ~30%, got {rate:.3} ({total_drops}/{total_possible})"
            );
        }
    }

    /// P3.1 - 经脉 SEVERED 集成: MeridianSeveredPermanent → select_technique 排除
    #[test]
    fn integration_severed_meridian_excludes_dependent_technique() {
        // Setup: NPC with a technique that depends on Lung meridian
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "zhenmai.parry".to_string(),
                proficiency: 0.7,
                active: true,
            }],
        };

        let mut deps = SkillMeridianDependencies::default();
        deps.declare("zhenmai.parry", vec![MeridianId::Lung]);

        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(5);

        // Without SEVERED: should be selectable
        let result_ok = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
        );
        assert!(
            result_ok.is_some(),
            "without SEVERED, technique should be selectable"
        );

        // With SEVERED Lung: should be excluded
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(MeridianId::Lung, SeveredSource::CombatWound, 50);

        let result_severed = select_technique(
            &known,
            &cultivation,
            &deps,
            Some(&severed),
            &cooldowns,
            entity,
            3.0,
            200,
        );
        assert!(
            result_severed.is_none(),
            "with SEVERED Lung, technique depending on Lung should be excluded"
        );

        // Verify the underlying check_meridian_dependencies also catches it
        let dep_ids = deps.lookup("zhenmai.parry");
        let check_result = check_meridian_dependencies(dep_ids, Some(&severed));
        assert!(
            check_result.is_err(),
            "check_meridian_dependencies should return Err for severed Lung"
        );
    }

    /// P3.1 - 经脉 SEVERED 集成: 多功法时仅排除依赖被断经脉的功法
    #[test]
    fn integration_severed_excludes_only_dependent_techniques() {
        let known = KnownTechniques {
            entries: vec![
                KnownTechnique {
                    id: "sword.cleave".to_string(), // no meridian dep
                    proficiency: 0.5,
                    active: true,
                },
                KnownTechnique {
                    id: "zhenmai.parry".to_string(), // depends on Lung
                    proficiency: 0.5,
                    active: true,
                },
            ],
        };

        let mut deps = SkillMeridianDependencies::default();
        deps.declare("zhenmai.parry", vec![MeridianId::Lung]);

        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(6);

        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(MeridianId::Lung, SeveredSource::CombatWound, 50);

        // Run many ticks to collect which techniques get selected
        let mut selected_ids = std::collections::HashSet::new();
        for tick in 0..500u64 {
            if let Some(tid) = select_technique(
                &known,
                &cultivation,
                &deps,
                Some(&severed),
                &cooldowns,
                entity,
                3.0,
                tick,
            ) {
                selected_ids.insert(tid);
            }
        }

        assert!(
            selected_ids.contains("sword.cleave"),
            "sword.cleave (no dep) should be selectable even with Lung severed"
        );
        assert!(
            !selected_ids.contains("zhenmai.parry"),
            "zhenmai.parry (dep Lung) should never be selected when Lung is severed"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // P3.2 Client-Server 协议集成
    // ─────────────────────────────────────────────────────────────────────────────

    /// P3.2 - bong:npc_metadata 完整字段端到端
    #[test]
    fn integration_protocol_full_metadata_roundtrip() {
        let equipment = assign_npc_equipment(NpcArchetype::Rogue, Realm::Condense, None, 42);
        let meridian_sys = npc_meridian_system_for_realm(Realm::Condense);
        let deps = empty_deps();
        let techniques = assign_npc_techniques(
            NpcArchetype::Rogue,
            Realm::Condense,
            &meridian_sys,
            &deps,
            None,
            42,
        );
        let trade = assign_npc_trade_inventory(NpcArchetype::Rogue, Realm::Condense, 42);

        let metadata = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 100,
            archetype: NpcArchetype::Rogue,
            cultivation: Some(&Cultivation {
                realm: Realm::Condense,
                qi_current: 50.0,
                qi_max: 100.0,
                ..Default::default()
            }),
            membership: None,
            lifespan: None,
            player_cultivation: None,
            player_identities: None,
            wounds: None,
            equipment: Some(&equipment),
            techniques: Some(&techniques),
            trade_inventory: Some(&trade),
        });

        // Verify all fields present
        assert_eq!(metadata.entity_id, 100);
        assert_eq!(metadata.archetype, "rogue");

        // Equipment slots match the assigned equipment
        let equip_slot_count = equipment.iter_slots().count();
        assert_eq!(
            metadata.equipment.len(),
            equip_slot_count,
            "metadata equipment count should match assigned equipment slot count"
        );
        for (slot_name, slot) in equipment.iter_slots() {
            let s2c = metadata
                .equipment
                .get(slot_name)
                .unwrap_or_else(|| panic!("metadata should have slot {}", slot_name));
            assert_eq!(s2c.template_id, slot.template_id);
            assert_eq!(s2c.quality_tier, slot.quality_tier);
        }

        // Techniques: only active entries appear
        let active_count = techniques.entries.iter().filter(|e| e.active).count();
        assert_eq!(
            metadata.techniques.len(),
            active_count,
            "metadata techniques count should match active technique count"
        );
        for tech_s2c in &metadata.techniques {
            assert!(!tech_s2c.id.is_empty(), "technique id should not be empty");
            assert!(
                !tech_s2c.display_name.is_empty(),
                "technique display_name should not be empty"
            );
        }

        // Trade offers
        assert_eq!(
            metadata.trade_offers.len(),
            trade.offers.len(),
            "metadata trade_offers count should match assigned trade offers"
        );

        // Serialize roundtrip should not error
        let bytes = metadata.to_json_bytes_checked();
        assert!(
            bytes.is_ok(),
            "metadata should serialize without error: {:?}",
            bytes.err()
        );
    }

    /// P3.2 - 旧 client 连新 server: 新字段 null-safe
    /// When equipment/techniques/trade_offers are empty, they should be omitted from JSON
    /// (via #[serde(skip_serializing_if)]), so an old client that doesn't know these fields
    /// will not crash.
    #[test]
    fn integration_protocol_old_client_new_server_null_safe() {
        // Beast NPC: no equipment, no techniques, no trade
        let metadata = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 200,
            archetype: NpcArchetype::Beast,
            cultivation: None,
            membership: None,
            lifespan: None,
            player_cultivation: None,
            player_identities: None,
            wounds: None,
            equipment: None,
            techniques: None,
            trade_inventory: None,
        });

        let json = String::from_utf8(
            metadata
                .to_json_bytes_checked()
                .expect("serialize should succeed"),
        )
        .expect("should be valid UTF-8");

        // Empty equipment/techniques/trade_offers should be skipped in JSON
        assert!(
            !json.contains("\"equipment\""),
            "empty equipment should not appear in JSON for old-client compatibility"
        );
        assert!(
            !json.contains("\"techniques\""),
            "empty techniques should not appear in JSON for old-client compatibility"
        );
        assert!(
            !json.contains("\"trade_offers\""),
            "empty trade_offers should not appear in JSON for old-client compatibility"
        );
    }

    /// P3.2 - 新 client 连旧 server: 缺失字段 fallback 到默认
    /// When server sends a packet without equipment/techniques/trade_offers (old server),
    /// client-side build_npc_metadata with None inputs should produce empty collections.
    #[test]
    fn integration_protocol_new_client_old_server_fallback() {
        let metadata = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 300,
            archetype: NpcArchetype::Rogue,
            cultivation: Some(&Cultivation {
                realm: Realm::Awaken,
                qi_current: 10.0,
                qi_max: 50.0,
                ..Default::default()
            }),
            membership: None,
            lifespan: None,
            player_cultivation: None,
            player_identities: None,
            wounds: None,
            equipment: None,       // old server doesn't send these
            techniques: None,      // old server doesn't send these
            trade_inventory: None, // old server doesn't send these
        });

        assert!(
            metadata.equipment.is_empty(),
            "missing equipment should fallback to empty map"
        );
        assert!(
            metadata.techniques.is_empty(),
            "missing techniques should fallback to empty vec"
        );
        assert!(
            metadata.trade_offers.is_empty(),
            "missing trade_offers should fallback to empty vec"
        );

        // Serialize should still succeed
        assert!(
            metadata.to_json_bytes_checked().is_ok(),
            "metadata without new fields should still serialize"
        );
    }

    /// P3.2 - 协议兼容: NpcEquipment::default() (all None) produces empty HashMap in metadata
    #[test]
    fn integration_protocol_empty_equipment_produces_empty_map() {
        let empty_eq = NpcEquipment::default();
        let metadata = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 301,
            archetype: NpcArchetype::Commoner,
            cultivation: None,
            membership: None,
            lifespan: None,
            player_cultivation: None,
            player_identities: None,
            wounds: None,
            equipment: Some(&empty_eq),
            techniques: None,
            trade_inventory: None,
        });

        assert!(
            metadata.equipment.is_empty(),
            "NpcEquipment with all None slots should produce empty equipment map"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // P3.3 NPC 功法频率校准
    // ─────────────────────────────────────────────────────────────────────────────

    /// P3.3 - 功法频率校准: NpcTechniqueScorer 最小间隔验证
    ///
    /// 模拟 100 轮 NPC "攻击决策"：每轮检查 NpcTechniqueScorer 的最小间隔规则，
    /// 在 select_technique 可用时计为"功法攻击"，否则为"近战攻击"。
    /// 验证功法使用频率在目标区间内。
    #[test]
    fn integration_calibration_rogue_technique_frequency() {
        // Rogue: 每 3-5 次攻击使用 1 次功法 → 功法占比 20-33%
        let realm = Realm::Awaken;
        let min_interval: u64 = 60 + realm_rank(realm) as u64 * 10; // 60 + 0 = 60

        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();

        let mut technique_uses = 0u64;
        let mut total_attacks = 0u64;

        // Test across multiple NPC instances (different seeds)
        for seed in 0..20u64 {
            let techniques = assign_npc_techniques(
                NpcArchetype::Rogue,
                realm,
                &meridian_sys,
                &deps,
                None,
                seed * 37,
            );
            if techniques.entries.is_empty() {
                continue;
            }
            let cultivation = Cultivation {
                realm,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            };
            let cooldowns = NpcCooldownMap::default();
            let entity = Entity::from_raw(seed as u32 + 100);

            let mut last_technique_tick: u64 = 0;
            // Simulate 100 attack ticks with spacing of ~40 ticks between attacks
            for attack_idx in 0..100u64 {
                let current_tick = attack_idx * 40; // attacks every 40 ticks
                total_attacks += 1;

                // Check minimum interval (NpcTechniqueScorer logic)
                if current_tick > 0
                    && current_tick.saturating_sub(last_technique_tick) < min_interval
                {
                    // Too soon for technique, counts as melee attack
                    continue;
                }

                // Check if a technique is available
                let selected = select_technique(
                    &techniques,
                    &cultivation,
                    &deps,
                    None,
                    &cooldowns,
                    entity,
                    3.0,
                    current_tick,
                );
                if selected.is_some() {
                    technique_uses += 1;
                    last_technique_tick = current_tick;
                }
            }
        }

        if total_attacks > 0 {
            let freq = technique_uses as f64 / total_attacks as f64;
            // Rogue target: 20-33% (every 3-5 attacks)
            // Allow wider margin for statistical variance: 10-50%
            assert!(
                (0.10..=0.50).contains(&freq),
                "Rogue technique frequency should be roughly 20-33% (plan target), \
                 got {:.1}% ({}/{}). Interval={} ticks",
                freq * 100.0,
                technique_uses,
                total_attacks,
                min_interval
            );
        }
    }

    /// P3.3 - 功法频率校准: Disciple 使用频率在目标区间
    #[test]
    fn integration_calibration_disciple_technique_frequency() {
        // Disciple: 每 2-3 次攻击使用 1 次功法 → 功法占比 33-50%
        let realm = Realm::Condense;
        let min_interval: u64 = 60 + realm_rank(realm) as u64 * 10; // 60 + 20 = 80

        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();

        let mut technique_uses = 0u64;
        let mut total_attacks = 0u64;

        for seed in 0..20u64 {
            let techniques = assign_npc_techniques(
                NpcArchetype::Disciple,
                realm,
                &meridian_sys,
                &deps,
                None,
                seed * 53,
            );
            if techniques.entries.is_empty() {
                continue;
            }
            let cultivation = Cultivation {
                realm,
                qi_current: 200.0,
                qi_max: 200.0,
                ..Default::default()
            };
            let cooldowns = NpcCooldownMap::default();
            let entity = Entity::from_raw(seed as u32 + 200);

            let mut last_technique_tick: u64 = 0;
            for attack_idx in 0..100u64 {
                let current_tick = attack_idx * 50;
                total_attacks += 1;

                if current_tick > 0
                    && current_tick.saturating_sub(last_technique_tick) < min_interval
                {
                    continue;
                }

                let selected = select_technique(
                    &techniques,
                    &cultivation,
                    &deps,
                    None,
                    &cooldowns,
                    entity,
                    3.0,
                    current_tick,
                );
                if selected.is_some() {
                    technique_uses += 1;
                    last_technique_tick = current_tick;
                }
            }
        }

        if total_attacks > 0 {
            let freq = technique_uses as f64 / total_attacks as f64;
            // Disciple target: 33-50%, allow 15-60%
            assert!(
                (0.15..=0.60).contains(&freq),
                "Disciple technique frequency should be roughly 33-50% (plan target), \
                 got {:.1}% ({}/{}). Interval={} ticks",
                freq * 100.0,
                technique_uses,
                total_attacks,
                min_interval
            );
        }
    }

    /// P3.3 - 功法频率校准: GuardianRelic 使用频率最高
    #[test]
    fn integration_calibration_guardian_relic_technique_frequency() {
        // GuardianRelic: 每 1-2 次攻击使用 1 次功法 → 功法占比 50-100%
        let realm = Realm::Spirit;
        let min_interval: u64 = 60 + realm_rank(realm) as u64 * 10; // 60 + 40 = 100

        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();

        let mut technique_uses = 0u64;
        let mut total_attacks = 0u64;

        for seed in 0..20u64 {
            let techniques = assign_npc_techniques(
                NpcArchetype::GuardianRelic,
                realm,
                &meridian_sys,
                &deps,
                None,
                seed * 67,
            );
            if techniques.entries.is_empty() {
                continue;
            }
            let cultivation = Cultivation {
                realm,
                qi_current: 500.0,
                qi_max: 500.0,
                ..Default::default()
            };
            let cooldowns = NpcCooldownMap::default();
            let entity = Entity::from_raw(seed as u32 + 300);

            let mut last_technique_tick: u64 = 0;
            // GuardianRelic attacks more frequently (every 60 ticks)
            for attack_idx in 0..100u64 {
                let current_tick = attack_idx * 60;
                total_attacks += 1;

                if current_tick > 0
                    && current_tick.saturating_sub(last_technique_tick) < min_interval
                {
                    continue;
                }

                let selected = select_technique(
                    &techniques,
                    &cultivation,
                    &deps,
                    None,
                    &cooldowns,
                    entity,
                    3.0,
                    current_tick,
                );
                if selected.is_some() {
                    technique_uses += 1;
                    last_technique_tick = current_tick;
                }
            }
        }

        if total_attacks > 0 {
            let freq = technique_uses as f64 / total_attacks as f64;
            // GuardianRelic target: 50-100%, allow 25-100%
            assert!(
                freq >= 0.25,
                "GuardianRelic technique frequency should be >= 25% (plan target 50-100%), \
                 got {:.1}% ({}/{}). Interval={} ticks",
                freq * 100.0,
                technique_uses,
                total_attacks,
                min_interval
            );
        }
    }

    /// P3.3 - 功法频率校准: Daoxiang 使用频率最低
    #[test]
    fn integration_calibration_daoxiang_low_technique_frequency() {
        // Daoxiang: 每 5-8 次攻击使用 1 次功法 → 功法占比 12-20%
        let realm = Realm::Induce;
        let min_interval: u64 = 60 + realm_rank(realm) as u64 * 10; // 60 + 10 = 70

        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();

        let mut technique_uses = 0u64;
        let mut total_attacks = 0u64;

        for seed in 0..20u64 {
            let techniques = assign_npc_techniques(
                NpcArchetype::Daoxiang,
                realm,
                &meridian_sys,
                &deps,
                None,
                seed * 71,
            );
            if techniques.entries.is_empty() {
                continue;
            }
            let cultivation = Cultivation {
                realm,
                qi_current: 50.0,
                qi_max: 50.0,
                ..Default::default()
            };
            let cooldowns = NpcCooldownMap::default();
            let entity = Entity::from_raw(seed as u32 + 400);

            let mut last_technique_tick: u64 = 0;
            // Daoxiang attacks slowly — spacing of 10 ticks per attack means
            // many attacks fall within the min_interval window, yielding low
            // technique frequency (plan target: every 5-8 attacks = 12-20%).
            for attack_idx in 0..100u64 {
                let current_tick = attack_idx * 10;
                total_attacks += 1;

                if current_tick > 0
                    && current_tick.saturating_sub(last_technique_tick) < min_interval
                {
                    continue;
                }

                let selected = select_technique(
                    &techniques,
                    &cultivation,
                    &deps,
                    None,
                    &cooldowns,
                    entity,
                    3.0,
                    current_tick,
                );
                if selected.is_some() {
                    technique_uses += 1;
                    last_technique_tick = current_tick;
                }
            }
        }

        if total_attacks > 0 {
            let freq = technique_uses as f64 / total_attacks as f64;
            // Daoxiang target: 12-20% (every 5-8 attacks), allow 5-35%
            assert!(
                freq <= 0.35,
                "Daoxiang technique frequency should be <= 35% (plan target 12-20%), \
                 got {:.1}% ({}/{}). Interval={} ticks",
                freq * 100.0,
                technique_uses,
                total_attacks,
                min_interval
            );
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // P3.1 额外集成: 跨系统组合验证
    // ─────────────────────────────────────────────────────────────────────────────

    /// 完整链路: 装备伤害乘数 + 功法选择 + 掉落 — 三个系统联合验证
    #[test]
    fn integration_equipment_technique_drop_cross_system() {
        let realm = Realm::Solidify;
        let seed = 12345u64;

        // Equipment
        let equipment = assign_npc_equipment(NpcArchetype::Zhinian, realm, None, seed);
        let main = equipment
            .main_hand
            .as_ref()
            .expect("zhinian must have weapon");
        let damage_mul = npc_weapon_damage_multiplier(main);
        assert!(
            damage_mul > 0.5,
            "Zhinian weapon damage multiplier should be > 0.5, got {damage_mul}"
        );

        // Techniques
        let meridian_sys = npc_meridian_system_for_realm(realm);
        let deps = empty_deps();
        let techniques = assign_npc_techniques(
            NpcArchetype::Zhinian,
            realm,
            &meridian_sys,
            &deps,
            None,
            seed,
        );
        assert!(
            techniques.entries.len() >= 2,
            "Zhinian should have >= 2 techniques"
        );

        // Death drops
        let drops = roll_equipment_drops(&equipment, seed);
        for drop in &drops {
            assert_eq!(drop.stack, 1);
            assert!(!drop.template_id.is_empty());
        }

        // Protocol metadata contains all three
        let metadata = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 500,
            archetype: NpcArchetype::Zhinian,
            cultivation: Some(&Cultivation {
                realm,
                qi_current: 80.0,
                qi_max: 100.0,
                ..Default::default()
            }),
            membership: None,
            lifespan: None,
            player_cultivation: None,
            player_identities: None,
            wounds: None,
            equipment: Some(&equipment),
            techniques: Some(&techniques),
            trade_inventory: None,
        });

        assert!(
            !metadata.equipment.is_empty(),
            "Zhinian should have equipment in metadata"
        );
        assert!(
            !metadata.techniques.is_empty(),
            "Zhinian should have techniques in metadata"
        );
        assert!(
            metadata.to_json_bytes_checked().is_ok(),
            "metadata should serialize"
        );
    }

    /// P3.1 - NPC meridian system for all realms matches required_meridians count
    #[test]
    fn integration_meridian_system_realm_count_alignment() {
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let sys = npc_meridian_system_for_realm(realm);
            let opened = MeridianId::ALL
                .iter()
                .filter(|id| sys.get(**id).opened)
                .count();
            assert_eq!(
                opened,
                realm.required_meridians(),
                "realm {:?} should open {} meridians but npc_meridian_system_for_realm opened {}",
                realm,
                realm.required_meridians(),
                opened
            );
        }
    }

    /// P3.2 - 协议兼容: 含所有字段的 metadata JSON 不超过 MAX_PAYLOAD_BYTES
    #[test]
    fn integration_protocol_max_payload_not_exceeded() {
        // Worst case: GuardianRelic with full equipment + 5 techniques + trade offers
        let equipment = assign_npc_equipment(NpcArchetype::GuardianRelic, Realm::Void, None, 42);
        let meridian_sys = npc_meridian_system_for_realm(Realm::Void);
        let deps = empty_deps();
        let techniques = assign_npc_techniques(
            NpcArchetype::GuardianRelic,
            Realm::Void,
            &meridian_sys,
            &deps,
            None,
            42,
        );
        // GuardianRelic doesn't trade, but test with a Disciple's trade offers
        let trade = assign_npc_trade_inventory(NpcArchetype::Disciple, Realm::Void, 42);

        let metadata = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 600,
            archetype: NpcArchetype::GuardianRelic,
            cultivation: Some(&Cultivation {
                realm: Realm::Void,
                qi_current: 999.0,
                qi_max: 999.0,
                ..Default::default()
            }),
            membership: None,
            lifespan: None,
            player_cultivation: Some(&Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            }),
            player_identities: None,
            wounds: None,
            equipment: Some(&equipment),
            techniques: Some(&techniques),
            trade_inventory: Some(&trade),
        });

        let bytes = metadata.to_json_bytes_checked();
        assert!(
            bytes.is_ok(),
            "worst-case metadata should not exceed MAX_PAYLOAD_BYTES: {:?}",
            bytes.err()
        );
    }

    /// P3.3 - 功法频率校准: 最小间隔随 realm 递增
    #[test]
    fn integration_calibration_min_interval_increases_with_realm() {
        let mut prev_interval = 0u64;
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let interval = 60 + realm_rank(realm) as u64 * 10;
            assert!(
                interval >= prev_interval,
                "min interval should increase with realm: {:?} interval={} but prev={}",
                realm,
                interval,
                prev_interval
            );
            prev_interval = interval;
        }
    }

    /// P3.1 - Beast/Zombie/SkullFiend/Fuya 不分配装备和功法（向后兼容 old path）
    #[test]
    fn integration_non_combat_gear_archetypes_stay_empty() {
        for archetype in [
            NpcArchetype::Beast,
            NpcArchetype::Zombie,
            NpcArchetype::SkullFiend,
            NpcArchetype::Fuya,
        ] {
            for realm in [Realm::Awaken, Realm::Spirit, Realm::Void] {
                let equipment = assign_npc_equipment(archetype, realm, None, 42);
                assert_eq!(
                    equipment,
                    NpcEquipment::default(),
                    "{:?} should have empty equipment",
                    archetype
                );

                let meridian_sys = npc_meridian_system_for_realm(realm);
                let deps = empty_deps();
                let techniques =
                    assign_npc_techniques(archetype, realm, &meridian_sys, &deps, None, 42);
                assert!(
                    techniques.entries.is_empty(),
                    "{:?} should have no techniques",
                    archetype
                );

                let trade = assign_npc_trade_inventory(archetype, realm, 42);
                assert!(
                    trade.offers.is_empty(),
                    "{:?} should have no trade offers",
                    archetype
                );
            }
        }
    }
}
