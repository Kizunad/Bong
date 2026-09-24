#![allow(dead_code, unused_imports)]
use super::*;
use crate::combat::body_conditioning::apply_guangbo_ticao_bonuses;
use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem};
use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};
use crate::cultivation::meridian::severed::{
    MeridianSeveredPermanent, SeveredSource, SkillMeridianDependencies,
};
use crate::npc::lifecycle::NpcArchetype;

fn registry() -> &'static TechniqueRegistry {
    static REGISTRY: std::sync::OnceLock<TechniqueRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(TechniqueRegistry::load_for_tests)
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
    super::assign_npc_techniques(
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
    super::select_technique(
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
    super::has_usable_heal_technique(
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
fn assignment_filters_dispatch_and_uses_injected_race_identity() {
    use crate::body_plan::RaceGateOwned;

    let registry =
        TechniqueRegistry::load_for_tests_with_override("npc.heal_basic", |definition| {
            definition.required_race = RaceGateOwned::Species {
                species: vec![RaceId::new("whale")],
            };
        });
    let sys = full_regular_meridians();
    let deps = empty_deps();

    let human = super::assign_npc_techniques(
        &registry,
        NpcArchetype::Rogue,
        Realm::Induce,
        &sys,
        &deps,
        None,
        42,
    );
    assert!(
        human
            .entries
            .iter()
            .all(|entry| entry.id != "npc.heal_basic"),
        "human wrapper must reject whale-only metadata"
    );

    let whale = super::assign_npc_techniques_for_identity(
        &registry,
        NpcArchetype::Rogue,
        Realm::Induce,
        &sys,
        &deps,
        None,
        42,
        &RaceId::new("whale"),
        false,
    );
    assert!(
        whale
            .entries
            .iter()
            .any(|entry| entry.id == "npc.heal_basic"),
        "matching whale identity must admit whale-only metadata"
    );

    // DirectGeneric is a player cast-route classification, not an NPC active-candidate
    // filter. Active loadouts must contain only resolver-backed entries; the specialized
    // body-conditioning passive is retained through its separate passive path.
    let assigned = super::assign_npc_techniques(
        &registry,
        NpcArchetype::Rogue,
        Realm::Awaken,
        &full_regular_meridians(),
        &deps,
        None,
        6,
    );
    assert!(
        assigned.entries.iter().all(|entry| {
            registry.get(&entry.id).is_some_and(|definition| {
                definition.dispatch == TechniqueDispatch::MetadataBacked
                    || entry.id == "body.guangbo_ticao"
            })
        }),
        "NPC active loadout must exclude resolver-less direct_generic entries: {:?}",
        assigned.entries
    );
    let guangbo = assigned
        .entries
        .iter()
        .find(|entry| entry.id == "body.guangbo_ticao")
        .expect("legacy same-seed NPC oracle must retain body.guangbo_ticao");
    let mut attrs = crate::combat::components::DerivedAttrs::default();
    apply_guangbo_ticao_bonuses(
        &mut attrs,
        &KnownTechniques {
            entries: vec![guangbo.clone()],
        },
    );
    assert!(
        attrs.move_speed_multiplier > 1.0 && attrs.jump_height_multiplier > 1.0,
        "body.guangbo_ticao must remain a live NPC passive, attrs={attrs:?}"
    );
}

#[test]
fn assignment_excludes_resolverless_direct_generic_from_active_pool_but_keeps_passive() {
    let registry = TechniqueRegistry::load_for_tests_with_definition(TechniqueDefinition {
        id: "test.direct_generic_noop".to_string(),
        display_name: "无消费者探针".to_string(),
        grade: "common".to_string(),
        description: "NPC 不得主动选择 resolver-less direct_generic".to_string(),
        required_realm: "Awaken".to_string(),
        required_meridians: Vec::new(),
        required_race: crate::body_plan::RaceGateOwned::Any,
        qi_cost: 0.0,
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 1,
        range: 1.0,
        icon_texture: "bong-client:textures/gui/items/skill_scroll_sword_cleave.png".to_string(),
        category: SkillCategory::Attack,
        dispatch: TechniqueDispatch::DirectGeneric,
    });
    let assigned = super::assign_npc_techniques(
        &registry,
        NpcArchetype::Rogue,
        Realm::Awaken,
        &full_regular_meridians(),
        &empty_deps(),
        None,
        6,
    );
    assert!(
        assigned
            .entries
            .iter()
            .all(|entry| entry.id != "test.direct_generic_noop"),
        "resolver-less direct_generic must never enter the NPC active loadout: {:?}",
        assigned.entries
    );
    assert!(
        assigned
            .entries
            .iter()
            .any(|entry| entry.id == "body.guangbo_ticao"),
        "the specialized body.guangbo_ticao passive must remain available through its passive path"
    );
}

// === assign_npc_techniques: realm gating ===

#[test]
fn selection_uses_injected_registry_cost() {
    let registry = TechniqueRegistry::load_for_tests_with_override("sword.cleave", |definition| {
        definition.qi_cost = 101.0
    });
    let known = KnownTechniques {
        entries: vec![KnownTechnique {
            id: "sword.cleave".to_string(),
            proficiency: 1.0,
            active: true,
        }],
    };
    let cultivation = Cultivation {
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    };

    let result = super::select_technique(
        &registry,
        &known,
        &cultivation,
        &empty_deps(),
        None,
        None,
        &NpcCooldownMap::default(),
        Entity::from_raw(1),
        3.0,
        100,
        &default_ctx(),
        None,
    );

    assert!(
        result.is_none(),
        "injected unaffordable qi_cost must exclude the sole candidate"
    );
}

#[test]
fn meridian_deps_satisfied_no_deps() {
    let def = technique_definition_for_test("sword.cleave").unwrap();
    let sys = MeridianSystem::default();
    let deps = empty_deps();
    assert!(
        meridian_deps_satisfied(&def, &sys, &deps),
        "technique with no meridian deps should pass"
    );
}

#[test]
fn meridian_deps_satisfied_with_opened() {
    let def = technique_definition_for_test("zhenmai.parry").unwrap(); // requires Lung
    let sys = meridian_sys_with_opened(&[MeridianId::Lung]);
    let deps = empty_deps();
    assert!(
        meridian_deps_satisfied(&def, &sys, &deps),
        "technique with opened required meridian should pass"
    );
}

#[test]
fn meridian_deps_not_satisfied_when_closed() {
    let def = technique_definition_for_test("zhenmai.parry").unwrap(); // requires Lung
    let sys = MeridianSystem::default(); // all closed
    let deps = empty_deps();
    assert!(
        !meridian_deps_satisfied(&def, &sys, &deps),
        "technique with closed required meridian should fail"
    );
}

// === assign_npc_techniques: all archetypes x all realms ===

#[test]
fn cooldown_map_remove_all_for_clears_entity_entries() {
    let mut map = NpcCooldownMap::default();
    let npc_a = Entity::from_raw(10);
    let npc_b = Entity::from_raw(20);
    map.set(npc_a, "sword.cleave", 200);
    map.set(npc_a, "sword.thrust", 300);
    map.set(npc_b, "woliu.burst", 200);

    assert_eq!(map.len(), 3, "should have 3 entries before cleanup");

    map.remove_all_for(npc_a);

    assert_eq!(
        map.len(),
        1,
        "only npc_b's entry should remain after removing npc_a"
    );
    assert!(
        !map.is_on_cooldown(npc_a, "sword.cleave", 100),
        "npc_a cleave cooldown should be removed"
    );
    assert!(
        !map.is_on_cooldown(npc_a, "sword.thrust", 100),
        "npc_a thrust cooldown should be removed"
    );
    assert!(
        map.is_on_cooldown(npc_b, "woliu.burst", 100),
        "npc_b burst cooldown should remain intact"
    );
}

#[test]
fn cooldown_map_remove_all_for_noop_on_unknown_entity() {
    let mut map = NpcCooldownMap::default();
    let npc_a = Entity::from_raw(10);
    let npc_b = Entity::from_raw(20);
    map.set(npc_a, "sword.cleave", 200);

    map.remove_all_for(npc_b);

    assert_eq!(
        map.len(),
        1,
        "removing unknown entity should not affect existing entries"
    );
}

#[test]
fn action_falls_back_when_only_resolverless_direct_generic_is_known() {
    let registry = TechniqueRegistry::load_for_tests_with_definition(TechniqueDefinition {
        id: "test.direct_generic_noop".to_string(),
        display_name: "无消费者探针".to_string(),
        grade: "common".to_string(),
        description: "NPC action must fail rather than consume a melee turn".to_string(),
        required_realm: "Awaken".to_string(),
        required_meridians: Vec::new(),
        required_race: crate::body_plan::RaceGateOwned::Any,
        qi_cost: 0.0,
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 1,
        range: 10.0,
        icon_texture: "bong-client:textures/gui/items/skill_scroll_sword_cleave.png".to_string(),
        category: SkillCategory::Attack,
        dispatch: TechniqueDispatch::DirectGeneric,
    });
    let mut world = valence::prelude::bevy_ecs::world::World::new();
    world.insert_resource(registry);
    world.insert_resource(NpcCooldownMap::default());
    let actor = world
        .spawn((
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: "test.direct_generic_noop".to_string(),
                    proficiency: 1.0,
                    active: true,
                }],
            },
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
        ))
        .id();
    let action = world
        .spawn((
            NpcTechniqueAction,
            Actor(actor),
            big_brain::prelude::ActionState::Requested,
        ))
        .id();

    super::run_technique_action::<NpcTechniqueAction>(&mut world, None);

    assert_eq!(
        world.get::<big_brain::prelude::ActionState>(action),
        Some(&big_brain::prelude::ActionState::Failure),
        "resolver-less direct_generic must fail the NPC action and leave melee fallback available"
    );
}

#[test]
fn technique_scorer_ignores_resolverless_direct_generic_entries() {
    use big_brain::prelude::{Actor, Score};
    use valence::prelude::{App, Update};

    let registry = TechniqueRegistry::load_for_tests_with_definition(TechniqueDefinition {
        id: "test.direct_generic_noop".to_string(),
        display_name: "无消费者探针".to_string(),
        grade: "common".to_string(),
        description: "NPC scorer must ignore resolver-less direct_generic".to_string(),
        required_realm: "Awaken".to_string(),
        required_meridians: Vec::new(),
        required_race: crate::body_plan::RaceGateOwned::Any,
        qi_cost: 0.0,
        stamina_cost: 0.0,
        cast_ticks: 1,
        cooldown_ticks: 1,
        range: 10.0,
        icon_texture: "bong-client:textures/gui/items/skill_scroll_sword_cleave.png".to_string(),
        category: SkillCategory::Attack,
        dispatch: TechniqueDispatch::DirectGeneric,
    });
    let mut app = App::new();
    app.insert_resource(registry);
    app.insert_resource(NpcCooldownMap::default());
    app.insert_resource(SkillMeridianDependencies::default());
    app.insert_resource(crate::cultivation::tick::CultivationClock { tick: 100 });
    app.insert_resource(crate::npc::lod::NpcLodConfig::default());
    app.insert_resource(crate::npc::lod::NpcLodTick(0));
    app.add_systems(Update, npc_technique_scorer_system);

    let target = app.world_mut().spawn_empty().id();
    let npc = app
        .world_mut()
        .spawn((
            NpcBlackboard {
                nearest_player: Some(target),
                player_distance: 3.0,
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: "test.direct_generic_noop".to_string(),
                    proficiency: 1.0,
                    active: true,
                }],
            },
            crate::npc::spawn::NpcMarker,
            crate::npc::lod::NpcLodTier::Near,
        ))
        .id();
    let scorer_entity = app
        .world_mut()
        .spawn((NpcTechniqueScorer, Actor(npc), Score::default()))
        .id();

    app.update();

    let score = app
        .world()
        .get::<Score>(scorer_entity)
        .map(|score| score.get())
        .unwrap();
    assert_eq!(
        score, 0.0,
        "resolver-less direct_generic must not produce an active NPC technique score"
    );
}

// ─── M33：通用 scorer system 必须消费注入 registry 的 override 值 ───────────
//
// 若 npc_technique_scorer_system 改读默认/静态 catalog 的 qi_cost，override 后
// 的通用功法会错误通过 affordability gate。

#[test]
fn technique_scorer_system_follows_injected_registry_affordability() {
    use big_brain::prelude::{Actor, Score};
    use valence::prelude::{App, Update};

    let mut app = App::new();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        "sword.cleave",
        |definition| {
            definition.qi_cost = 101.0;
        },
    ));
    app.insert_resource(NpcCooldownMap::default());
    app.insert_resource(SkillMeridianDependencies::default());
    app.insert_resource(crate::cultivation::tick::CultivationClock { tick: 100 });
    app.insert_resource(crate::npc::lod::NpcLodConfig::default());
    app.insert_resource(crate::npc::lod::NpcLodTick(0));
    app.add_systems(Update, npc_technique_scorer_system);

    let target = app.world_mut().spawn_empty().id();
    let npc = app
        .world_mut()
        .spawn((
            NpcBlackboard {
                nearest_player: Some(target),
                player_distance: 3.0,
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: "sword.cleave".to_string(),
                    proficiency: 1.0,
                    active: true,
                }],
            },
            crate::npc::spawn::NpcMarker,
            crate::npc::lod::NpcLodTier::Near,
        ))
        .id();
    let scorer_entity = app
        .world_mut()
        .spawn((NpcTechniqueScorer, Actor(npc), Score::default()))
        .id();

    app.update();

    let score = app
        .world()
        .get::<Score>(scorer_entity)
        .map(|score| score.get())
        .unwrap();
    assert_eq!(
        score, 0.0,
        "通用 scorer 必须消费注入的 101 qi_cost 并拒绝 qi=100 的唯一候选"
    );

    let mut app = App::new();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        "sword.cleave",
        |definition| {
            definition.qi_cost = 1.0;
        },
    ));
    app.insert_resource(NpcCooldownMap::default());
    app.insert_resource(SkillMeridianDependencies::default());
    app.insert_resource(crate::cultivation::tick::CultivationClock { tick: 100 });
    app.insert_resource(crate::npc::lod::NpcLodConfig::default());
    app.insert_resource(crate::npc::lod::NpcLodTick(0));
    app.add_systems(Update, npc_technique_scorer_system);

    let target = app.world_mut().spawn_empty().id();
    let npc = app
        .world_mut()
        .spawn((
            NpcBlackboard {
                nearest_player: Some(target),
                player_distance: 3.0,
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: "sword.cleave".to_string(),
                    proficiency: 1.0,
                    active: true,
                }],
            },
            crate::npc::spawn::NpcMarker,
            crate::npc::lod::NpcLodTier::Near,
        ))
        .id();
    let scorer_entity = app
        .world_mut()
        .spawn((NpcTechniqueScorer, Actor(npc), Score::default()))
        .id();

    app.update();

    let score = app
        .world()
        .get::<Score>(scorer_entity)
        .map(|score| score.get())
        .unwrap();
    assert_eq!(
        score, 0.85,
        "通用 scorer 必须跟随注入的 1 qi_cost，选择可负担的唯一候选"
    );
}

// ─── M33：heal scorer system 必须消费注入 registry 的 override 值 ───────────
//
// 若 npc_heal_scorer_system 改读默认/静态 catalog 的 qi_cost，override 后
// heal 成本高于 NPC 当前 qi 时 scorer 会错误 emit 0.9（低血量 NPC 选不可负担
// 的 heal action）。直接调 select_technique 无法探测——scorer 自带 registry
// 查找/category/cooldown/meridian/affordability 过滤，这里用完整 bevy system
// 跑一遍证明生产路径消费注入 registry。

#[test]
fn heal_scorer_system_follows_injected_registry_affordability() {
    use big_brain::prelude::{Actor, Score};
    use valence::prelude::{App, Update};

    // checked-in npc.heal_basic 是 Awaken/Heal/低 qi_cost；override 成 500 qi_cost，
    // 任何 NPC（这里 100 qi）都负担不起 → scorer 必须给 0.0。
    let mut app = App::new();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        "npc.heal_basic",
        |definition| {
            definition.qi_cost = 500.0;
        },
    ));
    app.insert_resource(NpcCooldownMap::default());
    app.insert_resource(SkillMeridianDependencies::default());
    app.insert_resource(crate::cultivation::tick::CultivationClock { tick: 100 });
    app.insert_resource(crate::npc::lod::NpcLodConfig::default());
    app.insert_resource(crate::npc::lod::NpcLodTick(0));
    app.add_systems(Update, npc_heal_scorer_system);

    // NPC：低血量（health 10/100）且知道 npc.heal_basic——若 scorer 读默认
    // catalog（成本低）就会给 0.9。
    let npc = app
        .world_mut()
        .spawn((
            NpcBlackboard::default(),
            Cultivation {
                realm: Realm::Condense,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: "npc.heal_basic".to_string(),
                    proficiency: 1.0,
                    active: true,
                }],
            },
            crate::combat::components::Wounds {
                entries: Vec::new(),
                health_current: 10.0,
                health_max: 100.0,
            },
            crate::npc::spawn::NpcMarker,
            crate::npc::lod::NpcLodTier::Near,
        ))
        .id();

    // big-brain scorer entity：挂 NpcHealScorer + Actor(npc) + Score。
    let scorer_entity = app
        .world_mut()
        .spawn((NpcHealScorer, Actor(npc), Score::default()))
        .id();
    let _ = scorer_entity;

    app.update();

    let score = app
        .world()
        .get::<Score>(scorer_entity)
        .map(|score| score.get())
        .unwrap();
    assert_eq!(
        score, 0.0,
        "scorer must see the injected 500 qi_cost and reject the unaffordable heal (M33)"
    );

    // 对照组：override 成低成本（1.0）→ 低血量 NPC 必须得到 0.9，证明
    // scorer 正向跟随注入 registry 而非恒 0。
    let mut app = App::new();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        "npc.heal_basic",
        |definition| {
            definition.qi_cost = 1.0;
        },
    ));
    app.insert_resource(NpcCooldownMap::default());
    app.insert_resource(SkillMeridianDependencies::default());
    app.insert_resource(crate::cultivation::tick::CultivationClock { tick: 100 });
    app.insert_resource(crate::npc::lod::NpcLodConfig::default());
    app.insert_resource(crate::npc::lod::NpcLodTick(0));
    app.add_systems(Update, npc_heal_scorer_system);

    let npc = app
        .world_mut()
        .spawn((
            NpcBlackboard::default(),
            Cultivation {
                realm: Realm::Condense,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: "npc.heal_basic".to_string(),
                    proficiency: 1.0,
                    active: true,
                }],
            },
            crate::combat::components::Wounds {
                entries: Vec::new(),
                health_current: 10.0,
                health_max: 100.0,
            },
            crate::npc::spawn::NpcMarker,
            crate::npc::lod::NpcLodTier::Near,
        ))
        .id();
    let scorer_entity = app
        .world_mut()
        .spawn((NpcHealScorer, Actor(npc), Score::default()))
        .id();

    app.update();

    let score = app
        .world()
        .get::<Score>(scorer_entity)
        .map(|score| score.get())
        .unwrap();
    assert_eq!(
        score, 0.9,
        "scorer must admit an affordable heal when override lowers cost (M33)"
    );
}
