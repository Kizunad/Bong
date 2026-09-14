#![allow(dead_code, unused_imports)]

use std::collections::{BTreeMap, HashSet};
use valence::client::ClientMarker;
use valence::prelude::*;

use bong_server::combat::components::Lifecycle;
use bong_server::cultivation::components::{Contamination, Cultivation, MeridianSystem};
use bong_server::cultivation::life_record::LifeRecord;
use bong_server::cultivation::lifespan::{
    DeathRegistry, LifespanComponent, LifespanExtensionLedger,
};
use bong_server::cultivation::meridian::severed::MeridianSeveredPermanent;
use bong_server::cultivation::tribulation::{
    du_xu_prereqs_met, HalfStepRechallengeTriggerEvent, InitiateXuhuaTribulation,
};
#[cfg(test)]
use bong_server::fauna::daozhan::FakeBehavior;
use bong_server::fauna::daozhan::{DaoZhangBehaviorBlackboard, DaoZhangState};
use bong_server::fauna::dying_elder::DyingElderBlackboard;
use bong_server::fauna::mimic_spider::{
    MimicSpiderBlackboard, SpiderDisguiseState, SpiderTrapPotential,
};
use bong_server::npc::dormant::{
    durable_npc_identity_error, dvec3_from_array, planar_distance, vec3_to_array,
    DormantBehaviorIntent, DormantDaoxiangOriginSnapshot, DormantDaozhanSnapshot,
    DormantFuyaAuraSnapshot, DormantGuardianRelicSnapshot, DormantMimicSpiderSnapshot,
    DormantPatrolSnapshot, DormantTsyHostileSnapshot, DormantTsySentinelSnapshot,
    DormantZhinianPhase, NpcDormantSnapshot, NpcDormantStore, NpcVirtualizationConfig,
};
use bong_server::npc::faction::{FactionMembership, FactionRank};
use bong_server::npc::interaction_memory::NpcMemoryComponent;
use bong_server::npc::lifecycle::{NpcArchetype, NpcLifespan, NpcRegistry};
use bong_server::npc::lod::NpcLodTier;
use bong_server::npc::loot::{default_loot_for_archetype, NpcLootTable};
use bong_server::npc::movement::GameTick;
use bong_server::npc::patrol::NpcPatrol;
use bong_server::npc::relic::{GuardianDuty, TrialEval};
use bong_server::npc::scenario::ScenarioNpc;
use bong_server::npc::schedule::{
    home_base_for_archetype, hydrate_position_for, schedule_seed_from_char_id, NpcDailySchedule,
};
use bong_server::npc::spawn::{NpcMarker, NpcSkinSpawnContext};
use bong_server::npc::spawn_rat::RatBlackboard;
use bong_server::npc::spawn_spider::spawn_ash_spider_npc_at;
use bong_server::npc::territory::Territory;
use bong_server::npc::trade::NpcPlayerReputation;
use bong_server::npc::tsy_hostile::{
    spawn_tsy_daoxiang_at, spawn_tsy_fuya_at, spawn_tsy_sentinel_at, spawn_tsy_skull_fiend_at,
    spawn_tsy_zhinian_at, FuyaAura, TsyHostileMarker, TsySentinelMarker, ZhinianMind, ZhinianPhase,
};
use bong_server::skin::{NpcSkinFallbackPolicy, SkinPool};
use bong_server::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use bong_server::world::era::WorldEraState;
use bong_server::world::poi_novice::PoiNoviceRegistry;
use bong_server::world::tsy_container::LootContainer;
use bong_server::world::tsy_lifecycle::DaoxiangOrigin;
use bong_server::world::zone::ZoneRegistry;

use bong_server::npc::hydrate::*;
use valence::prelude::*;

use bong_server::cultivation::components::Realm;
use bong_server::fauna::visual::FaunaVisualKind;
use bong_server::npc::dormant::{DEHYDRATE_RADIUS_BLOCKS, HYDRATE_RADIUS_BLOCKS};
use bong_server::npc::interaction_memory::{
    NpcInteractionOutcome, NpcInteractionType, NpcMemoryEntry,
};
use bong_server::npc::trade::RepTier;
use bong_server::world::tsy_container::ContainerKind;
use bong_server::world::zone::{TsyDepth, Zone, DEFAULT_SPAWN_ZONE_NAME};
fn zone_registry() -> ZoneRegistry {
    ZoneRegistry {
        spatial_revision: 0,
        zones: vec![Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 128.0, 100.0)),
            spirit_qi: 0.8,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(10.0, 64.0, 10.0)],
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }],
    }
}

fn attack_memory_for(player_uuid: &str, timestamp: u64) -> NpcMemoryComponent {
    let mut memory = NpcMemoryComponent::default();
    memory.remember(NpcMemoryEntry {
        player_uuid: player_uuid.to_string(),
        interaction_type: NpcInteractionType::Attack,
        timestamp,
        outcome: NpcInteractionOutcome::Harmed,
    });
    memory
}

fn high_reputation_for(player_uuid: &str) -> NpcPlayerReputation {
    let mut reputation = NpcPlayerReputation::default();
    reputation.adjust(player_uuid, 0.3);
    reputation
}

#[test]
fn dehydrate_snapshot_prefers_zone_name_from_registry() {
    let registry = zone_registry();
    let zone_name = registry
        .find_zone(DimensionKind::Overworld, DVec3::new(10.0, 64.0, 10.0))
        .unwrap()
        .name
        .clone();
    assert_eq!(zone_name, DEFAULT_SPAWN_ZONE_NAME);
}

#[test]
fn dehydrate_refuses_rat_until_rat_specific_snapshot_exists() {
    let mut app = App::new();
    app.insert_resource(NpcDormantStore::default());
    app.insert_resource(NpcVirtualizationConfig {
        transition_interval_ticks: 1,
        dehydrate_without_players: true,
        ..Default::default()
    });
    app.add_systems(Update, dehydrate_far_npcs_system);

    let entity = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position(DVec3::new(10.0, 64.0, 10.0)),
            Lifecycle {
                character_id: "npc_rat_far".to_string(),
                ..Default::default()
            },
            LifeRecord::new("npc_rat_far"),
            DeathRegistry::new("npc_rat_far"),
            NpcArchetype::Beast,
            NpcLifespan::new(0.0, 1_000.0),
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 10.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
            RatBlackboard::new("spawn", valence::prelude::ChunkPos::new(0, 0)),
        ))
        .id();

    app.update();

    assert!(
        !app.world()
            .resource::<NpcDormantStore>()
            .contains("npc_rat_far"),
        "rat without a lossless rat-specific dormant format must remain live"
    );
    assert!(
        app.world().get::<RatBlackboard>(entity).is_some(),
        "refused rat dehydration must retain the behavior and reserve mirror carrier"
    );
    assert!(
        app.world().get::<Despawned>(entity).is_none(),
        "refused rat dehydration must not mark the entity Despawned"
    );
}

#[test]
fn dehydrate_requires_matching_canonical_durable_owner_components() {
    for (case, lifecycle_id, life_record_id, death_registry_id) in [
        ("missing_life_record", "npc_owner", None, Some("npc_owner")),
        (
            "missing_death_registry",
            "npc_owner",
            Some("npc_owner"),
            None,
        ),
        (
            "mismatched_life_record",
            "npc_owner",
            Some("npc_other"),
            Some("npc_owner"),
        ),
        (
            "mismatched_death_registry",
            "npc_owner",
            Some("npc_owner"),
            Some("npc_other"),
        ),
        ("blank", "", Some(""), Some("")),
        (
            "unassigned",
            "unassigned:life_record",
            Some("unassigned:life_record"),
            Some("unassigned:life_record"),
        ),
        (
            "surrounding_whitespace",
            " npc_owner ",
            Some(" npc_owner "),
            Some(" npc_owner "),
        ),
    ] {
        let mut app = App::new();
        app.insert_resource(NpcDormantStore::default());
        app.insert_resource(NpcVirtualizationConfig {
            transition_interval_ticks: 1,
            dehydrate_without_players: true,
            ..Default::default()
        });
        app.add_systems(Update, dehydrate_far_npcs_system);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(DVec3::new(10.0, 64.0, 10.0)),
                Lifecycle {
                    character_id: lifecycle_id.to_string(),
                    ..Default::default()
                },
                NpcArchetype::Rogue,
                NpcLifespan::new(0.0, 1_000.0),
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
            ))
            .id();
        if let Some(character_id) = life_record_id {
            app.world_mut()
                .entity_mut(entity)
                .insert(LifeRecord::new(character_id));
        }
        if let Some(character_id) = death_registry_id {
            app.world_mut()
                .entity_mut(entity)
                .insert(DeathRegistry::new(character_id));
        }

        app.update();

        assert!(
            app.world().resource::<NpcDormantStore>().is_empty(),
            "case={case}: invalid identity tuple must not enter durable dormant storage"
        );
        assert!(
            app.world().get::<Despawned>(entity).is_none(),
            "case={case}: failed identity preflight must leave the live NPC intact"
        );
    }
}

#[test]
fn dehydrate_marks_live_npc_despawned_for_valence_layer_cleanup() {
    let mut app = App::new();
    app.insert_resource(NpcDormantStore::default());
    app.insert_resource(NpcVirtualizationConfig {
        transition_interval_ticks: 1,
        dehydrate_without_players: true,
        ..Default::default()
    });
    app.add_systems(Update, dehydrate_far_npcs_system);

    let lifecycle = Lifecycle {
        character_id: "npc_far".to_string(),
        ..Default::default()
    };
    let entity = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position(DVec3::new(10.0, 64.0, 10.0)),
            lifecycle,
            LifeRecord::new("npc_far"),
            DeathRegistry::new("npc_far"),
            NpcArchetype::Rogue,
            NpcDailySchedule::for_archetype(NpcArchetype::Rogue, 42),
            NpcLifespan::new(0.0, 1_000.0),
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 10.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
        ))
        .id();

    app.update();

    assert!(app
        .world()
        .resource::<NpcDormantStore>()
        .contains("npc_far"));
    assert_eq!(
        app.world()
            .resource::<NpcDormantStore>()
            .snapshots
            .get("npc_far")
            .and_then(|snapshot| snapshot.schedule_seed),
        Some(42)
    );
    assert!(
        app.world().get::<Despawned>(entity).is_some(),
        "dehydrated NPC must be marked Despawned instead of raw despawned"
    );
}

#[test]
fn dehydrate_keeps_transient_scenario_npc_live_and_out_of_dormant_store() {
    let mut app = App::new();
    app.insert_resource(NpcDormantStore::default());
    app.insert_resource(NpcVirtualizationConfig {
        transition_interval_ticks: 1,
        dehydrate_without_players: true,
        ..Default::default()
    });
    app.add_systems(Update, dehydrate_far_npcs_system);

    let entity = app
        .world_mut()
        .spawn((
            NpcMarker,
            ScenarioNpc,
            Position(DVec3::new(10.0, 64.0, 10.0)),
            Lifecycle {
                character_id: "npc_scenario_far".to_string(),
                ..Default::default()
            },
            LifeRecord::new("npc_scenario_far"),
            DeathRegistry::new("npc_scenario_far"),
            NpcArchetype::Rogue,
            NpcDailySchedule::for_archetype(NpcArchetype::Rogue, 42),
            NpcLifespan::new(0.0, 1_000.0),
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 10.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
        ))
        .id();

    app.update();

    assert!(
        !app.world()
            .resource::<NpcDormantStore>()
            .contains("npc_scenario_far"),
        "scenario identity is transient and must never be serialized as a generic dormant zombie"
    );
    assert!(app.world().get::<ScenarioNpc>(entity).is_some());
    assert!(
        app.world().get::<Despawned>(entity).is_none(),
        "scenario NPC must remain live for the explicit scenario clear/terminal lifecycle"
    );
}

#[test]
fn dehydrate_snapshot_carries_npc_memory_and_player_reputation() {
    const PLAYER_ID: &str = "player-memory-1";

    let mut app = App::new();
    app.insert_resource(NpcDormantStore::default());
    app.insert_resource(NpcVirtualizationConfig {
        transition_interval_ticks: 1,
        dehydrate_without_players: true,
        ..Default::default()
    });
    app.add_systems(Update, dehydrate_far_npcs_system);

    let memory = attack_memory_for(PLAYER_ID, 77);
    let reputation = high_reputation_for(PLAYER_ID);
    app.world_mut().spawn((
        NpcMarker,
        Position(DVec3::new(10.0, 64.0, 10.0)),
        Lifecycle {
            character_id: "npc_memory_rep".to_string(),
            ..Default::default()
        },
        LifeRecord::new("npc_memory_rep"),
        DeathRegistry::new("npc_memory_rep"),
        NpcArchetype::Rogue,
        NpcDailySchedule::for_archetype(NpcArchetype::Rogue, 42),
        NpcLifespan::new(0.0, 1_000.0),
        Cultivation {
            realm: Realm::Awaken,
            qi_current: 10.0,
            qi_max: 100.0,
            ..Default::default()
        },
        MeridianSystem::default(),
        Contamination::default(),
        memory.clone(),
        reputation,
    ));

    app.update();

    let snapshot = app
        .world()
        .resource::<NpcDormantStore>()
        .snapshots
        .get("npc_memory_rep")
        .expect("dehydrated NPC should have a dormant snapshot");
    assert_eq!(
        snapshot
            .memory
            .as_ref()
            .expect("non-empty memory should be carried")
            .interactions,
        memory.interactions
    );
    assert_eq!(
        snapshot
            .player_reputation
            .as_ref()
            .expect("non-empty reputation should be carried")
            .tier(PLAYER_ID),
        RepTier::High
    );
}

fn spawn_loot_container(app: &mut App, family_id: &str, pos: DVec3) -> Entity {
    app.world_mut()
        .spawn((
            LootContainer::new(
                ContainerKind::RelicCore,
                family_id.to_string(),
                TsyDepth::Deep,
                "relic_core_deep".to_string(),
                0,
            ),
            Position(pos),
        ))
        .id()
}

/// P0/P1 pin：dehydrate 一个带 `TsySentinelMarker` 的活体秘境守灵，断言写入
/// `NpcDormantStore` 的快照 `tsy_sentinel` 字段为 `Some`，且携带
/// `guarding_container_pos`/`phase`/`max_phase`——修复前 `dormant_tsy_hostile_snapshot`
/// 完全不读 `TsySentinelMarker`，这个字段恒为不存在（编译期都没有），身份从
/// dehydrate 这一步就已经丢失。
#[test]
fn tsy_sentinel_dehydrates_with_sentinel_identity_payload() {
    let mut app = App::new();
    app.insert_resource(NpcDormantStore::default());
    app.insert_resource(NpcVirtualizationConfig {
        transition_interval_ticks: 1,
        dehydrate_without_players: true,
        ..Default::default()
    });
    app.add_systems(Update, dehydrate_far_npcs_system);

    let container = spawn_loot_container(&mut app, "tsy_lingxu_01", DVec3::new(20.0, 64.0, 20.0));

    let lifecycle = Lifecycle {
        character_id: "npc_sentinel_far".to_string(),
        ..Default::default()
    };
    let sentinel_entity = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position(DVec3::new(12.0, 64.0, 12.0)),
            lifecycle,
            LifeRecord::new("npc_sentinel_far"),
            DeathRegistry::new("npc_sentinel_far"),
            NpcArchetype::GuardianRelic,
            NpcDailySchedule::for_archetype(NpcArchetype::GuardianRelic, 7),
            NpcLifespan::new(0.0, 1_000.0),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 500.0,
                qi_max: 1000.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
            TsyHostileMarker {
                family_id: "tsy_lingxu_01".to_string(),
            },
            TsySentinelMarker {
                family_id: "tsy_lingxu_01".to_string(),
                guarding_container: Some(container),
                phase: 1,
                max_phase: 3,
            },
        ))
        .id();

    app.update();

    let store = app.world().resource::<NpcDormantStore>();
    let snap = store
        .snapshots
        .get("npc_sentinel_far")
        .expect("sentinel snapshot must exist in NpcDormantStore after dehydrate");
    let sentinel = snap.tsy_sentinel.as_ref().expect(
        "dehydrated TSY sentinel must carry Some(tsy_sentinel) payload — regression: \
         dormant_tsy_hostile_snapshot never read TsySentinelMarker, so this field would \
         not exist pre-fix and the sentinel silently degrades to a plain GuardianRelic",
    );
    assert_eq!(
        sentinel.guarding_container_pos,
        Some([20.0, 64.0, 20.0]),
        "guarding_container_pos must capture the guarded container's live Position at dehydrate time"
    );
    assert_eq!(
        sentinel.phase, 1,
        "phase must be copied verbatim from the live TsySentinelMarker (best-effort — \
         update_sentinel_phase_system corrects it against real Wounds next tick, §8.1 #2)"
    );
    assert_eq!(
        sentinel.max_phase, 3,
        "max_phase (design constant) must be preserved exactly"
    );
    assert!(
        app.world().get::<Despawned>(sentinel_entity).is_some(),
        "dehydrated sentinel entity must be marked Despawned"
    );
}

#[test]
fn dehydrate_refuses_spider_with_ephemeral_trap_owner() {
    let mut app = App::new();
    app.insert_resource(NpcDormantStore::default());
    app.insert_resource(NpcVirtualizationConfig {
        transition_interval_ticks: 1,
        dehydrate_without_players: true,
        ..Default::default()
    });
    app.add_systems(Update, dehydrate_far_npcs_system);
    let owner = app.world_mut().spawn_empty().id();
    let spider = app.world_mut().spawn_empty().id();
    let bundle =
        bong_server::npc::lifecycle::npc_runtime_bundle(spider, NpcArchetype::Beast, Realm::Awaken);
    let mut blackboard = MimicSpiderBlackboard::new("spawn", DVec3::ZERO);
    blackboard.trapped_by = Some(owner);
    app.world_mut().entity_mut(spider).insert((
        NpcMarker,
        Position(DVec3::new(10.0, 64.0, 10.0)),
        SpiderDisguiseState::Disguised,
        blackboard,
        SpiderTrapPotential {
            trap_owner: owner,
            placed_at: DVec3::ZERO,
            placed_tick: 0,
        },
        bundle,
    ));

    app.update();

    assert!(app.world().resource::<NpcDormantStore>().is_empty());
    assert!(app.world().get::<Despawned>(spider).is_none());
    assert!(app.world().get::<MimicSpiderBlackboard>(spider).is_some());
}

#[test]
fn dehydrate_rejects_invalid_daozhan_qi_and_missing_tsy_identity() {
    for (case, daozhan_qi, include_tsy_marker) in [
        ("negative", -1.0, true),
        ("nan", f64::NAN, true),
        ("positive_infinity", f64::INFINITY, true),
        ("missing_tsy_marker", 1.0, false),
    ] {
        let mut app = App::new();
        app.insert_resource(NpcDormantStore::default());
        app.insert_resource(NpcVirtualizationConfig {
            transition_interval_ticks: 1,
            dehydrate_without_players: true,
            ..Default::default()
        });
        app.add_systems(Update, dehydrate_far_npcs_system);
        let entity = app.world_mut().spawn_empty().id();
        let bundle = bong_server::npc::lifecycle::npc_runtime_bundle(
            entity,
            NpcArchetype::GuardianRelic,
            Realm::Awaken,
        );
        let mut blackboard = DaoZhangBehaviorBlackboard::new("tsy_lingxu_01", DVec3::ZERO, None);
        blackboard.daozhan_qi = daozhan_qi;
        app.world_mut().entity_mut(entity).insert((
            NpcMarker,
            Position(DVec3::new(10.0, 64.0, 10.0)),
            DaoZhangState::Mimicry,
            blackboard,
            bundle,
        ));
        if include_tsy_marker {
            app.world_mut().entity_mut(entity).insert(TsyHostileMarker {
                family_id: "tsy_lingxu_01".to_string(),
            });
        }

        app.update();

        assert!(
            app.world().resource::<NpcDormantStore>().is_empty(),
            "case={case}: invalid Daozhan owner must not enter durable storage"
        );
        assert!(
            app.world().get::<Despawned>(entity).is_none(),
            "case={case}: failed preflight must preserve the live behavior carrier"
        );
    }
}
