#![allow(dead_code, unused_imports)]
use bong_server::body_plan::RaceRegistry;
use bong_server::combat::body_conditioning::apply_guangbo_ticao_bonuses;
use bong_server::cultivation::components::Realm;
use bong_server::cultivation::components::{Cultivation, MeridianId, MeridianSystem};
use bong_server::cultivation::known_techniques::{
    parse_required_realm, SkillCategory, TechniqueDefinition, TechniqueDispatch, TechniqueRegistry,
};
use bong_server::cultivation::known_techniques::{KnownTechnique, KnownTechniques};
use bong_server::cultivation::meridian::severed::{
    MeridianSeveredPermanent, SeveredSource, SkillMeridianDependencies,
};
use bong_server::cultivation::technique_scroll::realm_rank;
use bong_server::npc::lifecycle::NpcArchetype;
use bong_server::npc::technique::*;
use std::path::Path;
use valence::prelude::Entity;

fn registry() -> &'static TechniqueRegistry {
    static REGISTRY: std::sync::OnceLock<TechniqueRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(bong_server::cultivation::known_techniques::DEFAULT_TECHNIQUES_PATH);
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let plans = bong_server::body_plan::BodyPlanRegistry::load_dir(
            root.join("assets/body_plans/plans"),
        )
        .unwrap();
        let races =
            RaceRegistry::load_file(root.join("assets/body_plans/races.json"), &plans).unwrap();
        TechniqueRegistry::load_from_path(path, &races)
            .expect("checked-in technique catalog must load")
    })
}

fn technique_definition_for_test(id: &str) -> Option<TechniqueDefinition> {
    registry().get(id).cloned()
}

fn assign_npc_techniques(
    archetype: NpcArchetype,
    realm: Realm,
    meridian_sys: &MeridianSystem,
    meridian_deps: &SkillMeridianDependencies,
    qi_color_hint: Option<&str>,
    entity_seed: u64,
) -> KnownTechniques {
    bong_server::npc::technique::assign_npc_techniques(
        registry(),
        archetype,
        realm,
        meridian_sys,
        meridian_deps,
        qi_color_hint,
        entity_seed,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_technique(
    known: &KnownTechniques,
    cultivation: &Cultivation,
    meridian_deps: &SkillMeridianDependencies,
    severed: Option<&MeridianSeveredPermanent>,
    meridian_sys: Option<&MeridianSystem>,
    cooldowns: &NpcCooldownMap,
    npc_entity: Entity,
    target_distance: f32,
    current_tick: u64,
    ctx: &NpcSkillScoringContext,
    category_filter: Option<SkillCategory>,
) -> Option<SelectedTechnique> {
    bong_server::npc::technique::select_technique(
        registry(),
        known,
        cultivation,
        meridian_deps,
        severed,
        meridian_sys,
        cooldowns,
        npc_entity,
        target_distance,
        current_tick,
        ctx,
        category_filter,
    )
}

#[allow(clippy::too_many_arguments)]
fn has_usable_heal_technique(
    known: &KnownTechniques,
    cultivation: &Cultivation,
    deps: &SkillMeridianDependencies,
    severed: Option<&MeridianSeveredPermanent>,
    meridian_sys: Option<&MeridianSystem>,
    cooldowns: &NpcCooldownMap,
    npc_entity: Entity,
    current_tick: u64,
) -> bool {
    bong_server::npc::technique::has_usable_heal_technique(
        registry(),
        known,
        cultivation,
        deps,
        severed,
        meridian_sys,
        cooldowns,
        npc_entity,
        current_tick,
    )
}

/// 创建一个有 N 条经脉已开的 MeridianSystem（方便测试）。
fn meridian_sys_with_opened(ids: &[MeridianId]) -> MeridianSystem {
    let mut sys = MeridianSystem::default();
    for id in ids {
        let m = sys.get_mut(*id);
        m.opened = true;
        m.integrity = 1.0;
        m.throughput_current = 1.0;
    }
    sys
}

/// 全 12 正经已开的 MeridianSystem。
fn full_regular_meridians() -> MeridianSystem {
    use MeridianId::*;
    meridian_sys_with_opened(&[
        Lung,
        LargeIntestine,
        Stomach,
        Spleen,
        Heart,
        SmallIntestine,
        Bladder,
        Kidney,
        Pericardium,
        TripleEnergizer,
        Gallbladder,
        Liver,
    ])
}

/// 全 20 条经脉已开的 MeridianSystem。
fn full_all_meridians() -> MeridianSystem {
    use MeridianId::*;
    meridian_sys_with_opened(&[
        Lung,
        LargeIntestine,
        Stomach,
        Spleen,
        Heart,
        SmallIntestine,
        Bladder,
        Kidney,
        Pericardium,
        TripleEnergizer,
        Gallbladder,
        Liver,
        Ren,
        Du,
        Chong,
        Dai,
        YinQiao,
        YangQiao,
        YinWei,
        YangWei,
    ])
}

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

// === assign_npc_techniques: archetype coverage ===

#[test]
fn assign_commoner_returns_empty() {
    let sys = MeridianSystem::default();
    let deps = empty_deps();
    let kt = assign_npc_techniques(NpcArchetype::Commoner, Realm::Awaken, &sys, &deps, None, 42);
    assert!(kt.entries.is_empty(), "commoner should have no techniques");
}

#[test]
fn assign_beast_returns_empty() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    let kt = assign_npc_techniques(NpcArchetype::Beast, Realm::Condense, &sys, &deps, None, 42);
    assert!(kt.entries.is_empty(), "beast should have no techniques");
}

#[test]
fn assign_skull_fiend_returns_empty() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    let kt = assign_npc_techniques(NpcArchetype::SkullFiend, Realm::Void, &sys, &deps, None, 42);
    assert!(
        kt.entries.is_empty(),
        "skull fiend should have no techniques"
    );
}

#[test]
fn assign_fuya_returns_empty() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    let kt = assign_npc_techniques(NpcArchetype::Fuya, Realm::Spirit, &sys, &deps, None, 42);
    assert!(kt.entries.is_empty(), "fuya should have no techniques");
}

#[test]
fn assign_zombie_returns_empty() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    let kt = assign_npc_techniques(NpcArchetype::Zombie, Realm::Awaken, &sys, &deps, None, 42);
    assert!(kt.entries.is_empty(), "zombie should have no techniques");
}

#[test]
fn assign_rogue_awaken_returns_1_to_3() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    // Run multiple seeds to hit different counts
    for seed in 0..50u64 {
        let kt = assign_npc_techniques(NpcArchetype::Rogue, Realm::Awaken, &sys, &deps, None, seed);
        assert!(
            !kt.entries.is_empty() && kt.entries.len() <= 3,
            "rogue awaken should have 1-3 techniques, got {} (seed={})",
            kt.entries.len(),
            seed
        );
        for entry in &kt.entries {
            assert!(
                entry.proficiency >= 0.2 && entry.proficiency <= 0.7,
                "rogue proficiency should be 0.2-0.7, got {} for {} (seed={})",
                entry.proficiency,
                entry.id,
                seed
            );
            assert!(entry.active, "assigned techniques should be active");
        }
    }
}

#[test]
fn assign_disciple_returns_2_to_6() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for seed in 0..50u64 {
        let kt = assign_npc_techniques(
            NpcArchetype::Disciple,
            Realm::Condense,
            &sys,
            &deps,
            None,
            seed * 7,
        );
        assert!(
                kt.entries.len() >= 2 && kt.entries.len() <= 6,
                "disciple Condense should have 2-6 techniques (base 2-4 + heal + buff), got {} (seed={})",
                kt.entries.len(),
                seed
            );
        for entry in &kt.entries {
            assert!(
                entry.proficiency >= 0.3 && entry.proficiency <= 0.8,
                "disciple proficiency should be 0.3-0.8, got {}",
                entry.proficiency
            );
        }
    }
}

#[test]
fn assign_guardian_relic_returns_3_to_7() {
    let sys = full_all_meridians();
    let deps = empty_deps();
    for seed in 0..50u64 {
        let kt = assign_npc_techniques(
            NpcArchetype::GuardianRelic,
            Realm::Spirit,
            &sys,
            &deps,
            None,
            seed * 13,
        );
        assert!(
                kt.entries.len() >= 3 && kt.entries.len() <= 7,
                "guardian relic Spirit should have 3-7 techniques (base 3-5 + heal + buff), got {} (seed={})",
                kt.entries.len(),
                seed
            );
        for entry in &kt.entries {
            assert!(
                entry.proficiency >= 0.6 && entry.proficiency <= 0.9,
                "guardian relic proficiency should be 0.6-0.9, got {}",
                entry.proficiency
            );
        }
    }
}

#[test]
fn assign_daoxiang_returns_1_to_3() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for seed in 0..50u64 {
        let kt = assign_npc_techniques(
            NpcArchetype::Daoxiang,
            Realm::Induce,
            &sys,
            &deps,
            None,
            seed * 17,
        );
        assert!(
            !kt.entries.is_empty() && kt.entries.len() <= 3,
            "daoxiang Induce should have 1-3 techniques (base 1-2 + heal), got {} (seed={})",
            kt.entries.len(),
            seed
        );
        for entry in &kt.entries {
            assert!(
                entry.proficiency >= 0.1 && entry.proficiency <= 0.4,
                "daoxiang proficiency should be 0.1-0.4, got {}",
                entry.proficiency
            );
        }
    }
}

#[test]
fn assign_zhinian_returns_2_to_5() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for seed in 0..50u64 {
        let kt = assign_npc_techniques(
            NpcArchetype::Zhinian,
            Realm::Condense,
            &sys,
            &deps,
            None,
            seed * 19,
        );
        assert!(
                kt.entries.len() >= 2 && kt.entries.len() <= 5,
                "zhinian Condense should have 2-5 techniques (base 2-3 + heal + buff), got {} (seed={})",
                kt.entries.len(),
                seed
            );
        for entry in &kt.entries {
            assert!(
                entry.proficiency >= 0.3 && entry.proficiency <= 0.6,
                "zhinian proficiency should be 0.3-0.6, got {}",
                entry.proficiency
            );
        }
    }
}

#[test]
fn realm_too_low_excludes_techniques() {
    // Awaken NPC should not get Induce+ techniques
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for seed in 0..100u64 {
        let kt = assign_npc_techniques(NpcArchetype::Rogue, Realm::Awaken, &sys, &deps, None, seed);
        for entry in &kt.entries {
            let def = technique_definition_for_test(&entry.id).expect("valid technique");
            let required = parse_required_realm(&def.required_realm).unwrap();
            assert!(
                realm_rank(required) <= realm_rank(Realm::Awaken),
                "Awaken NPC should not have technique {} requiring {:?}",
                entry.id,
                def.required_realm
            );
        }
    }
}

#[test]
fn no_meridians_opened_returns_empty_for_meridian_gated_techniques() {
    // MeridianSystem default = all closed
    let sys = MeridianSystem::default();
    let deps = empty_deps();
    // Even Induce rogue — techniques requiring meridians should be excluded
    let kt = assign_npc_techniques(NpcArchetype::Rogue, Realm::Induce, &sys, &deps, None, 42);
    for entry in &kt.entries {
        let def = technique_definition_for_test(&entry.id).expect("valid technique");
        assert!(
            def.required_meridians.is_empty(),
            "technique {} requires meridians but NPC has none opened — should have been filtered",
            entry.id
        );
    }
}

// === assign_npc_techniques: P1.4 NPC utility skill injection ===

#[test]
fn assign_induce_rogue_always_has_heal_basic() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for seed in 0..100u64 {
        let kt = assign_npc_techniques(NpcArchetype::Rogue, Realm::Induce, &sys, &deps, None, seed);
        assert!(
            kt.entries.iter().any(|e| e.id == "npc.heal_basic"),
            "Induce+ Rogue should always have npc.heal_basic (seed={})",
            seed
        );
    }
}

#[test]
fn assign_condense_rogue_always_has_heal_and_buff() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for seed in 0..100u64 {
        let kt = assign_npc_techniques(
            NpcArchetype::Rogue,
            Realm::Condense,
            &sys,
            &deps,
            None,
            seed,
        );
        assert!(
            kt.entries.iter().any(|e| e.id == "npc.heal_basic"),
            "Condense+ Rogue should always have npc.heal_basic (seed={})",
            seed
        );
        let has_buff = kt
            .entries
            .iter()
            .any(|e| e.id == "npc.buff_speed" || e.id == "npc.buff_defense");
        assert!(
            has_buff,
            "Condense+ Rogue should always have npc.buff_speed or npc.buff_defense (seed={})",
            seed
        );
    }
}

#[test]
fn assign_awaken_rogue_never_has_npc_utility_skills() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for seed in 0..100u64 {
        let kt = assign_npc_techniques(NpcArchetype::Rogue, Realm::Awaken, &sys, &deps, None, seed);
        for entry in &kt.entries {
            assert!(
                !entry.id.starts_with("npc."),
                "Awaken NPC should not have NPC utility skill {}, got it at seed={}",
                entry.id,
                seed
            );
        }
    }
}

#[test]
fn assign_npc_utility_skills_no_duplicates() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for seed in 0..200u64 {
        let kt = assign_npc_techniques(
            NpcArchetype::Disciple,
            Realm::Condense,
            &sys,
            &deps,
            None,
            seed,
        );
        let ids: Vec<&str> = kt.entries.iter().map(|e| e.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(
            ids.len(),
            unique.len(),
            "no duplicate technique IDs should exist, got {:?} (seed={})",
            ids,
            seed
        );
    }
}

#[test]
fn assign_npc_heal_blocked_when_meridians_closed() {
    let sys = MeridianSystem::default();
    let deps = empty_deps();
    for seed in 0..50u64 {
        let kt = assign_npc_techniques(NpcArchetype::Rogue, Realm::Induce, &sys, &deps, None, seed);
        assert!(
                !kt.entries.iter().any(|e| e.id == "npc.heal_basic"),
                "npc.heal_basic requires Spleen+Kidney meridians — should not appear with all closed (seed={})",
                seed
            );
    }
}

#[test]
fn assign_buff_is_speed_or_defense_deterministic_per_seed() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for seed in 0..100u64 {
        let a = assign_npc_techniques(
            NpcArchetype::Rogue,
            Realm::Condense,
            &sys,
            &deps,
            None,
            seed,
        );
        let b = assign_npc_techniques(
            NpcArchetype::Rogue,
            Realm::Condense,
            &sys,
            &deps,
            None,
            seed,
        );
        let buff_a: Vec<&str> = a
            .entries
            .iter()
            .filter(|e| e.id == "npc.buff_speed" || e.id == "npc.buff_defense")
            .map(|e| e.id.as_str())
            .collect();
        let buff_b: Vec<&str> = b
            .entries
            .iter()
            .filter(|e| e.id == "npc.buff_speed" || e.id == "npc.buff_defense")
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(
            buff_a, buff_b,
            "same seed should pick same buff variant (seed={})",
            seed
        );
    }
}

#[test]
fn assign_all_combat_archetypes_get_heal_at_induce() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for archetype in [
        NpcArchetype::Rogue,
        NpcArchetype::Disciple,
        NpcArchetype::GuardianRelic,
        NpcArchetype::Daoxiang,
        NpcArchetype::Zhinian,
    ] {
        let kt = assign_npc_techniques(archetype, Realm::Induce, &sys, &deps, None, 42);
        assert!(
            kt.entries.iter().any(|e| e.id == "npc.heal_basic"),
            "{:?} at Induce should have npc.heal_basic",
            archetype
        );
    }
}

#[test]
fn assign_non_combat_archetypes_never_get_npc_skills() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for archetype in [
        NpcArchetype::Commoner,
        NpcArchetype::Beast,
        NpcArchetype::SkullFiend,
        NpcArchetype::Fuya,
        NpcArchetype::Zombie,
    ] {
        let kt = assign_npc_techniques(archetype, Realm::Void, &sys, &deps, None, 42);
        assert!(
            kt.entries.is_empty(),
            "{:?} should have no techniques even at Void",
            archetype
        );
    }
}

// === assign_npc_techniques: determinism ===

#[test]
fn assign_techniques_deterministic() {
    let sys = full_regular_meridians();
    let deps = empty_deps();
    for archetype in [
        NpcArchetype::Rogue,
        NpcArchetype::Disciple,
        NpcArchetype::GuardianRelic,
        NpcArchetype::Daoxiang,
        NpcArchetype::Zhinian,
    ] {
        let a = assign_npc_techniques(archetype, Realm::Condense, &sys, &deps, None, 12345);
        let b = assign_npc_techniques(archetype, Realm::Condense, &sys, &deps, None, 12345);
        assert_eq!(
            a, b,
            "same seed should produce identical techniques for {:?}",
            archetype
        );
    }
}

// === select_technique: basic ===

#[test]
fn select_technique_with_available_returns_some() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "sword.cleave".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(result.is_some(), "should select a technique");
    let sel = result.unwrap();
    assert_eq!(sel.technique_id, "sword.cleave");
    assert_eq!(sel.target, SkillTarget::NearestEnemy);
}

#[test]
fn select_technique_inactive_excluded() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "sword.cleave".to_string(),
            proficiency: 0.5,
            active: false,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(result.is_none(), "inactive technique should be excluded");
}

#[test]
fn select_technique_on_cooldown_excluded() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "sword.cleave".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let mut cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    cooldowns.set(entity, "sword.cleave", 200); // on CD until tick 200

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(result.is_none(), "technique on cooldown should be excluded");
}

#[test]
fn select_technique_cooldown_expired_available() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "sword.cleave".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let mut cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    cooldowns.set(entity, "sword.cleave", 100); // expired at tick 100

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(result.is_some(), "expired cooldown should allow technique");
}

#[test]
fn select_technique_general_pool_excludes_heal_but_heal_channel_keeps_it() {
    // #3 回归锁：通用功法池(category_filter=None)必须排除 Heal——Heal 有专属
    // NpcHealScorer→NpcHealAction(filter=Some(Heal)) 通道。若漏出到通用池，
    // category_weight(Heal) 即便为 0 也被 .max(0.001) 抬成非零权重，NPC 满血时
    // 仍有概率用「通用功法回合」self-cast 治疗，抢占进攻。Defense 早已被排除，Heal 此前漏排。
    assert_eq!(
        technique_definition_for_test("npc.heal_basic").map(|d| d.category),
        Some(SkillCategory::Heal),
        "前置假设：npc.heal_basic 必须是 Heal 类别"
    );
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "npc.heal_basic".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 1000.0,
        qi_max: 1000.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    // 通用池：唯一候选是 heal → 被排除 → 候选空 → None（确定性，不依赖加权 roll）。
    let general = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(
        general.is_none(),
        "通用功法池必须排除 Heal，唯一 heal 候选时应返回 None，实得 {general:?}"
    );

    // 专属 heal 通道(filter=Some(Heal))：本修复不得误伤，仍须能选中 heal。
    let heal_channel = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        Some(SkillCategory::Heal),
    );
    assert_eq!(
        heal_channel.map(|s| s.technique_id),
        Some("npc.heal_basic".to_string()),
        "Heal 专属通道(filter=Some(Heal))必须仍能选中 heal，修复只动通用池"
    );
}

#[test]
fn select_technique_general_pool_picks_attack_over_leaked_heal() {
    // 通用池同时含 attack+heal 且 heal 熟练度远高时，heal 被排除 → 结果必为 attack
    // （确定性：候选集只剩 sword.cleave，与加权 roll 无关）。
    let known = KnownTechniques {
        entries: vec![
            KnownTechnique {
                id: "npc.heal_basic".to_string(),
                proficiency: 0.9,
                active: true,
            },
            KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.1,
                active: true,
            },
        ],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 1000.0,
        qi_max: 1000.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(7);

    let sel = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    )
    .expect("attack 功法仍在通用池，应可选");
    assert_eq!(
        sel.technique_id, "sword.cleave",
        "通用池排除 heal 后只剩 attack，即便 heal 熟练度更高也必须选 attack"
    );
    assert_eq!(sel.target, SkillTarget::NearestEnemy);
}

#[test]
fn select_technique_qi_insufficient_excluded() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "burst_meridian.beng_quan".to_string(), // qi_cost = 0.4
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Induce,
        qi_current: 0.1, // insufficient
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(
        result.is_none(),
        "technique with qi_cost > qi_current should be excluded"
    );
}

#[test]
fn select_technique_severed_meridian_excluded() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "woliu.burst".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let mut deps = SkillMeridianDependencies::default();
    deps.declare("woliu.burst", vec![MeridianId::Lung]);

    let mut severed = MeridianSeveredPermanent::default();
    severed.insert(MeridianId::Lung, SeveredSource::CombatWound, 50);

    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        Some(&severed),
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(
        result.is_none(),
        "technique with SEVERED dependent meridian should be excluded"
    );
}

// === select_technique: dugu-poison opened=false gate (skill-gate-001) ===

/// dugu 毒将经脉 opened 设为 false 但不写入 MeridianSeveredPermanent；
/// 传入 meridian_sys=Some 时，select_technique 应拒绝该功法。
#[test]
fn select_technique_dugu_poisoned_meridian_closed_excluded() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "woliu.burst".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let mut deps = SkillMeridianDependencies::default();
    deps.declare("woliu.burst", vec![MeridianId::Lung]);

    // Simulate dugu_poison_tick effect: Lung closed via opened=false, no SEVERED component
    let mut meridian_sys = meridian_sys_with_opened(&[
        MeridianId::LargeIntestine,
        MeridianId::Stomach,
        MeridianId::Spleen,
        MeridianId::Heart,
        MeridianId::SmallIntestine,
    ]);
    // Lung is left at default (opened=false) — simulates dugu poison closing it
    {
        let lung = meridian_sys.get_mut(MeridianId::Lung);
        lung.opened = false;
        lung.flow_capacity = 0.0;
    }

    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None, // no MeridianSeveredPermanent — dugu does NOT insert this
        Some(&meridian_sys),
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(
        result.is_none(),
        "dugu-poisoned NPC with Lung closed (opened=false, no SEVERED component) must not \
             be able to cast woliu.burst — expected None but got Some"
    );
}

/// 经脉 opened=true 时（无毒），select_technique 应正常选出功法（happy path）。
#[test]
fn select_technique_meridian_open_allows_technique() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "woliu.burst".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let mut deps = SkillMeridianDependencies::default();
    deps.declare("woliu.burst", vec![MeridianId::Lung]);

    // Lung is open
    let meridian_sys = meridian_sys_with_opened(&[MeridianId::Lung]);

    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        Some(&meridian_sys),
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(
        result.is_some(),
        "NPC with Lung open should be able to cast woliu.burst"
    );
    assert_eq!(result.unwrap().technique_id, "woliu.burst");
}

/// MeridianSystem 传 None 时（向后兼容），select_technique 不做 opened 检查，
/// 行为与修复前相同（仍可通过 SEVERED 途径拦截）。
#[test]
fn select_technique_no_meridian_sys_skips_opened_check() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "woliu.burst".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let mut deps = SkillMeridianDependencies::default();
    deps.declare("woliu.burst", vec![MeridianId::Lung]);

    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    // No meridian_sys — opened check is skipped, skill is selectable
    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None, // no meridian_sys → no opened check
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(
        result.is_some(),
        "without meridian_sys, select_technique should not gate on opened field"
    );
}

/// has_usable_heal_technique: dugu 毒关脉后 opened=false，heal 功法应被拒绝。
#[test]
fn has_usable_heal_dugu_poisoned_meridian_excluded() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "npc.heal_basic".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Induce,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    // Declare that npc.heal_basic depends on Spleen
    let mut deps = SkillMeridianDependencies::default();
    deps.declare("npc.heal_basic", vec![MeridianId::Spleen]);

    // dugu 毒把 Spleen 关掉（opened=false），但没有 MeridianSeveredPermanent
    let mut meridian_sys = MeridianSystem::default();
    {
        let spleen = meridian_sys.get_mut(MeridianId::Spleen);
        spleen.opened = false;
        spleen.flow_capacity = 0.0;
    }

    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = has_usable_heal_technique(
        &known,
        &cultivation,
        &deps,
        None, // no MeridianSeveredPermanent
        Some(&meridian_sys),
        &cooldowns,
        entity,
        100,
    );
    assert!(
        !result,
        "dugu-poisoned NPC with Spleen closed must not have usable heal — \
             expected false but got true"
    );
}

/// has_usable_heal_technique: 经脉 open 时治疗可用（happy path，含 opened 检查）。
#[test]
fn has_usable_heal_meridian_open_allows_heal() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "npc.heal_basic".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Induce,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let mut deps = SkillMeridianDependencies::default();
    deps.declare("npc.heal_basic", vec![MeridianId::Spleen]);

    let meridian_sys = meridian_sys_with_opened(&[MeridianId::Spleen]);
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = has_usable_heal_technique(
        &known,
        &cultivation,
        &deps,
        None,
        Some(&meridian_sys),
        &cooldowns,
        entity,
        100,
    );
    assert!(
        result,
        "NPC with Spleen open should have usable heal technique"
    );
}

#[test]
fn select_technique_all_on_cooldown_returns_none() {
    let known = KnownTechniques {
        entries: vec![
            KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.5,
                active: true,
            },
            KnownTechnique {
                id: "sword.thrust".to_string(),
                proficiency: 0.5,
                active: true,
            },
        ],
    };
    let cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let mut cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    cooldowns.set(entity, "sword.cleave", 200);
    cooldowns.set(entity, "sword.thrust", 200);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(
        result.is_none(),
        "all techniques on cooldown should return None"
    );
}

#[test]
fn select_technique_empty_known_returns_none() {
    let known = KnownTechniques {
        entries: Vec::new(),
    };
    let cultivation = Cultivation::default();
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(
        result.is_none(),
        "empty known techniques should return None"
    );
}

// === NpcCooldownMap ===

#[test]
fn cooldown_map_set_and_check() {
    let mut map = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    map.set(entity, "sword.cleave", 100);

    assert!(map.is_on_cooldown(entity, "sword.cleave", 50));
    assert!(map.is_on_cooldown(entity, "sword.cleave", 99));
    assert!(!map.is_on_cooldown(entity, "sword.cleave", 100));
    assert!(!map.is_on_cooldown(entity, "sword.cleave", 101));
}

#[test]
fn cooldown_map_different_entities_independent() {
    let mut map = NpcCooldownMap::default();
    let e1 = Entity::from_raw(1);
    let e2 = Entity::from_raw(2);
    map.set(e1, "sword.cleave", 100);

    assert!(map.is_on_cooldown(e1, "sword.cleave", 50));
    assert!(
        !map.is_on_cooldown(e2, "sword.cleave", 50),
        "cooldown for e1 should not affect e2"
    );
}

#[test]
fn cooldown_map_remove_all_for_entity() {
    let mut map = NpcCooldownMap::default();
    let e1 = Entity::from_raw(1);
    let e2 = Entity::from_raw(2);
    map.set(e1, "sword.cleave", 100);
    map.set(e1, "sword.thrust", 200);
    map.set(e2, "sword.cleave", 150);

    map.remove_all_for(e1);
    assert!(!map.is_on_cooldown(e1, "sword.cleave", 50));
    assert!(!map.is_on_cooldown(e1, "sword.thrust", 50));
    assert!(
        map.is_on_cooldown(e2, "sword.cleave", 50),
        "removing e1 entries should not affect e2"
    );
}

#[test]
fn cooldown_map_overwrite() {
    let mut map = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    map.set(entity, "sword.cleave", 100);
    map.set(entity, "sword.cleave", 200); // overwrite

    assert!(map.is_on_cooldown(entity, "sword.cleave", 150));
    assert!(!map.is_on_cooldown(entity, "sword.cleave", 200));
}

#[test]
fn cooldown_map_empty_check() {
    let map = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    assert!(
        !map.is_on_cooldown(entity, "sword.cleave", 0),
        "empty map should not report cooldown"
    );
}

// === select_technique: weighted random ===

#[test]
fn select_technique_higher_proficiency_more_likely() {
    let known = KnownTechniques {
        entries: vec![
            KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.01, // very low
                active: true,
            },
            KnownTechnique {
                id: "sword.thrust".to_string(),
                proficiency: 0.99, // very high
                active: true,
            },
        ],
    };
    let cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let mut thrust_count = 0;
    for tick in 0..1000u64 {
        if let Some(sel) = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            None,
            &cooldowns,
            entity,
            3.0,
            tick,
            &default_ctx(),
            None,
        ) {
            if sel.technique_id == "sword.thrust" {
                thrust_count += 1;
            }
        }
    }
    // sword.thrust with 0.99 should be selected much more than sword.cleave with 0.01
    assert!(
        thrust_count > 800,
        "high proficiency technique should be selected >80% of the time, got {}/1000",
        thrust_count
    );
}

// === meridian_deps_satisfied ===

#[test]
fn assign_all_archetypes_all_realms_valid() {
    let all_archetypes = [
        NpcArchetype::Zombie,
        NpcArchetype::Commoner,
        NpcArchetype::Rogue,
        NpcArchetype::Beast,
        NpcArchetype::Disciple,
        NpcArchetype::GuardianRelic,
        NpcArchetype::Daoxiang,
        NpcArchetype::Zhinian,
        NpcArchetype::Fuya,
        NpcArchetype::SkullFiend,
    ];
    let all_realms = [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ];
    let sys = full_all_meridians();
    let deps = empty_deps();

    for archetype in all_archetypes {
        for realm in all_realms {
            let kt = assign_npc_techniques(archetype, realm, &sys, &deps, None, 42);
            for entry in &kt.entries {
                assert!(
                    entry.proficiency >= 0.0 && entry.proficiency <= 1.0,
                    "{:?} x {:?}: proficiency {} out of range",
                    archetype,
                    realm,
                    entry.proficiency
                );
                assert!(entry.active);
                // Verify technique exists
                assert!(
                    technique_definition_for_test(&entry.id).is_some(),
                    "{:?} x {:?}: technique {} not found in definitions",
                    archetype,
                    realm,
                    entry.id
                );
                // Verify realm requirement met
                let def = technique_definition_for_test(&entry.id).unwrap();
                if let Some(required) = parse_required_realm(&def.required_realm) {
                    assert!(
                        realm_rank(required) <= realm_rank(realm),
                        "{:?} x {:?}: technique {} requires {:?} but NPC is {:?}",
                        archetype,
                        realm,
                        entry.id,
                        def.required_realm,
                        realm
                    );
                }
            }
        }
    }
}

// === parse_realm ===

#[test]
fn parse_realm_all_variants() {
    assert_eq!(parse_required_realm("Awaken"), Some(Realm::Awaken));
    assert_eq!(parse_required_realm("Induce"), Some(Realm::Induce));
    assert_eq!(parse_required_realm("Condense"), Some(Realm::Condense));
    assert_eq!(parse_required_realm("Solidify"), Some(Realm::Solidify));
    assert_eq!(parse_required_realm("Spirit"), Some(Realm::Spirit));
    assert_eq!(parse_required_realm("Void"), Some(Realm::Void));
    assert_eq!(parse_required_realm("invalid"), None);
    assert_eq!(parse_required_realm(""), None);
}

// === category_weight ===

#[test]
fn category_weight_heal_scales_with_missing_hp() {
    let ctx_full = NpcSkillScoringContext {
        hp_ratio: 1.0,
        ..default_ctx()
    };
    let ctx_half = NpcSkillScoringContext {
        hp_ratio: 0.5,
        ..default_ctx()
    };
    let ctx_low = NpcSkillScoringContext {
        hp_ratio: 0.3,
        ..default_ctx()
    };
    let ctx_zero = NpcSkillScoringContext {
        hp_ratio: 0.0,
        ..default_ctx()
    };

    let w_full = category_weight(SkillCategory::Heal, &ctx_full);
    let w_half = category_weight(SkillCategory::Heal, &ctx_half);
    let w_low = category_weight(SkillCategory::Heal, &ctx_low);
    let w_zero = category_weight(SkillCategory::Heal, &ctx_zero);

    assert!(
        w_full < f32::EPSILON,
        "full HP should yield ~0 heal weight, got {w_full}"
    );
    assert!(
        w_half > w_full,
        "half HP should yield higher heal weight than full"
    );
    assert!(
        w_low > w_half,
        "low HP should yield higher heal weight than half"
    );
    assert!(
        (w_zero - 0.9).abs() < f32::EPSILON,
        "zero HP should yield 0.9 heal weight, got {w_zero}"
    );
}

#[test]
fn category_weight_buff_in_combat_no_active() {
    let ctx = NpcSkillScoringContext {
        has_active_buff: false,
        in_combat: true,
        ..default_ctx()
    };
    assert!(
        (category_weight(SkillCategory::Buff, &ctx) - 0.6).abs() < f32::EPSILON,
        "buff in combat without active buff should be 0.6"
    );
}

#[test]
fn category_weight_buff_already_buffed() {
    let ctx = NpcSkillScoringContext {
        has_active_buff: true,
        in_combat: true,
        ..default_ctx()
    };
    assert!(
        (category_weight(SkillCategory::Buff, &ctx) - 0.05).abs() < f32::EPSILON,
        "buff with active buff should be 0.05"
    );
}

#[test]
fn category_weight_buff_out_of_combat() {
    let ctx = NpcSkillScoringContext {
        has_active_buff: false,
        in_combat: false,
        ..default_ctx()
    };
    assert!(
        (category_weight(SkillCategory::Buff, &ctx) - 0.05).abs() < f32::EPSILON,
        "buff out of combat should be 0.05"
    );
}

#[test]
fn category_weight_attack_constant() {
    let ctx = default_ctx();
    assert!(
        (category_weight(SkillCategory::Attack, &ctx) - 0.8).abs() < f32::EPSILON,
        "attack weight should be 0.8"
    );
}

#[test]
fn category_weight_control_constant() {
    let ctx = default_ctx();
    assert!(
        (category_weight(SkillCategory::Control, &ctx) - 0.4).abs() < f32::EPSILON,
        "control weight should be 0.4"
    );
}

#[test]
fn category_weight_defense_zero() {
    let ctx = default_ctx();
    assert!(
        category_weight(SkillCategory::Defense, &ctx) < f32::EPSILON,
        "defense weight should be 0.0"
    );
}

#[test]
fn select_technique_defense_excluded() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "sword.parry".to_string(),
            proficiency: 0.9,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        None,
    );
    assert!(
        result.is_none(),
        "Defense category techniques should be excluded from select_technique"
    );
}

#[test]
fn select_technique_low_qi_filters_high_cost() {
    let known = KnownTechniques {
        entries: vec![
            KnownTechnique {
                id: "sword.cleave".to_string(), // qi_cost = 0.0
                proficiency: 0.5,
                active: true,
            },
            KnownTechnique {
                id: "woliu.heart".to_string(), // qi_cost = 50.0
                proficiency: 0.5,
                active: true,
            },
        ],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let low_qi_ctx = NpcSkillScoringContext {
        qi_ratio: 0.1,
        ..default_ctx()
    };

    let mut cleave_selected = false;
    let mut heart_selected = false;
    for tick in 0..500u64 {
        if let Some(sel) = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            None,
            &cooldowns,
            entity,
            3.0,
            tick,
            &low_qi_ctx,
            None,
        ) {
            match sel.technique_id.as_str() {
                "sword.cleave" => cleave_selected = true,
                "woliu.heart" => heart_selected = true,
                _ => {}
            }
        }
    }

    assert!(
        cleave_selected,
        "low qi_cost technique should be selectable at low qi_ratio"
    );
    assert!(
        !heart_selected,
        "high qi_cost technique should be filtered at qi_ratio < 0.15"
    );
}

// === category_filter: select_technique with filter ===

#[test]
fn select_technique_category_filter_heal_only() {
    let known = KnownTechniques {
        entries: vec![
            KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.5,
                active: true,
            },
            KnownTechnique {
                id: "zhenmai.neutralize".to_string(),
                proficiency: 0.5,
                active: true,
            },
        ],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        Some(SkillCategory::Heal),
    );
    let sel = result.expect("with Heal filter, zhenmai.neutralize should be selectable");
    assert_eq!(sel.technique_id, "zhenmai.neutralize");
    assert_eq!(
        sel.target,
        SkillTarget::SelfCast,
        "Heal should route to SelfCast"
    );
}

#[test]
fn select_technique_category_filter_returns_none_when_no_match() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "sword.cleave".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        Some(SkillCategory::Heal),
    );
    assert!(
        result.is_none(),
        "no Heal techniques available, should return None"
    );
}

#[test]
fn select_technique_category_filter_defense_selectable_when_explicit() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "sword.parry".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);

    let result = select_technique(
        &known,
        &cultivation,
        &deps,
        None,
        None,
        &cooldowns,
        entity,
        3.0,
        100,
        &default_ctx(),
        Some(SkillCategory::Defense),
    );
    let sel = result.expect("Defense filter should override the default Defense exclusion");
    assert_eq!(sel.technique_id, "sword.parry");
    assert_eq!(
        sel.target,
        SkillTarget::NearestEnemy,
        "Defense should route to NearestEnemy"
    );
}

// === has_usable_heal_technique ===

#[test]
fn has_usable_heal_with_heal_technique() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "zhenmai.neutralize".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    assert!(
        has_usable_heal_technique(
            &known,
            &cultivation,
            &deps,
            None,
            None,
            &cooldowns,
            entity,
            100
        ),
        "NPC with active heal technique should have usable heal"
    );
}

#[test]
fn has_usable_heal_without_heal_technique() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "sword.cleave".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    assert!(
        !has_usable_heal_technique(
            &known,
            &cultivation,
            &deps,
            None,
            None,
            &cooldowns,
            entity,
            100
        ),
        "NPC with only Attack techniques should not have usable heal"
    );
}

#[test]
fn has_usable_heal_on_cooldown() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "zhenmai.neutralize".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let mut cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    cooldowns.set(entity, "zhenmai.neutralize", 200);
    assert!(
        !has_usable_heal_technique(
            &known,
            &cultivation,
            &deps,
            None,
            None,
            &cooldowns,
            entity,
            100
        ),
        "heal technique on cooldown should not be usable"
    );
}

#[test]
fn has_usable_heal_inactive_technique() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "zhenmai.neutralize".to_string(),
            proficiency: 0.5,
            active: false,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    assert!(
        !has_usable_heal_technique(
            &known,
            &cultivation,
            &deps,
            None,
            None,
            &cooldowns,
            entity,
            100
        ),
        "inactive heal technique should not be usable"
    );
}

#[test]
fn has_usable_heal_insufficient_qi() {
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "zhenmai.neutralize".to_string(),
            proficiency: 0.5,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let deps = empty_deps();
    let cooldowns = NpcCooldownMap::default();
    let entity = Entity::from_raw(1);
    let def = technique_definition_for_test("zhenmai.neutralize").unwrap();
    if def.qi_cost > 0.0 {
        assert!(
            !has_usable_heal_technique(
                &known,
                &cultivation,
                &deps,
                None,
                None,
                &cooldowns,
                entity,
                100
            ),
            "heal technique should not be usable with 0 qi when qi_cost > 0"
        );
    }
}

// === NpcCooldownMap cleanup on death ===
