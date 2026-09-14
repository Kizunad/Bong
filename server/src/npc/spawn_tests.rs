use super::*;
use crate::combat::components::StatusEffects;
use crate::combat::events::AttackIntent;
use crate::npc::brain;
use crate::npc::lifecycle::NpcLifespan;
use crate::npc::movement::GameTick;
use big_brain::prelude::{BigBrainPlugin, HasThinker, ThinkerBuilder};
use std::collections::HashMap;
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Commands, DVec3, Entity, EntityKind, EntityLayerId, EventReader, Position,
    PreUpdate, Res, Resource, Update,
};

use crate::combat::components::WoundKind;
use crate::combat::events::FIST_REACH;
use crate::cultivation::components::{Cultivation, Realm};
use crate::npc::faction::{FactionId, FactionMembership, FactionRank, MissionExecuteState};
use crate::npc::movement::MovementCapabilities;
use crate::npc::patrol::NpcPatrol;
use crate::npc::social::SocializeState;
use crate::npc::territory::{HuntState, ProtectYoungState, Territory, TerritoryPatrolState};
use crate::skin::{NpcPlayerSkin, NpcSkinFallbackPolicy, SignedSkin};
use crate::world::zone::{Zone, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

use super::common::DeferredNpcBrain;
use super::common::{
    fallback_rogue_commoner_kind, NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype,
    NpcMeleeProfile, NpcSkinSpawnContext,
};
use super::rogue::{
    classify_zones_by_qi, distribute_counts_evenly, initial_age_for_index,
    RoguePopulationSeedConfig,
};

use crate::npc::lifecycle::NpcRegistry;
use crate::npc::relic::{GuardState, GuardianDuty, TrialEval, TrialState};
use bevy_transform::components::{GlobalTransform, Transform};

#[derive(Clone, Copy, Resource)]
struct TestLayer(Entity);

#[derive(Default)]
struct CapturedAttackIntents(Vec<AttackIntent>);

impl Resource for CapturedAttackIntents {}

fn setup_test_layer(mut commands: Commands) {
    let layer = commands.spawn_empty().id();
    commands.insert_resource(TestLayer(layer));
}

fn spawn_test_npc(mut commands: Commands, layer: Res<TestLayer>) {
    zombie::spawn_single_zombie_npc(&mut commands, layer.0);
}

fn capture_attack_intents(
    mut events: EventReader<AttackIntent>,
    mut captured: valence::prelude::ResMut<CapturedAttackIntents>,
) {
    captured.0.extend(events.read().cloned());
}

#[test]
fn spawn_npc_creates_single_zombie_with_expected_components() {
    let mut app = App::new();
    app.add_plugins(BigBrainPlugin::new(PreUpdate));
    app.add_systems(
        valence::prelude::Startup,
        (setup_test_layer, spawn_test_npc.after(setup_test_layer)),
    );

    app.update();
    app.update();

    let npc_entities = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).collect::<Vec<_>>()
    };

    assert_eq!(
        npc_entities.len(),
        1,
        "expected exactly one NPC marker entity"
    );

    let npc_entity = npc_entities[0];

    let kind = app
        .world()
        .get::<EntityKind>(npc_entity)
        .expect("NPC should have EntityKind component");
    assert_eq!(*kind, EntityKind::ZOMBIE);

    let position = app
        .world()
        .get::<Position>(npc_entity)
        .expect("NPC should have Position component");
    assert_eq!(position.get(), DVec3::new(14.0, 66.0, 14.0));

    let transform = app
        .world()
        .get::<Transform>(npc_entity)
        .expect("NPC should have Transform component");
    assert_eq!(transform.translation.x, 14.0);
    assert_eq!(transform.translation.y, 66.0);
    assert_eq!(transform.translation.z, 14.0);

    let _global_transform = app
        .world()
        .get::<GlobalTransform>(npc_entity)
        .expect("NPC should have GlobalTransform component");

    let blackboard = app
        .world()
        .get::<NpcBlackboard>(npc_entity)
        .expect("NPC should have NpcBlackboard component");
    assert_eq!(blackboard.nearest_player, None);
    assert!(
        blackboard.player_distance.is_infinite(),
        "NpcBlackboard.player_distance should default to infinity"
    );

    let archetype = app
        .world()
        .get::<NpcMeleeArchetype>(npc_entity)
        .expect("NPC should have NpcMeleeArchetype component");
    let loadout = app
        .world()
        .get::<NpcCombatLoadout>(npc_entity)
        .expect("NPC should have NpcCombatLoadout component");
    let profile = app
        .world()
        .get::<NpcMeleeProfile>(npc_entity)
        .expect("NPC should have NpcMeleeProfile component");
    let capabilities = app
        .world()
        .get::<MovementCapabilities>(npc_entity)
        .expect("NPC should have MovementCapabilities component");
    let _status_effects = app
        .world()
        .get::<StatusEffects>(npc_entity)
        .expect("NPC should include StatusEffects for shared combat resolver");
    assert_eq!(
        loadout.melee_archetype,
        NpcCombatLoadout::default().melee_archetype
    );
    assert_eq!(
        loadout.movement_capabilities.can_sprint,
        NpcCombatLoadout::default().movement_capabilities.can_sprint
    );
    assert_eq!(
        loadout.movement_capabilities.can_dash,
        NpcCombatLoadout::default().movement_capabilities.can_dash
    );
    assert_eq!(*archetype, NpcMeleeArchetype::Brawler);
    assert_eq!(*profile, NpcMeleeArchetype::Brawler.profile());
    assert_eq!(profile.wound_kind, WoundKind::Blunt);
    assert_eq!(
        capabilities.can_sprint,
        NpcCombatLoadout::default().movement_capabilities.can_sprint
    );
    assert_eq!(
        capabilities.can_dash,
        NpcCombatLoadout::default().movement_capabilities.can_dash
    );

    let patrol = app
        .world()
        .get::<NpcPatrol>(npc_entity)
        .expect("NPC should have a patrol component");
    assert_eq!(patrol.home_zone, DEFAULT_SPAWN_ZONE_NAME);
    assert_eq!(patrol.current_target, DVec3::new(14.0, 66.0, 14.0));

    let layer_id = app
        .world()
        .get::<EntityLayerId>(npc_entity)
        .expect("NPC should have EntityLayerId component");
    assert_ne!(
        layer_id.0,
        Entity::PLACEHOLDER,
        "NPC should be assigned to a non-placeholder layer"
    );

    let _thinker_builder = app
        .world()
        .get::<ThinkerBuilder>(npc_entity)
        .expect("NPC should have a Thinker builder attached at spawn time");

    let npc_archetype = app
        .world()
        .get::<NpcArchetype>(npc_entity)
        .expect("NPC should include shared NpcArchetype component");
    assert_eq!(*npc_archetype, NpcArchetype::Zombie);

    let lifespan = app
        .world()
        .get::<NpcLifespan>(npc_entity)
        .expect("NPC should include shared lifespan component");
    assert_eq!(lifespan.age_ticks, 0.0);
    assert!(lifespan.max_age_ticks > 0.0);

    let has_thinker = app
        .world()
        .get::<HasThinker>(npc_entity)
        .expect("BigBrain should attach HasThinker to NPC");

    let _thinker = app
        .world()
        .get::<big_brain::prelude::Thinker>(has_thinker.entity())
        .expect("BigBrain thinker entity should contain Thinker component");
}

#[test]
fn startup_spawned_npc_default_thinker_emits_attack_intent_in_melee_range() {
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.insert_resource(crate::qi_physics::WorldQiAccount::default());
    app.add_event::<crate::qi_physics::QiTransfer>();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    crate::npc::lifecycle::register(&mut app);
    brain::register(&mut app);
    app.insert_resource(CapturedAttackIntents::default());
    app.insert_resource(GameTick(120));
    app.add_event::<AttackIntent>();
    app.add_systems(Update, capture_attack_intents);
    app.add_systems(
        valence::prelude::Startup,
        (setup_test_layer, spawn_test_npc.after(setup_test_layer)),
    );

    let player = app
        .world_mut()
        .spawn((ClientMarker, Position::new([14.8, 66.0, 14.0])))
        .id();

    for _ in 0..5 {
        app.update();
    }

    let captured = &app.world().resource::<CapturedAttackIntents>().0;
    assert!(
        !captured.is_empty(),
        "default startup NPC thinker should emit AttackIntent when a player enters melee range"
    );
    assert_eq!(captured[0].target, Some(player));
    assert_eq!(captured[0].reach, FIST_REACH);
    assert_eq!(captured[0].wound_kind, WoundKind::Blunt);
}

#[test]
fn spawn_commoner_npc_at_attaches_commoner_components() {
    let mut app = App::new();
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_commoner.after(setup_test_layer),
        ),
    );

    app.update();
    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).collect::<Vec<_>>()
    };
    assert_eq!(npcs.len(), 1);
    let npc = npcs[0];

    let archetype = *app.world().get::<NpcArchetype>(npc).unwrap();
    assert_eq!(archetype, NpcArchetype::Commoner);

    let kind = *app.world().get::<EntityKind>(npc).unwrap();
    assert_eq!(kind, EntityKind::VILLAGER);

    let hunger = *app
        .world()
        .get::<crate::npc::hunger::Hunger>(npc)
        .expect("commoner should have Hunger");
    assert_eq!(hunger.value, 1.0);

    let wander = *app
        .world()
        .get::<crate::npc::brain::WanderState>(npc)
        .expect("commoner should have WanderState");
    assert!(wander.destination.is_none());

    let lifespan = *app.world().get::<NpcLifespan>(npc).unwrap();
    assert_eq!(lifespan.age_ticks, 2.0);
    assert!(lifespan.max_age_ticks > 0.0);
}

fn spawn_test_commoner(mut commands: Commands, layer: Res<TestLayer>) {
    commoner::spawn_commoner_npc_at(
        &mut commands,
        NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
        layer.0,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(20.0, 66.0, 20.0),
        DVec3::new(20.0, 66.0, 20.0),
        Realm::Awaken,
        2.0,
    );
}

#[test]
fn spawn_rogue_npc_at_attaches_rogue_components() {
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.add_systems(
        valence::prelude::Startup,
        (setup_test_layer, spawn_test_rogue.after(setup_test_layer)),
    );

    app.update();
    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).collect::<Vec<_>>()
    };
    assert_eq!(npcs.len(), 1);
    let npc = npcs[0];

    assert_eq!(
        *app.world().get::<NpcArchetype>(npc).unwrap(),
        NpcArchetype::Rogue
    );

    assert!(
        app.world()
            .get::<crate::npc::brain::CultivateState>(npc)
            .is_some(),
        "rogue should carry CultivateState"
    );

    let lifespan = *app.world().get::<NpcLifespan>(npc).unwrap();
    assert_eq!(
        lifespan.max_age_ticks,
        NpcArchetype::Rogue.default_max_age_ticks()
    );
}

#[test]
fn deferred_seed_brain_attaches_when_lod_wakes() {
    let mut app = App::new();
    app.add_systems(PreUpdate, attach_deferred_npc_brain_system);
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            NpcLodTier::Far,
            DeferredNpcBrain::ScatteredCultivator,
        ))
        .id();

    app.update();

    assert!(app.world().get::<ThinkerBuilder>(npc).is_some());
    assert!(app.world().get::<DeferredNpcBrain>(npc).is_none());
}

#[test]
fn deferred_seed_brain_stays_detached_while_dormant() {
    let mut app = App::new();
    app.add_systems(PreUpdate, attach_deferred_npc_brain_system);
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            NpcLodTier::Dormant,
            DeferredNpcBrain::ScatteredCultivator,
        ))
        .id();

    app.update();

    assert!(app.world().get::<ThinkerBuilder>(npc).is_none());
    assert!(app.world().get::<DeferredNpcBrain>(npc).is_some());
}

// valence `despawn_marked_entities` 的等价复刻（同在 `Last`，真正移除 `Despawned` 实体）。
// 用于把 B0003 竞态的两侧（big_brain attach + valence despawn）拼到测试 app 里。
fn despawn_marked_entities_clone(marked: Query<Entity, With<Despawned>>, mut commands: Commands) {
    for entity in &marked {
        commands.entity(entity).despawn();
    }
}

// probe：与 big_brain 的 `thinker_component_attach_system` 同在 `BigBrainSet::Cleanup`。
// 累加「进入 Cleanup 时仍可被 attach query 命中的将死实体」数量，用于断言 strip 已在
// Cleanup 之前把 `ThinkerBuilder` 清掉。
#[derive(valence::prelude::Resource, Default)]
struct BareThinkerSeenInCleanup(usize);

fn probe_bare_thinker_in_cleanup(
    mut seen: valence::prelude::ResMut<BareThinkerSeenInCleanup>,
    bare: Query<Entity, DespawningBareThinkerFilter>,
) {
    seen.0 += bare.iter().count();
}

#[test]
fn strip_removal_visible_before_big_brain_cleanup_query() {
    // 确定性回归锁（回应 Pi review 对「Commands 延迟生效 / 单线程虚假信心」的质疑）：
    // strip 是 exclusive &mut World 系统，移除立即生效；probe 注册在
    // .in_set(BigBrainSet::Cleanup)（与 thinker_component_attach_system 同一调度点），
    // 进入 Cleanup 时不得再 query 到「Despawned + ThinkerBuilder + 无 HasThinker」实体——
    // 否则 attach query 命中将死实体 → insert(HasThinker) → B0003。结论与线程数无关。
    let mut app = App::new();
    app.add_plugins(BigBrainPlugin::new(PreUpdate));
    app.init_resource::<BareThinkerSeenInCleanup>();
    app.add_systems(
        Last,
        strip_brain_from_despawning_npcs_system.before(BigBrainSet::Cleanup),
    );
    app.add_systems(
        Last,
        probe_bare_thinker_in_cleanup.in_set(BigBrainSet::Cleanup),
    );

    app.world_mut().spawn((
        NpcMarker,
        Despawned,
        DeferredNpcBrain::ScatteredCultivator.build(),
    ));

    app.update();

    assert_eq!(
            app.world().resource::<BareThinkerSeenInCleanup>().0,
            0,
            "进入 BigBrainSet::Cleanup 时 strip 必须已移除将死 NPC 的 ThinkerBuilder（B0003 前置态须归零）"
        );
}

#[test]
fn strip_removes_thinker_builder_from_despawning_npc_without_thinker() {
    // 崩溃前置态：NPC 同 tick 被唤醒拿到 ThinkerBuilder + 被标 Despawned，但 big_brain 未 attach。
    let mut app = App::new();
    app.add_systems(Update, strip_brain_from_despawning_npcs_system);
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            Despawned,
            DeferredNpcBrain::ScatteredCultivator.build(),
        ))
        .id();

    app.update();

    assert!(
        app.world().get::<ThinkerBuilder>(npc).is_none(),
        "strip 必须移除将死 NPC（Despawned + 无 HasThinker）的 ThinkerBuilder，否则 big_brain \
             attach 会对已 despawn 的 entity insert HasThinker → Bevy B0003 panic"
    );
}

#[test]
fn strip_keeps_thinker_builder_on_live_npc() {
    // 活体 NPC（未标 Despawned）的 ThinkerBuilder 必须保留给 big_brain 正常 attach。
    let mut app = App::new();
    app.add_systems(Update, strip_brain_from_despawning_npcs_system);
    let npc = app
        .world_mut()
        .spawn((NpcMarker, DeferredNpcBrain::ScatteredCultivator.build()))
        .id();

    app.update();

    assert!(
        app.world().get::<ThinkerBuilder>(npc).is_some(),
        "strip 只针对 Despawned 实体：活体 NPC 的 ThinkerBuilder 不应被动到"
    );
}

#[test]
fn strip_leaves_already_attached_despawning_npc_for_big_brain_cleanup() {
    // 已 attach（持 HasThinker）的将死 NPC 不归 strip 管——该路径由 big_brain actor_gone_cleanup
    // 处理。用真实 big_brain 跑出 HasThinker 后再标 Despawned，验证 strip 的 Without<HasThinker>
    // 守卫跳过它、ThinkerBuilder 保留。
    let mut app = App::new();
    app.add_plugins(BigBrainPlugin::new(PreUpdate));
    app.add_systems(
        Last,
        strip_brain_from_despawning_npcs_system.before(BigBrainSet::Cleanup),
    );
    let npc = app
        .world_mut()
        .spawn((NpcMarker, DeferredNpcBrain::ScatteredCultivator.build()))
        .id();
    app.update();
    assert!(
        app.world().get::<HasThinker>(npc).is_some(),
        "前置：big_brain 应已在 Last 把 HasThinker attach 上"
    );

    app.world_mut().entity_mut(npc).insert(Despawned);
    app.update();

    assert!(
        app.world().get::<ThinkerBuilder>(npc).is_some(),
        "strip 不应剥掉已 attach NPC 的 ThinkerBuilder（Without<HasThinker> 守卫）"
    );
}

#[test]
fn despawning_woken_npc_does_not_panic_big_brain_attach_b0003() {
    // 生产 wiring 复刻 + 回归锁：big_brain cleanup（含 thinker_component_attach_system）与
    // valence 风格 despawn 同在 `Last`，strip 注册在 `.before(BigBrainSet::Cleanup)`。NPC 处于
    // 「ThinkerBuilder + Despawned + 无 HasThinker」崩溃前置态——strip 在 Cleanup 之前剥掉
    // ThinkerBuilder，big_brain attach 不再命中将死实体，整帧不得 panic，且实体被正常移除。
    let mut app = App::new();
    app.add_plugins(BigBrainPlugin::new(PreUpdate));
    app.add_systems(
        Last,
        strip_brain_from_despawning_npcs_system.before(BigBrainSet::Cleanup),
    );
    app.add_systems(Last, despawn_marked_entities_clone);

    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            Despawned,
            DeferredNpcBrain::ScatteredCultivator.build(),
        ))
        .id();

    app.update(); // 修复前此处 B0003 panic 整服崩溃

    assert!(
        app.world().get_entity(npc).is_none(),
        "将死 NPC 应在 Last 被 valence 风格 despawn 正常移除，且过程不触发 B0003"
    );
}

#[test]
fn spawn_scattered_cultivator_at_attaches_farming_brain_components() {
    let mut app = App::new();
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_scattered_cultivator.after(setup_test_layer),
        ),
    );

    app.update();
    app.update();

    let npc = only_spawned_npc(&mut app);

    assert_eq!(
        *app.world().get::<NpcArchetype>(npc).unwrap(),
        NpcArchetype::Rogue
    );
    let scattered = app
        .world()
        .get::<crate::npc::scattered_cultivator::ScatteredCultivator>(npc)
        .expect("scattered cultivator should mark seeded Rogue NPCs");
    assert_eq!(scattered.home_plot, None);
    assert_eq!(scattered.fail_streak, 0);
    assert!(matches!(
        scattered.temperament,
        crate::npc::scattered_cultivator::FarmingTemperament::Patient
            | crate::npc::scattered_cultivator::FarmingTemperament::Greedy
            | crate::npc::scattered_cultivator::FarmingTemperament::Anxious
            | crate::npc::scattered_cultivator::FarmingTemperament::Aggressive
    ));
    assert!(
        app.world().get::<ThinkerBuilder>(npc).is_some(),
        "scattered cultivator should carry a live farming thinker"
    );
    assert!(
        app.world()
            .get::<crate::npc::brain::CultivateState>(npc)
            .is_some(),
        "scattered cultivator remains a cultivating Rogue"
    );
    let trade = app
        .world()
        .get::<crate::npc::trade::NpcTradeInventory>(npc)
        .expect("scattered Rogue must expose the same trade inventory as seeded Rogue");
    assert!(
        !trade.offers.is_empty(),
        "Awaken Rogue fixture should have trade offers"
    );
}

#[test]
fn rogue_commoner_visual_kind_uses_player_only_for_real_skin() {
    assert_eq!(
        fallback_rogue_commoner_kind(&None),
        EntityKind::VILLAGER,
        "None skin should produce villager (neutral NPC model)",
    );
    assert_eq!(
        fallback_rogue_commoner_kind(&Some(SignedSkin {
            value: "value".into(),
            signature: "sig".into(),
            source: crate::skin::SkinSource::MineSkinRandom {
                hash: "hash".into(),
            },
        })),
        EntityKind::PLAYER,
        "real MineSkin skin should produce player entity",
    );
}

fn spawn_test_rogue(
    mut commands: Commands,
    layer: Res<TestLayer>,
    technique_registry: Res<crate::cultivation::known_techniques::TechniqueRegistry>,
) {
    rogue::spawn_rogue_npc_at(
        &mut commands,
        &technique_registry,
        NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
        layer.0,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(18.0, 66.0, 18.0),
        DVec3::new(18.0, 66.0, 18.0),
        Realm::Awaken,
        0.0,
    );
}

fn spawn_test_scattered_cultivator(mut commands: Commands, layer: Res<TestLayer>) {
    rogue::spawn_scattered_cultivator_at(
        &mut commands,
        NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
        layer.0,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(19.0, 66.0, 19.0),
        DVec3::new(19.0, 66.0, 19.0),
        0.9,
        Realm::Awaken,
        0.0,
    );
}

#[test]
fn spawn_beast_npc_at_attaches_live_territory_brain_components() {
    let mut app = App::new();
    app.add_systems(
        valence::prelude::Startup,
        (setup_test_layer, spawn_test_beast.after(setup_test_layer)),
    );
    app.update();
    app.update();

    let beast = only_spawned_npc(&mut app);

    assert!(app.world().get::<TerritoryPatrolState>(beast).is_some());
    assert!(app.world().get::<HuntState>(beast).is_some());
    assert!(app.world().get::<ProtectYoungState>(beast).is_some());
    assert!(app
        .world()
        .get::<crate::fauna::components::FaunaTag>(beast)
        .is_some());
    let tag = app
        .world()
        .get::<crate::fauna::components::FaunaTag>(beast)
        .expect("beast should carry fauna tag");
    assert_eq!(
        app.world().get::<EntityKind>(beast),
        Some(&crate::fauna::visual::entity_kind_for_beast(tag.beast_kind)),
        "beast should spawn with a fauna custom visual entity kind"
    );
    assert_eq!(
        app.world()
            .get::<crate::fauna::visual::FaunaVisualKind>(beast)
            .copied(),
        crate::fauna::visual::visual_kind_for_beast(tag.beast_kind)
    );
    let _thinker = app
        .world()
        .get::<ThinkerBuilder>(beast)
        .expect("beast should carry the live territory thinker");

    let wounds = app
        .world()
        .get::<crate::combat::components::Wounds>(beast)
        .expect("beast should have wounds");
    let expected_hp = tag.beast_kind.health_max();
    assert_eq!(
        wounds.health_max, expected_hp,
        "beast {:?} health_max should be {} (per-kind), not default 100",
        tag.beast_kind, expected_hp
    );
    assert_eq!(
        wounds.health_current, expected_hp,
        "beast {:?} health_current should start at health_max",
        tag.beast_kind
    );
}

#[test]
fn spawn_disciple_npc_at_attaches_mission_and_social_state() {
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_disciple.after(setup_test_layer),
        ),
    );
    app.update();
    app.update();

    let disciple = only_spawned_npc(&mut app);

    assert!(app.world().get::<MissionExecuteState>(disciple).is_some());
    assert!(app.world().get::<SocializeState>(disciple).is_some());
    assert!(app.world().get::<FactionMembership>(disciple).is_some());
    let _thinker = app
        .world()
        .get::<ThinkerBuilder>(disciple)
        .expect("disciple should carry the live faction/social thinker");
}

#[test]
fn spawned_relic_guard_loadout_uses_injected_registry() {
    let mut baseline = App::new();
    baseline
        .insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    baseline.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_relic_guard.after(setup_test_layer),
        ),
    );
    baseline.update();
    baseline.update();
    let baseline_guard = only_spawned_npc(&mut baseline);
    assert!(
        baseline
            .world()
            .get::<crate::cultivation::known_techniques::KnownTechniques>(baseline_guard)
            .expect("baseline relic guard must carry a technique loadout")
            .entries
            .iter()
            .any(|entry| entry.id == "npc.heal_basic"),
        "默认 registry 下守卫必须持有 npc.heal_basic，否则排除断言恒真"
    );

    let registry =
        crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests_with_override(
            "npc.heal_basic",
            |definition| {
                definition.required_race = crate::body_plan::RaceGateOwned::Species {
                    species: vec![crate::body_plan::RaceId::new("whale")],
                };
            },
        );
    let mut app = App::new();
    app.insert_resource(registry);
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_relic_guard.after(setup_test_layer),
        ),
    );
    app.update();
    app.update();

    let guard = only_spawned_npc(&mut app);
    let known = app
        .world()
        .get::<crate::cultivation::known_techniques::KnownTechniques>(guard)
        .expect("spawned relic guard must carry a technique loadout");
    assert!(
        known
            .entries
            .iter()
            .all(|entry| entry.id != "npc.heal_basic"),
        "human relic guard must exclude runtime whale-only technique; loadout={:?}",
        known.entries
    );
}

#[test]
fn spawn_relic_guard_npc_at_attaches_guardian_trial_state() {
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_relic_guard.after(setup_test_layer),
        ),
    );
    app.update();
    app.update();

    let guard = only_spawned_npc(&mut app);

    assert!(app.world().get::<GuardState>(guard).is_some());
    assert!(app.world().get::<TrialState>(guard).is_some());
    assert!(app.world().get::<GuardianDuty>(guard).is_some());
    assert!(app.world().get::<TrialEval>(guard).is_some());
    let _thinker = app
        .world()
        .get::<ThinkerBuilder>(guard)
        .expect("relic guard should carry the live guardian thinker");
}

#[test]
fn spawn_relic_guard_npc_at_writes_spirit_realm_into_cultivation() {
    // plan-npc-realm-distribution-v1 P0 R1 pin：GuardianRelic 守护者的 `guard_realm`
    // 局部变量（disciple.rs:217 定义为 Realm::Spirit）此前只喂 npc_meridian_system_for_realm/
    // assign_npc_techniques，最后 npc_runtime_bundle 恒吞成 Realm::Awaken。
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_relic_guard.after(setup_test_layer),
        ),
    );
    app.update();
    app.update();

    let guard = only_spawned_npc(&mut app);

    assert_eq!(
        app.world().get::<Cultivation>(guard).map(|c| c.realm),
        Some(Realm::Spirit),
        "GuardianRelic 守护者 Cultivation.realm 期望 Spirit（guard_realm 定义于 disciple.rs:217）"
    );
}

fn only_spawned_npc(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
    let npcs = query.iter(world).collect::<Vec<_>>();
    assert_eq!(npcs.len(), 1);
    npcs[0]
}

fn spawn_test_beast(mut commands: Commands, layer: Res<TestLayer>) {
    beast::spawn_beast_npc_at(
        &mut commands,
        layer.0,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(40.0, 66.0, 40.0),
        Territory::new(DVec3::new(40.0, 66.0, 40.0), 30.0),
        0.0,
    );
}

fn spawn_test_disciple(
    mut commands: Commands,
    layer: Res<TestLayer>,
    technique_registry: Res<crate::cultivation::known_techniques::TechniqueRegistry>,
) {
    disciple::spawn_disciple_npc_at(
        &mut commands,
        &technique_registry,
        NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
        layer.0,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(42.0, 66.0, 42.0),
        DVec3::new(42.0, 66.0, 42.0),
        FactionId::Attack,
        FactionRank::Disciple,
        Realm::Awaken,
        None,
        0.0,
    );
}

fn spawn_test_relic_guard(
    mut commands: Commands,
    layer: Res<TestLayer>,
    technique_registry: Res<crate::cultivation::known_techniques::TechniqueRegistry>,
) {
    disciple::spawn_relic_guard_npc_at(
        &mut commands,
        &technique_registry,
        layer.0,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(44.0, 66.0, 44.0),
        24.0,
        "relic:test",
        "trial:test",
    );
}

/// Helper: build a real (non-fallback) SignedSkin for testing.
fn real_signed_skin(label: &str) -> SignedSkin {
    SignedSkin {
        value: format!("value-{label}"),
        signature: format!("sig-{label}"),
        source: crate::skin::SkinSource::MineSkinRandom {
            hash: label.to_string(),
        },
    }
}

/// Helper: create a SkinPool pre-loaded with real skins for every
/// PREFETCH_KEYS bucket so that `next_for_profile` returns a non-fallback
/// skin regardless of the resolved pool key.
fn pool_with_real_skins() -> SkinPool {
    let mut pool = SkinPool::default();
    for key in crate::skin::npc_skin_selector::NpcSkinPoolKey::PREFETCH_KEYS {
        for i in 0..3 {
            pool.insert_for_key(key, real_signed_skin(&format!("{}-{i}", key.as_str())));
        }
    }
    pool
}

#[test]
fn spawn_disciple_with_real_skin_attaches_npc_player_skin_and_player_kind() {
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_disciple_with_skin.after(setup_test_layer),
        ),
    );
    app.update();
    app.update();

    let disciple = only_spawned_npc(&mut app);

    let kind = *app
        .world()
        .get::<EntityKind>(disciple)
        .expect("disciple should have EntityKind");
    assert_eq!(
        kind,
        EntityKind::PLAYER,
        "disciple spawned with a real skin should use PLAYER entity kind, got {:?}",
        kind
    );

    let npc_skin = app
        .world()
        .get::<NpcPlayerSkin>(disciple)
        .expect("disciple spawned with real skin must have NpcPlayerSkin component");
    assert!(
        !npc_skin.skin.value.is_empty(),
        "NpcPlayerSkin.skin should have a non-empty value"
    );
    assert!(
        !npc_skin.name.is_empty(),
        "NpcPlayerSkin.name should be non-empty"
    );
}

#[test]
fn spawn_disciple_without_skin_pool_falls_back_to_villager() {
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_disciple.after(setup_test_layer),
        ),
    );
    app.update();
    app.update();

    let disciple = only_spawned_npc(&mut app);

    let kind = *app
        .world()
        .get::<EntityKind>(disciple)
        .expect("disciple should have EntityKind");
    assert_eq!(
        kind,
        EntityKind::VILLAGER,
        "disciple spawned without skin pool should fall back to VILLAGER, got {:?}",
        kind
    );

    assert!(
        app.world().get::<NpcPlayerSkin>(disciple).is_none(),
        "disciple spawned without skin pool must NOT have NpcPlayerSkin component"
    );
}

#[test]
fn spawn_disciple_with_empty_skin_pool_falls_back_to_villager() {
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_disciple_with_empty_pool.after(setup_test_layer),
        ),
    );
    app.update();
    app.update();

    let disciple = only_spawned_npc(&mut app);

    let kind = *app
        .world()
        .get::<EntityKind>(disciple)
        .expect("disciple should have EntityKind");
    assert_eq!(
            kind,
            EntityKind::VILLAGER,
            "disciple spawned with empty skin pool (WaitForReady) should fall back to VILLAGER because draw_npc_skin returns None, got {:?}",
            kind
        );

    assert!(
            app.world().get::<NpcPlayerSkin>(disciple).is_none(),
            "disciple spawned with empty skin pool must NOT have NpcPlayerSkin — draw_npc_skin returns None when pool not ready"
        );
}

fn spawn_test_disciple_with_skin(
    mut commands: Commands,
    layer: Res<TestLayer>,
    technique_registry: Res<crate::cultivation::known_techniques::TechniqueRegistry>,
) {
    let mut pool = pool_with_real_skins();
    disciple::spawn_disciple_npc_at(
        &mut commands,
        &technique_registry,
        NpcSkinSpawnContext::new(Some(&mut pool), NpcSkinFallbackPolicy::AllowFallback),
        layer.0,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(42.0, 66.0, 42.0),
        DVec3::new(42.0, 66.0, 42.0),
        FactionId::Attack,
        FactionRank::Disciple,
        Realm::Awaken,
        None,
        0.0,
    );
}

fn spawn_test_disciple_with_empty_pool(
    mut commands: Commands,
    layer: Res<TestLayer>,
    technique_registry: Res<crate::cultivation::known_techniques::TechniqueRegistry>,
) {
    let mut pool = SkinPool::default();
    disciple::spawn_disciple_npc_at(
        &mut commands,
        &technique_registry,
        NpcSkinSpawnContext::new(Some(&mut pool), NpcSkinFallbackPolicy::WaitForReady),
        layer.0,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(42.0, 66.0, 42.0),
        DVec3::new(42.0, 66.0, 42.0),
        FactionId::Attack,
        FactionRank::Disciple,
        Realm::Awaken,
        None,
        0.0,
    );
}

// -----------------------------------------------------------------------
// Rogue population seed — pure-function tests + full-stack spawn smoke
// -----------------------------------------------------------------------

fn mk_zone(name: &str, spirit_qi: f64, center: [f64; 3]) -> Zone {
    Zone {
        name: name.to_string(),
        dimension: crate::world::dimension::DimensionKind::Overworld,
        bounds: (
            DVec3::new(center[0] - 200.0, -64.0, center[2] - 200.0),
            DVec3::new(center[0] + 200.0, 320.0, center[2] + 200.0),
        ),
        spirit_qi,
        danger_level: 1,
        active_events: Vec::new(),
        patrol_anchors: vec![DVec3::new(center[0], center[1], center[2])],
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

#[test]
fn classify_zones_by_qi_partitions_at_threshold() {
    let zones = vec![
        mk_zone("high", 0.7, [0.0, 66.0, 0.0]),
        mk_zone("mid", 0.4, [10.0, 66.0, 0.0]),
        mk_zone("low", 0.1, [20.0, 66.0, 0.0]),
    ];
    let (resource, other) = classify_zones_by_qi(&zones, 0.4);
    assert_eq!(resource.len(), 2, "0.7 and 0.4 should be >= 0.4");
    assert_eq!(other.len(), 1);
    assert_eq!(other[0].name, "low");
}

#[test]
fn distribute_counts_evenly_spreads_remainder_to_first_buckets() {
    assert_eq!(distribute_counts_evenly(20, 3), vec![7, 7, 6]);
    assert_eq!(distribute_counts_evenly(80, 3), vec![27, 27, 26]);
    assert_eq!(distribute_counts_evenly(10, 10), vec![1; 10]);
    assert_eq!(distribute_counts_evenly(0, 3), vec![0, 0, 0]);
    assert_eq!(distribute_counts_evenly(5, 0), Vec::<u32>::new());
}

#[test]
fn initial_age_spreads_across_10_buckets() {
    let max_age = 100_000.0;
    let ages: Vec<f64> = (0..20)
        .map(|i| initial_age_for_index(i, max_age, 0.8))
        .collect();
    // Two full cycles of 10 buckets each.
    assert_eq!(ages[0], 0.0);
    assert!(ages[9] > 0.0);
    assert_eq!(ages[0], ages[10], "bucket should repeat at index 10");
    let max_age_produced = ages.iter().cloned().fold(0.0_f64, f64::max);
    assert!(max_age_produced <= max_age * 0.8 + 1e-9);
}

#[test]
fn seed_splits_20_rogues_80_20_across_zones() {
    let scenario = valence::testing::ScenarioSingleClient::new();
    let mut app = scenario.app;
    crate::world::dimension::mark_test_layer_as_overworld(&mut app);

    let mut zones = ZoneRegistry::fallback();
    // fallback gives us "spawn" @ qi=0.3; override to match prod-style mix.
    zones.zones[0].spirit_qi = 0.3;
    zones
        .zones
        .push(mk_zone("resource_a", 0.7, [1000.0, 70.0, 0.0]));
    zones
        .zones
        .push(mk_zone("resource_b", 0.5, [2000.0, 70.0, 0.0]));
    zones
        .zones
        .push(mk_zone("resource_c", 0.4, [3000.0, 70.0, 0.0]));
    zones
        .zones
        .push(mk_zone("other_a", 0.2, [0.0, 70.0, 5000.0]));
    app.insert_resource(zones);
    // Use a higher max so the registry doesn't block 20 spawns.
    let mut registry = NpcRegistry {
        max_npc_count: 200,
        resume_npc_count: 180,
        ..Default::default()
    };
    // Clear per_zone_caps so zone caps don't limit this test.
    registry.per_zone_caps.clear();
    app.insert_resource(registry);
    app.insert_resource(RoguePopulationSeedConfig::default());
    app.add_event::<NpcSpawnNotice>();
    app.add_systems(Update, rogue::seed_initial_rogue_population_on_startup);

    let rogue_seed_batch_size = 5u32;
    for _ in 0..(20 / rogue_seed_batch_size + 1) {
        app.update();
    }

    let by_archetype = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
        query.iter(world).copied().collect::<Vec<_>>()
    };
    assert_eq!(
        by_archetype
            .iter()
            .filter(|a| **a == NpcArchetype::Rogue)
            .count(),
        20,
        "should seed exactly 20 rogues (target_count default)"
    );

    // Sanity: 80% resource / 20% other — count by home_zone.
    let zone_counts: HashMap<String, u32> = {
        let world = app.world_mut();
        let mut counts: HashMap<String, u32> = HashMap::new();
        let mut query = world.query_filtered::<&NpcPatrol, With<NpcMarker>>();
        for patrol in query.iter(world) {
            *counts.entry(patrol.home_zone.clone()).or_insert(0) += 1;
        }
        counts
    };
    let resource_total: u32 = ["resource_a", "resource_b", "resource_c"]
        .iter()
        .map(|n| zone_counts.get(*n).copied().unwrap_or(0))
        .sum();
    let other_total: u32 = ["spawn", "other_a"]
        .iter()
        .map(|n| zone_counts.get(*n).copied().unwrap_or(0))
        .sum();
    assert_eq!(
        resource_total, 16,
        "80% of 20 should land in resource zones"
    );
    assert_eq!(other_total, 4, "20% of 20 should land in other zones");

    // Registry 已扣 20 配额。
    let registry = app.world().resource::<NpcRegistry>();
    assert_eq!(registry.live_npc_count, 20);
}

#[test]
fn seed_respects_disabled_config() {
    let scenario = valence::testing::ScenarioSingleClient::new();
    let mut app = scenario.app;
    app.insert_resource(ZoneRegistry::fallback());
    app.insert_resource(NpcRegistry::default());
    app.insert_resource(RoguePopulationSeedConfig {
        target_count: 0,
        ..RoguePopulationSeedConfig::default()
    });
    app.add_event::<NpcSpawnNotice>();
    app.add_systems(Update, rogue::seed_initial_rogue_population_on_startup);

    app.update();

    let count = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    assert_eq!(count, 0);
}

#[test]
fn seed_falls_back_to_other_zones_when_no_resource_qualifies() {
    let scenario = valence::testing::ScenarioSingleClient::new();
    let mut app = scenario.app;
    crate::world::dimension::mark_test_layer_as_overworld(&mut app);
    let mut zones = ZoneRegistry::fallback();
    zones.zones[0].spirit_qi = 0.1; // 强制 < 0.4 门槛，使其归入 "other"
    app.insert_resource(zones);
    let mut registry = NpcRegistry::default();
    // Clear per_zone_caps so zone caps don't limit this test.
    registry.per_zone_caps.clear();
    app.insert_resource(registry);
    app.insert_resource(RoguePopulationSeedConfig {
        target_count: 10,
        ..RoguePopulationSeedConfig::default()
    });
    app.add_event::<NpcSpawnNotice>();
    app.add_systems(Update, rogue::seed_initial_rogue_population_on_startup);

    // With batch_size=5, need at least 2 ticks for 10 NPCs.
    app.update();
    app.update();

    let count = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    assert_eq!(count, 10, "all 10 rogues should land in fallback zone");
    let home_zone = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&NpcPatrol, With<NpcMarker>>();
        query.iter(world).next().unwrap().home_zone.clone()
    };
    assert_eq!(home_zone, DEFAULT_SPAWN_ZONE_NAME);
}

/// plan-npc-realm-distribution-v1 P1: 活体种群产出实体 pin 测试（专属，不可用 dormant
/// hydrate 往返代替 —— `seed_initial_rogue_population_on_startup` 产出真实 entity 且不进
/// dormant 快照，是与 `dormant_rogue_seed_snapshot` 完全独立的第二条种群生产线）。
///
/// 起 App 跑该 system 到 `progress.done`，直接 query 产出实体的 `Cultivation.realm`：
/// ① 不再恒为 `Realm::Awaken`（推翻 P1 目标的最直接回归信号）
/// ② 分布落在 §8.1 #1 分布表容差区间内（醒灵占比，statistical pin 非精确计数）
/// ③ 化虚不自然刷
/// ④ 同 seed 两次跑该 system 逐实体 realm 一致（确定性）
/// Deliberately large bounds (2000-block half-extent, area far above the
/// PoissonSpawnSampler's `>= 500x500` adaptive tier) so a few hundred rogues
/// can be seeded without tripping zone-saturation skips — this test cares
/// about the *realm* distribution, not exercising Poisson packing limits.
fn mk_big_zone(name: &str, spirit_qi: f64, center: [f64; 3]) -> Zone {
    Zone {
        name: name.to_string(),
        dimension: crate::world::dimension::DimensionKind::Overworld,
        bounds: (
            DVec3::new(center[0] - 2000.0, -64.0, center[2] - 2000.0),
            DVec3::new(center[0] + 2000.0, 320.0, center[2] + 2000.0),
        ),
        spirit_qi,
        danger_level: 1,
        active_events: Vec::new(),
        patrol_anchors: vec![DVec3::new(center[0], center[1], center[2])],
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

fn seed_rogue_population_realm_test_zones() -> ZoneRegistry {
    ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            // background bucket: spirit_qi < 0.4 threshold.
            mk_big_zone("background_big", 0.3, [0.0, 66.0, 0.0]),
            // resource bucket: spirit_qi >= 0.4 threshold.
            mk_big_zone("resource_big", 0.7, [10_000.0, 66.0, 0.0]),
        ],
    }
}

fn run_rogue_seed_to_completion(target_count: u32) -> Vec<Realm> {
    let scenario = valence::testing::ScenarioSingleClient::new();
    let mut app = scenario.app;
    crate::world::dimension::mark_test_layer_as_overworld(&mut app);
    app.insert_resource(seed_rogue_population_realm_test_zones());
    let mut registry = NpcRegistry {
        max_npc_count: 2000,
        resume_npc_count: 1980,
        ..Default::default()
    };
    registry.per_zone_caps.clear();
    app.insert_resource(registry);
    app.insert_resource(RoguePopulationSeedConfig {
        target_count,
        ..RoguePopulationSeedConfig::default()
    });
    app.add_event::<NpcSpawnNotice>();
    app.add_systems(Update, rogue::seed_initial_rogue_population_on_startup);

    let rogue_seed_batch_size = 5u32;
    for _ in 0..(target_count / rogue_seed_batch_size + 1) {
        app.update();
    }

    let world = app.world_mut();
    let mut query = world.query_filtered::<&Cultivation, With<NpcMarker>>();
    query.iter(world).map(|c| c.realm).collect()
}

#[test]
fn seed_initial_rogue_population_produces_realm_distribution_not_all_awaken() {
    let target_count = 300u32;
    let realms = run_rogue_seed_to_completion(target_count);
    assert_eq!(
        realms.len(),
        target_count as usize,
        "must have spawned target_count rogues before sampling their realm distribution"
    );

    // ① Not all Awaken — this is the direct regression signal for the bug this
    // plan fixes (`Realm::Awaken` hardcoded at the spawn_scattered_cultivator_at
    // call site instead of sampling from the §8.1 #1 distribution table).
    let non_awaken = realms.iter().filter(|r| **r != Realm::Awaken).count();
    assert!(
        non_awaken > 0,
        "expected at least some non-Awaken realms among {} seeded live rogues, got 0 — \
             this means seed_initial_rogue_population_on_startup regressed back to hardcoding \
             Realm::Awaken instead of sampling crate::npc::dormant::sample_rogue_seed_realm",
        realms.len()
    );

    // ② statistical pin against §8.1 #1: default resource_fraction=0.8 blends
    // 80% resource-zone table (醒灵 42.5%) with 20% background-zone table
    // (醒灵 57%) => blended 醒灵 expectation ≈ 45.4%. Generous tolerance band
    // because this is a statistical (not exact-count) pin over N=300 samples.
    let awaken_count = realms.iter().filter(|r| **r == Realm::Awaken).count();
    let awaken_ratio = awaken_count as f64 / realms.len() as f64;
    assert!(
        (0.25..=0.65).contains(&awaken_ratio),
        "醒灵占比 {awaken_ratio:.3}（{awaken_count}/{}）偏离 §8.1 #1 长尾分布预期 \
             （0.8×42.5% + 0.2×57% ≈ 45.4%），容差区间 [0.25, 0.65] —— 分布表或 salt 可能被误改",
        realms.len()
    );

    // ③ 化虚不自然刷（正典稀有，仅垂死大能一类特殊实体走非分布表路径）。
    assert!(
        !realms.contains(&Realm::Void),
        "化虚是正典稀有实体，绝不应出现在自然散修种群 seeder 抽样结果里"
    );

    // ④ determinism: an independent App run with identical config must
    // reproduce the exact same per-slot realm sequence, otherwise realm
    // distribution silently drifts across server restarts.
    let realms_rerun = run_rogue_seed_to_completion(target_count);
    assert_eq!(
        realms, realms_rerun,
        "同 seed 两次跑 seed_initial_rogue_population_on_startup 必须逐 NPC 境界一致（确定性），\
             否则重启后境界分布漂移"
    );
}

#[test]
fn reproduction_processor_spawns_commoner_from_event_and_decrements_registry() {
    let scenario = valence::testing::ScenarioSingleClient::new();
    let mut app = scenario.app;
    crate::world::dimension::mark_test_layer_as_overworld(&mut app);
    app.add_event::<NpcReproductionRequest>();
    app.add_event::<NpcSpawnNotice>();
    app.insert_resource(NpcRegistry::default());
    app.add_systems(Update, process_npc_reproduction_requests);

    app.update();

    app.world_mut().send_event(NpcReproductionRequest {
        archetype: NpcArchetype::Commoner,
        position: DVec3::new(30.0, 66.0, 30.0),
        home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        initial_age_ticks: 0.0,
        territory_center: None,
        territory_radius: None,
    });

    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
        query.iter(world).copied().collect::<Vec<_>>()
    };
    assert_eq!(npcs, vec![NpcArchetype::Commoner]);

    let registry = app.world().resource::<NpcRegistry>();
    assert_eq!(
        registry.live_npc_count, 1,
        "reproduction must reserve one spawn slot from NpcRegistry"
    );
}

#[test]
fn reproduction_processor_dispatches_beast_request_with_territory_hint() {
    let scenario = valence::testing::ScenarioSingleClient::new();
    let mut app = scenario.app;
    crate::world::dimension::mark_test_layer_as_overworld(&mut app);
    app.add_event::<NpcReproductionRequest>();
    app.add_event::<NpcSpawnNotice>();
    app.insert_resource(NpcRegistry::default());
    app.add_systems(Update, process_npc_reproduction_requests);

    app.update();

    let center = DVec3::new(50.0, 66.0, 50.0);
    app.world_mut().send_event(NpcReproductionRequest {
        archetype: NpcArchetype::Beast,
        position: center,
        home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        initial_age_ticks: 0.0,
        territory_center: Some(center),
        territory_radius: Some(30.0),
    });

    app.update();

    let (arch, has_territory) = {
        let world = app.world_mut();
        let mut query =
            world.query_filtered::<(&NpcArchetype, Option<&Territory>), With<NpcMarker>>();
        let (arch, territory) = query.iter(world).next().expect("spawned beast");
        (*arch, territory.is_some())
    };
    assert_eq!(arch, NpcArchetype::Beast);
    assert!(has_territory, "beast reproduction must attach Territory");

    let registry = app.world().resource::<NpcRegistry>();
    assert_eq!(registry.live_npc_count, 1);
}

#[test]
fn reproduction_processor_skips_beast_request_without_territory_hint() {
    let scenario = valence::testing::ScenarioSingleClient::new();
    let mut app = scenario.app;
    app.add_event::<NpcReproductionRequest>();
    app.add_event::<NpcSpawnNotice>();
    app.insert_resource(NpcRegistry::default());
    app.add_systems(Update, process_npc_reproduction_requests);

    app.update();

    app.world_mut().send_event(NpcReproductionRequest {
        archetype: NpcArchetype::Beast,
        position: DVec3::new(30.0, 66.0, 30.0),
        home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        initial_age_ticks: 0.0,
        territory_center: None,
        territory_radius: None,
    });

    app.update();

    let npc_count = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    assert_eq!(npc_count, 0, "beast without territory hint must not spawn");

    let registry = app.world().resource::<NpcRegistry>();
    assert_eq!(
        registry.live_npc_count, 0,
        "budget must not be reserved on rejected beast request"
    );
}

#[test]
fn reproduction_processor_skips_unsupported_archetype() {
    let scenario = valence::testing::ScenarioSingleClient::new();
    let mut app = scenario.app;
    app.add_event::<NpcReproductionRequest>();
    app.add_event::<NpcSpawnNotice>();
    app.insert_resource(NpcRegistry::default());
    app.add_systems(Update, process_npc_reproduction_requests);

    app.update();

    app.world_mut().send_event(NpcReproductionRequest {
        archetype: NpcArchetype::Zombie,
        position: DVec3::new(30.0, 66.0, 30.0),
        home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        initial_age_ticks: 0.0,
        territory_center: None,
        territory_radius: None,
    });

    app.update();

    let npc_count = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    assert_eq!(npc_count, 0);

    let registry = app.world().resource::<NpcRegistry>();
    assert_eq!(registry.live_npc_count, 0);
}

#[test]
fn reproduction_processor_rejects_when_registry_budget_exhausted() {
    let scenario = valence::testing::ScenarioSingleClient::new();
    let mut app = scenario.app;
    app.add_event::<NpcReproductionRequest>();
    app.add_event::<NpcSpawnNotice>();
    let mut registry = NpcRegistry::default();
    registry.live_npc_count = registry.max_npc_count;
    registry.spawn_paused = true;
    app.insert_resource(registry);
    app.add_systems(Update, process_npc_reproduction_requests);

    app.update();

    app.world_mut().send_event(NpcReproductionRequest {
        archetype: NpcArchetype::Commoner,
        position: DVec3::new(30.0, 66.0, 30.0),
        home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        initial_age_ticks: 0.0,
        territory_center: None,
        territory_radius: None,
    });

    app.update();

    let npc_count = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    assert_eq!(npc_count, 0);
}

// -- Bug #2: snap_spawn_y_to_surface regression tests ------------------

#[test]
fn snap_spawn_y_above_ground_snaps_down() {
    use crate::world::terrain::{SurfaceInfo, SurfaceProvider};
    struct FlatGround;
    impl SurfaceProvider for FlatGround {
        fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
            SurfaceInfo {
                y: 66,
                passable: true,
                water_y: i32::MIN,
            }
        }
    }
    let terrain = FlatGround;
    let pos = DVec3::new(10.5, 200.0, 20.5);
    let snapped = common::snap_spawn_y_to_surface(pos, Some(&terrain));
    assert!(
        (snapped.y - 67.0).abs() < 0.01,
        "spawn at Y=200 should snap to surface_y+1=67, got {}",
        snapped.y,
    );
    assert!(
        (snapped.x - pos.x).abs() < 0.01 && (snapped.z - pos.z).abs() < 0.01,
        "XZ should be unchanged",
    );
}

#[test]
fn snap_spawn_y_below_ground_snaps_up() {
    use crate::world::terrain::{SurfaceInfo, SurfaceProvider};
    struct FlatGround;
    impl SurfaceProvider for FlatGround {
        fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
            SurfaceInfo {
                y: 66,
                passable: true,
                water_y: i32::MIN,
            }
        }
    }
    let terrain = FlatGround;
    let pos = DVec3::new(5.0, 10.0, 5.0);
    let snapped = common::snap_spawn_y_to_surface(pos, Some(&terrain));
    assert!(
        (snapped.y - 67.0).abs() < 0.01,
        "spawn at Y=10 should snap to surface_y+1=67, got {}",
        snapped.y,
    );
}

#[test]
fn snap_spawn_y_impassable_surface_keeps_original() {
    use crate::world::terrain::{SurfaceInfo, SurfaceProvider};
    struct LavaSurface;
    impl SurfaceProvider for LavaSurface {
        fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
            SurfaceInfo {
                y: 66,
                passable: false,
                water_y: i32::MIN,
            }
        }
    }
    let terrain = LavaSurface;
    let pos = DVec3::new(5.0, 80.0, 5.0);
    let snapped = common::snap_spawn_y_to_surface(pos, Some(&terrain));
    assert!(
        (snapped.y - 80.0).abs() < 0.01,
        "impassable surface should keep original Y=80, got {}",
        snapped.y,
    );
}

#[test]
fn snap_spawn_y_no_terrain_keeps_original() {
    let pos = DVec3::new(5.0, 80.0, 5.0);
    let snapped =
        common::snap_spawn_y_to_surface(pos, None::<&crate::world::terrain::TerrainProvider>);
    assert!(
        (snapped.y - 80.0).abs() < 0.01,
        "no terrain provider should keep original Y=80, got {}",
        snapped.y,
    );
}

#[test]
fn startup_zombie_removed() {
    // Verify that spawn::register() no longer schedules PostStartup zombie spawn.
    // We only call spawn::register() (not brain::register()) to avoid needing
    // all the events that brain systems require.
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.insert_resource(crate::qi_physics::WorldQiAccount::default());
    app.add_event::<crate::qi_physics::QiTransfer>();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    crate::npc::lifecycle::register(&mut app);
    app.insert_resource(RoguePopulationSeedConfig {
        target_count: 0,
        ..RoguePopulationSeedConfig::default()
    });
    // Manually register spawn systems without using register() to avoid
    // BigBrainPlugin dependency. Instead, verify the register() source.
    app.add_event::<crate::npc::lifecycle::NpcSpawnNotice>();
    app.add_systems(
        Update,
        (
            process_npc_reproduction_requests,
            rogue::seed_initial_rogue_population_on_startup,
        ),
    );

    app.update(); // PostStartup
    app.update(); // First Update
    app.update(); // Extra tick

    let npc_count = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    // With target_count=0 and no PostStartup zombie, no NPC should exist.
    assert_eq!(
        npc_count, 0,
        "after register() + 3 ticks, no zombie should spawn (P1.4 removal); got {}",
        npc_count
    );
}

#[test]
fn batch_size_default_5() {
    // ROGUE_SEED_BATCH_SIZE is private, but we can observe the batching behavior:
    // with target=10, if batch_size=5, it takes exactly 2 ticks.
    let scenario = valence::testing::ScenarioSingleClient::new();
    let mut app = scenario.app;
    crate::world::dimension::mark_test_layer_as_overworld(&mut app);
    app.insert_resource(ZoneRegistry::fallback());
    let mut registry = NpcRegistry::default();
    registry.per_zone_caps.clear(); // Don't let zone caps interfere.
    app.insert_resource(registry);
    app.insert_resource(RoguePopulationSeedConfig {
        target_count: 10,
        ..RoguePopulationSeedConfig::default()
    });
    app.add_event::<crate::npc::lifecycle::NpcSpawnNotice>();
    app.add_systems(Update, rogue::seed_initial_rogue_population_on_startup);

    // Tick 1: should spawn exactly 5 (batch_size=5).
    app.update();
    let count_after_1 = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    assert_eq!(
        count_after_1, 5,
        "after 1 tick with target=10, batch_size=5 should spawn 5; got {}",
        count_after_1
    );

    // Tick 2: should spawn remaining 5.
    app.update();
    let count_after_2 = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    assert_eq!(
        count_after_2, 10,
        "after 2 ticks with target=10, batch_size=5 should total 10; got {}",
        count_after_2
    );
}

// ─────────────────────────────────────────────────────────────────────────
// plan-npc-realm-distribution-v1 P2 — 境界-功法-视觉单一来源一致性 audit
//
// P0 之前的 bug 形态是「意图 realm（喂进 assign_npc_techniques /
// select_npc_visual_profile 的局部变量）≠ 组件 realm（npc_runtime_bundle_with_age
// 恒吞成的 Cultivation::default()）」。P0 修完 choke point 后两者理论上已收敛到
// 同一个函数参数，但那只是「输入端不再分叉」——本审计从真实 spawn 出的 entity
// 上*读回* Cultivation.realm，再拿它去核对同一个 entity 上落地的 KnownTechniques /
// NpcVisualProfile，验证的是「组件落地结果」的一致性而非「调用参数」的一致性，
// 两者不是同一件事：只审计参数传递不会抓到「entity 落地后又被别的 system 悄悄
// 改写 Cultivation.realm 但没有联动重算功法/视觉」这类漂移。
//
// 覆盖：spawn_rogue_npc_at / spawn_scattered_cultivator_at / spawn_disciple_npc_at
// / spawn_commoner_npc_at，全 6 境界。GuardianRelic（spawn_relic_guard_npc_at）
// 走固定 Realm::Spirit 单点，已有专属 pin
// (spawn_relic_guard_npc_at_writes_spirit_realm_into_cultivation)，本审计不重复。

fn spawn_test_realm_audit_population(
    mut commands: Commands,
    layer: Res<TestLayer>,
    technique_registry: Res<crate::cultivation::known_techniques::TechniqueRegistry>,
) {
    let realms = [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ];
    for (i, realm) in realms.into_iter().enumerate() {
        let x = 200.0 + i as f64 * 10.0;
        rogue::spawn_rogue_npc_at(
            &mut commands,
            &technique_registry,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(x, 66.0, 200.0),
            DVec3::new(x, 66.0, 200.0),
            realm,
            0.0,
        );
        rogue::spawn_scattered_cultivator_at(
            &mut commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(x, 66.0, 210.0),
            DVec3::new(x, 66.0, 210.0),
            0.5,
            realm,
            0.0,
        );
        disciple::spawn_disciple_npc_at(
            &mut commands,
            &technique_registry,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(x, 66.0, 220.0),
            DVec3::new(x, 66.0, 220.0),
            FactionId::Attack,
            FactionRank::Disciple,
            realm,
            None,
            0.0,
        );
        commoner::spawn_commoner_npc_at(
            &mut commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(x, 66.0, 230.0),
            DVec3::new(x, 66.0, 230.0),
            realm,
            0.0,
        );
    }
}

#[test]
fn spawn_paths_technique_realm_never_exceeds_persisted_cultivation_realm() {
    use crate::cultivation::known_techniques::KnownTechniques;
    use crate::npc::technique::technique_realm_satisfied;

    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_realm_audit_population.after(setup_test_layer),
        ),
    );
    app.update();
    app.update();

    let technique_registry = app
        .world()
        .resource::<crate::cultivation::known_techniques::TechniqueRegistry>()
        .clone();
    let world = app.world_mut();
    let mut query = world.query_filtered::<(
        Entity,
        &Cultivation,
        Option<&KnownTechniques>,
        &NpcArchetype,
    ), With<NpcMarker>>();
    let mut checked_entities = 0usize;
    let mut checked_techniques = 0usize;
    for (entity, cultivation, known, archetype) in query.iter(world) {
        checked_entities += 1;
        let Some(known) = known else { continue };
        for entry in &known.entries {
            checked_techniques += 1;
            let def = technique_registry.get(&entry.id).unwrap_or_else(|| {
                panic!(
                    "entity={entity:?} archetype={archetype:?} realm={:?}: technique {} \
                         not found in TechniqueRegistry",
                    cultivation.realm, entry.id
                )
            });
            assert!(
                technique_realm_satisfied(def, cultivation.realm),
                "entity={entity:?} archetype={archetype:?}: persisted Cultivation.realm={:?} \
                     does not satisfy technique {} (required_realm={}) — 意图 realm 与组件落地 \
                     realm 出现双源漂移",
                cultivation.realm,
                entry.id,
                def.required_realm
            );
        }
    }

    // 防止 query 因 filter 打偏而恒真（entities=0 或 techniques=0 都会让上面的
    // assert 循环体一次不跑，测试看似通过实则没测到任何东西）。
    assert_eq!(
        checked_entities, 24,
        "expected 4 spawn fns x 6 realms = 24 NpcMarker entities, got {checked_entities} — \
             spawn population fixture 本身跑偏，下面的 realm 断言可能从未真正执行"
    );
    assert!(
        checked_techniques > 0,
        "expected at least one KnownTechniques entry across the 24 spawned entities — got 0, \
             the audit loop body never actually ran an assertion"
    );
}

#[test]
fn spawn_paths_visual_profile_matches_persisted_cultivation_realm() {
    use crate::skin::{select_npc_visual_profile, NpcVisualProfile};

    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.add_systems(
        valence::prelude::Startup,
        (
            setup_test_layer,
            spawn_test_realm_audit_population.after(setup_test_layer),
        ),
    );
    app.update();
    app.update();

    let world = app.world_mut();
    let mut query = world.query_filtered::<(
        Entity,
        &Cultivation,
        &NpcArchetype,
        Option<&NpcVisualProfile>,
        Option<&FactionMembership>,
    ), With<NpcMarker>>();
    let mut checked_profiles = 0usize;
    for (entity, cultivation, archetype, profile, membership) in query.iter(world) {
        let Some(profile) = profile else { continue };
        checked_profiles += 1;
        let faction_id = membership.map(|m| m.faction_id);
        let faction_rank = membership.map(|m| m.rank);
        // age_ratio 不影响 skin_tier/high_realm，只影响 age_band——用一个固定值
        // 重算，只比对 realm 派生的两个字段，不比对完整 struct。
        let recomputed =
            select_npc_visual_profile(*archetype, cultivation.realm, faction_id, faction_rank, 0.5);
        assert_eq!(
            profile.skin_tier, recomputed.skin_tier,
            "entity={entity:?} archetype={archetype:?}: NpcVisualProfile.skin_tier 落地值 {:?} \
                 与「用 entity 上实际 Cultivation.realm={:?} 重算」得到的 {:?} 不一致 — \
                 视觉档位吃的 realm 与组件最终落地的 realm 出现双源漂移",
            profile.skin_tier, cultivation.realm, recomputed.skin_tier
        );
        assert_eq!(
            profile.has_high_realm_aura(),
            recomputed.has_high_realm_aura(),
            "entity={entity:?} archetype={archetype:?}: NpcVisualProfile.high_realm 落地值 {} \
                 与用实际 Cultivation.realm={:?} 重算的 {} 不一致",
            profile.has_high_realm_aura(),
            cultivation.realm,
            recomputed.has_high_realm_aura()
        );
    }

    assert_eq!(
        checked_profiles, 24,
        "expected all 24 spawned entities (Rogue/ScatteredCultivator/Disciple/Commoner all \
             carry NpcVisualProfile) to be checked, got {checked_profiles} — fixture 跑偏或 \
             NpcVisualProfile 未落地，下面的 realm 一致性断言可能从未真正执行"
    );
}
